// Pure helpers for categories: the icons a category may carry, the starter set, and ordering and totals.
import type { Category, SessionRecord } from "./types";

/** The icons a category may carry (Lucide names). The server's `CATEGORY_ICONS` in validate.rs must match. */
export const ICONS: readonly string[] = [
  "book-open",
  "graduation-cap",
  "briefcase",
  "code",
  "pencil",
  "dumbbell",
  "palette",
  "music",
  "mail",
  "wrench",
  "languages",
  "heart",
  "house",
  "shopping-cart",
  "brain",
  "flask-conical",
  "laptop",
  "notebook-pen",
  "bike",
  "utensils",
  "leaf",
  "camera",
  "gamepad-2",
  "tag",
];

export const FALLBACK_ICON = "tag";

/** The icon to draw for a category: its own if it is a known one, else a plain tag. */
export function iconOf(category: { icon?: string | null } | undefined): string {
  const icon = category?.icon;
  return icon && ICONS.includes(icon) ? icon : FALLBACK_ICON;
}

export interface Starter {
  /** The catalog key of its name, so it is created in the person's language. */
  key: "study" | "work" | "reading" | "exercise" | "creative" | "admin";
  color: string;
  icon: string;
}

/** The usual categories offered to someone who has none. */
export const STARTERS: readonly Starter[] = [
  { key: "study", color: "mauve", icon: "graduation-cap" },
  { key: "work", color: "blue", icon: "briefcase" },
  { key: "reading", color: "teal", icon: "book-open" },
  { key: "exercise", color: "green", icon: "dumbbell" },
  { key: "creative", color: "pink", icon: "palette" },
  { key: "admin", color: "peach", icon: "mail" },
];

/** Categories with the most recently used first, then the rest in the order they were made. */
export function orderByRecent(categories: readonly Category[], sessions: readonly SessionRecord[]): Category[] {
  const lastUsed = new Map<string, number>();
  for (const s of sessions) {
    if (s.kind !== "focus" || !s.category) continue;
    lastUsed.set(s.category, Math.max(lastUsed.get(s.category) ?? 0, s.started_at));
  }
  return [...categories].sort((a, b) => {
    const byUse = (lastUsed.get(b.id) ?? 0) - (lastUsed.get(a.id) ?? 0);
    return byUse !== 0 ? byUse : a.id.localeCompare(b.id);
  });
}

export interface CategoryTime {
  /** Category id, or null for sessions without one. */
  id: string | null;
  ms: number;
}

/** Focus time per category in the given sessions, most time first. */
export function timePerCategory(sessions: readonly SessionRecord[]): CategoryTime[] {
  const totals = new Map<string | null, number>();
  for (const s of sessions) {
    if (s.kind !== "focus") continue;
    const spent = s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);
    totals.set(s.category ?? null, (totals.get(s.category ?? null) ?? 0) + spent);
  }
  return [...totals.entries()].map(([id, ms]) => ({ id, ms })).sort((a, b) => b.ms - a.ms);
}
