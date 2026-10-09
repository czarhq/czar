#!/bin/bash
echo "=========================================="
echo "Task Execution Started"
echo "=========================================="
echo "Task index: ${task_index}"
echo "Run ID: ${run_id}"
echo "Workflow: ${workflow_name}"
echo "Execution ID: ${EXECUTION_ID}"
echo ""

# Simulate some work
echo "Processing data..."
sleep 2

echo "Task processing complete!"
echo ""

# Output result as JSON
echo "Result:"
echo '{"status": "success", "processed_items": 42, "task_index": "'${task_index}'"}'

echo "=========================================="
echo "Task Execution Completed"
echo "=========================================="
