<script lang="ts">
  import { addCategory, app } from "./appState.svelte";
  import CategoryChip from "./CategoryChip.svelte";
  import CategoryManager from "./CategoryManager.svelte";
  import { STARTERS, iconOf, orderByRecent } from "./categories";
  import { t, type MessageKey } from "./i18n";
  import { recent } from "./recentStore.svelte";

  interface Props {
    /** The chosen category id; empty for none. */
    value: string;
    onpick: (id: string) => void;
    /** The category is fixed (for example typed as @name). */
    locked?: boolean;
    /** Offer a new category, the usual set and the editor too; off for the small per-task selectors. */
    extras?: boolean;
    /** Opens the quick form for a new category (the parent shows it). Needed with `extras`. */
    onnew?: () => void;
  }
  let { value, onpick, onnew, locked = false, extras = true }: Props = $props();

  const uid = `catmenu-${Math.random().toString(36).slice(2, 9)}`;
  const ordered = $derived(orderByRecent(app.categories, recent.sessions));
  const current = $derived(app.categories.find((c) => c.id === value));
  let manager: CategoryManager | undefined = $state();
  let adder = $state(false);

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

{#if extras}<CategoryManager bind:this={manager} />{/if}

<!-- One button that shows the category and opens the list; the list also makes a new one and opens the editor. -->
<button type="button" class="rounded-[var(--r-ctl)] disabled:opacity-60" data-menu={uid} disabled={locked} aria-haspopup="menu" aria-label={t("timer.category")} title={current?.name ?? t("timer.category.none")}>
  <span class="inline-flex items-center gap-1">
    <CategoryChip category={current} small iconOnlyNarrow class="min-h-9 px-3" />
    <svg class="size-4 text-subtext1" aria-hidden="true"><use href="/icons.svg#chevron-down" /></svg>
  </span>
</button>
<div id={uid} popover class="menu">
  <button type="button" class="menu-item" data-menu-close onclick={() => onpick("")}>
    <svg class="size-5 text-subtext0"><use href="/icons.svg#layers" /></svg><span class="flex-1">{t("timer.category.none")}</span>
    {#if value === ""}<svg class="size-5 text-accent"><use href="/icons.svg#check" /></svg>{/if}
  </button>
  {#each ordered as category (category.id)}
    <button type="button" class="menu-item" data-menu-close onclick={() => onpick(category.id)}>
      <svg class="size-5" style="color: var(--ctp-{category.color ?? 'overlay1'})"><use href="/icons.svg#{iconOf(category)}" /></svg>
      <span class="min-w-0 flex-1 truncate">{category.name}</span>
      {#if value === category.id}<svg class="size-5 text-accent"><use href="/icons.svg#check" /></svg>{/if}
    </button>
  {/each}
  {#if extras}<div class="menu-sep"></div>{/if}
  {#if extras && app.categories.length === 0}
    <button type="button" class="menu-item" data-menu-close disabled={adder} onclick={addStarters}>
      <svg class="size-5 text-subtext0"><use href="/icons.svg#sparkles" /></svg>{t("categories.starter")}
    </button>
  {/if}
  {#if extras}
  <button type="button" class="menu-item" data-menu-close onclick={() => onnew?.()}>
    <svg class="size-5 text-subtext0"><use href="/icons.svg#plus" /></svg>{t("categories.add")}
  </button>
  {/if}
  {#if extras && app.categories.length > 0}
    <button type="button" class="menu-item" data-menu-close onclick={() => manager?.open()}>
      <svg class="size-5 text-subtext0"><use href="/icons.svg#sliders-horizontal" /></svg>{t("categories.manage")}
    </button>
  {/if}
</div>
