#!/usr/bin/env bash
set -e

# Build binary once
echo "Building binary..."
cargo build --quiet

BIN="../../target/debug/czar"

# Cleanup function
cleanup() {
    echo "Stopping orchestrator..."
    kill $ORCH_PID 2>/dev/null || true
}
trap cleanup EXIT

echo "Starting orchestrator in background..."
$BIN serve &
ORCH_PID=$!

echo "Waiting for orchestrator to start..."
sleep 3

echo "Triggering workflow..."
RUN_ID=$($BIN trigger example-process | grep "Run ID:" | awk '{print $3}')
echo "Started run: $RUN_ID"

echo "Waiting for tasks to complete..."
sleep 10

echo "Checking status..."
$BIN status $RUN_ID
$BIN tasks $RUN_ID

cleanup

echo ""
echo "✓ Test complete!"
