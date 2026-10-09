#!/usr/bin/env bash
set -euo pipefail

usage() {
    cat <<'EOF'
Usage:
  ./scripts/cloudflare-full-local.sh up
  ./scripts/cloudflare-full-local.sh down
  ./scripts/cloudflare-full-local.sh test
  ./scripts/cloudflare-full-local.sh test-direct
  ./scripts/cloudflare-full-local.sh up-direct
  ./scripts/cloudflare-full-local.sh logs

Environment overrides:
  CZAR_CLOUDFLARE_FULL_PORT   Full app port (default: 8787)
  CZAR_CLOUDFLARE_D1_PORT     Local D1 harness port (default: 8788)
  CZAR_CLOUDFLARE_DIRECT_PORT Direct app port (default: 18080)
  CZAR_CLOUDFLARE_HOST_IP     Host IP visible from Docker
  CZAR_CLOUDFLARE_STATE_DIR   State dir (default: /tmp/czar-cloudflare-full-local)
  CZAR_FULL_BASIC_AUTH_USER   Basic auth username (default: czar)
  CZAR_FULL_BASIC_AUTH_PASS   Basic auth password (default: czar)
  CZAR_D1_ACCOUNT_ID          D1 account id (default: local-account)
  CZAR_D1_DATABASE_ID         D1 database id (default: local-database)
  CZAR_D1_API_TOKEN           D1 API token (default: test-token)
EOF
}

action="${1:-up}"
case "$action" in
    up|down|test|up-direct|test-direct|logs) ;;
    -h|--help|help)
        usage
        exit 0
        ;;
    *)
        usage >&2
        exit 1
        ;;
esac

project_root="$(git rev-parse --show-toplevel)"
react_dir="$project_root/crates/czar-web-react"
harness_dir="$project_root/crates/czar-store-d1/wrangler-harness"
full_dir="$project_root/deploy/cloudflare/full"
full_dev_vars="$full_dir/.dev.vars"

state_dir="${CZAR_CLOUDFLARE_STATE_DIR:-${TMPDIR:-/tmp}/czar-cloudflare-full-local}"
logs_dir="$state_dir/logs"
harness_log="$logs_dir/d1.log"
full_log="$logs_dir/full.log"
harness_pid_file="$state_dir/d1.pid"
full_pid_file="$state_dir/full.pid"
harness_persist_dir="$state_dir/wrangler-d1"
full_persist_dir="$state_dir/wrangler-full"
full_dev_vars_backup="$state_dir/full.dev.vars.backup"
full_dev_vars_absent_marker="$state_dir/full.dev.vars.absent"
root_html_file="$state_dir/root.html"
workflows_json_file="$state_dir/workflows.json"
run_create_json_file="$state_dir/run-create.json"
run_detail_json_file="$state_dir/run-detail.json"
direct_log="$logs_dir/direct.log"
direct_pid_file="$state_dir/direct.pid"

full_port="${CZAR_CLOUDFLARE_FULL_PORT:-8787}"
d1_port="${CZAR_CLOUDFLARE_D1_PORT:-8788}"
direct_port="${CZAR_CLOUDFLARE_DIRECT_PORT:-18080}"
basic_auth_user="${CZAR_FULL_BASIC_AUTH_USER:-czar}"
basic_auth_pass="${CZAR_FULL_BASIC_AUTH_PASS:-czar}"
d1_account_id="${CZAR_D1_ACCOUNT_ID:-local-account}"
d1_database_id="${CZAR_D1_DATABASE_ID:-local-database}"
d1_api_token="${CZAR_D1_API_TOKEN:-test-token}"

stream_logs=0
require_workflows_ready=0

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing required command: $1" >&2
        exit 1
    fi
}

ensure_node_24() {
    local required_major=24
    local node_major=""

    if command -v node >/dev/null 2>&1; then
        node_major="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || true)"
        if [ -n "$node_major" ] && [ "$node_major" -ge "$required_major" ]; then
            return 0
        fi
    fi

    export NVM_DIR="${NVM_DIR:-$HOME/.nvm}"
    if [ -s "$NVM_DIR/nvm.sh" ]; then
        # Non-interactive shells do not inherit the user's selected Node version.
        # shellcheck disable=SC1090
        . "$NVM_DIR/nvm.sh"
        nvm use 24 >/dev/null
    fi

    require_cmd node
    node_major="$(node -p 'process.versions.node.split(".")[0]')"
    if [ "$node_major" -lt "$required_major" ]; then
        echo "Node 24+ is required for deploy/cloudflare/full local development. Found $(node -v)." >&2
        exit 1
    fi
}

