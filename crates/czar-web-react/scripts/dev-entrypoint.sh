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
ensure_owner /app/node_modules

export HOME=/home/czar
export XDG_CACHE_HOME=/home/czar/.cache

exec su-exec "${CZAR_UID}:${CZAR_GID}" "$@"
