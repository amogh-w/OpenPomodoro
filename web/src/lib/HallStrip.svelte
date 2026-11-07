<script lang="ts">
  import { currentSlots, cycleDone, cycleTotal, hall } from "./hallStore.svelte";
  import { encouragement } from "./runes";
  import { t, type MessageKey } from "./i18n";
  import Medallion from "./Medallion.svelte";
  import { prefs } from "../platform/prefs.svelte";

  // The medallion being made, for the day panel: how far along it is and what is left. The runes themselves are the
  // markers in the session list below.
  const total = $derived(cycleTotal());
  const current = $derived(hall());
  const done = $derived(cycleDone(current));
  const message = $derived(encouragement(done, total));
</script>

{#if prefs.runes}
  <div class="flex items-center gap-4 rounded-lg bg-surface0/40 p-3">
    <Medallion slots={currentSlots(current, done)} {total} class="size-28 shrink-0" label={t("timer.round", { n: Math.min(done + 1, total), total })} />
    <div class="min-w-0">
      <p class="t-foot uppercase tracking-wide text-subtext1">{t("runes.stripTitle")}</p>
      <p class="t-title1 tabular-nums">{done} / {total}</p>
      <p class="t-foot text-subtext1">{t(message.key as MessageKey, message.params)}</p>
      <p class="t-foot mt-1 text-subtext1">{t("runes.goldHint")}</p>
    </div>
  </div>
{/if}
