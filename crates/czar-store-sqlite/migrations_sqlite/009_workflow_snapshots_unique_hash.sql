-- Ensure workflow snapshot hashes are unique per workflow and remap duplicates.
CREATE TEMP TABLE snapshot_remap AS
SELECT
    ws1.id AS old_id,
    (
        SELECT ws2.id
        FROM workflow_snapshots AS ws2
        WHERE ws2.workflow_id = ws1.workflow_id
          AND ws2.content_hash = ws1.content_hash
        ORDER BY ws2.created_at DESC, ws2.id DESC
        LIMIT 1
    ) AS keep_id
FROM workflow_snapshots AS ws1;

DELETE FROM snapshot_remap WHERE old_id = keep_id;

UPDATE workflows
SET current_snapshot_id = (
    SELECT keep_id
    FROM snapshot_remap
    WHERE old_id = workflows.current_snapshot_id
)
WHERE current_snapshot_id IN (SELECT old_id FROM snapshot_remap);

UPDATE runs
SET snapshot_id = (
    SELECT keep_id
    FROM snapshot_remap
    WHERE old_id = runs.snapshot_id
)
WHERE snapshot_id IN (SELECT old_id FROM snapshot_remap);

DELETE FROM workflow_snapshots
WHERE id IN (SELECT old_id FROM snapshot_remap);

DROP TABLE snapshot_remap;

DROP INDEX IF EXISTS idx_workflow_snapshots_content_hash;
CREATE UNIQUE INDEX idx_workflow_snapshots_content_hash
    ON workflow_snapshots(workflow_id, content_hash);
