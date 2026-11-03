import { describe, expect, it } from "vitest";
import vectors from "../../../test-vectors/timer.json";
import {
  type Result,
  type Timer,
  type TimerError,
  elapsed,
  extend,
  isEnded,
  isPaused,
  newTimer,
  overflow,
  pause,
  remaining,
  resume,
} from "./timer";

type Op =
  | { op: "start"; at: number; total: number; error?: TimerError }
  | { op: "pause"; at: number; error?: TimerError }
  | { op: "resume"; at: number; error?: TimerError }
  | { op: "extend"; at: number; delta: number }
  | { op: "expect"; at: number; elapsed: number; remaining: number; overflow: number; ended: boolean; paused: boolean };

interface Suite {
  cases: { name: string; ops: Op[] }[];
}

const suite = vectors as unknown as Suite;

/** Apply a result to the current timer, checking the expected error code (if any). */
function apply(current: Timer | null, result: Result<Timer>, error: TimerError | undefined, where: string): Timer | null {
  if (error === undefined) {
    if (!result.ok) throw new Error(`${where}: unexpected error ${result.error}`);
    return result.value;
  }
  expect(result.ok ? "ok" : result.error, where).toBe(error);
  return current; // a rejected op leaves the timer unchanged
}

describe("shared timer vectors", () => {
  it("has cases", () => expect(suite.cases.length).toBeGreaterThan(0));

  for (const c of suite.cases) {
    it(c.name, () => {
      let timer: Timer | null = null;
      c.ops.forEach((op, i) => {
        const where = `${c.name} op ${i}`;
        if (op.op === "start") {
          timer = apply(timer, newTimer(op.at, op.total), op.error, where);
          return;
        }
        if (timer === null) throw new Error(`${where}: timer not started`);
        if (op.op === "pause") timer = apply(timer, pause(timer, op.at), op.error, where);
        else if (op.op === "resume") timer = apply(timer, resume(timer, op.at), op.error, where);
        else if (op.op === "extend") timer = extend(timer, op.delta, op.at);
        else {
          expect(elapsed(timer, op.at), `${where} elapsed`).toBe(op.elapsed);
          expect(remaining(timer, op.at), `${where} remaining`).toBe(op.remaining);
          expect(overflow(timer, op.at), `${where} overflow`).toBe(op.overflow);
          expect(isEnded(timer, op.at), `${where} ended`).toBe(op.ended);
          expect(isPaused(timer), `${where} paused`).toBe(op.paused);
        }
      });
    });
  }
});
