/** Delay before reconnect attempt number `attempt` (0 is the first): doubles from 1 s up to 30 s. */
export function reconnectDelay(attempt: number): number {
  return Math.min(30_000, 1_000 * 2 ** Math.max(0, attempt));
}
