import { describe, expect, it } from "vitest";
import { announcementFor } from "./announce";

describe("announcementFor", () => {
  it("says nothing the first time it sees a state, or when nothing changed", () => {
    expect(announcementFor(null, "session")).toBeNull();
    expect(announcementFor("session", "session")).toBeNull();
  });

  it("announces the start of focus from idle, breathing or a break", () => {
    for (const from of ["idle", "breathe", "rest_end", "rest"]) expect(announcementFor(from, "session"), from).toBe("announce.focusStarted");
  });

  it("announces the moments of a session", () => {
    expect(announcementFor("idle", "breathe")).toBe("announce.breathe");
    expect(announcementFor("session", "session_end")).toBe("announce.focusTimeUp");
    expect(announcementFor("session_end", "reflect")).toBe("announce.focusComplete");
    expect(announcementFor("reflect", "rest")).toBe("announce.breakStarted");
    expect(announcementFor("rest", "rest_end")).toBe("announce.breakOver");
  });

  it("announces pausing and resuming for any kind of pause", () => {
    for (const p of ["paused_session", "paused_session_end", "paused_rest", "paused_rest_end"]) {
      expect(announcementFor("session", p), p).toBe("announce.paused");
      expect(announcementFor(p, "session"), p).toBe("announce.resumed");
    }
  });

  it("does not announce a pause turning into a different pause", () => {
    expect(announcementFor("paused_session", "paused_rest")).toBe("announce.paused");
  });

  it("stays quiet when the timer simply returns to idle", () => {
    for (const from of ["reflect", "rest", "rest_end", "session"]) expect(announcementFor(from, "idle"), from).toBeNull();
  });
});
