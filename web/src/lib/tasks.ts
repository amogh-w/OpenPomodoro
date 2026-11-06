// Pure helpers for the task list.
import type { Task } from "./types";

/**
 * Open tasks in the order they were added (ids sort by time), and done tasks with the most recently ticked off
 * first. A task without `done_at` that is marked done sorts last.
 */
export function splitTasks(tasks: readonly Task[]): { open: Task[]; done: Task[] } {
  const open = tasks.filter((x) => !x.done).sort((a, b) => a.id.localeCompare(b.id));
  const done = tasks.filter((x) => x.done).sort((a, b) => (b.done_at ?? 0) - (a.done_at ?? 0));
  return { open, done };
}
