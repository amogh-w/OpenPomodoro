<script lang="ts">
  interface Props {
    title: string;
    body: string;
    /** The button that goes ahead with the risky choice. */
    confirm: string;
    /** The button that keeps things as they are; it is the default one. */
    cancel: string;
  }
  let { title, body, confirm, cancel }: Props = $props();

  let dialog: HTMLDialogElement | undefined = $state();
  let settle: ((ok: boolean) => void) | null = null;

  /** Ask; resolves true if the person chose to go ahead. Closing the dialog any other way means no. */
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
    <div class="grid gap-2">
      <!-- svelte-ignore a11y_autofocus -->
      <button class="btn btn-filled" autofocus onclick={() => close(false)}>{cancel}</button>
      <button class="btn btn-gray" onclick={() => close(true)}>{confirm}</button>
    </div>
  </div>
</dialog>
