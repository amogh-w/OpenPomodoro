// Maths for the scrubbable duration dial. One full turn is 60 minutes; turning further keeps adding, so a
// 90-minute session is a turn and a half.

/** Angle in degrees from straight up, clockwise, of the point (x, y) around the centre (cx, cy). */
export function angleOf(cx: number, cy: number, x: number, y: number): number {
  const deg = (Math.atan2(x - cx, cy - y) * 180) / Math.PI;
  return (deg + 360) % 360;
}

/** The shortest signed turn from angle `from` to angle `to`, in -180..180, so crossing the top is seamless. */
export function turnBetween(from: number, to: number): number {
  return ((((to - from) % 360) + 540) % 360) - 180;
}

export interface DialRange {
  min: number;
  max: number;
  step: number;
}

export function snap(minutes: number, { min, max, step }: DialRange): number {
  const snapped = Math.round(minutes / step) * step;
  return Math.min(max, Math.max(min, snapped));
}

/** Minutes after turning `totalTurnDeg` (clockwise positive) from `startMinutes`, snapped and kept in range. */
export function scrub(startMinutes: number, totalTurnDeg: number, range: DialRange): number {
  return snap(startMinutes + (totalTurnDeg / 360) * 60, range);
}

/** Like `scrub` but not snapped: the exact (fractional) value under the pointer, kept in range, for smooth drawing. */
export function scrubRaw(startMinutes: number, totalTurnDeg: number, { min, max }: Pick<DialRange, "min" | "max">): number {
  return Math.min(max, Math.max(min, startMinutes + (totalTurnDeg / 360) * 60));
}

/** Where the handle sits on the ring, in degrees from the top, for a value in minutes. */
export function handleAngle(minutes: number): number {
  return ((minutes % 60) / 60) * 360;
}