port_in_use() {
    local port="$1"

    if command -v ss >/dev/null 2>&1; then
        ss -Hltn "( sport = :$port )" | grep -q .
        return
    fi

    if command -v lsof >/dev/null 2>&1; then
        lsof -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1
        return
    fi

    return 1
}

listening_pids_for_port() {
    local port="$1"

    if command -v lsof >/dev/null 2>&1; then
        lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true
        return
    fi

    if command -v ss >/dev/null 2>&1; then
        ss -Hltnp "( sport = :$port )" 2>/dev/null | sed -n 's/.*pid=\([0-9]\+\).*/\1/p' | sort -u
        return
    fi
}

assert_port_free() {
    local port="$1"
    local label="$2"
    if port_in_use "$port"; then
        echo "$label port $port is already in use. Run 'just down-cloudflare-full' or choose a different port." >&2
        exit 1
    fi
}

next_free_port() {
    local start_port="$1"
    local end_port="$2"
    local candidate=""

    for candidate in $(seq "$start_port" "$end_port"); do
        if ! port_in_use "$candidate"; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done

    return 1
}

select_test_ports_if_needed() {
    local original_full_port="$full_port"
    local original_d1_port="$d1_port"
    local original_direct_port="$direct_port"

    if port_in_use "$d1_port"; then
        d1_port="$(next_free_port 18788 18820)" || {
            echo "No free fallback port available for the local D1 harness." >&2
            exit 1
        }
    fi

    if [ "$action" = "test-direct" ]; then
        if port_in_use "$direct_port"; then
            direct_port="$(next_free_port 18080 18120)" || {
                echo "No free fallback port available for the direct Cloudflare app." >&2
                exit 1
            }
        fi
    else
        if port_in_use "$full_port"; then
            full_port="$(next_free_port 18787 18819)" || {
                echo "No free fallback port available for the Cloudflare full app." >&2
                exit 1
            }
        fi
    fi

    if [ "$original_d1_port" != "$d1_port" ]; then
        echo "Using fallback D1 harness port: $d1_port"
    fi
    if [ "$original_full_port" != "$full_port" ]; then
        echo "Using fallback Cloudflare full app port: $full_port"
    fi
    if [ "$original_direct_port" != "$direct_port" ]; then
        echo "Using fallback direct app port: $direct_port"
    fi
}

detect_host_ip() {
    if [ -n "${CZAR_CLOUDFLARE_HOST_IP:-}" ]; then
        printf '%s\n' "$CZAR_CLOUDFLARE_HOST_IP"
        return
    fi

    if command -v ip >/dev/null 2>&1; then
        local docker_ip
        docker_ip="$(ip -4 addr show docker0 2>/dev/null | awk '/inet / {print $2}' | cut -d/ -f1)"
        if [ -n "$docker_ip" ]; then
            printf '%s\n' "$docker_ip"
            return
        fi

        ip route get 1.1.1.1 2>/dev/null | awk '
            /src/ {
                for (i = 1; i <= NF; i++) {
                    if ($i == "src") {
                        print $(i + 1)
                        exit
                    }
                }
            }
        '
        return
    fi

    if command -v hostname >/dev/null 2>&1; then
        hostname -I 2>/dev/null | awk '{ print $1 }'
        return
    fi
}

show_logs() {
    if [ -f "$harness_log" ]; then
        echo
        echo "=== local D1 harness log ==="
        cat "$harness_log"
    fi
    if [ -f "$direct_log" ]; then
        echo
        echo "=== direct Cloudflare app log ==="
        cat "$direct_log"
    fi
    if [ -f "$full_log" ]; then
        echo
        echo "=== Cloudflare full app log ==="
        cat "$full_log"
    fi
}

read_pid_file() {
    local pid_file="$1"
    if [ ! -f "$pid_file" ]; then
        return 1
    fi
    tr -d '[:space:]' <"$pid_file"
}

