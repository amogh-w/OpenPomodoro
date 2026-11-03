import { afterEach, describe, expect, it, vi } from "vitest";

// A browser without IndexedDB (or with it blocked) must still work for as long as the page stays open.
afterEach(() => {
  vi.unstubAllGlobals();
  vi.resetModules();
});

describe("storage without IndexedDB", () => {
  it("falls back to memory for values and the outbox", async () => {
    vi.resetModules();
    vi.stubGlobal("indexedDB", undefined);
    const idb = await import("./idb");

    await idb.kvSet("k", { a: 1 });
    expect(await idb.kvGet("k")).toEqual({ a: 1 });
    await idb.kvDelete("k");
    expect(await idb.kvGet("k")).toBeUndefined();

    await idb.outboxAdd({ userId: "u", createdAt: 1, kind: "command", id: "a", input: { name: "init_focus" } });
    await idb.outboxAdd({ userId: "u", createdAt: 2, kind: "command", id: "b", input: { name: "stop" } });
    const items = await idb.outboxList("u");
    expect(items.map((i) => (i.kind === "command" ? i.id : ""))).toEqual(["a", "b"]);
    await idb.outboxRemove(items[0]?.seq ?? -1);
    expect(await idb.outboxList("u")).toHaveLength(1);
    await idb.outboxClear("u");
    expect(await idb.outboxList("u")).toEqual([]);
  });
});
