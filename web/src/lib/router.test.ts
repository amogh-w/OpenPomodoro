import { describe, expect, it } from "vitest";
import { hashFor, parseHash, routes } from "./router";

describe("parseHash", () => {
  it("maps known routes", () => {
    for (const route of routes) expect(parseHash(`#/${route}`)).toBe(route);
  });

  it("ignores a query string and trailing parts", () => {
    expect(parseHash("#/history?day=2026-11-03")).toBe("history");
    expect(parseHash("#/settings/extra")).toBe("settings");
  });

  it("falls back to the timer for empty or unknown hashes", () => {
    expect(parseHash("")).toBe("timer");
    expect(parseHash("#")).toBe("timer");
    expect(parseHash("#/nope")).toBe("timer");
  });

  it("round-trips with hashFor", () => {
    for (const route of routes) expect(parseHash(hashFor(route))).toBe(route);
  });
});
