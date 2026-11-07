// TypeScript port of crates/core/src/planner.rs. The Rust code is the source of truth; both pass
// test-vectors/planner.json. Given the stored timer state, one action, the time it happened and the settings,
// it works out the next state and the sessions that finished. Pure: the server does this on stored state, and
// the browser does it to keep the timer going while offline, so a queued action lands on the same result.
import { type EventName, transition } from "./machine";
import { type Timer, EXTEND_STEP_MS, elapsed, endInstant, extend, isEnded, isPaused, newTimer, pause, resume } from "./timer";
import type { Settings, TimerData } from "./types";

export const MIN_PHASE_MS = 5_000;
export const MAX_PHASE_MS = 12 * 60 * 60_000;
const MAX_EXTEND_STEPS = 12;

/** What the user asked for. Arguments are only read by the actions they belong to. */
export interface Action {
  name: string;
  intention?: string | null;
  category?: string | null;
  /** The task (a `task` row id) a started focus session works on. */
  task?: string | null;
  duration_ms?: number | null;
  delta_ms?: number | null;
  kind?: string | null;
  /** Notes and mood for the focus session this action saves (`take_rest`, `done`, `timer_ends` into an auto break). */
  notes?: string | null;
  mood?: string | null;
}

/** A finished piece of work, as stored in a session row. */
export type RecordData =
  | {
      kind: "focus";
      started_at: number;
      ended_at: number;
      duration_ms: number;
      planned_ms: number;
      intention: string | null;
      category: string | null;
      task: string | null;
      notes: string | null;
      mood: string | null;
    }
  | {
      kind: "short_break" | "long_break";
      started_at: number;
      ended_at: number;
      duration_ms: number;
      planned_ms: number;
    };

export type PlanResult = { ok: true; next: TimerData; records: RecordData[] } | { ok: false; error: string };

/** Thrown inside the helpers, turned into `{ ok: false }` by `plan`. The text is `invalid:<code>` or `rejected:<code>`. */
class PlanFail extends Error {}
const invalid = (code: string): PlanFail => new PlanFail(`invalid:${code}`);
const rejected = (code: string): PlanFail => new PlanFail(`rejected:${code}`);

const EVENT_NAMES: readonly EventName[] = [
  "init_focus",
  "skip_breathe",
  "start_focus",
  "timer_ends",
  "toggle_pause",
  "extend",
  "stop",
  "abandon",
  "take_rest",
  "done",
  "skip_rest",
];

export function idleState(): TimerData {
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
    cycle_started_at: null,
  };
}

export function timerOf(data: TimerData): Timer | null {
  if (data.start === null || data.total === null) return null;
  return { start: data.start, total: data.total, pauseBuffer: data.pause_buffer ?? 0, pauseTs: data.pause_ts };
}

function setTimer(data: TimerData, timer: Timer | null): void {
  data.start = timer?.start ?? null;
  data.total = timer?.total ?? null;
  data.pause_buffer = timer?.pauseBuffer ?? null;
  data.pause_ts = timer?.pauseTs ?? null;
}

/** Back to idle, keeping only the round count (and when the cycle was last restarted). */
function cleared(prev: TimerData): TimerData {
  return { ...idleState(), rounds: prev.rounds, cycle_started_at: prev.cycle_started_at };
}

/** Trim; refuse over-long text or control characters (a newline is fine); empty means none. */
function clean(text: string | null | undefined, maxChars: number): string | null {
  if (text === null || text === undefined) return null;
  const t = text.trim();
  if ([...t].length > maxChars || /[\p{Cc}]/u.test(t.replace(/\n/g, ""))) throw invalid("bad_args");
  return t === "" ? null : t;
}

const timerOrFail = (data: TimerData): Timer => timerOf(data) ?? (() => {
  throw rejected("unknown");
})();

type BreakKind = "short_break" | "long_break";

function breakAfter(rounds: number, s: Settings): BreakKind {
  return rounds > 0 && rounds % s.rounds_before_long === 0 ? "long_break" : "short_break";
}

const breakLength = (kind: BreakKind, s: Settings): number => (kind === "long_break" ? s.long_break_ms : s.short_break_ms);

function beginSession(next: TimerData, at: number): void {
  next.kind = "focus";
  const planned = next.planned_ms;
  const made = planned === null ? null : newTimer(at, planned);
  setTimer(next, made?.ok ? made.value : null);
  next.focus_started_at = at;
}

/** Begin the breathing step, or the focus session itself, at time `at`. */
function beginFocus(next: TimerData, newState: string, at: number, s: Settings): void {
  next.kind = "focus";
  if (newState === "breathe") {
    const made = newTimer(at, s.breathe_ms);
    setTimer(next, made.ok ? made.value : null);
    next.focus_started_at = null;
  } else {
    beginSession(next, at);
  }
}

