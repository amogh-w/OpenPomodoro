<script lang="ts">
  import { app, tick } from "./appState.svelte";
  import { groupByDay, localDayKey } from "./days";
  import { formatSpan } from "./format";
  import { t, tn } from "./i18n";
  import { recent } from "./recentStore.svelte";
  import type { SessionRecord } from "./types";

  const today = $derived(groupByDay(recent.sessions).find((d) => d.day === localDayKey(tick.now)));
  const sessions = $derived((today?.sessions ?? []).filter((s) => s.kind === "focus").toReversed());
  const time = (ms: number): string => new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  const spentOf = (s: SessionRecord): number => s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);
  const title = (s: SessionRecord): string => (s.intention && s.intention !== "" ? s.intention : t("timer.phase.focus"));
  const color = (id: string | null | undefined): string => `var(--ctp-${app.categories.find((c) => c.id === id)?.color ?? "red"})`;
</script>

<section class="card grid content-start gap-3 p-4" aria-labelledby="today-title">
  <div>
    <h2 id="today-title" class="t-foot text-subtext1">{t("today.title")}</h2>
    <div class="t-title1 tabular-nums">{formatSpan(today?.focusMs ?? 0)}</div>
    <div class="t-foot text-subtext1">{tn("history.sessions", today?.focusCount ?? 0)}</div>
  </div>
  {#if sessions.length > 0}
    <ul class="grid gap-1 border-t border-surface0 pt-3">
      {#each sessions as s (s.id)}
        <li class="flex items-center gap-3 py-1.5">
          <span class="size-3 shrink-0 rounded-full" style="background: {color(s.category)}" aria-hidden="true"></span>
          <span class="min-w-0 flex-1">
            <span class="block truncate">{title(s)}</span>
            <span class="t-foot block text-subtext1">{time(s.started_at)}</span>
          </span>
          <span class="tabular-nums text-subtext1">{formatSpan(spentOf(s))}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="t-foot border-t border-surface0 pt-3 text-subtext1">{t("today.empty")}</p>
  {/if}
</section>
