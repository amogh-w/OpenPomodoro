<script lang="ts">
  import CategoryBar from "./CategoryBar.svelte";
  import { addSession, app, serverNow } from "./appState.svelte";
  import { fromLocalInput, toLocalInput } from "./datetime";
  import { t } from "./i18n";
  import { refreshRecent } from "./recentStore.svelte";

  let { onsaved }: { onsaved?: () => void } = $props();

  type Mood = "focused" | "neutral" | "distracted";
  const moods: Mood[] = ["focused", "neutral", "distracted"];

  let dialog: HTMLDialogElement | undefined = $state();
  let start = $state("");
  let minutes = $state(25);
  let intention = $state("");
  let categoryId = $state("");
  let notes = $state("");
  let mood = $state<Mood | null>(null);
  let problem = $state<string | null>(null);
  let busy = $state(false);

  /** Open the dialog with sensible starting values: a session of the usual length that just finished. */
  export function show(): void {
    const length = Math.round((app.settings?.focus_ms ?? 25 * 60_000) / 60_000);
    minutes = length;
    start = toLocalInput(serverNow() - length * 60_000);
    intention = notes = categoryId = "";
    mood = null;
    problem = null;
    dialog?.showModal();
  }

  async function save(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const startedAt = fromLocalInput(start);
    if (startedAt === null) return void (problem = t("history.add.badStart"));
    if (!Number.isInteger(minutes) || minutes < 1 || minutes > 720) return void (problem = t("history.add.badLength"));
    const endedAt = startedAt + minutes * 60_000;
    if (endedAt > serverNow() + 60_000) return void (problem = t("history.add.future"));

    busy = true;
    problem = null;
    try {
      const saved = await addSession({
        kind: "focus",
        started_at: startedAt,
        ended_at: endedAt,
        ...(intention.trim() !== "" ? { intention: intention.trim() } : {}),
        category: categoryId === "" ? null : categoryId,
        notes: notes.trim() === "" ? null : notes.trim(),
        mood,
      });
      if (saved) {
        dialog?.close();
        void refreshRecent();
        onsaved?.();
      } else {
        problem = t("history.add.failed");
      }
    } finally {
      busy = false;
    }
  }
</script>

<dialog bind:this={dialog} class="modal" aria-labelledby="add-session-title">
  <form class="grid gap-3 p-5" onsubmit={save}>
    <h2 id="add-session-title" class="t-title3">{t("history.add.title")}</h2>

    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("history.add.start")}</span>
      <input class="field" type="datetime-local" bind:value={start} required />
    </label>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("history.add.length")}</span>
      <input class="field" type="number" min="1" max="720" step="1" bind:value={minutes} required />
    </label>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("timer.intention.label")}</span>
      <input class="field" bind:value={intention} maxlength="200" autocomplete="off" />
    </label>
    <div class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("timer.category")}</span>
      <CategoryBar value={categoryId} onpick={(id) => (categoryId = id)} />
    </div>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("timer.notes")}</span>
      <textarea class="field min-h-20" bind:value={notes} maxlength="4000"></textarea>
    </label>
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

    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}
    <div class="grid grid-cols-2 gap-2">
      <button class="btn btn-gray" type="button" onclick={() => dialog?.close()}>{t("common.cancel")}</button>
      <button class="btn btn-filled" type="submit" disabled={busy}>{t("history.add.save")}</button>
    </div>
  </form>
</dialog>
