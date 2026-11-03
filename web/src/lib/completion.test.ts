import { describe, expect, it } from "vitest";
import { completion, completionRate } from "./completion";
import type { SessionRecord } from "./types";

const focus = (duration: number, planned?: number): SessionRecord => ({
  id: "x",
  kind: "focus",
  started_at: 0,
  ended_at: duration,
  duration_ms: duration,
  ...(planned === undefined ? {} : { planned_ms: planned }),
});

describe("completion", () => {
  it("is complete when the planned time was reached, including running over", () => {
    expect(completion(focus(1500, 1500))).toBe(true);
    expect(completion(focus(1800, 1500))).toBe(true);
  });

  it("is incomplete when ended early", () => {
    expect(completion(focus(600, 1500))).toBe(false);
  });

  it("says nothing about breaks or sessions with no plan", () => {
    expect(completion({ ...focus(300, 300), kind: "short_break" })).toBeNull();
    expect(completion(focus(600))).toBeNull();
  });
});

describe("completionRate", () => {
  it("counts only sessions that can be compared", () => {
    expect(completionRate([focus(1500, 1500), focus(600, 1500), focus(100), focus(1500, 1500)])).toEqual({
      completed: 2,
      rated: 3,
      rate: 2 / 3,
    });
    expect(completionRate([])).toEqual({ completed: 0, rated: 0, rate: null });
  });
});
