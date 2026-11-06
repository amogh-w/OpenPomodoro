// A short log of what the app did and what went wrong, kept on this device so a problem can be described exactly:
// copy it from Settings and paste it into a report. It holds codes and counts only, never passwords, tokens or
// anything typed in a note or intention.
import type { DiagLine } from "./diagReport";

export { report } from "./diagReport";

const KEY = "op_diag";
export const MAX_LINES = 200;

export const diag = $state<{ lines: DiagLine[] }>({ lines: load() });

function load(): DiagLine[] {
  try {
    const raw: unknown = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    if (!Array.isArray(raw)) return [];
    return raw
      .filter((l): l is DiagLine => typeof l === "object" && l !== null && typeof (l as DiagLine).at === "number" && typeof (l as DiagLine).message === "string" && typeof (l as DiagLine).area === "string")
      .slice(-MAX_LINES);
  } catch {
    return [];
  }
}

function save(): void {
  try {
    localStorage.setItem(KEY, JSON.stringify($state.snapshot(diag.lines)));
  } catch {
    // Without storage the log lasts until the page closes.
  }
}

/** Note something. Messages are cut to a sensible length. */
export function note(area: string, message: string): void {
  diag.lines.push({ at: Date.now(), area, message: message.slice(0, 300) });
  if (diag.lines.length > MAX_LINES) diag.lines.splice(0, diag.lines.length - MAX_LINES);
  save();
}

export function clearDiag(): void {
  diag.lines = [];
  save();
}

/** Errors nobody caught belong in the log too. */
export function installDiagnostics(): void {
  addEventListener("error", (e) => note("error", e.message || "script error"));
  addEventListener("unhandledrejection", (e) => note("error", `unhandled: ${e.reason instanceof Error ? e.reason.message : String(e.reason)}`));
}
