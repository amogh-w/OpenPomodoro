// Earlier sessions, loaded for suggestions (autocomplete and recent notes). Only fetched while online; without them
// the suggestions are simply empty.
import { api } from "./api";
import type { SessionRecord } from "./types";

const DAY = 24 * 60 * 60 * 1000;
const WINDOW = 120 * DAY;

export const recent = $state<{ sessions: SessionRecord[] }>({ sessions: [] });

export async function refreshRecent(now: number = Date.now()): Promise<void> {
  try {
    recent.sessions = (await api.history(now - WINDOW, now + DAY)).sessions;
  } catch {
    // Offline or not signed in: keep what we had.
  }
}

export function forgetRecent(): void {
  recent.sessions = [];
}
