<script lang="ts">
  import { onMount } from "svelte";
  import CategoryPicker from "../lib/CategoryPicker.svelte";
  import ConfirmDialog from "../lib/ConfirmDialog.svelte";
  import CycleDots from "../lib/CycleDots.svelte";
  import { cycleDone, glowOf, hall, slotsOf } from "../lib/hallStore.svelte";
  import Medallion from "../lib/Medallion.svelte";
  import Rune from "../lib/Rune.svelte";
  import { encouragement, glyphFor, litWord } from "../lib/runes";
  import Dial from "../lib/Dial.svelte";
  import EditSessionDialog from "../lib/EditSessionDialog.svelte";
  import NotesPane from "../lib/NotesPane.svelte";
  import NotesColumn from "../lib/NotesColumn.svelte";
  import TaskList from "../lib/TaskList.svelte";
  import TodayPanel from "../lib/TodayPanel.svelte";
  import { app, currentView, sendCommand, setTaskDone, tick } from "../lib/appState.svelte";
  import { attachDraft, clearDraft, draft } from "../lib/draft.svelte";
  import { formatElapsed, formatRemaining, formatSpan } from "../lib/format";
  import { t, type MessageKey } from "../lib/i18n";
  import { parseIntention } from "../lib/intention";
  import { planPreview } from "../lib/planPreview.svelte";
  import { recentIntentions } from "../lib/recent";
  import { actionFor } from "../lib/shortcuts";
  import { recent, refreshRecent } from "../lib/recentStore.svelte";
  import { dueSpecialBreak } from "../lib/specialBreaks";
  import { markHandled, special } from "../lib/specialBreaksStore.svelte";
  import { shouldSendTimerEnds } from "../lib/timerView";
  import { openPip, pip, pipSupported } from "../platform/pip.svelte";
  import { alertPrefs, keymap, prefs, updateAlertPrefs } from "../platform/prefs.svelte";
  import type { CommandInput, Task } from "../lib/types";

  type Mood = "focused" | "neutral" | "distracted";
  const moods: Mood[] = ["focused", "neutral", "distracted"];

  const STEP_MS = 5 * 60_000;
  const view = $derived(currentView());

  // The form for starting a session.
  let intention = $state("");
  // What the dial sets, in seconds (the dial shows minutes, or seconds when it snaps to 5 s).
  let focusSec = $state(25 * 60);
  let touchedLength = $state(false);
  let categoryId = $state("");
  // The task the next session works on (picked from the list); its title fills the intention.
  let taskId = $state<string | null>(null);
  const chosenTask = $derived(app.tasks.find((x) => x.id === taskId) ?? null);
  // On the reflect screen: tick the task off along with saving the session.
  let finishTask = $state(false);
  const sessionTask = $derived(app.tasks.find((x) => x.id === app.timer?.task && !x.done) ?? null);

  function pickTask(task: Task): void {
    taskId = task.id;
    intention = task.title;
    if (task.category) categoryId = task.category;
  }
  // What the dial sets: a focus session, or (turned counter-clockwise) a break.
  let mode = $state<"focus" | "break">("focus");
  let breakSec = $state(5 * 60);
  let touchedBreak = $state(false);
  const parsed = $derived(parseIntention(intention, app.categories));
  const fromText = $derived(parsed.durationMs !== null);
  const length = $derived(Math.round((parsed.durationMs ?? focusSec * 1000) / 60_000));
  const chosenCategory = $derived(parsed.categoryId ?? (categoryId || null));
  const categoryName = (id: string | null): string | undefined => app.categories.find((c) => c.id === id)?.name;

  // Until the user turns the dial, it shows their usual length: the chosen category's own if it has one, else the
  // general setting (and 5 seconds when the dial is set to count seconds, for trying the flow).
  $effect(() => {
    const own = app.categories.find((c) => c.id === chosenCategory);
    if (app.settings && !touchedLength) focusSec = prefs.dialStep < 60 ? 5 : (own?.focus_min ?? Math.round(app.settings.focus_ms / 60_000)) * 60;
    if (app.settings && !touchedBreak) breakSec = prefs.dialStep < 60 ? 5 : (own?.break_min ?? Math.round(app.settings.short_break_ms / 60_000)) * 60;
  });
  const isBreak = $derived(mode === "break");
  const secondsDial = $derived(prefs.dialStep < 60);
  const unitSec = $derived(secondsDial ? 1 : 60);
  const plannedMs = $derived(parsed.durationMs !== null && !isBreak ? parsed.durationMs : (isBreak ? breakSec : focusSec) * 1000);
  /** The dial's number: minutes, or seconds when it snaps to 5 s. */
  const dialValue = $derived(Math.max(1, Math.round(plannedMs / 1000 / unitSec)));
  const pad = (n: number): string => String(n).padStart(2, "0");
  const hhmm = (ms: number): string => {
    const d = new Date(ms);
    return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
  };
  /** The span the timer covers: when it started to when it should end (to now, once it runs over). */
  const range = $derived.by((): string => {
    const timer = app.timer;
    if (!view || view.phase === "idle" || view.phase === "reflect" || timer?.start == null || timer.total == null) {
      return `${hhmm(tick.now)} → ${hhmm(tick.now + plannedMs)}`;
    }
    const idle = timer.pause_ts != null ? tick.now - timer.pause_ts : 0;
    const end = view.overflow ? tick.now : timer.start + timer.total + (timer.pause_buffer ?? 0) + idle;
    return `${hhmm(timer.start)} → ${hhmm(end)}`;
  });
  const categoryOf = (id: string | null | undefined) => app.categories.find((c) => c.id === id);

  // Tell the day view beside the timer what is about to start, so it can show where it would land.
  $effect(() => {
    planPreview.plannedMs = view?.phase === "idle" && !isBreak ? plannedMs : null;
    return () => {
      planPreview.plannedMs = null;
    };
  });

  let busy = $state(false);
  let problem = $state<string | null>(null);

  /** Send a command and say whether it went through (queued while offline counts). */
  async function run(input: CommandInput): Promise<boolean> {
    if (busy) return false;
    busy = true;
    problem = null;
    try {
      const result = await sendCommand(input);
      if (!result.ok && result.code !== "network") problem = t("timer.error");
      return result.ok;
    } finally {
      busy = false;
    }
  }

  // Notes written during a focus session, and the mood picked afterwards, travel with the command that saves it.
  const draftStates = new Set(["session", "session_end", "paused_session", "paused_session_end", "reflect"]);
  let mood = $state<Mood | null>(null);
  const suggestions = $derived(recentIntentions(recent.sessions));

  onMount(() => void refreshRecent());

  // The draft belongs to the session in progress; it is dropped when that session is saved or abandoned.
  $effect(() => {
    const state = app.timer?.state;
    if (!app.ready || !state) return;
    if (draftStates.has(state) && app.timer?.focus_started_at != null) attachDraft(app.timer.focus_started_at);
    else clearDraft();
  });
  $effect(() => {
    if (view?.phase !== "reflect") mood = null;
  });

  /** Leave the reflect screen: save the session with its notes and mood, then take a break or finish. */
  async function endReflect(name: "take_rest" | "done"): Promise<void> {
    const notes = draft.notes.trim();
    const taskDone = finishTask && sessionTask !== null ? sessionTask.id : null;
    if (await run({ name, ...(notes !== "" ? { notes } : {}), ...(mood ? { mood } : {}) })) {
      if (taskDone) void setTaskDone(taskDone, true);
      finishTask = false;
      clearDraft();
      void refreshRecent();
    }
  }

  /**
   * Finish the focus session. With "ask how it went" on, that stops at the reflect screen; with it off the session
   * is saved straight away (then a break or another session follows if asked for).
   */
  async function finishFocus(then: "reflect" | "break" | "again" = "reflect"): Promise<void> {
    const intentionNow = app.timer?.intention ?? undefined;
    const categoryNow = app.timer?.category ?? undefined;
    const taskNow = app.timer?.task ?? undefined;
    const lengthNow = app.timer?.planned_ms ?? undefined;
    if (!(await run({ name: "stop" }))) return;
    if (then === "reflect" && prefs.askReflection) return;
    if (then === "break") return void (await endReflect("take_rest"));
    if (!(await run({ name: "done", ...(draft.notes.trim() !== "" ? { notes: draft.notes.trim() } : {}) }))) return;
    clearDraft();
    void refreshRecent();
    if (then === "again") {
      await run({
        name: "init_focus",
        ...(intentionNow ? { intention: intentionNow } : {}),
        ...(categoryNow ? { category: categoryNow } : {}),
        ...(taskNow ? { task: taskNow } : {}),
        ...(lengthNow ? { duration_ms: lengthNow } : {}),
      });
    }
  }

  // A special break (lunch, a snack) that is due now; offered only on the idle screen.
  const offered = $derived(
    view?.phase === "idle" ? dueSpecialBreak(new Date(tick.now), special.list, special.handled) : null,
  );

  async function startSpecial(id: string, minutes: number): Promise<void> {
    markHandled(id);
    await run({ name: "take_rest", duration_ms: minutes * 60_000 });
  }

  async function startBreak(): Promise<void> {
    await run({ name: "take_rest", duration_ms: plannedMs });
  }

  const flip = (): void => {
    mode = isBreak ? "focus" : "break";
  };

  const soundOn = $derived(alertPrefs.sound);

  async function startSession(): Promise<void> {
    const started = await run({
      name: "init_focus",
      intention: parsed.text,
      category: chosenCategory ?? undefined,
      ...(chosenTask ? { task: chosenTask.id } : {}),
      duration_ms: plannedMs,
    });
    // The next session starts with a blank field; the earlier text is one tap away in the suggestions.
    if (started) {
      intention = "";
      taskId = null;
    }
  }

  function start(event: SubmitEvent): void {
    event.preventDefault();
    void (isBreak ? startBreak() : startSession());
  }

  let intentionField: HTMLInputElement | undefined = $state();
  let picker: CategoryPicker | undefined = $state();
  let editDialog: EditSessionDialog | undefined = $state();
  let earlyDialog: ConfirmDialog | undefined = $state();
  let abandonDialog: ConfirmDialog | undefined = $state();
  let breakDialog: ConfirmDialog | undefined = $state();
  const cycleLength = $derived(app.settings?.rounds_before_long ?? 4);
  // The runes: where the cycle stands (counted from the stored sessions, like the hall), and what the session just
  // finished carved.
  const hallNow = $derived(hall());
  const cycleDoneNow = $derived(cycleDone(hallNow));
  const cycleMessage = $derived(encouragement(cycleDoneNow, cycleLength));
  /** How much of the session was done: how far its rune is lit. */
  const carvedLit = $derived.by((): number => {
    const planned = app.timer?.planned_ms ?? 0;
    return planned > 0 ? Math.min(1, (app.timer?.focus_elapsed_ms ?? 0) / planned) : 1;
  });
  const sessionGlyph = $derived(glyphFor(app.timer?.focus_started_at ?? 0));
  const sessionGlow = $derived(glowOf(app.timer?.category));
  /** This session is the one that completes the medallion (saving it fills the last socket). */
  const sealsMedallion = $derived(cycleDoneNow + 1 >= cycleLength);
  const roundNow = $derived(((app.timer?.rounds ?? 0) % cycleLength) + 1);

  /** Rest is part of the cycle too, so cutting a break short asks first. Once it has run over, it just ends. */
  async function leaveBreak(command: "skip_rest" | "init_focus"): Promise<void> {
    if (view && !view.overflow && !(await breakDialog?.ask())) return;
    await run({ name: command });
  }

  /** A full session is the point, so stopping before the time is up asks first. Past the end it is simply done. */
  async function endEarly(): Promise<void> {
    if (view && !view.overflow && !(await earlyDialog?.ask())) return;
    await finishFocus();
  }

  async function abandonFocus(): Promise<void> {
    if (await abandonDialog?.ask()) await run({ name: "abandon" });
  }

  /** Keyboard shortcuts for the timer; the rules about when they apply live in shortcuts.ts. */
  function onKey(event: KeyboardEvent): void {
    const action = actionFor(
      { key: event.key, ctrlKey: event.ctrlKey, metaKey: event.metaKey, altKey: event.altKey, target: event.target as HTMLElement | null },
      { dialogOpen: document.querySelector("dialog[open]") !== null },
      $state.snapshot(keymap),
    );
    if (action === null || !view) return;
    const act = (go: () => void): void => {
      event.preventDefault();
      go();
    };
    switch (action) {
      case "cancel":
        (document.activeElement as HTMLElement | null)?.blur();
        break;
      case "focus_input":
        if (view.phase === "idle") act(() => intentionField?.focus());
        break;
      case "toggle_pause":
        if (view.can.toggle_pause) act(() => void run({ name: "toggle_pause" }));
        break;
      case "start":
        if (view.can.init_focus) act(() => void (isBreak ? startBreak() : startSession()));
        break;
      case "extend":
      case "shrink":
        if (view.can.extend) act(() => void run({ name: "extend", delta_ms: action === "extend" ? STEP_MS : -STEP_MS }));
        break;
      case "finish":
        if (view.phase === "focus" && view.can.stop) act(() => void endEarly());
        break;
      case "break":
        if (view.phase === "reflect" && view.can.take_rest) act(() => void endReflect("take_rest"));
        else if (view.phase === "idle" && view.can.take_rest) act(() => void run({ name: "take_rest" }));
        break;
    }
  }

  // When the time is up the server has to be told once. A failed try waits a few seconds before the next.
  let endedKey = "";
  let lastTry = 0;
  $effect(() => {
    if (!view || !shouldSendTimerEnds(view)) return;
    const key = `${view.state}:${app.timer?.start}:${app.timer?.total}`;
    if (key === endedKey && Date.now() - lastTry < 3_000) return;
    endedKey = key;
    lastTry = Date.now();
    // If a break starts by itself the session is saved right now, so its notes go along.
    const notes = view.phase === "focus" ? draft.notes.trim() : "";
    void sendCommand({ name: "timer_ends", ...(notes !== "" ? { notes } : {}) }).then(() => {
      if (app.timer && !draftStates.has(app.timer.state)) {
        clearDraft();
        void refreshRecent();
      }
    });
  });

  const R = 88;
  const C = 2 * Math.PI * R;

  // Longer readings (overtime, hours) get a smaller size so they stay inside the ring.
  const clockText = $derived(
    !view ? "" : view.overflow ? `+${formatElapsed(view.overflowMs)}` : formatRemaining(view.remainingMs),
  );

  function label(): string {
    if (!view) return "";
    if (view.overflow) return view.phase === "focus" ? t("timer.phase.overtime") : t("timer.phase.breakOver");
    if (view.kind === "short_break") return t("timer.phase.short_break");
    if (view.kind === "long_break") return t("timer.phase.long_break");
    return t("timer.phase.focus");
  }
