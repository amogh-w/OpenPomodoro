import { describe, expect, it } from "vitest";
import { report } from "./diagReport";

describe("report", () => {
  it("puts the facts first, then the lines oldest first with a time", () => {
    const text = report({ version: "0.2.0", online: "yes" }, [
      { at: Date.UTC(2026, 10, 6, 10, 30, 5, 123), area: "api", message: "server unreachable" },
      { at: Date.UTC(2026, 10, 6, 10, 31, 0, 0), area: "sync", message: "sent 2" },
    ]);
    expect(text.split("\n")).toEqual(["version: 0.2.0", "online: yes", "", "10:30:05.123 api server unreachable", "10:31:00.000 sync sent 2"]);
  });

  it("says so when nothing was noted", () => {
    expect(report({}, [])).toContain("(nothing noted)");
  });
});
