// Plain helpers for the diagnostics log, kept apart from the reactive store so they can be tested on their own.
export interface DiagLine {
  /** Unix milliseconds. */
  at: number;
  area: string;
  message: string;
}

/** The text to paste into a report: facts about this device first, then the lines, oldest first. */
export function report(facts: Record<string, string>, lines: readonly DiagLine[]): string {
  const head = Object.entries(facts).map(([k, v]) => `${k}: ${v}`);
  const stamp = (ms: number): string => new Date(ms).toISOString().slice(11, 23);
  const body = lines.map((l) => `${stamp(l.at)} ${l.area} ${l.message}`);
  return [...head, "", ...(body.length > 0 ? body : ["(nothing noted)"])].join("\n");
}

