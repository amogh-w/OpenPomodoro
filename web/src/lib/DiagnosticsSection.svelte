<script lang="ts">
  import { app } from "./appState.svelte";
  import { clearDiag, diag } from "./diag.svelte";
  import { report } from "./diagReport";
  import { t } from "./i18n";

  let copied = $state(false);
  const shown = $derived(diag.lines.slice(-40).toReversed());
  const time = (ms: number): string => new Date(ms).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit", second: "2-digit" });

  const text = (): string =>
    report({
      app: `OpenPomodoro ${__APP_VERSION__}`,
      time: new Date().toISOString(),
      browser: navigator.userAgent,
      online: String(app.online),
      "waiting to sync": String(app.pending),
      "timer state": app.timer?.state ?? "unknown",
    }, diag.lines);

  async function copy(): Promise<void> {
    try {
      await navigator.clipboard.writeText(text());
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 p-4" aria-labelledby="diag-title">
  <h2 id="diag-title" class="t-title3">{t("diag.title")}</h2>
  <p class="t-foot text-subtext1">{t("diag.hint")}</p>
  <div class="flex flex-wrap gap-2">
    <button class="btn btn-gray btn-sm" onclick={copy}>{copied ? t("access.copied") : t("diag.copy")}</button>
    <button class="btn btn-plain btn-sm" onclick={() => { clearDiag(); copied = false; }}>{t("diag.clear")}</button>
  </div>
  <details>
    <summary class="min-h-11 cursor-pointer py-2">{t("diag.show", { n: diag.lines.length })}</summary>
    <ul class="t-foot grid gap-1 font-mono text-subtext1">
      {#each shown as line (line.at + line.message)}
        <li class="break-words"><span class="tabular-nums">{time(line.at)}</span> {line.area} {line.message}</li>
      {:else}
        <li>{t("diag.empty")}</li>
      {/each}
    </ul>
  </details>
</section>
