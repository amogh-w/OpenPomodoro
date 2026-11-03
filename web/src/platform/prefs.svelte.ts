// Per-device preferences: things that belong to this screen, not to the account (so they are not synced).
// Stored under the app's `op_` prefix.
import { type AlertPrefs, parseAlertPrefs } from "../lib/alertPrefs";

const KEEP_AWAKE = "op_keep_awake";
const ALERTS = "op_alerts";
const ALLOW_LINKS = "op_allow_links";
const REDUCE_MOTION = "op_reduce_motion";
const DIAL_STEP = "op_dial_step";
const ASK_REFLECTION = "op_ask_reflection";
const FOCUS_ON_CATEGORY = "op_focus_on_category";

/** Minutes the timer dial snaps to. */
export const DIAL_STEPS = [1, 5, 10, 15] as const;
export type DialStep = (typeof DIAL_STEPS)[number];
const initialStep = (): DialStep => {
  const n = Number(read(DIAL_STEP));
  return (DIAL_STEPS as readonly number[]).includes(n) ? (n as DialStep) : 5;
};

function read(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function write(key: string, value: string): void {
  try {
    localStorage.setItem(key, value);
  } catch {
    // Unavailable storage means the choice lasts until the page closes.
  }
}

function readJson(key: string): unknown {
  try {
    const text = read(key);
    return text === null ? null : JSON.parse(text);
  } catch {
    return null;
  }
}

/** Until the person chooses, follow the system's "reduce motion" setting. */
function initialReduceMotion(): boolean {
  const stored = read(REDUCE_MOTION);
  if (stored !== null) return stored === "1";
  try {
    return matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

export const prefs = $state({
  keepAwake: read(KEEP_AWAKE) === "1",
  allowLinks: read(ALLOW_LINKS) === "1",
  reduceMotion: initialReduceMotion(),
  dialStep: initialStep(),
  /** Show the reflect screen (mood, notes) when a focus session is finished. */
  askReflection: read(ASK_REFLECTION) !== "0",
  /** Move to the intention field after choosing a category. */
  focusOnCategory: read(FOCUS_ON_CATEGORY) === "1",
});

export function setDialStep(step: DialStep): void {
  prefs.dialStep = step;
  write(DIAL_STEP, String(step));
}

export function setAskReflection(on: boolean): void {
  prefs.askReflection = on;
  write(ASK_REFLECTION, on ? "1" : "0");
}

export function setFocusOnCategory(on: boolean): void {
  prefs.focusOnCategory = on;
  write(FOCUS_ON_CATEGORY, on ? "1" : "0");
}

export function setReduceMotion(on: boolean): void {
  prefs.reduceMotion = on;
  write(REDUCE_MOTION, on ? "1" : "0");
}

export function setKeepAwake(on: boolean): void {
  prefs.keepAwake = on;
  write(KEEP_AWAKE, on ? "1" : "0");
}

/** Let links such as `#/start` act without asking each time. Off by default: any page can open a link. */
export function setAllowLinks(on: boolean): void {
  prefs.allowLinks = on;
  write(ALLOW_LINKS, on ? "1" : "0");
}

/** Sound, notification, warning and quiet-hours choices for this device. */
export const alertPrefs = $state<AlertPrefs>(parseAlertPrefs(readJson(ALERTS)));

export function updateAlertPrefs(change: Partial<AlertPrefs>): void {
  // Run the merged result through the parser so nothing invalid is ever kept or stored.
  const next = parseAlertPrefs({ ...$state.snapshot(alertPrefs), ...change });
  Object.assign(alertPrefs, next);
  write(ALERTS, JSON.stringify(next));
}
