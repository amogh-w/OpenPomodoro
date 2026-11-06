import en from "../locales/en";
import es from "../locales/es";
import { fill, pluralForm } from "./language";
import { activeLanguage } from "./language.svelte";

export type MessageKey = keyof typeof en;

type Params = Record<string, string | number>;

const tables: Record<string, Record<string, string>> = { en, es };

/** Look up a UI string in the language in use (English where a translation is missing). `{name}` placeholders are filled from `params`. */
export function t(key: MessageKey, params: Params = {}): string {
  const text = tables[activeLanguage()]?.[key] ?? en[key];
  return fill(text, params);
}

type PluralBase = "common.pending" | "account.pendingWarning" | "history.streak" | "history.sessions" | "insights.activeDays";

/** A string that depends on a count: `<base>.one` for 1, `<base>.other` otherwise. `{n}` is filled in. */
export function tn(base: PluralBase, n: number, params: Params = {}): string {
  return t(`${base}.${pluralForm(activeLanguage(), n)}`, { n, ...params });
}
