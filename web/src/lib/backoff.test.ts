import { describe, expect, it } from "vitest";
import { reconnectDelay } from "./backoff";

describe("reconnectDelay", () => {
  it("doubles and caps at 30 seconds", () => {
    expect([0, 1, 2, 3, 4, 5, 6, 50].map(reconnectDelay)).toEqual([1_000, 2_000, 4_000, 8_000, 16_000, 30_000, 30_000, 30_000]);
  });

  it("treats a negative attempt as the first", () => {
    expect(reconnectDelay(-3)).toBe(1_000);
  });
});
