import { describe, expect, it } from "vitest";
import { ALERT_KINDS, durationOf, scoreFor } from "./sounds";

describe("alert sounds", () => {
  it("has a pattern for every kind", () => {
    expect(ALERT_KINDS.sort()).toEqual(["break_end", "break_reminder", "focus_end", "focus_reminder", "warn"]);
    for (const kind of ALERT_KINDS) expect(scoreFor(kind).length, kind).toBeGreaterThan(0);
  });

  it("makes every kind sound different from the others", () => {
    const shapes = ALERT_KINDS.map((k) => JSON.stringify(scoreFor(k)));
    expect(new Set(shapes).size).toBe(ALERT_KINDS.length);
  });

  it("keeps every note audible and in a sensible range", () => {
    for (const kind of ALERT_KINDS) {
      for (const n of scoreFor(kind)) {
        expect(n.freq, kind).toBeGreaterThanOrEqual(200);
        expect(n.freq, kind).toBeLessThanOrEqual(2000);
        expect(n.length, kind).toBeGreaterThan(0.05);
        expect(n.at, kind).toBeGreaterThanOrEqual(0);
      }
    }
  });

  it("is short enough not to be annoying", () => {
    for (const kind of ALERT_KINDS) expect(durationOf(kind), kind).toBeLessThan(1.5);
  });

  it("the end of focus rises and the end of a break falls", () => {
    const rising = scoreFor("focus_end").map((n) => n.freq);
    expect([...rising].sort((a, b) => a - b)).toEqual(rising);
    const falling = scoreFor("break_end").map((n) => n.freq);
    expect([...falling].sort((a, b) => b - a)).toEqual(falling);
  });
});
