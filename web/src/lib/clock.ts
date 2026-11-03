// The server's clock is the one that counts: timers run on timestamps the server assigns, so every device must
// agree with it. This keeps the difference between the server's time and this device's and applies it.

export class ServerClock {
  private offset = 0;

  /**
   * Learn the offset from one server answer. `sentAt` and `receivedAt` are this device's clock before the
   * request and after the response; the server stamped its time somewhere in between, so half the round trip
   * is the best estimate of how stale the stamp is.
   */
  sync(serverTime: number, sentAt: number, receivedAt: number): void {
    const latency = Math.max(0, receivedAt - sentAt) / 2;
    this.offset = serverTime + latency - receivedAt;
  }

  /** The server's current time, as best we know it. */
  now(deviceNow: number = Date.now()): number {
    return deviceNow + this.offset;
  }

  get offsetMs(): number {
    return this.offset;
  }
}
