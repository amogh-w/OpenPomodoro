// Pure helpers for the task list.
import type { SessionRecord, Task } from "./types";

/**
 * Open tasks in the order they were added (ids sort by time), and done tasks with the most recently ticked off
 * first. A task without `done_at` that is marked done sorts last.
 */
export function splitTasks(tasks: readonly Task[]): { open: Task[]; done: Task[] } {
  const open = tasks.filter((x) => !x.done).sort((a, b) => a.id.localeCompare(b.id));
  const done = tasks.filter((x) => x.done).sort((a, b) => (b.done_at ?? 0) - (a.done_at ?? 0));
  return { open, done };
}

/** Focus time spent on each task, from the sessions that carry its id. */
export function timePerTask(sessions: readonly SessionRecord[]): Map<string, number> {
  const out = new Map<string, number>();
  for (const s of sessions) {
    if (s.kind !== "focus" || !s.task) continue;
    out.set(s.task, (out.get(s.task) ?? 0) + (s.duration_ms ?? Math.max(0, s.ended_at - s.started_at)));
  }
  return out;
}
