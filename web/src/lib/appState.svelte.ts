// The signed-in user's live data: the timer, settings and categories as the server last said them, the
// server's clock, and a WebSocket that says when to fetch again.
//
// It also keeps the app useful when the server cannot be reached. What the server last said is saved locally; an
// action taken while offline is worked out here with the same planner the server uses (so the timer keeps going
// and the screen answers at once), saved in the outbox, and sent when the server is back. Timer logic itself
// lives in planner.ts, timer.ts and machine.ts; this only keeps the state and talks to the server.
import { reconnectDelay } from "./backoff";
import { api, ApiError } from "./api";
import { createIdSource } from "./clientIds";
import { ServerClock } from "./clock";
import { t } from "./i18n";
import { kvDelete, kvGet, kvSet, outboxAdd, outboxClear, outboxList, outboxStore } from "./idb";
import { flush } from "./outbox";
import { plan } from "./planner";
import { type TimerView, rulesOf, viewOf } from "./timerView";
import { note } from "./diag.svelte";
import type { Category, CommandInput, EventInput, ServerState, Settings, Task, TimerData } from "./types";
import { leader } from "../platform/leader.svelte";

export const app = $state<{
  ready: boolean;
  online: boolean;
  timer: TimerData | null;
  settings: Settings | null;
  categories: Category[];
  tasks: Task[];
  /** Things done offline that have not reached the server yet. */
  pending: number;
  /** A message to show once, such as "a change made offline could not be applied". */
  notice: string | null;
  /** Counts the server's "something changed" pushes, so other parts of the app can refetch what they own. */
  changes: number;
}>({ ready: false, online: true, timer: null, settings: null, categories: [], tasks: [], pending: 0, notice: null, changes: 0 });

const clock = new ServerClock();

/** The server's time, refreshed several times a second while signed in, so screens can count live. */
export const tick = $state({ now: Date.now() });
let ticker: ReturnType<typeof setInterval> | undefined;
let tickMs = 250;

function runTicker(): void {
  clearInterval(ticker);
  tick.now = serverNow();
  ticker = setInterval(() => {
    tick.now = serverNow();
  }, tickMs);
}

/** Change how often the clock refreshes (slower saves power). Takes effect at once if the app is running. */
export function setTickRate(ms: number): void {
  tickMs = ms;
  if (running) runTicker();
}

/** The server's current time in Unix ms, as best this device can tell. */
export const serverNow = (): number => clock.now();

let onUnauthorized: () => void = () => {};
/** Called when the server says the session is gone (signed out elsewhere, disabled, expired). */
export function setUnauthorizedHandler(handler: () => void): void {
  onUnauthorized = handler;
}

// Ids for commands and events. The device part is per tab (session storage): two tabs acting in the same second
// must not make the same id, because the server would take the second for a repeat of the first.
let ids: ReturnType<typeof createIdSource> | null = null;
function nextId(): string {
  if (ids === null) {
    let store: Storage | null = null;
    try {
      store = sessionStorage;
    } catch {
      store = null;
    }
    ids = createIdSource(store ?? { getItem: () => null, setItem: () => {} });
  }
  return ids.next(serverNow());
}

// ---- what is remembered locally ----

type Cached = ServerState & { offset_ms: number };

let userId: string | null = null;
const stateKey = (id: string): string => `state:${id}`;

const channel = typeof BroadcastChannel === "undefined" ? null : new BroadcastChannel("op-sync");

function setFromState(state: ServerState): void {
  app.timer = state.timer;
  app.settings = state.settings;
  app.categories = state.categories;
  app.tasks = state.tasks ?? [];
  app.ready = true;
}

/** Remember the current state (the server's, or ours after an offline action) for the next load. */
function persistState(): void {
  if (!userId || !app.timer || !app.settings) return;
  // The reactive state is a proxy, which IndexedDB cannot copy; store plain snapshots.
  const cached: Cached = {
    server_time_ms: serverNow(),
    timer: $state.snapshot(app.timer),
    settings: $state.snapshot(app.settings),
    categories: $state.snapshot(app.categories),
    tasks: $state.snapshot(app.tasks),
    offset_ms: clock.offsetMs,
  };
  void kvSet(stateKey(userId), cached).catch((error: unknown) => console.warn("could not save state locally", error));
}

async function loadLocal(): Promise<void> {
  if (!userId) return;
  const cached = await kvGet<Cached>(stateKey(userId)).catch(() => undefined);
  if (cached) {
    clock.setOffset(cached.offset_ms);
    setFromState(cached);
  }
  app.pending = (await outboxList(userId).catch(() => [])).length;
}

