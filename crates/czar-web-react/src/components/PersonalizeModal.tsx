import { useState } from 'react';
import { createPortal } from 'react-dom';
import type { Theme } from '../appState';

const THEME_LABELS: Record<Theme, string> = {
  dark: 'Dark',
  light: 'Light',
  system: 'System',
};

interface PersonalizeModalProps {
  theme: Theme;
  onThemeChange: (theme: Theme) => void;
  logDetails: boolean;
  onLogDetailsChange: (value: boolean) => void;
}

export default function PersonalizeModal({ theme, onThemeChange, logDetails, onLogDetailsChange }: PersonalizeModalProps) {
  const [open, setOpen] = useState(false);

  return (
    <>
      <button
        className="secondary"
        title="Personalize"
        aria-label="Personalize"
        onClick={() => setOpen((v) => !v)}
        style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', width: 32, height: 32, padding: 0, borderRadius: '50%' }}
      >
        <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
          <circle cx="12" cy="8" r="4" />
          <path d="M4 20c0-4 3.6-7 8-7s8 3 8 7" />
        </svg>
      </button>

      {open && createPortal(
        <>
          <div
            style={{ position: 'fixed', inset: 0, zIndex: 9998, background: 'var(--overlay-bg)' }}
            onClick={() => setOpen(false)}
          />
          <div
            style={{
              position: 'fixed',
              top: '50%',
              left: '50%',
              transform: 'translate(-50%, -50%)',
              zIndex: 9999,
              background: 'var(--surface-overlay-deep)',
              border: '1px solid var(--border)',
              borderRadius: '14px',
              padding: '24px',
              width: '320px',
              boxShadow: 'var(--shadow)',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: '20px' }}>
              <h2 style={{ margin: 0, fontSize: '16px', fontWeight: 600 }}>Personalize</h2>
              <button
                type="button"
                className="secondary"
                onClick={() => setOpen(false)}
                style={{ padding: '2px 8px', fontSize: '18px', lineHeight: 1, borderRadius: '6px' }}
                aria-label="Close"
              >
                ×
              </button>
            </div>
            <div style={{ fontSize: '11px', fontWeight: 600, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.08em', marginBottom: '8px' }}>
              Theme
            </div>
            <div style={{ display: 'flex', gap: '4px' }}>
              {(['system', 'light', 'dark'] as const).map((t) => (
                <button
                  key={t}
                  type="button"
                  onClick={() => onThemeChange(t)}
                  style={{
                    flex: 1,
                    padding: '7px 0',
                    borderRadius: '6px',
                    fontSize: '13px',
                    fontWeight: theme === t ? 600 : 400,
                    background: theme === t ? 'var(--interactive-bg-tint)' : 'none',
                    border: theme === t ? '1px solid var(--interactive-border)' : '1px solid var(--border)',
                    color: theme === t ? 'var(--accent)' : 'var(--text)',
                    cursor: 'pointer',
                    boxShadow: 'none',
                    transform: 'none',
                  }}
                >
                  {THEME_LABELS[t]}
                </button>
              ))}
            </div>
            <div style={{ fontSize: '11px', fontWeight: 600, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.08em', marginTop: '16px', marginBottom: '8px' }}>
              Logs
            </div>
            <div style={{ display: 'flex', gap: '4px' }}>
              {([false, true] as const).map((v) => (
                <button
                  key={String(v)}
                  type="button"
                  onClick={() => onLogDetailsChange(v)}
                  style={{
                    flex: 1,
                    padding: '7px 0',
                    borderRadius: '6px',
                    fontSize: '13px',
                    fontWeight: logDetails === v ? 600 : 400,
                    background: logDetails === v ? 'var(--interactive-bg-tint)' : 'none',
                    border: logDetails === v ? '1px solid var(--interactive-border)' : '1px solid var(--border)',
                    color: logDetails === v ? 'var(--accent)' : 'var(--text)',
                    cursor: 'pointer',
                    boxShadow: 'none',
                    transform: 'none',
                  }}
                >
                  {v ? 'With details' : 'Clean'}
                </button>
              ))}
            </div>
          </div>
        </>,
        document.body
      )}
    </>
  );
}
