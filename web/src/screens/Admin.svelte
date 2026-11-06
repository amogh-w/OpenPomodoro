<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "../lib/api";
  import HooksPanel from "../lib/HooksPanel.svelte";
  import ServerLogPanel from "../lib/ServerLogPanel.svelte";
  import { errorMessage } from "../lib/errors";
  import { t } from "../lib/i18n";
  import { session } from "../lib/session.svelte";
  import type { User } from "../lib/types";

  let users = $state<User[]>([]);
  let open = $state(false);
  let notice = $state<{ tone: "success" | "danger"; text: string } | null>(null);

  let newName = $state("");
  let newPassword = $state("");
  let newRole = $state<"user" | "admin">("user");
  let creating = $state(false);

  // Which user's "temporary password" panel is open, and what has been typed there.
  let resetFor = $state<string | null>(null);
  let resetPassword = $state("");

  function say(tone: "success" | "danger", text: string): void {
    notice = { tone, text };
  }

  async function load(): Promise<void> {
    try {
      const [list, registration] = await Promise.all([api.adminUsers(), api.adminRegistration()]);
      users = list;
      open = registration.policy === "open";
    } catch (e) {
      say("danger", errorMessage(e));
    }
  }

  async function toggleRegistration(): Promise<void> {
    const wanted = !open;
    try {
      await api.adminSetRegistration(wanted ? "open" : "closed");
      open = wanted;
    } catch (e) {
      say("danger", errorMessage(e));
    }
  }

  async function createUser(event: SubmitEvent): Promise<void> {
    event.preventDefault();
    creating = true;
    notice = null;
    try {
      await api.adminCreateUser(newName, newPassword, newRole);
      newName = newPassword = "";
      newRole = "user";
      say("success", t("admin.created"));
      await load();
    } catch (e) {
      say("danger", errorMessage(e));
    } finally {
      creating = false;
    }
  }

  async function toggleDisabled(user: User): Promise<void> {
    try {
      await api.adminSetDisabled(user.id, !user.disabled);
      await load();
    } catch (e) {
      say("danger", errorMessage(e));
    }
  }

  async function applyReset(event: SubmitEvent, user: User): Promise<void> {
    event.preventDefault();
    try {
      await api.adminSetPassword(user.id, resetPassword);
      resetPassword = "";
      resetFor = null;
      say("success", t("admin.setPasswordDone"));
    } catch (e) {
      say("danger", errorMessage(e));
    }
  }

  onMount(load);
</script>

<div class="flex items-center gap-2">
  <a class="btn btn-plain btn-sm gap-0.5 px-2" href="#/settings">
    <svg class="size-5"><use href="/icons.svg#chevron-left" /></svg>{t("common.back")}
  </a>
</div>
<h1 class="t-large mt-2">{t("admin.title")}</h1>

{#if notice}
  <div class="alert mt-3 {notice.tone === 'success' ? 'tone-success' : 'tone-danger'}" role="status">
    <svg class="size-5 shrink-0"><use href="/icons.svg#{notice.tone === 'success' ? 'circle-check' : 'circle-x'}" /></svg>
    <div>{notice.text}</div>
  </div>
{/if}

<section class="card mt-4 p-4">
  <label class="flex items-center justify-between gap-3">
    <span>
      <span class="block">{t("admin.registration")}</span>
      <span class="t-foot text-subtext1">{t("admin.registrationHint")}</span>
    </span>
    <span class="switch"><input type="checkbox" checked={open} onchange={toggleRegistration} /></span>
  </label>
</section>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-1 p-2" aria-label={t("admin.title")}>
  {#each users as user (user.id)}
    <div class="grid gap-2 rounded-lg p-2">
      <div class="flex items-center justify-between gap-2">
        <div class="min-w-0">
          <div class="truncate t-headline">
            {user.username}
            {#if user.id === session.user?.id}<span class="t-foot text-subtext1">({t("admin.you")})</span>{/if}
          </div>
          <div class="t-foot text-subtext1">
            {user.role === "admin" ? t("admin.role.admin") : t("admin.role.user")}
            {#if user.disabled}· {t("admin.disabled")}{/if}
          </div>
        </div>
        <div class="flex shrink-0 gap-2">
          <button
            class="btn btn-gray btn-sm"
            onclick={() => {
              resetFor = resetFor === user.id ? null : user.id;
              resetPassword = "";
            }}
          >
            {t("admin.setPassword")}
          </button>
          {#if user.id !== session.user?.id}
            <button class="btn btn-gray btn-sm" onclick={() => toggleDisabled(user)}>
              {user.disabled ? t("admin.enable") : t("admin.disable")}
            </button>
          {/if}
        </div>
      </div>
      {#if resetFor === user.id}
        <form class="flex gap-2" onsubmit={(e) => applyReset(e, user)}>
          <input
            class="field min-w-0 flex-1"
            type="password"
            bind:value={resetPassword}
            aria-label={t("admin.setPassword")}
            autocomplete="new-password"
            required
          />
          <button class="btn btn-filled" type="submit">{t("admin.apply")}</button>
        </form>
      {/if}
    </div>
  {/each}
</section>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-2 p-4">
  <a class="btn btn-gray" href="/api/admin/backup" download>{t("admin.backup")}</a>
  <p class="t-foot text-subtext1">{t("admin.backup.hint")}</p>
</section>

<form class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 p-4" onsubmit={createUser}>
  <h2 class="t-title3">{t("admin.add")}</h2>
  <label class="grid gap-1.5">
    <span class="t-foot text-subtext1">{t("auth.username")}</span>
    <input class="field" bind:value={newName} autocomplete="off" autocapitalize="none" spellcheck="false" required />
  </label>
  <label class="grid gap-1.5">
    <span class="t-foot text-subtext1">{t("auth.password")}</span>
    <input class="field" type="password" bind:value={newPassword} autocomplete="new-password" required />
  </label>
  <div class="flex flex-wrap gap-2" role="group" aria-label={t("admin.add")}>
    <button
      class="chip"
      type="button"
      aria-pressed={newRole === "user"}
      onclick={() => (newRole = "user")}>{t("admin.role.user")}</button
    >
    <button
      class="chip"
      type="button"
      aria-pressed={newRole === "admin"}
      onclick={() => (newRole = "admin")}>{t("admin.role.admin")}</button
    >
  </div>
  <button class="btn btn-filled" type="submit" disabled={creating}>{t("admin.create")}</button>
</form>

<HooksPanel />

<ServerLogPanel />
