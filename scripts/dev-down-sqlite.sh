#!/usr/bin/env sh
set -eu

pids=$(lsof -ti:4000 2>/dev/null || true)
if [ -n "$pids" ]; then
    echo "Stopping czar server (pids: $(echo "$pids" | tr '\n' ' '))..."
    echo "$pids" | xargs kill 2>/dev/null || true
else
    echo "No czar server found on port 4000."
fi
