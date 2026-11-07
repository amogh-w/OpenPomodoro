<script lang="ts">
  import { onMount } from "svelte";
  import { currentSlots, cycleDone, cycleTotal, glowOf, hall, slotsOf } from "./hallStore.svelte";
  import { encouragement, litWord } from "./runes";
  import { formatSpan } from "./format";
  import { t, type MessageKey } from "./i18n";
  import Medallion from "./Medallion.svelte";
  import { refreshRecent } from "./recentStore.svelte";
  import Rune from "./Rune.svelte";
  import { prefs } from "../platform/prefs.svelte";

  // The sessions are loaded when the timer screen opens; on this screen they may not be yet.
  onMount(() => void refreshRecent());

  const total = $derived(cycleTotal());
  const current = $derived(hall());
  const done = $derived(cycleDone(current));
  const message = $derived(encouragement(done, total));
  const runes = $derived(current.runes.toReversed().slice(0, 56));
  const medallions = $derived(current.medallions.toReversed().slice(0, 24));
  const when = (ms: number): string => new Date(ms).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
  const time = (ms: number): string => new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
</script>

{#if prefs.runes}
  <section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 p-4" aria-labelledby="hall-title">
    <div class="flex items-center gap-4">
      <Medallion slots={currentSlots(current, done)} {total} class="size-20 shrink-0" />
      <div class="min-w-0">
        <h2 id="hall-title" class="t-title3">{t("runes.title")}</h2>
        <p class="text-subtext1">{t(message.key as MessageKey, message.params)}</p>
      </div>
    </div>

    {#if runes.length === 0}
      <p class="text-subtext1">{t("runes.empty")}</p>
    {:else}
      <div class="grid gap-3">
        <h3 class="t-headline">{t("runes.counts", { runes: current.runes.length, medallions: current.medallions.length })}</h3>
        {#if medallions.length > 0}
          <ul class="flex flex-wrap gap-2" aria-label={t("runes.medallions")}>
            {#each medallions as m (m.id)}
              <li title={t("runes.sealedOn", { date: when(m.completedAt) })}>
                <Medallion slots={slotsOf(m.runes)} total={m.runes.length} class="size-16" label={t("runes.sealedOn", { date: when(m.completedAt) })} />
              </li>
            {/each}
          </ul>
        {/if}
        <ul class="flex flex-wrap gap-1.5" aria-label={t("runes.runes")}>
          {#each runes as rune (rune.id)}
            {@const label = `${when(rune.startedAt)}, ${time(rune.startedAt)}, ${formatSpan(rune.endedAt - rune.startedAt)}, ${t(`runes.lit.${litWord(rune.lit)}` as MessageKey)}`}
            <li title={label}><Rune glyph={rune.glyph} lit={rune.lit} glow={glowOf(rune.category)} class="h-14 w-11" {label} /></li>
          {/each}
        </ul>
        <p class="t-foot text-subtext1">{t("runes.hint", { n: total })}</p>
      </div>
    {/if}
  </section>
{/if}
