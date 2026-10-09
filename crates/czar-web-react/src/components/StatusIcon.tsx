import {
  CircleCheck,
  CircleX,
  CircleSlash,
  Loader2,
  Clock,
  CircleDot,
  CirclePause,
  ChevronsRight,
  CircleHelp,
} from 'lucide-react';
import type { LucideProps } from 'lucide-react';
import { statusClass } from '../utils';

const STATUS_ICON: Record<string, React.ComponentType<LucideProps>> = {
  success:    CircleCheck,
  failed:     CircleX,
  cancelled:  CircleSlash,
  canceled:   CircleSlash,
  running:    Loader2,
  pending:    Clock,
  dispatched: CircleDot,
  paused:     CirclePause,
  skipped:    ChevronsRight,
};

const STATUS_CSS_COLOR: Record<string, string> = {
  success:    'var(--badge-ok-text)',
  failed:     'var(--badge-fail-text)',
  cancelled:  'var(--badge-skip-text)',
  canceled:   'var(--badge-skip-text)',
  running:    'var(--badge-pending-text)',
  pending:    'var(--badge-pending-text)',
  dispatched: 'var(--badge-pending-text)',
  paused:     'var(--badge-pending-text)',
  skipped:    'var(--badge-skip-text)',
};

interface StatusIconProps {
  status: string;
  size?: number;
  className?: string;
  /** When true, applies the status color directly instead of inheriting from the parent. */
  colored?: boolean;
}

/** Icon only — colored by default; pass colored={false} to inherit color from parent (e.g. inside a badge). */
export function StatusIcon({ status, size = 12, className, colored = true }: StatusIconProps) {
  const key = status.toLowerCase();
  const Icon = STATUS_ICON[key] ?? CircleHelp;
  const isRunning = key === 'running';
  return (
    <Icon
      size={size}
      className={[isRunning ? 'status-icon-spin' : '', className].filter(Boolean).join(' ')}
      style={{ flexShrink: 0, color: colored ? (STATUS_CSS_COLOR[key] ?? 'var(--muted)') : undefined }}
    />
  );
}

/** Colored badge with icon + label — drop-in replacement for `.badge .ok/.fail/…` spans. */
export function StatusBadge({ status, size = 11 }: { status: string; size?: number }) {
  return (
    <span className={`badge ${statusClass(status)}`} style={{ fontSize: `${size}px` }}>
      <StatusIcon status={status} size={size + 1} colored={false} />
      {status}
    </span>
  );
}
