// The garden: every finished focus session is a plant, sized by how much of its planned time was done, and every full
// cycle of sessions (the number of rounds before a long break) is a tree. All of it is worked out from the sessions
// that are already stored, so it needs no extra data and covers past history too.
import { localDayKey } from "./days";
import type { SessionRecord } from "./types";

/** 0 a sprout, 1 small, 2 medium, 3 fully grown. */
export type Stage = 0 | 1 | 2 | 3;

export interface Plant {
  id: string;
  stage: Stage;
  /** Fraction of the planned time that was done, 0 to 1. */
  size: number;
  /** Which of three shapes to draw; the same session always gets the same one. */
  variant: 0 | 1 | 2;
  category: string | null;
  startedAt: number;
  day: string;
}

export interface Tree {
  /** The id of the session that completed it. */
  id: string;
  completedAt: number;
  day: string;
  /** Plants that grew into it. */
  plants: number;
}

export interface Garden {
  plants: Plant[];
  trees: Tree[];
  /** The tree being grown now: sessions done in it so far, out of how many it takes. */
  cycle: { done: number; total: number };
}

const clamp01 = (n: number): number => Math.min(1, Math.max(0, n));

/** How much of the session's planned time was done. A session with no planned length counts as complete. */
export function sizeOf(session: Pick<SessionRecord, "duration_ms" | "planned_ms" | "started_at" | "ended_at">): number {
  const done = session.duration_ms ?? Math.max(0, session.ended_at - session.started_at);
  const planned = session.planned_ms ?? 0;
  return planned > 0 ? clamp01(done / planned) : 1;
}

export function stageOf(size: number): Stage {
  if (size >= 1) return 3;
  if (size >= 0.6) return 2;
  if (size >= 0.25) return 1;
  return 0;
}

/** A small stable number from a text, so a plant keeps its shape every time it is drawn. */
function hash(text: string): number {
  let h = 0;
  for (let i = 0; i < text.length; i++) h = (h * 31 + text.charCodeAt(i)) >>> 0;
  return h;
}

export function plantOf(session: SessionRecord): Plant {
  const size = sizeOf(session);
  return {
    id: session.id,
    stage: stageOf(size),
    size,
    variant: (hash(session.id) % 3) as 0 | 1 | 2,
    category: session.category ?? null,
    startedAt: session.started_at,
    day: localDayKey(session.started_at),
  };
}

/**
 * The garden for `sessions` (any order). Focus sessions are counted oldest first; the one that makes `cycleLength` in
 * a row completes a tree and the count starts again, and so does a long break (the same rule the timer's round count
 * follows). Short breaks change nothing.
 */
export function buildGarden(sessions: readonly SessionRecord[], cycleLength: number): Garden {
  const total = Math.max(1, Math.floor(cycleLength));
  const plants: Plant[] = [];
  const trees: Tree[] = [];
  let done = 0;
  for (const s of [...sessions].sort((a, b) => a.started_at - b.started_at || a.id.localeCompare(b.id))) {
    if (s.kind === "long_break") {
      done = 0;
      continue;
    }
    if (s.kind !== "focus") continue;
    plants.push(plantOf(s));
    done += 1;
    if (done === total) {
      trees.push({ id: s.id, completedAt: s.ended_at, day: localDayKey(s.ended_at), plants: total });
      done = 0;
    }
  }
  return { plants, trees, cycle: { done, total } };
}

export type Encouragement = { key: "garden.cycle.start" | "garden.cycle.going" | "garden.cycle.last"; params: { done: number; total: number; left: number } };

/** What to say about progress towards the next tree: nothing yet, partway, or one session to go. */
export function encouragement(done: number, total: number): Encouragement {
  const left = Math.max(0, total - done);
  const key = done <= 0 ? "garden.cycle.start" : left <= 1 ? "garden.cycle.last" : "garden.cycle.going";
  return { key, params: { done, total, left } };
}
