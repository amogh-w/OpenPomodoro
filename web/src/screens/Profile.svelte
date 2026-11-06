<script lang="ts">
  import GardenCard from "../lib/GardenCard.svelte";
  import AccessSettings from "../lib/AccessSettings.svelte";
  import AccountSection from "../lib/AccountSection.svelte";
  import TwoStepSection from "../lib/TwoStepSection.svelte";
  import DataSection from "../lib/DataSection.svelte";
  import { t } from "../lib/i18n";
  import { session } from "../lib/session.svelte";
</script>

<h1 class="sr-only">{t("profile.title")}</h1>

<section class="card mt-4 grid justify-items-center gap-1 p-6 text-center" aria-label={t("profile.title")}>
  <p class="t-title1 text-accent">{session.user?.username ?? ""}</p>
  <p class="text-subtext1">{session.user?.role === "admin" ? t("admin.role.admin") : t("profile.member")}</p>
  {#if session.user}
    <p class="t-foot text-subtext1">{t("profile.since", { date: new Date(session.user.created_at).toLocaleDateString(undefined, { day: "numeric", month: "long", year: "numeric" }) })}</p>
  {/if}
</section>

<GardenCard />

<div class="mt-4 lg:columns-2 lg:gap-x-6 [&>*]:break-inside-avoid lg:[&>*]:mt-0 lg:[&>*]:mb-6">
  <AccountSection />
  <TwoStepSection />
  <AccessSettings />
  <DataSection />
</div>
