<script lang="ts">
  import { app, removeCategory, updateCategory } from "./appState.svelte";
  import CategoryExtras from "./CategoryExtras.svelte";
  import ColorSwatches from "./ColorSwatches.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import IconGrid from "./IconGrid.svelte";
  import { iconOf } from "./categories";
  import { t } from "./i18n";
  import type { Category } from "./types";

  /** Edits an existing category; every change is saved as soon as it is made. */
  let { category, ondeleted }: { category: Category; ondeleted?: (id: string) => void } = $props();

  let problem = $state<string | null>(null);
  let confirm: ConfirmDialog | undefined = $state();

  async function save(change: { name?: string; color?: string; icon?: string | null }): Promise<void> {
    problem = null;
    if (!(await updateCategory(category.id, change))) problem = t("categories.failed");
  }

  async function remove(): Promise<void> {
    if (!(await confirm?.ask())) return;
    problem = null;
    const id = category.id;
    if (await removeCategory(id)) ondeleted?.(id);
    else problem = t("categories.failed");
  }
</script>

<ConfirmDialog bind:this={confirm} title={t("categories.delete.title", { name: category.name })} body={t("categories.delete.body")} />

<div class="grid gap-3 px-3 pb-3">
  <label class="grid gap-1.5">
    <span class="t-foot text-subtext1">{t("categories.name")}</span>
    <input
      class="field"
      value={category.name}
      maxlength="40"
      onchange={(e) => {
        const name = e.currentTarget.value.trim();
        if (name !== "") void save({ name });
        else e.currentTarget.value = category.name;
      }}
    />
  </label>
  <ColorSwatches value={category.color ?? ""} onpick={(color) => save({ color })} />
  <IconGrid value={iconOf(category)} color={category.color ?? "blue"} onpick={(icon) => save({ icon })} />
  <CategoryExtras {category} onproblem={(text) => (problem = text)} />
  {#if problem}<div class="alert tone-danger anim m-shake" role="alert">{problem}</div>{/if}
  <button type="button" class="btn btn-danger-tinted btn-sm justify-self-start" onclick={remove}>{t("categories.delete")}</button>
</div>
