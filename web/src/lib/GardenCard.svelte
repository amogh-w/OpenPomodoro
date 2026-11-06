<script lang="ts">
  import { onMount } from "svelte";
  import { app } from "./appState.svelte";
  import { buildGarden, encouragement } from "./garden";
  import { formatSpan } from "./format";
  import { t, type MessageKey } from "./i18n";
  import Plant from "./Plant.svelte";
  import { recent, refreshRecent } from "./recentStore.svelte";
  import Tree from "./Tree.svelte";
  import { prefs } from "../platform/prefs.svelte";

  // The sessions are loaded when the timer screen opens; on this screen they may not be yet.
  onMount(() => void refreshRecent());

  // The whole garden: the tree being grown now, the trees finished, and every plant from the last few months.
  const total = $derived(app.settings?.rounds_before_long ?? 4);
  const garden = $derived(buildGarden(recent.sessions, total));
  const done = $derived((app.timer?.rounds ?? 0) % total);
  const message = $derived(encouragement(done, total));
  const plants = $derived(garden.plants.toReversed().slice(0, 56));
  const trees = $derived(garden.trees.toReversed().slice(0, 24));
  const colorOf = (id: string | null): string => `var(--ctp-${app.categories.find((c) => c.id === id)?.color ?? "peach"})`;
  const when = (ms: number): string => new Date(ms).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
  const sizeWord = (size: number): string => `${Math.round(size * 100)}%`;
</script>

{#if prefs.garden}
  <section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 p-4" aria-labelledby="garden-title">
    <div class="flex items-center gap-4">
      <Tree {done} {total} class="h-20 w-[4.5rem] shrink-0" />
      <div class="min-w-0">
        <h2 id="garden-title" class="t-title3">{t("garden.title")}</h2>
        <p class="text-subtext1">{t(message.key as MessageKey, message.params)}</p>
      </div>
    </div>

    {#if plants.length === 0}
      <p class="text-subtext1">{t("garden.empty")}</p>
    {:else}
      <div class="grid gap-2">
        <h3 class="t-headline">{t("garden.counts", { plants: garden.plants.length, trees: garden.trees.length })}</h3>
        {#if trees.length > 0}
          <ul class="flex flex-wrap gap-2" aria-label={t("garden.trees")}>
            {#each trees as tree (tree.id)}
              <li title={t("garden.treeOn", { date: when(tree.completedAt) })}>
                <Tree done={tree.plants} total={tree.plants} class="h-16 w-14" label={t("garden.treeOn", { date: when(tree.completedAt) })} />
              </li>
            {/each}
          </ul>
        {/if}
        <ul class="flex flex-wrap gap-1" aria-label={t("garden.plants")}>
          {#each plants as plant (plant.id)}
            <li title="{when(plant.startedAt)} · {sizeWord(plant.size)}">
              <Plant stage={plant.stage} variant={plant.variant} bloom={colorOf(plant.category)} class="size-12" label="{when(plant.startedAt)}, {sizeWord(plant.size)}" />
            </li>
          {/each}
        </ul>
        <p class="t-foot text-subtext1">{t("garden.hint", { n: total })}</p>
      </div>
    {/if}
  </section>
{/if}
