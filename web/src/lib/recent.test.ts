import { describe, expect, it } from "vitest";
import { notesFeed, recentIntentions, recentNotes } from "./recent";
import type { SessionRecord } from "./types";

let n = 0;
function session(over: Partial<SessionRecord>): SessionRecord {
  n += 1;
  return { id: `s${n}`, kind: "focus", started_at: n * 1000, ended_at: n * 1000 + 500, ...over };
}

describe("recentIntentions", () => {
  it("lists past intentions newest first, each once, ignoring case", () => {
    const sessions = [
      session({ intention: "write report" }),
      session({ intention: "email" }),
      session({ intention: "Write Report" }),
      session({ intention: "review" }),
    ];
    expect(recentIntentions(sessions)).toEqual(["review", "Write Report", "email"]);
  });

  it("skips breaks, blanks and missing intentions", () => {
    const sessions = [
      session({ kind: "short_break", intention: "not work" }),
      session({ intention: "   " }),
      session({ intention: null }),
      session({}),
      session({ intention: " keep " }),
    ];
    expect(recentIntentions(sessions)).toEqual(["keep"]);
  });

  it("stops at the limit", () => {
    const sessions = Array.from({ length: 20 }, (_, i) => session({ intention: `task ${i}` }));
    expect(recentIntentions(sessions, 5)).toHaveLength(5);
    expect(recentIntentions(sessions, 5)[0]).toBe("task 19");
  });

  it("copes with no sessions", () => {
    expect(recentIntentions([])).toEqual([]);
  });
});

describe("recentNotes", () => {
  const sessions = [
    session({ intention: "a", notes: "first a" }),
    session({ intention: "b", notes: "only b" }),
    session({ intention: "a", notes: "second a" }),
    session({ intention: "c", notes: "   " }),
    session({ intention: "d", notes: null }),
    session({ intention: "e", notes: "latest e" }),
  ];

  it("shows the newest notes when nothing else is known", () => {
    expect(recentNotes(sessions, null, 3).map((x) => x.text)).toEqual(["latest e", "second a", "only b"]);
  });

  it("puts notes from the same intention first", () => {
    expect(recentNotes(sessions, "A", 3).map((x) => x.text)).toEqual(["second a", "first a", "latest e"]);
  });

  it("leaves out empty notes and breaks", () => {
    const withBreak = [...sessions, session({ kind: "long_break", notes: "break note" })];
    const texts = recentNotes(withBreak, null, 10).map((x) => x.text);
    expect(texts).not.toContain("break note");
    expect(texts).not.toContain("");
    expect(texts).toHaveLength(4);
  });
});

describe("notesFeed", () => {
  const sessions = [
    session({ notes: "first", category: "a" }),
    session({ notes: "  ", category: "a" }),
    session({ notes: "second", category: "b" }),
    session({ kind: "short_break", notes: "break note" }),
    session({ notes: "third", category: "a" }),
  ];

  it("lists sessions with notes, newest first", () => {
    expect(notesFeed(sessions, null).map((s) => s.notes)).toEqual(["third", "second", "first"]);
  });

  it("can be narrowed to one category and limited", () => {
    expect(notesFeed(sessions, "a").map((s) => s.notes)).toEqual(["third", "first"]);
    expect(notesFeed(sessions, null, 1)).toHaveLength(1);
  });
});
