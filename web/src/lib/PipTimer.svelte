<script lang="ts">
  import { app, currentView, sendCommand } from "./appState.svelte";
  import { formatElapsed, formatRemaining } from "./format";
  import { t } from "./i18n";
  import { registerPipCard } from "../platform/pip.svelte";
  import { phaseClass } from "./timerView";

  // The floating timer. It lives here, out of sight, until the picture-in-picture window takes it.
  const view = $derived(currentView());
  const STEP_MS = 5 * 60_000;
  let busy = $state(false);

  const label = $derived.by((): string => {
    if (!view) return "";
    if (view.phase === "break") return t("minibar.break");
    if (view.phase === "focus") return app.timer?.intention || t("timer.phase.focus");
    if (view.phase === "reflect") return t("timer.phase.reflect");
    return t("minibar.ready");
  });
  const running = $derived(view !== null && (view.phase === "focus" || view.phase === "break"));
  const clock = $derived.by((): string => {
    if (!view) return "";
    if (running) return view.overflow ? `+${formatElapsed(view.overflowMs)}` : formatRemaining(view.remainingMs);
    return formatRemaining(app.settings?.focus_ms ?? 25 * 60_000);
  });

  async function run(input: Parameters<typeof sendCommand>[0]): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      await sendCommand(input);
    } finally {
      busy = false;
    }
  }

  function main(): void {
    if (!view) return;
    if (view.phase === "idle") void run({ name: "init_focus" });
    else if (view.can.toggle_pause) void run({ name: "toggle_pause" });
  }

  function register(node: HTMLElement): { destroy(): void } {
    registerPipCard(node);
    return { destroy: () => registerPipCard(null) };
  }
</script>

<div class="hidden" aria-hidden="true">
  <div use:register class="grid min-h-dvh content-center justify-items-center gap-2 bg-mantle p-3 text-center text-text {view ? phaseClass(view.state) : ''}" role="group" aria-label={t("pip.label")}>
    <div class="max-w-full truncate text-subtext1">{label}</div>
    <div class="text-[2.75rem] font-bold leading-none tabular-nums">{clock}</div>
    <div class="flex items-center gap-3">
      <button class="btn btn-gray size-11 !px-0" aria-label={t("timer.minus5")} disabled={busy || !view?.can.extend} onclick={() => run({ name: "extend", delta_ms: -STEP_MS })}>
        <svg class="size-5"><use href="/icons.svg#minus" /></svg>
      </button>
      <button
        class="btn btn-filled size-12 !px-0"
        aria-label={view?.phase === "idle" ? t("timer.start") : view?.paused ? t("timer.resume") : t("timer.pause")}
        disabled={busy || !view || (view.phase !== "idle" && !view.can.toggle_pause)}
        onclick={main}
      >
        <svg class="size-6"><use href="/icons.svg#{view && running && !view.paused ? 'pause' : 'play'}" /></svg>
      </button>
      <button class="btn btn-gray size-11 !px-0" aria-label={t("timer.plus5")} disabled={busy || !view?.can.extend} onclick={() => run({ name: "extend", delta_ms: STEP_MS })}>
        <svg class="size-5"><use href="/icons.svg#plus" /></svg>
      </button>
    </div>
  </div>
</div>
