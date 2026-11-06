// The browser's side of the server API. Cookies carry the session; every request also sends the CSRF header
// the server requires for anything that changes data.
import type {
  CommandInput,
  DayTotal,
  CommandResponse,
  EventInput,
  EventResult,
  ServerState,
  SessionRecord,
  SetupStatus,
  User,
} from "./types";

export class ApiError extends Error {
  constructor(
    readonly status: number,
    readonly code: string,
    readonly retryAfterMs: number | null = null,
  ) {
    super(code);
    this.name = "ApiError";
  }

  /** The server could not be reached at all (status 0), as opposed to answering with an error. */
  get isNetwork(): boolean {
    return this.status === 0;
  }
}

type Method = "GET" | "POST" | "PUT" | "DELETE";

async function request<T>(method: Method, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = { "x-requested-with": "openpomodoro" };
  if (body !== undefined) headers["content-type"] = "application/json";

  let res: Response;
  try {
    res = await fetch(path, {
      method,
      headers,
      credentials: "same-origin",
      body: body === undefined ? undefined : JSON.stringify(body),
    });
  } catch {
    throw new ApiError(0, "network");
  }

  if (res.status === 204) return undefined as T;
  const isJson = res.headers.get("content-type")?.includes("application/json") ?? false;
  const data: unknown = isJson ? await res.json().catch(() => null) : null;

  if (!res.ok) {
    const info = (data ?? {}) as { error?: string; retry_after_ms?: number };
    throw new ApiError(res.status, info.error ?? "unknown", info.retry_after_ms ?? null);
  }
  return data as T;
}

export interface SoundInfo {
  slot: string;
  mime: string;
  size: number;
  updated_at: number;
}

/** Upload or download raw sound bytes; unlike the JSON calls the body and the answer are the file itself. */
async function soundFetch(method: "GET" | "PUT", slot: string, body?: Blob): Promise<Response> {
  let res: Response;
  try {
    res = await fetch(`/api/sounds/${slot}`, {
      method,
      headers: { "x-requested-with": "openpomodoro" },
      credentials: "same-origin",
      body,
    });
  } catch {
    throw new ApiError(0, "network");
  }
  if (!res.ok) {
    const info = (await res.json().catch(() => ({}))) as { error?: string };
    throw new ApiError(res.status, info.error ?? "unknown");
  }
  return res;
}

export const api = {
  listSounds: () => request<{ sounds: SoundInfo[] }>("GET", "/api/sounds"),
  uploadSound: async (slot: string, file: Blob): Promise<SoundInfo> => (await soundFetch("PUT", slot, file)).json() as Promise<SoundInfo>,
  downloadSound: async (slot: string): Promise<ArrayBuffer> => (await soundFetch("GET", slot)).arrayBuffer(),
  removeSound: (slot: string) => request<void>("DELETE", `/api/sounds/${slot}`),
  setupStatus: () => request<SetupStatus>("GET", "/api/setup"),
  setup: (username: string, password: string) => request<User>("POST", "/api/setup", { username, password }),
  register: (username: string, password: string) => request<User>("POST", "/api/register", { username, password }),
  signIn: (username: string, password: string) => request<User>("POST", "/api/signin", { username, password }),
  signOut: () => request<void>("POST", "/api/signout"),
  me: () => request<User>("GET", "/api/me"),
  changePassword: (current: string, next: string) =>
    request<void>("POST", "/api/me/password", { current, new: next }),
  wipeData: (password: string) => request<void>("POST", "/api/me/wipe", { password }),
  importBackup: (backup: unknown) =>
    request<{ imported: { categories: number; tasks: number; sessions: number; settings: number }; skipped: number }>("POST", "/api/me/import", backup),
  deleteAccount: (password: string) => request<void>("DELETE", "/api/me", { password }),

  adminUsers: () => request<User[]>("GET", "/api/admin/users"),
  adminCreateUser: (username: string, password: string, role: "user" | "admin") =>
    request<User>("POST", "/api/admin/users", { username, password, role }),
  adminSetPassword: (id: string, password: string) =>
    request<void>("POST", `/api/admin/users/${encodeURIComponent(id)}/password`, { password }),
  adminSetDisabled: (id: string, disabled: boolean) =>
    request<void>("POST", `/api/admin/users/${encodeURIComponent(id)}/disabled`, { disabled }),
  adminRegistration: () => request<{ policy: "open" | "closed" }>("GET", "/api/admin/registration"),
  adminSetRegistration: (policy: "open" | "closed") =>
    request<void>("PUT", "/api/admin/registration", { policy }),

  state: () => request<ServerState>("GET", "/api/state"),
  command: (id: string, input: CommandInput) =>
    request<CommandResponse>("POST", "/api/commands", { id, ...input }),
  events: (events: EventInput[]) => request<{ results: EventResult[] }>("POST", "/api/events", { events }),
  summary: (tzOffsetMinutes: number) => request<{ days: DayTotal[] }>("GET", `/api/summary?tz=${tzOffsetMinutes}`),
  history: (from: number, to: number) =>
    request<{ sessions: SessionRecord[]; truncated: boolean }>("GET", `/api/history?from=${from}&to=${to}`),
};
