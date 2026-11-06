<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./api";
  import { t } from "./i18n";

  let lines = $state<{ at: number; area: string; message: string }[]>([]);
  let loaded = $state(false);
  let copied = $state(false);

  async function load(): Promise<void> {
    try {
      lines = (await api.adminLog()).lines;
    } finally {
      loaded = true;
    }
  }
  onMount(() => void load());

  const stamp = (ms: number): string => new Date(ms).toISOString().slice(11, 19);
  const shown = $derived(lines.toReversed());

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(lines.map((l) => `${new Date(l.at).toISOString()} ${l.area} ${l.message}`).join("\n"));
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 p-4" aria-labelledby="serverlog-title">
  <h2 id="serverlog-title" class="t-title3">{t("serverlog.title")}</h2>
  <p class="t-foot text-subtext1">{t("serverlog.hint")}</p>
  <div class="flex flex-wrap gap-2">
    <button class="btn btn-gray btn-sm" onclick={load}>{t("serverlog.refresh")}</button>
    <button class="btn btn-gray btn-sm" disabled={lines.length === 0} onclick={copy}>{copied ? t("access.copied") : t("diag.copy")}</button>
  </div>
  <ul class="t-foot grid max-h-72 gap-1 overflow-y-auto font-mono text-subtext1">
    {#each shown as line (line.at + line.message)}
      <li class="break-words"><span class="tabular-nums">{stamp(line.at)}</span> {line.area} {line.message}</li>
    {:else}
      {#if loaded}<li>{t("diag.empty")}</li>{/if}
    {/each}
  </ul>
</section>
