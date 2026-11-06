import { describe, expect, it } from "vitest";
import { barLayout } from "./shareImage";

const area = { x: 10, y: 100, width: 300, height: 200 };

describe("barLayout", () => {
  it("gives nothing for no days", () => {
    expect(barLayout([], area)).toEqual([]);
  });

  it("fills the height with the tallest day and keeps bars on the baseline", () => {
    const bars = barLayout([0, 30, 60], area, 0);
    expect(bars.map((b) => Math.round(b.height))).toEqual([3, 100, 200]);
    for (const b of bars) expect(b.y + b.height).toBeCloseTo(300);
    expect(bars[0]?.width).toBeCloseTo(100);
  });

  it("spaces bars across the width with the gap between them", () => {
    const bars = barLayout([1, 1, 1, 1], area, 10);
    const last = bars[3];
    expect((last?.x ?? 0) + (last?.width ?? 0)).toBeCloseTo(310);
    expect((bars[1]?.x ?? 0) - ((bars[0]?.x ?? 0) + (bars[0]?.width ?? 0))).toBeCloseTo(10);
  });
});
