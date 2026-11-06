import { describe, expect, it } from "vitest";
import { type KeyPress, actionFor, bindKey, defaultKeymap, isTyping, normalizeKey, parseKeymap } from "./shortcuts";

const press = (key: string, over: Partial<KeyPress> = {}): KeyPress => ({
  key,
  ctrlKey: false,
  metaKey: false,
  altKey: false,
  target: { tagName: "BODY" },
  ...over,
});
const free = { dialogOpen: false };

describe("actionFor", () => {
  it("maps the keys to actions", () => {
    const table: [string, string][] = [
      [" ", "toggle_pause"], ["p", "toggle_pause"], ["P", "toggle_pause"], ["s", "start"], ["+", "extend"], ["=", "extend"],
      ["-", "shrink"], ["_", "shrink"], ["/", "focus_input"], ["f", "finish"], ["b", "break"], ["Escape", "cancel"],
    ];
    for (const [key, action] of table) expect(actionFor(press(key), free), key).toBe(action);
  });

  it("ignores keys it has no use for", () => {
    for (const key of ["a", "x", "1", "Enter", "Tab", "ArrowUp", "F5"]) expect(actionFor(press(key), free), key).toBeNull();
  });

  it("never fires while the person is typing", () => {
    for (const tagName of ["INPUT", "TEXTAREA", "SELECT", "input"]) {
      expect(actionFor(press("p", { target: { tagName } }), free), tagName).toBeNull();
      expect(actionFor(press(" ", { target: { tagName } }), free), tagName).toBeNull();
    }
    expect(actionFor(press("s", { target: { tagName: "DIV", isContentEditable: true } }), free)).toBeNull();
  });

  it("lets Escape through even while typing, so a field can be left", () => {
    expect(actionFor(press("Escape", { target: { tagName: "INPUT" } }), free)).toBe("cancel");
  });

  it("leaves browser and system shortcuts alone", () => {
    for (const mod of ["ctrlKey", "metaKey", "altKey"] as const) {
      expect(actionFor(press("p", { [mod]: true }), free), mod).toBeNull();
      expect(actionFor(press("-", { [mod]: true }), free), mod).toBeNull();
    }
  });

  it("does nothing while a dialog is open", () => {
    expect(actionFor(press("p"), { dialogOpen: true })).toBeNull();
    expect(actionFor(press("Escape"), { dialogOpen: true })).toBeNull();
  });

  it("leaves Space to a focused button, link or summary, but P still works", () => {
    for (const tagName of ["BUTTON", "A", "SUMMARY"]) {
      expect(actionFor(press(" ", { target: { tagName } }), free), tagName).toBeNull();
      expect(actionFor(press("p", { target: { tagName } }), free), tagName).toBe("toggle_pause");
    }
  });

  it("works with no focused element", () => {
    expect(actionFor(press("p", { target: null }), free)).toBe("toggle_pause");
  });
});

describe("isTyping", () => {
  it("is true only for fields", () => {
    expect(isTyping({ tagName: "INPUT" })).toBe(true);
    expect(isTyping({ tagName: "BUTTON" })).toBe(false);
    expect(isTyping(null)).toBe(false);
  });
});

describe("custom keys", () => {
  it("normalizes to one lower-case character", () => {
    expect([normalizeKey("P"), normalizeKey(" "), normalizeKey("/"), normalizeKey("Enter"), normalizeKey("F5"), normalizeKey("")]).toEqual([
      "p", " ", "/", null, null, null,
    ]);
  });

  it("uses a changed key and stops using the old one", () => {
    const result = bindKey(defaultKeymap, "start", "g");
    expect("map" in result).toBe(true);
    if (!("map" in result)) return;
    expect(actionFor(press("g"), free, result.map)).toBe("start");
    expect(actionFor(press("G"), free, result.map)).toBe("start");
    expect(actionFor(press("s"), free, result.map)).toBeNull();
  });

  it("refuses a key another action has, and keys that cannot be bound", () => {
    expect(bindKey(defaultKeymap, "start", "b")).toEqual({ error: "taken", by: "break" });
    expect(bindKey(defaultKeymap, "start", "Enter")).toEqual({ error: "invalid" });
    expect("map" in bindKey(defaultKeymap, "start", "s")).toBe(true);
  });

  it("repairs a stored keymap action by action", () => {
    expect(parseKeymap(null)).toEqual(defaultKeymap);
    const map = parseKeymap({ start: ["g"], finish: ["g"], break: "x", extend: [], shrink: ["Tab"], focus_input: ["/"] });
    expect(map.start).toEqual(["g"]);
    expect(map.finish).toEqual(defaultKeymap.finish);
    expect(map.break).toEqual(defaultKeymap.break);
    expect(map.extend).toEqual(defaultKeymap.extend);
    expect(map.shrink).toEqual(defaultKeymap.shrink);
  });

  it("starts over when a stored key collides with another action's default", () => {
    expect(parseKeymap({ start: ["p"] })).toEqual(defaultKeymap);
  });

  it("keeps Escape and the Space-on-a-button rule whatever the keys", () => {
    const map = bindKey(defaultKeymap, "start", " ");
    expect("error" in map).toBe(true); // Space belongs to pause
    expect(actionFor(press("Escape", { target: { tagName: "INPUT" } }), free, defaultKeymap)).toBe("cancel");
  });
});
