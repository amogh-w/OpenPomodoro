// Pure helpers for the task list.
import type { SessionRecord, Task } from "./types";

/** All tasks in the order they were added (ids sort by time). A ticked task stays where it was, so nothing jumps. */
export function orderTasks(tasks: readonly Task[]): Task[] {
  return [...tasks].sort((a, b) => a.id.localeCompare(b.id));
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
