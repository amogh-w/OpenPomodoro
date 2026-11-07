<script lang="ts">
  import { sendCommand } from "./appState.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
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

  let confirm: ConfirmDialog | undefined = $state();
  let busy = $state(false);

  /** Break the medallion: ask first, then start the count again. The runes stay in the hall. */
  async function breakIt(): Promise<void> {
    if (busy || !(await confirm?.ask())) return;
    busy = true;
    try {
      await sendCommand({ name: "reset_cycle" });
    } finally {
      busy = false;
    }
  }
</script>

{#if prefs.runes}
  <div class="flex items-center gap-4 rounded-lg bg-surface0/40 p-3">
    <Medallion slots={currentSlots(current, done)} {total} class="size-28 shrink-0" label={t("timer.round", { n: Math.min(done + 1, total), total })} />
    <div class="min-w-0">
      <p class="t-foot uppercase tracking-wide text-subtext1">{t("runes.stripTitle")}</p>
      <p class="t-title1 tabular-nums">{done} / {total}</p>
      <p class="t-foot text-subtext1">{t(message.key as MessageKey, message.params)}</p>
      <p class="t-foot mt-1 text-subtext1">{t("runes.goldHint")}</p>
      {#if done > 0}
        <button class="btn btn-plain btn-sm -ml-3 mt-1" disabled={busy} onclick={breakIt}>{t("runes.break")}</button>
      {/if}
    </div>
  </div>
  <ConfirmDialog
    bind:this={confirm}
    title={t("runes.break.title")}
    body={t("runes.break.body", { done, total })}
  />
{/if}
