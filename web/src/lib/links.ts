// Links that control the app: `#/start?intent=write&duration=30&category=Deep%20work`, `#/pause`, `#/resume`,
// `#/break`, `#/finish`, `#/analytics`, and the same with the `web+openpomodoro:` scheme. Anything that changes
// the timer is only ever carried out after the person says yes (see LinkHandler), because any page can open a link.
import { matchCategory } from "./intention";
import type { Route } from "./router";
import type { TimerView } from "./timerView";
import type { Category, CommandInput } from "./types";

/** Where to go afterwards: `x-success` when the action worked, `x-error` when it did not. Only on links that act. */
export interface Callbacks {
  success: string | null;
  error: string | null;
}

export type LinkAction =
  | { kind: "navigate"; route: Route }
  | { kind: "start"; intention: string | null; durationMs: number | null; category: string | null; notes: string | null; callbacks?: Callbacks }
  | { kind: "pause"; callbacks?: Callbacks }
  | { kind: "resume"; callbacks?: Callbacks }
  | { kind: "break"; callbacks?: Callbacks }
  | { kind: "finish"; callbacks?: Callbacks };

const SCHEME = /^web\+openpomodoro:(?:\/\/)?/i;
const MAX_INTENTION = 200;
const MAX_CATEGORY = 64;
const MAX_NOTES = 4000;

/** Trimmed text of at most `max` characters, or null when missing, blank or too long. */
function text(value: string | null, max: number): string | null {
  const t = value?.trim() ?? "";
  return t !== "" && [...t].length <= max ? t : null;
}

/** Minutes as whole minutes from 1 to 720, in milliseconds. */
function minutes(value: string | null): number | null {
  if (value === null || !/^\d{1,3}$/.test(value.trim())) return null;
  const n = Number(value);
  return n >= 1 && n <= 720 ? n * 60_000 : null;
}

const MAX_CALLBACK = 500;

/**
 * A callback address we are willing to go to: a plain http or https address with a host and no user name or password.
 * Anything else (other schemes such as `javascript:`, relative paths, very long text) is dropped. Returns the address
 * in its normal written form.
 */
export function safeCallbackUrl(value: string | null): string | null {
  if (value === null || value.length > MAX_CALLBACK) return null;
  try {
    const url = new URL(value);
    if ((url.protocol !== "https:" && url.protocol !== "http:") || url.host === "" || url.username !== "" || url.password !== "") return null;
    return url.href;
  } catch {
    return null;
  }
}

/** The `x-success` and `x-error` addresses of a link, or nothing when it has neither. */
function callbacksOf(params: URLSearchParams): { callbacks: Callbacks } | Record<string, never> {
  const success = safeCallbackUrl(params.get("x-success"));
  const error = safeCallbackUrl(params.get("x-error"));
  return success !== null || error !== null ? { callbacks: { success, error } } : {};
}

/** Read a hash (`#/start?...`) or a protocol link into an action. Anything unknown is null, and ignored. */
export function parseLink(raw: string): LinkAction | null {
  const body = raw.trim().replace(/^#/, "").replace(SCHEME, "").replace(/^\/+/, "");
  const cut = body.indexOf("?");
  const path = cut === -1 ? body : body.slice(0, cut);
  const params = new URLSearchParams(cut === -1 ? "" : body.slice(cut + 1));

  switch (path.split("/")[0]?.toLowerCase()) {
    case "start":
      return {
        kind: "start",
        intention: text(params.get("intent"), MAX_INTENTION),
        durationMs: minutes(params.get("duration")),
        category: text(params.get("category"), MAX_CATEGORY),
        notes: text(params.get("notes"), MAX_NOTES),
        ...callbacksOf(params),
      };
    case "pause":
      return { kind: "pause", ...callbacksOf(params) };
    case "resume":
      return { kind: "resume", ...callbacksOf(params) };
    case "break":
      return { kind: "break", ...callbacksOf(params) };
    case "finish":
      return { kind: "finish", ...callbacksOf(params) };
    case "analytics":
      return { kind: "navigate", route: "history" };
    default:
      return null;
  }
}

/** The hash a `?link=web+openpomodoro:...` visit (from the browser's protocol handler) stands for, or null. */
export function hashFromProtocolLink(link: string): string | null {
  if (!SCHEME.test(link.trim())) return null;
  const action = parseLink(link);
  if (action === null) return null;
  return `#/${link.trim().replace(SCHEME, "").replace(/^\/+/, "")}`;
}

/** Does carrying this action out change the timer (so it needs a yes)? */
export const changesTimer = (action: LinkAction): boolean => action.kind !== "navigate";

export type LinkCommand = { command: CommandInput; notes: string | null } | { unavailable: true };

/** The command that would carry out `action` right now, or `unavailable` if the timer cannot do it in this state. */
export function commandFor(
  action: LinkAction,
  view: Pick<TimerView, "can" | "paused">,
  categories: Category[],
): LinkCommand {
  switch (action.kind) {
    case "start": {
      if (!view.can.init_focus) return { unavailable: true };
      const command: CommandInput = { name: "init_focus" };
      if (action.intention !== null) command.intention = action.intention;
      if (action.durationMs !== null) command.duration_ms = action.durationMs;
      const category = action.category === null ? null : matchCategory(action.category, categories);
      if (category) command.category = category.id;
      return { command, notes: action.notes };
    }
    case "pause":
      return view.can.toggle_pause && !view.paused ? { command: { name: "toggle_pause" }, notes: null } : { unavailable: true };
    case "resume":
      return view.can.toggle_pause && view.paused ? { command: { name: "toggle_pause" }, notes: null } : { unavailable: true };
    case "break":
      return view.can.take_rest ? { command: { name: "take_rest" }, notes: null } : { unavailable: true };
    case "finish":
      return view.can.stop ? { command: { name: "stop" }, notes: null } : { unavailable: true };
    case "navigate":
      return { unavailable: true };
  }
}
