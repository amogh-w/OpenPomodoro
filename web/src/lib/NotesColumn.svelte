<script lang="ts">
  import { app, tick } from "./appState.svelte";
  import { draft, setNotes } from "./draft.svelte";
  import { formatAgo, formatSpan } from "./format";
  import { t } from "./i18n";
  import { notesFeed } from "./recent";
  import { recent } from "./recentStore.svelte";
  import type { SessionRecord } from "./types";

  // The notes for the session in progress, with earlier notes beside them (the pane on wide screens).
  let sameCategory = $state(false);
  const category = $derived(app.timer?.category ?? null);
  const feed = $derived(notesFeed(recent.sessions, sameCategory && category !== null ? category : null));
  const spentOf = (s: SessionRecord): number => s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);
  const found = (id: string | null | undefined) => app.categories.find((c) => c.id === id);
  const color = (id: string | null | undefined): string => `var(--ctp-${found(id)?.color ?? "red"})`;
</script>

<section class="card grid gap-3 p-4" aria-labelledby="notes-col-title">
  <h2 id="notes-col-title" class="t-title3">{t("timer.notes")}</h2>
  <textarea
    class="field min-h-56 w-full"
    aria-labelledby="notes-col-title"
    placeholder={t("timer.notes.prompt")}
    maxlength="4000"
    value={draft.notes}
    oninput={(e) => setNotes(e.currentTarget.value)}
  ></textarea>

  <div class="flex items-center justify-between gap-2 border-t border-surface0 pt-3">
    <h3 class="t-foot text-subtext1">{t("timer.notes.recent")}</h3>
    {#if category !== null}
      <button class="chip" aria-pressed={sameCategory} onclick={() => (sameCategory = !sameCategory)}>
        {t("notes.sameCategory")}
      </button>
    {/if}
  </div>

  {#if feed.length > 0}
    <ul class="grid max-h-[28rem] gap-3 overflow-y-auto">
      {#each feed as s (s.id)}
        <li class="border-l-2 pl-3" style="border-color: {color(s.category)}">
          <div class="flex items-baseline justify-between gap-2">
            <span class="t-foot min-w-0 truncate font-semibold">{s.intention || t("timer.phase.focus")}</span>
            <span class="t-foot shrink-0 tabular-nums text-subtext1">{formatSpan(spentOf(s))}</span>
          </div>
          <div class="t-foot text-subtext1">
            {#if found(s.category)}{found(s.category)?.name} · {/if}{formatAgo(s.started_at, tick.now)}
          </div>
          <p class="mt-1 whitespace-pre-line text-subtext1">{s.notes}</p>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="t-foot text-subtext1">{t("notes.feed.empty")}</p>
  {/if}
</section>
