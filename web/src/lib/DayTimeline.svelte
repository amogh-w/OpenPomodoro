<script lang="ts">
  import { timelineBlocks } from "./days";
  import type { CalEvent } from "./ics";
  import { t } from "./i18n";
  import type { Category, SessionRecord } from "./types";

  interface Props {
    sessions: SessionRecord[];
    /** Local midnight at the start of the day, in Unix ms. */
    dayStart: number;
    categories: Category[];
    /** Show a marker for this moment (only when the day is today). */
    now?: number | null;
    /** Events from the calendar file, drawn as outlines in a thin lane under the sessions. */
    events?: CalEvent[];
  }
  let { sessions, dayStart, categories, now = null, events = [] }: Props = $props();

  const blocks = $derived(timelineBlocks(sessions, dayStart));
  const DAY_MS = 86_400_000;
  const calBlocks = $derived(
    events
      .filter((e) => e.end > dayStart && e.start < dayStart + DAY_MS)
      .map((e) => {
        const start = Math.max(e.start, dayStart);
        const end = Math.min(e.end, dayStart + DAY_MS);
        return { id: e.id, title: e.title, left: ((start - dayStart) / DAY_MS) * 100, width: ((end - start) / DAY_MS) * 100 };
      }),
  );
  const marker = $derived(now === null ? null : ((now - dayStart) / 86_400_000) * 100);
  const colorOf = (kind: string, category: string | null): string => {
    if (kind !== "focus") return "var(--ctp-green)";
    const color = categories.find((c) => c.id === category)?.color;
    return `var(--ctp-${color ?? "red"})`;
  };
</script>

<div class="grid gap-1">
  <div class="relative h-9 overflow-hidden rounded-lg bg-surface0" role="img" aria-label={t("history.timeline")}>
    {#each blocks as block (block.id)}
      <div
        class="absolute inset-y-1 rounded-sm {block.kind === 'focus' ? '' : 'opacity-60'}"
        style="left: {block.left}%; width: {block.width}%; background: {colorOf(block.kind, block.category)}"
      ></div>
    {/each}
    {#if marker !== null && marker >= 0 && marker <= 100}
      <div class="absolute inset-y-0 w-0.5 bg-text" style="left: {marker}%"></div>
    {/if}
  </div>
  {#if calBlocks.length > 0}
    <div class="relative h-3 rounded-sm bg-surface0/40" aria-hidden="true">
      {#each calBlocks as ev (ev.id)}
        <div class="absolute inset-y-0.5 rounded-sm border border-dashed border-overlay1 bg-surface1/30" style="left: {ev.left}%; width: max({ev.width}%, 2px)" title={ev.title}></div>
      {/each}
    </div>
  {/if}
  <div class="t-foot flex justify-between text-subtext1 tabular-nums" aria-hidden="true">
    <span>0</span><span>6</span><span>12</span><span>18</span><span>24</span>
  </div>
</div>
