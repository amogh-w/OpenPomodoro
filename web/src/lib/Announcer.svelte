<script lang="ts">
  import { app } from "./appState.svelte";
  import { announcementFor } from "./announce";
  import { t } from "./i18n";

  // A visually hidden live region: assistive technology reads whatever text appears in it.
  let message = $state("");
  let previous: string | null = null;

  $effect(() => {
    const state = app.timer?.state ?? null;
    if (state === null) {
      previous = null; // signed out or not loaded yet: the next state seen is "the first look"
      return;
    }
    const key = announcementFor(previous, state);
    previous = state;
    if (key !== null) message = t(key);
  });
</script>

<div class="sr-only" role="status" aria-live="polite" aria-atomic="true">{message}</div>
