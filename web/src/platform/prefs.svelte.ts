// Per-device preferences: things that belong to this screen, not to the account (so they are not synced).
// Stored under the app's `op_` prefix.
import { type AlertPrefs, parseAlertPrefs } from "../lib/alertPrefs";

const KEEP_AWAKE = "op_keep_awake";
const ALERTS = "op_alerts";

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

export const prefs = $state({ keepAwake: read(KEEP_AWAKE) === "1" });

export function setKeepAwake(on: boolean): void {
  prefs.keepAwake = on;
  write(KEEP_AWAKE, on ? "1" : "0");
}

/** Sound, notification, warning and quiet-hours choices for this device. */
export const alertPrefs = $state<AlertPrefs>(parseAlertPrefs(readJson(ALERTS)));

export function updateAlertPrefs(change: Partial<AlertPrefs>): void {
  // Run the merged result through the parser so nothing invalid is ever kept or stored.
  const next = parseAlertPrefs({ ...$state.snapshot(alertPrefs), ...change });
  Object.assign(alertPrefs, next);
  write(ALERTS, JSON.stringify(next));
}
