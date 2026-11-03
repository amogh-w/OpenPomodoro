import { describe, expect, it } from "vitest";
import { dayStartMs, groupByDay, localDayKey, monthBounds, timelineBlocks } from "./days";
import type { SessionRecord } from "./types";

const MIN = 60_000;
const at = (y: number, mo: number, d: number, h = 0, mi = 0): number => new Date(y, mo - 1, d, h, mi).getTime();

let n = 0;
function s(over: Partial<SessionRecord> & { started_at: number }): SessionRecord {
  n += 1;
  const dur = over.duration_ms ?? 25 * MIN;
  return { id: `s${n}`, kind: "focus", ended_at: over.started_at + dur, duration_ms: dur, ...over };
}

describe("local days", () => {
  it("names the local day and finds its start", () => {
    expect(localDayKey(at(2026, 1, 5, 23, 59))).toBe("2026-01-05");
    expect(localDayKey(at(2026, 1, 6, 0, 0))).toBe("2026-01-06");
    expect(dayStartMs("2026-11-03")).toBe(at(2026, 11, 3));
  });

  it("bounds a month from its first midnight to the next month's", () => {
    expect(monthBounds(2026, 10)).toEqual({ from: at(2026, 11, 1), to: at(2026, 12, 1) });
    expect(monthBounds(2026, 11)).toEqual({ from: at(2026, 12, 1), to: at(2027, 1, 1) });
  });
});

describe("groupByDay", () => {
  it("groups by local day, newest first, sessions in the order they happened", () => {
    const groups = groupByDay([
      s({ started_at: at(2026, 11, 3, 14) }),
      s({ started_at: at(2026, 11, 4, 9) }),
      s({ started_at: at(2026, 11, 3, 9) }),
    ]);
    expect(groups.map((g) => g.day)).toEqual(["2026-11-04", "2026-11-03"]);
    expect(groups[1]?.sessions.map((x) => new Date(x.started_at).getHours())).toEqual([9, 14]);
  });

  it("totals focus and break time separately", () => {
    const [g] = groupByDay([
      s({ started_at: at(2026, 11, 3, 9), duration_ms: 25 * MIN }),
      s({ started_at: at(2026, 11, 3, 10), duration_ms: 10 * MIN }),
      s({ started_at: at(2026, 11, 3, 11), kind: "short_break", duration_ms: 5 * MIN }),
    ]);
    expect(g).toMatchObject({ focusMs: 35 * MIN, breakMs: 5 * MIN, focusCount: 2 });
  });

  it("falls back to the time between start and end when no duration was stored", () => {
    const record: SessionRecord = { id: "x", kind: "focus", started_at: at(2026, 11, 3, 9), ended_at: at(2026, 11, 3, 9, 20) };
    expect(groupByDay([record])[0]?.focusMs).toBe(20 * MIN);
  });

  it("is empty for no sessions", () => {
    expect(groupByDay([])).toEqual([]);
  });
});

describe("timelineBlocks", () => {
  const day = at(2026, 11, 3);

  it("places a session as a share of the 24 hours", () => {
    const [b] = timelineBlocks([s({ started_at: at(2026, 11, 3, 6), duration_ms: 6 * 60 * MIN })], day);
    expect(b?.left).toBeCloseTo(25);
    expect(b?.width).toBeCloseTo(25);
  });

  it("clips a session that crosses midnight to the day shown", () => {
    const late = s({ started_at: at(2026, 11, 3, 23), duration_ms: 2 * 60 * MIN });
    const [b] = timelineBlocks([late], day);
    expect(b?.left).toBeCloseTo((23 / 24) * 100);
    expect((b?.left ?? 0) + (b?.width ?? 0)).toBeCloseTo(100);
    const next = timelineBlocks([late], at(2026, 11, 4));
    expect(next[0]?.left).toBe(0);
    expect(next[0]?.width).toBeCloseTo((1 / 24) * 100);
  });

  it("leaves out a session from another day", () => {
    expect(timelineBlocks([s({ started_at: at(2026, 11, 5, 9) })], day)).toEqual([]);
  });

  it("keeps even a very short session visible", () => {
    const [b] = timelineBlocks([s({ started_at: at(2026, 11, 3, 9), duration_ms: 1000 })], day);
    expect(b?.width).toBeGreaterThanOrEqual(0.6);
  });

  it("carries the kind and category for colouring", () => {
    const [b] = timelineBlocks([s({ started_at: at(2026, 11, 3, 9), kind: "long_break", category: "c1" })], day);
    expect(b).toMatchObject({ kind: "long_break", category: "c1" });
  });
});
