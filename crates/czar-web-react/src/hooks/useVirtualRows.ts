import { useCallback, useEffect, useMemo, useRef, useState, type RefObject, type UIEvent } from 'react';

interface UseVirtualRowsOptions {
  count: number;
  rowHeight: number;
  overscan?: number;
  enabled?: boolean;
}

interface UseVirtualRowsResult<T extends HTMLElement> {
  containerRef: RefObject<T>;
  onScroll: (event: UIEvent<T>) => void;
  start: number;
  end: number;
  paddingTop: number;
  paddingBottom: number;
}

export function useVirtualRows<T extends HTMLElement = HTMLElement>({
  count,
  rowHeight,
  overscan = 8,
  enabled = true,
}: UseVirtualRowsOptions): UseVirtualRowsResult<T> {
  const containerRef = useRef<T>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportHeight, setViewportHeight] = useState(0);

  const onScroll = useCallback((event: UIEvent<T>) => {
    setScrollTop(event.currentTarget.scrollTop);
  }, []);

  useEffect(() => {
    const element = containerRef.current;
    if (!element || typeof ResizeObserver === 'undefined') return;
    const syncHeight = () => setViewportHeight(element.clientHeight);
    syncHeight();
    const observer = new ResizeObserver(syncHeight);
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const virtualize = enabled && count > 0 && viewportHeight > 0;

  return useMemo(() => {
    if (!virtualize) {
      return {
        containerRef,
        onScroll,
        start: 0,
        end: count,
        paddingTop: 0,
        paddingBottom: 0,
      };
    }

    const visibleCount = Math.max(1, Math.ceil(viewportHeight / rowHeight));
    const start = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
    const end = Math.min(count, start + visibleCount + overscan * 2);

    return {
      containerRef,
      onScroll,
      start,
      end,
      paddingTop: start * rowHeight,
      paddingBottom: Math.max(0, (count - end) * rowHeight),
    };
  }, [count, onScroll, overscan, rowHeight, scrollTop, viewportHeight, virtualize]);
}
