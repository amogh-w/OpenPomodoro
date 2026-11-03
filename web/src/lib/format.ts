const pad = (n: number): string => String(n).padStart(2, "0");

function clock(totalSeconds: number): string {
  const h = Math.floor(totalSeconds / 3600);
  const m = Math.floor((totalSeconds % 3600) / 60);
  const s = totalSeconds % 60;
  return h > 0 ? `${h}:${pad(m)}:${pad(s)}` : `${pad(m)}:${pad(s)}`;
}

/**
 * Time left, rounded up to the second: a fresh 25-minute phase reads 25:00 for its whole first second and
 * 00:00 only when it has really ended.
 */
export function formatRemaining(ms: number): string {
  return clock(Math.ceil(Math.max(0, ms) / 1000));
}

/** Time past the end or time spent, rounded down: it reads 00:00 until a full second has passed. */
export function formatElapsed(ms: number): string {
  return clock(Math.floor(Math.max(0, ms) / 1000));
}

/** Whole minutes for short labels (page title, app badge), rounded up like the countdown. */
export function minutesLeft(ms: number): number {
  return Math.ceil(Math.max(0, ms) / 60_000);
}

/** Longer spans for totals: `3h 25m`, `45m`, `0m`. Minutes are rounded to the nearest. */
export function formatSpan(ms: number): string {
  const minutes = Math.round(Math.max(0, ms) / 60_000);
  const h = Math.floor(minutes / 60);
  const m = minutes % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

/** How long ago, in the reader's language: "5 minutes ago", "yesterday". */
export function formatAgo(thenMs: number, nowMs: number, locale?: string): string {
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto" });
  const seconds = Math.round((thenMs - nowMs) / 1000);
  const steps: [Intl.RelativeTimeFormatUnit, number][] = [
    ["day", 86_400],
    ["hour", 3_600],
    ["minute", 60],
  ];
  for (const [unit, size] of steps) {
    if (Math.abs(seconds) >= size) return rtf.format(Math.trunc(seconds / size), unit);
  }
  return rtf.format(0, "second");
}
