import { describe, expect, it } from "vitest";
import { MAX_REMINDERS } from "./alertPrefs";
import { type AlertConfig, type AlertInput, type AlertMemory, emptyMemory, nextAlerts } from "./alertPlan";

const MIN = 60_000;
const config: AlertConfig = { warnLeadMs: MIN, reminderMs: 5 * MIN };

function input(over: Partial<AlertInput> = {}): AlertInput {
  return { key: "focus:0:1500000", phase: "focus", paused: false, ended: false, remainingMs: 10 * MIN, overflowMs: 0, totalMs: 25 * MIN, ...over };
}

/** Feed a series of moments through, keeping the memory like the app does. */
function run(moments: Partial<AlertInput>[], cfg: AlertConfig = config) {
  let memory: AlertMemory = emptyMemory;
  return moments.map((m) => {
    const out = nextAlerts(input(m), memory, cfg);
    memory = out.memory;
    return out.alerts;
  });
}

describe("nextAlerts", () => {
  it("is quiet while a phase runs normally and with nothing to alert about", () => {
    expect(run([{}, { remainingMs: 5 * MIN }])).toEqual([[], []]);
    expect(nextAlerts(null, emptyMemory, config).alerts).toEqual([]);
    expect(nextAlerts(input({ key: null }), emptyMemory, config).alerts).toEqual([]);
  });

  it("warns once when the time left drops to the lead", () => {
    expect(run([{ remainingMs: 2 * MIN }, { remainingMs: MIN }, { remainingMs: 50_000 }, { remainingMs: 10_000 }])).toEqual([[], ["warn"], [], []]);
  });

  it("does not warn when the warning is switched off or the phase is short", () => {
    expect(run([{ remainingMs: MIN }], { ...config, warnLeadMs: 0 })).toEqual([[]]);
    expect(run([{ remainingMs: MIN, totalMs: 2 * MIN }])).toEqual([[]]);
    expect(run([{ remainingMs: MIN, totalMs: 2 * MIN + 1 }])).toEqual([["warn"]]);
  });

  it("announces the end of focus once", () => {
    expect(run([{ remainingMs: 1_000 }, { ended: true, remainingMs: 0, overflowMs: 0 }, { ended: true, remainingMs: 0, overflowMs: 2_000 }])).toEqual([["warn"], ["focus_end"], []]);
  });

  it("announces the end of a break differently", () => {
    const brk = { key: "short_break:0:300000", phase: "break" as const, totalMs: 5 * MIN };
    expect(run([{ ...brk, ended: true, remainingMs: 0 }])).toEqual([["break_end"]]);
  });

  it("repeats a reminder each interval in overtime, up to the limit", () => {
    const over = (n: number) => ({ ended: true, remainingMs: 0, overflowMs: n * 5 * MIN + 1_000 });
    const moments = [{ ended: true, remainingMs: 0, overflowMs: 0 }, ...Array.from({ length: 16 }, (_, i) => over(i + 1))];
    const alerts = run(moments).flat();
    expect(alerts[0]).toBe("focus_end");
    const reminders = alerts.filter((a) => a === "focus_reminder");
    expect(reminders).toHaveLength(MAX_REMINDERS);
  });

  it("does not remind between intervals", () => {
    expect(run([
      { ended: true, remainingMs: 0, overflowMs: 0 },
      { ended: true, remainingMs: 0, overflowMs: 4 * MIN },
      { ended: true, remainingMs: 0, overflowMs: 5 * MIN },
      { ended: true, remainingMs: 0, overflowMs: 6 * MIN },
    ])).toEqual([["focus_end"], [], ["focus_reminder"], []]);
  });

  it("has no reminders when they are off", () => {
    expect(run([{ ended: true, remainingMs: 0, overflowMs: 0 }, { ended: true, remainingMs: 0, overflowMs: 60 * MIN }], { ...config, reminderMs: 0 })).toEqual([["focus_end"], []]);
  });

  it("stays silent while paused, then carries on", () => {
    expect(run([{ remainingMs: MIN, paused: true }, { remainingMs: MIN }])).toEqual([[], ["warn"]]);
  });

  it("starts afresh for a new phase or after the end is extended", () => {
    expect(run([
      { ended: true, remainingMs: 0 },
      { key: "focus:0:1800000", totalMs: 30 * MIN, remainingMs: 5 * MIN },
      { key: "focus:0:1800000", totalMs: 30 * MIN, ended: true, remainingMs: 0 },
    ])).toEqual([["focus_end"], [], ["focus_end"]]);
  });

  it("does not play a late end after the computer was asleep, and skips old reminders", () => {
    const woke = { ended: true, remainingMs: 0, overflowMs: 3 * 60 * MIN };
    expect(run([woke, { ...woke, overflowMs: woke.overflowMs + 1_000 }])).toEqual([[], []]);
  });

  it("after a late wake the next reminder still comes on schedule once the limit allows", () => {
    // 12 reminders have long passed (limit reached), so nothing more is ever played.
    const longAgo = { ended: true, remainingMs: 0, overflowMs: 24 * 60 * MIN };
    expect(run([longAgo, { ...longAgo, overflowMs: longAgo.overflowMs + 10 * MIN }])).toEqual([[], []]);
    // 12 minutes late with 5-minute reminders: two have passed; the third is the next to sound.
    const late = (extra: number) => ({ ended: true, remainingMs: 0, overflowMs: 12 * MIN + extra });
    expect(run([late(0), late(2 * MIN), late(4 * MIN)])).toEqual([[], [], ["focus_reminder"]]);
  });

  it("plays one reminder, not several, if time jumped over more than one interval", () => {
    expect(run([
      { ended: true, remainingMs: 0, overflowMs: 0 },
      { ended: true, remainingMs: 0, overflowMs: 21 * MIN },
    ])).toEqual([["focus_end"], ["focus_reminder"]]);
  });
});
