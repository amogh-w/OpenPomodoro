import { describe, expect, it } from "vitest";
import { buildHeatmap, levelCuts } from "./heatmap";

const MIN = 60_000;
const day = (d: string, minutes: number) => ({ day: d, focus_ms: minutes * MIN, break_ms: 0, focus_count: 1 });

describe("levelCuts", () => {
  it("is flat for no focus at all", () => {
    expect(levelCuts([])).toEqual([0, 0, 0]);
    expect(levelCuts([0, 0])).toEqual([0, 0, 0]);
  });

  it("splits the days that have focus into quarters", () => {
    expect(levelCuts([10, 20, 30, 40, 50, 60, 70, 80].map((m) => m * MIN))).toEqual([20 * MIN, 40 * MIN, 60 * MIN]);
  });
});

describe("buildHeatmap", () => {
  // 2026-11-06 is a Friday.
  const today = "2026-11-06";

  it("draws whole weeks, Monday first, ending with this week", () => {
    const weeks = buildHeatmap([], today);
    expect(weeks).toHaveLength(53);
    expect(weeks.every((w) => w.length === 7)).toBe(true);
    const last = weeks[52] ?? [];
    expect(last.map((c) => c.day)).toEqual([
      "2026-11-02", "2026-11-03", "2026-11-04", "2026-11-05", "2026-11-06", "2026-11-07", "2026-11-08",
    ]);
    expect(last.map((c) => c.future)).toEqual([false, false, false, false, false, true, true]);
    expect(weeks[0]?.[0]?.day).toBe("2025-11-03");
  });

  it("shades days by their share and leaves empty and future days at 0", () => {
    const totals = [day("2026-11-02", 10), day("2026-11-03", 30), day("2026-11-04", 60), day("2026-11-05", 120), day("2026-11-08", 500)];
    const last = buildHeatmap(totals, today)[52] ?? [];
    expect(last.map((c) => c.level)).toEqual([1, 2, 3, 4, 0, 0, 0]);
  });

  it("ignores summaries outside the window", () => {
    const weeks = buildHeatmap([day("2020-01-01", 60)], today);
    expect(weeks.flat().every((c) => c.level === 0)).toBe(true);
  });
});
