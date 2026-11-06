// A calendar file (.ics) read for the day view's overlay. Read-only and local: the file is read in the browser and
// never uploaded. Understands plain timed events; repeating events, all-day events and time zones other than UTC
// or "local" are not expanded or converted (a zone name is treated as the local zone).

export interface CalEvent {
  id: string;
  /** Unix milliseconds. */
  start: number;
  end: number;
  title: string;
}

export const MAX_EVENTS = 5000;
export const MAX_FILE_BYTES = 5 * 1024 * 1024;

/** Undo the format's line folding: a line that starts with a space or tab continues the one before. */
function unfold(text: string): string[] {
  const lines: string[] = [];
  for (const raw of text.split(/\r\n|\n|\r/)) {
    if ((raw.startsWith(" ") || raw.startsWith("\t")) && lines.length > 0) lines[lines.length - 1] += raw.slice(1);
    else lines.push(raw);
  }
  return lines;
}

function unescapeText(value: string): string {
  return value.replace(/\\([nN,;\\])/g, (_, c: string) => (c === "n" || c === "N" ? " " : c));
}

/** A `DTSTART`-style value as Unix ms; null for all-day dates and anything unreadable. */
export function parseIcsTime(value: string): number | null {
  const m = /^(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})(Z?)$/.exec(value.trim());
  if (!m) return null;
  const [y, mo, d, h, mi, s] = m.slice(1, 7).map(Number) as [number, number, number, number, number, number];
  if (mo < 1 || mo > 12 || d < 1 || d > 31 || h > 23 || mi > 59 || s > 60) return null;
  const ms = m[7] === "Z" ? Date.UTC(y, mo - 1, d, h, mi, s) : new Date(y, mo - 1, d, h, mi, s).getTime();
  return Number.isNaN(ms) ? null : ms;
}

/** Events that overlap `[from, to)`, soonest first, at most `MAX_EVENTS`. */
export function parseIcs(text: string, from: number, to: number): CalEvent[] {
  const out: CalEvent[] = [];
  let current: Partial<CalEvent> & { ok: boolean } | null = null;
  let n = 0;
  for (const line of unfold(text)) {
    if (line === "BEGIN:VEVENT") {
      current = { ok: true };
      continue;
    }
    if (line === "END:VEVENT") {
      if (current && current.ok && current.start !== undefined) {
        const end = current.end !== undefined && current.end > current.start ? current.end : current.start + 30 * 60_000;
        if (end > from && current.start < to) out.push({ id: current.id ?? `e${n}`, start: current.start, end, title: current.title ?? "" });
        n += 1;
      }
      current = null;
      if (out.length >= MAX_EVENTS) break;
      continue;
    }
    if (!current) continue;
    const colon = line.indexOf(":");
    if (colon < 0) continue;
    const name = line.slice(0, colon).split(";")[0]?.toUpperCase() ?? "";
    const value = line.slice(colon + 1);
    if (name === "DTSTART") {
      const t = parseIcsTime(value);
      if (t === null) current.ok = false; // all-day or unreadable: leave it out
      else current.start = t;
    } else if (name === "DTEND") {
      const t = parseIcsTime(value);
      if (t !== null) current.end = t;
    } else if (name === "SUMMARY") current.title = unescapeText(value).slice(0, 120);
    else if (name === "UID") current.id = value.slice(0, 120);
  }
  return out.sort((a, b) => a.start - b.start);
}
