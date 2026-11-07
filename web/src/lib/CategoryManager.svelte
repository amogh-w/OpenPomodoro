<script lang="ts">
  import { onMount } from "svelte";
  import CategoryList from "./CategoryList.svelte";
  import { t } from "./i18n";

  let dialog: HTMLDialogElement | undefined = $state();
  // Several managers can exist on one page (one per editor), so the heading id is unique.
  const uid = `manage-${Math.random().toString(36).slice(2, 9)}`;

  // The picker sits inside the timer's form, so the dialog moves to the page body: its buttons and Enter key
  // then cannot submit that form.
  onMount(() => {
    const node = dialog;
    if (!node) return;
    document.body.appendChild(node);
    return () => node.remove();
  });

  export function open(): void {
    dialog?.showModal();
  }
</script>

<dialog bind:this={dialog} class="modal" aria-labelledby={uid}>
  <div class="grid max-h-[85dvh] grid-rows-[auto_minmax(0,1fr)] gap-3 p-5">
    <div class="flex items-center justify-between gap-3">
      <h2 id={uid} class="t-title3">{t("categories.title")}</h2>
      <button type="button" class="btn btn-gray size-11 !px-0" aria-label={t("common.close")} onclick={() => dialog?.close()}>
        <svg class="size-5"><use href="/icons.svg#x" /></svg>
      </button>
    </div>
    <div class="overflow-y-auto"><CategoryList /></div>
  </div>
</dialog>
