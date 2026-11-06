// Special breaks: a break at a fixed time of day (lunch, a snack) that is offered on top of the normal cycle and
// does not move it on. Kept per device, like the daily reminder. Pure parsing and "is one due" logic.

export interface SpecialBreak {
  id: string;
  label: string;
  /** "HH:MM", local time. */
  time: string;
  minutes: number;
  /** Monday first. */
  days: boolean[];
}

export const MAX_SPECIAL_BREAKS = 6;
/** A special break is still offered this long after its time (the app may have been busy or opened late). */
export const SPECIAL_GRACE_MIN = 20;
const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/;

export const weekdays = (): boolean[] => [true, true, true, true, true, true, true];

/** Read stored breaks, dropping anything malformed and keeping at most `MAX_SPECIAL_BREAKS`. */
export function parseSpecialBreaks(raw: unknown): SpecialBreak[] {
  if (!Array.isArray(raw)) return [];
  const out: SpecialBreak[] = [];
  const seen = new Set<string>();
  for (const item of raw) {
    if (typeof item !== "object" || item === null) continue;
    const o = item as Record<string, unknown>;
    if (typeof o.id !== "string" || o.id === "" || seen.has(o.id)) continue;
    if (typeof o.time !== "string" || !TIME.test(o.time)) continue;
    if (typeof o.minutes !== "number" || !Number.isFinite(o.minutes)) continue;
    const days =
      Array.isArray(o.days) && o.days.length === 7 && o.days.every((d) => typeof d === "boolean")
        ? (o.days as boolean[])
        : weekdays();
    seen.add(o.id);
    out.push({
      id: o.id,
      label: typeof o.label === "string" ? o.label.slice(0, 40) : "",
      time: o.time,
      minutes: Math.min(120, Math.max(1, Math.round(o.minutes))),
      days,
    });
    if (out.length === MAX_SPECIAL_BREAKS) break;
  }
  return out;
}

const minutesOf = (hhmm: string): number => Number(hhmm.slice(0, 2)) * 60 + Number(hhmm.slice(3, 5));

/** Local `YYYY-MM-DD`. */
export function dayKeyOf(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}

/**
 * The break to offer now, if any: on one of its days, from its time until the grace has passed, and not yet
 * handled today (`handled` maps a break id to the day it was started or dismissed). The earliest time wins.
 */
export function dueSpecialBreak(date: Date, list: readonly SpecialBreak[], handled: Record<string, string>): SpecialBreak | null {
  const today = dayKeyOf(date);
  const weekdayFromMonday = (date.getDay() + 6) % 7;
  const now = date.getHours() * 60 + date.getMinutes();
  const due = list.filter((b) => {
    if (!b.days[weekdayFromMonday] || handled[b.id] === today) return false;
    const at = minutesOf(b.time);
    return now >= at && now < at + SPECIAL_GRACE_MIN;
  });
  due.sort((a, b) => minutesOf(a.time) - minutesOf(b.time));
  return due[0] ?? null;
}
