<script lang="ts">
  import { app, updateCategory } from "./appState.svelte";
  import { t } from "./i18n";
  import type { Category } from "./types";

  // A category's group (for reports) and its own usual lengths. Empty means "none": the general settings apply.
  let { category, onproblem }: { category: Category; onproblem?: (text: string | null) => void } = $props();

  const groups = $derived([...new Set(app.categories.map((c) => c.group).filter((g): g is string => typeof g === "string" && g !== ""))].sort());
  const listId = $derived(`groups-${category.id}`);

  async function save(change: { group?: string | null; focus_min?: number | null; break_min?: number | null }): Promise<void> {
    onproblem?.(null);
    if (!(await updateCategory(category.id, change))) onproblem?.(t("categories.failed"));
  }

  /** A whole number of minutes in range, or null when the box is empty; anything else changes nothing. */
  function minutes(text: string, max: number): number | null | undefined {
    if (text.trim() === "") return null;
    const n = Number(text);
    return Number.isInteger(n) && n >= 1 && n <= max ? n : undefined;
  }
</script>

<div class="grid gap-3">
  <label class="grid gap-1.5">
    <span class="t-foot text-subtext1">{t("categories.group")}</span>
    <input
      class="field"
      value={category.group ?? ""}
      maxlength="40"
      list={listId}
      placeholder={t("categories.group.placeholder")}
      onchange={(e) => void save({ group: e.currentTarget.value.trim() === "" ? null : e.currentTarget.value.trim() })}
    />
    <datalist id={listId}>{#each groups as g (g)}<option value={g}></option>{/each}</datalist>
  </label>
  <div class="grid grid-cols-2 gap-3">
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("categories.focusMin")}</span>
      <input
        class="field"
        inputmode="numeric"
        value={category.focus_min ?? ""}
        placeholder={t("categories.default")}
        onchange={(e) => {
          const m = minutes(e.currentTarget.value, 180);
          if (m === undefined) e.currentTarget.value = String(category.focus_min ?? "");
          else void save({ focus_min: m });
        }}
      />
    </label>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("categories.breakMin")}</span>
      <input
        class="field"
        inputmode="numeric"
        value={category.break_min ?? ""}
        placeholder={t("categories.default")}
        onchange={(e) => {
          const m = minutes(e.currentTarget.value, 60);
          if (m === undefined) e.currentTarget.value = String(category.break_min ?? "");
          else void save({ break_min: m });
        }}
      />
    </label>
  </div>
</div>
