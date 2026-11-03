// Browser adapter: keep the screen on while a phase runs. Feature-detected; where the browser has no wake
// lock, nothing happens and nothing breaks.
let sentinel: WakeLockSentinel | null = null;

export const wakeLockSupported = (): boolean => typeof navigator !== "undefined" && "wakeLock" in navigator;

export async function holdAwake(): Promise<void> {
  if (!wakeLockSupported() || sentinel !== null) return;
  try {
    const lock = await navigator.wakeLock.request("screen");
    sentinel = lock;
    // The browser drops the lock when the page is hidden; forget it so it can be taken again.
    lock.addEventListener("release", () => {
      if (sentinel === lock) sentinel = null;
    });
  } catch {
    // Refused (low battery, no permission): the screen simply may sleep.
  }
}

export async function releaseAwake(): Promise<void> {
  const lock = sentinel;
  sentinel = null;
  try {
    await lock?.release();
  } catch {
    // Already released.
  }
}
