// Suggestions drawn from what the person has already done: intentions to autocomplete, and notes from earlier
// sessions to show while writing new ones.
import type { SessionRecord } from "./types";

const focusOnly = (sessions: SessionRecord[]): SessionRecord[] =>
  sessions.filter((s) => s.kind === "focus").sort((a, b) => b.started_at - a.started_at);

/** Past intentions, most recent first, each once (ignoring case), at most `limit`. */
export function recentIntentions(sessions: SessionRecord[], limit = 8): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const s of focusOnly(sessions)) {
    const text = s.intention?.trim();
    if (!text || seen.has(text.toLowerCase())) continue;
    seen.add(text.toLowerCase());
    out.push(text);
    if (out.length >= limit) break;
  }
  return out;
}

export interface RecentNote {
  text: string;
  intention: string | null;
  at: number;
}

/**
 * Notes from earlier sessions, newest first. Notes from sessions with the same intention as the one being written
 * come first, since they are the ones most likely to matter.
 */
export function recentNotes(sessions: SessionRecord[], intention: string | null, limit = 3): RecentNote[] {
  const wanted = intention?.trim().toLowerCase() ?? "";
  const notes = focusOnly(sessions)
    .filter((s) => s.notes && s.notes.trim() !== "")
    .map((s) => ({ text: (s.notes as string).trim(), intention: s.intention?.trim() || null, at: s.started_at }));
  const same = wanted === "" ? [] : notes.filter((n) => n.intention?.toLowerCase() === wanted);
  const rest = notes.filter((n) => !same.includes(n));
  return [...same, ...rest].slice(0, limit);
}

/** Every focus session that has notes, newest first; `category` narrows it to one category (null means all). */
export function notesFeed(sessions: SessionRecord[], category: string | null, limit = 20): SessionRecord[] {
  return focusOnly(sessions)
    .filter((s) => s.notes && s.notes.trim() !== "" && (category === null || s.category === category))
    .slice(0, limit);
}
