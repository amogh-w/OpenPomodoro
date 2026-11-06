// Numbers for the History overview of one month: totals, the average day, the best hour, the focus-to-break ratio,
// how sessions felt, the days as bars and the time per category. Pure, in the person's local time.
import { localDayKey } from "./days";
import type { Category, SessionRecord } from "./types";

const spent = (s: SessionRecord): number => s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);

export interface CategoryRow {
  /** Category id, or null for sessions without one. */
  id: string | null;
  focusMs: number;
  sessions: number;
  /** Share of all focus time, 0 to 1. */
  share: number;
  /** Change against the same category in the earlier period, as a fraction (0.5 is +50%); null if there was none. */
  change: number | null;
}

export interface Insights {
  focusMs: number;
  breakMs: number;
  sessions: number;
  /** Days with at least one focus session. */
  activeDays: number;
  /** Average focus time on an active day. */
  avgPerDayMs: number;
  /** Focus time per hour of the day (24 entries). */
  byHour: number[];
  /** The hour (0 to 23) with the most focus time, or null with no data. */
  bestHour: number | null;
  /** The hour in which the most time went to sessions rated distracted, or null if none were. */
  distractedHour: number | null;
  /** Focus time divided by break time; null when there were no breaks. */
  ratio: number | null;
  mood: { focused: number; neutral: number; distracted: number };
  /** Focus time per day, one entry for every day of the period (`YYYY-MM-DD` keys, in order). */
  days: { day: string; focusMs: number }[];
  categories: CategoryRow[];
  /** Total focus time against the earlier period as a fraction; null if there was no earlier focus. */
  change: number | null;
}

/** Add the part of [start, end) that falls in each hour of the day to `hours`. */
function spread(hours: number[], start: number, end: number): void {
  let cursor = start;
  while (cursor < end) {
    const d = new Date(cursor);
    const nextHour = new Date(d.getFullYear(), d.getMonth(), d.getDate(), d.getHours() + 1).getTime();
    const stop = Math.min(end, nextHour);
    hours[d.getHours()] = (hours[d.getHours()] ?? 0) + (stop - cursor);
    cursor = stop;
  }
}

const argmax = (values: number[]): number | null => {
  let best = -1;
  let at: number | null = null;
  values.forEach((v, i) => {
    if (v > best && v > 0) {
      best = v;
      at = i;
    }
  });
  return at;
};

/** `periodDays` lists every day of the period so that quiet days show as empty bars. */
export function computeInsights(sessions: SessionRecord[], earlier: SessionRecord[], periodDays: string[]): Insights {
  const focus = sessions.filter((s) => s.kind === "focus");
  const breaks = sessions.filter((s) => s.kind !== "focus");
  const focusMs = focus.reduce((n, s) => n + spent(s), 0);
  const breakMs = breaks.reduce((n, s) => n + spent(s), 0);

  const perDay = new Map<string, number>(periodDays.map((d) => [d, 0]));
  const byHour = new Array<number>(24).fill(0);
  const distractedHours = new Array<number>(24).fill(0);
  const mood = { focused: 0, neutral: 0, distracted: 0 };
  const perCategory = new Map<string | null, { ms: number; n: number }>();

  for (const s of focus) {
    const day = localDayKey(s.started_at);
    perDay.set(day, (perDay.get(day) ?? 0) + spent(s));
    spread(byHour, s.started_at, s.started_at + spent(s));
    if (s.mood) mood[s.mood] += 1;
    if (s.mood === "distracted") spread(distractedHours, s.started_at, s.started_at + spent(s));
    const row = perCategory.get(s.category ?? null) ?? { ms: 0, n: 0 };
    row.ms += spent(s);
    row.n += 1;
    perCategory.set(s.category ?? null, row);
  }

  const earlierFocus = earlier.filter((s) => s.kind === "focus");
  const earlierTotal = earlierFocus.reduce((n, s) => n + spent(s), 0);
  const earlierByCategory = new Map<string | null, number>();
  for (const s of earlierFocus) earlierByCategory.set(s.category ?? null, (earlierByCategory.get(s.category ?? null) ?? 0) + spent(s));

  const activeDays = [...perDay.values()].filter((ms) => ms > 0).length;
  const categories = [...perCategory.entries()]
    .map(([id, row]): CategoryRow => {
      const before = earlierByCategory.get(id) ?? 0;
      return { id, focusMs: row.ms, sessions: row.n, share: focusMs > 0 ? row.ms / focusMs : 0, change: before > 0 ? row.ms / before - 1 : null };
    })
    .sort((a, b) => b.focusMs - a.focusMs);

  return {
    focusMs,
    breakMs,
    sessions: focus.length,
    activeDays,
    avgPerDayMs: activeDays > 0 ? Math.round(focusMs / activeDays) : 0,
    byHour,
    bestHour: argmax(byHour),
    distractedHour: argmax(distractedHours),
    ratio: breakMs > 0 ? focusMs / breakMs : null,
    mood,
    days: periodDays.map((day) => ({ day, focusMs: perDay.get(day) ?? 0 })),
    categories,
    change: earlierTotal > 0 ? focusMs / earlierTotal - 1 : null,
  };
}

