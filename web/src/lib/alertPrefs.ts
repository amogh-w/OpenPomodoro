// Per-device alert preferences: how loud, whether to notify, quiet hours, and how often to nag. They belong to the
// device (a phone and a desktop want different things), so they are stored locally and not synced.

export interface AlertPrefs {
  /** Sounds on or off. */
  sound: boolean;
  /** 0 to 100. */
  volume: number;
  /** Show system notifications (also needs the browser's permission). */
  notify: boolean;
  /** Minutes before the end for the "ending soon" tone; 0 is off. */
  warnMin: number;
  /** Minutes between reminders while a phase is in overtime; 0 is off. */
  reminderMin: number;
  quietEnabled: boolean;
  /** "HH:MM", local time. Quiet hours may cross midnight. */
  quietStart: string;
  quietEnd: string;
  /** A once-a-day nudge to start working, while the app is open. */
  dailyEnabled: boolean;
  /** "HH:MM", local time. */
  dailyTime: string;
  /** Monday first: which weekdays the nudge is for. */
  dailyDays: boolean[];
}

export const defaultAlertPrefs: AlertPrefs = {
  sound: true,
  volume: 70,
  notify: false,
  warnMin: 1,
  reminderMin: 5,
  quietEnabled: false,
  quietStart: "22:00",
  quietEnd: "07:00",
  dailyEnabled: false,
  dailyTime: "09:00",
  dailyDays: [true, true, true, true, true, false, false],
};

/** Most overtime reminders for one phase, however long it runs. */
export const MAX_REMINDERS = 12;

const TIME = /^([01]\d|2[0-3]):([0-5]\d)$/;

const num = (v: unknown, min: number, max: number, fallback: number): number =>
  typeof v === "number" && Number.isFinite(v) ? Math.min(max, Math.max(min, Math.round(v))) : fallback;
const flag = (v: unknown, fallback: boolean): boolean => (typeof v === "boolean" ? v : fallback);
const time = (v: unknown, fallback: string): string => (typeof v === "string" && TIME.test(v) ? v : fallback);

/** Read stored preferences, keeping anything valid and falling back per field for anything that is not. */
export function parseAlertPrefs(raw: unknown): AlertPrefs {
  const o = (typeof raw === "object" && raw !== null ? raw : {}) as Record<string, unknown>;
  const d = defaultAlertPrefs;
  return {
    sound: flag(o.sound, d.sound),
    volume: num(o.volume, 0, 100, d.volume),
    notify: flag(o.notify, d.notify),
    warnMin: num(o.warnMin, 0, 30, d.warnMin),
    reminderMin: num(o.reminderMin, 0, 60, d.reminderMin),
    quietEnabled: flag(o.quietEnabled, d.quietEnabled),
    quietStart: time(o.quietStart, d.quietStart),
    quietEnd: time(o.quietEnd, d.quietEnd),
    dailyEnabled: flag(o.dailyEnabled, d.dailyEnabled),
    dailyTime: time(o.dailyTime, d.dailyTime),
    dailyDays:
      Array.isArray(o.dailyDays) && o.dailyDays.length === 7
        ? o.dailyDays.map((v, i) => flag(v, d.dailyDays[i] ?? false))
        : [...d.dailyDays],
  };
}

const minutesOf = (hhmm: string): number => Number(hhmm.slice(0, 2)) * 60 + Number(hhmm.slice(3, 5));

/**
 * Is `date` (local time) inside quiet hours? The start is included and the end is not; a range whose end is
 * earlier than its start runs through midnight; equal start and end means no quiet time at all.
 */
export function inQuietHours(date: Date, prefs: Pick<AlertPrefs, "quietEnabled" | "quietStart" | "quietEnd">): boolean {
  if (!prefs.quietEnabled) return false;
  const start = minutesOf(prefs.quietStart);
  const end = minutesOf(prefs.quietEnd);
  const now = date.getHours() * 60 + date.getMinutes();
  if (start === end) return false;
  return start < end ? now >= start && now < end : now >= start || now < end;
}

/** How long after the chosen time the daily nudge may still appear (the app may have been opened a little late). */
export const DAILY_GRACE_MIN = 10;

/**
 * Is the daily nudge due at `date`? It is for the chosen weekdays, from the chosen time until a few minutes after,
 * and only once a day (`lastDay` is the local day it last showed, `YYYY-MM-DD`).
 */
export function dailyReminderDue(
  date: Date,
  prefs: Pick<AlertPrefs, "dailyEnabled" | "dailyTime" | "dailyDays">,
  lastDay: string | null,
): boolean {
  if (!prefs.dailyEnabled) return false;
  const weekdayFromMonday = (date.getDay() + 6) % 7;
  if (!prefs.dailyDays[weekdayFromMonday]) return false;
  const day = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
  if (lastDay === day) return false;
  const now = date.getHours() * 60 + date.getMinutes();
  const at = minutesOf(prefs.dailyTime);
  return now >= at && now < at + DAILY_GRACE_MIN;
}
