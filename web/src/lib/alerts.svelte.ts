// Plays the alerts while the app is open: asks the pure planner what is due on every tick and does the sounds and
// notifications. Only the leader tab does this, so several open tabs make one sound, not several.
import { dailyReminderDue, inQuietHours } from "./alertPrefs";
import { emptyTickMemory, nextTick, type TickMemory } from "./ticking";
import { emptyNudgeMemory, nextNudge, type NudgeMemory } from "./nudge";
import { type AlertInput, type AlertMemory, emptyMemory, nextAlerts } from "./alertPlan";
import { dueSpecialBreak } from "./specialBreaks";
import { markAlerted, special } from "./specialBreaksStore.svelte";
import { app, currentView } from "./appState.svelte";
import { localDayKey } from "./days";
import { t } from "./i18n";
import type { AlertKind } from "./sounds";
import { clearCustomSounds, loadCustomSounds } from "../platform/customSounds.svelte";
import { session } from "./session.svelte";
import { leader } from "../platform/leader.svelte";
import { playAlert, playTick } from "../platform/audio";
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
    case "nudge":
      return { title: "", body: "" };
  }
}

export function startAlerts(): () => void {
  let memory: AlertMemory = emptyMemory;
  let nudgeMemory: NudgeMemory = emptyNudgeMemory;
  let tickMemory: TickMemory = emptyTickMemory;

  const stopEffect = $effect.root(() => {
    // The person's own sounds come with the account: load them on sign-in, drop them on sign-out.
    $effect(() => {
      const id = session.user?.id;
      if (id) void loadCustomSounds(id);
      else clearCustomSounds();
    });

    $effect(() => {
      const view = currentView();
      const timer = app.timer;
      const prefs = $state.snapshot(alertPrefs);

      // Not the leader: stay out of it, and forget what we knew, so that if we become the leader we treat
      // whatever is already over as old news instead of playing it late.
      if (!leader.isLeader || !view || !timer) {
        memory = emptyMemory;
        nudgeMemory = emptyNudgeMemory;
        tickMemory = emptyTickMemory;
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

      // The presence nudge: only while a focus session is counting down.
      const nudge = nextNudge(
        phase === "focus" && timer.start !== null
          ? {
              key: `${timer.start}:${timer.total}`,
              elapsedMs: view.totalMs - view.remainingMs,
              running: !view.paused && !view.ended && !view.overflow,
            }
          : null,
        nudgeMemory,
        prefs.nudgeMin * MIN,
      );
      nudgeMemory = nudge.memory;
      if (nudge.play && prefs.sound && !inQuietHours(new Date(), prefs)) playAlert("nudge", prefs.volume);

      // The optional tick, for focus and for breaks on their own.
      const wantsTick = phase === "focus" ? prefs.tickFocus : phase === "break" ? prefs.tickBreak : false;
      const tick = nextTick(
        wantsTick && timer.start !== null
          ? {
              key: `${timer.kind}:${timer.start}:${timer.total}`,
              elapsedMs: view.totalMs - view.remainingMs,
              running: !view.paused && !view.ended && !view.overflow,
            }
          : null,
        tickMemory,
      );
      tickMemory = tick.memory;
      if (tick.play && prefs.sound && !inQuietHours(new Date(), prefs)) playTick(prefs.volume);

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

  // The daily nudge: once a day, on the chosen days and time, if nothing is running. Checked twice a minute.
  const DAILY_KEY = "op_daily_last";
  const lastDaily = (): string | null => {
    try {
      return localStorage.getItem(DAILY_KEY);
    } catch {
      return null;
    }
  };
  const dailyTimer = setInterval(() => {
    const prefs = $state.snapshot(alertPrefs);
    const now = new Date();
    const view = currentView();
    if (!leader.isLeader || !view || view.phase !== "idle") return;
    if (!dailyReminderDue(now, prefs, lastDaily()) || inQuietHours(now, prefs)) return;
    try {
      localStorage.setItem(DAILY_KEY, localDayKey(now.getTime()));
    } catch {
      // Without storage it may show again after a reload within the grace minutes.
    }
    if (prefs.sound) playAlert("focus_reminder", prefs.volume);
    if (prefs.notify) void notify(t("alerts.daily.title"), t("alerts.daily.body"), "op-daily");
  }, 30_000);

  // A special break coming due: one sound and notification a day for each, if nothing is running.
  const specialTimer = setInterval(() => {
    const view = currentView();
    if (!leader.isLeader || !view || view.phase !== "idle") return;
    const now = new Date();
    const due = dueSpecialBreak(now, $state.snapshot(special.list), $state.snapshot(special.alerted));
    if (!due) return;
    markAlerted(due.id, now);
    const prefs = $state.snapshot(alertPrefs);
    if (inQuietHours(now, prefs)) return;
    const name = due.label || t("special.default");
    if (prefs.sound) playAlert("break_end", prefs.volume);
    if (prefs.notify) void notify(t("alerts.special.title", { name }), t("alerts.special.body", { minutes: due.minutes }), `op-special-${due.id}`);
  }, 30_000);

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
    clearInterval(dailyTimer);
    clearInterval(specialTimer);
    document.removeEventListener("visibilitychange", onVisibility);
  };
}
