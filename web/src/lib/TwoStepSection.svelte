<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./api";
  import { errorMessage } from "./errors";
  import { t } from "./i18n";

  let enabled = $state(false);
  let left = $state(0);
  let loaded = $state(false);
  let step = $state<"idle" | "password" | "code" | "off">("idle");
  let password = $state("");
  let code = $state("");
  let setup = $state<{ secret: string; uri: string } | null>(null);
  /** The recovery codes of a setup just finished. Shown once; they are not kept anywhere in the page afterwards. */
  let recovery = $state<string[] | null>(null);
  let problem = $state<string | null>(null);
  let copied = $state(false);

  async function load(): Promise<void> {
    try {
      const status = await api.totpStatus();
      enabled = status.enabled;
      left = status.recovery_left;
    } finally {
      loaded = true;
    }
  }
  onMount(() => void load());

  function reset(): void {
    step = "idle";
    password = code = "";
    setup = null;
    problem = null;
  }

  async function start(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    problem = null;
    try {
      setup = await api.totpStart(password);
      password = "";
      step = "code";
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  async function enable(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    problem = null;
    try {
      recovery = (await api.totpEnable(code)).recovery_codes;
      reset();
      await load();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  async function disable(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    problem = null;
    try {
      await api.totpDisable(password);
      reset();
      await load();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  async function copy(text: string): Promise<void> {
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
    } catch {
      copied = false;
    }
  }
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 p-4" aria-labelledby="twostep-title">
  <h2 id="twostep-title" class="t-title3">{t("twostep.title")}</h2>
  <p class="t-foot text-subtext1">{t("twostep.hint")}</p>

  {#if recovery}
    <div class="alert tone-success grid gap-2" role="status">
      <span>{t("twostep.recovery.title")}</span>
      <ul class="grid grid-cols-2 gap-1 font-mono text-sm">
        {#each recovery as c (c)}<li>{c}</li>{/each}
      </ul>
      <span class="flex flex-wrap gap-2">
        <button class="btn btn-gray btn-sm" onclick={() => copy((recovery ?? []).join("\n"))}>{copied ? t("access.copied") : t("access.copy")}</button>
        <button class="btn btn-plain btn-sm" onclick={() => { recovery = null; copied = false; }}>{t("common.dismiss")}</button>
      </span>
      <span class="t-foot">{t("twostep.recovery.once")}</span>
    </div>
  {/if}

  {#if loaded}
    {#if enabled && step !== "off"}
      <p>{t("twostep.on", { n: left })}</p>
      <button class="btn btn-gray justify-self-start" onclick={() => { step = "off"; problem = null; }}>{t("twostep.turnOff")}</button>
    {:else if step === "off"}
      <form class="grid gap-2" onsubmit={disable}>
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("account.currentPassword")}</span>
          <input class="field" type="password" bind:value={password} autocomplete="current-password" required />
        </label>
        <div class="flex gap-2">
          <button class="btn btn-danger-tinted" type="submit">{t("twostep.turnOff")}</button>
          <button class="btn btn-plain" type="button" onclick={reset}>{t("common.cancel")}</button>
        </div>
      </form>
    {:else if step === "idle"}
      <button class="btn btn-gray justify-self-start" onclick={() => (step = "password")}>{t("twostep.turnOn")}</button>
    {:else if step === "password"}
      <form class="grid gap-2" onsubmit={start}>
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("account.currentPassword")}</span>
          <input class="field" type="password" bind:value={password} autocomplete="current-password" required />
        </label>
        <div class="flex gap-2">
          <button class="btn btn-filled" type="submit">{t("twostep.continue")}</button>
          <button class="btn btn-plain" type="button" onclick={reset}>{t("common.cancel")}</button>
        </div>
      </form>
    {:else if step === "code" && setup}
      <p>{t("twostep.addKey")}</p>
      <code class="break-all rounded-md bg-surface0 p-2 text-sm">{setup.secret}</code>
      <div class="flex flex-wrap gap-2">
        <button class="btn btn-gray btn-sm" onclick={() => copy(setup?.secret ?? "")}>{copied ? t("access.copied") : t("twostep.copyKey")}</button>
        <a class="btn btn-plain btn-sm" href={setup.uri}>{t("twostep.openApp")}</a>
      </div>
      <form class="grid gap-2" onsubmit={enable}>
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext1">{t("twostep.enterCode")}</span>
          <input class="field" bind:value={code} inputmode="numeric" autocomplete="one-time-code" maxlength="7" required />
        </label>
        <div class="flex gap-2">
          <button class="btn btn-filled" type="submit">{t("twostep.finish")}</button>
          <button class="btn btn-plain" type="button" onclick={reset}>{t("common.cancel")}</button>
        </div>
      </form>
    {/if}
  {/if}

  {#if problem}<p class="alert tone-danger" role="alert">{problem}</p>{/if}
</section>
