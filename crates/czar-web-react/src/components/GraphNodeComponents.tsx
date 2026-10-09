import { Handle, Position, type NodeProps } from '@xyflow/react';

export const GROUP_FRAME_PREFIX = '__frame__';
export const COND_PREFIX = '__cond__';

export interface GroupFrameNodeData extends Record<string, unknown> {
  groupName: string;
  taskCount: number;
  onToggle: () => void;
}

export interface ConditionGateNodeData extends Record<string, unknown> {
  conditionExpr: string;
}

function GroupFrameNode({
  data,
  sourcePosition = Position.Right,
  targetPosition = Position.Left,
}: NodeProps) {
  const d = data as GroupFrameNodeData;
  return (
    <div style={{ width: '100%', height: '100%', position: 'relative' }}>
      <Handle type="target" position={targetPosition} style={{ opacity: 0, pointerEvents: 'none' }} />
      <Handle type="source" position={sourcePosition} style={{ opacity: 0, pointerEvents: 'none' }} />
      <div
        style={{
          position: 'absolute',
          top: 0,
          left: 0,
          right: 0,
          height: 26,
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          padding: '0 8px',
          background: 'var(--interactive-bg-hover)',
          borderBottom: '1px solid var(--interactive-border-soft)',
          borderRadius: '8px 8px 0 0',
          cursor: 'default',
        }}
      >
        <span style={{ fontSize: 11, fontWeight: 700, color: 'var(--accent-2)', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {d.groupName} ({d.taskCount})
        </span>
        <button
          type="button"
          onClick={(e) => { e.stopPropagation(); d.onToggle(); }}
          style={{ background: 'none', border: 'none', color: 'var(--accent-2)', cursor: 'pointer', fontSize: 11, padding: '0 2px', flexShrink: 0 }}
          title="Collapse group"
        >
          ▼
        </button>
      </div>
    </div>
  );
}

function ConditionGateNode({
  data,
  sourcePosition = Position.Right,
  targetPosition = Position.Left,
}: NodeProps) {
  const d = data as ConditionGateNodeData;
  return (
    <div style={{ width: '100%', height: '100%', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 3, padding: '0 6px' }}>
      <Handle type="target" position={targetPosition} />
      <Handle type="source" position={sourcePosition} />
      <div style={{ fontSize: 9, fontWeight: 800, color: 'inherit', letterSpacing: '0.12em', textTransform: 'uppercase' }}>
        if
      </div>
      <div style={{ fontSize: 10, color: 'inherit', fontStyle: 'italic', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', maxWidth: '100%', textAlign: 'center' }}
        title={d.conditionExpr}
      >
        {d.conditionExpr}
      </div>
    </div>
  );
}

export const NODE_TYPES = { groupFrame: GroupFrameNode, conditionGate: ConditionGateNode };
