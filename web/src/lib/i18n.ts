import en from "../locales/en";

export type MessageKey = keyof typeof en;

type Params = Record<string, string | number>;

/** Look up a UI string. `{name}` placeholders are filled from `params`; unknown placeholders stay as written. */
export function t(key: MessageKey, params: Params = {}): string {
  return en[key].replace(/\{(\w+)\}/g, (whole, name: string) => (name in params ? String(params[name]) : whole));
}
