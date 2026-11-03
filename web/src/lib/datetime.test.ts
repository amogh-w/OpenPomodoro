import { describe, expect, it } from "vitest";
import { fromLocalInput, toLocalInput } from "./datetime";

describe("datetime-local helpers", () => {
  it("formats local time to the minute with zero padding", () => {
    expect(toLocalInput(new Date(2026, 0, 5, 3, 7, 59).getTime())).toBe("2026-01-05T03:07");
    expect(toLocalInput(new Date(2026, 10, 30, 23, 59).getTime())).toBe("2026-11-30T23:59");
  });

  it("round-trips to the minute", () => {
    const ms = new Date(2026, 5, 15, 14, 30).getTime();
    expect(fromLocalInput(toLocalInput(ms))).toBe(ms);
  });

  it("rejects empty or malformed input", () => {
    for (const bad of ["", "2026-11-03", "2026-11-03 14:05", "nonsense", "2026-13-40T99:99"]) {
      expect(fromLocalInput(bad), bad).toBeNull();
    }
  });
});
