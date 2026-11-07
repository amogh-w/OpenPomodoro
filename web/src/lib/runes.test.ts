import { describe, expect, it } from "vitest";
import { GLYPH_COUNT } from "./glyphs";
import { buildHall, encouragement, glyphFor, lastRunes, litWord, runeOf, sizeOf } from "./runes";
import type { SessionRecord } from "./types";

let n = 0;
const MIN = 60_000;
const at = (hour: number, minute = 0): number => new Date(2026, 10, 3, hour, minute).getTime();

function focus(startHour: number, minutes = 25, planned = 25, over: Partial<SessionRecord> = {}): SessionRecord {
  const started = at(startHour);
  return { id: `s${++n}`, kind: "focus", started_at: started, ended_at: started + minutes * MIN, duration_ms: minutes * MIN, planned_ms: planned * MIN, ...over };
}
const rest = (startHour: number, kind: "short_break" | "long_break" = "short_break"): SessionRecord => ({
  id: `b${++n}`,
  kind,
  started_at: at(startHour, 40),
  ended_at: at(startHour, 45),
  duration_ms: 5 * MIN,
});

describe("sizeOf and litWord", () => {
  it("is the share of the planned time done, capped at 1", () => {
    expect(sizeOf(focus(9, 10, 25))).toBeCloseTo(0.4);
    expect(sizeOf(focus(9, 30, 25))).toBe(1);
    expect(sizeOf(focus(9, 0, 25))).toBe(0);
  });

  it("counts a session with no planned time as complete, and falls back to the span without a duration", () => {
    expect(sizeOf({ started_at: 0, ended_at: MIN, duration_ms: MIN })).toBe(1);
    expect(sizeOf({ started_at: 0, ended_at: 10 * MIN, planned_ms: 20 * MIN })).toBeCloseTo(0.5);
  });

  it("has words for four brightness levels, blazing only for a complete session", () => {
    expect([0, 0.24, 0.25, 0.59, 0.6, 0.99, 1].map(litWord)).toEqual(["faint", "faint", "half", "half", "bright", "bright", "blazing"]);
  });
});

describe("runeOf", () => {
  it("keeps the same mark for the same session and uses every mark across sessions", () => {
    const s = focus(9);
    expect(runeOf(s).glyph).toBe(runeOf({ ...s }).glyph);
    expect(runeOf(s).glyph).toBe(glyphFor(s.started_at));
    const marks = new Set(Array.from({ length: 200 }, (_, i) => runeOf({ ...s, started_at: s.started_at + i * 61_000 }).glyph));
    expect(marks.size).toBe(GLYPH_COUNT);
    for (const m of marks) expect(m).toBeLessThan(GLYPH_COUNT);
  });

  it("carries the category, the times and the day", () => {
    const r = runeOf(focus(9, 25, 25, { category: "c1" }));
    expect(r.category).toBe("c1");
    expect(r.day).toBe("2026-11-03");
    expect(r.endedAt - r.startedAt).toBe(25 * MIN);
  });
});

describe("buildHall", () => {
  it("is empty for nothing", () => {
    expect(buildHall([], 4)).toEqual({ runes: [], medallions: [], cycle: { runes: [], done: 0, total: 4 } });
  });

  it("makes a rune per focus session and a medallion for every full cycle", () => {
    const sessions = [focus(9), rest(9), focus(10), rest(10), focus(11), rest(11), focus(12), rest(12, "long_break"), focus(14), rest(14), focus(15)];
    const h = buildHall(sessions, 4);
    expect(h.runes).toHaveLength(6);
    expect(h.medallions).toHaveLength(1);
    expect(h.medallions[0]?.runes).toHaveLength(4);
    expect(h.cycle.done).toBe(2);
    expect(h.cycle.runes.map((r) => r.id)).toEqual(h.runes.slice(4).map((r) => r.id));
  });

  it("does not care about the order it is given", () => {
    const sessions = [focus(9), focus(10), focus(11), focus(12), focus(13)];
    expect(buildHall([...sessions].reverse(), 4)).toEqual(buildHall(sessions, 4));
  });

  it("counts an ended-early session as a round but lights its rune only partly", () => {
    const h = buildHall([focus(9), focus(10, 5, 25), focus(11), focus(12)], 4);
    expect(h.medallions).toHaveLength(1);
    expect(h.runes.map((r) => Math.round(r.lit * 100))).toEqual([100, 20, 100, 100]);
  });

  it("starts the count again after a long break, even partway through", () => {
    const h = buildHall([focus(9), focus(10), rest(11, "long_break"), focus(12)], 4);
    expect(h.medallions).toHaveLength(0);
    expect(h.cycle.done).toBe(1);
  });

  it("follows the cycle length it is given, and treats nonsense as one", () => {
    const sessions = [focus(9), focus(10), focus(11), focus(12)];
    expect(buildHall(sessions, 2).medallions).toHaveLength(2);
    expect(buildHall(sessions, 1).medallions).toHaveLength(4);
    expect(buildHall(sessions, 0).cycle.total).toBe(1);
  });

  it("ignores breaks for runes", () => {
    expect(buildHall([rest(9), rest(10, "long_break")], 4).runes).toEqual([]);
  });
});

describe("breaking the medallion", () => {
  it("keeps the runes but starts the count again from the moment it was broken", () => {
    const before = [focus(9), focus(10), focus(11)];
    const after = [focus(13), focus(14)];
    const h = buildHall([...before, ...after], 4, at(12));
    expect(h.runes).toHaveLength(5);
    expect(h.medallions).toHaveLength(0);
    expect(h.cycle.done).toBe(2);
    expect(h.cycle.runes.map((r) => r.id)).toEqual(after.map((s) => s.id));
  });

  it("changes nothing when it was never broken, or broken before everything, or after everything", () => {
    const sessions = [focus(9), focus(10), focus(11), focus(12), focus(13)];
    expect(buildHall(sessions, 4, null)).toEqual(buildHall(sessions, 4));
    expect(buildHall(sessions, 4, at(1)).medallions).toHaveLength(1);
    expect(buildHall(sessions, 4, at(23)).cycle.done).toBe(0);
  });

  it("a medallion sealed after the break counts only the new sessions", () => {
    const sessions = [focus(9), focus(10), focus(13), focus(14), focus(15), focus(16)];
    const h = buildHall(sessions, 4, at(12));
    expect(h.medallions).toHaveLength(1);
    expect(h.medallions[0]?.runes.map((r) => r.id)).toEqual(sessions.slice(2).map((s) => s.id));
  });
});

describe("encouragement", () => {
  it("says how far along the next medallion is", () => {
    expect(encouragement(0, 4).key).toBe("runes.cycle.start");
    expect(encouragement(1, 4).key).toBe("runes.cycle.going");
    expect(encouragement(2, 4).params).toEqual({ done: 2, total: 4, left: 2 });
    expect(encouragement(3, 4).key).toBe("runes.cycle.last");
    expect(encouragement(0, 1).key).toBe("runes.cycle.start");
  });
});

describe("lastRunes", () => {
  const runes = [focus(9), focus(10), focus(11), focus(12), focus(13)].map(runeOf);

  it("takes the most recent ones, oldest first", () => {
    expect(lastRunes(runes, 2).map((r) => r.id)).toEqual(runes.slice(3).map((r) => r.id));
    expect(lastRunes(runes, 5)).toHaveLength(5);
  });

  it("gives what there is when fewer are known, and nothing for zero", () => {
    expect(lastRunes(runes, 9)).toHaveLength(5);
    expect(lastRunes(runes, 0)).toEqual([]);
    expect(lastRunes([], 3)).toEqual([]);
  });
});
