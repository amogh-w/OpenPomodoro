import { describe, expect, it } from "vitest";
import { type KeyValueStore, createIdSource } from "./clientIds";

function memoryStore(): KeyValueStore & { data: Map<string, string> } {
  const data = new Map<string, string>();
  return { data, getItem: (k) => data.get(k) ?? null, setItem: (k, v) => void data.set(k, v) };
}

describe("createIdSource", () => {
  it("creates a device id once and keeps it", () => {
    const store = memoryStore();
    const a = createIdSource(store, () => "device-1");
    const b = createIdSource(store, () => "device-2");
    expect(a.deviceId).toBe("device-1");
    expect(b.deviceId).toBe("device-1");
  });

  it("does not repeat an id after a reload within the same second", () => {
    const store = memoryStore();
    const first = createIdSource(store, () => "d").next(5_000);
    const afterReload = createIdSource(store, () => "d").next(5_000);
    expect(afterReload > first).toBe(true);
  });

  it("stays increasing when the clock goes back across a reload", () => {
    const store = memoryStore();
    const first = createIdSource(store, () => "d").next(9_000_000);
    const second = createIdSource(store, () => "d").next(1_000);
    expect(second > first).toBe(true);
  });

  it("still works when storage throws", () => {
    const broken: KeyValueStore = {
      getItem: () => {
        throw new Error("blocked");
      },
      setItem: () => {
        throw new Error("blocked");
      },
    };
    const source = createIdSource(broken, () => "d");
    expect(source.next(1_000) < source.next(1_000)).toBe(true);
  });
});
