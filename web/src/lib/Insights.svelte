<script lang="ts">
  import { completionRate } from "./completion";
  import { formatSpan } from "./format";
  import { t, tn } from "./i18n";
  import { byIntention, type Insights } from "./insights";
  import type { SessionRecord, Category } from "./types";

  let { data, categories, sessions }: { data: Insights; categories: Category[]; sessions: SessionRecord[] } = $props();
  const done = $derived(completionRate(sessions));

  const hour = (h: number | null): string => (h === null ? "–" : new Date(2000, 0, 1, h).toLocaleTimeString(undefined, { hour: "numeric" }));
  const percent = (f: number | null): string => (f === null ? "" : `${f >= 0 ? "+" : "−"}${Math.round(Math.abs(f) * 100)}%`);
  const intentions = $derived(byIntention(sessions));
  const nameOf = (id: string | null): string => categories.find((c) => c.id === id)?.name ?? t("timer.category.none");
  const colorOf = (id: string | null): string => `var(--ctp-${categories.find((c) => c.id === id)?.color ?? "overlay1"})`;
  const peak = $derived(Math.max(1, ...data.days.map((d) => d.focusMs)));
  const average = $derived(data.activeDays > 0 ? data.avgPerDayMs : 0);
  const moods = $derived(
    (["focused", "neutral", "distracted"] as const).map((m) => ({ key: m, count: data.mood[m] })),
  );
</script>

<section class="grid gap-3" aria-label={t("insights.title")}>
  <div class="grid grid-cols-2 gap-3 sm:grid-cols-3">
    <div class="card p-3">
      <div class="t-foot text-subtext1">{t("insights.totalFocus")}</div>
      <div class="t-title2 tabular-nums">{formatSpan(data.focusMs)}</div>
      {#if data.change !== null}
        <div class="t-foot {data.change >= 0 ? 'text-green' : 'text-red'}">{percent(data.change)} {t("insights.vsLast")}</div>
      {/if}
    </div>
    <div class="card p-3">
      <div class="t-foot text-subtext1">{t("insights.avgDay")}</div>
      <div class="t-title2 tabular-nums">{formatSpan(data.avgPerDayMs)}</div>
      <div class="t-foot text-subtext1">{tn("insights.activeDays", data.activeDays)}</div>
    </div>
    <div class="card p-3">
      <div class="t-foot text-subtext1">{t("insights.completed")}</div>
      <div class="t-title2 tabular-nums">{done.rate === null ? "–" : `${Math.round(done.rate * 100)}%`}</div>
      {#if done.rated > 0}<div class="t-foot text-subtext1">{t("insights.completedOf", { done: done.completed, rated: done.rated })}</div>{/if}
    </div>
    <div class="card p-3">
      <div class="t-foot text-subtext1">{t("insights.bestTime")}</div>
      <div class="t-title2 tabular-nums">{hour(data.bestHour)}</div>
    </div>
    <div class="card p-3">
      <div class="t-foot text-subtext1">{t("insights.totalBreak")}</div>
      <div class="t-title2 tabular-nums">{formatSpan(data.breakMs)}</div>
    </div>
    <div class="card p-3">
      <div class="t-foot text-subtext1">{t("insights.ratio")}</div>
      <div class="t-title2 tabular-nums">{data.ratio === null ? "–" : `${data.ratio.toFixed(1)} : 1`}</div>
    </div>
    <div class="card p-3">
      <div class="t-foot text-subtext1">{t("insights.distracted")}</div>
      <div class="t-title2 tabular-nums">{hour(data.distractedHour)}</div>
    </div>
  </div>

  <div class="card grid grid-cols-3 gap-2 p-3 text-center" role="group" aria-label={t("insights.mood")}>
    {#each moods as m (m.key)}
      <div>
        <div class="t-title2 tabular-nums">{m.count}</div>
        <div class="t-foot text-subtext1">{t(`mood.${m.key}`)}</div>
      </div>
    {/each}
  </div>

  <div class="card grid gap-2 p-3">
    <h3 class="t-foot text-subtext1">{t("insights.daily")}</h3>
    <div class="relative flex h-32 items-end gap-0.5" role="img" aria-label={t("insights.daily")}>
      {#each data.days as d (d.day)}
        <div
          class="min-h-px flex-1 rounded-t-sm bg-red"
          style="height: {(d.focusMs / peak) * 100}%; opacity: {d.focusMs > 0 ? 1 : 0.2}"
          title="{d.day}: {formatSpan(d.focusMs)}"
        ></div>
      {/each}
      {#if average > 0}
        <div class="pointer-events-none absolute inset-x-0 border-t border-dashed border-subtext1" style="bottom: {(average / peak) * 100}%"></div>
      {/if}
    </div>
    <div class="t-foot flex justify-between text-subtext1 tabular-nums" aria-hidden="true">
      <span>1</span><span>{data.days.length}</span>
    </div>
  </div>

  {#if intentions.length > 0}
    <div class="card overflow-x-auto p-1">
      <table class="tbl">
        <caption class="sr-only">{t("insights.intentions")}</caption>
        <thead>
          <tr>
            <th scope="col">{t("insights.intention")}</th>
            <th scope="col" class="text-right">{t("insights.share")}</th>
            <th scope="col" class="text-right">{t("insights.count")}</th>
            <th scope="col" class="text-right">{t("insights.time")}</th>
          </tr>
        </thead>
        <tbody>
          {#each intentions as row (row.name)}
            <tr>
              <td class="max-w-[12rem] truncate">{row.name}</td>
              <td class="text-right tabular-nums">{Math.round(row.share * 100)}%</td>
              <td class="text-right tabular-nums text-subtext1">{row.sessions}</td>
              <td class="text-right tabular-nums">{formatSpan(row.focusMs)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  {#if data.categories.length > 0}
    <div class="card overflow-x-auto p-1">
      <table class="tbl">
        <caption class="sr-only">{t("insights.categories")}</caption>
        <thead>
          <tr>
            <th scope="col">{t("timer.category")}</th>
            <th scope="col" class="text-right">{t("insights.share")}</th>
            <th scope="col" class="text-right">{t("insights.change")}</th>
            <th scope="col" class="text-right">{t("insights.time")}</th>
          </tr>
        </thead>
        <tbody>
          {#each data.categories as row (row.id ?? "none")}
            <tr>
              <td>
                <span class="mr-2 inline-block size-2.5 rounded-full" style="background: {colorOf(row.id)}" aria-hidden="true"></span>{nameOf(row.id)}
              </td>
              <td class="text-right tabular-nums">{Math.round(row.share * 100)}%</td>
              <td class="text-right tabular-nums text-subtext1">{percent(row.change)}</td>
              <td class="text-right tabular-nums">{formatSpan(row.focusMs)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}
</section>
