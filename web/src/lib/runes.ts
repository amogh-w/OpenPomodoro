// The hall: every finished focus session is a rune, lit as far as the session went, and every full cycle of sessions
// (the number of rounds before a long break) seals a medallion. All of it is worked out from the sessions that are
// already stored, so it needs no extra data and covers past history too.
import { GLYPH_COUNT } from "./glyphs";
import { localDayKey } from "./days";
import type { SessionRecord } from "./types";

export interface Rune {
  id: string;
  /** How much of the planned time was done, 0 to 1: how far the rune is lit. */
  lit: number;
  /** Which of the invented marks to draw; the same session always gets the same one. */
  glyph: number;
  category: string | null;
  startedAt: number;
  endedAt: number;
  day: string;
}

/** What a medallion needs to draw one carved rune. */
export interface Slot {
  glyph: number;
  lit: number;
  glow?: string;
}

export interface Medallion {
  /** The id of the session that sealed it. */
  id: string;
  completedAt: number;
  day: string;
  /** The runes that were carved into it, in order. */
  runes: Rune[];
}

export interface Hall {
  /** Every rune, oldest first. */
  runes: Rune[];
  /** Sealed medallions, oldest first. */
  medallions: Medallion[];
  /** The medallion being made now: the runes carved into it so far, and how many it takes. */
  cycle: { runes: Rune[]; done: number; total: number };
}

const clamp01 = (n: number): number => Math.min(1, Math.max(0, n));

/** How much of the session's planned time was done. A session with no planned length counts as complete. */
export function sizeOf(session: Pick<SessionRecord, "duration_ms" | "planned_ms" | "started_at" | "ended_at">): number {
  const done = session.duration_ms ?? Math.max(0, session.ended_at - session.started_at);
  const planned = session.planned_ms ?? 0;
  return planned > 0 ? clamp01(done / planned) : 1;
}

export type LitWord = "faint" | "half" | "bright" | "blazing";

/** A few words for how brightly a rune burns, for the reflect screen and the spoken label. */
export function litWord(lit: number): LitWord {
  if (lit >= 1) return "blazing";
  if (lit >= 0.6) return "bright";
  if (lit >= 0.25) return "half";
  return "faint";
}

/** A small stable number from a text, so a rune keeps its mark every time it is drawn. */
function hash(text: string): number {
  let h = 0;
  for (let i = 0; i < text.length; i++) h = (h * 31 + text.charCodeAt(i)) >>> 0;
  return h;
}

/** The mark for a session, from when it started: the running timer can show the very mark it will have once saved. */
export function glyphFor(startedAt: number): number {
  return hash(String(startedAt)) % GLYPH_COUNT;
}

export function runeOf(session: SessionRecord): Rune {
  return {
    id: session.id,
    lit: sizeOf(session),
    glyph: glyphFor(session.started_at),
    category: session.category ?? null,
    startedAt: session.started_at,
    endedAt: session.ended_at,
    day: localDayKey(session.started_at),
  };
}

/**
 * The hall for `sessions` (any order). Focus sessions are counted oldest first; the one that makes `cycleLength` in a
 * row seals a medallion and the count starts again, and so does a long break (the same rule the timer's round count
 * follows). Short breaks change nothing. `cycleStartedAt` is when the person last broke the medallion: sessions before it
 * stay in the hall as runes but start no count towards the next medallion.
 */
export function buildHall(sessions: readonly SessionRecord[], cycleLength: number, cycleStartedAt: number | null = null): Hall {
  const total = Math.max(1, Math.floor(cycleLength));
  const runes: Rune[] = [];
  const medallions: Medallion[] = [];
  let current: Rune[] = [];
  let broken = cycleStartedAt === null;
  for (const s of [...sessions].sort((a, b) => a.started_at - b.started_at || a.id.localeCompare(b.id))) {
    // The first session after the medallion was broken starts a fresh count.
    if (!broken && cycleStartedAt !== null && s.started_at >= cycleStartedAt) {
      current = [];
      broken = true;
    }
    if (s.kind === "long_break") {
      current = [];
      continue;
    }
    if (s.kind !== "focus") continue;
    const rune = runeOf(s);
    runes.push(rune);
    current.push(rune);
    if (current.length === total) {
      medallions.push({ id: s.id, completedAt: s.ended_at, day: localDayKey(s.ended_at), runes: current });
      current = [];
    }
  }
  // Broken after every stored session: the next medallion starts empty.
  if (!broken) current = [];
  return { runes, medallions, cycle: { runes: current, done: current.length, total } };
}

export type Encouragement = { key: "runes.cycle.start" | "runes.cycle.going" | "runes.cycle.last"; params: { done: number; total: number; left: number } };

/** What to say about progress towards the next medallion: nothing yet, partway, or one session to go. */
export function encouragement(done: number, total: number): Encouragement {
  const left = Math.max(0, total - done);
  const key = done <= 0 ? "runes.cycle.start" : left <= 1 ? "runes.cycle.last" : "runes.cycle.going";
  return { key, params: { done, total, left } };
}

/**
 * The runes of the medallion being made, when the timer's own round count says `done` sessions are in it: the most
 * recent `done` runes, oldest first. If fewer are known (history not loaded yet, or older than what is kept), the
 * ones there are returned and the caller shows the rest as plain full runes.
 */
export function lastRunes(runes: readonly Rune[], done: number): Rune[] {
  return done <= 0 ? [] : runes.slice(-done);
}
