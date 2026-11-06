<script lang="ts">
  import { onMount } from "svelte";
  import { type SignedInBrowser, type TokenInfo, api } from "./api";
  import { tick } from "./appState.svelte";
  import { errorMessage } from "./errors";
  import { formatAgo } from "./format";
  import { t } from "./i18n";
  import { describeAgent } from "./userAgent";

  let tokens = $state<TokenInfo[]>([]);
  let sessions = $state<SignedInBrowser[]>([]);
  let name = $state("");
  /** The secret of the token just made. Shown once; it is not kept anywhere else. */
  let fresh = $state<{ name: string; token: string } | null>(null);
  let copied = $state(false);
  let problem = $state<string | null>(null);

  async function load(): Promise<void> {
    try {
      [tokens, sessions] = await Promise.all([api.listTokens().then((r) => r.tokens), api.listSessions().then((r) => r.sessions)]);
    } catch (error) {
      problem = errorMessage(error);
    }
  }
  onMount(() => void load());

  async function create(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    problem = null;
    copied = false;
    try {
      const made = await api.createToken(name.trim());
      fresh = { name: made.name, token: made.token };
      name = "";
      await load();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  async function copy(): Promise<void> {
    if (!fresh) return;
    try {
      await navigator.clipboard.writeText(fresh.token);
      copied = true;
    } catch {
      copied = false;
    }
  }

  async function revokeToken(id: string): Promise<void> {
    problem = null;
    try {
      await api.revokeToken(id);
      await load();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  async function revokeSession(id: string): Promise<void> {
    problem = null;
    try {
      await api.revokeSession(id);
      await load();
    } catch (error) {
      problem = errorMessage(error);
    }
  }

  const ago = (ms: number): string => formatAgo(ms, tick.now);
  const agentName = (ua: string): string => {
    const a = describeAgent(ua);
    return a ? t("access.agent", { browser: a.browser, os: a.os }) : t("access.unknownBrowser");
  };
  const others = $derived(sessions.filter((s) => !s.current));

  async function signOutOthers(): Promise<void> {
    problem = null;
    try {
      for (const s of others) await api.revokeSession(s.id);
      await load();
    } catch (error) {
      problem = errorMessage(error);
    }
  }
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-4 p-4" aria-labelledby="access-title">
  <h2 id="access-title" class="t-title3">{t("access.title")}</h2>

  <div class="grid gap-2">
    <h3 class="t-headline">{t("access.sessions")}</h3>
    <p class="t-foot text-subtext1">{t("access.sessions.hint")}</p>
    <ul class="grid gap-2">
      {#each sessions as s (s.id)}
        <li class="flex items-center justify-between gap-2">
          <span class="min-w-0 flex-1">
            <span class="block truncate" title={s.agent}>{agentName(s.agent)}</span>
            <span class="t-foot block text-subtext1">
              {s.current ? t("access.thisBrowser") : t("access.lastUsed", { when: ago(s.last_seen) })}
            </span>
          </span>
          {#if !s.current}
            <button class="btn btn-gray btn-sm shrink-0" onclick={() => revokeSession(s.id)}>{t("access.signOut")}</button>
          {/if}
        </li>
      {/each}
    </ul>
    {#if others.length > 1}
      <button class="btn btn-plain btn-sm justify-self-start" onclick={signOutOthers}>{t("access.signOutOthers", { n: others.length })}</button>
    {/if}
  </div>

  <div class="grid gap-2">
    <h3 class="t-headline">{t("access.tokens")}</h3>
    <p class="t-foot text-subtext1">{t("access.tokens.hint")}</p>

    {#if fresh}
      <div class="alert tone-success grid gap-2" role="status">
        <span>{t("access.tokens.created", { name: fresh.name })}</span>
        <code class="break-all rounded-md bg-surface0 p-2 text-sm">{fresh.token}</code>
        <span class="flex flex-wrap gap-2">
          <button class="btn btn-gray btn-sm" onclick={copy}>{copied ? t("access.copied") : t("access.copy")}</button>
          <button class="btn btn-plain btn-sm" onclick={() => (fresh = null)}>{t("common.dismiss")}</button>
        </span>
        <span class="t-foot">{t("access.tokens.once")}</span>
      </div>
    {/if}

    <ul class="grid gap-2">
      {#each tokens as token (token.id)}
        <li class="flex items-center justify-between gap-2">
          <span class="min-w-0 flex-1">
            <span class="block truncate">{token.name}</span>
            <span class="t-foot block text-subtext1">
              {token.last_used === null ? t("access.neverUsed") : t("access.lastUsed", { when: ago(token.last_used) })}
            </span>
          </span>
          <button class="btn btn-plain btn-danger btn-sm shrink-0" aria-label={t("access.revoke", { name: token.name })} onclick={() => revokeToken(token.id)}>
            {t("access.revokeShort")}
          </button>
        </li>
      {/each}
    </ul>

    <form class="flex min-w-0 gap-2" onsubmit={create}>
      <input class="field min-w-0 flex-1" bind:value={name} maxlength="60" placeholder={t("access.tokens.placeholder")} aria-label={t("access.tokens.name")} autocomplete="off" />
      <button class="btn btn-gray shrink-0" type="submit" disabled={name.trim() === ""}>{t("access.tokens.make")}</button>
    </form>
  </div>

  {#if problem}<p class="alert tone-danger" role="alert">{problem}</p>{/if}
</section>
