// The hall as the screens see it: the runes and medallions worked out from the sessions loaded in the browser, with
// progress that always agrees with them. Until sessions have loaded (or offline) it falls back to the timer's own
// round count.
import { app } from "./appState.svelte";
import { recent } from "./recentStore.svelte";
import { type Hall, type Rune, type Slot, buildHall } from "./runes";

export const cycleTotal = (): number => app.settings?.rounds_before_long ?? 4;

export const hall = (): Hall => buildHall(recent.sessions, cycleTotal());

/** Sessions finished in the medallion being made. */
export function cycleDone(current: Hall = hall()): number {
  return recent.sessions.length > 0 ? current.cycle.done : (app.timer?.rounds ?? 0) % cycleTotal();
}

/** The glow colour of a category (the accent when it has none). */
export const glowOf = (category: string | null | undefined): string =>
  category ? `var(--ctp-${app.categories.find((c) => c.id === category)?.color ?? "peach"})` : "var(--accent)";

/** Runes as the medallion draws them. With no stored sessions to draw from, `fallbackDone` full runes stand in. */
export function slotsOf(runes: readonly Rune[], fallbackDone = 0): Slot[] {
  if (runes.length > 0 || fallbackDone <= 0) return runes.map((r) => ({ glyph: r.glyph, lit: r.lit, glow: glowOf(r.category) }));
  return Array.from({ length: fallbackDone }, (_, i) => ({ glyph: i * 3 + 1, lit: 1 }));
}
