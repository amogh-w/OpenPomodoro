// Plays the alerts while the app is open: asks the pure planner what is due on every tick and does the sounds and
// notifications. Only the leader tab does this, so several open tabs make one sound, not several.
import { inQuietHours } from "./alertPrefs";
import { type AlertInput, type AlertMemory, emptyMemory, nextAlerts } from "./alertPlan";
import { app, currentView } from "./appState.svelte";
import { t } from "./i18n";
import type { AlertKind } from "./sounds";
import { leader } from "../platform/leader.svelte";
import { playAlert } from "../platform/audio";
import { notify } from "../platform/notify";
import { alertPrefs } from "../platform/prefs.svelte";

const MIN = 60_000;
/** After being away this long, tell the person if a phase ended in the meantime. */
const AWAY_MS = 30_000;

function describe(kind: AlertKind, intention: string | null, overflowMs: number): { title: string; body: string } {
  const minutes = Math.max(1, Math.round(overflowMs / MIN));
  switch (kind) {
    case "focus_end":
      return { title: t("alerts.focusEnd.title"), body: intention ?? t("alerts.focusEnd.body") };
    case "break_end":
      return { title: t("alerts.breakEnd.title"), body: t("alerts.breakEnd.body") };
    case "focus_reminder":
      return { title: t("alerts.reminder.title"), body: t("alerts.reminder.focus", { minutes }) };
    case "break_reminder":
      return { title: t("alerts.reminder.title"), body: t("alerts.reminder.break", { minutes }) };
    case "warn":
      return { title: "", body: "" };
  }
}

export function startAlerts(): () => void {
  let memory: AlertMemory = emptyMemory;

  const stopEffect = $effect.root(() => {
    $effect(() => {
      const view = currentView();
      const timer = app.timer;
      const prefs = $state.snapshot(alertPrefs);

      // Not the leader: stay out of it, and forget what we knew, so that if we become the leader we treat
      // whatever is already over as old news instead of playing it late.
      if (!leader.isLeader || !view || !timer) {
        memory = emptyMemory;
        return;
      }

      const phase = view.phase === "focus" ? "focus" : view.phase === "break" ? "break" : null;
      const input: AlertInput | null =
        phase !== null && timer.start !== null
          ? {
              key: `${timer.kind}:${timer.start}:${timer.total}`,
              phase,
              paused: view.paused,
              ended: view.ended || view.overflow,
              remainingMs: view.remainingMs,
              overflowMs: view.overflowMs,
              totalMs: view.totalMs,
            }
          : null;

      const planned = nextAlerts(input, memory, { warnLeadMs: prefs.warnMin * MIN, reminderMs: prefs.reminderMin * MIN });
      memory = planned.memory;
      if (planned.alerts.length === 0) return;

      const quiet = inQuietHours(new Date(), prefs);
      for (const kind of planned.alerts) {
        if (quiet) continue;
        if (prefs.sound) playAlert(kind, prefs.volume);
        if (prefs.notify && kind !== "warn") {
          const { title, body } = describe(kind, timer.intention, view.overflowMs);
          void notify(title, body, `op-${kind}`);
        }
      }
    });
  });

  // Coming back after a while: say if a phase ended while we were not looking (the sound was skipped on purpose).
  let hiddenAt = 0;
  const onVisibility = (): void => {
    if (document.visibilityState === "hidden") {
      hiddenAt = Date.now();
      return;
    }
    const away = hiddenAt > 0 && Date.now() - hiddenAt > AWAY_MS;
    hiddenAt = 0;
    if (!away) return;
    setTimeout(() => {
      const view = currentView();
      if (view && view.overflow && view.overflowMs > AWAY_MS && (view.phase === "focus" || view.phase === "break") && app.notice === null) {
        app.notice = t("alerts.endedAgo", {
          what: view.phase === "focus" ? t("timer.phase.focus") : t("alerts.breakWord"),
          minutes: Math.max(1, Math.floor(view.overflowMs / MIN)),
        });
      }
    }, 1500); // let the refresh that follows waking land first
  };
  document.addEventListener("visibilitychange", onVisibility);

  return () => {
    stopEffect();
    document.removeEventListener("visibilitychange", onVisibility);
  };
}
