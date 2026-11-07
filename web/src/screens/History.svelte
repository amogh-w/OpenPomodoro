<script lang="ts">
  import { onMount } from "svelte";
  import AddSession from "../lib/AddSession.svelte";
  import { calendar } from "../lib/calendarStore.svelte";
  import CategoryBreakdown from "../lib/CategoryBreakdown.svelte";
  import CategoryChip from "../lib/CategoryChip.svelte";
  import DayTimeline from "../lib/DayTimeline.svelte";
  import Heatmap from "../lib/Heatmap.svelte";
  import Insights from "../lib/Insights.svelte";
  import SessionEditor from "../lib/SessionEditor.svelte";
  import { ApiError, api } from "../lib/api";
  import { app, serverNow, tick } from "../lib/appState.svelte";
  import { iconOf, timePerCategory } from "../lib/categories";
  import { downloadBackup } from "../lib/backupFile";
  import { dayStartMs, groupByDay, localDayKey, monthBounds } from "../lib/days";
  import { formatSpan } from "../lib/format";
  import { t, tn } from "../lib/i18n";
  import { refreshRecent } from "../lib/recentStore.svelte";
  import { computeInsights, daysOfMonth } from "../lib/insights";
  import { completion } from "../lib/completion";
  import { computeStats } from "../lib/stats";
  import type { DayTotal, SessionRecord } from "../lib/types";

  let add: AddSession | undefined = $state();
  let editor: SessionEditor | undefined = $state();

  const first = new Date(serverNow());
  let view = $state({ year: first.getFullYear(), month0: first.getMonth() });
  let sessions = $state<SessionRecord[]>([]);
  let earlier = $state<SessionRecord[]>([]);
  let notesOnly = $state(false);
  const NONE = "__none";
  let categoryFilter = $state("");
  let totals = $state<DayTotal[]>([]);
  let expanded = $state<string | null>(null);
  let offline = $state(false);
  let loaded = $state(false);

  const todayKey = $derived(localDayKey(tick.now));
  const stats = $derived(computeStats(totals, todayKey));
  const shown = $derived(
    sessions.filter((s) => (!notesOnly || (s.notes ?? "").trim() !== "") && (categoryFilter === "" || (categoryFilter === NONE ? !s.category : s.category === categoryFilter))),
  );
  const days = $derived(groupByDay(shown));
  const insights = $derived(computeInsights(sessions, earlier, daysOfMonth(view.year, view.month0)));
  const isCurrentMonth = $derived(view.year === first.getFullYear() && view.month0 === first.getMonth());
  const monthLabel = $derived(new Date(view.year, view.month0, 1).toLocaleDateString(undefined, { month: "long", year: "numeric" }));
  const categoryOf = (id: string | null | undefined) => app.categories.find((c) => c.id === id);
  const categoryColor = (id: string | null | undefined): string => `var(--ctp-${categoryOf(id)?.color ?? "overlay1"})`;
  // The filter chips: categories with the most time this month first, then the unused ones, then "none" if any.
  const monthTimes = $derived(timePerCategory(sessions));
  const filterRows = $derived(
    [...app.categories]
      .sort((a, b) => (monthTimes.find((r) => r.id === b.id)?.ms ?? 0) - (monthTimes.find((r) => r.id === a.id)?.ms ?? 0) || a.name.localeCompare(b.name))
      .map((c) => ({ category: c, ms: monthTimes.find((r) => r.id === c.id)?.ms ?? 0 })),
  );
  const noneMs = $derived(monthTimes.find((r) => r.id === null)?.ms ?? 0);

  async function load(): Promise<void> {
    const { from, to } = monthBounds(view.year, view.month0);
    try {
      const before = monthBounds(view.year, view.month0 - 1);
      const [month, last, summary] = await Promise.all([
        api.history(from, to),
        api.history(before.from, before.to),
        api.summary(-new Date().getTimezoneOffset()),
      ]);
      sessions = month.sessions;
      earlier = last.sessions;
      totals = summary.days;
      offline = false;
      if (expanded === null || !days.some((d) => d.day === expanded)) expanded = days.find((d) => d.day === todayKey)?.day ?? days[0]?.day ?? null;
    } catch (error) {
      if (error instanceof ApiError && (error.isNetwork || error.status >= 500)) offline = true;
      else throw error;
    } finally {
      loaded = true;
    }
  }

  function changeMonth(by: number): void {
    const d = new Date(view.year, view.month0 + by, 1);
    view = { year: d.getFullYear(), month0: d.getMonth() };
    expanded = null;
    void load();
  }

  const reload = (): void => {
    void load();
    void refreshRecent();
  };

  const time = (ms: number): string => new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  const dayLabel = (key: string): string => new Date(dayStartMs(key)).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" });
  const spentOf = (s: SessionRecord): number => s.duration_ms ?? Math.max(0, s.ended_at - s.started_at);
  const kindLabel = (s: SessionRecord): string => (s.intention && s.intention !== "" ? s.intention : t(`timer.phase.${s.kind}`));

  const backup = (): Promise<boolean> => downloadBackup(localDayKey(Date.now()).replaceAll("-", ""));

  onMount(() => void load());