function beginBreak(next: TimerData, kind: BreakKind, at: number, s: Settings): void {
  next.kind = kind;
  const made = newTimer(at, breakLength(kind, s));
  setTimer(next, made.ok ? made.value : null);
  next.planned_ms = null;
  next.focus_started_at = null;
  next.focus_ended_at = null;
  next.focus_elapsed_ms = null;
}

function breakRecord(prev: TimerData, endedAt: number): RecordData {
  const timer = timerOrFail(prev);
  const kind = prev.kind;
  if (kind !== "short_break" && kind !== "long_break") throw rejected("unknown");
  return { kind, started_at: timer.start, ended_at: endedAt, duration_ms: elapsed(timer, endedAt), planned_ms: timer.total };
}

/** The focus work that just finished, from the live timer (a focus phase that ended without going through reflect). */
function focusRecordFromTimer(prev: TimerData, endedAt: number, extras: Extras): RecordData {
  const timer = timerOrFail(prev);
  return {
    kind: "focus",
    started_at: prev.focus_started_at ?? timer.start,
    ended_at: endedAt,
    duration_ms: elapsed(timer, endedAt),
    planned_ms: prev.planned_ms ?? timer.total,
    intention: prev.intention,
    category: prev.category,
    task: prev.task,
    notes: extras.notes,
    mood: extras.mood,
  };
}

/** The focus work held in `reflect`. */
function focusRecordFromReflect(prev: TimerData, extras: Extras): RecordData {
  const { focus_started_at: startedAt, focus_ended_at: endedAt, focus_elapsed_ms: spent } = prev;
  if (startedAt === null || endedAt === null || spent === null) throw rejected("unknown");
  return {
    kind: "focus",
    started_at: startedAt,
    ended_at: endedAt,
    duration_ms: spent,
    planned_ms: prev.planned_ms ?? spent,
    intention: prev.intention,
    category: prev.category,
    task: prev.task,
    notes: extras.notes,
    mood: extras.mood,
  };
}

interface Extras {
  notes: string | null;
  mood: string | null;
}

const MAX_NOTES_CHARS = 4000;
const MOODS: readonly string[] = ["focused", "neutral", "distracted"];

function extrasOf(action: Action): Extras {
  const mood = action.mood ?? null;
  if (mood !== null && !MOODS.includes(mood)) throw invalid("bad_args");
  return { notes: clean(action.notes, MAX_NOTES_CHARS), mood };
}

/**
 * Save a finished focus session. Ending a session saves it and it moves the cycle on, however long it ran; the only
 * way to keep nothing is to abandon it.
 */
function keepFocus(next: TimerData, records: RecordData[], record: RecordData): void {
  records.push(record);
  next.rounds += 1;
}

function keepBreak(next: TimerData, record: RecordData): void {
  if (record.kind === "long_break") next.rounds = 0;
}

