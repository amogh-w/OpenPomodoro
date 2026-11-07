// Shortcuts typed into the intention field: `30m`, `1h`, `1h30m` set the length; `@name` picks a category.
// They are removed from the text that gets saved.
import type { Category } from "./types";

export interface ParsedIntention {
  text: string;
  /** Length typed as a shortcut, in ms. */
  durationMs: number | null;
  /** The category whose name matched `@name`, if any. */
  categoryId: string | null;
}

const DURATION = /^(?:(\d{1,2})h)?(?:(\d{1,3})m)?$/i;

function durationOf(token: string): number | null {
  const m = DURATION.exec(token);
  if (!m || (m[1] === undefined && m[2] === undefined)) return null;
  const minutes = Number(m[1] ?? 0) * 60 + Number(m[2] ?? 0);
  return minutes > 0 ? minutes * 60_000 : null;
}

const normal = (s: string): string => s.toLowerCase().replace(/\s+/g, "");

/** An exact name match, else a name that starts with what was typed (only if that is unambiguous). */
export function matchCategory(typed: string, categories: Category[]): Category | null {
  const wanted = normal(typed);
  if (wanted === "") return null;
  const exact = categories.find((c) => normal(c.name) === wanted);
  if (exact) return exact;
  const starts = categories.filter((c) => normal(c.name).startsWith(wanted));
  return starts.length === 1 ? (starts[0] as Category) : null;
}

export function parseIntention(input: string, categories: Category[]): ParsedIntention {
  let durationMs: number | null = null;
  let categoryId: string | null = null;
  const kept: string[] = [];

  for (const word of input.split(/\s+/).filter((w) => w !== "")) {
    const minutes = durationOf(word);
    if (minutes !== null) {
      durationMs = minutes;
    } else if (word.startsWith("@") && word.length > 1) {
      const found = matchCategory(word.slice(1), categories);
      if (found) categoryId = found.id;
      else kept.push(word); // not a category we know; leave it as written
    } else {
      kept.push(word);
    }
  }
  return { text: kept.join(" "), durationMs, categoryId };
}

/** The text without any length shortcut (`30m`, `1h30m`), so the dial can take over. */
export function stripDuration(input: string): string {
  return input
    .split(/\s+/)
    .filter((w) => w !== "" && durationOf(w) === null)
    .join(" ");
}

/** The text without any `@name` that picked a category, so the selector can take over. Unknown @words stay. */
export function stripCategory(input: string, categories: Category[]): string {
  return input
    .split(/\s+/)
    .filter((w) => w !== "" && !(w.startsWith("@") && w.length > 1 && matchCategory(w.slice(1), categories)))
    .join(" ");
}

/** What follows the `@` in the word being typed at the end of the text, or null if that word is not an `@word`. */
export function atFragment(input: string): string | null {
  const last = /(?:^|\s)@(\S*)$/.exec(input);
  return last ? (last[1] ?? "") : null;
}

/** Categories whose name starts with what has been typed after the `@` (all of them when nothing is typed yet). */
export function categoryMatches(fragment: string, categories: Category[]): Category[] {
  const wanted = normal(fragment);
  return categories.filter((c) => normal(c.name).startsWith(wanted));
}

/** Finish the `@word` being typed with a category's name (spaces left out, as the matcher expects). */
export function completeAt(input: string, name: string): string {
  return input.replace(/@\S*$/, `@${name.replace(/\s+/g, "")} `);
}
