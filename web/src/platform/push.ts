// Browser adapter: alerts while the app is closed, through Web Push. The browser subscribes with the server's key; the
// server sends a short message when a phase ends or the daily reminder is due and nothing is open. Works where the
// browser supports push (on iPhone only for an app added to the home screen).
import { api } from "../lib/api";
import { t } from "../lib/i18n";
import { activeLanguage } from "../lib/language.svelte";
import type { AlertPrefs } from "../lib/alertPrefs";

export type PushState = "unsupported" | "denied" | "off" | "on";

export const pushSupported = (): boolean =>
  typeof navigator !== "undefined" && "serviceWorker" in navigator && typeof PushManager !== "undefined" && typeof Notification !== "undefined";

/** The server's key as the bytes the browser wants (URL-safe base64 without padding). */
export function keyBytes(text: string): Uint8Array {
  const padded = text.replace(/-/g, "+").replace(/_/g, "/").padEnd(Math.ceil(text.length / 4) * 4, "=");
  const raw = atob(padded);
  return Uint8Array.from(raw, (c) => c.charCodeAt(0));
}

const minutes = (hhmm: string): number => Number(hhmm.slice(0, 2)) * 60 + Number(hhmm.slice(3, 5));

/** What this device wants the server to know: when to stay quiet, the daily reminder, and the words to use. */
export function pushPrefs(alerts: AlertPrefs, now: Date = new Date()) {
  void activeLanguage(); // the words below follow the language in use
  const days = alerts.dailyDays;
  return {
    phase: true,
    daily: alerts.dailyEnabled ? { minutes: minutes(alerts.dailyTime), days } : null,
    quiet: alerts.quietEnabled ? ([minutes(alerts.quietStart), minutes(alerts.quietEnd)] as [number, number]) : null,
    tz: -now.getTimezoneOffset(),
    texts: {
      focus_title: t("alerts.focusEnd.title"),
      focus_body: t("alerts.focusEnd.body"),
      break_title: t("alerts.breakEnd.title"),
      break_body: t("alerts.breakEnd.body"),
      daily_title: t("alerts.daily.title"),
      daily_body: t("alerts.daily.body"),
      test_title: t("push.test.title"),
      test_body: t("push.test.body"),
    },
  };
}

async function registration(): Promise<ServiceWorkerRegistration | null> {
  try {
    return (await navigator.serviceWorker.getRegistration()) ?? null;
  } catch {
    return null;
  }
}

export async function pushState(): Promise<PushState> {
  if (!pushSupported()) return "unsupported";
  if (Notification.permission === "denied") return "denied";
  const reg = await registration();
  return (await reg?.pushManager.getSubscription()) ? "on" : "off";
}

async function send(sub: PushSubscription, alerts: AlertPrefs): Promise<void> {
  const keys = sub.toJSON().keys;
  if (!keys?.p256dh || !keys.auth) throw new Error("no keys");
  await api.pushSubscribe(sub.endpoint, { p256dh: keys.p256dh, auth: keys.auth }, pushPrefs(alerts));
}

/** Ask permission (from a tap), subscribe and tell the server. Throws when the browser or the server refuses. */
export async function enablePush(alerts: AlertPrefs): Promise<void> {
  if (!pushSupported()) throw new Error("unsupported");
  if ((await Notification.requestPermission()) !== "granted") throw new Error("denied");
  const reg = await registration();
  if (!reg) throw new Error("no service worker");
  const { key } = await api.pushKey();
  const sub =
    (await reg.pushManager.getSubscription()) ??
    (await reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: keyBytes(key) as BufferSource }));
  await send(sub, alerts);
}

export async function disablePush(): Promise<void> {
  const sub = await (await registration())?.pushManager.getSubscription();
  if (!sub) return;
  const endpoint = sub.endpoint;
  await sub.unsubscribe().catch(() => false);
  await api.pushUnsubscribe(endpoint).catch(() => {});
}

/** Tell the server about changed preferences (quiet hours, daily reminder, language). Does nothing if not subscribed. */
export async function syncPush(alerts: AlertPrefs): Promise<void> {
  if (!pushSupported() || Notification.permission !== "granted") return;
  const sub = await (await registration())?.pushManager.getSubscription();
  if (sub) await send(sub, alerts).catch(() => {});
}
