// A person's own theme, as JSON. The kit's theme.js applies it; this mirrors its rules so the app can check a file
// and give a reason before saving it (and so a restored backup can be filtered the same way).
export const BASE_FLAVORS = ["latte", "frappe", "macchiato", "mocha", "contrast-light", "contrast-dark", "nord", "gruvbox", "solarized"] as const;

/** The colour tokens a custom theme may set (the kit's `--ctp-*` names without the prefix). */
export const TOKENS = [
  "rosewater", "flamingo", "pink", "mauve", "red", "maroon", "peach", "yellow", "green", "teal", "sky", "sapphire", "blue", "lavender",
  "text", "subtext1", "subtext0", "overlay2", "overlay1", "overlay0", "surface2", "surface1", "surface0", "base", "mantle", "crust",
] as const;

export interface CustomTheme {
  base: (typeof BASE_FLAVORS)[number];
  colors: Partial<Record<(typeof TOKENS)[number], string>>;
}

export const MAX_CUSTOM_BYTES = 4096;

export type CustomThemeError = "too_large" | "not_json" | "not_object" | "no_colors";

const HEX = /^#[0-9a-fA-F]{6}$/;

/** Read a theme file. Unknown keys and bad colours are dropped; a file with no usable colour is refused. */
export function parseCustomTheme(text: string): { ok: true; theme: CustomTheme } | { ok: false; error: CustomThemeError } {
  if (text.length > MAX_CUSTOM_BYTES) return { ok: false, error: "too_large" };
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return { ok: false, error: "not_json" };
  }
  if (typeof raw !== "object" || raw === null || Array.isArray(raw)) return { ok: false, error: "not_object" };
  const o = raw as { base?: unknown; colors?: unknown };
  const base = BASE_FLAVORS.find((f) => f === o.base) ?? "mocha";
  const source = typeof o.colors === "object" && o.colors !== null ? (o.colors as Record<string, unknown>) : {};
  const colors: CustomTheme["colors"] = {};
  for (const token of TOKENS) {
    const value = source[token];
    if (typeof value === "string" && HEX.test(value)) colors[token] = value;
  }
  return Object.keys(colors).length === 0 ? { ok: false, error: "no_colors" } : { ok: true, theme: { base, colors } };
}

/** Something to start from: a dark theme with a few colours changed. */
export const EXAMPLE_THEME = `{
  "base": "mocha",
  "colors": {
    "base": "#14181f",
    "mantle": "#0f1217",
    "crust": "#0a0c10",
    "text": "#e8ecf2",
    "blue": "#6cb6ff",
    "green": "#7ee787"
  }
}`;
