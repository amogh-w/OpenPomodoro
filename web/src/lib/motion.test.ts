import { describe, expect, it } from "vitest";
import { tickInterval } from "./motion";

describe("tickInterval", () => {
  it("refreshes more slowly when motion is reduced", () => {
    expect(tickInterval(false)).toBe(250);
    expect(tickInterval(true)).toBe(1000);
  });
});