function buildNext(
  prev: TimerData,
  name: EventName,
  out: { state: string; effect: "save_focus" | "save_break" | null },
  action: Action,
  at: number,
  s: Settings,
): { next: TimerData; records: RecordData[] } {
  let next: TimerData = { ...prev, state: out.state };
  const records: RecordData[] = [];

  const startFocus = (): void => {
    if (out.effect === "save_break") {
      const record = breakRecord(prev, at);
      keepBreak(next, record);
      records.push(record);
    }
    beginFocus(next, out.state, at, s);
  };

  switch (name) {
    case "init_focus":
    case "start_focus": {
      const asked = action.duration_ms ?? null;
      if (asked !== null && (asked < MIN_PHASE_MS || asked > MAX_PHASE_MS)) throw invalid("bad_args");
      next.planned_ms = asked ?? s.focus_ms;
      next.intention = clean(action.intention, 200);
      next.category = clean(action.category, 64);
      next.task = clean(action.task, 64);
      startFocus();
      break;
    }
    case "skip_breathe":
      beginSession(next, at);
      break;
    case "timer_ends": {
      const timer = timerOrFail(prev);
      if (!isEnded(timer, at) || isPaused(timer)) throw rejected("not_ended");
      // A phase that auto-starts the next one starts it where the last one ended, not when we noticed.
      const end = Math.min(endInstant(timer), at);
      if (prev.state === "breathe") {
        beginSession(next, end);
      } else if (prev.state === "session" && out.state === "rest") {
        keepFocus(next, records, focusRecordFromTimer(prev, end, extrasOf(action)));
        beginBreak(next, breakAfter(next.rounds, s), end, s);
      } else if (prev.state === "rest" && (out.state === "breathe" || out.state === "session")) {
        const record = breakRecord(prev, end);
        keepBreak(next, record);
        records.push(record);
        next.planned_ms = prev.planned_ms ?? s.focus_ms;
        beginFocus(next, out.state, end, s);
      }
      // otherwise the phase goes into overtime: the timer keeps counting
      break;
    }
    case "toggle_pause": {
      const timer = timerOrFail(prev);
      const result = isPaused(timer) ? resume(timer, at) : pause(timer, at);
      if (!result.ok) throw rejected("unknown");
      setTimer(next, result.value);
      break;
    }
    case "extend": {
      const delta = action.delta_ms ?? null;
      if (delta === null) throw invalid("bad_args");
      setTimer(next, extend(timerOrFail(prev), delta, at));
      break;
    }
    case "stop": {
      const timer = timerOrFail(prev);
      if (out.state === "reflect") {
        next.focus_ended_at = at;
        next.focus_elapsed_ms = elapsed(timer, at);
        next.kind = null;
        setTimer(next, null);
      } else {
        const record = breakRecord(prev, at);
        next = cleared(prev);
        keepBreak(next, record);
        records.push(record);
      }
      break;
    }
    case "abandon":
      next = cleared(prev);
      break;
    case "take_rest": {
      let explicit: BreakKind | null;
      if (action.kind === "short") explicit = "short_break";
      else if (action.kind === "long") explicit = "long_break";
      else if (action.kind === undefined || action.kind === null) explicit = null;
      else throw invalid("bad_args");
      const fromReflect = out.effect === "save_focus";
      if (fromReflect) keepFocus(next, records, focusRecordFromReflect(prev, extrasOf(action)));
      // The cycle picks the break after a kept focus round; an unscheduled break is a short one.
      beginBreak(next, explicit ?? (fromReflect ? breakAfter(next.rounds, s) : "short_break"), at, s);
      if (action.duration_ms !== undefined && action.duration_ms !== null) {
        if (action.duration_ms < MIN_PHASE_MS || action.duration_ms > MAX_PHASE_MS) throw invalid("bad_args");
        next.total = action.duration_ms;
      }
      break;
    }
    case "done": {
      next = cleared(prev);
      keepFocus(next, records, focusRecordFromReflect(prev, extrasOf(action)));
      break;
    }
    case "skip_rest": {
      const record = breakRecord(prev, at);
      next = cleared(prev);
      keepBreak(next, record);
      records.push(record);
      if (out.state === "breathe" || out.state === "session") {
        next.state = out.state;
        next.planned_ms = prev.planned_ms ?? s.focus_ms;
        next.intention = prev.intention;
        next.category = prev.category;
        next.task = prev.task;
        beginFocus(next, out.state, at, s);
      }
      break;
    }
  }
  next.state = out.state;
  return { next, records };
}

/**
 * Change what the focus session in progress is about (intention and category) without touching its clock. Allowed
 * while a focus runs (also in overtime or paused) and on the reflect screen; an empty field clears it.
 */
function editSession(prev: TimerData, action: Action): PlanResult {
  const runningFocus = prev.kind === "focus" && prev.start !== null;
  if (!runningFocus && prev.state !== "reflect") throw rejected("unknown");
  return {
    ok: true,
    next: { ...prev, intention: clean(action.intention, 200), category: clean(action.category, 64) },
    records: [],
  };
}

/** Decide what `action`, taken at time `at`, does to the timer. */
export function plan(prev: TimerData, action: Action, at: number, settings: Settings): PlanResult {
  try {
    if (action.name === "edit_session") return editSession(prev, action);
    if (action.name === "reset_cycle") return { ok: true, next: { ...prev, rounds: 0, cycle_started_at: at }, records: [] };
    const name = EVENT_NAMES.find((n) => n === action.name);
    if (name === undefined) throw invalid("unknown_command");

    let phaseEnded = false;
    if (name === "extend") {
      const delta = action.delta_ms ?? 0;
      const stepOk = delta !== 0 && delta % EXTEND_STEP_MS === 0 && Math.abs(delta) <= MAX_EXTEND_STEPS * EXTEND_STEP_MS;
      if (!stepOk) throw invalid("bad_args");
      const timer = timerOf(prev);
      phaseEnded = timer !== null && isEnded(extend(timer, delta, at), at);
    }

    const rules = {
      breathe: settings.breathe,
      autoStartBreak: settings.auto_start_break,
      autoStartFocus: settings.auto_start_focus,
      strict: settings.strict,
    };
    const out = transition(prev.state as never, { name, phaseEnded }, rules);
    if (!out.ok) throw rejected(out.rejected);
    return { ok: true, ...buildNext(prev, name, out, action, at, settings) };
  } catch (error) {
    if (error instanceof PlanFail) return { ok: false, error: error.message };
    throw error;
  }
}
