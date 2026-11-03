// TypeScript port of crates/core/src/machine.rs. The Rust code is the source of truth; both pass
// test-vectors/machine.json. Pure: it returns the next state and what must be saved.

export type PausedFrom = "session" | "session_end" | "rest" | "rest_end";

export type State =
  | "idle"
  | "breathe"
  | "session"
  | "session_end"
  | `paused_${PausedFrom}`
  | "reflect"
  | "rest"
  | "rest_end";

export type EventName =
  | "init_focus"
  | "skip_breathe"
  | "start_focus"
  | "timer_ends"
  | "toggle_pause"
  | "extend"
  | "stop"
  | "abandon"
  | "take_rest"
  | "done"
  | "skip_rest";

/** `phaseEnded` is only read for `extend`: the timer's answer after the change was applied. */
export interface MachineEvent {
  name: EventName;
  phaseEnded?: boolean;
}

export interface Rules {
  breathe: boolean;
  autoStartBreak: boolean;
  autoStartFocus: boolean;
  strict: boolean;
}

export const defaultRules: Rules = { breathe: false, autoStartBreak: false, autoStartFocus: false, strict: false };

export type Effect = "save_focus" | "save_break";
export type Rejected = "unknown" | "strict";

export type Outcome = { ok: true; state: State; effect: Effect | null } | { ok: false; rejected: Rejected };

const ok = (state: State, effect: Effect | null = null): Outcome => ({ ok: true, state, effect });
const no = (rejected: Rejected): Outcome => ({ ok: false, rejected });

const isPaused = (s: State): s is `paused_${PausedFrom}` => s.startsWith("paused_");
const resumeState = (s: `paused_${PausedFrom}`): State => s.slice("paused_".length) as State;

const FOCUS = new Set<State>(["session", "session_end", "paused_session", "paused_session_end"]);
const BREAK = new Set<State>(["rest", "rest_end", "paused_rest", "paused_rest_end"]);

/** First focus state: the breathing step when it is on, otherwise straight into the session. */
const focusStart = (r: Rules): State => (r.breathe ? "breathe" : "session");

export function transition(state: State, event: MachineEvent, r: Rules): Outcome {
  switch (event.name) {
    case "init_focus":
      if (state === "idle") return ok(focusStart(r));
      if (state === "rest_end") return ok(focusStart(r), "save_break");
      break;
    case "start_focus":
      if (state === "idle") return ok("session");
      if (state === "rest_end") return ok("session", "save_break");
      break;
    case "skip_breathe":
      if (state === "breathe") return ok("session");
      break;
    case "timer_ends":
      if (state === "breathe") return ok("session");
      if (state === "session") return r.autoStartBreak ? ok("rest", "save_focus") : ok("session_end");
      if (state === "rest") return r.autoStartFocus ? ok(focusStart(r), "save_break") : ok("rest_end");
      break;
    case "toggle_pause":
      if (state === "session" || state === "session_end" || state === "rest" || state === "rest_end") {
        return r.strict ? no("strict") : ok(`paused_${state}`);
      }
      if (isPaused(state)) return ok(resumeState(state));
      break;
    case "extend": {
      const ended = event.phaseEnded === true;
      if (state === "session" || state === "rest" || state === "paused_session" || state === "paused_rest") {
        return ok(state);
      }
      // An overflow phase runs again only when the timer does: session_end -> session, rest_end -> rest.
      if (state === "session_end" || state === "rest_end") return ok(ended ? state : (state.slice(0, -4) as State));
      if (state === "paused_session_end" || state === "paused_rest_end") {
        return ok(ended ? state : (state.slice(0, -4) as State));
      }
      break;
    }
    case "stop":
      if (FOCUS.has(state)) return ok("reflect");
      if (BREAK.has(state)) {
        if (r.strict && (state === "rest" || state === "paused_rest")) return no("strict");
        return ok("idle", "save_break");
      }
      break;
    case "abandon":
      if (state === "breathe") return ok("idle");
      if (FOCUS.has(state)) return r.strict ? no("strict") : ok("idle");
      break;
    case "take_rest":
      if (state === "reflect") return ok("rest", "save_focus");
      if (state === "idle") return ok("rest");
      break;
    case "done":
      if (state === "reflect") return ok("idle", "save_focus");
      break;
    case "skip_rest":
      if (BREAK.has(state)) {
        if (r.strict && (state === "rest" || state === "paused_rest")) return no("strict");
        return ok(r.autoStartFocus ? focusStart(r) : "idle", "save_break");
      }
      break;
  }
  return no("unknown");
}
