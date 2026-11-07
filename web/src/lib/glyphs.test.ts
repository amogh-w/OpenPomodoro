import { describe, expect, it } from "vitest";
import { GLYPHS, GLYPH_COUNT } from "./glyphs";

/** The bounding box of a path made of absolute M, L, H, V and Z commands. */
function box(path: string): { x: [number, number]; y: [number, number] } {
  const xs: number[] = [];
  const ys: number[] = [];
  let x = 0;
  let y = 0;
  for (const [, command, args] of path.matchAll(/([MLHVZ])([^MLHVZ]*)/g)) {
    const n = (args ?? "").trim().split(/\s+/).filter(Boolean).map(Number);
    if (command === "M" || command === "L") [x, y] = [n[0] ?? x, n[1] ?? y];
    else if (command === "H") x = n[0] ?? x;
    else if (command === "V") y = n[0] ?? y;
    else continue;
    xs.push(x);
    ys.push(y);
  }
  return { x: [Math.min(...xs), Math.max(...xs)], y: [Math.min(...ys), Math.max(...ys)] };
}

describe("glyphs", () => {
  it("has eight, all different", () => {
    expect(GLYPH_COUNT).toBe(8);
    expect(new Set(GLYPHS).size).toBe(8);
  });

  it("centres every glyph on the tablet (16, 20) and keeps it clear of the edges", () => {
    for (const path of GLYPHS) {
      const b = box(path);
      expect((b.x[0] + b.x[1]) / 2).toBeCloseTo(16, 5);
      expect((b.y[0] + b.y[1]) / 2).toBeCloseTo(20, 5);
      expect(b.x[0]).toBeGreaterThanOrEqual(5);
      expect(b.x[1]).toBeLessThanOrEqual(27);
      expect(b.y[0]).toBeGreaterThanOrEqual(5);
      expect(b.y[1]).toBeLessThanOrEqual(35);
    }
  });
});
