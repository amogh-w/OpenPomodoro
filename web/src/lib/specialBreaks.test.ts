import { describe, expect, it } from "vitest";
import { dueSpecialBreak, MAX_SPECIAL_BREAKS, parseSpecialBreaks, type SpecialBreak } from "./specialBreaks";

const lunch: SpecialBreak = { id: "a", label: "Lunch", time: "12:30", minutes: 30, days: [true, true, true, true, true, false, false] };
const snack: SpecialBreak = { id: "b", label: "Snack", time: "12:40", minutes: 10, days: [true, true, true, true, true, true, true] };
const monday = (h: number, m: number): Date => new Date(2026, 10, 2, h, m); // a Monday

describe("parseSpecialBreaks", () => {
  it("gives nothing for junk", () => {
    for (const raw of [undefined, null, 5, "x", {}, [1, "a", null]]) expect(parseSpecialBreaks(raw)).toEqual([]);
  });

  it("keeps valid entries, repairs fields and drops bad or repeated ones", () => {
    const out = parseSpecialBreaks([
      lunch,
      { ...lunch },
      { id: "c", time: "25:00", minutes: 5 },
      { id: "d", label: "x".repeat(100), time: "08:00", minutes: 500, days: [true] },
    ]);
    expect(out.map((b) => b.id)).toEqual(["a", "d"]);
    expect(out[1]).toMatchObject({ minutes: 120, days: [true, true, true, true, true, true, true] });
    expect(out[1]?.label.length).toBe(40);
  });

  it("keeps at most the allowed number", () => {
    const many = Array.from({ length: 20 }, (_, i) => ({ ...lunch, id: `i${i}` }));
    expect(parseSpecialBreaks(many)).toHaveLength(MAX_SPECIAL_BREAKS);
  });
});

describe("dueSpecialBreak", () => {
  it("is due from its time for the grace period", () => {
    const at = (h: number, m: number) => dueSpecialBreak(monday(h, m), [lunch], {})?.id ?? null;
    expect([at(12, 29), at(12, 30), at(12, 49), at(12, 50)]).toEqual([null, "a", "a", null]);
  });

  it("respects the weekdays and what was handled today", () => {
    expect(dueSpecialBreak(new Date(2026, 10, 7, 12, 31), [lunch], {})).toBeNull(); // Saturday
    expect(dueSpecialBreak(monday(12, 31), [lunch], { a: "2026-11-02" })).toBeNull();
    expect(dueSpecialBreak(monday(12, 31), [lunch], { a: "2026-11-01" })?.id).toBe("a");
  });

  it("offers the earliest when two overlap", () => {
    expect(dueSpecialBreak(monday(12, 45), [snack, lunch], {})?.id).toBe("a");
    expect(dueSpecialBreak(monday(12, 45), [snack, lunch], { a: "2026-11-02" })?.id).toBe("b");
  });
});
