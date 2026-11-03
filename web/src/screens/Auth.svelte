<script lang="ts">
  import { errorMessage } from "../lib/errors";
  import { t } from "../lib/i18n";
  import { bootstrap, createFirstAccount, register, session, signIn } from "../lib/session.svelte";

  // First run shows account creation; otherwise sign-in, with sign-up when the admin allows it.
  let wantsRegister = $state(false);
  const mode = $derived(session.status === "setup" ? "setup" : wantsRegister ? "register" : "signin");

  let username = $state("");
  let password = $state("");
  let confirm = $state("");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const creating = $derived(mode !== "signin");

  async function submit(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    error = null;
    if (creating && password !== confirm) {
      error = t("auth.mismatch");
      return;
    }
    busy = true;
    try {
      if (mode === "setup") await createFirstAccount(username, password);
      else if (mode === "register") await register(username, password);
      else await signIn(username, password);
      password = confirm = "";
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

{#if session.status === "error"}
  <div class="grid gap-3 text-center">
    <div class="alert tone-danger" role="alert">
      <svg class="size-5 shrink-0"><use href="/icons.svg#circle-x" /></svg>
      <div>{t("auth.unreachable")}</div>
    </div>
    <button class="btn btn-gray" onclick={bootstrap}>{t("common.retry")}</button>
  </div>
{:else}
  <form class="grid gap-4" onsubmit={submit}>
    <div>
      <h1 class="t-large">
        {mode === "setup" ? t("auth.setup.title") : mode === "register" ? t("auth.register.title") : t("auth.signin.title")}
      </h1>
      {#if mode === "setup"}
        <p class="mt-2 text-subtext0">{t("auth.setup.intro")}</p>
      {/if}
    </div>

    {#if error}
      <div class="alert tone-danger" role="alert">
        <svg class="size-5 shrink-0"><use href="/icons.svg#circle-x" /></svg>
        <div>{error}</div>
      </div>
    {/if}

    <div class="grid gap-3">
      <label class="grid gap-1.5">
        <span class="t-foot text-subtext0">{t("auth.username")}</span>
        <input
          class="field"
          bind:value={username}
          autocomplete="username"
          autocapitalize="none"
          spellcheck="false"
          required
        />
      </label>
      <label class="grid gap-1.5">
        <span class="t-foot text-subtext0">{t("auth.password")}</span>
        <input
          class="field"
          type="password"
          bind:value={password}
          autocomplete={creating ? "new-password" : "current-password"}
          required
        />
      </label>
      {#if creating}
        <label class="grid gap-1.5">
          <span class="t-foot text-subtext0">{t("auth.confirm")}</span>
          <input class="field" type="password" bind:value={confirm} autocomplete="new-password" required />
        </label>
      {/if}
    </div>

    <button class="btn btn-filled w-full" type="submit" disabled={busy}>
      {mode === "setup" ? t("auth.setup.submit") : mode === "register" ? t("auth.register.submit") : t("auth.signin.submit")}
    </button>

    {#if mode !== "setup" && session.registration === "open"}
      <button
        class="btn btn-plain w-full"
        type="button"
        onclick={() => {
          wantsRegister = !wantsRegister;
          error = null;
        }}
      >
        {wantsRegister ? t("auth.link.signin") : t("auth.link.register")}
      </button>
    {/if}
  </form>
{/if}
