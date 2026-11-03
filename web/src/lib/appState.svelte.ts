// The signed-in user's live data: the timer, settings and categories as the server last said them, the
// server's clock, and a WebSocket that says when to fetch again. Timer logic itself lives in timer.ts and
// machine.ts; this only keeps the state and talks to the server.
import { reconnectDelay } from "./backoff";
import { api, ApiError } from "./api";
import { createIdSource } from "./clientIds";
import { ServerClock } from "./clock";
import { type TimerView, rulesOf, viewOf } from "./timerView";
import type { Category, CommandInput, ServerState, Settings, TimerData } from "./types";

export const app = $state<{
  ready: boolean;
  online: boolean;
  timer: TimerData | null;
  settings: Settings | null;
  categories: Category[];
}>({ ready: false, online: true, timer: null, settings: null, categories: [] });

const clock = new ServerClock();

/** The server's time, refreshed several times a second while signed in, so screens can count live. */
export const tick = $state({ now: Date.now() });
let ticker: ReturnType<typeof setInterval> | undefined;

/** The server's current time in Unix ms, as best this device can tell. */
export const serverNow = (): number => clock.now();

let onUnauthorized: () => void = () => {};
/** Called when the server says the session is gone (signed out elsewhere, disabled, expired). */
export function setUnauthorizedHandler(handler: () => void): void {
  onUnauthorized = handler;
}

let ids: ReturnType<typeof createIdSource> | null = null;
function nextId(): string {
  if (ids === null) {
    let store: Storage | null = null;
    try {
      store = localStorage;
    } catch {
      store = null;
    }
    ids = createIdSource(store ?? { getItem: () => null, setItem: () => {} });
  }
  return ids.next(serverNow());
}

function apply(state: ServerState, sentAt: number, receivedAt: number): void {
  clock.sync(state.server_time_ms, sentAt, receivedAt);
  app.timer = state.timer;
  app.settings = state.settings;
  app.categories = state.categories;
  app.ready = true;
  app.online = true;
}

/** Returns true when the error was handled here (signed out, or the server is unreachable). */
function handleFailure(error: unknown): boolean {
  if (!(error instanceof ApiError)) return false;
  if (error.status === 401) {
    onUnauthorized();
    return true;
  }
  if (error.isNetwork || error.status >= 500) {
    app.online = false;
    return true;
  }
  return false;
}

export async function refresh(): Promise<void> {
  const sentAt = Date.now();
  try {
    apply(await api.state(), sentAt, Date.now());
  } catch (error) {
    if (!handleFailure(error)) throw error;
  }
}

export type CommandOutcome = { ok: true; status: "applied" | "duplicate" } | { ok: false; code: string };

/** Send a timer command. The server decides; the answer carries the new state. */
export async function sendCommand(input: CommandInput): Promise<CommandOutcome> {
  const sentAt = Date.now();
  try {
    const res = await api.command(nextId(), input);
    apply(res.state, sentAt, Date.now());
    return { ok: true, status: res.status };
  } catch (error) {
    if (error instanceof ApiError) {
      handleFailure(error);
      return { ok: false, code: error.code };
    }
    throw error;
  }
}

// ---- live updates ----

let socket: WebSocket | null = null;
let attempt = 0;
let retry: ReturnType<typeof setTimeout> | undefined;
let running = false;

function connect(): void {
  if (!running) return;
  const scheme = location.protocol === "https:" ? "wss" : "ws";
  const ws = new WebSocket(`${scheme}://${location.host}/api/ws`);
  socket = ws;
  ws.onopen = () => {
    attempt = 0;
    // Anything that changed while we were not listening is picked up by this fetch.
    void refresh().catch(() => {});
  };
  ws.onmessage = (event: MessageEvent<unknown>) => {
    try {
      const message = JSON.parse(String(event.data)) as { type?: string };
      if (message.type === "changed") void refresh().catch(() => {});
    } catch {
      // Not ours; ignore.
    }
  };
  ws.onclose = () => {
    if (socket === ws) socket = null;
    if (running) retry = setTimeout(connect, reconnectDelay(attempt++));
  };
}

function onVisible(): void {
  if (document.visibilityState === "visible" && running) void refresh().catch(() => {});
}

/** Start keeping state fresh for the signed-in user. */
export function start(): void {
  if (running) return;
  running = true;
  attempt = 0;
  document.addEventListener("visibilitychange", onVisible);
  tick.now = serverNow();
  ticker = setInterval(() => {
    tick.now = serverNow();
  }, 250);
  void refresh().catch(() => {});
  connect();
}

/** Stop, and forget everything about the user (sign-out, session lost). */
export function stop(): void {
  running = false;
  clearTimeout(retry);
  clearInterval(ticker);
  document.removeEventListener("visibilitychange", onVisible);
  socket?.close();
  socket = null;
  app.ready = false;
  app.online = true;
  app.timer = null;
  app.settings = null;
  app.categories = [];
}

export type SettingName = keyof Settings;

/** Save one setting. The screen changes at once; the server confirms (or the old value comes back). */
export async function saveSetting(name: SettingName, value: number | boolean): Promise<boolean> {
  const previous = app.settings;
  if (previous) app.settings = { ...previous, [name]: value };
  try {
    const { results } = await api.events([
      { id: nextId(), entity: "setting", op: "add", key: name, payload: { value } },
    ]);
    const accepted = results[0]?.result !== "invalid";
    if (!accepted) app.settings = previous;
    return accepted;
  } catch (error) {
    app.settings = previous;
    if (handleFailure(error)) return false;
    throw error;
  }
}

/** The timer view for the signed-in user right now, or null before the first answer from the server. */
export function currentView(): TimerView | null {
  return app.timer && app.settings ? viewOf(app.timer, tick.now, rulesOf(app.settings)) : null;
}
