#!/bin/bash
# Local test script for the orchestrator
# This demonstrates how to test the orchestrator locally with a process executor

set -e

echo "=========================================="
echo "Rust Cloud Run Orchestrator - Local Test"
echo "=========================================="
echo ""

echo "1. Building the project..."
cargo build --release
echo "✓ Build successful"
echo ""

echo "2. Checking that test script exists..."
ls -lh scripts/example-task.sh
echo "✓ Test script found"
echo ""

echo "3. Testing the process executor directly..."
echo "   This would normally execute tasks as local processes."
echo ""

echo "Example workflow with process executor:"
echo "  Name: example-process"
echo "  Executor: process"
echo "  Script: scripts/example-task.sh"
echo "  Task count: 5"
echo ""

echo "When the orchestrator runs, it would:"
echo "  1. Poll for pending runs in the database"
echo "  2. Create 5 tasks for the workflow"
echo "  3. Execute scripts/example-task.sh for each task"
echo "  4. Pass environment variables: task_index, run_id, workflow_name"
echo "  5. Monitor task completion and update status"
echo ""

echo "To run this test with PostgreSQL:"
echo "  # Start PostgreSQL"
echo "  just db-start"
echo ""
echo "  # Initialize database"
echo "  just init"
echo ""
echo "  # Create example workflow"
echo "  just create-example-workflow"
echo ""
echo "  # Start orchestrator (in one terminal)"
echo "  just run"
echo ""
echo "  # Trigger workflow (in another terminal)"
echo "  just trigger example-process"
echo ""
echo "  # Check status"
echo "  just status"
echo ""

echo "=========================================="
echo "Test complete!"
echo "=========================================="
