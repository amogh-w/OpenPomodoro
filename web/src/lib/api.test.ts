import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError, api } from "./api";

function reply(status: number, body?: unknown): Response {
  const headers = new Headers(body === undefined ? {} : { "content-type": "application/json" });
  return new Response(body === undefined ? null : JSON.stringify(body), { status, headers });
}

function stubFetch(response: Response | Error) {
  const fn = vi.fn(async (_url: string, _init?: RequestInit) => {
    if (response instanceof Error) throw response;
    return response;
  });
  vi.stubGlobal("fetch", fn);
  return fn;
}

afterEach(() => vi.unstubAllGlobals());

describe("api", () => {
  it("sends the csrf header and same-origin credentials on every request", async () => {
    const fetchFn = stubFetch(reply(200, { needs_setup: false, registration: "closed" }));
    await api.setupStatus();
    const [url, init] = fetchFn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/setup");
    expect(init.method).toBe("GET");
    expect(init.credentials).toBe("same-origin");
    expect((init.headers as Record<string, string>)["x-requested-with"]).toBe("openpomodoro");
    expect(init.body).toBeUndefined();
  });

  it("sends JSON bodies with a content type", async () => {
    const fetchFn = stubFetch(reply(200, { id: "u", username: "a", role: "user", disabled: false, created_at: 1 }));
    await api.signIn("amy", "a long enough password");
    const [, init] = fetchFn.mock.calls[0] as [string, RequestInit];
    expect(init.method).toBe("POST");
    expect((init.headers as Record<string, string>)["content-type"]).toBe("application/json");
    expect(JSON.parse(init.body as string)).toEqual({ username: "amy", password: "a long enough password" });
  });

  it("returns undefined for 204", async () => {
    stubFetch(reply(204));
    await expect(api.signOut()).resolves.toBeUndefined();
  });

  it("turns an error body into an ApiError with the server's code", async () => {
    stubFetch(reply(401, { error: "invalid_credentials" }));
    const err = await api.signIn("a", "b").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ status: 401, code: "invalid_credentials", retryAfterMs: null });
  });

  it("carries the retry delay of a rate limit", async () => {
    stubFetch(reply(429, { error: "rate_limited", retry_after_ms: 30_000 }));
    const err = (await api.signIn("a", "b").catch((e: unknown) => e)) as ApiError;
    expect(err.code).toBe("rate_limited");
    expect(err.retryAfterMs).toBe(30_000);
  });

  it("reports an unreachable server as a network error", async () => {
    stubFetch(new TypeError("Failed to fetch"));
    const err = (await api.state().catch((e: unknown) => e)) as ApiError;
    expect(err.isNetwork).toBe(true);
    expect(err.code).toBe("network");
  });

  it("copes with an error that has no JSON body", async () => {
    stubFetch(reply(502));
    const err = (await api.state().catch((e: unknown) => e)) as ApiError;
    expect(err).toMatchObject({ status: 502, code: "unknown" });
  });

  it("puts ids in the path safely", async () => {
    const fetchFn = stubFetch(reply(204));
    await api.adminSetDisabled("a/b c", true);
    expect((fetchFn.mock.calls[0] as [string])[0]).toBe("/api/admin/users/a%2Fb%20c/disabled");
  });

  it("posts commands with their id", async () => {
    const fetchFn = stubFetch(reply(200, { status: "applied", state: {} }));
    await api.command("id-1", { name: "init_focus", intention: "write" });
    const [url, init] = fetchFn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/commands");
    expect(JSON.parse(init.body as string)).toEqual({ id: "id-1", name: "init_focus", intention: "write" });
  });
});
