import type { ReactNode } from 'react';

export interface MessageModalAction {
  label: string;
  onClick: () => void;
  kind?: 'primary' | 'secondary' | 'danger';
  disabled?: boolean;
}

interface MessageModalProps {
  open: boolean;
  title: string;
  message: ReactNode;
  details?: string;
  actions: MessageModalAction[];
  onClose: () => void;
}

export default function MessageModal({
  open,
  title,
  message,
  details,
  actions,
  onClose,
}: MessageModalProps) {
  if (!open) return null;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="panel run-config-modal message-modal"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="run-config-header">
          <h3 style={{ margin: 0 }}>{title}</h3>
          <button type="button" className="secondary" onClick={onClose}>Close</button>
        </div>

        <p className="message-modal-text">{message}</p>
        {details && <pre className="message-modal-pre">{details}</pre>}

        <div className="run-config-actions">
          {actions.map((action) => (
            <button
              key={action.label}
              type="button"
              className={action.kind === 'danger' ? 'danger-button' : (action.kind ?? 'secondary')}
              onClick={action.onClick}
              disabled={action.disabled}
            >
              {action.label}
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
