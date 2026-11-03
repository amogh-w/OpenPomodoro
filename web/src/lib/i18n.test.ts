import { describe, expect, it } from "vitest";
import en from "../locales/en";
import { t } from "./i18n";

describe("t", () => {
  it("returns the catalog string", () => {
    expect(t("nav.timer")).toBe(en["nav.timer"]);
  });

  it("has no empty strings in the catalog", () => {
    for (const [key, value] of Object.entries(en)) expect(value, key).not.toBe("");
  });
});
