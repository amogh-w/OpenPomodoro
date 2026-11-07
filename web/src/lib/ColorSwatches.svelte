<script lang="ts">
  import { t, type MessageKey } from "./i18n";
  import { accents } from "./theme";

  let { value, onpick }: { value: string; onpick: (color: string) => void } = $props();
</script>

<div class="grid grid-cols-6 gap-2" role="group" aria-label={t("categories.color")}>
  {#each accents as color (color)}
    <button
      type="button"
      class="size-9 justify-self-center rounded-full ring-text ring-offset-2 ring-offset-base transition-transform duration-150 ease-ios active:scale-[.97] {value === color ? 'ring-2' : ''}"
      style="background: var(--ctp-{color})"
      aria-label={t(`accent.${color}` as MessageKey)}
      aria-pressed={value === color}
      onclick={() => onpick(color)}
    ></button>
  {/each}
</div>
