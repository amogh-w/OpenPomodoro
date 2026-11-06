import { describe, expect, it } from "vitest";
import { emptyTickMemory, nextTick } from "./ticking";

const at = (seconds: number, running = true, key = "a") => ({ key, elapsedMs: seconds * 1000, running });

describe("nextTick", () => {
  it("does nothing without a phase", () => {
    expect(nextTick(null, { key: "a", second: 3 })).toEqual({ play: false, memory: emptyTickMemory });
  });

  it("learns the position on first sight, then ticks once per new second", () => {
    let m = nextTick(at(10.2), emptyTickMemory).memory;
    const plays: boolean[] = [];
    for (const s of [10.6, 11.0, 11.4, 12.01, 12.5]) {
      const r = nextTick(at(s), m);
      plays.push(r.play);
      m = r.memory;
    }
    expect(plays).toEqual([false, true, false, true, false]);
  });

  it("makes one tick after a long gap, not a burst", () => {
    const m = nextTick(at(5), emptyTickMemory).memory;
    const r = nextTick(at(65), m);
    expect(r.play).toBe(true);
    expect(nextTick(at(65.4), r.memory).play).toBe(false);
  });

  it("is silent while paused and resumes without catching up", () => {
    const m = nextTick(at(5), emptyTickMemory).memory;
    const paused = nextTick(at(20, false), m);
    expect(paused.play).toBe(false);
    expect(nextTick(at(20.3), paused.memory).play).toBe(false);
    expect(nextTick(at(21.1), paused.memory).play).toBe(true);
  });

  it("starts over for a new phase", () => {
    expect(nextTick(at(0, true, "b"), { key: "a", second: 99 })).toEqual({ play: false, memory: { key: "b", second: 0 } });
  });
});
