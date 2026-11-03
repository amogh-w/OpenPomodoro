import { describe, expect, it } from "vitest";
import { formatElapsed, formatRemaining, minutesLeft } from "./format";

describe("formatRemaining", () => {
  it("shows the full time during the first second and rounds up after", () => {
    expect(formatRemaining(25 * 60_000)).toBe("25:00");
    expect(formatRemaining(25 * 60_000 - 200)).toBe("25:00");
    expect(formatRemaining(25 * 60_000 - 1_000)).toBe("24:59");
    expect(formatRemaining(1)).toBe("00:01");
    expect(formatRemaining(0)).toBe("00:00");
  });

  it("switches to hours when needed", () => {
    expect(formatRemaining(60 * 60_000)).toBe("1:00:00");
    expect(formatRemaining(90 * 60_000 + 5_000)).toBe("1:30:05");
  });

  it("never goes negative", () => {
    expect(formatRemaining(-5_000)).toBe("00:00");
  });
});

describe("formatElapsed", () => {
  it("rounds down", () => {
    expect(formatElapsed(0)).toBe("00:00");
    expect(formatElapsed(999)).toBe("00:00");
    expect(formatElapsed(61_999)).toBe("01:01");
    expect(formatElapsed(3_600_000)).toBe("1:00:00");
  });
});

describe("minutesLeft", () => {
  it("rounds up", () => {
    expect([0, 1, 60_000, 60_001, 25 * 60_000].map(minutesLeft)).toEqual([0, 1, 1, 2, 25]);
  });
});