/** Forget everything kept for this user on this device (sign-out, deleted account). */
export async function discardLocal(id: string): Promise<void> {
  await Promise.all([outboxClear(id), kvDelete(stateKey(id)), kvDelete("user")]).catch(() => {});
}

export const pendingCount = (): number => app.pending;

// ---- talking to the server ----

function applyServerState(state: ServerState, sentAt: number, receivedAt: number): void {
  const before = clock.offsetMs;
  clock.sync(state.server_time_ms, sentAt, receivedAt);
  const moved = clock.offsetMs - before;
  if (Math.abs(moved) > 5000) note("clock", `offset to the server moved by ${Math.round(moved / 1000)} s`);
  setFromState(state);
  app.online = true;
  persistState();
}

const unreachable = (error: ApiError): boolean => error.isNetwork || error.status >= 500;

/** Returns true when the error was handled here (signed out, or the server is unreachable). */
function handleFailure(error: unknown): boolean {
  if (!(error instanceof ApiError)) return false;
  if (error.status === 401) {
    note("api", "signed out by the server (401)");
    onUnauthorized();
    return true;
  }
  if (unreachable(error)) {
    if (app.online) note("api", `server unreachable (${error.status || "network"})`);
    app.online = false;
    return true;
  }
  return false;
}

let flushing = false;

/** Send what was queued offline. True when nothing is left waiting. Only the leader tab sends. */
export async function flushOutbox(): Promise<boolean> {
  if (!userId || app.pending === 0) return true;
  if (!leader.isLeader || flushing) return false;
  flushing = true;
  try {
    const result = await flush(userId, outboxStore, api);
    app.pending = (await outboxList(userId)).length;
    for (const d of result.dropped) {
      console.warn("the server refused something done offline:", d.code, d.item);
      note("sync", `server refused a queued item: ${d.code}`);
    }
    if (result.sent > 0) note("sync", `sent ${result.sent} queued item(s)`);
    if (result.dropped.length > 0) app.notice = t("common.dropped");
    if (result.unauthorized) onUnauthorized();
    else if (!result.complete) app.online = false;
    if (result.sent + result.dropped.length > 0) channel?.postMessage({ type: "synced", userId });
    return app.pending === 0;
  } finally {
    flushing = false;
  }
}

export async function refresh(): Promise<void> {
  if (userId && app.pending > 0) {
    if (leader.isLeader) {
      if (!(await flushOutbox())) return; // still waiting: keep showing what we worked out locally
    } else {
      app.pending = (await outboxList(userId).catch(() => [])).length;
      if (app.pending > 0) return; // the leader tab is sending it
    }
  }
  const sentAt = Date.now();
  try {
    applyServerState(await api.state(), sentAt, Date.now());
  } catch (error) {
    if (!handleFailure(error)) throw error;
  }
}

export type CommandOutcome =
  | { ok: true; status: "applied" | "duplicate" | "queued" }
  | { ok: false; code: string };

/** Work an action out here, show it at once, and keep it for sending when the server is back. */
async function queueCommand(id: string, input: CommandInput, at: number): Promise<CommandOutcome> {
  if (!userId || !app.timer || !app.settings) return { ok: false, code: "offline" };
  const planned = plan(app.timer, input, at, app.settings);
  if (!planned.ok) {
    note("timer", `${input.name} refused offline: ${planned.error}`);
    return { ok: false, code: planned.error.replace(":", "_") };
  }
  note("timer", `${input.name} queued (offline)`);

  app.timer = planned.next;
  await outboxAdd({ userId, kind: "command", id, input: { ...input, at }, createdAt: Date.now() });
  app.pending += 1;
  persistState();
  channel?.postMessage({ type: "local", userId });
  return { ok: true, status: "queued" };
}

/**
 * Send a timer command. The server decides; the answer carries the new state. When the server cannot be reached
 * the command is worked out locally and queued, with the same id, so if the server did get it the replay is
 * recognised as a repeat.
 */
export async function sendCommand(input: CommandInput): Promise<CommandOutcome> {
  const id = nextId();
  const at = serverNow();
  if (app.pending > 0 || !app.online) return queueCommand(id, input, at);

  const sentAt = Date.now();
  try {
    const res = await api.command(id, input);
    applyServerState(res.state, sentAt, Date.now());
    return { ok: true, status: res.status };
  } catch (error) {
    if (!(error instanceof ApiError)) throw error;
    if (unreachable(error)) {
      app.online = false;
      return queueCommand(id, input, at);
    }
    handleFailure(error);
    note("timer", `${input.name} refused: ${error.code}`);
    return { ok: false, code: error.code };
  }
}

