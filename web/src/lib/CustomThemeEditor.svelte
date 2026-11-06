<script lang="ts">
  import { EXAMPLE_THEME, parseCustomTheme } from "./customTheme";
  import { t, type MessageKey } from "./i18n";
  import { readCustomText, saveCustomTheme } from "./theme";

  let text = $state(readCustomText());
  let problem = $state<string | null>(null);
  let applied = $state(false);

  function apply(): void {
    applied = false;
    const result = parseCustomTheme(text);
    if (!result.ok) {
      problem = t(`theme.custom.error.${result.error}` as MessageKey);
      return;
    }
    problem = null;
    saveCustomTheme(text);
    applied = true;
  }
</script>

<div class="mt-3 grid gap-2">
  <h3 class="t-headline">{t("theme.custom.title")}</h3>
  <p class="t-foot text-subtext1">{t("theme.custom.hint")}</p>
  <textarea
    class="field min-h-40 font-mono text-sm"
    bind:value={text}
    spellcheck="false"
    autocapitalize="none"
    aria-label={t("theme.custom.title")}
    oninput={() => (applied = false)}
  ></textarea>
  {#if problem}<p class="alert tone-warn" role="alert">{problem}</p>{/if}
  {#if applied}<p class="t-foot" role="status">{t("theme.custom.applied")}</p>{/if}
  <div class="flex flex-wrap gap-2">
    <button class="btn btn-filled btn-sm" onclick={apply}>{t("theme.custom.apply")}</button>
    <button class="btn btn-gray btn-sm" onclick={() => { text = EXAMPLE_THEME; applied = false; problem = null; }}>{t("theme.custom.example")}</button>
  </div>
</div>