/** Every `YYYY-MM-DD` of a month (`month0` is 0 for January). */
export function daysOfMonth(year: number, month0: number): string[] {
  const count = new Date(year, month0 + 1, 0).getDate();
  return Array.from({ length: count }, (_, i) => localDayKey(new Date(year, month0, i + 1).getTime()));
}

export interface IntentionRow {
  /** The intention as first written (the grouping ignores case and spacing). */
  name: string;
  sessions: number;
  focusMs: number;
  share: number;
}

/** Focus time grouped by what each session was about, most time first. Sessions without an intention are left out. */
export function byIntention(sessions: SessionRecord[], limit = 10): IntentionRow[] {
  const groups = new Map<string, { name: string; sessions: number; focusMs: number }>();
  let total = 0;
  for (const s of sessions) {
    if (s.kind !== "focus") continue;
    const ms = s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);
    total += ms;
    const name = (s.intention ?? "").trim();
    if (name === "") continue;
    const key = name.toLowerCase().replace(/\s+/g, " ");
    const row = groups.get(key) ?? { name, sessions: 0, focusMs: 0 };
    row.sessions += 1;
    row.focusMs += ms;
    groups.set(key, row);
  }
  return [...groups.values()]
    .sort((a, b) => b.focusMs - a.focusMs)
    .slice(0, limit)
    .map((row) => ({ ...row, share: total > 0 ? row.focusMs / total : 0 }));
}

export interface GroupRow {
  name: string;
  focusMs: number;
  sessions: number;
  share: number;
  /** The categories in this group that had time. */
  categories: string[];
}

/** Focus time by category group, most time first. Categories without a group are not listed; none at all gives nothing. */
export function byGroup(sessions: SessionRecord[], categories: Category[]): GroupRow[] {
  const groupOf = new Map(categories.filter((c) => c.group).map((c) => [c.id, c] as const));
  if (groupOf.size === 0) return [];
  const rows = new Map<string, { focusMs: number; sessions: number; categories: Set<string> }>();
  let total = 0;
  for (const s of sessions) {
    if (s.kind !== "focus") continue;
    const ms = s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);
    total += ms;
    const category = s.category ? groupOf.get(s.category) : undefined;
    if (!category?.group) continue;
    const row = rows.get(category.group) ?? { focusMs: 0, sessions: 0, categories: new Set<string>() };
    row.focusMs += ms;
    row.sessions += 1;
    row.categories.add(category.name);
    rows.set(category.group, row);
  }
  return [...rows.entries()]
    .map(([name, r]) => ({ name, focusMs: r.focusMs, sessions: r.sessions, share: total > 0 ? r.focusMs / total : 0, categories: [...r.categories].sort() }))
    .sort((a, b) => b.focusMs - a.focusMs);
}
