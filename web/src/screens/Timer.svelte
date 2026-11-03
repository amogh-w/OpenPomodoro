<script lang="ts">
  import Dial from "../lib/Dial.svelte";
  import { app, currentView, sendCommand } from "../lib/appState.svelte";
  import { formatElapsed, formatRemaining } from "../lib/format";
  import { t } from "../lib/i18n";
  import { parseIntention } from "../lib/intention";
  import { shouldSendTimerEnds } from "../lib/timerView";
  import type { CommandInput } from "../lib/types";

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

  async function run(input: CommandInput): Promise<void> {
    if (busy) return;
    busy = true;
    problem = null;
    try {
      const result = await sendCommand(input);
      if (!result.ok && result.code !== "network") problem = t("timer.error");
    } finally {
      busy = false;
    }
  }

  function start(event: SubmitEvent): void {
    event.preventDefault();
    void run({
      name: "init_focus",
      intention: parsed.text,
      category: chosenCategory ?? undefined,
      duration_ms: length * 60_000,
    });
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
    void sendCommand({ name: "timer_ends" });
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

{#if !app.ready || !view}
  <p class="mt-8 text-center text-subtext0" role="status">{t("common.loading")}</p>
{:else if view.phase === "idle"}
  <form class="grid gap-5" onsubmit={start}>
    <h1 class="t-large">{t("timer.title")}</h1>

    <label class="grid gap-1.5">
      <span class="t-foot text-subtext0">{t("timer.intention.label")}</span>
      <input
        class="field"
        bind:value={intention}
        placeholder={t("timer.intention.placeholder")}
        maxlength="200"
        autocomplete="off"
      />
    </label>

    {#if fromText || parsed.categoryId}
      <div class="flex flex-wrap gap-2" aria-live="polite">
        {#if fromText}<span class="chip" aria-pressed="true">{t("timer.hint.length", { minutes: length })}</span>{/if}
        {#if parsed.categoryId}
          <span class="chip" aria-pressed="true">{t("timer.hint.category", { name: categoryName(parsed.categoryId) ?? "" })}</span>
        {/if}
      </div>
    {/if}

    <Dial
      value={length}
      disabled={fromText}
      onchange={(next) => {
        minutes = next;
        touchedLength = true;
      }}
    />
    {#if fromText}<p class="t-foot -mt-3 text-center text-subtext0">{t("timer.dial.fromText")}</p>{/if}

    {#if app.categories.length > 0}
      <label class="grid gap-1.5">
        <span class="t-foot text-subtext0">{t("timer.category")}</span>
        <select class="field" bind:value={categoryId} disabled={parsed.categoryId !== null}>
          <option value="">{t("timer.category.none")}</option>
          {#each app.categories as category (category.id)}
            <option value={category.id}>{category.name}</option>
          {/each}
        </select>
      </label>
    {/if}

    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}

    <button class="btn btn-filled btn-lg w-full" type="submit" disabled={busy}>{t("timer.start")}</button>
    <p class="t-foot text-center text-subtext0">
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
  <div class="grid gap-5 pt-8 text-center">
    <h1 class="t-title1">{t("timer.phase.reflect")}</h1>
    <p class="text-subtext1">{t("timer.focused", { time: formatElapsed(app.timer?.focus_elapsed_ms ?? 0) })}</p>
    {#if app.timer?.intention}<p class="t-headline">{app.timer.intention}</p>{/if}
    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}
    <div class="grid gap-2">
      <button class="btn btn-filled btn-lg" disabled={busy || !view.can.take_rest} onclick={() => run({ name: "take_rest" })}>
        {t("timer.takeBreak")}
      </button>
      <button class="btn btn-gray" disabled={busy || !view.can.done} onclick={() => run({ name: "done" })}>
        {t("timer.done")}
      </button>
    </div>
  </div>
{:else}
  <div class="grid justify-items-center gap-5 pt-4 text-center">
    <h1 class="t-title2">{label()}</h1>

    <div class="relative size-64" role="timer" aria-label={t("timer.remaining")}>
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
        <div class="{clockText.length > 5 ? 'text-5xl' : 'text-6xl'} font-bold tabular-nums">{clockText}</div>
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
