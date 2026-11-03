<script lang="ts">
  import { app, sendCommand } from "./appState.svelte";
  import { t } from "./i18n";

  let dialog: HTMLDialogElement | undefined = $state();
  let intention = $state("");
  let categoryId = $state("");
  let problem = $state<string | null>(null);
  let busy = $state(false);

  /** Open it with what the session is about now. */
  export function show(): void {
    intention = app.timer?.intention ?? "";
    categoryId = app.timer?.category ?? "";
    problem = null;
    dialog?.showModal();
  }

  async function save(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    busy = true;
    problem = null;
    try {
      const result = await sendCommand({
        name: "edit_session",
        ...(intention.trim() !== "" ? { intention: intention.trim() } : {}),
        ...(categoryId !== "" ? { category: categoryId } : {}),
      });
      if (result.ok || result.code === "network") dialog?.close();
      else problem = t("timer.error");
    } finally {
      busy = false;
    }
  }
</script>

<dialog bind:this={dialog} class="modal" aria-labelledby="edit-running-title">
  <form class="grid gap-3 p-5" onsubmit={save}>
    <h2 id="edit-running-title" class="t-title3">{t("timer.editSession")}</h2>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("timer.intention.label")}</span>
      <input class="field" bind:value={intention} maxlength="200" autocomplete="off" />
    </label>
    {#if app.categories.length > 0}
      <label class="grid gap-1.5">
        <span class="t-foot text-subtext1">{t("timer.category")}</span>
        <select class="field" bind:value={categoryId}>
          <option value="">{t("timer.category.none")}</option>
          {#each app.categories as category (category.id)}
            <option value={category.id}>{category.name}</option>
          {/each}
        </select>
      </label>
    {/if}
    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}
    <div class="grid grid-cols-2 gap-2">
      <button class="btn btn-gray" type="button" onclick={() => dialog?.close()}>{t("common.cancel")}</button>
      <button class="btn btn-filled" type="submit" disabled={busy}>{t("history.add.save")}</button>
    </div>
  </form>
</dialog>
