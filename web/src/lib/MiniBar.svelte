<script lang="ts">
  import { app, currentView, sendCommand } from "./appState.svelte";
  import { formatElapsed, formatRemaining } from "./format";
  import { t } from "./i18n";
  import { navigate } from "./router.svelte";

  // A small player along the bottom of every screen except the timer: what is running, and the few controls you
  // reach for without leaving the page.
  const view = $derived(currentView());
  const STEP_MS = 5 * 60_000;
  let busy = $state(false);

  const running = $derived(view !== null && (view.phase === "focus" || view.phase === "break"));
  const category = $derived(app.categories.find((c) => c.id === app.timer?.category));
  const label = $derived.by((): string => {
    if (!view) return "";
    if (view.phase === "break") return t("minibar.break");
    if (view.phase === "focus") return app.timer?.intention || (category?.name ?? t("timer.phase.focus"));
    if (view.phase === "reflect") return t("timer.phase.reflect");
    return t("minibar.ready");
  });
  const clock = $derived.by((): string => {
    if (!view) return "";
    if (running) return view.overflow ? `+${formatElapsed(view.overflowMs)}` : formatRemaining(view.remainingMs);
    return formatRemaining(app.settings?.focus_ms ?? 25 * 60_000);
  });
  const playing = $derived(running && !view?.paused);

  async function run(input: Parameters<typeof sendCommand>[0]): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      await sendCommand(input);
    } finally {
      busy = false;
    }
  }

  /** Start focus when idle, pause or resume while something runs, and go to the timer for the reflect screen. */
  function main(): void {
    if (!view) return;
    if (view.phase === "reflect") return navigate("timer");
    if (view.phase === "idle") void run({ name: "init_focus" });
    else if (view.can.toggle_pause) void run({ name: "toggle_pause" });
  }
</script>

{#if view}
  <div
    class="fixed inset-x-0 bottom-[calc(4.75rem+env(safe-area-inset-bottom))] z-20 mx-auto flex max-w-[480px] items-center gap-3 border-t border-surface0 bg-mantle px-4 py-2 md:max-w-xl lg:bottom-0 lg:left-[4.5rem] lg:max-w-none lg:px-10"
    role="group"
    aria-label={t("minibar.label")}
  >
    <button class="flex min-w-0 flex-1 items-center gap-2 text-left" onclick={() => navigate("timer")} aria-label={t("minibar.open")}>
      <span class="size-2.5 shrink-0 rounded-full" style="background: {category ? `var(--ctp-${category.color})` : 'var(--ctp-surface2)'}" aria-hidden="true"></span>
      <span class="truncate">{label}</span>
    </button>
    <span class="t-title3 tabular-nums" aria-live="off">{clock}</span>
    <button class="btn btn-plain size-11 !px-0" aria-label={t("timer.minus5")} disabled={busy || !view.can.extend} onclick={() => run({ name: "extend", delta_ms: -STEP_MS })}>
      <svg class="size-5"><use href="/icons.svg#minus" /></svg>
    </button>
    <button
      class="btn btn-filled size-11 !px-0"
      aria-label={playing ? t("timer.pause") : view.phase === "idle" ? t("timer.start") : t("timer.resume")}
      disabled={busy || (view.phase !== "idle" && view.phase !== "reflect" && !view.can.toggle_pause)}
      onclick={main}
    >
      <svg class="size-5"><use href="/icons.svg#{playing ? 'pause' : 'play'}" /></svg>
    </button>
    <button class="btn btn-plain size-11 !px-0" aria-label={t("timer.plus5")} disabled={busy || !view.can.extend} onclick={() => run({ name: "extend", delta_ms: STEP_MS })}>
      <svg class="size-5"><use href="/icons.svg#plus" /></svg>
    </button>
  </div>
{/if}
