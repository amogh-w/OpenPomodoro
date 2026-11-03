<script lang="ts">
  import { app, saveSetting, type SettingName } from "./appState.svelte";
  import { t } from "./i18n";
  import { stepMinutes } from "./stepper";
  import { prefs, setAllowLinks, setKeepAwake } from "../platform/prefs.svelte";
  import { wakeLockSupported } from "../platform/wakeLock";

  const MIN = 60_000;

  const durations: { name: SettingName; label: () => string; low: number; high: number }[] = [
    { name: "focus_ms", label: () => t("settings.focusLength"), low: 1, high: 180 },
    { name: "short_break_ms", label: () => t("settings.shortBreak"), low: 1, high: 60 },
    { name: "long_break_ms", label: () => t("settings.longBreak"), low: 1, high: 120 },
  ];

  const toggles: { name: SettingName; label: () => string }[] = [
    { name: "breathe", label: () => t("settings.breathe") },
    { name: "auto_start_break", label: () => t("settings.autoBreak") },
    { name: "auto_start_focus", label: () => t("settings.autoFocus") },
  ];

  const rounds = (): number => app.settings?.rounds_before_long ?? 4;
  const minutesOf = (name: SettingName): number => Math.round(Number(app.settings?.[name] ?? 0) / MIN);
</script>

{#if app.settings}
  <section class="card mt-4 grid gap-4 p-4" aria-labelledby="timer-settings">
    <h2 id="timer-settings" class="t-title3">{t("settings.timer")}</h2>

    {#each durations as d (d.name)}
      <div class="flex items-center justify-between gap-3">
        <span>{d.label()}</span>
        <div class="flex items-center gap-2">
          <button
            class="btn btn-gray size-11 !px-0"
            aria-label="{t('settings.decrease')}: {d.label()}"
            onclick={() => saveSetting(d.name, stepMinutes(minutesOf(d.name), -1, d.low, d.high) * MIN)}
          >
            <svg class="size-5"><use href="/icons.svg#minus" /></svg>
          </button>
          <span class="w-16 text-center tabular-nums">{t("settings.minutes", { minutes: minutesOf(d.name) })}</span>
          <button
            class="btn btn-gray size-11 !px-0"
            aria-label="{t('settings.increase')}: {d.label()}"
            onclick={() => saveSetting(d.name, stepMinutes(minutesOf(d.name), 1, d.low, d.high) * MIN)}
          >
            <svg class="size-5"><use href="/icons.svg#plus" /></svg>
          </button>
        </div>
      </div>
    {/each}

    <div class="flex items-center justify-between gap-3">
      <span>{t("settings.rounds")}</span>
      <div class="flex items-center gap-2">
        <button
          class="btn btn-gray size-11 !px-0"
          aria-label="{t('settings.decrease')}: {t('settings.rounds')}"
          onclick={() => saveSetting("rounds_before_long", Math.max(1, rounds() - 1))}
        >
          <svg class="size-5"><use href="/icons.svg#minus" /></svg>
        </button>
        <span class="w-16 text-center tabular-nums">{app.settings.rounds_before_long}</span>
        <button
          class="btn btn-gray size-11 !px-0"
          aria-label="{t('settings.increase')}: {t('settings.rounds')}"
          onclick={() => saveSetting("rounds_before_long", Math.min(12, rounds() + 1))}
        >
          <svg class="size-5"><use href="/icons.svg#plus" /></svg>
        </button>
      </div>
    </div>

    {#each toggles as toggle (toggle.name)}
      <label class="flex min-h-11 items-center justify-between gap-3">
        <span>{toggle.label()}</span>
        <span class="switch">
          <input
            type="checkbox"
            checked={Boolean(app.settings[toggle.name])}
            onchange={(e) => saveSetting(toggle.name, e.currentTarget.checked)}
          />
        </span>
      </label>
    {/each}

    <label class="flex min-h-11 items-center justify-between gap-3">
      <span>
        <span class="block">{t("settings.keepAwake")}</span>
        {#if !wakeLockSupported()}<span class="t-foot text-subtext0">{t("settings.keepAwakeUnsupported")}</span>{/if}
      </span>
      <span class="switch">
        <input
          type="checkbox"
          checked={prefs.keepAwake}
          disabled={!wakeLockSupported()}
          onchange={(e) => setKeepAwake(e.currentTarget.checked)}
        />
      </span>
    </label>

    <label class="flex min-h-11 items-center justify-between gap-3">
      <span>
        <span class="block">{t("settings.links")}</span>
        <span class="t-foot text-subtext0">{t("settings.links.hint")}</span>
      </span>
      <span class="switch">
        <input type="checkbox" checked={prefs.allowLinks} onchange={(e) => setAllowLinks(e.currentTarget.checked)} />
      </span>
    </label>
  </section>
{/if}
