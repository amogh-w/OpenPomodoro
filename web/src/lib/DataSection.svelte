<script lang="ts">
  import { api } from "./api";
  import { refresh } from "./appState.svelte";
  import { downloadBackup, restoreBackup } from "./backupFile";
  import { localDayKey } from "./days";
  import { errorMessage } from "./errors";
  import { clearDraft } from "./draft.svelte";
  import { t } from "./i18n";
  import { refreshRecent } from "./recentStore.svelte";
  import { applyThemeKey } from "./theme";

  let notice = $state<{ tone: "success" | "danger"; text: string } | null>(null);
  let busy = $state(false);

  let dialog: HTMLDialogElement | undefined = $state();
  let password = $state("");
  let wipeError = $state<string | null>(null);

  const say = (tone: "success" | "danger", text: string): void => {
    notice = { tone, text };
  };

  async function download(): Promise<void> {
    notice = null;
    if (!(await downloadBackup(localDayKey(Date.now()).replaceAll("-", "")))) say("danger", t("error.unknown"));
  }

  async function restore(event: Event): Promise<void> {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    input.value = ""; // so choosing the same file again still fires
    if (!file) return;
    busy = true;
    notice = null;
    try {
      const done = await restoreBackup(await file.text(), applyThemeKey);
      await Promise.all([refresh(), refreshRecent()]);
      const skipped = done.skipped > 0 ? ` ${t("data.skipped", { n: done.skipped })}` : "";
      say("success", `${t("data.restored", { sessions: done.sessions, categories: done.categories })}${skipped}`);
    } catch (e) {
      say("danger", errorMessage(e));
    } finally {
      busy = false;
    }
  }

  async function wipe(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    busy = true;
    wipeError = null;
    try {
      await api.wipeData(password);
      password = "";
      dialog?.close();
      clearDraft();
      await Promise.all([refresh(), refreshRecent()]);
      say("success", t("data.wiped"));
    } catch (e) {
      wipeError = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

<section class="card mt-4 grid gap-3 p-4" aria-labelledby="data-title">
  <h2 id="data-title" class="t-title3">{t("data.title")}</h2>
  <p class="t-foot text-subtext1">{t("data.hint")}</p>

  {#if notice}
    <div class="alert {notice.tone === 'success' ? 'tone-success' : 'tone-danger'}" role="status">
      <svg class="size-5 shrink-0"><use href="/icons.svg#{notice.tone === 'success' ? 'circle-check' : 'circle-x'}" /></svg>
      <div>{notice.text}</div>
    </div>
  {/if}

  <button class="btn btn-gray" onclick={download}>{t("data.download")}</button>
  <label class="btn btn-gray cursor-pointer {busy ? 'opacity-60' : ''}">
    {t("data.restore")}
    <input class="sr-only" type="file" accept="application/json,.json" disabled={busy} onchange={restore} />
  </label>
  <button class="btn btn-danger-tinted" onclick={() => dialog?.showModal()}>{t("data.wipe")}</button>
</section>

<dialog bind:this={dialog} class="modal" aria-labelledby="wipe-title">
  <form class="grid gap-3 p-5" onsubmit={wipe}>
    <h2 id="wipe-title" class="t-title3">{t("data.wipe")}</h2>
    <p class="text-subtext1">{t("data.wipe.warning")}</p>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("data.wipe.confirm")}</span>
      <input class="field" type="password" bind:value={password} autocomplete="current-password" required />
    </label>
    {#if wipeError}<div class="alert tone-danger" role="alert">{wipeError}</div>{/if}
    <div class="grid grid-cols-2 gap-2">
      <button class="btn btn-gray" type="button" onclick={() => dialog?.close()}>{t("common.cancel")}</button>
      <button class="btn btn-danger" type="submit" disabled={busy}>{t("data.wipe.forever")}</button>
    </div>
  </form>
</dialog>
