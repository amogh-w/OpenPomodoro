import { describe, expect, it } from "vitest";
import { stepMinutes } from "./stepper";

describe("stepMinutes", () => {
  it("goes by 5 in the usual range", () => {
    expect(stepMinutes(25, 1, 1, 180)).toBe(30);
    expect(stepMinutes(25, -1, 1, 180)).toBe(20);
  });

  it("goes by 1 at the low end", () => {
    expect(stepMinutes(1, 1, 1, 180)).toBe(2);
    expect(stepMinutes(4, 1, 1, 180)).toBe(5);
    expect(stepMinutes(5, -1, 1, 180)).toBe(4);
    expect(stepMinutes(2, -1, 1, 180)).toBe(1);
  });

  it("lands on multiples of 5 from an odd value", () => {
    expect(stepMinutes(7, 1, 1, 180)).toBe(10);
    expect(stepMinutes(7, -1, 1, 180)).toBe(5);
    expect(stepMinutes(5, 1, 1, 180)).toBe(10);
  });

  it("stays in range", () => {
    expect(stepMinutes(1, -1, 1, 180)).toBe(1);
    expect(stepMinutes(180, 1, 1, 180)).toBe(180);
    expect(stepMinutes(58, 1, 1, 60)).toBe(60);
  });
});
