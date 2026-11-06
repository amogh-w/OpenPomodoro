// What each alert sounds like, as short note patterns made with simple tones. Nothing is downloaded or bundled,
// so alerts work offline and there is no audio to license. Each kind has its own shape so they can be told
// apart without looking: a rising run for the end of focus, a falling pair for the end of a break, one soft tone
// for "ending soon", and plain repeated pings for overtime.

export type AlertKind = "warn" | "focus_end" | "break_end" | "focus_reminder" | "break_reminder" | "nudge";

export interface Note {
  /** Hertz. */
  freq: number;
  /** Seconds after the start of the sound. */
  at: number;
  /** Seconds the note lasts. */
  length: number;
}

const note = (freq: number, at: number, length: number): Note => ({ freq, at, length });

const SCORES: Record<AlertKind, Note[]> = {
  warn: [note(659.25, 0, 0.18)],
  focus_end: [note(523.25, 0, 0.2), note(659.25, 0.2, 0.2), note(783.99, 0.4, 0.45)],
  break_end: [note(783.99, 0, 0.22), note(587.33, 0.24, 0.5)],
  focus_reminder: [note(440, 0, 0.14), note(440, 0.22, 0.14)],
  break_reminder: [note(392, 0, 0.3)],
  nudge: [note(329.63, 0, 0.12)],
};

export function scoreFor(kind: AlertKind): readonly Note[] {
  return SCORES[kind];
}

export const ALERT_KINDS = Object.keys(SCORES) as AlertKind[];

/** Total length of a sound in seconds. */
export function durationOf(kind: AlertKind): number {
  return Math.max(...SCORES[kind].map((n) => n.at + n.length));
}
