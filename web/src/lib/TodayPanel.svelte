<script lang="ts">
  import { app, tick } from "./appState.svelte";
  import { completion } from "./completion";
  import AddSession from "./AddSession.svelte";
  import DayColumn from "./DayColumn.svelte";
  import { cycleDone, glowOf, hall } from "./hallStore.svelte";
  import HallStrip from "./HallStrip.svelte";
  import { dayStartMs, groupByDay, localDayKey, moodTally } from "./days";
  import { formatSpan } from "./format";
  import { t, tn } from "./i18n";
  import { prefs } from "../platform/prefs.svelte";
  import { calendar } from "./calendarStore.svelte";
  import { planPreview } from "./planPreview.svelte";
  import Rune from "./Rune.svelte";
  import { lastRunes, runeOf } from "./runes";
  import { recent, refreshRecent } from "./recentStore.svelte";
  import { addDays, weekStart } from "./stats";
  import type { SessionRecord } from "./types";

  // The day beside the timer: a week strip to pick a day, its total, its mood tally, a timeline and its sessions.
  const todayKey = $derived(localDayKey(tick.now));
  let picked = $state<string | null>(null);
  const day = $derived(picked ?? todayKey);
  const week = $derived(Array.from({ length: 7 }, (_, i) => addDays(weekStart(day), i)));
  const groups = $derived(groupByDay(recent.sessions));
  const group = $derived(groups.find((g) => g.day === day));
  const focused = (key: string): boolean => (groups.find((g) => g.day === key)?.focusCount ?? 0) > 0;
  const tally = $derived(moodTally(group?.sessions ?? []));
  const sessions = $derived((group?.sessions ?? []).filter((s) => s.kind === "focus").toReversed());
  const onToday = $derived(day === todayKey);
  // The runes of the medallion being made, so their rows can be edged in gold.
  const hallNow = $derived(hall());
  const inMedallion = $derived(new Set(lastRunes(hallNow.runes, cycleDone(hallNow)).map((r) => r.id)));
  const hoursOf = (key: string): string => ((groups.find((g) => g.day === key)?.focusMs ?? 0) / 3_600_000).toFixed(2);
  let add: AddSession | undefined = $state();

  const label = (key: string, opts: Intl.DateTimeFormatOptions): string => new Date(dayStartMs(key)).toLocaleDateString(undefined, opts);
  const time = (ms: number): string => new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  const spentOf = (s: SessionRecord): number => s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);
  const title = (s: SessionRecord): string => (s.intention && s.intention !== "" ? s.intention : t("timer.phase.focus"));
  const color = (id: string | null | undefined): string => `var(--ctp-${app.categories.find((c) => c.id === id)?.color ?? "red"})`;
  const shift = (days: number): void => {
    const next = addDays(day, days);
    picked = next >= todayKey ? null : next;
  };
</script>

<section class="card relative grid content-start gap-3 p-4" aria-labelledby="today-title">
  <div class="flex items-center justify-between gap-2">
    <button class="btn btn-gray btn-sm size-9 !px-0" aria-label={t("today.prevDay")} onclick={() => shift(-1)}>
      <svg class="size-4"><use href="/icons.svg#chevron-left" /></svg>
    </button>
    <button class="btn btn-gray btn-sm min-w-0 flex-1" aria-label={t("today.backToToday")} disabled={onToday} onclick={() => (picked = null)}>
      <span id="today-title" class="truncate">{onToday ? t("today.title") : label(day, { weekday: "long", day: "numeric", month: "short" })}</span>
    </button>
    <button class="btn btn-gray btn-sm size-9 !px-0" aria-label={t("today.nextDay")} disabled={onToday} onclick={() => shift(1)}>
      <svg class="size-4"><use href="/icons.svg#chevron-right" /></svg>
    </button>
  </div>

  <div class="grid grid-cols-7 gap-1" role="group" aria-label={t("today.week")}>
    {#each week as key (key)}
      <button
        class="grid min-h-14 justify-items-center rounded-lg bg-surface0/60 py-1 text-subtext1 hover:bg-surface0 aria-pressed:bg-accent/15 aria-pressed:text-accent {key > todayKey ? 'opacity-50' : ''}"
        aria-pressed={key === day}
        aria-label={label(key, { weekday: "long", day: "numeric", month: "long" })}
        disabled={key > todayKey}
        onclick={() => (picked = key === todayKey ? null : key)}
      >
        <span class="text-[11px] uppercase">{label(key, { weekday: "short" })}</span>
        <span class="font-semibold tabular-nums">{Number(key.slice(8))}</span>
        <span class="size-1 rounded-full {key === todayKey ? 'bg-accent' : ''}" aria-hidden="true"></span>
        <span class="text-[10px] tabular-nums opacity-80">{hoursOf(key)}</span>
      </button>
    {/each}
  </div>

  <p class="t-foot text-center font-semibold uppercase tracking-wide text-subtext1">
    <span aria-label={t("mood.focused")}>{tally.focused} ↑</span> · <span aria-label={t("mood.neutral")}>{tally.neutral} →</span> ·
    <span aria-label={t("mood.distracted")}>{tally.distracted} ↓</span>
    <span class="ml-2">{t("today.focusTotal", { time: formatSpan(group?.focusMs ?? 0) })}</span>
  </p>

  <HallStrip />

  <DayColumn
    sessions={group?.sessions ?? []}
    dayStart={dayStartMs(day)}
    categories={app.categories}
    now={onToday ? tick.now : null}
    plannedMs={onToday ? planPreview.plannedMs : null}
    events={calendar.events}
  />

  {#if sessions.length > 0}
    <ul class="grid gap-1 border-t border-surface0 pt-3">
      {#each sessions as s (s.id)}
        <li class="flex items-center gap-3 py-1.5">
          {#if prefs.runes}
            {@const rune = runeOf(s)}
            <Rune glyph={rune.glyph} lit={rune.lit} glow={glowOf(s.category)} gold={inMedallion.has(s.id)} class="h-11 w-9 shrink-0" />
          {:else}
            <span class="size-3 shrink-0 rounded-full" style="background: {color(s.category)}" aria-hidden="true"></span>
          {/if}
          <span class="min-w-0 flex-1">
            <span class="block truncate">{title(s)}</span>
            <span class="t-foot block text-subtext1">
              {time(s.started_at)}–{time(s.ended_at)}{#if completion(s) === true} · <span class="text-green">{t("completion.complete")}</span>{:else if completion(s) === false} · <span class="text-yellow">{t("completion.early")}</span>{/if}
            </span>
          </span>
          <span class="tabular-nums text-subtext1">{formatSpan(spentOf(s))}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="t-foot border-t border-surface0 pt-3 text-subtext1">{t(day === todayKey ? "today.empty" : "today.emptyDay")}</p>
  {/if}
  <button class="btn btn-filled absolute bottom-4 right-4 size-12 !px-0 shadow-lg" aria-label={t("history.add")} onclick={() => add?.show()}>
    <svg class="size-6"><use href="/icons.svg#plus" /></svg>
  </button>
</section>

<AddSession bind:this={add} onsaved={() => void refreshRecent()} />
