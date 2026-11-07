import { describe, expect, it } from "vitest";
import { atFragment, categoryMatches, completeAt, parseIntention, stripCategory, stripDuration } from "./intention";
import type { Category } from "./types";

const cats: Category[] = [
  { id: "c1", name: "Deep work" },
  { id: "c2", name: "Reading" },
  { id: "c3", name: "Review" },
];

describe("parseIntention", () => {
  it("leaves plain text alone", () => {
    expect(parseIntention("write the report", cats)).toEqual({ text: "write the report", durationMs: null, categoryId: null });
  });

  it("reads durations and removes them from the text", () => {
    expect(parseIntention("write 30m report", cats).durationMs).toBe(30 * 60_000);
    expect(parseIntention("write 30m report", cats).text).toBe("write report");
    expect(parseIntention("1h", cats).durationMs).toBe(60 * 60_000);
    expect(parseIntention("plan 1h30m", cats).durationMs).toBe(90 * 60_000);
    expect(parseIntention("plan 90m", cats).durationMs).toBe(90 * 60_000);
  });

  it("does not mistake ordinary words for durations", () => {
    for (const word of ["m", "h", "0m", "5x", "12", "calm", "1hm"]) {
      expect(parseIntention(`do ${word}`, cats).durationMs, word).toBeNull();
    }
  });

  it("the last duration wins", () => {
    expect(parseIntention("20m then 40m", cats).durationMs).toBe(40 * 60_000);
  });

  it("finds a category by exact name, ignoring case and spaces", () => {
    expect(parseIntention("essay @deepwork", cats)).toEqual({ text: "essay", durationMs: null, categoryId: "c1" });
    expect(parseIntention("@READING novel", cats).categoryId).toBe("c2");
  });

  it("accepts a unique prefix but not an ambiguous one", () => {
    expect(parseIntention("x @dee", cats).categoryId).toBe("c1");
    expect(parseIntention("x @re", cats).categoryId).toBeNull();
    expect(parseIntention("x @re", cats).text).toBe("x @re");
  });

  it("keeps an unknown @tag in the text", () => {
    expect(parseIntention("email @bob", cats)).toEqual({ text: "email @bob", durationMs: null, categoryId: null });
    expect(parseIntention("a @ b", cats).text).toBe("a @ b");
  });

  it("combines everything and tidies spacing", () => {
    expect(parseIntention("  draft   25m   intro @reading ", cats)).toEqual({
      text: "draft intro",
      durationMs: 25 * 60_000,
      categoryId: "c2",
    });
  });

  it("copes with empty input", () => {
    expect(parseIntention("", cats)).toEqual({ text: "", durationMs: null, categoryId: null });
  });
});

describe("editing the shortcuts away", () => {
  it("strips a length but keeps the words", () => {
    expect(stripDuration("write 30m report")).toBe("write report");
    expect(stripDuration("plan 1h30m @reading")).toBe("plan @reading");
    expect(stripDuration("calm 5x")).toBe("calm 5x");
  });

  it("strips a known @category but keeps an unknown one", () => {
    expect(stripCategory("essay @deepwork 30m", cats)).toBe("essay 30m");
    expect(stripCategory("email @bob", cats)).toBe("email @bob");
  });
});

describe("completing an @word", () => {
  it("finds the word being typed at the end", () => {
    expect(atFragment("essay @de")).toBe("de");
    expect(atFragment("essay @")).toBe("");
    expect(atFragment("@x done")).toBeNull();
    expect(atFragment("essay")).toBeNull();
    expect(atFragment("mail me@home")).toBeNull();
  });

  it("lists categories by prefix and finishes the word", () => {
    expect(categoryMatches("re", cats).map((c) => c.id)).toEqual(["c2", "c3"]);
    expect(categoryMatches("", cats)).toHaveLength(3);
    expect(categoryMatches("zzz", cats)).toEqual([]);
    expect(completeAt("essay @de", "Deep work")).toBe("essay @Deepwork ");
    expect(parseIntention(completeAt("essay @de", "Deep work"), cats).categoryId).toBe("c1");
  });
});
