// The calendar overlay of this device: events read from a file the person chose, kept in IndexedDB.
import { kvDelete, kvGet, kvSet } from "./idb";
import { type CalEvent, MAX_FILE_BYTES, parseIcs } from "./ics";

const KEY = "calendar-overlay";
const YEAR = 366 * 86_400_000;

export const calendar = $state<{ events: CalEvent[]; name: string }>({ events: [], name: "" });

export async function loadCalendar(): Promise<void> {
  const saved = await kvGet<{ name: string; events: CalEvent[] }>(KEY).catch(() => undefined);
  if (saved) {
    calendar.events = saved.events;
    calendar.name = saved.name;
  }
}

/** Read a chosen .ics file. Throws `Error("too_large")` or `Error("empty")`. */
export async function importCalendar(file: File): Promise<number> {
  if (file.size > MAX_FILE_BYTES) throw new Error("too_large");
  const now = Date.now();
  const events = parseIcs(await file.text(), now - YEAR, now + YEAR);
  if (events.length === 0) throw new Error("empty");
  calendar.events = events;
  calendar.name = file.name.slice(0, 80);
  await kvSet(KEY, { name: calendar.name, events }).catch(() => {});
  return events.length;
}

export async function clearCalendar(): Promise<void> {
  calendar.events = [];
  calendar.name = "";
  await kvDelete(KEY).catch(() => {});
}
