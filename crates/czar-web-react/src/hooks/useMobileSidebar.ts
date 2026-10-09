import { useEffect, useState } from 'react';

interface UseMobileSidebarResult {
  isMobileLayout: boolean;
  isSidebarOpen: boolean;
  setIsSidebarOpen: (open: boolean) => void;
}

export function useMobileSidebar(maxWidthPx: number): UseMobileSidebarResult {
  const [isMobileLayout, setIsMobileLayout] = useState(() => (
    typeof window !== 'undefined'
      ? window.matchMedia(`(max-width: ${maxWidthPx}px)`).matches
      : false
  ));
  const [isSidebarOpen, setIsSidebarOpen] = useState(false);

  useEffect(() => {
    if (typeof window === 'undefined') return;

    const mediaQuery = window.matchMedia(`(max-width: ${maxWidthPx}px)`);
    const onChange = () => {
      const mobile = mediaQuery.matches;
      setIsMobileLayout(mobile);
      if (!mobile) {
        setIsSidebarOpen(false);
      }
    };

    onChange();
    if (typeof mediaQuery.addEventListener === 'function') {
      mediaQuery.addEventListener('change', onChange);
      return () => mediaQuery.removeEventListener('change', onChange);
    }

    mediaQuery.addListener(onChange);
    return () => mediaQuery.removeListener(onChange);
  }, [maxWidthPx]);

  useEffect(() => {
    if (!isMobileLayout || !isSidebarOpen) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        setIsSidebarOpen(false);
      }
    };

    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    window.addEventListener('keydown', onKeyDown);

    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener('keydown', onKeyDown);
    };
  }, [isMobileLayout, isSidebarOpen]);

  return { isMobileLayout, isSidebarOpen, setIsSidebarOpen };
}
