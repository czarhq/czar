#!/usr/bin/env bash
set -e

# Build binary once to avoid spam
echo "Building release binary..."
cargo build --release --quiet

BIN="../../target/release/czar"

# Cleanup function
cleanup() {
    echo ""
    echo "Cleaning up..."
    kill $ORCH_PID 2>/dev/null || true
    exit 0
}
trap cleanup INT TERM EXIT

echo "Starting orchestrator..."
$BIN serve &
ORCH_PID=$!
sleep 2

echo "Triggering 100 workflows..."
for i in {1..100}; do
    $BIN trigger example-process >/dev/null &
done
wait

echo "Monitoring for 30 seconds..."
sleep 30

echo ""
$BIN status

cleanup
echo "✓ Load test complete"
