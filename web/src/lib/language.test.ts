import { describe, expect, it } from "vitest";
import en from "../locales/en";
import es from "../locales/es";
import { fill, isChoice, pluralForm, resolveLanguage } from "./language";

describe("resolveLanguage", () => {
  it("uses an explicit choice whatever the browser says", () => {
    expect(resolveLanguage("en", ["es-ES"])).toBe("en");
    expect(resolveLanguage("es", ["en-US"])).toBe("es");
  });

  it("follows the browser's first language the app has, ignoring the region", () => {
    expect(resolveLanguage("auto", ["es-MX", "en"])).toBe("es");
    expect(resolveLanguage("auto", ["fr-FR", "ES", "en"])).toBe("es");
    expect(resolveLanguage("auto", ["de", "fr"])).toBe("en");
    expect(resolveLanguage("auto", [])).toBe("en");
  });
});

describe("helpers", () => {
  it("accepts only known choices", () => {
    for (const ok of ["auto", "en", "es"]) expect(isChoice(ok)).toBe(true);
    for (const bad of ["fr", "", null, 3, "EN"]) expect(isChoice(bad)).toBe(false);
  });

  it("fills placeholders and leaves unknown ones", () => {
    expect(fill("{a} and {b}", { a: 1 })).toBe("1 and {b}");
  });

  it("chooses plural forms by the language's rules", () => {
    expect([pluralForm("en", 1), pluralForm("en", 0), pluralForm("es", 1), pluralForm("es", 2)]).toEqual(["one", "other", "one", "other"]);
  });
});

describe("the Spanish catalog", () => {
  it("has exactly the keys of the English one, none empty, with the same placeholders", () => {
    expect(Object.keys(es).sort()).toEqual(Object.keys(en).sort());
    const holes = (text: string): string[] => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1] ?? "").sort();
    for (const key of Object.keys(en) as (keyof typeof en)[]) {
      expect(es[key], key).not.toBe("");
      expect(holes(es[key]), key).toEqual(holes(en[key]));
    }
  });
});