</script>

<svelte:window onkeydown={onKey} />
<EditSessionDialog bind:this={editDialog} />
<ConfirmDialog
  bind:this={earlyDialog}
  title={t("timer.endEarly.title")}
  body={t("timer.endEarly.body", { left: formatSpan(view?.remainingMs ?? 0), n: roundNow, total: cycleLength })}
  confirm={t("timer.endEarly.confirm")}
  cancel={t("timer.endEarly.keep")}
/>
<ConfirmDialog
  bind:this={breakDialog}
  title={t("timer.endBreakEarly.title")}
  body={t("timer.endBreakEarly.body", { left: formatSpan(view?.remainingMs ?? 0) })}
  confirm={t("timer.endBreakEarly.confirm")}
  cancel={t("timer.endBreakEarly.keep")}
/>
<ConfirmDialog
  bind:this={abandonDialog}
  title={t("timer.abandon.title")}
  body={t("timer.abandon.body")}
  confirm={t("timer.abandon.confirm")}
  cancel={t("timer.endEarly.keep")}
/>

<div class={prefs.compact ? "" : "lg:grid lg:grid-cols-[minmax(0,1fr)_18rem] lg:items-start lg:gap-8 xl:grid-cols-[minmax(0,1fr)_20rem] xl:gap-12"}>
<div>
{#if !app.ready || !view}
  <p class="mt-8 text-center text-subtext1" role="status">{t("common.loading")}</p>
{:else if view.phase === "idle"}
  {#if offered}
    <div class="card mx-auto mb-4 flex max-w-xl flex-wrap items-center justify-between gap-3 p-4" role="status">
      <span>{t("special.offer", { name: offered.label || t("special.default"), minutes: offered.minutes })}</span>
      <span class="flex gap-2">
        <button class="btn btn-gray" onclick={() => markHandled(offered.id)}>{t("special.dismiss")}</button>
        <button class="btn btn-filled" disabled={busy || !view.can.take_rest} onclick={() => startSpecial(offered.id, offered.minutes)}>
          {t("special.start")}
        </button>
      </span>
    </div>
  {/if}
  <form class="mx-auto grid max-w-xl gap-4" onsubmit={start}>
    <div class="relative flex min-h-11 items-center justify-center">
      {#if app.settings && !prefs.compact}<CycleDots rounds={app.timer?.rounds ?? 0} total={app.settings.rounds_before_long} />{/if}
      <button
        type="button"
        class="btn btn-gray absolute right-0 top-0 size-11 !px-0"
        aria-label={isBreak ? t("timer.toFocus") : t("timer.toBreak")}
        onclick={flip}
      >
        <svg class="size-5"><use href="/icons.svg#{isBreak ? 'timer' : 'coffee'}" /></svg>
      </button>
    </div>

    <h1 class="t-large text-center">{isBreak ? t("timer.prompt.break") : t("timer.prompt")}</h1>
    {#if prefs.runes && !isBreak && !prefs.compact}
      <p class="t-foot -mt-2 text-center text-subtext1 lg:hidden">{t(cycleMessage.key as MessageKey, cycleMessage.params)}</p>
    {/if}

    {#if !isBreak}
      <button
        type="button"
        class="chip mx-auto gap-2"
        aria-haspopup="dialog"
        disabled={parsed.categoryId !== null}
        onclick={() => picker?.open()}
      >
        {#if categoryOf(chosenCategory)}
          <span class="size-2.5 rounded-full" style="background: var(--ctp-{categoryOf(chosenCategory)?.color})" aria-hidden="true"></span>
          {categoryOf(chosenCategory)?.name}
        {:else}
          {t("timer.category.none")}
        {/if}
        <svg class="size-4"><use href="/icons.svg#chevron-down" /></svg>
      </button>

      <input
        class="field"
        bind:this={intentionField}
        bind:value={intention}
        placeholder={t("timer.intention.placeholder")}
        aria-label={t("timer.intention.label")}
        maxlength="200"
        autocomplete="off"
        list="recent-intentions"
      />
      <datalist id="recent-intentions">
        {#each suggestions as suggestion (suggestion)}<option value={suggestion}></option>{/each}
      </datalist>

      {#if chosenTask}
        <div class="flex items-center justify-center gap-2">
          <span class="chip" aria-pressed="true">{t("tasks.workingOn", { title: chosenTask.title })}</span>
          <button type="button" class="btn btn-plain size-11 !px-0" aria-label={t("tasks.clear")} onclick={() => (taskId = null)}>
            <svg class="size-5"><use href="/icons.svg#x" /></svg>
          </button>
        </div>
      {/if}

      {#if fromText || parsed.categoryId}
        <div class="flex flex-wrap justify-center gap-2" aria-live="polite">
          {#if fromText}<span class="chip" aria-pressed="true">{t("timer.hint.length", { minutes: length })}</span>{/if}
          {#if parsed.categoryId}
            <span class="chip" aria-pressed="true">{t("timer.hint.category", { name: categoryName(parsed.categoryId) ?? "" })}</span>
          {/if}
        </div>
      {/if}
    {/if}

    <div class="grid gap-3">
      <Dial
        class="lg:size-[min(21rem,38vh)]"
        value={dialValue}
        step={secondsDial ? prefs.dialStep : prefs.dialStep / 60}
        min={secondsDial ? prefs.dialStep : prefs.dialStep / 60}
        max={secondsDial ? 600 : 180}
        unit={secondsDial ? "sec" : "min"}
        tone={isBreak ? "green" : "accent"}
        disabled={!isBreak && fromText}
        onchange={(next) => {
          if (isBreak) {
            breakSec = next * unitSec;
            touchedBreak = true;
          } else {
            focusSec = next * unitSec;
            touchedLength = true;
          }
        }}
      />
      <div class="t-title1 text-center tabular-nums" aria-hidden="true">{formatRemaining(plannedMs)}</div>
      <div class="t-foot mx-auto rounded-full bg-surface0 px-3 py-1 tabular-nums text-subtext1" aria-label={t("timer.range")}>{range}</div>
      {#if !isBreak && fromText}<p class="t-foot text-center text-subtext1">{t("timer.dial.fromText")}</p>{/if}
    </div>

    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}

    <button class="btn btn-filled btn-lg mx-auto w-full max-w-xs" type="submit" disabled={busy}>
      {isBreak ? t("timer.startBreak") : t("timer.start")}
    </button>
    <p class="t-foot text-center text-subtext1">
      {t("timer.round", { n: roundNow, total: cycleLength })}
    </p>
  </form>
  <CategoryPicker
    bind:this={picker}
    value={categoryId}
    onpick={(id) => {
      categoryId = id;
      if (id !== "" && prefs.focusOnCategory) intentionField?.focus();
    }}
  />
  {#if !prefs.compact}
    <div class="mx-auto mt-5 max-w-xl"><TaskList selected={taskId} onpick={pickTask} /></div>
  {/if}
{:else if view.phase === "breathe"}
  <div class="grid justify-items-center gap-6 pt-8 text-center">
    <h1 class="t-title1">{t("timer.phase.breathe")}</h1>
    <div class="breathe-circle grid size-56 place-items-center rounded-full bg-sky/40" aria-hidden="true">
      <div class="size-32 rounded-full bg-sky/60"></div>
    </div>
    <p class="text-subtext1">{t("timer.breatheHint")}</p>
    <p class="t-title2 tabular-nums" aria-label={t("timer.remaining")}>{formatRemaining(view.remainingMs)}</p>
    <button class="btn btn-gray" disabled={busy || !view.can.skip_breathe} onclick={() => run({ name: "skip_breathe" })}>
      {t("timer.skipBreathe")}
    </button>
    {#if view.can.abandon}
      <button class="btn btn-plain" disabled={busy} onclick={() => run({ name: "abandon" })}>{t("timer.abandon")}</button>
    {/if}
  </div>
{:else if view.phase === "reflect"}
  <div class="mx-auto grid max-w-xl gap-5 pt-8 text-center">
    <h1 class="t-title1">{t("timer.phase.reflect")}</h1>
    <p class="text-subtext1">{t("timer.focused", { time: formatElapsed(app.timer?.focus_elapsed_ms ?? 0) })}</p>
    {#if prefs.runes}
      <!-- What this session carved: a rune lit as far as the time given, and the medallion it goes into. -->
      <div class="grid justify-items-center gap-2" role="status">
        {#if sealsMedallion}
          <Medallion
            slots={[...slotsOf(hallNow.cycle.runes, cycleDoneNow), { glyph: sessionGlyph, lit: carvedLit, glow: sessionGlow }]}
            total={cycleLength}
            class="size-28"
            label={t("runes.sealed", { total: cycleLength })}
          />
          <p class="t-headline">{t("runes.sealed", { total: cycleLength })}</p>
        {:else}
          <Rune glyph={sessionGlyph} lit={carvedLit} glow={sessionGlow} class="h-24 w-[4.8rem]" />
          <p class="text-subtext1">{t(`runes.carved.${litWord(carvedLit)}` as MessageKey)}</p>
          <p class="t-foot text-subtext1">{t(encouragement(cycleDoneNow + 1, cycleLength).key as MessageKey, encouragement(cycleDoneNow + 1, cycleLength).params)}</p>
        {/if}
      </div>
    {/if}
    {#if app.timer?.intention}<p class="t-headline">{app.timer.intention}</p>{/if}
    <div class="grid gap-2">
      <span class="t-foot text-subtext1">{t("timer.mood")}</span>
      <div class="flex flex-wrap justify-center gap-2" role="group" aria-label={t("timer.mood")}>
        {#each moods as option (option)}
          <button class="chip" aria-pressed={mood === option} onclick={() => (mood = mood === option ? null : option)}>
            {t(`mood.${option}`)}
          </button>
        {/each}
      </div>
    </div>
    {#if sessionTask}
      <label class="flex min-h-11 items-center justify-between gap-3 text-left">
        <span>{t("tasks.finishIt", { title: sessionTask.title })}</span>
        <span class="switch"><input type="checkbox" bind:checked={finishTask} /></span>
      </label>
    {/if}
    <NotesPane open={draft.notes !== ""} />
    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}
    <div class="grid gap-2">
      <button class="btn btn-filled btn-lg" disabled={busy || !view.can.take_rest} onclick={() => endReflect("take_rest")}>
        {t("timer.takeBreak")}
      </button>
      <button class="btn btn-gray" disabled={busy || !view.can.done} onclick={() => endReflect("done")}>
        {t("timer.done")}
      </button>
    </div>
  </div>
{:else}
  <div class="grid justify-items-center gap-5 pt-4 text-center">
    {#if app.settings}<CycleDots rounds={app.timer?.rounds ?? 0} total={app.settings.rounds_before_long} running={view.phase === "focus"} />{/if}

    {#if prefs.compact}
      <!-- the compact view keeps only the clock and the buttons -->
    {:else if view.phase === "focus"}
      <span class="chip" aria-pressed="true">
        {#if categoryOf(app.timer?.category)}
          <span class="size-2.5 rounded-full" style="background: var(--ctp-{categoryOf(app.timer?.category)?.color})" aria-hidden="true"></span>
          {categoryOf(app.timer?.category)?.name}
        {:else}
          {t("timer.category.none")}
        {/if}
      </span>
    {:else}
      <div class="t-foot text-subtext1">{t("timer.next")}{#if app.timer?.intention}: {app.timer.intention}{/if}</div>
    {/if}
    <h1 class="t-title2">{label()}</h1>
    {#if view.phase === "focus"}
      <button class="btn btn-plain btn-sm -mt-3" onclick={() => editDialog?.show()}>{t("timer.editSession")}</button>
    {/if}

    <div class="relative size-64 [container-type:inline-size] lg:size-[min(24rem,40vh)]" role="timer" aria-label={t("timer.remaining")}>
      <svg class="size-full -rotate-90" viewBox="0 0 200 200" aria-hidden="true">
        <circle cx="100" cy="100" r={R} fill="none" class="stroke-surface1" stroke-width="12" />
        <circle
          cx="100"
          cy="100"
          r={R}
          fill="none"
          class={view.overflow ? "stroke-yellow" : view.phase === "break" ? "stroke-green" : "stroke-red"}
          stroke-width="12"
          stroke-linecap="round"
          stroke-dasharray={C}
          stroke-dashoffset={view.overflow ? 0 : C * view.progress}
        />
      </svg>
      <div class="absolute inset-0 grid place-content-center justify-items-center gap-2">
        {#if prefs.runes && view.phase === "focus" && !prefs.compact}
          <!-- The rune for this session lights from the bottom as the time goes. -->
          <Rune glyph={sessionGlyph} lit={view.overflow ? 1 : view.progress} glow={sessionGlow} class="h-[21cqw] w-[16.8cqw]" />
        {/if}
        <div class="font-bold leading-none tabular-nums" style="font-size: {clockText.length > 5 ? 15 : 21}cqw">{clockText}</div>
        {#if view.paused}<div class="text-subtext1">{t("timer.phase.paused")}</div>{/if}
      </div>
    </div>

    {#if app.timer?.intention && view.phase === "focus"}
      <p class="t-headline max-w-full truncate">{app.timer.intention}</p>
    {/if}

    <div class="flex items-center justify-center gap-3">
      <button
        class="btn btn-gray size-11 !px-0"
        aria-label={t("timer.minus5")}
        disabled={busy || !view.can.extend}
        onclick={() => run({ name: "extend", delta_ms: -STEP_MS })}
      >
        <svg class="size-5"><use href="/icons.svg#minus" /></svg>
      </button>
      <div class="t-foot rounded-full bg-surface0 px-3 py-1.5 tabular-nums text-subtext1" aria-label={t("timer.range")}>{range}</div>
      <button
        class="btn btn-gray size-11 !px-0"
        aria-label={t("timer.plus5")}
        disabled={busy || !view.can.extend}
        onclick={() => run({ name: "extend", delta_ms: STEP_MS })}
      >
        <svg class="size-5"><use href="/icons.svg#plus" /></svg>
      </button>
    </div>

    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}

    {#if view.phase === "focus"}<div class="w-full lg:hidden"><NotesPane /></div>{/if}

    <!-- Three round buttons for every running state: sound, the main action, and a stop menu. -->
    <div class="flex items-center justify-center gap-4">
      <button
        class="btn btn-gray size-14 !px-0"
        aria-label={soundOn ? t("timer.soundOff") : t("timer.soundOn")}
        aria-pressed={!soundOn}
        onclick={() => updateAlertPrefs({ sound: !soundOn })}
      >
        <svg class="size-6"><use href="/icons.svg#{soundOn ? 'volume-2' : 'volume-x'}" /></svg>
      </button>

      {#if view.phase === "focus" && view.overflow}
        <button class="btn btn-gray size-14 !px-0" aria-label={t("timer.again")} title={t("timer.again")} disabled={busy} onclick={() => finishFocus("again")}>
          <svg class="size-6"><use href="/icons.svg#repeat" /></svg>
        </button>
        <button class="btn btn-filled size-16 !px-0" aria-label={t("timer.takeBreak")} title={t("timer.takeBreak")} disabled={busy} onclick={() => finishFocus("break")}>
          <svg class="size-7"><use href="/icons.svg#coffee" /></svg>
        </button>
      {:else}
        <button
          class="btn btn-filled size-16 !px-0"
          aria-label={view.paused ? t("timer.resume") : t("timer.pause")}
          disabled={busy || !view.can.toggle_pause}
          onclick={() => run({ name: "toggle_pause" })}
        >
          <svg class="size-7 fill-current"><use href="/icons.svg#{view.paused ? 'play' : 'pause'}" /></svg>
        </button>
      {/if}

      {#if view.phase === "focus"}
        <button class="btn btn-gray size-14 !px-0" aria-label={t("timer.stop")} title={t("timer.stop")} data-menu="stop-menu" disabled={busy}>
          <svg class="size-6"><use href="/icons.svg#square" /></svg>
        </button>
        <div id="stop-menu" popover class="menu">
          <button class="menu-item" disabled={!view.can.stop} data-menu-close onclick={endEarly}>
            <svg class="size-5 text-subtext0"><use href="/icons.svg#circle-check" /></svg>{t("timer.finishSession")}
          </button>
          {#if view.can.abandon}
            <div class="menu-sep"></div>
            <button class="menu-item text-red" data-menu-close onclick={abandonFocus}>
              <svg class="size-5"><use href="/icons.svg#trash-2" /></svg>{t("timer.abandonSession")}
            </button>
          {/if}
        </div>
      {:else}
        <button
          class="btn btn-gray size-14 !px-0"
          aria-label={t("timer.endBreak")}
          title={t("timer.endBreak")}
          disabled={busy || !view.can.skip_rest}
          onclick={() => leaveBreak("skip_rest")}
        >
          <svg class="size-6"><use href="/icons.svg#square" /></svg>
        </button>
      {/if}
    </div>

    {#if view.phase === "break" && view.can.init_focus}
      <button class="btn btn-filled" disabled={busy} onclick={() => leaveBreak("init_focus")}>{t("timer.startFocus")}</button>
    {/if}

    {#if pipSupported() && !pip.open}
      <button class="btn btn-plain btn-sm" onclick={() => void openPip()}>
        <svg class="size-4"><use href="/icons.svg#timer" /></svg>{t("pip.open")}
      </button>
    {/if}
  </div>
{/if}
</div>
{#if !prefs.compact}
<aside class="hidden lg:sticky lg:top-6 lg:block" aria-label={t("today.title")}>
  {#if view?.phase === "focus"}<NotesColumn />{:else}<TodayPanel />{/if}
</aside>
{/if}
</div>
