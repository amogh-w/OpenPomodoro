// The optional tick: a faint click once a second while a phase counts down. Pure, like the other alert planners:
// the caller keeps the memory and plays the sound.

export interface TickMemory {
  key: string;
  /** Whole seconds already ticked for. */
  second: number;
}

export const emptyTickMemory: TickMemory = { key: "", second: 0 };

export interface TickInput {
  /** Identifies this run of this phase. */
  key: string;
  elapsedMs: number;
  /** Counting down: not paused and not over. */
  running: boolean;
}

/** At most one tick per call, and none for seconds that went by unseen (a sleeping tab never catches up in a burst). */
export function nextTick(input: TickInput | null, memory: TickMemory): { play: boolean; memory: TickMemory } {
  if (!input) return { play: false, memory: emptyTickMemory };
  const second = Math.max(0, Math.floor(input.elapsedMs / 1000));
  if (memory.key !== input.key) return { play: false, memory: { key: input.key, second } };
  if (!input.running) return { play: false, memory: { key: memory.key, second: Math.max(memory.second, second) } };
  return second > memory.second ? { play: true, memory: { key: memory.key, second } } : { play: false, memory };
}
