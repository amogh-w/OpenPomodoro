<script lang="ts">
  import { onMount, tick as afterRender } from "svelte";
  import { timelineBlocks } from "./days";
  import type { CalEvent } from "./ics";
  import { t } from "./i18n";
  import type { Category, SessionRecord } from "./types";

  interface Props {
    sessions: SessionRecord[];
    /** Local midnight at the start of the day, in Unix ms. */
    dayStart: number;
    categories: Category[];
    /** Draw the current-time line (only when the day is today). */
    now?: number | null;
    /** A session about to start now (length in ms), drawn as a hatched block from the current time. */
    plannedMs?: number | null;
    /** Events from the calendar file, drawn as outlines behind the sessions. */
    events?: CalEvent[];
  }
  let { sessions, dayStart, categories, now = null, plannedMs = null, events = [] }: Props = $props();

  const HOUR_PX = 56;
  const DAY_MS = 86_400_000;
  const blocks = $derived(timelineBlocks(sessions, dayStart));
  const hours = Array.from({ length: 24 }, (_, h) => h);
  const nowPct = $derived(now === null ? null : ((now - dayStart) / DAY_MS) * 100);
  const showNow = $derived(nowPct !== null && nowPct >= 0 && nowPct <= 100);
  const plan = $derived(showNow && plannedMs !== null ? { top: nowPct ?? 0, height: (plannedMs / DAY_MS) * 100 } : null);
  const calBlocks = $derived(
    events
      .filter((e) => e.end > dayStart && e.start < dayStart + DAY_MS)
      .map((e) => {
        const start = Math.max(e.start, dayStart);
        const end = Math.min(e.end, dayStart + DAY_MS);
        return { id: e.id, title: e.title, top: ((start - dayStart) / DAY_MS) * 100, height: ((end - start) / DAY_MS) * 100 };
      }),
  );
  const clock = (ms: number): string => new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
  const colorOf = (kind: string, category: string | null): string => {
    if (kind !== "focus") return "var(--ctp-green)";
    return `var(--ctp-${categories.find((c) => c.id === category)?.color ?? "red"})`;
  };

  let scroller: HTMLDivElement | undefined = $state();

  /** Show the part of the day that matters: around now, else the first session, else the morning. */
  async function reveal(): Promise<void> {
    await afterRender();
    if (!scroller) return;
    const focusHour = showNow
      ? ((now ?? dayStart) - dayStart) / 3_600_000
      : blocks.length > 0
        ? ((blocks[0]?.left ?? 0) / 100) * 24
        : 8;
    scroller.scrollTop = Math.max(0, (focusHour - 2) * HOUR_PX);
  }
  onMount(() => void reveal());
  // Scroll again when another day is picked.
  $effect(() => {
    void dayStart;
    void reveal();
  });
</script>

<!-- A scrollable region must be reachable by keyboard, so it takes focus. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="relative max-h-[26rem] overflow-y-auto rounded-lg bg-surface0/40" bind:this={scroller} tabindex="0" role="region" aria-label={t("history.timeline")}>
  <div class="relative ml-12 mr-2" style="height: {24 * HOUR_PX}px">
    {#each hours as h (h)}
      <div class="absolute inset-x-0 border-t border-surface1/70" style="top: {(h / 24) * 100}%">
        <span class="t-foot absolute -left-12 -top-2.5 w-10 text-right tabular-nums text-subtext1" aria-hidden="true">{String(h).padStart(2, "0")}:00</span>
      </div>
    {/each}
    {#each calBlocks as ev (ev.id)}
      <div
        class="absolute inset-x-1 overflow-hidden rounded-md border border-dashed border-overlay1 bg-surface1/30 px-1.5 text-[11px] leading-tight text-subtext1"
        style="top: {ev.top}%; height: max({ev.height}%, 4px)"
        title={ev.title}
      >
        {ev.title}
      </div>
    {/each}
    {#each blocks as block (block.id)}
      <div
        class="absolute inset-x-1 rounded-md {block.kind === 'focus' ? '' : 'opacity-60'}"
        style="top: {block.left}%; height: max({block.width}%, 4px); background: {colorOf(block.kind, block.category)}"
      ></div>
    {/each}
    {#if plan}
      <div
        class="absolute inset-x-1 rounded-md border border-accent/60"
        style="top: {plan.top}%; height: max({plan.height}%, 6px); background: repeating-linear-gradient(135deg, color-mix(in oklab, var(--accent) 45%, transparent) 0 6px, color-mix(in oklab, var(--accent) 20%, transparent) 6px 12px)"
        aria-hidden="true"
      ></div>
    {/if}
    {#if showNow && now !== null}
      <div class="absolute inset-x-0 z-10 border-t-2 border-red" style="top: {nowPct}%">
        <span class="t-foot absolute -left-12 -top-3 rounded bg-red px-1 font-semibold tabular-nums text-crust">{clock(now)}</span>
      </div>
    {/if}
  </div>
</div>
