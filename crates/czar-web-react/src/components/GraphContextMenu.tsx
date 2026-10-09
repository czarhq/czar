import { isGroupNode, getGroupNameFromNodeId } from './groupedGraphUtils';
import { GROUP_FRAME_PREFIX, COND_PREFIX } from './GraphNodeComponents';
import type { GraphOrientation } from './graphUtils';

interface GraphContextMenuProps {
  contextMenu: { nodeId?: string; x: number; y: number } | null;
  onDismiss: () => void;
  onToggleGroup?: (groupName: string) => void;
  nodeToGroups: Map<string, string[]>;
  collapsedGroups: Set<string>;
  orientation: GraphOrientation;
  onOrientationChange: (orientation: GraphOrientation) => void;
  onDownloadPng: () => void;
  onDownloadSvg: () => void;
}

export function GraphContextMenu({
  contextMenu, onDismiss, onToggleGroup, nodeToGroups, collapsedGroups,
  orientation, onOrientationChange, onDownloadPng, onDownloadSvg,
}: GraphContextMenuProps) {
  if (!contextMenu) return null;
  const { nodeId, x, y } = contextMenu;
  const handleContextMenu = (event: React.MouseEvent) => {
    event.preventDefault();
    event.stopPropagation();
  };

  const nodeItems: Array<{ label: string; groupName: string }> = [];
  if (nodeId && onToggleGroup && !nodeId.startsWith(COND_PREFIX)) {
    if (isGroupNode(nodeId)) {
      nodeItems.push({ label: 'Expand', groupName: getGroupNameFromNodeId(nodeId) });
    } else if (nodeId.startsWith(GROUP_FRAME_PREFIX)) {
      nodeItems.push({ label: 'Collapse', groupName: nodeId.slice(GROUP_FRAME_PREFIX.length) });
    } else {
      for (const groupName of nodeToGroups.get(nodeId) ?? []) {
        nodeItems.push({ label: collapsedGroups.has(groupName) ? 'Expand' : 'Collapse', groupName });
      }
    }
  }

  return (
    <>
      <div
        style={{ position: 'fixed', inset: 0, zIndex: 9999 }}
        onClick={onDismiss}
        onContextMenu={handleContextMenu}
      />
      <div className="ctx-menu" style={{ left: x, top: y }} onContextMenu={handleContextMenu}>
        {nodeItems.map(({ label, groupName }) => (
          <button key={groupName} type="button" className="ctx-menu-item"
            onClick={() => { onToggleGroup!(groupName); onDismiss(); }} title={groupName}
          >
            {label} <span style={{ color: 'var(--muted)', fontWeight: 400 }}>&ldquo;{groupName}&rdquo;</span>
          </button>
        ))}
        {nodeItems.length > 0 && <div style={{ height: '1px', background: 'var(--border)', margin: '4px 0' }} />}
        <button type="button" className="ctx-menu-item" onClick={() => { onOrientationChange(orientation === 'LR' ? 'TB' : 'LR'); onDismiss(); }}>
          {orientation === 'LR' ? 'Switch to Top → Bottom' : 'Switch to Left → Right'}
        </button>
        <button type="button" className="ctx-menu-item" onClick={() => { onDownloadPng(); onDismiss(); }}>Download PNG</button>
        <button type="button" className="ctx-menu-item" onClick={() => { onDownloadSvg(); onDismiss(); }}>Download SVG</button>
      </div>
    </>
  );
}