// ---- live updates ----

let socket: WebSocket | null = null;
let attempt = 0;
let retry: ReturnType<typeof setTimeout> | undefined;
let watchdog: ReturnType<typeof setInterval> | undefined;
let running = false;

function connect(): void {
  if (!running) return;
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  const ws = new WebSocket(`${scheme}://${location.host}/api/ws`);
  socket = ws;
  ws.onopen = () => {
    note("live", attempt > 0 ? `reconnected after ${attempt} attempt(s)` : "connected");
    attempt = 0;
    // Anything that changed while we were not listening is picked up by this fetch.
    void refresh().catch(() => {});
  };
  ws.onmessage = (event: MessageEvent<unknown>) => {
    try {
      const message = JSON.parse(String(event.data)) as { type?: string };
      if (message.type === "changed") {
        app.changes += 1;
        void refresh().catch(() => {});
      }
    } catch {
      // Not ours; ignore.
    }
  };
  ws.onclose = () => {
    if (socket === ws) socket = null;
    if (running) note("live", "connection closed, retrying");
    if (running) retry = setTimeout(connect, reconnectDelay(attempt++));
  };
}

function onVisible(): void {
  if (document.visibilityState === "visible" && running) void refresh().catch(() => {});
}

function onOnline(): void {
  if (running) void refresh().catch(() => {});
}

function onChannel(event: MessageEvent<{ type?: string; userId?: string }>): void {
  if (!running || event.data.userId !== userId) return;
  // Another tab queued something or sent the queue: take its view of the world.
  if (event.data.type === "local") void loadLocal();
  else if (event.data.type === "synced") void loadLocal().then(() => refresh()).catch(() => {});
}

/** Start keeping state fresh for the signed-in user. `offline` means the server was already found unreachable. */
export function start(id: string, options: { offline?: boolean } = {}): void {
  if (running) return;
  running = true;
  userId = id;
  attempt = 0;
  document.addEventListener("visibilitychange", onVisible);
  addEventListener("online", onOnline);
  channel?.addEventListener("message", onChannel);
  runTicker();
  // While the server is unreachable or something is waiting to be sent, keep trying.
  watchdog = setInterval(() => {
    if (!app.online || app.pending > 0) void refresh().catch(() => {});
  }, 10_000);

  void (async () => {
    await loadLocal(); // show what we last knew straight away, even with no connection
    if (options.offline) app.online = false;
    await refresh().catch(() => {});
  })();
  connect();
}

/** Stop, and forget what is on screen (sign-out, session lost). What is saved on the device stays. */
export function stop(): void {
  running = false;
  userId = null;
  clearTimeout(retry);
  clearInterval(ticker);
  clearInterval(watchdog);
  document.removeEventListener("visibilitychange", onVisible);
  removeEventListener("online", onOnline);
  channel?.removeEventListener("message", onChannel);
  socket?.close();
  socket = null;
  app.ready = false;
  app.online = true;
  app.timer = null;
  app.settings = null;
  app.categories = [];
  app.tasks = [];
  app.pending = 0;
  app.notice = null;
}

export type SettingName = keyof Settings;

async function queueEvents(events: EventInput[]): Promise<boolean> {
  if (!userId) return false;
  await outboxAdd({ userId, kind: "events", events, createdAt: Date.now() });
  app.pending += 1;
  persistState();
  channel?.postMessage({ type: "local", userId });
  return true;
}

/**
 * Change what is on screen at once, then save `events`. `undo` puts the screen back if the server refuses them.
 * Offline, or with older changes still waiting, the events are queued instead and the change stays.
 */
async function commitEvents(events: EventInput[], change: () => void, undo: () => void): Promise<boolean> {
  change();
  if (app.pending > 0 || !app.online) return queueEvents(events);
  try {
    const { results } = await api.events(events);
    if (results.some((r) => r.result === "invalid")) {
      undo();
      return false;
    }
    persistState();
    return true;
  } catch (error) {
    if (error instanceof ApiError && unreachable(error)) {
      app.online = false;
      return queueEvents(events);
    }
    undo();
    if (handleFailure(error)) return false;
    throw error;
  }
}

