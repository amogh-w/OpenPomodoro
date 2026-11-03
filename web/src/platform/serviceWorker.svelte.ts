// Browser adapter: service worker registration and the "update ready" flow.
export const sw = $state({ updateReady: false, offlineReady: false });

let waiting: ServiceWorker | null = null;

export function registerServiceWorker(): void {
  if (!import.meta.env.PROD || !("serviceWorker" in navigator)) return;

  const hadController = Boolean(navigator.serviceWorker.controller);
  let reloading = false;
  navigator.serviceWorker.addEventListener("controllerchange", () => {
    // A first install is not an update; only reload when a running version was replaced.
    if (!hadController || reloading) return;
    reloading = true;
    location.reload();
  });

  navigator.serviceWorker.register("/sw.js").then((reg) => {
    if (reg.waiting && navigator.serviceWorker.controller) {
      waiting = reg.waiting;
      sw.updateReady = true;
    }
    reg.addEventListener("updatefound", () => {
      const worker = reg.installing;
      worker?.addEventListener("statechange", () => {
        if (worker.state !== "installed") return;
        if (navigator.serviceWorker.controller) {
          waiting = worker;
          sw.updateReady = true;
        } else {
          sw.offlineReady = true;
        }
      });
    });
  });
}

export function applyUpdate(): void {
  waiting?.postMessage({ type: "SKIP_WAITING" });
}
