// Arranging sessions for the History screen, in the person's local time.
import type { SessionRecord } from "./types";

const pad = (n: number): string => String(n).padStart(2, "0");

/** `YYYY-MM-DD` of an instant in the local time zone. */
export function localDayKey(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}

/** Local midnight at the start of a `YYYY-MM-DD` day. */
export function dayStartMs(key: string): number {
  const [y, m, d] = key.split("-").map(Number) as [number, number, number];
  return new Date(y, m - 1, d).getTime();
}

/** The local start of a month (month0 is 0 for January) and of the next one. */
export function monthBounds(year: number, month0: number): { from: number; to: number } {
  return { from: new Date(year, month0, 1).getTime(), to: new Date(year, month0 + 1, 1).getTime() };
}

export interface DayGroup {
  day: string;
  focusMs: number;
  breakMs: number;
  focusCount: number;
  /** Oldest first, the order the day happened in. */
  sessions: SessionRecord[];
}

const spent = (s: SessionRecord): number => s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);

/** Sessions grouped by the local day they started, newest day first. */
export function groupByDay(sessions: SessionRecord[]): DayGroup[] {
  const groups = new Map<string, DayGroup>();
  for (const s of [...sessions].sort((a, b) => a.started_at - b.started_at)) {
    const day = localDayKey(s.started_at);
    const group = groups.get(day) ?? { day, focusMs: 0, breakMs: 0, focusCount: 0, sessions: [] };
    group.sessions.push(s);
    if (s.kind === "focus") {
      group.focusMs += spent(s);
      group.focusCount += 1;
    } else {
      group.breakMs += spent(s);
    }
    groups.set(day, group);
  }
  return [...groups.values()].sort((a, b) => (a.day < b.day ? 1 : -1));
}

export interface TimelineBlock {
  id: string;
  kind: SessionRecord["kind"];
  category: string | null;
  /** 0 to 100: where the block starts and how wide it is, as a percentage of the 24 hours. */
  left: number;
  width: number;
}

const DAY = 24 * 60 * 60_000;

/** Blocks for a day's strip: each session clipped to the day, with a minimum width so a short one stays visible. */
export function timelineBlocks(sessions: SessionRecord[], dayStart: number, dayLength: number = DAY): TimelineBlock[] {
  const blocks: TimelineBlock[] = [];
  for (const s of sessions) {
    const start = Math.max(s.started_at, dayStart);
    const end = Math.min(s.ended_at, dayStart + dayLength);
    if (end <= start && s.ended_at > s.started_at) continue; // entirely outside the day
    const left = ((start - dayStart) / dayLength) * 100;
    const width = Math.max(0.6, ((Math.max(end, start) - start) / dayLength) * 100);
    blocks.push({ id: s.id, kind: s.kind, category: s.category ?? null, left, width: Math.min(width, 100 - left) });
  }
  return blocks;
}

export interface MoodTally {
  focused: number;
  neutral: number;
  distracted: number;
}

/** How many focus sessions were reflected on as focused, neutral and distracted (unrated ones are not counted). */
export function moodTally(sessions: SessionRecord[]): MoodTally {
  const tally: MoodTally = { focused: 0, neutral: 0, distracted: 0 };
  for (const s of sessions) if (s.kind === "focus" && s.mood) tally[s.mood] += 1;
  return tally;
}
