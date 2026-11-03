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

  async function carryOut(action: LinkAction): Promise<void> {
    links.pending = null;
    const view = currentView();
    const planned = view ? commandFor(action, view, app.categories) : { unavailable: true as const };
    if ("unavailable" in planned) {
      app.notice = t("links.unavailable");
      return;
    }
    const result = await sendCommand(planned.command);
    if (!result.ok) {
      app.notice = t("links.failed");
      return;
    }
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
    if (pending) void carryOut(pending);
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
