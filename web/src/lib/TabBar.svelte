<script lang="ts">
  import { t, type MessageKey } from "./i18n";
  import { navigate, router } from "./router.svelte";
  import type { Route } from "./router";

  const tabs: { route: Route; icon: string; label: MessageKey }[] = [
    { route: "timer", icon: "timer", label: "nav.timer" },
    { route: "history", icon: "calendar", label: "nav.history" },
    { route: "settings", icon: "settings", label: "nav.settings" },
  ];

  // The users screen belongs to Settings, so Settings stays selected there.
  const active = (route: Route): boolean => router.route === route || (route === "settings" && router.route === "admin");

  /** Arrow keys, Home and End move between tabs, as in any tab list. */
  function onKey(event: KeyboardEvent): void {
    const current = tabs.findIndex((tab) => active(tab.route));
    const moves: Record<string, number> = { ArrowRight: current + 1, ArrowLeft: current - 1, Home: 0, End: tabs.length - 1 };
    const target = moves[event.key];
    if (target === undefined) return;
    event.preventDefault();
    const next = tabs[(target + tabs.length) % tabs.length];
    if (!next) return;
    navigate(next.route);
    document.getElementById(`tab-${next.route}`)?.focus();
  }
</script>

<nav aria-label={t("nav.label")} class="fixed inset-x-0 bottom-0 z-20 mx-auto max-w-[480px]">
  <div class="tabbar" role="tablist" tabindex="-1" onkeydown={onKey}>
    {#each tabs as tab (tab.route)}
      <button
        id="tab-{tab.route}"
        class="tab"
        role="tab"
        aria-selected={active(tab.route)}
        aria-controls="screen"
        tabindex={active(tab.route) ? 0 : -1}
        onclick={() => navigate(tab.route)}
      >
        <svg class="size-6"><use href="/icons.svg#{tab.icon}" /></svg>
        {t(tab.label)}
      </button>
    {/each}
  </div>
</nav>
