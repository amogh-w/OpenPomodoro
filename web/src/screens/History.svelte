<script lang="ts">
  import { onMount } from "svelte";
  import AddSession from "../lib/AddSession.svelte";
  import { formatElapsed } from "../lib/format";
  import { t } from "../lib/i18n";
  import { recent, refreshRecent } from "../lib/recentStore.svelte";

  let add: AddSession | undefined = $state();

  onMount(() => void refreshRecent());

  const latest = $derived([...recent.sessions].sort((a, b) => b.started_at - a.started_at).slice(0, 20));
</script>

<div class="flex items-center justify-between gap-3">
  <h1 class="t-large">{t("history.title")}</h1>
  <button class="btn btn-gray btn-sm" onclick={() => add?.show()}>{t("history.add")}</button>
</div>

<section class="mt-4 grid gap-2" aria-label={t("history.recent")}>
  {#each latest as s (s.id)}
    <div class="card flex items-center justify-between gap-3 p-3">
      <div class="min-w-0">
        <div class="truncate">{s.intention ?? t(`timer.phase.${s.kind}`)}</div>
        <div class="t-foot text-subtext0">{new Date(s.started_at).toLocaleString()}</div>
      </div>
      <span class="tabular-nums text-subtext1">{formatElapsed(s.duration_ms ?? s.ended_at - s.started_at)}</span>
    </div>
  {:else}
    <p class="text-subtext0">{t("history.none")}</p>
  {/each}
</section>

<AddSession bind:this={add} />
