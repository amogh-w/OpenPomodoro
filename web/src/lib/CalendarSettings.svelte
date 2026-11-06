<script lang="ts">
  import { calendar, clearCalendar, importCalendar } from "./calendarStore.svelte";
  import { t } from "./i18n";

  let problem = $state<string | null>(null);
  let done = $state<number | null>(null);

  async function choose(event: Event & { currentTarget: HTMLInputElement }): Promise<void> {
    const input = event.currentTarget;
    const file = input.files?.[0];
    input.value = "";
    if (!file) return;
    problem = null;
    done = null;
    try {
      done = await importCalendar(file);
    } catch (error) {
      problem = error instanceof Error && error.message === "too_large" ? t("calendar.tooLarge") : t("calendar.empty");
    }
  }
</script>

<section class="card mt-4 grid min-w-0 grid-cols-[minmax(0,1fr)] gap-3 p-4" aria-labelledby="calendar-title">
  <h2 id="calendar-title" class="t-title3">{t("calendar.title")}</h2>
  <p class="t-foot text-subtext1">{t("calendar.hint")}</p>
  {#if calendar.events.length > 0}
    <p>{t("calendar.loaded", { name: calendar.name, n: calendar.events.length })}</p>
  {/if}
  <div class="flex flex-wrap gap-2">
    <label class="btn btn-gray btn-sm cursor-pointer">
      {calendar.events.length > 0 ? t("calendar.replace") : t("calendar.choose")}
      <input class="sr-only" type="file" accept=".ics,text/calendar" onchange={choose} />
    </label>
    {#if calendar.events.length > 0}
      <button class="btn btn-plain btn-sm" onclick={() => { void clearCalendar(); done = null; }}>{t("calendar.remove")}</button>
    {/if}
  </div>
  {#if done !== null}<p class="t-foot" role="status">{t("calendar.done", { n: done })}</p>{/if}
  {#if problem}<p class="alert tone-warn" role="alert">{problem}</p>{/if}
</section>
