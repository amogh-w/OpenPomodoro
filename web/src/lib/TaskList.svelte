<script lang="ts">
  import { addTask, app, removeTask, setTaskDone } from "./appState.svelte";
  import { formatSpan } from "./format";
  import { t } from "./i18n";
  import { recent } from "./recentStore.svelte";
  import { orderTasks, timePerTask } from "./tasks";
  import type { Task } from "./types";

  let { selected = null, onpick }: { selected?: string | null; onpick: (task: Task) => void } = $props();

  let title = $state("");
  const tasks = $derived(orderTasks(app.tasks));
  const spent = $derived(timePerTask(recent.sessions));

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

  {#if tasks.length === 0}
    <p class="t-foot text-subtext1">{t("tasks.empty")}</p>
  {:else}
    <ul class="grid gap-1">
      {#each tasks as task (task.id)}
        <li class="flex items-center gap-2">
          <label class="flex size-11 shrink-0 cursor-pointer items-center justify-center">
            <input
              class="check"
              type="checkbox"
              checked={task.done === true}
              aria-label={t(task.done ? "tasks.markOpen" : "tasks.markDone", { title: task.title })}
              onchange={(e) => setTaskDone(task.id, e.currentTarget.checked)}
            />
          </label>
          <!-- One element for open and ticked tasks, so ticking changes colour and strike-through but never the layout. -->
          <button
            class="min-h-11 min-w-0 flex-1 rounded-[var(--r-ctl)] px-2 text-left {task.done ? 'text-subtext1' : ''} {selected === task.id && !task.done ? 'bg-surface0 font-semibold' : ''}"
            aria-pressed={selected === task.id && !task.done}
            disabled={task.done === true}
            onclick={() => onpick(task)}
          >
            <span class="block truncate {task.done ? 'line-through' : ''}">{task.title}</span>
            {#if (spent.get(task.id) ?? 0) > 0}<span class="t-foot block text-subtext1">{t("tasks.spent", { time: formatSpan(spent.get(task.id) ?? 0) })}</span>{/if}
          </button>
          <button class="btn btn-plain size-11 !px-0" aria-label={t("tasks.remove", { title: task.title })} onclick={() => removeTask(task.id)}>
            <svg class="size-5"><use href="/icons.svg#trash-2" /></svg>
          </button>
        </li>
      {/each}
    </ul>
    {#if tasks.some((x) => !x.done)}<p class="t-foot text-subtext1">{t("tasks.pickHint")}</p>{/if}
  {/if}
</section>