kill_managed_pid() {
    local pid_file="$1"
    local pid=""

    pid="$(read_pid_file "$pid_file" || true)"
    if [ -z "$pid" ]; then
        rm -f "$pid_file"
        return
    fi

    if kill -0 "$pid" 2>/dev/null; then
        kill "$pid" 2>/dev/null || true
        wait "$pid" 2>/dev/null || true
    fi

    rm -f "$pid_file"
}

cleanup_stale_wrangler_processes() {
    local pids=()

    mapfile -t pids < <(
        ps -eo pid=,args= | awk \
            -v d1_port="$d1_port" \
            -v full_port="$full_port" \
            '
                index($0, "wrangler dev --config wrangler.jsonc") &&
                (index($0, "--port " d1_port) || index($0, "--port " full_port)) {
                    print $1
                }
            '
    )

    if [ "${#pids[@]}" -gt 0 ]; then
        kill "${pids[@]}" 2>/dev/null || true
        wait "${pids[@]}" 2>/dev/null || true
    fi
}

cleanup_stale_port_listeners() {
    local port="$1"
    local pid=""
    local comm=""
    local args=""

    while IFS= read -r pid; do
        [ -n "$pid" ] || continue
        comm="$(ps -p "$pid" -o comm= 2>/dev/null | tr -d '[:space:]')"
        args="$(ps -p "$pid" -o args= 2>/dev/null || true)"

        case "$comm" in
            workerd)
                kill "$pid" 2>/dev/null || true
                ;;
            node|npm|npx|pnpm)
                if [[ "$args" == *"$harness_dir"* ]] || [[ "$args" == *"$full_dir"* ]]; then
                    kill "$pid" 2>/dev/null || true
                fi
                ;;
        esac
    done < <(listening_pids_for_port "$port")
}

cleanup_cloudflare_containers() {
    local ids=()

    mapfile -t ids < <(docker ps -aq --filter "name=workerd-czar-full-CzarFullApp-")
    if [ "${#ids[@]}" -gt 0 ]; then
        docker rm -f "${ids[@]}" >/dev/null 2>&1 || true
    fi
}

direct_container_name() {
    printf 'czar-cloudflare-full-direct-%s\n' "$direct_port"
}

cleanup_direct_container() {
    docker rm -f "$(direct_container_name)" >/dev/null 2>&1 || true
}

save_dev_vars() {
    mkdir -p "$state_dir"
    rm -f "$full_dev_vars_backup" "$full_dev_vars_absent_marker"
    if [ -f "$full_dev_vars" ]; then
        cp "$full_dev_vars" "$full_dev_vars_backup"
    else
        : >"$full_dev_vars_absent_marker"
    fi
}

restore_dev_vars() {
    if [ -f "$full_dev_vars_backup" ]; then
        mv "$full_dev_vars_backup" "$full_dev_vars"
    elif [ -f "$full_dev_vars_absent_marker" ]; then
        rm -f "$full_dev_vars"
    fi

    rm -f "$full_dev_vars_absent_marker"
}

down_internal() {
    kill_managed_pid "$direct_pid_file"
    kill_managed_pid "$full_pid_file"
    kill_managed_pid "$harness_pid_file"
    cleanup_stale_wrangler_processes
    cleanup_stale_port_listeners "$direct_port"
    cleanup_stale_port_listeners "$full_port"
    cleanup_stale_port_listeners "$d1_port"
    cleanup_direct_container
    cleanup_cloudflare_containers
    restore_dev_vars
    rm -rf "$state_dir"
}

on_exit() {
    local exit_code="$1"

    trap - EXIT INT TERM
    if [ "$exit_code" -ne 0 ] && [ "$action" != "up" ] && [ "$action" != "up-direct" ] && [ "$action" != "down" ] && [ "$action" != "logs" ]; then
        show_logs
    fi
    down_internal
    exit "$exit_code"
}

wait_for_status() {
    local url="$1"
    local expected="$2"
    local auth="${3:-}"
    local timeout_secs="${4:-120}"
    local deadline=$((SECONDS + timeout_secs))
    local status=""

    while [ "$SECONDS" -lt "$deadline" ]; do
        if [ -n "$auth" ]; then
            status="$(curl --connect-timeout 2 --max-time 5 -sS -o /dev/null -w '%{http_code}' -u "$auth" "$url" || true)"
        else
            status="$(curl --connect-timeout 2 --max-time 5 -sS -o /dev/null -w '%{http_code}' "$url" || true)"
        fi

        if [ "$status" = "$expected" ]; then
            return 0
        fi

        sleep 1
    done

    echo "Timed out waiting for $url to return $expected." >&2
    return 1
}

