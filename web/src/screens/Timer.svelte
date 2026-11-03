<script lang="ts">
  import { onMount } from "svelte";
  import Dial from "../lib/Dial.svelte";
  import NotesPane from "../lib/NotesPane.svelte";
  import NotesColumn from "../lib/NotesColumn.svelte";
  import TodayPanel from "../lib/TodayPanel.svelte";
  import { app, currentView, sendCommand } from "../lib/appState.svelte";
  import { attachDraft, clearDraft, draft } from "../lib/draft.svelte";
  import { formatElapsed, formatRemaining } from "../lib/format";
  import { t } from "../lib/i18n";
  import { parseIntention } from "../lib/intention";
  import { recentIntentions } from "../lib/recent";
  import { actionFor } from "../lib/shortcuts";
  import { recent, refreshRecent } from "../lib/recentStore.svelte";
  import { shouldSendTimerEnds } from "../lib/timerView";
  import type { CommandInput } from "../lib/types";

  type Mood = "focused" | "neutral" | "distracted";
  const moods: Mood[] = ["focused", "neutral", "distracted"];

  const STEP_MS = 5 * 60_000;
  const view = $derived(currentView());

  // The form for starting a session.
  let intention = $state("");
  let minutes = $state(25);
  let touchedLength = $state(false);
  let categoryId = $state("");
  const parsed = $derived(parseIntention(intention, app.categories));
  const fromText = $derived(parsed.durationMs !== null);
  const length = $derived(parsed.durationMs !== null ? Math.round(parsed.durationMs / 60_000) : minutes);
  const chosenCategory = $derived(parsed.categoryId ?? (categoryId || null));
  const categoryName = (id: string | null): string | undefined => app.categories.find((c) => c.id === id)?.name;

  // Until the user turns the dial, it shows their usual focus length.
  $effect(() => {
    if (app.settings && !touchedLength) minutes = Math.round(app.settings.focus_ms / 60_000);
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
    if (await run({ name, ...(notes !== "" ? { notes } : {}), ...(mood ? { mood } : {}) })) {
      clearDraft();
      void refreshRecent();
    }
  }

  async function startSession(): Promise<void> {
    const started = await run({
      name: "init_focus",
      intention: parsed.text,
      category: chosenCategory ?? undefined,
      duration_ms: length * 60_000,
    });
    // The next session starts with a blank field; the earlier text is one tap away in the suggestions.
    if (started) intention = "";
  }

  function start(event: SubmitEvent): void {
    event.preventDefault();
    void startSession();
  }

  let intentionField: HTMLInputElement | undefined = $state();

  /** Keyboard shortcuts for the timer; the rules about when they apply live in shortcuts.ts. */
  function onKey(event: KeyboardEvent): void {
    const action = actionFor(
      { key: event.key, ctrlKey: event.ctrlKey, metaKey: event.metaKey, altKey: event.altKey, target: event.target as HTMLElement | null },
      { dialogOpen: document.querySelector("dialog[open]") !== null },
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
        if (view.can.init_focus) act(() => void startSession());
        break;
      case "extend":
      case "shrink":
        if (view.can.extend) act(() => void run({ name: "extend", delta_ms: action === "extend" ? STEP_MS : -STEP_MS }));
        break;
      case "finish":
        if (view.phase === "focus" && view.can.stop) act(() => void run({ name: "stop" }));
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

<div class="xl:grid xl:grid-cols-[minmax(0,1fr)_19rem] xl:items-start xl:gap-10">
<div>
{#if !app.ready || !view}
  <p class="mt-8 text-center text-subtext1" role="status">{t("common.loading")}</p>
{:else if view.phase === "idle"}
  <form class="mx-auto grid max-w-xl gap-5 2xl:max-w-none 2xl:grid-cols-[minmax(0,1fr)_minmax(0,20rem)] 2xl:gap-x-10" onsubmit={start}>
    <h1 class="t-large 2xl:col-span-2">{t("timer.title")}</h1>

    <label class="grid gap-1.5 2xl:col-start-2 2xl:row-start-2">
      <span class="t-foot text-subtext1">{t("timer.intention.label")}</span>
      <input
        class="field"
        bind:this={intentionField}
        bind:value={intention}
        placeholder={t("timer.intention.placeholder")}
        maxlength="200"
        autocomplete="off"
        list="recent-intentions"
      />
      <datalist id="recent-intentions">
        {#each suggestions as suggestion (suggestion)}<option value={suggestion}></option>{/each}
      </datalist>
    </label>

    {#if fromText || parsed.categoryId}
      <div class="flex flex-wrap gap-2 2xl:col-start-2" aria-live="polite">
        {#if fromText}<span class="chip" aria-pressed="true">{t("timer.hint.length", { minutes: length })}</span>{/if}
        {#if parsed.categoryId}
          <span class="chip" aria-pressed="true">{t("timer.hint.category", { name: categoryName(parsed.categoryId) ?? "" })}</span>
        {/if}
      </div>
    {/if}

    <div class="grid gap-3 2xl:col-start-1 2xl:row-span-6 2xl:row-start-2 2xl:content-center">
    <Dial
      class="lg:size-80"
      value={length}
      disabled={fromText}
      onchange={(next) => {
        minutes = next;
        touchedLength = true;
      }}
    />
    {#if fromText}<p class="t-foot text-center text-subtext1">{t("timer.dial.fromText")}</p>{/if}
    </div>

    {#if app.categories.length > 0}
      <label class="grid gap-1.5 2xl:col-start-2">
        <span class="t-foot text-subtext1">{t("timer.category")}</span>
        <select class="field" bind:value={categoryId} disabled={parsed.categoryId !== null}>
          <option value="">{t("timer.category.none")}</option>
          {#each app.categories as category (category.id)}
            <option value={category.id}>{category.name}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if problem}<div class="alert tone-danger 2xl:col-start-2" role="alert">{problem}</div>{/if}

    <button class="btn btn-filled btn-lg w-full 2xl:col-start-2" type="submit" disabled={busy}>{t("timer.start")}</button>
    <p class="t-foot text-center text-subtext1 2xl:col-start-2">
      {t("timer.round", { n: (app.timer?.rounds ?? 0) + 1, total: app.settings?.rounds_before_long ?? 4 })}
    </p>
  </form>
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
    <h1 class="t-title2">{label()}</h1>

    <div class="relative size-64 lg:size-96" role="timer" aria-label={t("timer.remaining")}>
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
      <div class="absolute inset-0 grid place-content-center gap-1">
        <div class="{clockText.length > 5 ? 'text-5xl lg:text-7xl' : 'text-6xl lg:text-8xl'} font-bold tabular-nums">{clockText}</div>
        {#if view.paused}<div class="text-subtext1">{t("timer.phase.paused")}</div>{/if}
      </div>
    </div>

    {#if app.timer?.intention && view.phase === "focus"}
      <p class="t-headline max-w-full truncate">{app.timer.intention}</p>
    {/if}

    <div class="flex items-center justify-center gap-3">
      <button
        class="btn btn-gray size-14 !px-0"
        aria-label={t("timer.minus5")}
        disabled={busy || !view.can.extend}
        onclick={() => run({ name: "extend", delta_ms: -STEP_MS })}
      >
        <svg class="size-6"><use href="/icons.svg#minus" /></svg>
      </button>
      <button
        class="btn btn-filled size-16 !px-0"
        aria-label={view.paused ? t("timer.resume") : t("timer.pause")}
        disabled={busy || !view.can.toggle_pause}
        onclick={() => run({ name: "toggle_pause" })}
      >
        <svg class="size-7 fill-current"><use href="/icons.svg#{view.paused ? 'play' : 'pause'}" /></svg>
      </button>
      <button
        class="btn btn-gray size-14 !px-0"
        aria-label={t("timer.plus5")}
        disabled={busy || !view.can.extend}
        onclick={() => run({ name: "extend", delta_ms: STEP_MS })}
      >
        <svg class="size-6"><use href="/icons.svg#plus" /></svg>
      </button>
    </div>

    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}

    {#if view.phase === "focus"}<div class="w-full xl:hidden"><NotesPane /></div>{/if}

    <div class="flex flex-wrap items-center justify-center gap-2">
      {#if view.phase === "focus"}
        <button class="btn btn-gray" disabled={busy || !view.can.stop} onclick={() => run({ name: "stop" })}>
          {t("timer.finish")}
        </button>
        {#if view.can.abandon}
          <button class="btn btn-plain" disabled={busy} onclick={() => run({ name: "abandon" })}>{t("timer.abandon")}</button>
        {/if}
      {:else}
        {#if view.can.init_focus}
          <button class="btn btn-filled" disabled={busy} onclick={() => run({ name: "init_focus" })}>
            {t("timer.startFocus")}
          </button>
        {/if}
        <button class="btn btn-gray" disabled={busy || !view.can.skip_rest} onclick={() => run({ name: "skip_rest" })}>
          {t("timer.endBreak")}
        </button>
      {/if}
    </div>
  </div>
{/if}
</div>
<aside class="hidden xl:block" aria-label={t("today.title")}>
  {#if view?.phase === "focus"}<NotesColumn />{:else}<TodayPanel />{/if}
</aside>
</div>
