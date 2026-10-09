import { useEffect, useState } from 'react';

export interface PollCadence {
  successMs: number;
  errorBaseMs: number;
  maxBackoffMs: number;
}

interface NetworkConnectionInfo {
  effectiveType?: 'slow-2g' | '2g' | '3g' | '4g';
  saveData?: boolean;
  addEventListener?: (type: 'change', listener: () => void) => void;
  removeEventListener?: (type: 'change', listener: () => void) => void;
  addListener?: (listener: () => void) => void;
  removeListener?: (listener: () => void) => void;
}

const DEFAULT_POLL_CADENCE: PollCadence = {
  successMs: 5000,
  errorBaseMs: 3000,
  maxBackoffMs: 15000,
};

function isLocalHostname(hostname: string): boolean {
  const lower = hostname.toLowerCase();
  return lower === 'localhost'
    || lower === '127.0.0.1'
    || lower === '::1'
    || lower.endsWith('.local');
}

function readConnectionInfo(): NetworkConnectionInfo | null {
  if (typeof navigator === 'undefined') return null;
  const nav = navigator as Navigator & {
    connection?: NetworkConnectionInfo;
    mozConnection?: NetworkConnectionInfo;
    webkitConnection?: NetworkConnectionInfo;
  };
  return nav.connection ?? nav.mozConnection ?? nav.webkitConnection ?? null;
}

function computePollCadence(): PollCadence {
  if (typeof window === 'undefined') return DEFAULT_POLL_CADENCE;

  const local = isLocalHostname(window.location.hostname);
  const connection = readConnectionInfo();
  const effectiveType = connection?.effectiveType ?? '4g';
  const saveData = Boolean(connection?.saveData);
  const coarsePointer = window.matchMedia('(pointer: coarse)').matches;
  const narrowViewport = window.matchMedia('(max-width: 900px)').matches;
  const mobileLike = coarsePointer || narrowViewport;

  if (local) {
    return {
      successMs: mobileLike ? 1800 : 1200,
      errorBaseMs: 1500,
      maxBackoffMs: 12000,
    };
  }

  if (saveData || effectiveType === 'slow-2g' || effectiveType === '2g') {
    return {
      successMs: 7000,
      errorBaseMs: 4000,
      maxBackoffMs: 20000,
    };
  }

  if (effectiveType === '3g' || mobileLike) {
    return {
      successMs: 5000,
      errorBaseMs: 3000,
      maxBackoffMs: 15000,
    };
  }

  return DEFAULT_POLL_CADENCE;
}

export function usePollCadence(): PollCadence {
  const [pollCadence, setPollCadence] = useState<PollCadence>(() => computePollCadence());

  useEffect(() => {
    if (typeof window === 'undefined') return;
    const connection = readConnectionInfo();

    const refreshCadence = () => {
      const next = computePollCadence();
      setPollCadence((prev) => (
        prev.successMs === next.successMs
        && prev.errorBaseMs === next.errorBaseMs
        && prev.maxBackoffMs === next.maxBackoffMs
          ? prev
          : next
      ));
    };

    refreshCadence();
    window.addEventListener('resize', refreshCadence);

    if (connection?.addEventListener) {
      connection.addEventListener('change', refreshCadence);
    } else if (connection?.addListener) {
      connection.addListener(refreshCadence);
    }

    return () => {
      window.removeEventListener('resize', refreshCadence);
      if (connection?.removeEventListener) {
        connection.removeEventListener('change', refreshCadence);
      } else if (connection?.removeListener) {
        connection.removeListener(refreshCadence);
      }
    };
  }, []);

  return pollCadence;
}
