#!/bin/sh
set -eu

ensure_owner() {
    path="$1"
    desired="${CZAR_UID}:${CZAR_GID}"
    install -d "$path"
    current="$(stat -c '%u:%g' "$path" 2>/dev/null || printf 'missing')"
    if [ "$current" != "$desired" ]; then
        chown -R "$desired" "$path"
    fi
}

ensure_owner /home/czar
ensure_owner /home/czar/.cache
ensure_owner /cargo
ensure_owner /cargo/registry
ensure_owner /cargo/git
ensure_owner /app/target

export HOME=/home/czar
export XDG_CACHE_HOME=/home/czar/.cache

if [ "${CZAR_DEV_NO_WATCH:-}" = "1" ]; then
    exec gosu "${CZAR_UID}:${CZAR_GID}" cargo run -p czar-cli --bin czar -- serve --addr 0.0.0.0:4000
fi

exec gosu "${CZAR_UID}:${CZAR_GID}" "$@"
