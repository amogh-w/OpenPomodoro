<script lang="ts">
  import { iconOf } from "./categories";
  import { t } from "./i18n";
  import type { Category } from "./types";

  interface Props {
    /** The category to show; leave out for "no category". */
    category?: Category;
    /** Filled with the category colour (the chosen one), else a soft tint. */
    selected?: boolean;
    /** Smaller, for rows and tables. */
    small?: boolean;
    /** Muted text after the name, such as a time. */
    detail?: string;
    /** Show only the icon on a narrow screen (the name appears from the `sm` width up). */
    iconOnlyNarrow?: boolean;
    class?: string;
  }
  let { category, selected = false, small = false, detail = "", iconOnlyNarrow = false, class: extra = "" }: Props = $props();

  const color = $derived(`var(--ctp-${category?.color ?? "overlay1"})`);
  // Colour from the theme only. The tint keeps the text readable in Latte and Mocha; the icon carries the colour.
  const look = $derived(
    selected
      ? `background: ${color}; color: var(--ctp-crust); box-shadow: inset 0 0 0 1px ${color}`
      : `background: color-mix(in oklab, ${color} 14%, transparent); color: var(--ctp-text); box-shadow: inset 0 0 0 1px color-mix(in oklab, ${color} 38%, transparent)`,
  );
  const iconStyle = $derived(selected ? "" : `color: ${color}`);
</script>

<span
  class="inline-flex max-w-full items-center gap-1.5 rounded-[var(--r-ctl)] font-medium {small ? 't-foot px-2 py-0.5' : 'px-3 py-1.5'} {extra}"
  style={look}
>
  <svg class="shrink-0 {small ? 'size-3.5' : 'size-4'}" style={iconStyle} aria-hidden="true"><use href="/icons.svg#{category ? iconOf(category) : 'layers'}" /></svg>
  <span class="truncate {iconOnlyNarrow ? 'max-sm:hidden' : ''}">{category ? category.name : t("timer.category.none")}</span>
  {#if detail !== ""}<span class="tabular-nums {selected ? 'opacity-80' : 'text-subtext1'} font-normal">{detail}</span>{/if}
</span>
