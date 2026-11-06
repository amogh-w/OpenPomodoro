<script lang="ts">
  import { formatSpan } from "./format";
  import { buildHeatmap } from "./heatmap";
  import { t } from "./i18n";
  import type { DayTotal } from "./types";

  let { totals, todayKey }: { totals: DayTotal[]; todayKey: string } = $props();

  const weeks = $derived(buildHeatmap(totals, todayKey));
  const total = $derived(weeks.flat().reduce((sum, c) => sum + c.focusMs, 0));
  const activeDays = $derived(weeks.flat().filter((c) => c.level > 0).length);
  // How strongly each level is tinted; the colour itself is the theme accent.
  const strength = [0, 25, 45, 70, 100];
  const label = (day: string, ms: number): string =>
    `${new Date(`${day}T12:00:00`).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short", year: "numeric" })}: ${ms > 0 ? formatSpan(ms) : t("heatmap.none")}`;

  let scroller: HTMLDivElement | undefined = $state();
  // Start scrolled to the latest weeks, which are at the right edge.
  $effect(() => {
    if (scroller && weeks.length > 0) scroller.scrollLeft = scroller.scrollWidth;
  });
</script>

<section class="card grid gap-3 p-4" aria-labelledby="heatmap-title">
  <div class="flex flex-wrap items-baseline justify-between gap-2">
    <h2 id="heatmap-title" class="t-title3">{t("heatmap.title")}</h2>
    <p class="t-foot text-subtext1">{t("heatmap.summary", { time: formatSpan(total), days: activeDays })}</p>
  </div>
  <!-- A scrollable region must be reachable by keyboard, so it takes focus. -->
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div class="overflow-x-auto pb-1" bind:this={scroller} tabindex="0" role="region" aria-label={t("heatmap.title")}>
    <div class="flex w-max gap-[3px]">
      {#each weeks as week (week[0]?.day)}
        <div class="grid gap-[3px]">
          {#each week as cell (cell.day)}
            <div
              class="size-3 rounded-[3px] {cell.future ? 'opacity-0' : 'bg-surface0'}"
              style={cell.level > 0 ? `background: color-mix(in oklab, var(--accent) ${strength[cell.level]}%, var(--ctp-surface0))` : ""}
              title={cell.future ? "" : label(cell.day, cell.focusMs)}
              aria-label={cell.future ? undefined : label(cell.day, cell.focusMs)}
              role={cell.future ? undefined : "img"}
            ></div>
          {/each}
        </div>
      {/each}
    </div>
  </div>
  <div class="t-foot flex items-center justify-end gap-1.5 text-subtext1" aria-hidden="true">
    {t("heatmap.less")}
    {#each strength as pct, i (i)}
      <span class="size-3 rounded-[3px] bg-surface0" style={pct > 0 ? `background: color-mix(in oklab, var(--accent) ${pct}%, var(--ctp-surface0))` : ""}></span>
    {/each}
    {t("heatmap.more")}
  </div>
</section>
