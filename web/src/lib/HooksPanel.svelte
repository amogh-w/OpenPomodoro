<script lang="ts">
  import { onMount } from "svelte";
  import { type HookInfo, api } from "./api";
  import { errorMessage } from "./errors";
  import { t } from "./i18n";

  let hooks = $state<HookInfo[]>([]);
  let loaded = $state(false);
  /** The outcome of the last test of each hook, by its position in the file. */
  let results = $state<Record<number, string>>({});

  onMount(async () => {
    try {
      hooks = (await api.adminHooks()).hooks;
    } finally {
      loaded = true;
    }
  });

  async function test(index: number): Promise<void> {
    results[index] = t("hooks.running");
    try {
      const r = await api.adminTestHook(index);
      results[index] =
        r.result === "exited"
          ? t("hooks.exited", { code: Number(r.detail) })
          : r.result === "timed_out"
            ? t("hooks.timedOut")
            : t("hooks.failed", { why: String(r.detail ?? "") });
    } catch (error) {
      results[index] = errorMessage(error);
    }
  }
</script>

<section class="card mt-4 grid gap-3 p-4" aria-labelledby="hooks-title">
  <h2 id="hooks-title" class="t-title3">{t("hooks.title")}</h2>
  <p class="t-foot text-subtext1">{t("hooks.hint")}</p>
  {#if loaded && hooks.length === 0}
    <p class="text-subtext1">{t("hooks.none")}</p>
  {/if}
  <ul class="grid gap-3">
    {#each hooks as hook, index (index)}
      <li class="grid gap-1 rounded-[var(--r-ctl)] bg-surface0 p-3">
        <span class="flex flex-wrap items-center justify-between gap-2">
          <span><span class="font-semibold">{hook.event === "*" ? t("hooks.anyEvent") : hook.event}</span>{#if !hook.enabled} <span class="t-foot text-subtext1">({t("hooks.off")})</span>{/if}</span>
          <button class="btn btn-gray btn-sm" onclick={() => test(index)}>{t("hooks.test")}</button>
        </span>
        <code class="t-foot break-all text-subtext1">{hook.command.join(" ")}</code>
        {#if results[index]}<span class="t-foot" role="status">{results[index]}</span>{/if}
      </li>
    {/each}
  </ul>
</section>
