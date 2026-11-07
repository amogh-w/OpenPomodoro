<script lang="ts">
  import { ICONS } from "./categories";
  import { t } from "./i18n";

  let { value, color, onpick }: { value: string | null | undefined; color: string; onpick: (icon: string) => void } = $props();
</script>

<div class="grid grid-cols-6 gap-2 sm:grid-cols-8" role="group" aria-label={t("categories.icon")}>
  {#each ICONS as icon (icon)}
    <button
      type="button"
      class="grid size-10 place-items-center justify-self-center rounded-[var(--r-ctl)] transition-transform duration-150 ease-ios active:scale-[.97] {value === icon ? 'text-crust' : 'bg-surface0 text-subtext1'}"
      style={value === icon ? `background: var(--ctp-${color})` : ""}
      aria-label={t("categories.icon.label", { name: icon })}
      aria-pressed={value === icon}
      onclick={() => onpick(icon)}
    >
      <svg class="size-5"><use href="/icons.svg#{icon}" /></svg>
    </button>
  {/each}
</div>
