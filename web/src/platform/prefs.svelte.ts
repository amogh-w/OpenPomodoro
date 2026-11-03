// Per-device preferences: things that belong to this screen, not to the account (so they are not synced).
// Stored under the app's `op_` prefix.
const KEEP_AWAKE = "op_keep_awake";

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

export const prefs = $state({ keepAwake: read(KEEP_AWAKE) === "1" });

export function setKeepAwake(on: boolean): void {
  prefs.keepAwake = on;
  write(KEEP_AWAKE, on ? "1" : "0");
}
