<script lang="ts">
  import { app, tick } from "./appState.svelte";
  import { completion } from "./completion";
  import AddSession from "./AddSession.svelte";
  import { iconOf } from "./categories";
  import { cycleDone, glowOf, hall } from "./hallStore.svelte";
  import HallStrip from "./HallStrip.svelte";
  import { dayStartMs, groupByDay, localDayKey, moodTally } from "./days";
  import { formatSpan } from "./format";
  import { t, tn } from "./i18n";
  import { prefs } from "../platform/prefs.svelte";
  import Rune from "./Rune.svelte";
  import { lastRunes, runeOf } from "./runes";
  import { recent, refreshRecent } from "./recentStore.svelte";
  import type { SessionRecord } from "./types";

  // Today beside the timer: the total and mood tally, the medallion, and what was finished.
  const todayKey = $derived(localDayKey(tick.now));
  const groups = $derived(groupByDay(recent.sessions));
  const group = $derived(groups.find((g) => g.day === todayKey));
  const tally = $derived(moodTally(group?.sessions ?? []));
  const sessions = $derived((group?.sessions ?? []).filter((s) => s.kind === "focus").toReversed());
  // The runes of the medallion being made, so their rows can be edged in gold.
  const hallNow = $derived(hall());
  const inMedallion = $derived(new Set(lastRunes(hallNow.runes, cycleDone(hallNow)).map((r) => r.id)));
  let add: AddSession | undefined = $state();

  const label = (key: string, opts: Intl.DateTimeFormatOptions): string => new Date(dayStartMs(key)).toLocaleDateString(undefined, opts);
  const time = (ms: number): string => new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  const spentOf = (s: SessionRecord): number => s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);
  const title = (s: SessionRecord): string => (s.intention && s.intention !== "" ? s.intention : t("timer.phase.focus"));
  const color = (id: string | null | undefined): string => `var(--ctp-${app.categories.find((c) => c.id === id)?.color ?? "red"})`;
</script>

<section class="card grid grid-cols-[minmax(0,1fr)] content-start gap-3 p-4" aria-labelledby="today-title">
  <div class="flex items-center justify-between gap-2">
    <h2 id="today-title" class="t-headline">{t("today.title")}</h2>
    <button class="btn btn-gray btn-sm size-9 !px-0" aria-label={t("history.add")} onclick={() => add?.show()}>
      <svg class="size-4"><use href="/icons.svg#plus" /></svg>
    </button>
  </div>

  <p class="t-foot font-semibold uppercase tracking-wide text-subtext1">
    <span aria-label={t("mood.focused")}>{tally.focused} ↑</span> · <span aria-label={t("mood.neutral")}>{tally.neutral} →</span> ·
    <span aria-label={t("mood.distracted")}>{tally.distracted} ↓</span>
    <span class="ml-2">{t("today.focusTotal", { time: formatSpan(group?.focusMs ?? 0) })}</span>
  </p>

  <HallStrip />

  {#if sessions.length > 0}
    <ul class="anim m-stagger grid max-h-[min(22rem,34vh)] grid-cols-[minmax(0,1fr)] gap-1 overflow-y-auto overflow-x-hidden overscroll-contain border-t border-surface0 pt-3 pr-1">
      {#each sessions as s, i (s.id)}
        <li class="flex items-center gap-3 py-1.5" style="--i:{Math.min(i, 5)}">
          {#if prefs.runes}
            {@const rune = runeOf(s)}
            <Rune glyph={rune.glyph} lit={rune.lit} glow={glowOf(s.category)} gold={inMedallion.has(s.id)} class="h-11 w-9 shrink-0" />
          {:else}
            <span
              class="grid size-9 shrink-0 place-items-center rounded-[var(--r-ctl)]"
              style="background: color-mix(in oklab, {color(s.category)} 18%, transparent); color: {color(s.category)}"
              aria-hidden="true"
            >
              <svg class="size-5"><use href="/icons.svg#{iconOf(app.categories.find((c) => c.id === s.category))}" /></svg>
            </span>
          {/if}
          <span class="min-w-0 flex-1">
            <span class="block truncate">{title(s)}</span>
            <span class="t-foot block text-subtext1">
              <span class="font-medium text-text">{app.categories.find((c) => c.id === s.category)?.name ?? t("timer.category.none")}</span> · {time(s.started_at)}–{time(s.ended_at)}{#if completion(s) === true} · <span class="text-green">{t("completion.complete")}</span>{:else if completion(s) === false} · <span class="text-yellow">{t("completion.early")}</span>{/if}
            </span>
          </span>
          <span class="tabular-nums text-subtext1">{formatSpan(spentOf(s))}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="t-foot border-t border-surface0 pt-3 text-subtext1">{t("today.empty")}</p>
  {/if}
</section>

<AddSession bind:this={add} onsaved={() => void refreshRecent()} />
