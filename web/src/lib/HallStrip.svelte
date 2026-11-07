<script lang="ts">
  import { currentSlots, cycleDone, cycleTotal, glowOf, hall, slotsOf } from "./hallStore.svelte";
  import { encouragement, litWord } from "./runes";
  import { formatSpan } from "./format";
  import { t, type MessageKey } from "./i18n";
  import Medallion from "./Medallion.svelte";
  import Rune from "./Rune.svelte";
  import { prefs } from "../platform/prefs.svelte";

  // A line of the hall for the day panel: the medallion being made and the runes carved on the day shown.
  let { day }: { day: string } = $props();

  const total = $derived(cycleTotal());
  const current = $derived(hall());
  const done = $derived(cycleDone(current));
  const message = $derived(encouragement(done, total));
  const carved = $derived(current.runes.filter((r) => r.day === day));
  const time = (ms: number): string => new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
</script>

{#if prefs.runes}
  <div class="flex items-center gap-3 rounded-lg bg-surface0/40 p-2">
    <Medallion slots={currentSlots(current, done)} {total} class="size-16 shrink-0" />
    <div class="min-w-0 flex-1">
      <p class="t-foot text-subtext1">{t(message.key as MessageKey, message.params)}</p>
      {#if carved.length > 0}
        <ul class="mt-1.5 flex flex-wrap gap-1" aria-label={t("runes.carvedOnDay")}>
          {#each carved as rune (rune.id)}
            {@const label = `${time(rune.startedAt)}, ${formatSpan(rune.endedAt - rune.startedAt)}, ${t(`runes.lit.${litWord(rune.lit)}` as MessageKey)}`}
            <li title={label}><Rune glyph={rune.glyph} lit={rune.lit} glow={glowOf(rune.category)} class="h-8 w-[1.6rem]" {label} /></li>
          {/each}
        </ul>
      {/if}
    </div>
  </div>
{/if}
