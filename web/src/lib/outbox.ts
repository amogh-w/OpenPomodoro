// The outbox: things the user did while the server could not be reached, kept until they can be sent. Each item
// carries its own id, so sending one twice (after a lost answer, or from two tabs) does nothing the second time.
import { ApiError } from "./api";
import type { CommandInput, EventInput, EventResult } from "./types";

type Base = { userId: string; createdAt: number };
export type OutboxItem = Base & { seq: number } & (
    | { kind: "command"; id: string; input: CommandInput }
    | { kind: "events"; events: EventInput[] }
  );

/** An item before the store has numbered it. */
export type NewOutboxItem = Base & (
    | { kind: "command"; id: string; input: CommandInput }
    | { kind: "events"; events: EventInput[] }
  );

export interface OutboxApi {
  command(id: string, input: CommandInput): Promise<unknown>;
  events(events: EventInput[]): Promise<{ results: EventResult[] }>;
}

export interface OutboxStore {
  /** The user's items, oldest first. */
  list(userId: string): Promise<OutboxItem[]>;
  remove(seq: number): Promise<void>;
}

export interface Dropped {
  item: OutboxItem;
  /** The server's reason (`rejected_unknown`, `bad_args`, ...). */
  code: string;
}

export interface FlushResult {
  sent: number;
  /** Items the server refused for good; they are removed, not retried. */
  dropped: Dropped[];
  /** Everything was sent or dropped. False when sending stopped early and items remain. */
  complete: boolean;
  /** The server no longer accepts our session. */
  unauthorized: boolean;
}

/**
 * Send the user's items in order. Stops at the first sign that the server is unreachable (the rest wait for next
 * time, order kept). An item the server refuses outright is dropped and the rest carry on.
 */
export async function flush(userId: string, store: OutboxStore, api: OutboxApi): Promise<FlushResult> {
  const result: FlushResult = { sent: 0, dropped: [], complete: true, unauthorized: false };

  for (const item of await store.list(userId)) {
    try {
      if (item.kind === "command") {
        await api.command(item.id, item.input);
      } else {
        const { results } = await api.events(item.events);
        const refused = results.find((r) => r.result === "invalid");
        if (refused) result.dropped.push({ item, code: refused.error ?? "invalid" });
      }
      if (!result.dropped.some((d) => d.item === item)) result.sent += 1;
      await store.remove(item.seq);
    } catch (error) {
      if (!(error instanceof ApiError)) throw error;
      if (error.status === 401) {
        result.unauthorized = true;
        result.complete = false;
        return result;
      }
      if (error.isNetwork || error.status >= 500) {
        result.complete = false;
        return result;
      }
      // 4xx: the server understood and said no (the world moved on, or the item was malformed).
      result.dropped.push({ item, code: error.code });
      await store.remove(item.seq);
    }
  }
  return result;
}
