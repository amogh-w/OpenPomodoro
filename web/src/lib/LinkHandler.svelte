<script lang="ts">
  import { app, currentView, sendCommand } from "./appState.svelte";
  import { attachDraft, setNotes } from "./draft.svelte";
  import { t } from "./i18n";
  import { type LinkAction, commandFor } from "./links";
  import { links } from "./router.svelte";
  import { session } from "./session.svelte";
  import { prefs, setAllowLinks } from "../platform/prefs.svelte";

  let dialog: HTMLDialogElement | undefined = $state();
  let always = $state(false);

  const pending = $derived(links.pending);
  const ready = $derived(session.status === "signed_in" && app.ready);

  /** What the link would do, in words, for the confirmation. */
  function describe(action: LinkAction): string {
    switch (action.kind) {
      case "start": {
        const parts = [
          action.intention !== null ? `“${action.intention}”` : null,
          action.durationMs !== null ? t("timer.hint.length", { minutes: Math.round(action.durationMs / 60_000) }) : null,
          action.category !== null ? t("timer.hint.category", { name: action.category }) : null,
        ].filter((p): p is string => p !== null);
        return `${t("links.ask.start")}${parts.length > 0 ? ` ${parts.join(" · ")}` : ""}`;
      }
      case "pause":
        return t("links.ask.pause");
      case "resume":
        return t("links.ask.resume");
      case "break":
        return t("links.ask.break");
      case "finish":
        return t("links.ask.finish");
      case "navigate":
        return "";
    }
  }

  /** Go to the link's own callback address afterwards (only ever after the person saw and agreed to it). */
  function callBack(action: LinkAction, honour: boolean, ok: boolean, code?: string): void {
    if (!honour || action.kind === "navigate" || !action.callbacks) return;
    const target = ok ? action.callbacks.success : action.callbacks.error;
    if (target === null) return;
    const url = new URL(target);
    if (!ok && code) url.searchParams.set("error", code);
    location.assign(url.href);
  }

  /** Where the link says it will return to, for the confirmation. */
  function returnHost(action: LinkAction): string | null {
    if (action.kind === "navigate" || !action.callbacks) return null;
    const target = action.callbacks.success ?? action.callbacks.error;
    return target === null ? null : new URL(target).host;
  }

  async function carryOut(action: LinkAction, honourCallbacks = false): Promise<void> {
    links.pending = null;
    const view = currentView();
    const planned = view ? commandFor(action, view, app.categories) : { unavailable: true as const };
    if ("unavailable" in planned) {
      app.notice = t("links.unavailable");
      callBack(action, honourCallbacks, false, "unavailable");
      return;
    }
    const result = await sendCommand(planned.command);
    if (!result.ok) {
      app.notice = t("links.failed");
      callBack(action, honourCallbacks, false, result.code);
      return;
    }
    callBack(action, honourCallbacks, true);
    // Notes from a start link become the session's draft notes.
    const key = app.timer?.focus_started_at;
    if (planned.notes !== null && key != null) {
      attachDraft(key);
      setNotes(planned.notes);
    }
  }

  // Wait until the person is signed in and the app has its state, then act or ask.
  $effect(() => {
    if (!pending || !ready) return;
    if (prefs.allowLinks) void carryOut(pending);
    else if (dialog && !dialog.open) dialog.showModal();
  });

  function allow(): void {
    if (always) setAllowLinks(true);
    dialog?.close();
    if (pending) void carryOut(pending, true);
  }

  function refuse(): void {
    dialog?.close();
  }
</script>

<dialog
  bind:this={dialog}
  class="modal"
  aria-labelledby="link-title"
  onclose={() => {
    // Closed with Escape or Cancel: forget the link. (Allowing clears it itself.)
    if (links.pending) links.pending = null;
    always = false;
  }}
>
  <div class="grid gap-3 p-5">
    <h2 id="link-title" class="t-title3">{t("links.title")}</h2>
    <p class="text-subtext1">{pending ? describe(pending) : ""}</p>
    {#if pending && returnHost(pending)}
      <p class="t-foot text-subtext1">{t("links.returnsTo", { host: returnHost(pending) ?? "" })}</p>
    {/if}
    <label class="flex min-h-11 items-center gap-3">
      <input class="check" type="checkbox" bind:checked={always} />
      <span class="t-foot">{t("links.always")}</span>
    </label>
    <div class="grid grid-cols-2 gap-2">
      <button class="btn btn-gray" onclick={refuse}>{t("common.cancel")}</button>
      <button class="btn btn-filled" onclick={allow}>{t("links.allow")}</button>
    </div>
  </div>
</dialog>
