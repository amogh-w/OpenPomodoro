// Totals and streaks from the server's per-day sums. Days are `YYYY-MM-DD` strings in the person's own time zone;
// the arithmetic on them is done in UTC so that daylight saving can never skip or repeat a day.
import type { DayTotal } from "./types";

export interface Stats {
  todayMs: number;
  /** Monday to today. */
  weekMs: number;
  allTimeMs: number;
  sessions: number;
  /** Consecutive days with focus, ending today (or yesterday, while today is still open). */
  currentStreak: number;
  longestStreak: number;
}

const DAY_MS = 86_400_000;

const toUtc = (key: string): number => {
  const [y, m, d] = key.split("-").map(Number) as [number, number, number];
  return Date.UTC(y, m - 1, d);
};

const fromUtc = (ms: number): string => new Date(ms).toISOString().slice(0, 10);

export function addDays(key: string, days: number): string {
  return fromUtc(toUtc(key) + days * DAY_MS);
}

/** The Monday of the week containing `key`. */
export function weekStart(key: string): string {
  const weekday = new Date(toUtc(key)).getUTCDay(); // 0 is Sunday
  return addDays(key, -((weekday + 6) % 7));
}

/** The calendar day of `ms` at `offsetMinutes` east of UTC. */
export function dayKey(ms: number, offsetMinutes: number): string {
  return fromUtc(ms + offsetMinutes * 60_000);
}

export function computeStats(days: DayTotal[], today: string): Stats {
  const worked = new Set(days.filter((d) => d.focus_count > 0).map((d) => d.day));
  const monday = weekStart(today);

  let todayMs = 0;
  let weekMs = 0;
  let allTimeMs = 0;
  let sessions = 0;
  for (const d of days) {
    allTimeMs += d.focus_ms;
    sessions += d.focus_count;
    if (d.day === today) todayMs += d.focus_ms;
    if (d.day >= monday && d.day <= today) weekMs += d.focus_ms;
  }

  let longestStreak = 0;
  let run = 0;
  let previous: string | null = null;
  for (const day of [...worked].sort()) {
    run = previous !== null && addDays(previous, 1) === day ? run + 1 : 1;
    longestStreak = Math.max(longestStreak, run);
    previous = day;
  }

  // A streak is still alive while today is open: an empty today does not break yesterday's run.
  let cursor = worked.has(today) ? today : addDays(today, -1);
  let currentStreak = 0;
  while (worked.has(cursor)) {
    currentStreak += 1;
    cursor = addDays(cursor, -1);
  }

  return { todayMs, weekMs, allTimeMs, sessions, currentStreak, longestStreak };
}
