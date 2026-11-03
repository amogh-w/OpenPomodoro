import { describe, expect, it } from "vitest";
import { validUi, withUi } from "./backupFile";

describe("validUi", () => {
  it("keeps the three look settings when they are valid", () => {
    expect(validUi({ "ui-flavor": "mocha", "ui-accent": "peach", "ui-radius": "soft" })).toEqual({ "ui-flavor": "mocha", "ui-accent": "peach", "ui-radius": "soft" });
  });

  it("drops unknown keys and unknown values", () => {
    const out = validUi({ "ui-flavor": "neon", "ui-accent": "lavender", "ui-radius": 3, "op_device": "x", "evil": "<script>" });
    expect(out).toEqual({ "ui-accent": "lavender" });
  });

  it("copes with anything that is not an object", () => {
    for (const raw of [undefined, null, "mocha", 5, [], true]) expect(validUi(raw), String(raw)).toEqual({});
  });
});

describe("withUi", () => {
  it("adds the stored look to the backup under ui and leaves the rest alone", () => {
    const stored: Record<string, string> = { "ui-flavor": "latte", "ui-accent": "blue" };
    const out = withUi({ app: "openpomodoro", sessions: [1] }, (k) => stored[k] ?? null);
    expect(out).toEqual({ app: "openpomodoro", sessions: [1], ui: { "ui-flavor": "latte", "ui-accent": "blue" } });
  });

  it("writes an empty ui when nothing is stored", () => {
    expect(withUi({ a: 1 }, () => null)).toEqual({ a: 1, ui: {} });
  });
});
