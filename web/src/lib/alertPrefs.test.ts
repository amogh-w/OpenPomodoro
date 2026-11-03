import { describe, expect, it } from "vitest";
import { defaultAlertPrefs, inQuietHours, parseAlertPrefs } from "./alertPrefs";

const at = (h: number, m = 0): Date => new Date(2026, 10, 3, h, m);
const quiet = (start: string, end: string, enabled = true) => ({ quietEnabled: enabled, quietStart: start, quietEnd: end });

describe("parseAlertPrefs", () => {
  it("gives the defaults for nothing or junk", () => {
    for (const raw of [undefined, null, 5, "x", []]) expect(parseAlertPrefs(raw)).toEqual(defaultAlertPrefs);
  });

  it("keeps valid values", () => {
    const p = parseAlertPrefs({ sound: false, volume: 30, notify: true, warnMin: 2, reminderMin: 10, quietEnabled: true, quietStart: "23:30", quietEnd: "06:15" });
    expect(p).toEqual({ sound: false, volume: 30, notify: true, warnMin: 2, reminderMin: 10, quietEnabled: true, quietStart: "23:30", quietEnd: "06:15" });
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
