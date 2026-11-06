// Which language the app speaks: pure rules, so they are easy to test. The choice itself lives in language.svelte.ts.

export const LANGUAGES = [
  { code: "en", name: "English" },
  { code: "es", name: "Español" },
] as const;

export type LanguageCode = (typeof LANGUAGES)[number]["code"];
export type LanguageChoice = "auto" | LanguageCode;

export const isLanguage = (value: unknown): value is LanguageCode => LANGUAGES.some((l) => l.code === value);

export const isChoice = (value: unknown): value is LanguageChoice => value === "auto" || isLanguage(value);

/**
 * The language to use: the person's choice, or for "auto" the first of the browser's preferred languages that the app
 * has (matching on the part before any region, so `es-MX` finds `es`), else English.
 */
export function resolveLanguage(choice: LanguageChoice, preferred: readonly string[]): LanguageCode {
  if (choice !== "auto") return choice;
  for (const tag of preferred) {
    const base = tag.toLowerCase().split("-")[0];
    const found = LANGUAGES.find((l) => l.code === base);
    if (found) return found.code;
  }
  return "en";
}

/** Fill `{name}` placeholders; unknown ones stay as written. */
export function fill(text: string, params: Record<string, string | number>): string {
  return text.replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

/** `one` for the singular and `other` for everything else; the choice follows the language's own plural rules. */
export function pluralForm(code: string, n: number): "one" | "other" {
  return new Intl.PluralRules(code).select(n) === "one" ? "one" : "other";
}
