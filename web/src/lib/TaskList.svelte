<script lang="ts">
  import { addTask, app, removeTask, setTaskDone, updateTask } from "./appState.svelte";
  import CategorySelect from "./CategorySelect.svelte";
  import { formatSpan } from "./format";
  import { t } from "./i18n";
  import { recent } from "./recentStore.svelte";
  import { orderTasks, timePerTask } from "./tasks";
  import type { Task } from "./types";

  /** `embedded`: no card of its own (it sits inside another card, which is also inside the timer form). */
  let {
    selected = null,
    category = null,
    onpick,
    embedded = false,
  }: {
    selected?: string | null;
    /** The category chosen for the session: a task added now is filed under it. */
    category?: string | null;
    onpick: (task: Task) => void;
    embedded?: boolean;
  } = $props();

  let title = $state("");
  const tasks = $derived(orderTasks(app.tasks));
  const spent = $derived(timePerTask(recent.sessions));

  async function add(): Promise<void> {
    const text = title.trim();
    if (text === "") return;
    title = "";
    await addTask(text, category);
  }
</script>

<section class={embedded ? "grid gap-2" : "card grid gap-3 p-4"} aria-label={t("tasks.title")}>
  {#if tasks.length === 0}
    <p class="t-foot text-subtext1">{t("tasks.empty")}</p>
  {:else}
    <ul class="anim m-stagger grid gap-1">
      {#each tasks as task, i (task.id)}
        <li class="flex items-center gap-2" style="--i:{Math.min(i, 5)}">
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
            type="button"
            class="min-h-11 min-w-0 flex-1 rounded-[var(--r-ctl)] px-2 text-left {task.done ? 'text-subtext1' : ''} {selected === task.id && !task.done ? 'bg-surface0 font-semibold' : ''}"
            aria-pressed={selected === task.id && !task.done}
            disabled={task.done === true}
            onclick={() => onpick(task)}
          >
            <span class="block truncate {task.done ? 'line-through' : ''}">{task.title}</span>
            {#if (spent.get(task.id) ?? 0) > 0}<span class="t-foot block text-subtext1">{t("tasks.spent", { time: formatSpan(spent.get(task.id) ?? 0) })}</span>{/if}
          </button>
          <!-- Change the category of a task that is already there. -->
          <CategorySelect row extras={false} value={task.category ?? ""} onpick={(id) => void updateTask(task.id, { category: id === "" ? null : id })} />
          <button type="button" class="btn btn-plain size-11 !px-0" aria-label={t("tasks.remove", { title: task.title })} onclick={() => removeTask(task.id)}>
            <svg class="size-5"><use href="/icons.svg#trash-2" /></svg>
          </button>
        </li>
      {/each}
    </ul>
    {#if tasks.some((x) => !x.done)}<p class="t-foot text-subtext1">{t("tasks.pickHint")}</p>{/if}
  {/if}
  <!-- Not a <form>: this can sit inside the timer form, whose Enter key must not start a session. -->
  <div class="flex items-center gap-2">
    <input
      class="field min-w-0 flex-1"
      bind:value={title}
      maxlength="200"
      placeholder={t("tasks.add.placeholder")}
      aria-label={t("tasks.add")}
      autocomplete="off"
      onkeydown={(e) => {
        if (e.key !== "Enter") return;
        e.preventDefault();
        void add();
      }}
    />
    <button class="btn btn-gray" type="button" disabled={title.trim() === ""} onclick={() => void add()}>{t("tasks.add")}</button>
  </div>

</section>
