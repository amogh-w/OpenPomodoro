<script lang="ts">
  import { timelineBlocks } from "./days";
  import { t } from "./i18n";
  import type { Category, SessionRecord } from "./types";

  interface Props {
    sessions: SessionRecord[];
    /** Local midnight at the start of the day, in Unix ms. */
    dayStart: number;
    categories: Category[];
    /** Show a marker for this moment (only when the day is today). */
    now?: number | null;
  }
  let { sessions, dayStart, categories, now = null }: Props = $props();

  const blocks = $derived(timelineBlocks(sessions, dayStart));
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
  <div class="t-foot flex justify-between text-subtext1 tabular-nums" aria-hidden="true">
    <span>0</span><span>6</span><span>12</span><span>18</span><span>24</span>
  </div>
</div>
