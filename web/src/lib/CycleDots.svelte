<script lang="ts">
  import { t } from "./i18n";

  /** Rounds finished so far, the number in a cycle, and whether a focus round is under way. */
  let { rounds, total, running = false }: { rounds: number; total: number; running?: boolean } = $props();

  const done = $derived(rounds % total);
</script>

<div class="flex justify-center gap-1.5" role="img" aria-label={t("timer.round", { n: done + 1, total })}>
  {#each Array.from({ length: total }, (_, i) => i) as i (i)}
    <span
      class="size-2 rounded-full {i < done ? 'bg-accent' : running && i === done ? 'bg-accent/60 ring-2 ring-accent/30' : 'bg-surface2'}"
    ></span>
  {/each}
</div>
