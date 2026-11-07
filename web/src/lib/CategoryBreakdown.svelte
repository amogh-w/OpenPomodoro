<script lang="ts">
  import { timePerCategory } from "./categories";
  import { formatSpan } from "./format";
  import { t } from "./i18n";
  import CategoryChip from "./CategoryChip.svelte";
  import type { Category, SessionRecord } from "./types";

  /** How a set of sessions splits across categories: a bar with one segment each, and a legend with the time. */
  let { sessions, categories }: { sessions: SessionRecord[]; categories: Category[] } = $props();

  const rows = $derived(timePerCategory(sessions));
  const total = $derived(rows.reduce((sum, r) => sum + r.ms, 0));
  const find = (id: string | null): Category | undefined => categories.find((c) => c.id === id);
  const colorOf = (id: string | null): string => `var(--ctp-${find(id)?.color ?? "overlay1"})`;
</script>

{#if rows.length > 0}
  <div class="grid gap-2" role="group" aria-label={t("history.byCategory")}>
    <div class="flex h-2 gap-0.5 overflow-hidden rounded-full bg-surface0" aria-hidden="true">
      {#each rows as row (row.id ?? "none")}
        <div class="h-full min-w-1" style="flex: {row.ms} 1 0; background: {colorOf(row.id)}"></div>
      {/each}
    </div>
    <div class="flex flex-wrap gap-1.5">
      {#each rows as row (row.id ?? "none")}
        <CategoryChip category={find(row.id)} small detail={total > 0 ? formatSpan(row.ms) : ""} />
      {/each}
    </div>
  </div>
{/if}
