import { describe, expect, it } from "vitest";
import { emptyNudgeMemory, nextNudge } from "./nudge";

const MIN = 60_000;
const at = (elapsedMin: number, running = true) => ({ key: "s1", elapsedMs: elapsedMin * MIN, running });

describe("nextNudge", () => {
  it("does nothing when off or when there is no session", () => {
    expect(nextNudge(at(30), emptyNudgeMemory, 0).play).toBe(false);
    expect(nextNudge(null, { key: "s1", count: 2 }, 5 * MIN)).toEqual({ play: false, memory: emptyNudgeMemory });
  });

  it("learns where things stand on the first look, even mid-session", () => {
    expect(nextNudge(at(12), emptyNudgeMemory, 5 * MIN)).toEqual({ play: false, memory: { key: "s1", count: 2 } });
  });

  it("plays once per whole interval", () => {
    let m = nextNudge(at(0), emptyNudgeMemory, 5 * MIN).memory;
    const plays: boolean[] = [];
    for (const min of [1, 4.9, 5, 5.1, 9, 10, 10.5]) {
      const r = nextNudge(at(min), m, 5 * MIN);
      plays.push(r.play);
      m = r.memory;
    }
    expect(plays).toEqual([false, false, true, false, false, true, false]);
  });

  it("plays one, not a burst, after a long sleep", () => {
    const m = nextNudge(at(0), emptyNudgeMemory, 5 * MIN).memory;
    const r = nextNudge(at(23), m, 5 * MIN);
    expect(r.play).toBe(true);
    expect(nextNudge(at(23.5), r.memory, 5 * MIN).play).toBe(false);
  });

  it("stays quiet while paused and does not replay the interval on resume", () => {
    const m = nextNudge(at(0), emptyNudgeMemory, 5 * MIN).memory;
    const paused = nextNudge(at(6, false), m, 5 * MIN);
    expect(paused.play).toBe(false);
    expect(nextNudge(at(6.5), paused.memory, 5 * MIN).play).toBe(false);
  });

  it("starts over for a new session", () => {
    const r = nextNudge({ key: "s2", elapsedMs: 0, running: true }, { key: "s1", count: 4 }, 5 * MIN);
    expect(r).toEqual({ play: false, memory: { key: "s2", count: 0 } });
  });
});
