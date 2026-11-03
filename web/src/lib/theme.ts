// Thin wrapper over the kit's theme.js. The kit owns the `ui-*` keys and the data-flavor / data-accent /
// data-radius hooks; this file only writes a choice and asks the kit to apply it. No second switcher.
export type ThemePref = "flavor" | "accent" | "radius";

export const flavors = ["auto", "latte", "frappe", "macchiato", "mocha"] as const;
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
