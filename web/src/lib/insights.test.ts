import { describe, expect, it } from "vitest";
import { computeInsights, daysOfMonth } from "./insights";
import type { SessionRecord } from "./types";

const at = (day: number, h: number, m = 0): number => new Date(2026, 9, day, h, m).getTime();
let n = 0;
function s(over: Partial<SessionRecord> & { from: number; minutes: number }): SessionRecord {
  n += 1;
  const { from, minutes, ...rest } = over;
  return { id: `s${n}`, kind: "focus", started_at: from, ended_at: from + minutes * 60_000, duration_ms: minutes * 60_000, ...rest };
}
const october = daysOfMonth(2026, 9);

describe("daysOfMonth", () => {
  it("lists every day in order", () => {
    expect(october).toHaveLength(31);
    expect(october[0]).toBe("2026-10-01");
    expect(october[30]).toBe("2026-10-31");
    expect(daysOfMonth(2028, 1)).toHaveLength(29);
  });
});

describe("computeInsights", () => {
  const sessions = [
    s({ from: at(1, 9), minutes: 60, category: "a", mood: "focused" }),
    s({ from: at(1, 10, 30), minutes: 30, category: "b", mood: "distracted" }),
    s({ from: at(3, 9), minutes: 30, category: "a" }),
    s({ kind: "short_break", from: at(1, 10), minutes: 10 }),
    s({ kind: "long_break", from: at(3, 9, 30), minutes: 10 }),
  ];
  const r = computeInsights(sessions, [], october);

  it("adds up focus and break time", () => {
    expect(r.focusMs).toBe(120 * 60_000);
    expect(r.breakMs).toBe(20 * 60_000);
    expect(r.sessions).toBe(3);
    expect(r.ratio).toBe(6);
  });

  it("averages over days that had focus", () => {
    expect(r.activeDays).toBe(2);
    expect(r.avgPerDayMs).toBe(60 * 60_000);
  });

  it("finds the best hour and the most distracted hour", () => {
    expect(r.bestHour).toBe(9);
    expect(r.distractedHour).toBe(10);
    expect(r.byHour[9]).toBe(90 * 60_000);
  });

  it("tallies moods and gives a bar for every day", () => {
    expect(r.mood).toEqual({ focused: 1, neutral: 0, distracted: 1 });
    expect(r.days).toHaveLength(31);
    expect(r.days[0]).toEqual({ day: "2026-10-01", focusMs: 90 * 60_000 });
    expect(r.days[1]?.focusMs).toBe(0);
  });

  it("ranks categories with their share", () => {
    expect(r.categories.map((c) => c.id)).toEqual(["a", "b"]);
    expect(r.categories[0]?.share).toBeCloseTo(0.75);
  });

  it("spreads a session across the hours it covers", () => {
    const x = computeInsights([s({ from: at(2, 8, 45), minutes: 30 })], [], october);
    expect(x.byHour[8]).toBe(15 * 60_000);
    expect(x.byHour[9]).toBe(15 * 60_000);
  });

  it("compares with the earlier period, in total and per category", () => {
    const earlier = [s({ from: at(1, 9), minutes: 60, category: "a" })];
    const x = computeInsights(sessions, earlier, october);
    expect(x.change).toBeCloseTo(1);
    expect(x.categories[0]?.change).toBeCloseTo(0.5);
    expect(x.categories[1]?.change).toBeNull();
  });

  it("has nothing to say about an empty month", () => {
    const x = computeInsights([], [], october);
    expect([x.focusMs, x.bestHour, x.distractedHour, x.ratio, x.change, x.avgPerDayMs]).toEqual([0, null, null, null, null, 0]);
  });
});
