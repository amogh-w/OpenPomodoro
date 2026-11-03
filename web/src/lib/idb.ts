// Local storage that survives reloads and works offline: the last state the server told us about, who is signed
// in, and the outbox. IndexedDB when the browser allows it; otherwise (private windows, blocked storage) an
// in-memory copy so the app still works for as long as the page is open.
import type { NewOutboxItem, OutboxItem, OutboxStore } from "./outbox";

const NAME = "openpomodoro";
const KV = "kv";
const OUTBOX = "outbox";

let opening: Promise<IDBDatabase | null> | null = null;

function open(): Promise<IDBDatabase | null> {
  opening ??= new Promise((resolve) => {
    if (typeof indexedDB === "undefined") return resolve(null);
    try {
      const request = indexedDB.open(NAME, 1);
      request.onupgradeneeded = () => {
        const db = request.result;
        db.createObjectStore(KV);
        db.createObjectStore(OUTBOX, { keyPath: "seq", autoIncrement: true });
      };
      request.onsuccess = () => resolve(request.result);
      request.onerror = () => resolve(null);
      request.onblocked = () => resolve(null);
    } catch {
      resolve(null);
    }
  });
  return opening;
}

/** Run one request in a transaction and wait for it to be committed. */
async function run<T>(store: string, mode: IDBTransactionMode, make: (s: IDBObjectStore) => IDBRequest<T>): Promise<T | null> {
  const db = await open();
  if (db === null) return null;
  return new Promise((resolve, reject) => {
    const tx = db.transaction(store, mode);
    const request = make(tx.objectStore(store));
    tx.oncomplete = () => resolve(request.result);
    tx.onerror = () => reject(tx.error);
    tx.onabort = () => reject(tx.error);
  });
}

// The in-memory stand-in, used only when IndexedDB is not available.
const memoryKv = new Map<string, unknown>();
let memoryOutbox: OutboxItem[] = [];
let memorySeq = 0;

export async function kvGet<T>(key: string): Promise<T | undefined> {
  const stored = await run<T>(KV, "readonly", (s) => s.get(key) as IDBRequest<T>);
  return stored === null ? (memoryKv.get(key) as T | undefined) : (stored ?? undefined);
}

export async function kvSet(key: string, value: unknown): Promise<void> {
  const done = await run(KV, "readwrite", (s) => s.put(value, key));
  if (done === null) memoryKv.set(key, value);
}

export async function kvDelete(key: string): Promise<void> {
  const done = await run(KV, "readwrite", (s) => s.delete(key));
  if (done === null) memoryKv.delete(key);
}

export async function outboxAdd(item: NewOutboxItem): Promise<void> {
  const added = await run(OUTBOX, "readwrite", (s) => s.add(item));
  if (added === null) memoryOutbox.push({ ...item, seq: ++memorySeq } as OutboxItem);
}

export async function outboxList(userId: string): Promise<OutboxItem[]> {
  const all = await run<OutboxItem[]>(OUTBOX, "readonly", (s) => s.getAll() as IDBRequest<OutboxItem[]>);
  const items = all ?? memoryOutbox;
  return items.filter((i) => i.userId === userId).sort((a, b) => a.seq - b.seq);
}

export async function outboxRemove(seq: number): Promise<void> {
  const done = await run(OUTBOX, "readwrite", (s) => s.delete(seq));
  if (done === null) memoryOutbox = memoryOutbox.filter((i) => i.seq !== seq);
}

export async function outboxClear(userId: string): Promise<void> {
  for (const item of await outboxList(userId)) await outboxRemove(item.seq);
}

export const outboxStore: OutboxStore = { list: outboxList, remove: outboxRemove };
