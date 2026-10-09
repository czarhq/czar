-- Ensure workflow snapshot hashes are unique per workflow and remap duplicates.
CREATE TEMP TABLE snapshot_remap AS
SELECT
    id AS old_id,
    FIRST_VALUE(id) OVER (
        PARTITION BY workflow_id, content_hash
        ORDER BY created_at DESC, id DESC
    ) AS keep_id
FROM workflow_snapshots;

DELETE FROM snapshot_remap WHERE old_id = keep_id;

UPDATE workflows AS w
SET current_snapshot_id = r.keep_id
FROM snapshot_remap AS r
WHERE w.current_snapshot_id = r.old_id;

UPDATE runs AS r
SET snapshot_id = m.keep_id
FROM snapshot_remap AS m
WHERE r.snapshot_id = m.old_id;

DELETE FROM workflow_snapshots AS s
USING snapshot_remap AS r
WHERE s.id = r.old_id;

DROP TABLE snapshot_remap;

DROP INDEX IF EXISTS idx_workflow_snapshots_content_hash;
CREATE UNIQUE INDEX idx_workflow_snapshots_content_hash
    ON workflow_snapshots(workflow_id, content_hash);
