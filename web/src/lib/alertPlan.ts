// Decides which alerts are due right now, from what the timer is doing. Pure: the caller keeps the memory between
// calls and does the playing. One alert per event however often this is asked, and no flood after the computer
// slept: anything long overdue is skipped rather than played.
import { MAX_REMINDERS } from "./alertPrefs";
import type { AlertKind } from "./sounds";

export interface AlertConfig {
  /** Milliseconds before the end for the "ending soon" tone; 0 is off. */
  warnLeadMs: number;
  /** Milliseconds between overtime reminders; 0 is off. */
  reminderMs: number;
}

/** What has already been announced for the current phase. */
export interface AlertMemory {
  key: string;
  warned: boolean;
  endHandled: boolean;
  reminders: number;
}

export const emptyMemory: AlertMemory = { key: "", warned: false, endHandled: false, reminders: 0 };

export interface AlertInput {
  /** Identifies this run of this phase (kind, start and planned length); null when there is no phase to alert about. */
  key: string | null;
  phase: "focus" | "break";
  paused: boolean;
  /** The planned time is up (whether or not the server has caught up). */
  ended: boolean;
  remainingMs: number;
  overflowMs: number;
  totalMs: number;
}

/** If the end is noticed later than this, it is not announced (the app was asleep or in the background). */
export const MISSED_AFTER_MS = 30_000;

export function nextAlerts(
  input: AlertInput | null,
  memory: AlertMemory,
  config: AlertConfig,
): { alerts: AlertKind[]; memory: AlertMemory } {
  if (input === null || input.key === null) return { alerts: [], memory: emptyMemory };

  const mem: AlertMemory = input.key === memory.key ? { ...memory } : { ...emptyMemory, key: input.key };
  const alerts: AlertKind[] = [];
  if (input.paused) return { alerts, memory: mem };

  const focus = input.phase === "focus";

  // Ending soon: only for phases comfortably longer than the warning, so a short phase does not start with a beep.
  if (
    config.warnLeadMs > 0 &&
    !mem.warned &&
    !input.ended &&
    input.remainingMs > 0 &&
    input.remainingMs <= config.warnLeadMs &&
    input.totalMs > config.warnLeadMs * 2
  ) {
    mem.warned = true;
    alerts.push("warn");
  }

  if (input.ended && !mem.endHandled) {
    mem.endHandled = true;
    mem.warned = true;
    if (input.overflowMs <= MISSED_AFTER_MS) {
      alerts.push(focus ? "focus_end" : "break_end");
    } else if (config.reminderMs > 0) {
      // Found long after the end: skip the reminders that have already come and gone.
      mem.reminders = Math.min(MAX_REMINDERS, Math.floor(input.overflowMs / config.reminderMs));
    } else {
      mem.reminders = MAX_REMINDERS;
    }
  }

  if (input.ended && config.reminderMs > 0 && mem.reminders < MAX_REMINDERS) {
    const due = Math.min(MAX_REMINDERS, Math.floor(input.overflowMs / config.reminderMs));
    if (due > mem.reminders) {
      mem.reminders = due;
      alerts.push(focus ? "focus_reminder" : "break_reminder"); // one sound, however many were missed
    }
  }

  return { alerts, memory: mem };
}
