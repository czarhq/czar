import { useState, useEffect, useRef } from 'react';
import hljs from 'highlight.js/lib/core';
import yaml from 'highlight.js/lib/languages/yaml';
import 'highlight.js/styles/github-dark-dimmed.css';

hljs.registerLanguage('yaml', yaml);

interface WorkflowYamlPanelProps {
  sourceYaml?: string | null;
}

export default function WorkflowYamlPanel({ sourceYaml }: WorkflowYamlPanelProps) {
  const [open, setOpen] = useState(false);
  const codeRef = useRef<HTMLElement>(null);
  const hasYaml = Boolean(sourceYaml && sourceYaml.trim().length > 0);

  useEffect(() => {
    if (open && codeRef.current && hasYaml) {
      codeRef.current.removeAttribute('data-highlighted');
      hljs.highlightElement(codeRef.current);
    }
  }, [open, sourceYaml, hasYaml]);

  return (
    <>
      <button
        type="button"
        className="secondary subtle-control"
        style={{ textTransform: 'lowercase', letterSpacing: '0.02em' }}
        disabled={!hasYaml}
        onClick={() => setOpen(true)}
        title="View workflow YAML"
        aria-label="View workflow YAML"
      >
        yaml
      </button>

      {open && hasYaml && (
        <div
          style={{
            position: 'fixed',
            inset: 0,
            zIndex: 1000,
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            background: 'var(--overlay-bg)',
            backdropFilter: 'blur(4px)',
          }}
          onClick={() => setOpen(false)}
        >
          <div
            style={{
              width: '90%',
              maxWidth: '800px',
              maxHeight: '80vh',
              background: 'var(--panel)',
              border: '1px solid var(--border)',
              borderRadius: '10px',
              boxShadow: 'var(--shadow)',
              display: 'flex',
              flexDirection: 'column',
              overflow: 'hidden',
            }}
            onClick={(e) => e.stopPropagation()}
          >
            <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '12px 16px', borderBottom: '1px solid var(--border)', flexShrink: 0 }}>
              <h2 style={{ margin: 0, fontSize: '13px', fontWeight: 600, color: 'var(--muted)', textTransform: 'uppercase', letterSpacing: '0.08em' }}>YAML</h2>
              <button type="button" className="secondary" style={{ padding: '4px 10px', fontSize: '13px' }} onClick={() => setOpen(false)}>
                Close
              </button>
            </div>
            <pre style={{ margin: 0, overflowY: 'auto', background: 'var(--panel-2)' }}>
              <code ref={codeRef} className="language-yaml" style={{ fontSize: '13px', fontFamily: '"JetBrains Mono", ui-monospace, monospace' }}>
                {sourceYaml}
              </code>
            </pre>
          </div>
        </div>
      )}
    </>
  );
}
