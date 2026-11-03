const pad = (n: number): string => String(n).padStart(2, "0");

/** A time as the value of an `<input type="datetime-local">`: local time, minutes only (`2026-11-03T14:05`). */
export function toLocalInput(ms: number): string {
  const d = new Date(ms);
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}T${pad(d.getHours())}:${pad(d.getMinutes())}`;
}

/** The reverse: Unix ms for a local `datetime-local` value, or null if it is empty or not a date. */
export function fromLocalInput(value: string): number | null {
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}$/.test(value)) return null;
  const ms = new Date(value).getTime();
  return Number.isFinite(ms) ? ms : null;
}
