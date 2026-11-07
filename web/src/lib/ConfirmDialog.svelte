<script lang="ts">
  import { t } from "./i18n";

  interface Props {
    title: string;
    body: string;
  }
  let { title, body }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let settle: ((ok: boolean) => void) | null = null;

  /** Ask a plain yes or no question; resolves true on yes. Closing the dialog any other way means no. */
  export function ask(): Promise<boolean> {
    dialog?.showModal();
    return new Promise((resolve) => {
      settle = resolve;
    });
  }

  function close(ok: boolean): void {
    dialog?.close();
    settle?.(ok);
    settle = null;
  }
</script>

<dialog bind:this={dialog} class="modal" aria-labelledby="confirm-title" oncancel={() => close(false)}>
  <div class="grid gap-3 p-5">
    <h2 id="confirm-title" class="t-title3">{title}</h2>
    <p class="text-subtext1">{body}</p>
    <div class="grid grid-cols-2 gap-2">
      <!-- No is the default, so a stray Enter never does the risky thing. -->
      <!-- svelte-ignore a11y_autofocus -->
      <button class="btn btn-filled" autofocus onclick={() => close(false)}>{t("confirm.no")}</button>
      <button class="btn btn-gray" onclick={() => close(true)}>{t("confirm.yes")}</button>
    </div>
  </div>
</dialog>
