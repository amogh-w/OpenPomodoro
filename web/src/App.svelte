<script lang="ts">
  import { onMount } from "svelte";
  import { t, tn } from "./lib/i18n";
  import { startAlerts } from "./lib/alerts.svelte";
  import { app, currentView, setTickRate } from "./lib/appState.svelte";
  import { activeLanguage } from "./lib/language.svelte";
  import { tickInterval } from "./lib/motion";
  import { formatElapsed, formatRemaining, minutesLeft } from "./lib/format";
  import { phaseClass } from "./lib/timerView";
  import { prefs, setCompact } from "./platform/prefs.svelte";
  import { installAudioUnlock } from "./platform/audio";
  import { setBadge } from "./platform/badge";
  import { holdAwake, releaseAwake } from "./platform/wakeLock";
  import { router } from "./lib/router.svelte";
  import { bootstrap, session } from "./lib/session.svelte";
  import { applyUpdate, sw } from "./platform/serviceWorker.svelte";
  import Announcer from "./lib/Announcer.svelte";
  import LinkHandler from "./lib/LinkHandler.svelte";
  import MiniBar from "./lib/MiniBar.svelte";
  import PipTimer from "./lib/PipTimer.svelte";
  import TabBar from "./lib/TabBar.svelte";
  import Admin from "./screens/Admin.svelte";
  import Auth from "./screens/Auth.svelte";
  import Profile from "./screens/Profile.svelte";
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

  // "Reduce motion": a class that switches animations off, and a slower clock to save power.
  $effect(() => {
    document.documentElement.classList.toggle("reduce-motion", prefs.reduceMotion);
    setTickRate(tickInterval(prefs.reduceMotion));
  });
  // Compact view only applies on the timer screen; the other screens always use the full layout.
  const compact = $derived(prefs.compact && router.route === "timer" && session.status === "signed_in");
  // The page's declared language follows the app's, for screen readers and spell checking.
  $effect(() => {
    document.documentElement.lang = activeLanguage();
  });
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

<div class="relative z-10 {session.status === 'signed_in' && !compact ? 'lg:pl-[4.5rem]' : ''}">
<div
  class="mx-auto min-h-dvh px-4 pt-[max(1rem,env(safe-area-inset-top))] {compact
    ? 'max-w-[22rem] pb-6'
    : `max-w-[480px] md:max-w-xl lg:max-w-5xl lg:px-10 lg:pt-10 ${router.route === 'timer' ? 'pb-24 lg:pb-12' : 'pb-40 lg:pb-28'}`}"
>
  {#if sw.updateReady}
    <div class="alert tone-info mb-3" role="status">
      <svg class="size-5 shrink-0"><use href="/icons.svg#info" /></svg>
      <div class="flex-1">{t("sw.updateReady")}</div>
      <button class="btn btn-filled btn-sm" onclick={applyUpdate}>{t("sw.updateNow")}</button>
    </div>
  {/if}

  {#if sw.offlineReady}
    <div class="alert tone-success mb-3" role="status">
      <svg class="size-5 shrink-0"><use href="/icons.svg#circle-check" /></svg>
      <div class="flex-1">{t("sw.offlineReady")}</div>
      <button class="btn btn-gray btn-sm" onclick={() => (sw.offlineReady = false)}>{t("common.dismiss")}</button>
    </div>
  {/if}

  {#if session.status === "loading"}
    <p class="mt-8 text-center text-subtext1" role="status">{t("common.loading")}</p>
  {:else if session.status !== "signed_in"}
    <main class="mx-auto max-w-sm pt-8 lg:pt-24"><Auth /></main>
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
      <div id="screen" role="tabpanel" aria-labelledby="tab-{router.route === 'admin' ? 'profile' : router.route}">
        {#if router.route === "timer"}
          <Timer />
        {:else if router.route === "history"}
          <History />
        {:else if router.route === "profile"}
          <Profile />
        {:else if router.route === "admin" && session.user?.role === "admin"}
          <Admin />
        {:else}
          <Settings />
        {/if}
      </div>
    </main>
  {/if}
</div>
</div>

{#if session.status === "signed_in"}
  <Announcer />
  <PipTimer />
  {#if router.route === "timer"}
    <button
      class="btn btn-gray fixed right-3 top-[max(0.75rem,env(safe-area-inset-top))] z-30 size-11 !px-0"
      aria-label={compact ? t("compact.off") : t("compact.on")}
      onclick={() => setCompact(!prefs.compact)}
    >
      <svg class="size-5"><use href="/icons.svg#{compact ? 'maximize-2' : 'minimize-2'}" /></svg>
    </button>
  {/if}
  {#if !compact}<TabBar />{/if}
  {#if router.route !== "timer"}<MiniBar />{/if}
  <LinkHandler />
{/if}
