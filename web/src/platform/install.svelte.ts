// Browser adapter: the install prompt. Only offered where the browser fires beforeinstallprompt.
interface BeforeInstallPromptEvent extends Event {
  prompt(): Promise<void>;
}

export const install = $state({
  available: false,
  installed: matchMedia("(display-mode: standalone)").matches,
});

let deferred: BeforeInstallPromptEvent | null = null;

addEventListener("beforeinstallprompt", (event) => {
  event.preventDefault();
  deferred = event as BeforeInstallPromptEvent;
  install.available = true;
});

addEventListener("appinstalled", () => {
  deferred = null;
  install.available = false;
  install.installed = true;
});

export async function promptInstall(): Promise<void> {
  if (!deferred) return;
  await deferred.prompt();
  deferred = null;
  install.available = false;
}
