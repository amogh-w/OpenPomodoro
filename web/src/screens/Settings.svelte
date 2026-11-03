<script lang="ts">
  import { onMount } from "svelte";
  import { t, type MessageKey } from "../lib/i18n";
  import { accents, flavors, radii, setTheme, syncThemeButtons } from "../lib/theme";
  import { install, promptInstall } from "../platform/install.svelte";

  // The kit marks the selected buttons (aria-pressed / ring) itself once they exist.
  onMount(syncThemeButtons);
</script>

<h1 class="t-large">{t("settings.title")}</h1>

<section class="card mt-4 grid gap-5 p-4" aria-labelledby="appearance">
  <h2 id="appearance" class="t-title3">{t("settings.appearance")}</h2>

  <div>
    <div class="t-foot mb-2 text-subtext0">{t("settings.theme")}</div>
    <div class="flex flex-wrap gap-2">
      {#each flavors as flavor (flavor)}
        <button class="chip" data-flavor={flavor} onclick={() => setTheme("flavor", flavor)}>
          {t(`flavor.${flavor}`)}
        </button>
      {/each}
    </div>
  </div>

  <div>
    <div class="t-foot mb-2 text-subtext0">{t("settings.accent")}</div>
    <div class="grid grid-cols-6 gap-2">
      {#each accents as accent (accent)}
        <button
          class="size-11 rounded-full ring-text ring-offset-2 ring-offset-base"
          style="background: var(--ctp-{accent})"
          data-accent={accent}
          aria-label={t(`accent.${accent}` as MessageKey)}
          onclick={() => setTheme("accent", accent)}
        ></button>
      {/each}
    </div>
  </div>

  <div>
    <div class="t-foot mb-2 text-subtext0">{t("settings.radius")}</div>
    <div class="flex flex-wrap gap-2">
      {#each radii as radius (radius)}
        <button class="chip" data-radius={radius} onclick={() => setTheme("radius", radius)}>
          {t(`radius.${radius}`)}
        </button>
      {/each}
    </div>
  </div>
</section>

<section class="card mt-4 grid gap-3 p-4" aria-labelledby="app">
  <h2 id="app" class="t-title3">{t("settings.app")}</h2>
  {#if install.installed}
    <p class="text-subtext0">{t("install.installed")}</p>
  {:else if install.available}
    <button class="btn btn-filled" onclick={promptInstall}>{t("install.button")}</button>
  {:else}
    <p class="text-subtext0">{t("install.unavailable")}</p>
  {/if}
</section>
