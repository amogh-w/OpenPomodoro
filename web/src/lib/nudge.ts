// The presence nudge: a soft sound every N minutes while a focus session runs, a small "still with it?" without a
// notification. Pure; the caller keeps the memory between ticks and plays the sound.

export interface NudgeMemory {
  /** Identifies this run of this session; a new key starts counting again. */
  key: string;
  /** How many intervals had already passed when last looked at. */
  count: number;
}

export const emptyNudgeMemory: NudgeMemory = { key: "", count: 0 };

export interface NudgeInput {
  key: string;
  /** Milliseconds of focus so far, pauses not counted. */
  elapsedMs: number;
  /** Counting down, not paused and not over. */
  running: boolean;
}

/**
 * One nudge when another whole interval has passed. Coming back to a session already under way, or after sleeping
 * through several intervals, never plays a burst: the first look only learns where things stand and later looks
 * play at most one.
 */
export function nextNudge(
  input: NudgeInput | null,
  memory: NudgeMemory,
  intervalMs: number,
): { play: boolean; memory: NudgeMemory } {
  if (!input || intervalMs <= 0) return { play: false, memory: input ? memory : emptyNudgeMemory };
  const passed = Math.max(0, Math.floor(input.elapsedMs / intervalMs));
  if (memory.key !== input.key) return { play: false, memory: { key: input.key, count: passed } };
  if (!input.running) return { play: false, memory: { key: memory.key, count: Math.max(memory.count, passed) } };
  return passed > memory.count ? { play: true, memory: { key: memory.key, count: passed } } : { play: false, memory };
}
