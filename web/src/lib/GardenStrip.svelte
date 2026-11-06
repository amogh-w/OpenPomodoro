<script lang="ts">
  import { app } from "./appState.svelte";
  import { buildGarden, encouragement } from "./garden";
  import { t, type MessageKey } from "./i18n";
  import Plant from "./Plant.svelte";
  import { recent } from "./recentStore.svelte";
  import Tree from "./Tree.svelte";
  import { prefs } from "../platform/prefs.svelte";

  // A line of the garden for the day panel: the tree being grown and the plants of the day shown.
  let { day }: { day: string } = $props();

  const total = $derived(app.settings?.rounds_before_long ?? 4);
  const garden = $derived(buildGarden(recent.sessions, total));
  const done = $derived((app.timer?.rounds ?? 0) % total);
  const message = $derived(encouragement(done, total));
  const today = $derived(garden.plants.filter((p) => p.day === day));
  const colorOf = (id: string | null): string => `var(--ctp-${app.categories.find((c) => c.id === id)?.color ?? "peach"})`;
</script>

{#if prefs.garden}
  <div class="flex items-center gap-3 rounded-lg bg-surface0/40 p-2">
    <Tree {done} {total} class="h-12 w-10 shrink-0" />
    <div class="min-w-0 flex-1">
      <p class="t-foot text-subtext1">{t(message.key as MessageKey, message.params)}</p>
      {#if today.length > 0}
        <ul class="mt-1 flex flex-wrap gap-0.5" aria-label={t("garden.plantedOnDay")}>
          {#each today as plant (plant.id)}
            <li><Plant stage={plant.stage} variant={plant.variant} bloom={colorOf(plant.category)} class="size-8" /></li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/if}