wait_for_nonempty_workflows() {
    local deadline=$((SECONDS + 180))

    while [ "$SECONDS" -lt "$deadline" ]; do
        local body
        body="$(curl --connect-timeout 2 --max-time 5 -sS -u "$basic_auth_user:$basic_auth_pass" "http://127.0.0.1:$full_port/api/workflows" || true)"
        if [ -n "$body" ] && WORKFLOW_BODY="$body" node <<'EOF'
const payload = process.env.WORKFLOW_BODY;
if (!payload) process.exit(1);

try {
  const parsed = JSON.parse(payload);
  const items = Array.isArray(parsed) ? parsed : parsed?.items;
  if (!Array.isArray(items) || items.length === 0) process.exit(1);
  process.exit(0);
} catch {
  process.exit(1);
}
EOF
        then
            return 0
        fi
        sleep 5
    done

    echo "Timed out waiting for /api/workflows to return a non-empty workflow list." >&2
    return 1
}

trigger_example_run() {
    local workflow_name="$1"
    local status=""

    status="$(curl --connect-timeout 2 --max-time 10 -sS \
        -u "$basic_auth_user:$basic_auth_pass" \
        -H 'Content-Type: application/json' \
        -o "$run_create_json_file" \
        -w '%{http_code}' \
        -X POST \
        -d "{\"workflow\":\"$workflow_name\"}" \
        "http://127.0.0.1:$full_port/api/runs" || true)"

    if [ "$status" != "200" ]; then
        echo "Expected POST /api/runs to return 200, got $status." >&2
        [ -f "$run_create_json_file" ] && cat "$run_create_json_file" >&2
        return 1
    fi

    RUN_CREATE_BODY="$(cat "$run_create_json_file")" node <<'EOF'
const payload = process.env.RUN_CREATE_BODY;
if (!payload) process.exit(1);

try {
  const parsed = JSON.parse(payload);
  if (typeof parsed?.run_id !== 'string' || parsed.run_id.length === 0) process.exit(1);
  process.stdout.write(parsed.run_id);
} catch {
  process.exit(1);
}
EOF
}

wait_for_run_success() {
    local run_id="$1"
    local deadline=$((SECONDS + 90))

    while [ "$SECONDS" -lt "$deadline" ]; do
        local status=""
        local run_state=""

        status="$(curl --connect-timeout 2 --max-time 10 -sS \
            -u "$basic_auth_user:$basic_auth_pass" \
            -o "$run_detail_json_file" \
            -w '%{http_code}' \
            "http://127.0.0.1:$full_port/api/runs/$run_id" || true)"

        if [ "$status" = "200" ]; then
            run_state="$(RUN_DETAIL_BODY="$(cat "$run_detail_json_file")" node <<'EOF'
const payload = process.env.RUN_DETAIL_BODY;
if (!payload) process.exit(1);

try {
  const parsed = JSON.parse(payload);
  process.stdout.write(String(parsed?.run?.status ?? '').toLowerCase());
} catch {
  process.exit(1);
}
EOF
)"
            if [ "$run_state" = "success" ]; then
                return 0
            fi
            if [ "$run_state" = "failed" ] || [ "$run_state" = "cancelled" ] || [ "$run_state" = "canceled" ]; then
                echo "Run $run_id reached terminal failure state: $run_state" >&2
                cat "$run_detail_json_file" >&2
                return 1
            fi
        fi

        sleep 2
    done

    echo "Timed out waiting for run $run_id to reach success." >&2
    [ -f "$run_detail_json_file" ] && cat "$run_detail_json_file" >&2
    return 1
}

start_logged_process() {
    local pid_file="$1"
    local log_file="$2"
    local cwd="$3"
    shift 3

    mkdir -p "$logs_dir"
    : >"$log_file"

    if [ "$stream_logs" -eq 1 ]; then
        (
            cd "$cwd"
            exec "$@" \
                > >(tee -a "$log_file") \
                2> >(tee -a "$log_file" >&2)
        ) &
    else
        (
            cd "$cwd"
            exec "$@" >>"$log_file" 2>&1
        ) &
    fi

    printf '%s\n' "$!" >"$pid_file"
}

