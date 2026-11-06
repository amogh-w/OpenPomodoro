import { describe, expect, it } from "vitest";
import { defaultRules } from "./machine";
import { phaseClass, phaseOf, shouldSendTimerEnds, viewOf } from "./timerView";
import type { TimerData } from "./types";

const MIN = 60_000;

function data(over: Partial<TimerData> = {}): TimerData {
  return {
    state: "idle",
    kind: null,
    start: null,
    total: null,
    pause_buffer: null,
    pause_ts: null,
    planned_ms: null,
    intention: null,
    category: null,
    task: null,
    focus_started_at: null,
    focus_ended_at: null,
    focus_elapsed_ms: null,
    rounds: 0,
    ...over,
  };
}

const session = (over: Partial<TimerData> = {}): TimerData =>
  data({ state: "session", kind: "focus", start: 0, total: 25 * MIN, pause_buffer: 0, ...over });

describe("viewOf", () => {
  it("is idle with nothing to count", () => {
    const v = viewOf(data(), 5 * MIN, defaultRules);
    expect(v).toMatchObject({ phase: "idle", paused: false, overflow: false, ended: false, remainingMs: 0 });
    expect(v.can.init_focus).toBe(true);
    expect(v.can.toggle_pause).toBe(false);
    expect(v.can.stop).toBe(false);
  });

  it("counts down a running session", () => {
    const v = viewOf(session(), 10 * MIN, defaultRules);
    expect(v).toMatchObject({ phase: "focus", elapsedMs: 10 * MIN, remainingMs: 15 * MIN, progress: 0.4, ended: false });
    expect(v.can).toMatchObject({ toggle_pause: true, extend: true, stop: true, abandon: true, init_focus: false });
  });

  it("notices a phase that is over before the server has moved on", () => {
    const v = viewOf(session(), 25 * MIN, defaultRules);
    expect(v.ended).toBe(true);
    expect(v.overflow).toBe(false);
    expect(shouldSendTimerEnds(v)).toBe(true);
    expect(shouldSendTimerEnds(viewOf(session(), 25 * MIN - 1, defaultRules))).toBe(false);
  });

  it("counts past the end in overflow", () => {
    const v = viewOf(session({ state: "session_end" }), 28 * MIN, defaultRules);
    expect(v).toMatchObject({ phase: "focus", overflow: true, overflowMs: 3 * MIN, remainingMs: 0, progress: 1 });
    expect(shouldSendTimerEnds(v), "already in overflow, nothing to send").toBe(false);
  });

  it("freezes while paused and never asks to end", () => {
    const v = viewOf(session({ state: "paused_session", pause_ts: 10 * MIN }), 60 * MIN, defaultRules);
    expect(v).toMatchObject({ paused: true, elapsedMs: 10 * MIN, ended: false });
    expect(shouldSendTimerEnds(v)).toBe(false);
    expect(v.can.toggle_pause, "resume is allowed").toBe(true);
  });

  it("applies strict mode to the offered buttons", () => {
    const strict = { ...defaultRules, strict: true };
    const v = viewOf(session(), 5 * MIN, strict);
    expect(v.can.toggle_pause).toBe(false);
    expect(v.can.abandon).toBe(false);
    expect(v.can.stop).toBe(true);
    const paused = viewOf(session({ state: "paused_session", pause_ts: 5 * MIN }), 9 * MIN, strict);
    expect(paused.can.toggle_pause, "a paused session can always resume").toBe(true);
  });

  it("offers the right choices in reflect and in a break", () => {
    const reflect = viewOf(data({ state: "reflect" }), 0, defaultRules);
    expect(reflect.can).toMatchObject({ take_rest: true, done: true, init_focus: false });
    const rest = viewOf(data({ state: "rest", kind: "short_break", start: 0, total: 5 * MIN, pause_buffer: 0 }), MIN, defaultRules);
    expect(rest).toMatchObject({ phase: "break", remainingMs: 4 * MIN });
    expect(rest.can).toMatchObject({ skip_rest: true, stop: true, extend: true });
  });

  it("treats an unknown stored state as idle", () => {
    expect(viewOf(data({ state: "from_the_future" }), 0, defaultRules).state).toBe("idle");
  });

  it("copes with a timer shrunk to nothing", () => {
    const v = viewOf(session({ total: 0 }), 5 * MIN, defaultRules);
    expect(v.progress).toBe(1);
    expect(v.ended).toBe(true);
  });
});

describe("phases and colours", () => {
  it("groups states into phases", () => {
    expect(phaseOf("paused_session_end")).toBe("focus");
    expect(phaseOf("paused_rest")).toBe("break");
    expect(phaseOf("rest_end")).toBe("break");
    expect(phaseOf("reflect")).toBe("reflect");
  });

  it("gives every state a tint, focus and overflow differently", () => {
    expect(phaseClass("idle")).toBe("");
    expect(phaseClass("session")).toBe("phase-focus");
    expect(phaseClass("paused_session")).toBe("phase-focus");
    expect(phaseClass("session_end")).toBe("phase-overflow");
    expect(phaseClass("rest")).toBe("phase-break");
    expect(phaseClass("rest_end")).toBe("phase-break-overflow");
    expect(new Set(["session", "session_end", "rest", "rest_end", "breathe", "reflect"].map((s) => phaseClass(s as never))).size).toBe(6);
  });
});
