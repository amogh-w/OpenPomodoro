<script lang="ts">
  import { app } from "./appState.svelte";
  import { t } from "./i18n";
  import { currentSlots, cycleDone, hall } from "./hallStore.svelte";
  import Medallion from "./Medallion.svelte";
  import { prefs } from "../platform/prefs.svelte";

  /** Rounds finished so far, the number in a cycle, and whether a focus round is under way. */
  let { rounds, total, running = false }: { rounds: number; total: number; running?: boolean } = $props();

  const current = $derived(hall());
  const done = $derived(app.timer ? cycleDone(current) : rounds % total);
</script>

{#if prefs.runes}
  <!-- The cycle as a medal being made: one socket for every session, filled as each one finishes. -->
  <Medallion slots={currentSlots(current, done)} {total} class="size-14" label={t("timer.round", { n: done + 1, total })} />
{:else}
  <div class="flex justify-center gap-1.5" role="img" aria-label={t("timer.round", { n: done + 1, total })}>
    {#each Array.from({ length: total }, (_, i) => i) as i (i)}
      <span
        class="size-2 rounded-full {i < done ? 'bg-accent' : running && i === done ? 'bg-accent/60 ring-2 ring-accent/30' : 'bg-surface2'}"
      ></span>
    {/each}
  </div>
{/if}
