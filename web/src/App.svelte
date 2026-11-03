<script lang="ts">
  import { t } from "./lib/i18n";
  import { router } from "./lib/router.svelte";
  import { applyUpdate, sw } from "./platform/serviceWorker.svelte";
  import TabBar from "./lib/TabBar.svelte";
  import Timer from "./screens/Timer.svelte";
  import History from "./screens/History.svelte";
  import Settings from "./screens/Settings.svelte";
</script>

<div class="mx-auto min-h-dvh max-w-[480px] px-4 pb-24 pt-[max(1rem,env(safe-area-inset-top))]">
  {#if sw.updateReady}
    <div class="alert tone-info mb-3" role="status">
      <svg class="size-5 shrink-0"><use href="/icons.svg#info" /></svg>
      <div class="flex-1">{t("sw.updateReady")}</div>
      <button class="btn btn-filled btn-sm" onclick={applyUpdate}>{t("sw.updateNow")}</button>
    </div>
  {/if}

  <main>
    {#if router.route === "timer"}
      <Timer />
    {:else if router.route === "history"}
      <History />
    {:else}
      <Settings />
    {/if}
  </main>
</div>

<TabBar />