prepare_stack() {
    require_cmd cargo
    require_cmd curl
    require_cmd docker
    require_cmd git
    ensure_node_24
    require_cmd pnpm

    if ! docker info >/dev/null 2>&1; then
        echo "Docker is required for local Cloudflare container development and does not appear to be reachable." >&2
        exit 1
    fi

    host_ip="$(detect_host_ip)"
    if [ -z "$host_ip" ]; then
        echo "Unable to determine a host IP reachable from the local Cloudflare container. Set CZAR_CLOUDFLARE_HOST_IP." >&2
        exit 1
    fi

    d1_base_url="${CZAR_D1_BASE_URL:-http://$host_ip:$d1_port}"

    down_internal
    if [ "$action" = "test" ] || [ "$action" = "test-direct" ]; then
        select_test_ports_if_needed
        d1_base_url="${CZAR_D1_BASE_URL:-http://$host_ip:$d1_port}"
    else
        assert_port_free "$d1_port" "Local D1 harness"
    fi
    if [ "$action" = "up-direct" ] || [ "$action" = "test-direct" ]; then
        assert_port_free "$direct_port" "Direct Cloudflare app"
    elif [ "$action" != "test" ]; then
        assert_port_free "$full_port" "Cloudflare full app"
    fi

    mkdir -p "$logs_dir"
    rm -rf "$harness_persist_dir" "$full_persist_dir"

    save_dev_vars
    cat >"$full_dev_vars" <<EOF
BASIC_AUTH_USER=$basic_auth_user
BASIC_AUTH_PASS=$basic_auth_pass
CZAR_D1_ACCOUNT_ID=$d1_account_id
CZAR_D1_DATABASE_ID=$d1_database_id
CZAR_D1_API_TOKEN=$d1_api_token
CZAR_D1_BASE_URL=$d1_base_url
EOF

    echo "Preparing React assets..."
    (cd "$react_dir" && pnpm install --frozen-lockfile && CZAR_WEB_BASE_PATH=/ pnpm run build)

    echo "Preparing local D1 harness..."
    (cd "$harness_dir" && pnpm install --frozen-lockfile)

    echo "Preparing Cloudflare full deployment dependencies..."
    (cd "$full_dir" && pnpm install --frozen-lockfile)

    cleanup_direct_container
    cleanup_cloudflare_containers
}

start_stack() {
    prepare_stack

    echo "Starting local D1 HTTP harness on http://127.0.0.1:$d1_port ..."
    start_logged_process \
        "$harness_pid_file" \
        "$harness_log" \
        "$harness_dir" \
        pnpm exec wrangler dev --config wrangler.jsonc --persist-to "$harness_persist_dir" --ip 0.0.0.0 --port "$d1_port" --log-level warn
    wait_for_status "http://127.0.0.1:$d1_port/health" "200"

    echo "Starting Cloudflare Worker + container on http://127.0.0.1:$full_port ..."
    start_logged_process \
        "$full_pid_file" \
        "$full_log" \
        "$full_dir" \
        pnpm exec wrangler dev --config wrangler.jsonc --persist-to "$full_persist_dir" --ip 127.0.0.1 --port "$full_port" --log-level warn

    wait_for_status "http://127.0.0.1:$full_port/" "401" "" "300"
    wait_for_status "http://127.0.0.1:$full_port/api/workflows" "200" "$basic_auth_user:$basic_auth_pass" "300"
    if [ "$require_workflows_ready" -eq 1 ]; then
        wait_for_nonempty_workflows
    fi
}

build_direct_image() {
    echo "Building direct Cloudflare app image..."
    docker build -f "$full_dir/Dockerfile" -t czarfullapp:debug "$project_root" >/dev/null
}

start_direct_app() {
    build_direct_image

    echo "Starting direct Cloudflare app container on http://127.0.0.1:$direct_port ..."
    cleanup_direct_container
    start_logged_process \
        "$direct_pid_file" \
        "$direct_log" \
        "$project_root" \
        docker run --rm --name "$(direct_container_name)" -p "$direct_port:8080" \
        -e CZAR_DB_BACKEND=d1 \
        -e CZAR_D1_ACCOUNT_ID="$d1_account_id" \
        -e CZAR_D1_DATABASE_ID="$d1_database_id" \
        -e CZAR_D1_API_TOKEN="$d1_api_token" \
        -e CZAR_D1_BASE_URL="$d1_base_url" \
        -e CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS=1 \
        -e CZAR_WORKFLOW_DEFINITIONS_DIR=/app/cloudflare-workflows \
        -e RUST_LOG="${CZAR_FULL_RUST_LOG:-warn}" \
        czarfullapp:debug

    wait_for_status "http://127.0.0.1:$direct_port/api/workflows" "200" "" "120"
}

