// The hall as the screens see it: the runes and medallions worked out from the sessions loaded in the browser, with
// progress that always agrees with them. Until sessions have loaded (or offline) it falls back to the timer's own
// round count.
import { app } from "./appState.svelte";
import { recent } from "./recentStore.svelte";
import { type Hall, type Slot, buildHall, lastRunes } from "./runes";

export const cycleTotal = (): number => app.settings?.rounds_before_long ?? 4;

export const hall = (): Hall => buildHall(recent.sessions, cycleTotal());

/**
 * Sessions finished in the medallion being made. This is the timer's own round count (the same number that says when a
 * long break is due and that "Round 2 of 4" shows), so every screen agrees with it. Only when the timer is not known
 * yet does it fall back to counting the stored sessions.
 */
export function cycleDone(current: Hall = hall()): number {
  return app.timer ? app.timer.rounds % cycleTotal() : current.cycle.done;
}

/** The glow colour of a category (the accent when it has none). */
export const glowOf = (category: string | null | undefined): string =>
  category ? `var(--ctp-${app.categories.find((c) => c.id === category)?.color ?? "peach"})` : "var(--accent)";

/** The medallion being made, as its sockets are filled: the latest runes, padded with plain full ones if they are not loaded. */
export function currentSlots(current: Hall = hall(), done: number = cycleDone(current)): Slot[] {
  const known = lastRunes(current.runes, done).map((r) => ({ glyph: r.glyph, lit: r.lit, glow: glowOf(r.category) }));
  const missing = Math.max(0, done - known.length);
  return [...Array.from({ length: missing }, (_, i) => ({ glyph: i * 3 + 1, lit: 1 })), ...known];
}

/** Runes as a sealed medallion draws them. */
export const slotsOf = (runes: readonly { glyph: number; lit: number; category: string | null }[]): Slot[] =>
  runes.map((r) => ({ glyph: r.glyph, lit: r.lit, glow: glowOf(r.category) }));
