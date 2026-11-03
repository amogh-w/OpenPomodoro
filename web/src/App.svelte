<script lang="ts">
  import { onMount } from "svelte";
  import { t, tn } from "./lib/i18n";
  import { startAlerts } from "./lib/alerts.svelte";
  import { app, currentView } from "./lib/appState.svelte";
  import { formatElapsed, formatRemaining, minutesLeft } from "./lib/format";
  import { phaseClass } from "./lib/timerView";
  import { prefs } from "./platform/prefs.svelte";
  import { installAudioUnlock } from "./platform/audio";
  import { setBadge } from "./platform/badge";
  import { holdAwake, releaseAwake } from "./platform/wakeLock";
  import { router } from "./lib/router.svelte";
  import { bootstrap, session } from "./lib/session.svelte";
  import { applyUpdate, sw } from "./platform/serviceWorker.svelte";
  import LinkHandler from "./lib/LinkHandler.svelte";
  import TabBar from "./lib/TabBar.svelte";
  import Admin from "./screens/Admin.svelte";
  import Auth from "./screens/Auth.svelte";
  import History from "./screens/History.svelte";
  import Settings from "./screens/Settings.svelte";
  import Timer from "./screens/Timer.svelte";

  onMount(bootstrap);

  // Sounds and notifications for the phases (only the leader tab acts), once the first tap has unlocked audio.
  onMount(() => {
    installAudioUnlock();
    return startAlerts();
  });

  const view = $derived(currentView());
  const tint = $derived(view ? phaseClass(view.state) : "");

  // Time left in the page title and on the app icon (A-08), so it shows when the tab is in the background.
  $effect(() => {
    const name = t("app.name");
    if (!view || view.phase === "idle" || view.phase === "reflect") {
      document.title = name;
    } else {
      const time = view.overflow ? `+${formatElapsed(view.overflowMs)}` : formatRemaining(view.remainingMs);
      document.title = `${time} · ${name}`;
    }
  });

  $effect(() => {
    const running = view && (view.phase === "focus" || view.phase === "break") && !view.paused && !view.overflow;
    setBadge(running ? minutesLeft(view.remainingMs) : null);
  });

  // Keep the screen on while a phase runs, if the user asked for it. The browser drops the lock when the
  // page is hidden, so it is taken again when the page comes back.
  let visible = $state(document.visibilityState === "visible");
  onMount(() => {
    const onChange = () => (visible = document.visibilityState === "visible");
    document.addEventListener("visibilitychange", onChange);
    return () => document.removeEventListener("visibilitychange", onChange);
  });
  $effect(() => {
    const active = view !== null && view.phase !== "idle" && view.phase !== "reflect" && !view.paused;
    if (prefs.keepAwake && active && visible) void holdAwake();
    else void releaseAwake();
  });
</script>

{#key tint}
  {#if tint}<div class="phase-layer {tint}" aria-hidden="true"></div>{/if}
{/key}

<div class="relative z-10 mx-auto min-h-dvh max-w-[480px] px-4 pb-24 pt-[max(1rem,env(safe-area-inset-top))]">
  {#if sw.updateReady}
    <div class="alert tone-info mb-3" role="status">
      <svg class="size-5 shrink-0"><use href="/icons.svg#info" /></svg>
      <div class="flex-1">{t("sw.updateReady")}</div>
      <button class="btn btn-filled btn-sm" onclick={applyUpdate}>{t("sw.updateNow")}</button>
    </div>
  {/if}

  {#if session.status === "loading"}
    <p class="mt-8 text-center text-subtext0" role="status">{t("common.loading")}</p>
  {:else if session.status !== "signed_in"}
    <main class="pt-8"><Auth /></main>
  {:else}
    {#if !app.online}
      <div class="alert tone-warn mb-3" role="status">
        <svg class="size-5 shrink-0"><use href="/icons.svg#triangle-alert" /></svg>
        <div>
          {t("common.offline")}
          {#if app.pending > 0}<div class="mt-1 font-semibold">{tn("common.pending", app.pending)}</div>{/if}
        </div>
      </div>
    {:else if app.pending > 0}
      <div class="alert tone-info mb-3" role="status">
        <svg class="size-5 shrink-0"><use href="/icons.svg#info" /></svg>
        <div>{tn("common.pending", app.pending)}</div>
      </div>
    {/if}
    {#if app.notice}
      <div class="alert tone-warn mb-3" role="status">
        <svg class="size-5 shrink-0"><use href="/icons.svg#triangle-alert" /></svg>
        <div class="flex-1">{app.notice}</div>
        <button class="btn btn-gray btn-sm" onclick={() => (app.notice = null)}>{t("common.dismiss")}</button>
      </div>
    {/if}
    <main>
      {#if router.route === "timer"}
        <Timer />
      {:else if router.route === "history"}
        <History />
      {:else if router.route === "admin" && session.user?.role === "admin"}
        <Admin />
      {:else}
        <Settings />
      {/if}
    </main>
  {/if}
</div>

{#if session.status === "signed_in"}
  <TabBar />
  <LinkHandler />
{/if}