run_direct_test_assertions() {
    local api_status=""
    local run_id=""
    local create_status=""

    api_status="$(curl --connect-timeout 2 --max-time 10 -sS -o "$workflows_json_file" -w '%{http_code}' "http://127.0.0.1:$direct_port/api/workflows" || true)"
    if [ "$api_status" != "200" ]; then
        echo "Expected direct /api/workflows to return 200, got $api_status." >&2
        return 1
    fi

    if ! WORKFLOW_BODY="$(cat "$workflows_json_file")" node <<'EOF'
const payload = process.env.WORKFLOW_BODY;
if (!payload) process.exit(1);

try {
  const parsed = JSON.parse(payload);
  const items = Array.isArray(parsed) ? parsed : parsed?.items;
  if (!Array.isArray(items) || items.length === 0) process.exit(1);
  process.exit(0);
} catch {
  process.exit(1);
}
EOF
    then
        echo "Expected the direct app to preload example workflows, got: $(cat "$workflows_json_file")" >&2
        return 1
    fi

    create_status="$(curl --connect-timeout 2 --max-time 10 -sS \
        -H 'Content-Type: application/json' \
        -o "$run_create_json_file" \
        -w '%{http_code}' \
        -X POST \
        -d '{"workflow":"simple_demo"}' \
        "http://127.0.0.1:$direct_port/api/runs" || true)"

    if [ "$create_status" != "200" ]; then
        echo "Expected direct POST /api/runs to return 200, got $create_status." >&2
        [ -f "$run_create_json_file" ] && cat "$run_create_json_file" >&2
        return 1
    fi

    run_id="$(RUN_CREATE_JSON_FILE="$run_create_json_file" node <<'EOF'
const fs = require('fs');
const body = fs.readFileSync(process.env.RUN_CREATE_JSON_FILE, 'utf8');
const parsed = JSON.parse(body);
if (typeof parsed?.run_id !== 'string' || parsed.run_id.length === 0) {
  process.exit(1);
}
process.stdout.write(parsed.run_id);
EOF
    )"

    local deadline=$((SECONDS + 90))
    while [ "$SECONDS" -lt "$deadline" ]; do
        local run_state=""
        local detail_status=""
        detail_status="$(curl --connect-timeout 2 --max-time 10 -sS \
            -o "$run_detail_json_file" \
            -w '%{http_code}' \
            "http://127.0.0.1:$direct_port/api/runs/$run_id" || true)"
        if [ "$detail_status" != "200" ]; then
            sleep 2
            continue
        fi
        run_state="$(RUN_DETAIL_BODY="$(cat "$run_detail_json_file")" node <<'EOF'
const payload = process.env.RUN_DETAIL_BODY;
if (!payload) process.exit(1);
const parsed = JSON.parse(payload);
process.stdout.write(String(parsed?.run?.status ?? '').toLowerCase());
EOF
)"
        if [ "$run_state" = "success" ]; then
            return 0
        fi
        if [ "$run_state" = "failed" ] || [ "$run_state" = "cancelled" ] || [ "$run_state" = "canceled" ]; then
            echo "Direct app run $run_id reached terminal failure state: $run_state" >&2
            cat "$run_detail_json_file" >&2
            return 1
        fi
        sleep 2
    done

    echo "Timed out waiting for direct app run $run_id to reach success." >&2
    [ -f "$run_detail_json_file" ] && cat "$run_detail_json_file" >&2
    return 1
}

