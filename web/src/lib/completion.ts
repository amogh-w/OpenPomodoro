// A focus session is "complete" when it ran for the whole time it was planned for (running over counts). One that
// was stopped early is still saved and counted in the totals, but it is marked so the difference is visible.
import type { SessionRecord } from "./types";

/** True for a finished focus session, false for one ended early, null when it is a break or has no plan to compare with. */
export function completion(s: SessionRecord): boolean | null {
  if (s.kind !== "focus" || s.planned_ms == null || s.duration_ms == null) return null;
  return s.duration_ms >= s.planned_ms;
}

/** How many focus sessions were completed, how many were rated at all, and the share completed (null if none rated). */
export function completionRate(sessions: SessionRecord[]): { completed: number; rated: number; rate: number | null } {
  let completed = 0;
  let rated = 0;
  for (const s of sessions) {
    const done = completion(s);
    if (done === null) continue;
    rated += 1;
    if (done) completed += 1;
  }
  return { completed, rated, rate: rated > 0 ? completed / rated : null };
}
