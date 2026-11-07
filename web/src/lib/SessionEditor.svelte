<script lang="ts">
  import CategoryBar from "./CategoryBar.svelte";
  import { app, deleteSession, editSession } from "./appState.svelte";
  import { t } from "./i18n";
  import type { SessionRecord } from "./types";

  type Mood = "focused" | "neutral" | "distracted";
  const moods: Mood[] = ["focused", "neutral", "distracted"];

  let { onchanged }: { onchanged: () => void } = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let session = $state<SessionRecord | null>(null);
  let intention = $state("");
  let categoryId = $state("");
  let notes = $state("");
  let mood = $state<Mood | null>(null);
  let confirming = $state(false);
  let problem = $state<string | null>(null);
  let busy = $state(false);

  export function show(record: SessionRecord): void {
    session = record;
    intention = record.intention ?? "";
    categoryId = record.category ?? "";
    notes = record.notes ?? "";
    mood = record.mood ?? null;
    confirming = false;
    problem = null;
    dialog?.showModal();
  }

  async function finish(done: Promise<boolean>): Promise<void> {
    busy = true;
    problem = null;
    try {
      if (await done) {
        dialog?.close();
        onchanged();
      } else {
        problem = t("history.edit.failed");
      }
    } finally {
      busy = false;
    }
  }

  function save(event: SubmitEvent): void {
    event.preventDefault();
    if (!session) return;
    // Breaks keep to the fields they have; focus sessions can change all four.
    const change: Record<string, unknown> =
      session.kind === "focus"
        ? { intention: intention.trim(), category: categoryId === "" ? null : categoryId, notes: notes.trim() === "" ? null : notes.trim(), mood }
        : { notes: notes.trim() === "" ? null : notes.trim() };
    void finish(editSession(session.id, change));
  }
</script>

<dialog bind:this={dialog} class="modal" aria-labelledby="edit-session-title">
  <form class="grid gap-3 p-5" onsubmit={save}>
    <h2 id="edit-session-title" class="t-title3">{t("history.edit.title")}</h2>

    {#if session?.kind === "focus"}
      <label class="grid gap-1.5">
        <span class="t-foot text-subtext1">{t("timer.intention.label")}</span>
        <input class="field" bind:value={intention} maxlength="200" autocomplete="off" />
      </label>
      <div class="grid gap-1.5">
        <span class="t-foot text-subtext1">{t("timer.category")}</span>
        <CategoryBar value={categoryId} onpick={(id) => (categoryId = id)} />
      </div>
    {/if}
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("timer.notes")}</span>
      <textarea class="field min-h-24" bind:value={notes} maxlength="4000"></textarea>
    </label>
    {#if session?.kind === "focus"}
      <div class="grid gap-1.5">
        <span class="t-foot text-subtext1">{t("timer.mood")}</span>
        <div class="flex flex-wrap gap-2" role="group" aria-label={t("timer.mood")}>
          {#each moods as option (option)}
            <button class="chip" type="button" aria-pressed={mood === option} onclick={() => (mood = mood === option ? null : option)}>
              {t(`mood.${option}`)}
            </button>
          {/each}
        </div>
      </div>
    {/if}

    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}

    {#if confirming}
      <div class="alert tone-warn" role="alert">{t("history.edit.confirm")}</div>
      <div class="grid grid-cols-2 gap-2">
        <button class="btn btn-gray" type="button" onclick={() => (confirming = false)}>{t("common.cancel")}</button>
        <button class="btn btn-danger" type="button" disabled={busy} onclick={() => session && finish(deleteSession(session.id))}>
          {t("history.edit.yes")}
        </button>
      </div>
    {:else}
      <div class="grid grid-cols-2 gap-2">
        <button class="btn btn-gray" type="button" onclick={() => dialog?.close()}>{t("common.cancel")}</button>
        <button class="btn btn-filled" type="submit" disabled={busy}>{t("history.edit.save")}</button>
      </div>
      <button class="btn btn-danger-tinted" type="button" onclick={() => (confirming = true)}>{t("history.edit.delete")}</button>
    {/if}
  </form>
</dialog>
