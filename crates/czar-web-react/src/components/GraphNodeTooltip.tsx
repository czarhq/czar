interface GraphNodeTooltipProps {
  tooltip: { nodeId: string; x: number; y: number } | null;
  nodeParams?: Record<string, Record<string, unknown>>;
}

export function GraphNodeTooltip({ tooltip, nodeParams }: GraphNodeTooltipProps) {
  if (!tooltip || !nodeParams) return null;
  const tooltipParams = nodeParams[tooltip.nodeId];
  if (!tooltipParams || Object.keys(tooltipParams).length === 0) return null;

  return (
    <div style={{
      position: 'fixed',
      left: tooltip.x + 14,
      top: tooltip.y - 8,
      zIndex: 9999,
      background: 'var(--panel-2)',
      border: '1px solid var(--border)',
      borderRadius: '8px',
      padding: '8px 12px',
      fontSize: '12px',
      pointerEvents: 'none',
      boxShadow: '0 4px 16px rgba(0,0,0,0.5)',
      whiteSpace: 'nowrap',
    }}>
      <div style={{ fontWeight: 600, marginBottom: '6px', color: 'var(--accent)' }}>{tooltip.nodeId}</div>
      {typeof tooltipParams.condition_expr === 'string' && (
        <div style={{ marginBottom: '6px', padding: '4px 8px', borderRadius: '4px', background: 'var(--badge-skip-bg)', border: '1px solid var(--badge-skip-border)' }}>
          <span style={{ color: 'var(--badge-skip-text)', fontWeight: 600 }}>if:</span>
          <span style={{ color: 'var(--badge-skip-text)', marginLeft: '6px' }}>{tooltipParams.condition_expr as string}</span>
        </div>
      )}
      <table style={{ borderSpacing: '0 2px' }}>
        <tbody>
          {Object.entries(tooltipParams)
            .filter(([k]) => k !== 'condition_expr')
            .map(([k, v]) => (
              <tr key={k}>
                <td style={{ color: 'var(--muted)', paddingRight: '10px' }}>{k}</td>
                <td style={{ color: 'var(--text)' }}>{typeof v === 'string' ? v : JSON.stringify(v)}</td>
              </tr>
            ))}
        </tbody>
      </table>
    </div>
  );
}
