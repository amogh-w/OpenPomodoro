<script lang="ts">
  import { t } from "./i18n";
  import { ALERT_KINDS, type AlertKind } from "./sounds";
  import { audioUnlocked, playAlert } from "../platform/audio";
  import { notificationsSupported, permission, requestPermission } from "../platform/notify";
  import { alertPrefs, updateAlertPrefs } from "../platform/prefs.svelte";

  const warnChoices = [0, 1, 2, 5];
  const reminderChoices = [0, 1, 5, 10, 15];

  let perm = $state(permission());
  let locked = $state(!audioUnlocked());

  async function setNotify(on: boolean): Promise<void> {
    if (!on) return updateAlertPrefs({ notify: false });
    // Browsers only show the permission question in answer to a tap, so it is asked here.
    perm = await requestPermission();
    updateAlertPrefs({ notify: perm === "granted" });
  }

  function preview(kind: AlertKind): void {
    playAlert(kind, alertPrefs.volume > 0 ? alertPrefs.volume : 70);
    locked = !audioUnlocked();
  }
</script>

<section class="card mt-4 grid gap-4 p-4" aria-labelledby="alert-settings">
  <h2 id="alert-settings" class="t-title3">{t("settings.alerts")}</h2>
  <p class="t-foot text-subtext0">{t("settings.alerts.openOnly")}</p>

  <label class="flex min-h-11 items-center justify-between gap-3">
    <span>{t("settings.sound")}</span>
    <span class="switch">
      <input type="checkbox" checked={alertPrefs.sound} onchange={(e) => updateAlertPrefs({ sound: e.currentTarget.checked })} />
    </span>
  </label>

  <label class="grid gap-1.5">
    <span class="flex justify-between">
      <span>{t("settings.volume")}</span>
      <span class="text-subtext0 tabular-nums">{alertPrefs.volume}%</span>
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
        <span class="t-foot text-subtext0">{t("settings.notifyUnsupported")}</span>
      {:else if perm === "denied"}
        <span class="t-foot text-subtext0">{t("settings.notifyBlocked")}</span>
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

  <div class="grid gap-2">
    <label class="flex min-h-11 items-center justify-between gap-3">
      <span>
        <span class="block">{t("settings.quiet")}</span>
        <span class="t-foot text-subtext0">{t("settings.quiet.hint")}</span>
      </span>
      <span class="switch">
        <input type="checkbox" checked={alertPrefs.quietEnabled} onchange={(e) => updateAlertPrefs({ quietEnabled: e.currentTarget.checked })} />
      </span>
    </label>
    {#if alertPrefs.quietEnabled}
      <div class="grid grid-cols-2 gap-3">
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext0">{t("settings.quiet.from")}</span>
          <input class="field" type="time" value={alertPrefs.quietStart} onchange={(e) => updateAlertPrefs({ quietStart: e.currentTarget.value })} />
        </label>
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext0">{t("settings.quiet.to")}</span>
          <input class="field" type="time" value={alertPrefs.quietEnd} onchange={(e) => updateAlertPrefs({ quietEnd: e.currentTarget.value })} />
        </label>
      </div>
    {/if}
  </div>

  <div class="grid gap-2">
    <span>{t("settings.preview")}</span>
    <div class="flex flex-wrap gap-2">
      {#each ALERT_KINDS as kind (kind)}
        <button class="btn btn-gray btn-sm" onclick={() => preview(kind)}>{t(`settings.sounds.${kind}`)}</button>
      {/each}
    </div>
    {#if locked}<p class="t-foot text-subtext0">{t("settings.audioLocked")}</p>{/if}
  </div>
</section>
