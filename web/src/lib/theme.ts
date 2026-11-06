// Thin wrapper over the kit's theme.js. The kit owns the `ui-*` keys and the data-flavor / data-accent /
// data-radius hooks; this file only writes a choice and asks the kit to apply it. No second switcher.
export type ThemePref = "flavor" | "accent" | "radius";

export const flavors = [
  "auto",
  "latte",
  "frappe",
  "macchiato",
  "mocha",
  "contrast-light",
  "contrast-dark",
  "nord",
  "gruvbox",
  "solarized",
] as const;
export const radii = ["tight", "apple", "soft"] as const;
export const accents: readonly string[] = ACCENTS;

export function readTheme(): { flavor: string; accent: string; radius: string } {
  return {
    flavor: store.get("ui-flavor") ?? "auto",
    accent: store.get("ui-accent") ?? "lavender",
    radius: store.get("ui-radius") ?? "apple",
  };
}

export function setTheme(pref: ThemePref, value: string): void {
  store.set(`ui-${pref}`, value);
  applyTheme();
}

/** Let the kit mark the currently selected buttons (aria-pressed, ring) after they are rendered. */
export function syncThemeButtons(): void {
  applyTheme();
}

/** Put back one look setting from a restored backup (the key is `ui-flavor`, `ui-accent` or `ui-radius`). */
export function applyThemeKey(key: string, value: string): void {
  store.set(key, value);
  applyTheme();
}

/** The JSON of the person's own theme, if one was saved. */
export function readCustomText(): string {
  return store.get("ui-custom") ?? "";
}

/** Save a custom theme and switch to it now. The caller has already checked it with `parseCustomTheme`. */
export function saveCustomTheme(text: string): void {
  store.set("ui-custom", text);
  store.set("ui-flavor", "custom");
  applyTheme();
}
