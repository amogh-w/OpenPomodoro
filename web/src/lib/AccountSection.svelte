<script lang="ts">
  import { api } from "./api";
  import { errorMessage } from "./errors";
  import { t, tn } from "./i18n";
  import { app } from "./appState.svelte";
  import { accountGone, session, signOut } from "./session.svelte";

  let current = $state("");
  let next = $state("");
  let changing = $state(false);
  let notice = $state<{ tone: "success" | "danger"; text: string } | null>(null);

  let signOutDialog: HTMLDialogElement | undefined = $state();
  let dialog: HTMLDialogElement | undefined = $state();
  let deletePassword = $state("");
  let deleteError = $state<string | null>(null);
  let deleting = $state(false);

  async function changePassword(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    changing = true;
    notice = null;
    try {
      await api.changePassword(current, next);
      current = next = "";
      notice = { tone: "success", text: t("account.passwordChanged") };
    } catch (e) {
      notice = { tone: "danger", text: errorMessage(e) };
    } finally {
      changing = false;
    }
  }

  /** Signs out; if something done offline has not been sent, asks first. */
  async function trySignOut(): Promise<void> {
    if ((await signOut()) === "pending") signOutDialog?.showModal();
  }

  async function forceSignOut(): Promise<void> {
    signOutDialog?.close();
    await signOut(true);
  }

  async function deleteAccount(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    deleting = true;
    deleteError = null;
    try {
      await api.deleteAccount(deletePassword);
      dialog?.close();
      deletePassword = "";
      await accountGone();
    } catch (e) {
      deleteError = errorMessage(e);
    } finally {
      deleting = false;
    }
  }
</script>

<section class="card mt-4 grid gap-4 p-4" aria-labelledby="account">
  <div>
    <h2 id="account" class="t-title3">{t("account.title")}</h2>
    <p class="mt-1 text-subtext1">{t("account.signedInAs", { name: session.user?.username ?? "" })}</p>
  </div>

  {#if session.user?.role === "admin"}
    <a class="btn btn-gray" href="#/admin">{t("account.manageUsers")}</a>
  {/if}

  <form class="grid gap-3" onsubmit={changePassword}>
    <h3 class="t-headline">{t("account.changePassword")}</h3>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("account.currentPassword")}</span>
      <input class="field" type="password" bind:value={current} autocomplete="current-password" required />
    </label>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("account.newPassword")}</span>
      <input class="field" type="password" bind:value={next} autocomplete="new-password" required />
    </label>
    {#if notice}
      <div class="alert {notice.tone === 'success' ? 'tone-success' : 'tone-danger'}" role="status">
        <svg class="size-5 shrink-0">
          <use href="/icons.svg#{notice.tone === 'success' ? 'circle-check' : 'circle-x'}" />
        </svg>
        <div>{notice.text}</div>
      </div>
    {/if}
    <button class="btn btn-gray" type="submit" disabled={changing}>{t("account.changePassword")}</button>
  </form>

  <div class="grid gap-2">
    <button class="btn btn-gray" onclick={trySignOut}>{t("account.signOut")}</button>
    <button class="btn btn-danger" onclick={() => dialog?.showModal()}>{t("account.delete")}</button>
  </div>
</section>

<dialog bind:this={dialog} class="modal" aria-labelledby="delete-title">
  <form class="grid gap-3 p-5" onsubmit={deleteAccount}>
    <h2 id="delete-title" class="t-title3">{t("account.delete")}</h2>
    <p class="text-subtext1">{t("account.deleteWarning")}</p>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("account.deleteConfirm")}</span>
      <input class="field" type="password" bind:value={deletePassword} autocomplete="current-password" required />
    </label>
    {#if deleteError}
      <div class="alert tone-danger" role="alert">
        <svg class="size-5 shrink-0"><use href="/icons.svg#circle-x" /></svg>
        <div>{deleteError}</div>
      </div>
    {/if}
    <div class="grid grid-cols-2 gap-2">
      <button class="btn btn-gray" type="button" onclick={() => dialog?.close()}>{t("common.cancel")}</button>
      <button class="btn btn-danger" type="submit" disabled={deleting}>{t("account.deleteForever")}</button>
    </div>
  </form>
</dialog>

<dialog bind:this={signOutDialog} class="modal" aria-labelledby="signout-title">
  <div class="grid gap-3 p-5">
    <h2 id="signout-title" class="t-title3">{t("account.pendingTitle")}</h2>
    <p class="text-subtext1">{tn("account.pendingWarning", app.pending)}</p>
    <div class="grid grid-cols-2 gap-2">
      <button class="btn btn-gray" onclick={() => signOutDialog?.close()}>{t("account.stayHere")}</button>
      <button class="btn btn-danger" onclick={forceSignOut}>{t("account.signOutAnyway")}</button>
    </div>
  </div>
</dialog>
