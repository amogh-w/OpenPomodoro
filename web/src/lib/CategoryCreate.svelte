<script lang="ts">
  import { addCategory } from "./appState.svelte";
  import ColorSwatches from "./ColorSwatches.svelte";
  import IconGrid from "./IconGrid.svelte";
  import { t } from "./i18n";

  interface Props {
    /** Called with the new category's id. */
    onadded: (id: string) => void;
    oncancel: () => void;
  }
  let { onadded, oncancel }: Props = $props();

  let name = $state("");
  let color = $state("blue");
  let icon = $state("tag");
  let problem = $state<string | null>(null);

  async function submit(): Promise<void> {
    const text = name.trim();
    if (text === "") return;
    problem = null;
    const id = await addCategory(text, color, icon);
    if (id) {
      name = "";
      onadded(id);
    } else problem = t("categories.failed");
  }
</script>

<!-- Not a <form>: this sits inside the timer form, whose Enter key must not start a session. -->
<div class="card grid gap-3 p-3 text-left">
  <!-- svelte-ignore a11y_autofocus -->
  <input class="field" bind:value={name} maxlength="40" placeholder={t("categories.name")} aria-label={t("categories.name")} autocomplete="off"
    autofocus
    onkeydown={(e) => {
      if (e.key !== "Enter") return;
      e.preventDefault();
      void submit();
    }}
  />
  <ColorSwatches value={color} onpick={(c) => (color = c)} />
  <IconGrid value={icon} {color} onpick={(i) => (icon = i)} />
  {#if problem}<div class="alert tone-danger anim m-shake" role="alert">{problem}</div>{/if}
  <div class="grid grid-cols-2 gap-2">
    <button class="btn btn-gray" type="button" onclick={oncancel}>{t("common.cancel")}</button>
    <button class="btn btn-filled" type="button" disabled={name.trim() === ""} onclick={() => void submit()}>{t("categories.add")}</button>
  </div>
</div>
