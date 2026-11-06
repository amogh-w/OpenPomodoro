// Saving a backup to a file and restoring one. The backup is the server's JSON export; this adds the browser's look
// (theme, accent, corner radius) when saving and puts it back when restoring.
import { api } from "./api";

const UI_KEYS = ["ui-flavor", "ui-accent", "ui-radius"] as const;

const FLAVORS = ["auto", "latte", "frappe", "macchiato", "mocha"];
const ACCENTS = ["rosewater", "flamingo", "pink", "mauve", "red", "peach", "yellow", "green", "teal", "sky", "blue", "lavender"];
const RADII = ["tight", "apple", "soft"];

const ALLOWED: Record<(typeof UI_KEYS)[number], string[]> = { "ui-flavor": FLAVORS, "ui-accent": ACCENTS, "ui-radius": RADII };

/** The look settings a backup carries, keeping only known keys with known values. */
export function validUi(raw: unknown): Record<string, string> {
  const out: Record<string, string> = {};
  if (typeof raw !== "object" || raw === null) return out;
  const source = raw as Record<string, unknown>;
  for (const key of UI_KEYS) {
    const value = source[key];
    if (typeof value === "string" && ALLOWED[key].includes(value)) out[key] = value;
  }
  return out;
}

/** What the server wrote, with this browser's look added under `ui`. */
export function withUi(backup: Record<string, unknown>, read: (key: string) => string | null): Record<string, unknown> {
  const ui: Record<string, string> = {};
  for (const key of UI_KEYS) {
    const value = read(key);
    if (value !== null) ui[key] = value;
  }
  return { ...backup, ui };
}

function readStored(key: string): string | null {
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

/** Download the JSON backup. Returns false if the server did not give one. */
export async function downloadBackup(day: string): Promise<boolean> {
  const res = await fetch("/api/export?format=json", { credentials: "same-origin" });
  if (!res.ok) return false;
  const data = (await res.json()) as Record<string, unknown>;
  const blob = new Blob([JSON.stringify(withUi(data, readStored), null, 2)], { type: "application/json" });
  const link = document.createElement("a");
  link.href = URL.createObjectURL(blob);
  link.download = `openpomodoro-${day}.json`;
  link.click();
  URL.revokeObjectURL(link.href);
  return true;
}

export interface Restored {
  categories: number;
  tasks: number;
  sessions: number;
  settings: number;
  skipped: number;
}

/** Restore a backup file. Throws an `ApiError` with code `bad_backup` for a file that is not one. */
export async function restoreBackup(text: string, applyLook: (key: string, value: string) => void): Promise<Restored> {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    parsed = null;
  }
  const result = await api.importBackup(parsed);
  const ui = validUi((parsed as { ui?: unknown } | null)?.ui);
  for (const [key, value] of Object.entries(ui)) applyLook(key, value);
  return { ...result.imported, skipped: result.skipped };
}
