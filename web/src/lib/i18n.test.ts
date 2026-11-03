import { describe, expect, it } from "vitest";
import en from "../locales/en";
import { t, tn } from "./i18n";

describe("t", () => {
  it("returns the catalog string", () => {
    expect(t("nav.timer")).toBe(en["nav.timer"]);
  });

  it("has no empty strings in the catalog", () => {
    for (const [key, value] of Object.entries(en)) expect(value, key).not.toBe("");
  });
});

describe("tn", () => {
  it("picks the singular for exactly one", () => {
    expect(tn("common.pending", 1)).toBe("1 change waiting to sync.");
  });

  it("uses the plural otherwise, including zero", () => {
    expect(tn("common.pending", 2)).toBe("2 changes waiting to sync.");
    expect(tn("common.pending", 0)).toBe("0 changes waiting to sync.");
  });

  it("works for the sign-out warning too", () => {
    expect(tn("account.pendingWarning", 1)).toContain("1 change has");
    expect(tn("account.pendingWarning", 3)).toContain("3 changes have");
  });
});