</script>

<div class="flex items-center justify-between gap-3">
  <h1 class="t-large">{t("history.title")}</h1>
  <button class="btn btn-gray btn-sm" onclick={() => add?.show()}>{t("history.add")}</button>
</div>

{#if offline}
  <div class="alert tone-warn mt-3" role="status">
    <svg class="size-5 shrink-0"><use href="/icons.svg#triangle-alert" /></svg>
    <div>{t("history.needsConnection")}</div>
  </div>
{/if}

<section class="mt-4 grid grid-cols-2 gap-3 lg:grid-cols-4" aria-label={t("history.title")}>
  <div class="card p-3">
    <div class="t-foot text-subtext1">{t("history.today")}</div>
    <div class="t-title2 tabular-nums">{formatSpan(stats.todayMs)}</div>
  </div>
  <div class="card p-3">
    <div class="t-foot text-subtext1">{t("history.week")}</div>
    <div class="t-title2 tabular-nums">{formatSpan(stats.weekMs)}</div>
  </div>
  <div class="card p-3">
    <div class="t-foot text-subtext1">{t("history.allTime")}</div>
    <div class="t-title2 tabular-nums">{formatSpan(stats.allTimeMs)}</div>
    <div class="t-foot text-subtext1">{tn("history.sessions", stats.sessions)}</div>
  </div>
  <div class="card p-3">
    <div class="t-foot text-subtext1">{t("history.streak.title")}</div>
    <div class="t-title2">{tn("history.streak", stats.currentStreak)}</div>
    <div class="t-foot text-subtext1">{t("history.longest", { n: stats.longestStreak })}</div>
  </div>
</section>

<div class="mt-6 flex items-center justify-between">
  <button class="btn btn-gray size-11 !px-0" aria-label={t("history.prevMonth")} onclick={() => changeMonth(-1)}>
    <svg class="size-5"><use href="/icons.svg#chevron-left" /></svg>
  </button>
  <h2 class="t-title3" aria-live="polite">{monthLabel}</h2>
  <button class="btn btn-gray size-11 !px-0" aria-label={t("history.nextMonth")} disabled={isCurrentMonth} onclick={() => changeMonth(1)}>
    <svg class="size-5"><use href="/icons.svg#chevron-right" /></svg>
  </button>
</div>

<div class="mt-3"><Heatmap {totals} {todayKey} /></div>

<div class="mt-3 grid gap-4 lg:grid-cols-2 lg:items-start lg:gap-6">
  <Insights data={insights} categories={app.categories} {sessions} />

  <div class="grid content-start gap-3">
    <div class="flex flex-wrap items-center gap-2">
      <button class="chip" aria-pressed={notesOnly} onclick={() => (notesOnly = !notesOnly)}>{t("history.notesOnly")}</button>
    </div>
    {#if app.categories.length > 0}
      <div class="flex flex-wrap gap-2" role="group" aria-label={t("history.filter")}>
        <button type="button" class="chip" aria-pressed={categoryFilter === ""} onclick={() => (categoryFilter = "")}>{t("history.allCategories")}</button>
        {#each filterRows as row (row.category.id)}
          <button
            type="button"
            class="min-h-9 rounded-[var(--r-ctl)] transition-transform duration-150 ease-ios active:scale-[.97]"
            aria-pressed={categoryFilter === row.category.id}
            onclick={() => (categoryFilter = categoryFilter === row.category.id ? "" : row.category.id)}
          >
            <CategoryChip category={row.category} small selected={categoryFilter === row.category.id} detail={row.ms > 0 ? formatSpan(row.ms) : ""} class="min-h-9 px-3" />
          </button>
        {/each}
        {#if noneMs > 0}
          <button
            type="button"
            class="min-h-9 rounded-[var(--r-ctl)] transition-transform duration-150 ease-ios active:scale-[.97]"
            aria-pressed={categoryFilter === NONE}
            onclick={() => (categoryFilter = categoryFilter === NONE ? "" : NONE)}
          >
            <CategoryChip small selected={categoryFilter === NONE} detail={formatSpan(noneMs)} class="min-h-9 px-3" />
          </button>
        {/if}
      </div>
    {/if}
<section class="grid content-start gap-2" aria-label={t("history.title")}>
  {#each days as day (day.day)}
    {@const open = expanded === day.day}
    <div class="card overflow-hidden">
      <button
        class="flex min-h-14 w-full items-center justify-between gap-3 p-3 text-left"
        aria-expanded={open}
        onclick={() => (expanded = open ? null : day.day)}
      >
        <span>
          <span class="block t-headline">{dayLabel(day.day)}</span>
          <span class="t-foot text-subtext1">{tn("history.sessions", day.focusCount)}</span>
        </span>
        <span class="tabular-nums text-subtext1">{formatSpan(day.focusMs)}</span>
      </button>
      {#if open}
        <div class="grid gap-3 border-t border-surface0 p-3">
          <DayTimeline
            sessions={day.sessions}
            dayStart={dayStartMs(day.day)}
            categories={app.categories}
            now={day.day === todayKey ? tick.now : null}
            events={calendar.events}
          />
          <CategoryBreakdown sessions={day.sessions} categories={app.categories} />
          <div class="grid gap-1">
            {#each day.sessions as s (s.id)}
              <button class="flex min-h-12 w-full items-center gap-3 rounded-lg p-2 text-left hover:bg-surface0" onclick={() => editor?.show(s)}>
                <span
                  class="grid size-9 shrink-0 place-items-center rounded-[var(--r-ctl)]"
                  style="background: color-mix(in oklab, {s.kind === 'focus' ? categoryColor(s.category) : 'var(--ctp-green)'} 18%, transparent); color: {s.kind === 'focus' ? categoryColor(s.category) : 'var(--ctp-green)'}"
                  aria-hidden="true"
                >
                  <svg class="size-5"><use href="/icons.svg#{s.kind === 'focus' ? iconOf(categoryOf(s.category)) : 'coffee'}" /></svg>
                </span>
                <span class="min-w-0 flex-1">
                  <span class="block truncate">{kindLabel(s)}</span>
                  <span class="t-foot block text-subtext1">
                    {#if s.kind === "focus"}<span class="font-medium text-text">{categoryOf(s.category)?.name ?? t("timer.category.none")}</span> · {/if}{time(s.started_at)}–{time(s.ended_at)}{#if s.mood} · {t(`mood.${s.mood}`)}{/if}{#if completion(s) === true}
                    · <span class="text-green">{t("completion.complete")}</span>{:else if completion(s) === false} · <span class="text-yellow">{t("completion.early")}</span>{/if}
                  </span>
                  {#if s.notes}<span class="t-foot line-clamp-1 block text-subtext1">{s.notes}</span>{/if}
                </span>
                <span class="tabular-nums text-subtext1">{formatSpan(spentOf(s))}</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {:else}
    {#if loaded && !offline}<p class="text-subtext1">{t("history.none")}</p>{/if}
  {/each}
</section>
  </div>
</div>

<section class="card mt-6 grid gap-3 p-4" aria-labelledby="export-title">
  <h2 id="export-title" class="t-title3">{t("history.export")}</h2>
  <p class="t-foot text-subtext1">{t("history.export.hint")}</p>
  <div class="grid gap-2 sm:grid-cols-3">
    <button class="btn btn-gray" onclick={backup}>{t("history.export.json")}</button>
    <a class="btn btn-gray" href="/api/export?format=csv" download>{t("history.export.csv")}</a>
    <a class="btn btn-gray" href="/api/export?format=ics" download>{t("history.export.ics")}</a>
  </div>
</section>

<AddSession bind:this={add} onsaved={reload} />
<SessionEditor bind:this={editor} onchanged={reload} />
