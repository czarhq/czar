import { useState, useEffect, useCallback } from 'react';

/**
 * Manages the open/close state of the fuzzy search modal and the
 * Ctrl+P / Ctrl+K keyboard shortcut that toggles it.
 */
export function useFuzzySearch(
  onSelectRun: (runId: string) => void,
  onSelectWorkflow: (name: string) => void,
) {
  const [isOpen, setIsOpen] = useState(false);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && (e.key === 'p' || e.key === 'k')) {
        if (
          document.activeElement &&
          ['INPUT', 'TEXTAREA', 'SELECT'].includes(
            (document.activeElement as HTMLElement).tagName
          )
        ) {
          return;
        }
        e.preventDefault();
        setIsOpen((open) => !open);
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  const handleSelectRun = useCallback(
    (runId: string) => {
      setIsOpen(false);
      onSelectRun(runId);
    },
    [onSelectRun]
  );

  const handleSelectWorkflow = useCallback(
    (name: string) => {
      setIsOpen(false);
      onSelectWorkflow(name);
    },
    [onSelectWorkflow]
  );

  const close = useCallback(() => setIsOpen(false), []);

  return { isOpen, handleSelectRun, handleSelectWorkflow, close };
}
