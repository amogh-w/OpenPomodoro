import { describe, expect, it } from "vitest";
import { addDays, computeStats, dayKey, weekStart } from "./stats";
import type { DayTotal } from "./types";

const MIN = 60_000;
const day = (key: string, focusMin: number, count = 1): DayTotal => ({ day: key, focus_ms: focusMin * MIN, break_ms: 0, focus_count: count });

describe("day arithmetic", () => {
  it("adds days across month, year and leap boundaries", () => {
    expect(addDays("2026-11-30", 1)).toBe("2026-12-01");
    expect(addDays("2026-12-31", 1)).toBe("2027-01-01");
    expect(addDays("2028-02-28", 1)).toBe("2028-02-29");
    expect(addDays("2026-03-01", -1)).toBe("2026-02-28");
  });

  it("finds the Monday of a week", () => {
    expect(weekStart("2026-11-03")).toBe("2026-11-02"); // a Tuesday
    expect(weekStart("2026-11-02")).toBe("2026-11-02"); // the Monday itself
    expect(weekStart("2026-11-08")).toBe("2026-11-02"); // the Sunday
    expect(weekStart("2026-11-09")).toBe("2026-11-09");
  });

  it("names the local day of an instant", () => {
    const ms = Date.UTC(2026, 10, 3, 23, 30);
    expect(dayKey(ms, 0)).toBe("2026-11-03");
    expect(dayKey(ms, 120)).toBe("2026-11-04");
    expect(dayKey(ms, -300)).toBe("2026-11-03");
    expect(dayKey(Date.UTC(2026, 10, 3, 2, 0), -300)).toBe("2026-11-02");
  });
});

describe("computeStats", () => {
  const today = "2026-11-04"; // a Wednesday

  it("is all zeros with nothing recorded", () => {
    expect(computeStats([], today)).toEqual({ todayMs: 0, weekMs: 0, allTimeMs: 0, sessions: 0, currentStreak: 0, longestStreak: 0 });
  });

  it("adds up today, this week (from Monday) and all time", () => {
    const days = [day("2026-10-30", 60), day("2026-11-01", 20), day("2026-11-02", 25), day("2026-11-03", 50), day("2026-11-04", 30, 2)];
    const s = computeStats(days, today);
    expect(s.todayMs).toBe(30 * MIN);
    expect(s.weekMs).toBe((25 + 50 + 30) * MIN);
    expect(s.allTimeMs).toBe((60 + 20 + 25 + 50 + 30) * MIN);
    expect(s.sessions).toBe(6);
  });

  it("ignores days after today when adding up the week", () => {
    expect(computeStats([day("2026-11-04", 10), day("2026-11-05", 99)], today).weekMs).toBe(10 * MIN);
  });

  it("counts a streak ending today", () => {
    const s = computeStats([day("2026-11-02", 1), day("2026-11-03", 1), day("2026-11-04", 1)], today);
    expect(s.currentStreak).toBe(3);
    expect(s.longestStreak).toBe(3);
  });

  it("keeps a streak alive through an empty today", () => {
    expect(computeStats([day("2026-11-02", 1), day("2026-11-03", 1)], today).currentStreak).toBe(2);
  });

  it("breaks the streak after a missed day", () => {
    const s = computeStats([day("2026-11-01", 1), day("2026-11-02", 1), day("2026-11-04", 1)], today);
    expect(s.currentStreak).toBe(1);
    expect(s.longestStreak).toBe(2);
  });

  it("has no current streak if yesterday and today are both empty", () => {
    const s = computeStats([day("2026-11-01", 1), day("2026-11-02", 1)], today);
    expect(s.currentStreak).toBe(0);
    expect(s.longestStreak).toBe(2);
  });

  it("does not count days with only breaks", () => {
    const breaksOnly: DayTotal = { day: "2026-11-03", focus_ms: 0, break_ms: 5 * MIN, focus_count: 0 };
    expect(computeStats([breaksOnly, day("2026-11-04", 1)], today).currentStreak).toBe(1);
  });

  it("finds the longest streak anywhere in the history, across a year end", () => {
    const run = ["2026-12-30", "2026-12-31", "2027-01-01", "2027-01-02"].map((d) => day(d, 1));
    expect(computeStats([...run, day("2026-12-01", 1)], "2027-03-01").longestStreak).toBe(4);
  });
});
