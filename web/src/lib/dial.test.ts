import { describe, expect, it } from "vitest";
import { angleOf, handleAngle, scrub, snap, turnBetween } from "./dial";

const range = { min: 5, max: 180, step: 5 };

describe("angleOf", () => {
  it("measures clockwise from the top", () => {
    expect(angleOf(0, 0, 0, -10)).toBeCloseTo(0);
    expect(angleOf(0, 0, 10, 0)).toBeCloseTo(90);
    expect(angleOf(0, 0, 0, 10)).toBeCloseTo(180);
    expect(angleOf(0, 0, -10, 0)).toBeCloseTo(270);
  });
});

describe("turnBetween", () => {
  it("takes the short way round, including across the top", () => {
    expect(turnBetween(10, 30)).toBeCloseTo(20);
    expect(turnBetween(30, 10)).toBeCloseTo(-20);
    expect(turnBetween(350, 10)).toBeCloseTo(20);
    expect(turnBetween(10, 350)).toBeCloseTo(-20);
  });
});

describe("snap", () => {
  it("rounds to the step and stays in range", () => {
    expect(snap(27, range)).toBe(25);
    expect(snap(28, range)).toBe(30);
    expect(snap(1, range)).toBe(5);
    expect(snap(999, range)).toBe(180);
  });
});

describe("scrub", () => {
  it("adds a minute per 6 degrees, snapped", () => {
    expect(scrub(25, 0, range)).toBe(25);
    expect(scrub(25, 30, range)).toBe(30);
    expect(scrub(25, 360, range)).toBe(85);
    expect(scrub(25, -90, range)).toBe(10);
    expect(scrub(25, 4, range)).toBe(25);
  });

  it("does not leave the range however far it is turned", () => {
    expect(scrub(25, -3600, range)).toBe(5);
    expect(scrub(25, 36000, range)).toBe(180);
  });
});

describe("handleAngle", () => {
  it("wraps each hour", () => {
    expect([0, 15, 30, 60, 75, 90].map(handleAngle)).toEqual([0, 90, 180, 0, 90, 180]);
  });
});
