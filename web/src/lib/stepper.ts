/**
 * Next value for a minutes stepper: single minutes up to 5 (handy for trying things out), then multiples of 5.
 * Always stays within `low..high`.
 */
export function stepMinutes(minutes: number, direction: 1 | -1, low: number, high: number): number {
  let next: number;
  if (direction > 0) next = minutes < 5 ? minutes + 1 : Math.floor(minutes / 5) * 5 + 5;
  else next = minutes <= 5 ? minutes - 1 : Math.ceil(minutes / 5) * 5 - 5;
  return Math.min(high, Math.max(low, next));
}
