import { describe, expect, it } from "vitest";
import { ApiError } from "./api";
import { type OutboxApi, type OutboxItem, type OutboxStore, flush } from "./outbox";

function command(seq: number, id: string): OutboxItem {
  return { seq, userId: "u", createdAt: 0, kind: "command", id, input: { name: "init_focus", at: seq * 1000 } };
}

function events(seq: number): OutboxItem {
  return {
    seq,
    userId: "u",
    createdAt: 0,
    kind: "events",
    events: [{ id: `e${seq}`, entity: "setting", op: "add", key: "strict", payload: { value: true } }],
  };
}

function memoryStore(items: OutboxItem[]): OutboxStore & { items: OutboxItem[] } {
  const store = {
    items,
    list: async (userId: string) => store.items.filter((i) => i.userId === userId),
    remove: async (seq: number) => {
      store.items = store.items.filter((i) => i.seq !== seq);
    },
  };
  return store;
}

/** An API that records what it was asked and answers from a script. */
function scriptedApi(answers: (Error | "ok" | "invalid-event")[]): OutboxApi & { calls: string[] } {
  const calls: string[] = [];
  let n = 0;
  const next = (): Error | "ok" | "invalid-event" => answers[n++] ?? "ok";
  return {
    calls,
    command: async (id) => {
      calls.push(`command:${id}`);
      const a = next();
      if (a instanceof Error) throw a;
      return {};
    },
    events: async (evs) => {
      calls.push(`events:${evs[0]?.id}`);
      const a = next();
      if (a instanceof Error) throw a;
      return { results: evs.map((e) => (a === "invalid-event" ? { id: e.id, result: "invalid", error: "bad_setting_value" } : { id: e.id, result: "added" })) };
    },
  };
}

describe("flush", () => {
  it("sends everything in order and empties the outbox", async () => {
    const store = memoryStore([command(1, "a"), events(2), command(3, "b")]);
    const api = scriptedApi([]);
    const result = await flush("u", store, api);
    expect(api.calls).toEqual(["command:a", "events:e2", "command:b"]);
    expect(result).toMatchObject({ sent: 3, dropped: [], complete: true, unauthorized: false });
    expect(store.items).toEqual([]);
  });

  it("does nothing for an empty outbox", async () => {
    const result = await flush("u", memoryStore([]), scriptedApi([]));
    expect(result).toEqual({ sent: 0, dropped: [], complete: true, unauthorized: false });
  });

  it("only sends the signed-in user's items", async () => {
    const other: OutboxItem = { ...command(2, "theirs"), userId: "someone-else" };
    const store = memoryStore([command(1, "mine"), other]);
    const api = scriptedApi([]);
    await flush("u", store, api);
    expect(api.calls).toEqual(["command:mine"]);
    expect(store.items).toEqual([other]);
  });

  it("stops when the server cannot be reached and keeps order for next time", async () => {
    const store = memoryStore([command(1, "a"), command(2, "b"), command(3, "c")]);
    const api = scriptedApi(["ok", new ApiError(0, "network")]);
    const result = await flush("u", store, api);
    expect(api.calls).toEqual(["command:a", "command:b"]);
    expect(result).toMatchObject({ sent: 1, complete: false });
    expect(store.items.map((i) => i.seq)).toEqual([2, 3]);
  });

  it("treats a server error as unreachable and keeps the item", async () => {
    const store = memoryStore([command(1, "a")]);
    const result = await flush("u", store, scriptedApi([new ApiError(503, "unknown")]));
    expect(result.complete).toBe(false);
    expect(store.items).toHaveLength(1);
  });

  it("drops an item the server refuses and carries on with the rest", async () => {
    const store = memoryStore([command(1, "a"), command(2, "b")]);
    const api = scriptedApi([new ApiError(409, "rejected_unknown"), "ok"]);
    const result = await flush("u", store, api);
    expect(api.calls).toEqual(["command:a", "command:b"]);
    expect(result.sent).toBe(1);
    expect(result.complete).toBe(true);
    expect(result.dropped.map((d) => [d.item.seq, d.code])).toEqual([[1, "rejected_unknown"]]);
    expect(store.items).toEqual([]);
  });

  it("reports events the server found invalid", async () => {
    const store = memoryStore([events(1)]);
    const result = await flush("u", store, scriptedApi(["invalid-event"]));
    expect(result.sent).toBe(0);
    expect(result.dropped.map((d) => d.code)).toEqual(["bad_setting_value"]);
    expect(store.items).toEqual([]);
  });

  it("stops and says so when the session is gone", async () => {
    const store = memoryStore([command(1, "a"), command(2, "b")]);
    const result = await flush("u", store, scriptedApi([new ApiError(401, "unauthorized")]));
    expect(result).toMatchObject({ unauthorized: true, complete: false });
    expect(store.items).toHaveLength(2);
  });

  it("lets a surprise bug through instead of hiding it", async () => {
    const store = memoryStore([command(1, "a")]);
    await expect(flush("u", store, scriptedApi([new TypeError("boom")]))).rejects.toThrow("boom");
    expect(store.items).toHaveLength(1);
  });
});
