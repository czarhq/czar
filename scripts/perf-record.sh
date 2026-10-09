#!/usr/bin/env bash
# Parse perf-test result files and append a summary row to docs/reports/perf/results.md.
# Usage: ./scripts/perf-record.sh <results-prefix> [note]
#   results-prefix: path prefix for timestamped result files (e.g. crates/czar-cli/perf-results/20260328-100839)
#   note:           optional freeform note (e.g. "after scheduler refactor")
set -euo pipefail

RESULTS_PREFIX="${1:-}"
NOTE="${2:-}"

if [[ -z "$RESULTS_PREFIX" ]]; then
    echo "Usage: $0 <results-prefix> [note]" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel)"
REPORT="$PROJECT_ROOT/docs/reports/perf/results.md"
mkdir -p "$(dirname "$REPORT")"

COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
DATE=$(date -u +"%Y-%m-%d")

# Parse a single result file and print structured fields for the summary table:
#   config avg_latency task_throughput rss_kb avg_cpu peak_cpu peak_pending peak_dispatched peak_running status_updates_processed
parse_result() {
    local file="$1"
    local config
    config=$(basename "$file" .txt | sed 's/^[0-9]\{8\}-[0-9]\{6\}-//')

    local avg_latency task_throughput rss_kb avg_cpu peak_cpu peak_pending peak_dispatched peak_running status_updates
    avg_latency=$(grep "Avg latency:"      "$file" | awk '{print $3}' | tr -d 's')
    task_throughput=$(grep "Task completion:" "$file" | awk '{print $3}')
    rss_kb=$(grep "Scheduler RSS:"        "$file" | grep -v growth | awk '{print $3}')
    avg_cpu=$(grep "Scheduler CPU avg:"   "$file" | awk '{print $4}' | tr -d '%')
    peak_cpu=$(grep "Scheduler CPU peak:" "$file" | awk '{print $4}' | tr -d '%')
    peak_pending=$(grep "Peak task queue:" "$file" | awk '{print $4}')
    peak_dispatched=$(grep "Peak task queue:" "$file" | awk '{print $6}' | tr -d ',')
    peak_running=$(grep "Peak task queue:" "$file" | awk '{print $8}')
    status_updates=$(grep "Time processing status updates:" "$file" | grep -oE '[0-9]+ updates' | awk '{print $1}')

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s' \
        "$config" "$avg_latency" "$task_throughput" "$rss_kb" \
        "$avg_cpu" "$peak_cpu" "$peak_pending" "$peak_dispatched" "$peak_running" \
        "$status_updates"
}

# Create report file with header if it doesn't exist
if [[ ! -f "$REPORT" ]]; then
    cat > "$REPORT" <<'EOF'
# Performance Benchmark Results

Results recorded by `just perf-record`. Each row is one full run of `just perf-all`.
Latency = avg task latency (created_at → finished_at). Throughput = tasks/sec.
RSS = scheduler peak resident memory. CPU = scheduler process (avg / peak across 500ms samples).
Peak queue = max concurrent tasks seen in each state during the run.
Status updates = total status update events processed by the scheduler.

| Date | Commit | Config | Avg Latency | Throughput | RSS KB | CPU avg | CPU peak | Peak pending | Peak dispatched | Peak running | Status updates | Note |
|------|--------|--------|------------:|-----------:|-------:|--------:|---------:|-------------:|----------------:|-------------:|---------------:|------|
EOF
fi

# Parse each result file and append rows
for file in "${RESULTS_PREFIX}"-*.txt; do
    [[ -f "$file" ]] || continue
    IFS=$'\t' read -r config avg_latency task_throughput rss_kb avg_cpu peak_cpu \
        peak_pending peak_dispatched peak_running status_updates \
        <<< "$(parse_result "$file")"
    printf "| %s | %s | %s | %ss | %s tasks/s | %s | %s%% | %s%% | %s | %s | %s | %s | %s |\n" \
        "$DATE" "$COMMIT" "$config" \
        "$avg_latency" "$task_throughput" "$rss_kb" \
        "$avg_cpu" "$peak_cpu" \
        "$peak_pending" "$peak_dispatched" "$peak_running" \
        "$status_updates" "$NOTE" \
        >> "$REPORT"
done

echo "Appended results to $REPORT"
