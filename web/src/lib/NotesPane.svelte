<script lang="ts">
  import { app } from "./appState.svelte";
  import { draft, setNotes } from "./draft.svelte";
  import { t } from "./i18n";
  import { recentNotes } from "./recent";
  import { recent } from "./recentStore.svelte";

  let { open = false }: { open?: boolean } = $props();

  // Notes from earlier sessions with the same intention come first, so past thinking is in view while writing.
  const suggestions = $derived(recentNotes(recent.sessions, app.timer?.intention ?? null, 3));
</script>

<details class="card w-full p-3 text-left" {open}>
  <summary class="min-h-11 cursor-pointer py-2.5 font-semibold">{t("timer.notes")}</summary>
  <textarea
    class="field mt-1 min-h-24 w-full"
    placeholder={t("timer.notes.placeholder")}
    maxlength="4000"
    value={draft.notes}
    oninput={(e) => setNotes(e.currentTarget.value)}
  ></textarea>
  {#if suggestions.length > 0}
    <div class="mt-3 grid gap-1.5">
      <span class="t-foot text-subtext1">{t("timer.notes.recent")}</span>
      {#each suggestions as note (note.at)}
        <p class="t-foot line-clamp-2 border-l-2 border-surface1 pl-2 text-subtext1">{note.text}</p>
      {/each}
    </div>
  {/if}
</details>
