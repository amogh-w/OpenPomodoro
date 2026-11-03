<script lang="ts">
  import { t, type MessageKey } from "./i18n";
  import { navigate, router } from "./router.svelte";
  import type { Route } from "./router";

  const tabs: { route: Route; icon: string; label: MessageKey }[] = [
    { route: "timer", icon: "timer", label: "nav.timer" },
    { route: "history", icon: "calendar", label: "nav.history" },
    { route: "settings", icon: "settings", label: "nav.settings" },
  ];
</script>

<nav aria-label={t("nav.label")} class="fixed inset-x-0 bottom-0 mx-auto max-w-[480px]">
  <div class="tabbar">
    {#each tabs as tab (tab.route)}
      <!-- aria-selected is the kit's styling hook for .tab; aria-current is what assistive tech reads. -->
      <!-- svelte-ignore a11y_role_supports_aria_props_implicit -->
      <button
        class="tab"
        aria-selected={router.route === tab.route}
        aria-current={router.route === tab.route ? "page" : undefined}
        onclick={() => navigate(tab.route)}
      >
        <svg class="size-6"><use href="/icons.svg#{tab.icon}" /></svg>
        {t(tab.label)}
      </button>
    {/each}
  </div>
</nav>
