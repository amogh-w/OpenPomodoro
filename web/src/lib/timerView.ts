// What the timer screen shows and which buttons it offers, worked out from the stored timer state, the time
// and the user's rules. The state machine is asked, not copied, so the screen can never offer something the
// server would refuse.
import { type EventName, type Rules, type State, transition } from "./machine";
import { type Timer, elapsed, isEnded, overflow, remaining } from "./timer";
import type { Settings, TimerData } from "./types";

export type Phase = "idle" | "breathe" | "focus" | "break" | "reflect";

const STATES: readonly State[] = [
  "idle",
  "breathe",
  "session",
  "session_end",
  "paused_session",
  "paused_session_end",
  "paused_rest",
  "paused_rest_end",
  "reflect",
  "rest",
  "rest_end",
];

const COMMANDS = [
  "init_focus",
  "skip_breathe",
  "toggle_pause",
  "extend",
  "stop",
  "abandon",
  "take_rest",
  "done",
  "skip_rest",
] as const satisfies readonly EventName[];

export type Command = (typeof COMMANDS)[number];

export interface TimerView {
  state: State;
  phase: Phase;
  paused: boolean;
  /** The planned end has passed and the phase is counting past it. */
  overflow: boolean;
  /** The timer says the phase is over but the server has not moved on yet (the app should say so). */
  ended: boolean;
  elapsedMs: number;
  remainingMs: number;
  overflowMs: number;
  totalMs: number;
  /** Fraction of the planned time used, 0 to 1. */
  progress: number;
  kind: TimerData["kind"];
  can: Record<Command, boolean>;
}

export function rulesOf(settings: Settings): Rules {
  return {
    breathe: settings.breathe,
    autoStartBreak: settings.auto_start_break,
    autoStartFocus: settings.auto_start_focus,
    strict: settings.strict,
  };
}

export function parseState(code: string): State {
  return (STATES as readonly string[]).includes(code) ? (code as State) : "idle";
}

export function phaseOf(state: State): Phase {
  switch (state) {
    case "idle":
      return "idle";
    case "breathe":
      return "breathe";
    case "session":
    case "session_end":
    case "paused_session":
    case "paused_session_end":
      return "focus";
    case "reflect":
      return "reflect";
    default:
      return "break";
  }
}

export function timerOf(data: TimerData): Timer | null {
  if (data.start === null || data.total === null) return null;
  return { start: data.start, total: data.total, pauseBuffer: data.pause_buffer ?? 0, pauseTs: data.pause_ts };
}

const isOverflowState = (s: State): boolean =>
  s === "session_end" || s === "rest_end" || s === "paused_session_end" || s === "paused_rest_end";

export function viewOf(data: TimerData, now: number, rules: Rules): TimerView {
  const state = parseState(data.state);
  const timer = timerOf(data);
  const paused = state.startsWith("paused_");

  const can = Object.fromEntries(
    COMMANDS.map((name) => [name, transition(state, { name, phaseEnded: false }, rules).ok]),
  ) as Record<Command, boolean>;

  if (timer === null) {
    return {
      state,
      phase: phaseOf(state),
      paused,
      overflow: false,
      ended: false,
      elapsedMs: 0,
      remainingMs: 0,
      overflowMs: 0,
      totalMs: 0,
      progress: 0,
      kind: data.kind,
      can,
    };
  }

  const ended = isEnded(timer, now) && !paused;
  return {
    state,
    phase: phaseOf(state),
    paused,
    overflow: isOverflowState(state) || (ended && overflow(timer, now) > 0),
    ended,
    elapsedMs: elapsed(timer, now),
    remainingMs: remaining(timer, now),
    overflowMs: overflow(timer, now),
    totalMs: timer.total,
    progress: timer.total > 0 ? Math.min(1, elapsed(timer, now) / timer.total) : 1,
    kind: data.kind,
    can,
  };
}

/** Should the app tell the server the phase is over? Only a running phase whose time is up and that is not already in overflow. */
export function shouldSendTimerEnds(view: TimerView): boolean {
  return view.ended && !view.paused && (view.state === "breathe" || view.state === "session" || view.state === "rest");
}

/** CSS class for the whole-screen tint of the current state. */
export function phaseClass(state: State): string {
  switch (state) {
    case "breathe":
      return "phase-breathe";
    case "session":
    case "paused_session":
      return "phase-focus";
    case "session_end":
    case "paused_session_end":
      return "phase-overflow";
    case "rest":
    case "paused_rest":
      return "phase-break";
    case "rest_end":
    case "paused_rest_end":
      return "phase-break-overflow";
    case "reflect":
      return "phase-reflect";
    default:
      return "";
  }
}
