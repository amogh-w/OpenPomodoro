<script lang="ts">
  import CategoryExtras from "./CategoryExtras.svelte";
  import { addCategory, app, removeCategory, updateCategory } from "./appState.svelte";
  import { t, type MessageKey } from "./i18n";
  import { accents } from "./theme";

  interface Props {
    /** The chosen category id; empty for none. */
    value: string;
    onpick: (id: string) => void;
  }
  let { value, onpick }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let filter = $state("");
  let editing = $state<string | null>(null);
  let adding = $state(false);
  let draft = $state({ name: "", color: "blue" });
  let problem = $state<string | null>(null);

  const shown = $derived(app.categories.filter((c) => c.name.toLowerCase().includes(filter.trim().toLowerCase())));
  const accentLabel = (color: string): string => t(`accent.${color}` as MessageKey);

  export function open(): void {
    filter = "";
    editing = null;
    adding = false;
    problem = null;
    dialog?.showModal();
  }

  function pick(id: string): void {
    onpick(id);
    dialog?.close();
  }

  async function create(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const name = draft.name.trim();
    if (name === "") return;
    problem = null;
    if (await addCategory(name, draft.color)) {
      draft.name = "";
      adding = false;
    } else problem = t("categories.failed");
  }

  async function save(id: string, change: { name?: string; color?: string }): Promise<void> {
    problem = null;
    if (!(await updateCategory(id, change))) problem = t("categories.failed");
  }

  async function remove(id: string): Promise<void> {
    problem = null;
    editing = null;
    if (id === value) onpick("");
    if (!(await removeCategory(id))) problem = t("categories.failed");
  }
</script>

<dialog bind:this={dialog} class="modal" aria-labelledby="picker-title">
  <div class="grid max-h-[80dvh] grid-rows-[auto_auto_minmax(0,1fr)_auto] gap-3 p-5">
    <div class="flex items-center justify-between gap-3">
      <h2 id="picker-title" class="t-title3">{t("categories.title")}</h2>
      <button class="btn btn-gray size-11 !px-0" aria-label={t("common.close")} onclick={() => dialog?.close()}>
        <svg class="size-5"><use href="/icons.svg#x" /></svg>
      </button>
    </div>

    <input class="field" bind:value={filter} placeholder={t("picker.filter")} aria-label={t("picker.filter")} autocomplete="off" />

    <ul class="grid content-start gap-1 overflow-y-auto">
      <li>
        <button class="flex min-h-11 w-full items-center gap-3 rounded-[var(--r-ctl)] px-3 text-left hover:bg-surface0 {value === '' ? 'bg-surface0' : ''}" aria-pressed={value === ""} onclick={() => pick("")}>
          <span class="size-3 shrink-0 rounded-full bg-surface2" aria-hidden="true"></span>
          <span class="flex-1">{t("timer.category.none")}</span>
        </button>
      </li>
      {#each shown as category (category.id)}
        <li class="grid gap-2 rounded-[var(--r-ctl)] {value === category.id ? 'bg-surface0' : ''}">
          <div class="flex items-center gap-1">
            <button class="flex min-h-11 min-w-0 flex-1 items-center gap-3 rounded-[var(--r-ctl)] px-3 text-left hover:bg-surface0" aria-pressed={value === category.id} onclick={() => pick(category.id)}>
              <span class="size-3 shrink-0 rounded-full" style="background: var(--ctp-{category.color ?? 'blue'})" aria-hidden="true"></span>
              <span class="truncate">{category.name}</span>
            </button>
            <button class="btn btn-plain btn-sm" aria-expanded={editing === category.id} onclick={() => (editing = editing === category.id ? null : category.id)}>
              {t("categories.edit")}
            </button>
          </div>
          {#if editing === category.id}
            <div class="grid gap-3 px-3 pb-3">
              <input
                class="field"
                value={category.name}
                maxlength="40"
                aria-label={t("categories.name")}
                onchange={(e) => {
                  const name = e.currentTarget.value.trim();
                  if (name !== "") void save(category.id, { name });
                }}
              />
              <div class="grid grid-cols-6 gap-2" role="group" aria-label={t("categories.color")}>
                {#each accents as color (color)}
                  <button
                    class="size-9 rounded-full ring-text ring-offset-2 ring-offset-base {category.color === color ? 'ring-2' : ''}"
                    style="background: var(--ctp-{color})"
                    aria-label={accentLabel(color)}
                    aria-pressed={category.color === color}
                    onclick={() => save(category.id, { color })}
                  ></button>
                {/each}
              </div>
              <CategoryExtras {category} onproblem={(text) => (problem = text)} />
              <button class="btn btn-danger-tinted btn-sm justify-self-start" onclick={() => remove(category.id)}>{t("categories.delete")}</button>
            </div>
          {/if}
        </li>
      {/each}
      {#if shown.length === 0 && filter.trim() !== ""}<li class="px-3 py-2 text-subtext1">{t("picker.none")}</li>{/if}
    </ul>

    <div class="grid gap-3">
      {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}
      {#if adding}
        <form class="grid gap-3 border-t border-surface0 pt-3" onsubmit={create}>
          <input class="field" bind:value={draft.name} maxlength="40" placeholder={t("categories.name")} aria-label={t("categories.name")} required />
          <div class="grid grid-cols-6 gap-2" role="group" aria-label={t("categories.color")}>
            {#each accents as color (color)}
              <button
                type="button"
                class="size-9 rounded-full ring-text ring-offset-2 ring-offset-base {draft.color === color ? 'ring-2' : ''}"
                style="background: var(--ctp-{color})"
                aria-label={accentLabel(color)}
                aria-pressed={draft.color === color}
                onclick={() => (draft.color = color)}
              ></button>
            {/each}
          </div>
          <div class="grid grid-cols-2 gap-2">
            <button class="btn btn-gray" type="button" onclick={() => (adding = false)}>{t("common.cancel")}</button>
            <button class="btn btn-filled" type="submit">{t("categories.add")}</button>
          </div>
        </form>
      {:else}
        <button class="btn btn-filled" onclick={() => (adding = true)}>{t("picker.add")}</button>
      {/if}
    </div>
  </div>
</dialog>
