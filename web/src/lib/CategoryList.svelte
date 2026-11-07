<script lang="ts">
  import { app } from "./appState.svelte";
  import CategoryChip from "./CategoryChip.svelte";
  import CategoryCreate from "./CategoryCreate.svelte";
  import CategoryEditor from "./CategoryEditor.svelte";
  import { t } from "./i18n";

  /** Every category with its editor, and a form to add one. Used in the manage dialog and in Settings. */
  let editing = $state<string | null>(null);
  let adding = $state(false);
</script>

<div class="grid gap-2">
  {#if app.categories.length === 0 && !adding}<p class="text-subtext1">{t("categories.empty")}</p>{/if}
  {#each app.categories as category (category.id)}
    <div class="rounded-[var(--r-ctl)] bg-surface0/50">
      <div class="flex min-h-12 items-center justify-between gap-3 p-2 pl-3">
        <CategoryChip {category} />
        <button type="button" class="btn btn-gray btn-sm" aria-expanded={editing === category.id} onclick={() => (editing = editing === category.id ? null : category.id)}>
          {editing === category.id ? t("categories.close") : t("categories.edit")}
        </button>
      </div>
      {#if editing === category.id}<CategoryEditor {category} ondeleted={() => (editing = null)} />{/if}
    </div>
  {/each}
  {#if adding}
    <CategoryCreate onadded={() => (adding = false)} oncancel={() => (adding = false)} />
  {:else}
    <button type="button" class="btn btn-filled" onclick={() => (adding = true)}>{t("categories.add")}</button>
  {/if}
</div>
