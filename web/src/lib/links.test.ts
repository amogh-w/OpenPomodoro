import { describe, expect, it } from "vitest";
import { changesTimer, commandFor, hashFromProtocolLink, parseLink } from "./links";
import type { TimerView } from "./timerView";
import type { Category } from "./types";

const cats: Category[] = [{ id: "c1", name: "Deep work" }, { id: "c2", name: "Reading" }];

const view = (can: Partial<TimerView["can"]>, paused = false): Pick<TimerView, "can" | "paused"> => ({
  can: { init_focus: false, skip_breathe: false, toggle_pause: false, extend: false, stop: false, abandon: false, take_rest: false, done: false, skip_rest: false, ...can },
  paused,
});

describe("parseLink", () => {
  it("reads a start link with all its parts", () => {
    expect(parseLink("#/start?intent=write%20report&duration=30&category=Deep%20work&notes=first%20draft")).toEqual({
      kind: "start", intention: "write report", durationMs: 30 * 60_000, category: "Deep work", notes: "first draft",
    });
  });

  it("treats + as a space and works without any parts", () => {
    expect(parseLink("#/start?intent=write+report")).toMatchObject({ intention: "write report" });
    expect(parseLink("#/start")).toEqual({ kind: "start", intention: null, durationMs: null, category: null, notes: null });
  });

  it("reads the simple actions, ignoring case and trailing parts", () => {
    expect(parseLink("#/pause")).toEqual({ kind: "pause" });
    expect(parseLink("#/RESUME")).toEqual({ kind: "resume" });
    expect(parseLink("#/break?x=1")).toEqual({ kind: "break" });
    expect(parseLink("#/finish/")).toEqual({ kind: "finish" });
    expect(parseLink("#/analytics")).toEqual({ kind: "navigate", route: "history" });
  });

  it("accepts the protocol forms", () => {
    expect(parseLink("web+openpomodoro:start?intent=x")).toMatchObject({ kind: "start", intention: "x" });
    expect(parseLink("web+openpomodoro://pause")).toEqual({ kind: "pause" });
    expect(parseLink("WEB+OPENPOMODORO://Finish")).toEqual({ kind: "finish" });
  });

  it("drops values that are out of range instead of guessing", () => {
    const a = parseLink("#/start?duration=0&intent=%20%20&category=" + "x".repeat(65));
    expect(a).toEqual({ kind: "start", intention: null, durationMs: null, category: null, notes: null });
    for (const bad of ["abc", "-5", "1000", "12.5", "721", ""]) {
      expect(parseLink(`#/start?duration=${bad}`), bad).toMatchObject({ durationMs: null });
    }
    expect(parseLink("#/start?duration=720")).toMatchObject({ durationMs: 720 * 60_000 });
    expect(parseLink(`#/start?intent=${"y".repeat(201)}`)).toMatchObject({ intention: null });
  });

  it("ignores anything it does not know", () => {
    for (const unknown of ["", "#", "#/", "#/timer", "#/settings", "#/nonsense", "#/delete-everything", "https://evil.example/start"]) {
      expect(parseLink(unknown), unknown).toBeNull();
    }
  });
});

describe("hashFromProtocolLink", () => {
  it("turns a protocol link into the hash it means", () => {
    expect(hashFromProtocolLink("web+openpomodoro://start?intent=x&duration=20")).toBe("#/start?intent=x&duration=20");
    expect(hashFromProtocolLink("web+openpomodoro:pause")).toBe("#/pause");
  });

  it("refuses other schemes and unknown actions", () => {
    expect(hashFromProtocolLink("https://example.com/start")).toBeNull();
    expect(hashFromProtocolLink("javascript:alert(1)")).toBeNull();
    expect(hashFromProtocolLink("web+openpomodoro://nonsense")).toBeNull();
    expect(hashFromProtocolLink("")).toBeNull();
  });
});

describe("changesTimer", () => {
  it("is true for everything except opening a screen", () => {
    expect(changesTimer({ kind: "pause" })).toBe(true);
    expect(changesTimer({ kind: "start", intention: null, durationMs: null, category: null, notes: null })).toBe(true);
    expect(changesTimer({ kind: "navigate", route: "history" })).toBe(false);
  });
});

describe("commandFor", () => {
  it("starts a session with the link's intention, length and category", () => {
    const result = commandFor(parseLink("#/start?intent=write&duration=30&category=deep&notes=hello") as never, view({ init_focus: true }), cats);
    expect(result).toEqual({ command: { name: "init_focus", intention: "write", duration_ms: 30 * 60_000, category: "c1" }, notes: "hello" });
  });

  it("starts with nothing extra when the link has nothing extra", () => {
    expect(commandFor({ kind: "start", intention: null, durationMs: null, category: null, notes: null }, view({ init_focus: true }), cats)).toEqual({
      command: { name: "init_focus" }, notes: null,
    });
  });

  it("ignores a category that does not exist", () => {
    const r = commandFor({ kind: "start", intention: null, durationMs: null, category: "nope", notes: null }, view({ init_focus: true }), cats);
    expect(r).toEqual({ command: { name: "init_focus" }, notes: null });
  });

  it("pauses only a running timer and resumes only a paused one", () => {
    expect(commandFor({ kind: "pause" }, view({ toggle_pause: true }), cats)).toEqual({ command: { name: "toggle_pause" }, notes: null });
    expect(commandFor({ kind: "pause" }, view({ toggle_pause: true }, true), cats)).toEqual({ unavailable: true });
    expect(commandFor({ kind: "resume" }, view({ toggle_pause: true }, true), cats)).toEqual({ command: { name: "toggle_pause" }, notes: null });
    expect(commandFor({ kind: "resume" }, view({ toggle_pause: true }, false), cats)).toEqual({ unavailable: true });
  });

  it("offers break and finish only where the state machine does", () => {
    expect(commandFor({ kind: "break" }, view({ take_rest: true }), cats)).toEqual({ command: { name: "take_rest" }, notes: null });
    expect(commandFor({ kind: "break" }, view({}), cats)).toEqual({ unavailable: true });
    expect(commandFor({ kind: "finish" }, view({ stop: true }), cats)).toEqual({ command: { name: "stop" }, notes: null });
    expect(commandFor({ kind: "finish" }, view({}), cats)).toEqual({ unavailable: true });
  });

  it("cannot start while a session is already running", () => {
    expect(commandFor({ kind: "start", intention: "x", durationMs: null, category: null, notes: null }, view({ init_focus: false }), cats)).toEqual({ unavailable: true });
  });
});
