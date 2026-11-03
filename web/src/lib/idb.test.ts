import "fake-indexeddb/auto";
import { beforeEach, describe, expect, it } from "vitest";
import { kvDelete, kvGet, kvSet, outboxAdd, outboxClear, outboxList, outboxRemove } from "./idb";

const cmd = (userId: string, id: string) => ({
  userId,
  createdAt: 1,
  kind: "command" as const,
  id,
  input: { name: "init_focus", at: 5 },
});

beforeEach(async () => {
  await outboxClear("alice");
  await outboxClear("bob");
});

describe("key-value storage", () => {
  it("stores, reads and deletes values", async () => {
    expect(await kvGet("nothing")).toBeUndefined();
    await kvSet("state:alice", { timer: { state: "session" }, n: 1 });
    expect(await kvGet("state:alice")).toEqual({ timer: { state: "session" }, n: 1 });
    await kvDelete("state:alice");
    expect(await kvGet("state:alice")).toBeUndefined();
  });

  it("overwrites an existing key", async () => {
    await kvSet("k", 1);
    await kvSet("k", 2);
    expect(await kvGet("k")).toBe(2);
  });
});

describe("outbox storage", () => {
  it("lists a user's items oldest first with increasing numbers", async () => {
    await outboxAdd(cmd("alice", "a"));
    await outboxAdd(cmd("alice", "b"));
    await outboxAdd(cmd("alice", "c"));
    const items = await outboxList("alice");
    expect(items.map((i) => (i.kind === "command" ? i.id : ""))).toEqual(["a", "b", "c"]);
    expect(items.map((i) => i.seq)).toEqual([...items.map((i) => i.seq)].sort((x, y) => x - y));
    expect(new Set(items.map((i) => i.seq)).size).toBe(3);
  });

  it("keeps users apart", async () => {
    await outboxAdd(cmd("alice", "a"));
    await outboxAdd(cmd("bob", "b"));
    expect((await outboxList("alice")).map((i) => i.userId)).toEqual(["alice"]);
    expect((await outboxList("bob")).map((i) => i.userId)).toEqual(["bob"]);
  });

  it("removes one item, or all of a user's", async () => {
    await outboxAdd(cmd("alice", "a"));
    await outboxAdd(cmd("alice", "b"));
    await outboxAdd(cmd("bob", "x"));
    const [first] = await outboxList("alice");
    await outboxRemove(first?.seq ?? -1);
    expect(await outboxList("alice")).toHaveLength(1);
    await outboxClear("alice");
    expect(await outboxList("alice")).toEqual([]);
    expect(await outboxList("bob")).toHaveLength(1);
  });

  it("round-trips an events item", async () => {
    await outboxAdd({
      userId: "alice",
      createdAt: 1,
      kind: "events",
      events: [{ id: "e", entity: "setting", op: "add", key: "strict", payload: { value: true } }],
    });
    const [item] = await outboxList("alice");
    expect(item).toMatchObject({ kind: "events", events: [{ key: "strict" }] });
  });
});
