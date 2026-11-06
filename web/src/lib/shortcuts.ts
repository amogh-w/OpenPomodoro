// Keyboard shortcuts for the timer screen. Pure: given a key press and what is going on, which action (if any).
// Anything that could be a person typing, or a browser or system shortcut, is left alone.

export type ShortcutAction = "toggle_pause" | "start" | "extend" | "shrink" | "focus_input" | "finish" | "break" | "cancel";

export interface KeyPress {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  /** The element that has focus. */
  target: { tagName?: string; isContentEditable?: boolean } | null;
}

export interface ShortcutContext {
  /** A dialog is open, so keys belong to it. */
  dialogOpen: boolean;
}

const TYPING_TAGS = new Set(["INPUT", "TEXTAREA", "SELECT"]);
/** Elements that already do something when Space is pressed on them. */
const ACTIVATED_BY_SPACE = new Set(["BUTTON", "A", "SUMMARY"]);

export function isTyping(target: KeyPress["target"]): boolean {
  return target !== null && (TYPING_TAGS.has((target.tagName ?? "").toUpperCase()) || target.isContentEditable === true);
}

/** The actions whose keys can be changed. Escape always leaves a field and is not one of them. */
export const REBINDABLE = ["toggle_pause", "start", "extend", "shrink", "finish", "break", "focus_input"] as const;
export type Rebindable = (typeof REBINDABLE)[number];
export type Keymap = Record<Rebindable, string[]>;

export const defaultKeymap: Keymap = {
  toggle_pause: [" ", "p"],
  start: ["s"],
  extend: ["+", "="],
  shrink: ["-", "_"],
  finish: ["f"],
  break: ["b"],
  focus_input: ["/"],
};

/** A key as stored: one character, letters in lower case. Anything else (Enter, arrows, F5, Tab) cannot be bound. */
export function normalizeKey(key: string): string | null {
  if (key === " ") return " ";
  if (key.length !== 1 || key.trim() === "") return null;
  return key.toLowerCase();
}

/** Read a stored keymap, falling back to the default for any action whose keys are missing or invalid. */
export function parseKeymap(raw: unknown): Keymap {
  const o = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  const out = Object.fromEntries(REBINDABLE.map((a) => [a, [...defaultKeymap[a]]])) as Keymap;
  const used = new Set<string>();
  for (const action of REBINDABLE) {
    const value = o[action];
    if (!Array.isArray(value) || value.length === 0 || value.length > 3) continue;
    const keys = value.map((k) => (typeof k === "string" ? normalizeKey(k) : null));
    if (keys.some((k) => k === null || used.has(k))) continue;
    out[action] = keys as string[];
    for (const k of keys as string[]) used.add(k);
  }
  // A stored choice may collide with another action's default; two actions on one key would be a trap, so start over.
  const all = REBINDABLE.flatMap((a) => out[a]);
  if (new Set(all).size !== all.length) return Object.fromEntries(REBINDABLE.map((a) => [a, [...defaultKeymap[a]]])) as Keymap;
  return out;
}

/** Give `action` the single key `key`. Refused when the key cannot be bound or another action already uses it. */
export function bindKey(map: Keymap, action: Rebindable, key: string): { map: Keymap } | { error: "invalid" | "taken"; by?: Rebindable } {
  const normal = normalizeKey(key);
  if (normal === null) return { error: "invalid" };
  const owner = REBINDABLE.find((a) => a !== action && map[a].includes(normal));
  if (owner) return { error: "taken", by: owner };
  return { map: { ...map, [action]: [normal] } };
}

export function actionFor(event: KeyPress, context: ShortcutContext, keymap: Keymap = defaultKeymap): ShortcutAction | null {
  if (event.ctrlKey || event.metaKey || event.altKey) return null;
  if (context.dialogOpen) return null;

  // Escape always works: it leaves a field, even in the middle of typing.
  if (event.key === "Escape") return "cancel";
  if (isTyping(event.target)) return null;

  // Space on a focused button presses that button; it must not also toggle the timer.
  if (event.key === " " && ACTIVATED_BY_SPACE.has((event.target?.tagName ?? "").toUpperCase())) return null;

  const key = normalizeKey(event.key);
  if (key === null) return null;
  return REBINDABLE.find((action) => keymap[action].includes(key)) ?? null;
}

/** How a key is shown to people. */
export const keyLabel = (key: string): string =>
  key === " " ? "Space" : key === "Escape" ? "Esc" : key.length === 1 ? key.toUpperCase() : key;

/** For the settings screen: the catalog key that names each action. */
export const SHORTCUT_LABELS: Record<Rebindable, string> = {
  toggle_pause: "shortcuts.pause",
  start: "shortcuts.start",
  extend: "shortcuts.extend",
  shrink: "shortcuts.shrink",
  finish: "shortcuts.finish",
  break: "shortcuts.break",
  focus_input: "shortcuts.focus",
};
