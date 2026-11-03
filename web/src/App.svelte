<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "./lib/i18n";
  import { app } from "./lib/appState.svelte";
  import { router } from "./lib/router.svelte";
  import { bootstrap, session } from "./lib/session.svelte";
  import { applyUpdate, sw } from "./platform/serviceWorker.svelte";
  import TabBar from "./lib/TabBar.svelte";
  import Admin from "./screens/Admin.svelte";
  import Auth from "./screens/Auth.svelte";
  import History from "./screens/History.svelte";
  import Settings from "./screens/Settings.svelte";
  import Timer from "./screens/Timer.svelte";

  onMount(bootstrap);
</script>

<div class="mx-auto min-h-dvh max-w-[480px] px-4 pb-24 pt-[max(1rem,env(safe-area-inset-top))]">
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
        <div>{t("common.offline")}</div>
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
{/if}
