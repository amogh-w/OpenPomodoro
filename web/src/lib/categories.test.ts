import { describe, expect, it } from "vitest";
import { FALLBACK_ICON, ICONS, STARTERS, iconOf, orderByRecent, timePerCategory } from "./categories";
import type { Category, SessionRecord } from "./types";

// The kit's accent names (the server checks the same twelve).
const accents = ["rosewater", "flamingo", "pink", "mauve", "red", "peach", "yellow", "green", "teal", "sky", "blue", "lavender"];

const focus = (category: string | null, started_at: number, ms = 60_000): SessionRecord => ({
  id: `s${started_at}`,
  kind: "focus",
  started_at,
  ended_at: started_at + ms,
  duration_ms: ms,
  category,
});

describe("icons", () => {
  it("falls back to a tag for none or unknown", () => {
    expect(iconOf({ icon: "book-open" })).toBe("book-open");
    expect(iconOf({ icon: "skull" })).toBe(FALLBACK_ICON);
    expect(iconOf({ icon: null })).toBe(FALLBACK_ICON);
    expect(iconOf(undefined)).toBe(FALLBACK_ICON);
  });

  it("has no duplicates, and the fallback is one of them", () => {
    expect(new Set(ICONS).size).toBe(ICONS.length);
    expect(ICONS).toContain(FALLBACK_ICON);
  });
});

describe("starters", () => {
  it("use real colours and known icons, each once", () => {
    for (const s of STARTERS) {
      expect(accents).toContain(s.color);
      expect(ICONS).toContain(s.icon);
    }
    expect(new Set(STARTERS.map((s) => s.key)).size).toBe(STARTERS.length);
  });
});

describe("orderByRecent", () => {
  const cats: Category[] = [
    { id: "a", name: "Zeta" },
    { id: "b", name: "Beta" },
    { id: "c", name: "Gamma" },
  ];

  it("puts the latest used first and the unused in the order made", () => {
    expect(orderByRecent(cats, [focus("c", 100), focus("b", 200), focus("c", 50)]).map((c) => c.id)).toEqual(["b", "c", "a"]);
  });

  it("keeps the order made with no sessions, and ignores breaks", () => {
    expect(orderByRecent(cats, []).map((c) => c.id)).toEqual(["a", "b", "c"]);
    const rest: SessionRecord = { ...focus("c", 900), kind: "short_break" };
    expect(orderByRecent(cats, [rest]).map((c) => c.id)).toEqual(["a", "b", "c"]);
  });
});

describe("timePerCategory", () => {
  it("adds focus time per category, uncategorised as null, biggest first", () => {
    const out = timePerCategory([focus("a", 0, 60_000), focus("a", 1, 60_000), focus(null, 2, 300_000), focus("b", 3, 10_000)]);
    expect(out).toEqual([
      { id: null, ms: 300_000 },
      { id: "a", ms: 120_000 },
      { id: "b", ms: 10_000 },
    ]);
  });

  it("is empty with nothing", () => {
    expect(timePerCategory([])).toEqual([]);
  });
});
