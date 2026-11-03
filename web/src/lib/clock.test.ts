import { describe, expect, it } from "vitest";
import { ServerClock } from "./clock";

describe("ServerClock", () => {
  it("starts in step with the device clock", () => {
    expect(new ServerClock().now(1_000)).toBe(1_000);
  });

  it("follows a server that is ahead", () => {
    const clock = new ServerClock();
    clock.sync(10_000, 1_000, 1_000); // instant answer; server is 9 s ahead
    expect(clock.now(1_000)).toBe(10_000);
    expect(clock.now(4_000)).toBe(13_000);
  });

  it("follows a server that is behind", () => {
    const clock = new ServerClock();
    clock.sync(1_000, 5_000, 5_000);
    expect(clock.now(5_000)).toBe(1_000);
  });

  it("allows for half the round trip", () => {
    const clock = new ServerClock();
    clock.sync(10_000, 1_000, 1_400); // the answer took 400 ms, so the stamp is about 200 ms old
    expect(clock.now(1_400)).toBe(10_200);
  });

  it("ignores a negative round trip from a clock that jumped", () => {
    const clock = new ServerClock();
    clock.sync(10_000, 2_000, 1_000);
    expect(clock.now(1_000)).toBe(10_000);
  });
});
