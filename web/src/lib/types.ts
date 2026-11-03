// Shapes of the server's JSON (see crates/server/src/api).

export interface User {
  id: string;
  username: string;
  role: "admin" | "user";
  disabled: boolean;
  created_at: number;
}

export interface SetupStatus {
  needs_setup: boolean;
  registration: "open" | "closed";
}

/** The user's stored timer, exactly as the server keeps it. Times are Unix milliseconds. */
export interface TimerData {
  state: string;
  kind: "focus" | "short_break" | "long_break" | null;
  start: number | null;
  total: number | null;
  pause_buffer: number | null;
  pause_ts: number | null;
  planned_ms: number | null;
  intention: string | null;
  category: string | null;
  focus_started_at: number | null;
  focus_ended_at: number | null;
  focus_elapsed_ms: number | null;
  rounds: number;
}

export interface Settings {
  focus_ms: number;
  short_break_ms: number;
  long_break_ms: number;
  rounds_before_long: number;
  breathe: boolean;
  breathe_ms: number;
  auto_start_break: boolean;
  auto_start_focus: boolean;
  strict: boolean;
}

export interface Category {
  id: string;
  name: string;
  color?: string;
  is_break?: boolean;
}

export interface ServerState {
  server_time_ms: number;
  timer: TimerData;
  settings: Settings;
  categories: Category[];
}

export interface CommandResponse {
  status: "applied" | "duplicate";
  state: ServerState;
}

export interface SessionRecord {
  id: string;
  kind: "focus" | "short_break" | "long_break";
  started_at: number;
  ended_at: number;
  duration_ms?: number;
  planned_ms?: number;
  intention?: string | null;
  category?: string | null;
  notes?: string | null;
  mood?: "focused" | "neutral" | "distracted" | null;
}

/** One local day of recorded time, from `/api/summary`. */
export interface DayTotal {
  /** `YYYY-MM-DD`. */
  day: string;
  focus_ms: number;
  break_ms: number;
  focus_count: number;
}

export interface CommandInput {
  name: string;
  at?: number;
  intention?: string;
  category?: string;
  duration_ms?: number;
  delta_ms?: number;
  kind?: "short" | "long";
  notes?: string;
  mood?: "focused" | "neutral" | "distracted";
}

export interface EventInput {
  id: string;
  entity: "session" | "category" | "setting";
  op: "add" | "edit" | "delete";
  key: string;
  payload?: Record<string, unknown> | null;
}

export interface EventResult {
  id: string;
  result: string;
  error?: string;
}
