import { describe, expect, it } from "vitest";
import { dailyReminderDue, defaultAlertPrefs, inQuietHours, parseAlertPrefs } from "./alertPrefs";

const at = (h: number, m = 0): Date => new Date(2026, 10, 3, h, m);
const quiet = (start: string, end: string, enabled = true) => ({ quietEnabled: enabled, quietStart: start, quietEnd: end });

describe("parseAlertPrefs", () => {
  it("gives the defaults for nothing or junk", () => {
    for (const raw of [undefined, null, 5, "x", []]) expect(parseAlertPrefs(raw)).toEqual(defaultAlertPrefs);
  });

  it("keeps valid values", () => {
    const p = parseAlertPrefs({ sound: false, volume: 30, notify: true, warnMin: 2, reminderMin: 10, nudgeMin: 15, tickFocus: true, quietEnabled: true, quietStart: "23:30", quietEnd: "06:15" });
    expect(p).toEqual({
      sound: false,
      volume: 30,
      notify: true,
      warnMin: 2,
      reminderMin: 10,
      nudgeMin: 15,
      tickFocus: true,
      tickBreak: false,
      speak: false,
      quietEnabled: true,
      quietStart: "23:30",
      quietEnd: "06:15",
      dailyEnabled: false,
      dailyTime: "09:00",
      dailyDays: [true, true, true, true, true, false, false],
    });
  });

  it("keeps the daily reminder choices and repairs bad ones", () => {
    const p = parseAlertPrefs({ dailyEnabled: true, dailyTime: "07:45", dailyDays: [true, false, true, false, true, false, true] });
    expect([p.dailyEnabled, p.dailyTime, p.dailyDays]).toEqual([true, "07:45", [true, false, true, false, true, false, true]]);
    const bad = parseAlertPrefs({ dailyTime: "nine", dailyDays: [true, true] });
    expect([bad.dailyTime, bad.dailyDays]).toEqual([defaultAlertPrefs.dailyTime, defaultAlertPrefs.dailyDays]);
  });

  it("clamps numbers and falls back field by field", () => {
    const p = parseAlertPrefs({ volume: 500, warnMin: -3, reminderMin: "soon", quietStart: "25:00", quietEnd: "7:00", sound: "yes" });
    expect(p.volume).toBe(100);
    expect(p.warnMin).toBe(0);
    expect(p.reminderMin).toBe(defaultAlertPrefs.reminderMin);
    expect(p.quietStart).toBe(defaultAlertPrefs.quietStart);
    expect(p.quietEnd).toBe(defaultAlertPrefs.quietEnd);
    expect(p.sound).toBe(defaultAlertPrefs.sound);
  });

  it("rounds fractional numbers and ignores NaN", () => {
    expect(parseAlertPrefs({ volume: 33.6 }).volume).toBe(34);
    expect(parseAlertPrefs({ volume: Number.NaN }).volume).toBe(defaultAlertPrefs.volume);
  });
});

describe("inQuietHours", () => {
  it("is never quiet when switched off", () => {
    expect(inQuietHours(at(23), quiet("22:00", "07:00", false))).toBe(false);
  });

  it("covers a same-day range, start included and end excluded", () => {
    const q = quiet("13:00", "14:30");
    expect([at(12, 59), at(13), at(14, 29), at(14, 30)].map((d) => inQuietHours(d, q))).toEqual([false, true, true, false]);
  });

  it("runs through midnight", () => {
    const q = quiet("22:00", "07:00");
    expect([at(21, 59), at(22), at(23, 59), at(0), at(6, 59), at(7), at(12)].map((d) => inQuietHours(d, q))).toEqual([
      false, true, true, true, true, false, false,
    ]);
  });

  it("treats equal start and end as no quiet time", () => {
    expect(inQuietHours(at(5), quiet("08:00", "08:00"))).toBe(false);
  });
});

describe("dailyReminderDue", () => {
  const prefs = { dailyEnabled: true, dailyTime: "09:00", dailyDays: [true, true, true, true, true, false, false] };
  const monday = (h: number, m: number): Date => new Date(2026, 10, 2, h, m); // 2 November 2026 is a Monday

  it("is due from the chosen time for a few minutes, once", () => {
    expect(dailyReminderDue(monday(8, 59), prefs, null)).toBe(false);
    expect(dailyReminderDue(monday(9, 0), prefs, null)).toBe(true);
    expect(dailyReminderDue(monday(9, 9), prefs, null)).toBe(true);
    expect(dailyReminderDue(monday(9, 10), prefs, null)).toBe(false);
    expect(dailyReminderDue(monday(9, 1), prefs, "2026-11-02")).toBe(false);
    expect(dailyReminderDue(monday(9, 1), prefs, "2026-11-01")).toBe(true);
  });

  it("respects the weekdays and the switch", () => {
    expect(dailyReminderDue(new Date(2026, 10, 7, 9, 1), prefs, null)).toBe(false); // Saturday
    expect(dailyReminderDue(monday(9, 1), { ...prefs, dailyEnabled: false }, null)).toBe(false);
  });
});
