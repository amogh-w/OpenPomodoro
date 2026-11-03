// The notes being written for the focus session in progress. They are kept as the person types (and across a
// reload) and only become part of the session when it is saved, with the command that ends it.
const KEY = "op_draft";

function read(): { key: number; notes: string } | null {
  try {
    const raw = localStorage.getItem(KEY);
    if (raw === null) return null;
    const parsed = JSON.parse(raw) as { key?: unknown; notes?: unknown };
    return typeof parsed.key === "number" && typeof parsed.notes === "string"
      ? { key: parsed.key, notes: parsed.notes }
      : null;
  } catch {
    return null;
  }
}

function write(value: { key: number; notes: string } | null): void {
  try {
    if (value === null) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, JSON.stringify(value));
  } catch {
    // Without storage the draft lasts until the page closes.
  }
}

/** `key` is when the session started, so a draft never leaks into a different session. */
export const draft = $state<{ key: number | null; notes: string }>({ key: null, notes: "" });

/** Make the draft belong to the session starting at `key` (null for none), restoring a saved one if it matches. */
export function attachDraft(key: number | null): void {
  if (key === draft.key) return;
  draft.key = key;
  const saved = key === null ? null : read();
  draft.notes = saved !== null && saved.key === key ? saved.notes : "";
}

export function setNotes(text: string): void {
  draft.notes = text;
  if (draft.key !== null) write({ key: draft.key, notes: text });
}

/** The session was saved: forget the draft. */
export function clearDraft(): void {
  draft.key = null;
  draft.notes = "";
  write(null);
}
