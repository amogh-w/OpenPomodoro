<script lang="ts">
  import { app, tick } from "./appState.svelte";
  import DayTimeline from "./DayTimeline.svelte";
  import { dayStartMs, groupByDay, localDayKey, moodTally } from "./days";
  import { formatSpan } from "./format";
  import { t, tn } from "./i18n";
  import { recent } from "./recentStore.svelte";
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
  const onCurrentWeek = $derived(week.includes(todayKey));

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

<section class="card grid content-start gap-3 p-4" aria-labelledby="today-title">
  <div class="flex items-center justify-between gap-2">
    <button class="btn btn-gray btn-sm size-9 !px-0" aria-label={t("today.prevWeek")} onclick={() => shift(-7)}>
      <svg class="size-4"><use href="/icons.svg#chevron-left" /></svg>
    </button>
    <h2 id="today-title" class="t-headline">{day === todayKey ? t("today.title") : label(day, { weekday: "long", day: "numeric", month: "short" })}</h2>
    <button class="btn btn-gray btn-sm size-9 !px-0" aria-label={t("today.nextWeek")} disabled={onCurrentWeek} onclick={() => shift(7)}>
      <svg class="size-4"><use href="/icons.svg#chevron-right" /></svg>
    </button>
  </div>

  <div class="grid grid-cols-7 gap-1" role="group" aria-label={t("today.week")}>
    {#each week as key (key)}
      <button
        class="grid min-h-12 justify-items-center rounded-lg py-1 text-subtext1 hover:bg-surface0 aria-pressed:bg-accent/15 aria-pressed:text-accent {key > todayKey ? 'opacity-50' : ''}"
        aria-pressed={key === day}
        aria-label={label(key, { weekday: "long", day: "numeric", month: "long" })}
        disabled={key > todayKey}
        onclick={() => (picked = key === todayKey ? null : key)}
      >
        <span class="text-[11px] uppercase">{label(key, { weekday: "short" })}</span>
        <span class="font-semibold tabular-nums">{Number(key.slice(8))}</span>
        <span class="size-1 rounded-full {focused(key) ? 'bg-accent' : ''}" aria-hidden="true"></span>
      </button>
    {/each}
  </div>

  <div>
    <div class="t-title1 tabular-nums">{formatSpan(group?.focusMs ?? 0)}</div>
    <div class="t-foot text-subtext1">
      {tn("history.sessions", group?.focusCount ?? 0)}{#if tally.focused + tally.neutral + tally.distracted > 0}
        · {tally.focused} {t("mood.focused").toLowerCase()} · {tally.neutral} {t("mood.neutral").toLowerCase()} · {tally.distracted} {t("mood.distracted").toLowerCase()}{/if}
    </div>
  </div>

  <DayTimeline sessions={group?.sessions ?? []} dayStart={dayStartMs(day)} categories={app.categories} now={day === todayKey ? tick.now : null} />

  {#if sessions.length > 0}
    <ul class="grid gap-1 border-t border-surface0 pt-3">
      {#each sessions as s (s.id)}
        <li class="flex items-center gap-3 py-1.5">
          <span class="size-3 shrink-0 rounded-full" style="background: {color(s.category)}" aria-hidden="true"></span>
          <span class="min-w-0 flex-1">
            <span class="block truncate">{title(s)}</span>
            <span class="t-foot block text-subtext1">{time(s.started_at)}–{time(s.ended_at)}</span>
          </span>
          <span class="tabular-nums text-subtext1">{formatSpan(spentOf(s))}</span>
        </li>
      {/each}
    </ul>
  {:else}
    <p class="t-foot border-t border-surface0 pt-3 text-subtext1">{t(day === todayKey ? "today.empty" : "today.emptyDay")}</p>
  {/if}
</section>
