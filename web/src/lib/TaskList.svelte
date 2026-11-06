<script lang="ts">
  import { addTask, app, removeTask, setTaskDone } from "./appState.svelte";
  import { t } from "./i18n";
  import { splitTasks } from "./tasks";
  import type { Task } from "./types";

  let { selected = null, onpick }: { selected?: string | null; onpick: (task: Task) => void } = $props();

  let title = $state("");
  let showDone = $state(false);
  const groups = $derived(splitTasks(app.tasks));

  async function add(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const text = title.trim();
    if (text === "") return;
    title = "";
    await addTask(text);
  }
</script>

<section class="card grid gap-3 p-4" aria-labelledby="tasks-title">
  <h2 id="tasks-title" class="t-title3">{t("tasks.title")}</h2>

  <form class="flex gap-2" onsubmit={add}>
    <input class="field min-w-0 flex-1" bind:value={title} maxlength="200" placeholder={t("tasks.add.placeholder")} aria-label={t("tasks.add")} autocomplete="off" />
    <button class="btn btn-gray" type="submit" disabled={title.trim() === ""}>{t("tasks.add")}</button>
  </form>

  {#if groups.open.length === 0}
    <p class="t-foot text-subtext1">{t("tasks.empty")}</p>
  {:else}
    <ul class="grid gap-1">
      {#each groups.open as task (task.id)}
        <li class="flex items-center gap-2">
          <label class="check flex size-11 shrink-0 items-center justify-center">
            <input type="checkbox" checked={false} aria-label={t("tasks.markDone", { title: task.title })} onchange={() => setTaskDone(task.id, true)} />
          </label>
          <button class="min-h-11 min-w-0 flex-1 truncate rounded-[var(--r-ctl)] px-2 text-left {selected === task.id ? 'bg-surface0 font-semibold' : ''}" aria-pressed={selected === task.id} onclick={() => onpick(task)}>
            {task.title}
          </button>
          <button class="btn btn-plain size-11 !px-0" aria-label={t("tasks.remove", { title: task.title })} onclick={() => removeTask(task.id)}>
            <svg class="size-5"><use href="/icons.svg#trash-2" /></svg>
          </button>
        </li>
      {/each}
    </ul>
    <p class="t-foot text-subtext1">{t("tasks.pickHint")}</p>
  {/if}

  {#if groups.done.length > 0}
    <button class="btn btn-plain btn-sm justify-self-start" aria-expanded={showDone} onclick={() => (showDone = !showDone)}>
      {t("tasks.done", { n: groups.done.length })}
    </button>
    {#if showDone}
      <ul class="grid gap-1">
        {#each groups.done as task (task.id)}
          <li class="flex items-center gap-2 text-subtext1">
            <label class="check flex size-11 shrink-0 items-center justify-center">
              <input type="checkbox" checked={true} aria-label={t("tasks.markOpen", { title: task.title })} onchange={() => setTaskDone(task.id, false)} />
            </label>
            <span class="min-w-0 flex-1 truncate line-through">{task.title}</span>
            <button class="btn btn-plain size-11 !px-0" aria-label={t("tasks.remove", { title: task.title })} onclick={() => removeTask(task.id)}>
              <svg class="size-5"><use href="/icons.svg#trash-2" /></svg>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>
