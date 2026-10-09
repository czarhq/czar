import { useState, useEffect, useCallback } from 'react';
import { type Theme, readTheme, saveTheme } from '../appState';

function getSystemTheme(): 'dark' | 'light' {
  if (typeof window === 'undefined') return 'dark';
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

function resolveEffective(theme: Theme): 'dark' | 'light' {
  if (theme !== 'system') return theme;
  return getSystemTheme();
}

export function useTheme() {
  const [theme, setThemeState] = useState<Theme>(readTheme);
  const [effectiveTheme, setEffectiveTheme] = useState<'dark' | 'light'>(() =>
    resolveEffective(readTheme())
  );

  // Apply theme to <html> element whenever the preference changes
  useEffect(() => {
    const effective = resolveEffective(theme);
    setEffectiveTheme(effective);
    document.documentElement.setAttribute('data-theme', effective);
  }, [theme]);

  // Track system preference changes when theme is set to 'system'
  useEffect(() => {
    if (theme !== 'system') return;
    const mq = window.matchMedia('(prefers-color-scheme: light)');
    const handler = () => {
      const effective = mq.matches ? 'light' : 'dark';
      setEffectiveTheme(effective);
      document.documentElement.setAttribute('data-theme', effective);
    };
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, [theme]);

  const setTheme = useCallback((next: Theme) => {
    saveTheme(next);
    setThemeState(next);
  }, []);

  return { theme, effectiveTheme, setTheme };
}