run_test_assertions() {
    local unauth_status=""
    local root_status=""
    local api_status=""
    local run_id=""

    unauth_status="$(curl --connect-timeout 2 --max-time 5 -sS -o /dev/null -w '%{http_code}' "http://127.0.0.1:$full_port/" || true)"
    if [ "$unauth_status" != "401" ]; then
        echo "Expected unauthenticated root to return 401, got $unauth_status." >&2
        return 1
    fi

    root_status="$(curl --connect-timeout 2 --max-time 10 -sS -u "$basic_auth_user:$basic_auth_pass" -o "$root_html_file" -w '%{http_code}' "http://127.0.0.1:$full_port/" || true)"
    if [ "$root_status" != "200" ]; then
        echo "Expected authenticated root to return 200, got $root_status." >&2
        return 1
    fi

    if ! grep -qi '<!doctype html>' "$root_html_file"; then
        echo "Root response did not look like the React app HTML." >&2
        return 1
    fi

    api_status="$(curl --connect-timeout 2 --max-time 10 -sS -u "$basic_auth_user:$basic_auth_pass" -o "$workflows_json_file" -w '%{http_code}' "http://127.0.0.1:$full_port/api/workflows" || true)"
    if [ "$api_status" != "200" ]; then
        echo "Expected /api/workflows to return 200, got $api_status." >&2
        return 1
    fi

    if ! WORKFLOW_BODY="$(cat "$workflows_json_file")" node <<'EOF'
const payload = process.env.WORKFLOW_BODY;
if (!payload) process.exit(1);

try {
  const parsed = JSON.parse(payload);
  const items = Array.isArray(parsed) ? parsed : parsed?.items;
  if (!Array.isArray(items) || items.length === 0) process.exit(1);
  process.exit(0);
} catch {
  process.exit(1);
}
EOF
    then
        echo "Expected the Cloudflare app to preload example workflows, got: $(cat "$workflows_json_file")" >&2
        return 1
    fi

    run_id="$(trigger_example_run "simple_demo")" || return 1
    wait_for_run_success "$run_id"
}

run_up() {
    stream_logs=1
    require_workflows_ready=0
    trap 'exit 130' INT TERM
    trap 'on_exit $?' EXIT

    start_stack

    echo
    echo "Cloudflare full app is ready."
    echo "URL: http://127.0.0.1:$full_port"
    echo "Basic auth: $basic_auth_user / $basic_auth_pass"
    echo "Local D1 API: $d1_base_url"
    echo

    wait "$(read_pid_file "$full_pid_file")"
}

run_test() {
    stream_logs=0
    require_workflows_ready=1
    trap 'exit 130' INT TERM
    trap 'on_exit $?' EXIT

    start_stack
    run_test_assertions
    echo "Cloudflare full local smoke test passed."
}

run_up_direct() {
    stream_logs=1
    trap 'exit 130' INT TERM
    trap 'on_exit $?' EXIT

    prepare_stack

    echo "Starting local D1 HTTP harness on http://127.0.0.1:$d1_port ..."
    start_logged_process \
        "$harness_pid_file" \
        "$harness_log" \
        "$harness_dir" \
        pnpm exec wrangler dev --config wrangler.jsonc --persist-to "$harness_persist_dir" --ip 0.0.0.0 --port "$d1_port" --log-level warn
    wait_for_status "http://127.0.0.1:$d1_port/health" "200"
    start_direct_app

    echo
    echo "Direct Cloudflare app is ready."
    echo "URL: http://127.0.0.1:$direct_port"
    echo "Local D1 API: $d1_base_url"
    echo

    wait "$(read_pid_file "$direct_pid_file")"
}

run_test_direct() {
    stream_logs=0
    trap 'exit 130' INT TERM
    trap 'on_exit $?' EXIT

    prepare_stack

    echo "Starting local D1 HTTP harness on http://127.0.0.1:$d1_port ..."
    start_logged_process \
        "$harness_pid_file" \
        "$harness_log" \
        "$harness_dir" \
        pnpm exec wrangler dev --config wrangler.jsonc --persist-to "$harness_persist_dir" --ip 0.0.0.0 --port "$d1_port" --log-level warn
    wait_for_status "http://127.0.0.1:$d1_port/health" "200"
    start_direct_app
    run_direct_test_assertions
    echo "Direct Cloudflare app local smoke test passed."
}

run_logs() {
    show_logs
}

case "$action" in
    up)
        run_up
        ;;
    down)
        down_internal
        ;;
    test)
        run_test
        ;;
    up-direct)
        run_up_direct
        ;;
    test-direct)
        run_test_direct
        ;;
    logs)
        run_logs
        ;;
esac
