// Browser adapter: system notifications. They only appear while the app is open somewhere (browsers give no way
// to schedule one for a closed app), and only after the person has allowed them.
export type Permission = NotificationPermission | "unsupported";

export const notificationsSupported = (): boolean => typeof Notification !== "undefined";

export function permission(): Permission {
  return notificationsSupported() ? Notification.permission : "unsupported";
}

/** Ask the browser. Must be called from a tap or click. */
export async function requestPermission(): Promise<Permission> {
  if (!notificationsSupported()) return "unsupported";
  try {
    return await Notification.requestPermission();
  } catch {
    return Notification.permission;
  }
}

/** Show a notification. Goes through the service worker where there is one (required on phones). */
export async function notify(title: string, body: string, tag: string): Promise<void> {
  if (permission() !== "granted") return;
  const options: NotificationOptions = { body, tag, icon: "/icon-192.png" };
  try {
    const registration = await navigator.serviceWorker?.getRegistration();
    if (registration) {
      await registration.showNotification(title, options);
      return;
    }
  } catch {
    // fall through to the page-level API
  }
  try {
    new Notification(title, options);
  } catch {
    // Blocked or unsupported in this context.
  }
}