/** Save one setting. The screen changes at once; the server confirms (or the old value comes back). */
export async function saveSetting(name: SettingName, value: number | boolean): Promise<boolean> {
  const previous = app.settings;
  const event: EventInput = { id: nextId(), entity: "setting", op: "add", key: name, payload: { value } };
  return commitEvents(
    [event],
    () => {
      if (previous) app.settings = { ...previous, [name]: value };
    },
    () => {
      app.settings = previous;
    },
  );
}

// ---- categories and sessions ----

export async function addCategory(name: string, color: string): Promise<boolean> {
  const id = nextId();
  const before = app.categories;
  return commitEvents(
    [{ id, entity: "category", op: "add", key: id, payload: { name, color, is_break: false } }],
    () => {
      app.categories = [...before, { id, name, color, is_break: false }];
    },
    () => {
      app.categories = before;
    },
  );
}

export async function updateCategory(
  id: string,
  change: { name?: string; color?: string; group?: string | null; focus_min?: number | null; break_min?: number | null },
): Promise<boolean> {
  const before = app.categories;
  return commitEvents(
    [{ id: nextId(), entity: "category", op: "edit", key: id, payload: change }],
    () => {
      app.categories = before.map((c) => (c.id === id ? { ...c, ...change } : c));
    },
    () => {
      app.categories = before;
    },
  );
}

export async function removeCategory(id: string): Promise<boolean> {
  const before = app.categories;
  return commitEvents(
    [{ id: nextId(), entity: "category", op: "delete", key: id }],
    () => {
      app.categories = before.filter((c) => c.id !== id);
    },
    () => {
      app.categories = before;
    },
  );
}

// ---- tasks ----

export async function addTask(title: string, category: string | null = null): Promise<boolean> {
  const id = nextId();
  const before = app.tasks;
  const row: Task = { id, title, done: false, done_at: null, category };
  return commitEvents(
    [{ id, entity: "task", op: "add", key: id, payload: { title, done: false, category } }],
    () => {
      app.tasks = [...before, row];
    },
    () => {
      app.tasks = before;
    },
  );
}

export async function updateTask(id: string, change: { title?: string; category?: string | null }): Promise<boolean> {
  const before = app.tasks;
  return commitEvents(
    [{ id: nextId(), entity: "task", op: "edit", key: id, payload: change }],
    () => {
      app.tasks = before.map((x) => (x.id === id ? { ...x, ...change } : x));
    },
    () => {
      app.tasks = before;
    },
  );
}

/** Tick a task off (or put it back). The time it was ticked off is kept, so the list can show recent wins. */
export async function setTaskDone(id: string, done: boolean): Promise<boolean> {
  const before = app.tasks;
  const done_at = done ? Date.now() : null;
  return commitEvents(
    [{ id: nextId(), entity: "task", op: "edit", key: id, payload: { done, done_at } }],
    () => {
      app.tasks = before.map((x) => (x.id === id ? { ...x, done, done_at } : x));
    },
    () => {
      app.tasks = before;
    },
  );
}

export async function removeTask(id: string): Promise<boolean> {
  const before = app.tasks;
  return commitEvents(
    [{ id: nextId(), entity: "task", op: "delete", key: id }],
    () => {
      app.tasks = before.filter((x) => x.id !== id);
    },
    () => {
      app.tasks = before;
    },
  );
}

/** A session added after the fact (the person forgot to start the timer), or a break. */
export interface NewSession {
  kind: "focus" | "short_break" | "long_break";
  started_at: number;
  ended_at: number;
  intention?: string;
  category?: string | null;
  notes?: string | null;
  mood?: "focused" | "neutral" | "distracted" | null;
}

export async function addSession(session: NewSession): Promise<boolean> {
  const id = nextId();
  const duration = session.ended_at - session.started_at;
  return commitEvents(
    [
      {
        id,
        entity: "session",
        op: "add",
        key: id,
        payload: { ...session, duration_ms: duration, planned_ms: duration },
      },
    ],
    () => {},
    () => {},
  );
}

/** Change fields of a saved session (notes, mood, intention, category). */
export async function editSession(id: string, change: Record<string, unknown>): Promise<boolean> {
  return commitEvents([{ id: nextId(), entity: "session", op: "edit", key: id, payload: change }], () => {}, () => {});
}

export async function deleteSession(id: string): Promise<boolean> {
  return commitEvents([{ id: nextId(), entity: "session", op: "delete", key: id }], () => {}, () => {});
}

/** The timer view for the signed-in user right now, or null before the first answer from the server. */
export function currentView(): TimerView | null {
  return app.timer && app.settings ? viewOf(app.timer, tick.now, rulesOf(app.settings)) : null;
}
