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
function matchCategory(typed: string, categories: Category[]): Category | null {
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
