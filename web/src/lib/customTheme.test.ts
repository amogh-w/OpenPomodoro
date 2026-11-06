import { describe, expect, it } from "vitest";
import { EXAMPLE_THEME, MAX_CUSTOM_BYTES, parseCustomTheme } from "./customTheme";

describe("parseCustomTheme", () => {
  it("accepts the example", () => {
    const r = parseCustomTheme(EXAMPLE_THEME);
    expect(r.ok && r.theme.base).toBe("mocha");
    expect(r.ok && r.theme.colors.blue).toBe("#6cb6ff");
  });

  it("falls back to mocha for an unknown base and drops unknown keys and bad colours", () => {
    const r = parseCustomTheme(JSON.stringify({ base: "neon", colors: { red: "#ff0000", blue: "blue", green: "#12345", evil: "#000000", text: "url(x)" } }));
    expect(r).toEqual({ ok: true, theme: { base: "mocha", colors: { red: "#ff0000" } } });
  });

  it("refuses what cannot be used, with a reason", () => {
    expect(parseCustomTheme("nope")).toEqual({ ok: false, error: "not_json" });
    expect(parseCustomTheme("[1]")).toEqual({ ok: false, error: "not_object" });
    expect(parseCustomTheme("null")).toEqual({ ok: false, error: "not_object" });
    expect(parseCustomTheme('{"base":"nord"}')).toEqual({ ok: false, error: "no_colors" });
    expect(parseCustomTheme('{"colors":{"red":"red"}}')).toEqual({ ok: false, error: "no_colors" });
    expect(parseCustomTheme(" ".repeat(MAX_CUSTOM_BYTES + 1))).toEqual({ ok: false, error: "too_large" });
  });
});
