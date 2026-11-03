// TypeScript port of crates/core/src/timer.rs. The Rust code is the source of truth; both pass
// test-vectors/timer.json. All times are Unix milliseconds. Functions are pure: they return a new Timer.

/** 5 minutes: the step for extending or shrinking a phase. */
export const EXTEND_STEP_MS = 5 * 60 * 1000;

export type TimerError = "invalid_total" | "already_paused" | "not_paused";

export interface Timer {
  readonly start: number;
  readonly total: number;
  readonly pauseBuffer: number;
  readonly pauseTs: number | null;
}

export type Result<T> = { ok: true; value: T } | { ok: false; error: TimerError };

export function newTimer(start: number, total: number): Result<Timer> {
  if (total <= 0) return { ok: false, error: "invalid_total" };
  return { ok: true, value: { start, total, pauseBuffer: 0, pauseTs: null } };
}

export const isPaused = (t: Timer): boolean => t.pauseTs !== null;

/** Time the phase has run; never negative (a clock set back reads 0). */
export function elapsed(t: Timer, now: number): number {
  const at = t.pauseTs ?? now;
  return Math.max(0, at - t.start - t.pauseBuffer);
}

export const remaining = (t: Timer, now: number): number => Math.max(0, t.total - elapsed(t, now));
export const overflow = (t: Timer, now: number): number => Math.max(0, elapsed(t, now) - t.total);
export const isEnded = (t: Timer, now: number): boolean => elapsed(t, now) >= t.total;

/** True when `now` is earlier than the timer's own timestamps, so the caller can write a diagnostic. */
export function clockWentBack(t: Timer, now: number): boolean {
  return now < t.start || (t.pauseTs !== null && now < t.pauseTs);
}

export function pause(t: Timer, now: number): Result<Timer> {
  if (t.pauseTs !== null) return { ok: false, error: "already_paused" };
  return { ok: true, value: { ...t, pauseTs: now } };
}

/** The paused span joins pauseBuffer; if the clock went back meanwhile the span counts as 0. */
export function resume(t: Timer, now: number): Result<Timer> {
  if (t.pauseTs === null) return { ok: false, error: "not_paused" };
  return { ok: true, value: { ...t, pauseTs: null, pauseBuffer: t.pauseBuffer + Math.max(0, now - t.pauseTs) } };
}

/**
 * Extend (delta > 0) or shrink (delta < 0). Elapsed is never reset.
 * Before the end: total += delta, but never below elapsed (shrinking past zero ends the phase now).
 * At or past the end: a positive delta starts a fresh countdown of delta from now; a shrink does nothing.
 */
export function extend(t: Timer, delta: number, now: number): Timer {
  const e = elapsed(t, now);
  if (e >= t.total) return delta > 0 ? { ...t, total: e + delta } : t;
  return { ...t, total: Math.max(t.total + delta, e) };
}
