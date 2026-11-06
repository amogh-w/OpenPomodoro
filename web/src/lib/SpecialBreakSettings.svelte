<script lang="ts">
  import { t } from "./i18n";
  import { MAX_SPECIAL_BREAKS } from "./specialBreaks";
  import { addSpecialBreak, removeSpecialBreak, special, updateSpecialBreak } from "./specialBreaksStore.svelte";
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 p-4" aria-labelledby="special-settings">
  <div>
    <h2 id="special-settings" class="t-title3">{t("special.title")}</h2>
    <p class="t-foot text-subtext1">{t("special.hint")}</p>
  </div>

  {#each special.list as item (item.id)}
    <div class="grid gap-3 rounded-[var(--r-ctl)] bg-surface0 p-3">
      <label class="grid gap-1.5">
        <span class="t-foot text-subtext1">{t("special.label")}</span>
        <input
          class="field"
          type="text"
          maxlength="40"
          value={item.label}
          placeholder={t("special.label.placeholder")}
          onchange={(e) => updateSpecialBreak(item.id, { label: e.currentTarget.value })}
        />
      </label>
      <div class="grid grid-cols-2 gap-3">
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("special.time")}</span>
          <input class="field" type="time" value={item.time} onchange={(e) => updateSpecialBreak(item.id, { time: e.currentTarget.value })} />
        </label>
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("special.minutes")}</span>
          <input
            class="field"
            type="number"
            min="1"
            max="120"
            value={item.minutes}
            onchange={(e) => updateSpecialBreak(item.id, { minutes: e.currentTarget.valueAsNumber })}
          />
        </label>
      </div>
      <div class="flex flex-wrap gap-2" role="group" aria-label={t("alerts.daily.days")}>
        {#each item.days as on, i (i)}
          <button
            class="chip"
            aria-pressed={on}
            onclick={() => updateSpecialBreak(item.id, { days: item.days.map((v, j) => (j === i ? !v : v)) })}
          >
            {new Date(2026, 10, 2 + i).toLocaleDateString(undefined, { weekday: "short" })}
          </button>
        {/each}
      </div>
      <button class="btn btn-plain btn-danger btn-sm justify-self-start" onclick={() => removeSpecialBreak(item.id)}>
        {t("special.remove")}
      </button>
    </div>
  {/each}

  <button class="btn btn-gray justify-self-start" disabled={special.list.length >= MAX_SPECIAL_BREAKS} onclick={addSpecialBreak}>
    {t("special.add")}
  </button>
</section>
