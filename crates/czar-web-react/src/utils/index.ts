type DateOrder = 'DMY' | 'MDY' | 'YMD';
type TimeMode = '24h' | '12h';
type TimezoneMode = 'local' | 'utc';

export interface RunTimestampLike {
  created_at: string;
  started_at?: string;
}

export interface DateTimeDisplayConfig {
  dateOrder: DateOrder;
  dateSeparator: string;
  showTime: boolean;
  showSeconds: boolean;
  timeMode: TimeMode;
  timezone: TimezoneMode;
}

const dateTimeDisplayConfig: DateTimeDisplayConfig = {
  dateOrder: 'DMY',
  dateSeparator: '-',
  showTime: true,
  showSeconds: false,
  timeMode: '24h',
  timezone: 'local',
};

export function getDateTimeDisplayConfig(): DateTimeDisplayConfig {
  return { ...dateTimeDisplayConfig };
}

export function setDateTimeDisplayConfig(next: Partial<DateTimeDisplayConfig>): void {
  Object.assign(dateTimeDisplayConfig, next);
}

function pad2(value: number): string {
  return value.toString().padStart(2, '0');
}

export function fmtDateTime(s: string): string {
  const date = new Date(s);
  if (!Number.isFinite(date.getTime())) return s;

  const read = dateTimeDisplayConfig.timezone === 'utc'
    ? {
        day: date.getUTCDate(),
        month: date.getUTCMonth() + 1,
        year: date.getUTCFullYear(),
        hours: date.getUTCHours(),
        minutes: date.getUTCMinutes(),
        seconds: date.getUTCSeconds(),
      }
    : {
        day: date.getDate(),
        month: date.getMonth() + 1,
        year: date.getFullYear(),
        hours: date.getHours(),
        minutes: date.getMinutes(),
        seconds: date.getSeconds(),
      };

  const dateByOrder: Record<DateOrder, string> = {
    DMY: `${pad2(read.day)}${dateTimeDisplayConfig.dateSeparator}${pad2(read.month)}${dateTimeDisplayConfig.dateSeparator}${read.year}`,
    MDY: `${pad2(read.month)}${dateTimeDisplayConfig.dateSeparator}${pad2(read.day)}${dateTimeDisplayConfig.dateSeparator}${read.year}`,
    YMD: `${read.year}${dateTimeDisplayConfig.dateSeparator}${pad2(read.month)}${dateTimeDisplayConfig.dateSeparator}${pad2(read.day)}`,
  };

  if (!dateTimeDisplayConfig.showTime) {
    return dateByOrder[dateTimeDisplayConfig.dateOrder];
  }

  if (dateTimeDisplayConfig.timeMode === '12h') {
    const suffix = read.hours >= 12 ? 'PM' : 'AM';
    const h12 = read.hours % 12 || 12;
    const time = dateTimeDisplayConfig.showSeconds
      ? `${pad2(h12)}:${pad2(read.minutes)}:${pad2(read.seconds)} ${suffix}`
      : `${pad2(h12)}:${pad2(read.minutes)} ${suffix}`;
    return `${dateByOrder[dateTimeDisplayConfig.dateOrder]} ${time}`;
  }

  const time24 = dateTimeDisplayConfig.showSeconds
    ? `${pad2(read.hours)}:${pad2(read.minutes)}:${pad2(read.seconds)}`
    : `${pad2(read.hours)}:${pad2(read.minutes)}`;
  return `${dateByOrder[dateTimeDisplayConfig.dateOrder]} ${time24}`;
}

export function fmtDuration(start: string, end: string): string {
  try {
    const secs = Math.floor((new Date(end).getTime() - new Date(start).getTime()) / 1000);
    if (secs < 60) return `${secs}s`;
    if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
    return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
  } catch {
    return '?';
  }
}

export function getRunOccurredAt(run: RunTimestampLike): string {
  return run.started_at ?? run.created_at;
}

export function fmtRunOccurredAt(run: RunTimestampLike): string {
  return fmtDateTime(getRunOccurredAt(run));
}

export function fmtRunOccurredAtLabel(run: RunTimestampLike): string {
  const label = run.started_at ? 'Ran' : 'Queued';
  return `${label}: ${fmtRunOccurredAt(run)}`;
}

export function statusClass(status: string): string {
  const s = status.toLowerCase();
  if (s === 'success') return 'ok';
  if (['running', 'pending', 'paused', 'dispatched'].includes(s)) return 'pending';
  if (s === 'skipped') return 'skip';
  return 'fail';
}

export function truncate(s: string, maxLen: number): string {
  return s.length <= maxLen ? s : `${s.substring(0, maxLen)}...`;
}

export function fmtRelativeTime(s: string): string {
  const secs = Math.floor((Date.now() - new Date(s).getTime()) / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months}mo ago`;
  return `${Math.floor(months / 12)}y ago`;
}
