<script lang="ts">
  import { addCategory, app } from "./appState.svelte";
  import CategoryChip from "./CategoryChip.svelte";
  import CategoryCreate from "./CategoryCreate.svelte";
  import CategoryManager from "./CategoryManager.svelte";
  import { STARTERS, orderByRecent } from "./categories";
  import { t, type MessageKey } from "./i18n";
  import { recent } from "./recentStore.svelte";

  interface Props {
    /** The chosen category id; empty for none. */
    value: string;
    onpick: (id: string) => void;
    /** The category is fixed (for example typed as @name), so the choice cannot change. */
    locked?: boolean;
  }
  let { value, onpick, locked = false }: Props = $props();

  const ordered = $derived(orderByRecent(app.categories, recent.sessions));
  let adding = $state(false);
  let adder = $state(false);
  let manager: CategoryManager | undefined = $state();

  /** Add the usual set, one after another, then choose the first. */
  async function addStarters(): Promise<void> {
    adder = true;
    let first: string | null = null;
    for (const s of STARTERS) {
      const id = await addCategory(t(`categories.starter.${s.key}` as MessageKey), s.color, s.icon);
      first ??= id;
    }
    adder = false;
    if (first) onpick(first);
  }
</script>

<CategoryManager bind:this={manager} />

<div class="grid gap-2">
  {#if app.categories.length === 0 && !adding}
    <div class="card grid justify-items-center gap-2 p-4 text-center">
      <p class="t-foot text-subtext1">{t("categories.starter.hint")}</p>
      <div class="flex flex-wrap justify-center gap-2">
        <button type="button" class="btn btn-filled" disabled={adder} onclick={addStarters}>{t("categories.starter")}</button>
        <button type="button" class="btn btn-gray" onclick={() => (adding = true)}>{t("categories.add")}</button>
      </div>
    </div>
  {:else}
    <div class="flex flex-wrap items-center justify-center gap-2" role="group" aria-label={t("timer.category")}>
      {#each ordered as category (category.id)}
        <button
          type="button"
          class="min-h-11 rounded-[var(--r-ctl)] transition-transform duration-150 ease-ios active:scale-[.97] disabled:opacity-60"
          aria-pressed={value === category.id}
          disabled={locked}
          onclick={() => onpick(value === category.id ? "" : category.id)}
        >
          <CategoryChip {category} selected={value === category.id} class="min-h-11" />
        </button>
      {/each}
      {#if !locked}
        <button type="button" class="btn btn-gray btn-sm gap-1" aria-expanded={adding} onclick={() => (adding = !adding)}>
          <svg class="size-4"><use href="/icons.svg#plus" /></svg>{t("categories.new")}
        </button>
        <button type="button" class="btn btn-plain btn-sm" onclick={() => manager?.open()}>{t("categories.manage")}</button>
      {/if}
    </div>
  {/if}
  {#if adding && !locked}
    <CategoryCreate
      onadded={(id) => {
        adding = false;
        onpick(id);
      }}
      oncancel={() => (adding = false)}
    />
  {/if}
</div>
