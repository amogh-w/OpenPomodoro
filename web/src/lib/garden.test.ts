import { describe, expect, it } from "vitest";
import { buildGarden, encouragement, plantOf, sizeOf, stageOf } from "./garden";
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

describe("sizeOf and stageOf", () => {
  it("is the share of the planned time done, capped at 1", () => {
    expect(sizeOf(focus(9, 10, 25))).toBeCloseTo(0.4);
    expect(sizeOf(focus(9, 30, 25))).toBe(1);
    expect(sizeOf(focus(9, 0, 25))).toBe(0);
  });

  it("counts a session with no planned time as complete, and falls back to the span without a duration", () => {
    expect(sizeOf({ started_at: 0, ended_at: MIN, duration_ms: MIN })).toBe(1);
    expect(sizeOf({ started_at: 0, ended_at: 10 * MIN, planned_ms: 20 * MIN })).toBeCloseTo(0.5);
  });

  it("has four stages with a full one only for a complete session", () => {
    expect([0, 0.24, 0.25, 0.59, 0.6, 0.99, 1].map(stageOf)).toEqual([0, 0, 1, 1, 2, 2, 3]);
  });
});

describe("plantOf", () => {
  it("keeps the same shape for the same session and spreads shapes across sessions", () => {
    const s = focus(9);
    expect(plantOf(s).variant).toBe(plantOf({ ...s }).variant);
    const shapes = new Set(Array.from({ length: 30 }, (_, i) => plantOf({ ...s, id: `id-${i}` }).variant));
    expect(shapes.size).toBe(3);
  });

  it("carries the category and the day", () => {
    const p = plantOf(focus(9, 25, 25, { category: "c1" }));
    expect(p.category).toBe("c1");
    expect(p.day).toBe("2026-11-03");
  });
});

describe("buildGarden", () => {
  it("is empty for nothing", () => {
    expect(buildGarden([], 4)).toEqual({ plants: [], trees: [], cycle: { done: 0, total: 4 } });
  });

  it("makes a plant per focus session and a tree for every full cycle", () => {
    const sessions = [focus(9), rest(9), focus(10), rest(10), focus(11), rest(11), focus(12), rest(12, "long_break"), focus(14), rest(14), focus(15)];
    const g = buildGarden(sessions, 4);
    expect(g.plants).toHaveLength(6);
    expect(g.trees).toHaveLength(1);
    expect(g.trees[0]?.plants).toBe(4);
    expect(g.cycle).toEqual({ done: 2, total: 4 });
  });

  it("does not care about the order it is given", () => {
    const sessions = [focus(9), focus(10), focus(11), focus(12), focus(13)];
    expect(buildGarden([...sessions].reverse(), 4)).toEqual(buildGarden(sessions, 4));
  });

  it("counts an ended-early session as a round but plants it small", () => {
    const g = buildGarden([focus(9), focus(10, 5, 25), focus(11), focus(12)], 4);
    expect(g.trees).toHaveLength(1);
    expect(g.plants.map((p) => p.stage)).toEqual([3, 0, 3, 3]);
  });

  it("starts the count again after a long break, even partway through", () => {
    const g = buildGarden([focus(9), focus(10), rest(11, "long_break"), focus(12)], 4);
    expect(g.trees).toHaveLength(0);
    expect(g.cycle.done).toBe(1);
  });

  it("follows the cycle length it is given, and treats nonsense as one", () => {
    const sessions = [focus(9), focus(10), focus(11), focus(12)];
    expect(buildGarden(sessions, 2).trees).toHaveLength(2);
    expect(buildGarden(sessions, 1).trees).toHaveLength(4);
    expect(buildGarden(sessions, 0).cycle.total).toBe(1);
  });

  it("ignores breaks for plants", () => {
    expect(buildGarden([rest(9), rest(10, "long_break")], 4).plants).toEqual([]);
  });
});

describe("encouragement", () => {
  it("says how far along the next tree is", () => {
    expect(encouragement(0, 4).key).toBe("garden.cycle.start");
    expect(encouragement(1, 4).key).toBe("garden.cycle.going");
    expect(encouragement(2, 4).params).toEqual({ done: 2, total: 4, left: 2 });
    expect(encouragement(3, 4).key).toBe("garden.cycle.last");
    expect(encouragement(0, 1).key).toBe("garden.cycle.start");
  });
});
