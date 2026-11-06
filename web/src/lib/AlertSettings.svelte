<script lang="ts">
  import { ApiError } from "./api";
  import { t } from "./i18n";
  import { session } from "./session.svelte";
  import { ALERT_KINDS, type AlertKind } from "./sounds";
  import { audioUnlocked, playAlert } from "../platform/audio";
  import { notificationsSupported, permission, requestPermission } from "../platform/notify";
  import { customSounds, removeCustomSound, uploadCustomSound } from "../platform/customSounds.svelte";
  import { alertPrefs, updateAlertPrefs } from "../platform/prefs.svelte";

  const warnChoices = [0, 1, 2, 5];
  const reminderChoices = [0, 1, 5, 10, 15];
  const nudgeChoices = [0, 5, 10, 15, 30];

  let perm = $state(permission());
  let locked = $state(!audioUnlocked());

  async function setNotify(on: boolean): Promise<void> {
    if (!on) return updateAlertPrefs({ notify: false });
    // Browsers only show the permission question in answer to a tap, so it is asked here.
    perm = await requestPermission();
    updateAlertPrefs({ notify: perm === "granted" });
  }

  let soundProblem = $state<string | null>(null);

  async function chooseFile(kind: AlertKind, event: Event & { currentTarget: HTMLInputElement }): Promise<void> {
    const input = event.currentTarget;
    const file = input.files?.[0];
    input.value = "";
    const id = session.user?.id;
    if (!file || !id) return;
    soundProblem = null;
    try {
      await uploadCustomSound(id, kind, file);
      preview(kind);
    } catch (error) {
      soundProblem =
        error instanceof ApiError && error.code === "sound_too_large"
          ? t("settings.sounds.tooLarge")
          : error instanceof ApiError && error.isNetwork
            ? t("settings.sounds.offline")
            : t("settings.sounds.notPlayable");
    }
  }

  async function reset(kind: AlertKind): Promise<void> {
    const id = session.user?.id;
    if (!id) return;
    soundProblem = null;
    try {
      await removeCustomSound(id, kind);
    } catch {
      soundProblem = t("settings.sounds.offline");
    }
  }

  function preview(kind: AlertKind): void {
    playAlert(kind, alertPrefs.volume > 0 ? alertPrefs.volume : 70);
    locked = !audioUnlocked();
  }
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 p-4" aria-labelledby="alert-settings">
  <h2 id="alert-settings" class="t-title3">{t("settings.alerts")}</h2>
  <p class="t-foot text-subtext1">{t("settings.alerts.openOnly")}</p>

  <label class="flex min-h-11 items-center justify-between gap-3">
    <span>{t("settings.sound")}</span>
    <span class="switch">
      <input type="checkbox" checked={alertPrefs.sound} onchange={(e) => updateAlertPrefs({ sound: e.currentTarget.checked })} />
    </span>
  </label>

  <label class="grid gap-1.5">
    <span class="flex justify-between">
      <span>{t("settings.volume")}</span>
      <span class="text-subtext1 tabular-nums">{alertPrefs.volume}%</span>
    </span>
    <input
      class="range"
      type="range"
      min="0"
      max="100"
      step="5"
      value={alertPrefs.volume}
      disabled={!alertPrefs.sound}
      style="--p: {alertPrefs.volume}%"
      oninput={(e) => updateAlertPrefs({ volume: Number(e.currentTarget.value) })}
    />
  </label>

  <label class="flex min-h-11 items-center justify-between gap-3">
    <span>
      <span class="block">{t("settings.notify")}</span>
      {#if !notificationsSupported()}
        <span class="t-foot text-subtext1">{t("settings.notifyUnsupported")}</span>
      {:else if perm === "denied"}
        <span class="t-foot text-subtext1">{t("settings.notifyBlocked")}</span>
      {/if}
    </span>
    <span class="switch">
      <input
        type="checkbox"
        checked={alertPrefs.notify && perm === "granted"}
        disabled={!notificationsSupported() || perm === "denied"}
        onchange={(e) => setNotify(e.currentTarget.checked)}
      />
    </span>
  </label>

  <div class="grid gap-2">
    <span>{t("settings.warn")}</span>
    <div class="flex flex-wrap gap-2">
      {#each warnChoices as minutes (minutes)}
        <button class="chip" aria-pressed={alertPrefs.warnMin === minutes} onclick={() => updateAlertPrefs({ warnMin: minutes })}>
          {minutes === 0 ? t("settings.warn.off") : t("settings.warn.min", { minutes })}
        </button>
      {/each}
    </div>
  </div>

  <div class="grid gap-2">
    <span>{t("settings.reminder")}</span>
    <div class="flex flex-wrap gap-2">
      {#each reminderChoices as minutes (minutes)}
        <button class="chip" aria-pressed={alertPrefs.reminderMin === minutes} onclick={() => updateAlertPrefs({ reminderMin: minutes })}>
          {minutes === 0 ? t("settings.reminder.off") : t("settings.reminder.every", { minutes })}
        </button>
      {/each}
    </div>
  </div>

  <div class="grid gap-1">
    <span class="t-foot text-subtext1">{t("settings.tick")}</span>
    <label class="flex min-h-11 items-center justify-between gap-3">
      <span>{t("settings.tick.focus")}</span>
      <span class="switch">
        <input type="checkbox" checked={alertPrefs.tickFocus} onchange={(e) => updateAlertPrefs({ tickFocus: e.currentTarget.checked })} />
      </span>
    </label>
    <label class="flex min-h-11 items-center justify-between gap-3">
      <span>{t("settings.tick.break")}</span>
      <span class="switch">
        <input type="checkbox" checked={alertPrefs.tickBreak} onchange={(e) => updateAlertPrefs({ tickBreak: e.currentTarget.checked })} />
      </span>
    </label>
  </div>

  <div class="grid gap-2">
    <span>{t("settings.nudge")}
      <span class="t-foot block text-subtext1">{t("settings.nudge.hint")}</span>
    </span>
    <div class="flex flex-wrap gap-2">
      {#each nudgeChoices as minutes (minutes)}
        <button class="chip" aria-pressed={alertPrefs.nudgeMin === minutes} onclick={() => updateAlertPrefs({ nudgeMin: minutes })}>
          {minutes === 0 ? t("settings.nudge.off") : t("settings.nudge.every", { minutes })}
        </button>
      {/each}
    </div>
  </div>

  <div class="grid gap-2">
    <label class="flex min-h-11 items-center justify-between gap-3">
      <span>
        <span class="block">{t("settings.quiet")}</span>
        <span class="t-foot text-subtext1">{t("settings.quiet.hint")}</span>
      </span>
      <span class="switch">
        <input type="checkbox" checked={alertPrefs.quietEnabled} onchange={(e) => updateAlertPrefs({ quietEnabled: e.currentTarget.checked })} />
      </span>
    </label>
    {#if alertPrefs.quietEnabled}
      <div class="grid grid-cols-2 gap-3">
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("settings.quiet.from")}</span>
          <input class="field" type="time" value={alertPrefs.quietStart} onchange={(e) => updateAlertPrefs({ quietStart: e.currentTarget.value })} />
        </label>
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("settings.quiet.to")}</span>
          <input class="field" type="time" value={alertPrefs.quietEnd} onchange={(e) => updateAlertPrefs({ quietEnd: e.currentTarget.value })} />
        </label>
      </div>
    {/if}
  </div>

  <div class="grid gap-2">
    <label class="flex min-h-11 items-center justify-between gap-3">
      <span>
        <span class="block">{t("alerts.daily")}</span>
        <span class="t-foot text-subtext1">{t("alerts.daily.hint")}</span>
      </span>
      <span class="switch">
        <input type="checkbox" checked={alertPrefs.dailyEnabled} onchange={(e) => updateAlertPrefs({ dailyEnabled: e.currentTarget.checked })} />
      </span>
    </label>
    {#if alertPrefs.dailyEnabled}
      <label class="grid gap-1.5">
        <span class="t-foot text-subtext1">{t("alerts.daily.time")}</span>
        <input class="field" type="time" value={alertPrefs.dailyTime} onchange={(e) => updateAlertPrefs({ dailyTime: e.currentTarget.value })} />
      </label>
      <div class="grid gap-1.5">
        <span class="t-foot text-subtext1">{t("alerts.daily.days")}</span>
        <div class="flex flex-wrap gap-2" role="group" aria-label={t("alerts.daily.days")}>
          {#each alertPrefs.dailyDays as on, i (i)}
            <button
              class="chip"
              aria-pressed={on}
              onclick={() => updateAlertPrefs({ dailyDays: alertPrefs.dailyDays.map((v, j) => (j === i ? !v : v)) })}
            >
              {new Date(2026, 10, 2 + i).toLocaleDateString(undefined, { weekday: "short" })}
            </button>
          {/each}
        </div>
      </div>
    {/if}
  </div>

  <div class="grid gap-2">
    <span>{t("settings.preview")}</span>
    <p class="t-foot text-subtext1">{t("settings.sounds.hint")}</p>
    <ul class="grid gap-2">
      {#each ALERT_KINDS as kind (kind)}
        <li class="flex flex-wrap items-center justify-between gap-2">
          <span>
            {t(`settings.sounds.${kind}`)}
            {#if customSounds.slots[kind]}<span class="t-foot block text-subtext1">{t("settings.sounds.custom")}</span>{/if}
          </span>
          <span class="flex flex-wrap gap-2">
            <button class="btn btn-gray btn-sm" onclick={() => preview(kind)}>{t("settings.sounds.play")}</button>
            <label class="btn btn-gray btn-sm cursor-pointer">
              {t("settings.sounds.choose")}
              <input class="sr-only" type="file" accept="audio/*" onchange={(e) => chooseFile(kind, e)} />
            </label>
            {#if customSounds.slots[kind]}
              <button class="btn btn-plain btn-sm" onclick={() => reset(kind)}>{t("settings.sounds.reset")}</button>
            {/if}
          </span>
        </li>
      {/each}
    </ul>
    {#if soundProblem}<p class="alert tone-warn" role="alert">{soundProblem}</p>{/if}
    {#if locked}<p class="t-foot text-subtext1">{t("settings.audioLocked")}</p>{/if}
  </div>
</section>
