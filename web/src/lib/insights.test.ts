import { describe, expect, it } from "vitest";
import { byGroup, byIntention, computeInsights, daysOfMonth } from "./insights";
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

describe("byIntention", () => {
  const f = (intention: string | null, minutes: number, kind: "focus" | "short_break" = "focus") => ({
    id: Math.random().toString(),
    kind,
    started_at: 0,
    ended_at: minutes * 60_000,
    duration_ms: minutes * 60_000,
    intention,
  });

  it("groups by intention ignoring case and spacing, most time first", () => {
    const rows = byIntention([f("Write report", 25), f("write  REPORT", 25), f("Email", 10), f(null, 15), f("Break stuff", 5, "short_break")] as never);
    expect(rows.map((r) => [r.name, r.sessions, r.focusMs / 60_000])).toEqual([["Write report", 2, 50], ["Email", 1, 10]]);
    expect(rows[0]?.share).toBeCloseTo(50 / 75);
  });

  it("limits the list and copes with nothing", () => {
    expect(byIntention([])).toEqual([]);
    const many = Array.from({ length: 15 }, (_, i) => f(`task ${i}`, i + 1));
    expect(byIntention(many as never)).toHaveLength(10);
    expect(byIntention(many as never, 3)).toHaveLength(3);
  });
});

describe("byGroup", () => {
  const cats = [
    { id: "a", name: "Writing", group: "Work" },
    { id: "b", name: "Email", group: "Work" },
    { id: "c", name: "Piano", group: "Home" },
    { id: "d", name: "Misc" },
  ];
  const f = (category: string | null, minutes: number, kind: "focus" | "short_break" = "focus") => ({
    id: String(Math.random()),
    kind,
    started_at: 0,
    ended_at: minutes * 60_000,
    duration_ms: minutes * 60_000,
    category,
  });

  it("adds categories under their group, most time first, and leaves ungrouped ones out", () => {
    const rows = byGroup([f("a", 30), f("b", 20), f("c", 40), f("d", 10), f(null, 10), f("a", 5, "short_break")] as never, cats);
    expect(rows.map((r) => [r.name, r.focusMs / 60_000, r.sessions, r.categories])).toEqual([
      ["Work", 50, 2, ["Email", "Writing"]],
      ["Home", 40, 1, ["Piano"]],
    ]);
    expect(rows[0]?.share).toBeCloseTo(50 / 110);
  });

  it("gives nothing when no category has a group", () => {
    expect(byGroup([f("a", 30)] as never, [{ id: "a", name: "Writing" }])).toEqual([]);
  });
});
