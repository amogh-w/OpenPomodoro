import en from "../locales/en";

export type MessageKey = keyof typeof en;

type Params = Record<string, string | number>;

/** Look up a UI string. `{name}` placeholders are filled from `params`; unknown placeholders stay as written. */
export function t(key: MessageKey, params: Params = {}): string {
  return en[key].replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}

type PluralBase = "common.pending" | "account.pendingWarning" | "history.streak" | "history.sessions";

/** A string that depends on a count: `<base>.one` for 1, `<base>.other` otherwise. `{n}` is filled in. */
export function tn(base: PluralBase, n: number, params: Params = {}): string {
  const form = new Intl.PluralRules("en").select(n) === "one" ? "one" : "other";
  return t(`${base}.${form}`, { n, ...params });
}
