// The special breaks of this device, and which ones were already handled today (all in localStorage).
import { dayKeyOf, MAX_SPECIAL_BREAKS, parseSpecialBreaks, type SpecialBreak, weekdays } from "./specialBreaks";

const LIST = "op_special_breaks";
const HANDLED = "op_special_handled";
const ALERTED = "op_special_alerted";

function readJson(key: string): unknown {
  try {
    const text = localStorage.getItem(key);
    return text === null ? null : JSON.parse(text);
  } catch {
    return null;
  }
}

function write(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // Unavailable storage means the choice lasts until the page closes.
  }
}

const dayMap = (key: string): Record<string, string> => {
  const raw = readJson(key);
  const out: Record<string, string> = {};
  if (typeof raw === "object" && raw !== null) {
    for (const [k, v] of Object.entries(raw)) if (typeof v === "string") out[k] = v;
  }
  return out;
};

export const special = $state({
  list: parseSpecialBreaks(readJson(LIST)),
  /** Break id -> the day it was started or dismissed. */
  handled: dayMap(HANDLED),
  /** Break id -> the day its sound or notification was shown. */
  alerted: dayMap(ALERTED),
});

const save = (): void => write(LIST, $state.snapshot(special.list));

export function addSpecialBreak(): void {
  if (special.list.length >= MAX_SPECIAL_BREAKS) return;
  special.list.push({ id: `sb${Date.now().toString(36)}`, label: "", time: "12:30", minutes: 30, days: weekdays() });
  save();
}

export function updateSpecialBreak(id: string, change: Partial<Omit<SpecialBreak, "id">>): void {
  // A cleared time or length field gives an empty value; keep the old one rather than losing the whole break.
  if (change.time === "") delete change.time;
  if (change.minutes !== undefined && !Number.isFinite(change.minutes)) delete change.minutes;
  const next = parseSpecialBreaks(special.list.map((b) => (b.id === id ? { ...$state.snapshot(b), ...change } : $state.snapshot(b))));
  special.list = next;
  save();
}

export function removeSpecialBreak(id: string): void {
  special.list = special.list.filter((b) => b.id !== id);
  save();
}

export function markHandled(id: string, now: Date = new Date()): void {
  special.handled[id] = dayKeyOf(now);
  write(HANDLED, $state.snapshot(special.handled));
}

export function markAlerted(id: string, now: Date = new Date()): void {
  special.alerted[id] = dayKeyOf(now);
  write(ALERTED, $state.snapshot(special.alerted));
}
