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

export function actionFor(event: KeyPress, context: ShortcutContext): ShortcutAction | null {
  if (event.ctrlKey || event.metaKey || event.altKey) return null;
  if (context.dialogOpen) return null;

  // Escape always works: it leaves a field, even in the middle of typing.
  if (event.key === "Escape") return "cancel";
  if (isTyping(event.target)) return null;

  // Space on a focused button presses that button; it must not also toggle the timer.
  if (event.key === " " && ACTIVATED_BY_SPACE.has((event.target?.tagName ?? "").toUpperCase())) return null;

  switch (event.key) {
    case " ":
    case "p":
    case "P":
      return "toggle_pause";
    case "s":
    case "S":
      return "start";
    case "+":
    case "=":
      return "extend";
    case "-":
    case "_":
      return "shrink";
    case "/":
      return "focus_input";
    case "f":
    case "F":
      return "finish";
    case "b":
    case "B":
      return "break";
    default:
      return null;
  }
}

/** For the settings screen: what each key does, as catalog keys. */
export const SHORTCUT_HELP: readonly { keys: string; action: string }[] = [
  { keys: "Space / P", action: "shortcuts.pause" },
  { keys: "S", action: "shortcuts.start" },
  { keys: "+ / −", action: "shortcuts.extend" },
  { keys: "F", action: "shortcuts.finish" },
  { keys: "B", action: "shortcuts.break" },
  { keys: "/", action: "shortcuts.focus" },
  { keys: "Esc", action: "shortcuts.cancel" },
];
