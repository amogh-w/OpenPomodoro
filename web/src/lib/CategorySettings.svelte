<script lang="ts">
  import { addCategory, app, removeCategory, updateCategory } from "./appState.svelte";
  import { t, type MessageKey } from "./i18n";
  import { accents } from "./theme";

  let adding = $state({ name: "", color: "blue" });
  let editing = $state<string | null>(null);
  let problem = $state<string | null>(null);

  const accentLabel = (color: string): string => t(`accent.${color}` as MessageKey);

  async function add(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    const name = adding.name.trim();
    if (name === "") return;
    problem = null;
    if (await addCategory(name, adding.color)) adding.name = "";
    else problem = t("categories.failed");
  }

  async function save(id: string, change: { name?: string; color?: string }): Promise<void> {
    problem = null;
    if (!(await updateCategory(id, change))) problem = t("categories.failed");
  }

  async function remove(id: string): Promise<void> {
    problem = null;
    editing = null;
    if (!(await removeCategory(id))) problem = t("categories.failed");
  }
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 p-4" aria-labelledby="categories-title">
  <h2 id="categories-title" class="t-title3">{t("categories.title")}</h2>

  {#if app.categories.length === 0}
    <p class="text-subtext1">{t("categories.empty")}</p>
  {/if}

  {#each app.categories as category (category.id)}
    <div class="grid gap-3 rounded-lg bg-surface0/50 p-3">
      <div class="flex items-center justify-between gap-3">
        <span class="flex min-w-0 items-center gap-2.5">
          <span class="size-4 shrink-0 rounded-full" style="background: var(--ctp-{category.color ?? 'blue'})" aria-hidden="true"></span>
          <span class="truncate">{category.name}</span>
        </span>
        <button class="btn btn-gray btn-sm" onclick={() => (editing = editing === category.id ? null : category.id)}>
          {editing === category.id ? t("categories.close") : t("categories.edit")}
        </button>
      </div>
      {#if editing === category.id}
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("categories.name")}</span>
          <input
            class="field"
            value={category.name}
            maxlength="40"
            onchange={(e) => {
              const name = e.currentTarget.value.trim();
              if (name !== "") void save(category.id, { name });
            }}
          />
        </label>
        <div class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("categories.color")}</span>
          <div class="grid grid-cols-6 gap-2">
            {#each accents as color (color)}
              <button
                class="size-11 rounded-full ring-text ring-offset-2 ring-offset-base {category.color === color ? 'ring-2' : ''}"
                style="background: var(--ctp-{color})"
                aria-label={accentLabel(color)}
                aria-pressed={category.color === color}
                onclick={() => save(category.id, { color })}
              ></button>
            {/each}
          </div>
        </div>
        <button class="btn btn-danger-tinted" onclick={() => remove(category.id)}>{t("categories.delete")}</button>
      {/if}
    </div>
  {/each}

  <form class="grid gap-3 border-t border-surface0 pt-3" onsubmit={add}>
    <h3 class="t-headline">{t("categories.add")}</h3>
    <label class="grid gap-1.5">
      <span class="t-foot text-subtext1">{t("categories.name")}</span>
      <input class="field" bind:value={adding.name} maxlength="40" required />
    </label>
    <div class="grid grid-cols-6 gap-2" role="group" aria-label={t("categories.color")}>
      {#each accents as color (color)}
        <button
          type="button"
          class="size-11 rounded-full ring-text ring-offset-2 ring-offset-base {adding.color === color ? 'ring-2' : ''}"
          style="background: var(--ctp-{color})"
          aria-label={accentLabel(color)}
          aria-pressed={adding.color === color}
          onclick={() => (adding.color = color)}
        ></button>
      {/each}
    </div>
    {#if problem}<div class="alert tone-danger" role="alert">{problem}</div>{/if}
    <button class="btn btn-filled" type="submit">{t("categories.add")}</button>
  </form>
</section>
