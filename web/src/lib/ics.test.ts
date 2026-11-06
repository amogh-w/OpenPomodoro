import { describe, expect, it } from "vitest";
import { parseIcs, parseIcsTime } from "./ics";

const DAY = Date.UTC(2026, 10, 6);
const wrap = (...events: string[]): string => ["BEGIN:VCALENDAR", "VERSION:2.0", ...events, "END:VCALENDAR"].join("\r\n");
const event = (...lines: string[]): string => ["BEGIN:VEVENT", ...lines, "END:VEVENT"].join("\r\n");

describe("parseIcsTime", () => {
  it("reads UTC and local times and refuses dates and junk", () => {
    expect(parseIcsTime("20261106T093000Z")).toBe(Date.UTC(2026, 10, 6, 9, 30));
    expect(parseIcsTime("20261106T093000")).toBe(new Date(2026, 10, 6, 9, 30).getTime());
    for (const bad of ["20261106", "nope", "", "20261306T093000Z"]) expect(parseIcsTime(bad), bad).toBeNull();
  });
});

describe("parseIcs", () => {
  it("reads timed events with their title, in time order", () => {
    const text = wrap(
      event("UID:b", "DTSTART:20261106T140000Z", "DTEND:20261106T150000Z", "SUMMARY:Review"),
      event("UID:a", "DTSTART:20261106T090000Z", "DTEND:20261106T093000Z", "SUMMARY:Stand-up\\, daily"),
    );
    const events = parseIcs(text, DAY, DAY + 86_400_000);
    expect(events.map((e) => [e.id, e.title, (e.end - e.start) / 60_000])).toEqual([["a", "Stand-up, daily", 30], ["b", "Review", 60]]);
  });

  it("joins folded lines", () => {
    const text = wrap(event("DTSTART:20261106T090000Z", "DTEND:20261106T100000Z", "SUMMARY:A very long", "  title"));
    expect(parseIcs(text, DAY, DAY + 86_400_000)[0]?.title).toBe("A very long title");
  });

  it("leaves out all-day events, events outside the window, and gives a missing end half an hour", () => {
    const text = wrap(
      event("DTSTART;VALUE=DATE:20261106", "SUMMARY:Holiday"),
      event("DTSTART:20250101T090000Z", "DTEND:20250101T100000Z", "SUMMARY:Old"),
      event("DTSTART:20261106T120000Z", "SUMMARY:No end"),
    );
    const events = parseIcs(text, DAY, DAY + 86_400_000);
    expect(events.map((e) => e.title)).toEqual(["No end"]);
    expect((events[0]?.end ?? 0) - (events[0]?.start ?? 0)).toBe(30 * 60_000);
  });

  it("copes with parameters on the property names and with nothing at all", () => {
    const text = wrap(event("DTSTART;TZID=Europe/Berlin:20261106T120000", "DTEND;TZID=Europe/Berlin:20261106T130000", "SUMMARY:Local"));
    expect(parseIcs(text, 0, Number.MAX_SAFE_INTEGER)).toHaveLength(1);
    expect(parseIcs("", 0, 1)).toEqual([]);
    expect(parseIcs("garbage\nBEGIN:VEVENT", 0, 1)).toEqual([]);
  });
});
