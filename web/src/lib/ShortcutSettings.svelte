<script lang="ts">
  import { t, type MessageKey } from "./i18n";
  import { bindKey, keyLabel, REBINDABLE, type Rebindable, SHORTCUT_LABELS } from "./shortcuts";
  import { keymap, resetKeymap, setKeymap } from "../platform/prefs.svelte";

  /** The action waiting for its new key, if any. */
  let listening = $state<Rebindable | null>(null);
  let problem = $state<string | null>(null);

  function onKey(event: KeyboardEvent): void {
    if (listening === null) return;
    event.preventDefault();
    event.stopPropagation();
    if (event.key === "Escape") {
      listening = null;
      problem = null;
      return;
    }
    // A bare modifier press is the start of a combination, not a choice.
    if (["Shift", "Control", "Alt", "Meta"].includes(event.key)) return;
    const result = bindKey($state.snapshot(keymap), listening, event.key);
    if ("map" in result) {
      setKeymap(result.map);
      listening = null;
      problem = null;
    } else if (result.error === "taken" && result.by) {
      problem = t("shortcuts.taken", { action: t(SHORTCUT_LABELS[result.by] as MessageKey) });
    } else {
      problem = t("shortcuts.invalid");
    }
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<section class="card mt-4 grid gap-3 p-4" aria-labelledby="shortcuts-title">
  <h2 id="shortcuts-title" class="t-title3">{t("shortcuts.title")}</h2>
  <p class="t-foot text-subtext1">{t("shortcuts.hint")}</p>
  <dl class="grid gap-2">
    {#each REBINDABLE as action (action)}
      <div class="flex min-h-11 items-center justify-between gap-3">
        <dt class="text-subtext1">{t(SHORTCUT_LABELS[action] as MessageKey)}</dt>
        <dd class="flex items-center gap-2">
          {#if listening === action}
            <span class="t-foot" role="status">{t("shortcuts.press")}</span>
          {:else}
            <span class="flex gap-1">
              {#each keymap[action] as key (key)}<kbd class="rounded-md bg-surface0 px-2 py-1 text-sm">{keyLabel(key)}</kbd>{/each}
            </span>
          {/if}
          <button
            class="btn btn-gray btn-sm"
            onclick={() => {
              listening = listening === action ? null : action;
              problem = null;
            }}
          >
            {listening === action ? t("common.cancel") : t("shortcuts.change")}
          </button>
        </dd>
      </div>
    {/each}
    <div class="flex min-h-11 items-center justify-between gap-3">
      <dt class="text-subtext1">{t("shortcuts.cancel")}</dt>
      <dd><kbd class="rounded-md bg-surface0 px-2 py-1 text-sm">{keyLabel("Escape")}</kbd></dd>
    </div>
  </dl>
  {#if problem}<p class="alert tone-warn" role="alert">{problem}</p>{/if}
  <button class="btn btn-plain btn-sm justify-self-start" onclick={() => { listening = null; problem = null; resetKeymap(); }}>
    {t("shortcuts.reset")}
  </button>
</section>
