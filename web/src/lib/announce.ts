// What a screen reader should be told when the timer changes state. The clock itself is not announced every
// second (that would be unbearable); only the moments that matter are.
import type { MessageKey } from "./i18n";

const paused = (state: string): boolean => state.startsWith("paused_");

/**
 * The announcement for going from `previous` to `next`, or null for silence. `previous` is null on the first
 * look at the state (when the app opens), which is never announced.
 */
export function announcementFor(previous: string | null, next: string): MessageKey | null {
  if (previous === null || previous === next) return null;
  if (paused(next)) return "announce.paused";
  if (paused(previous) && !paused(next)) return "announce.resumed";
  switch (next) {
    case "breathe":
      return "announce.breathe";
    case "session":
      return "announce.focusStarted";
    case "session_end":
      return "announce.focusTimeUp";
    case "reflect":
      return "announce.focusComplete";
    case "rest":
      return "announce.breakStarted";
    case "rest_end":
      return "announce.breakOver";
    default:
      return null;
  }
}
