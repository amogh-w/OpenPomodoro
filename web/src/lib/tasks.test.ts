import { describe, expect, it } from "vitest";
import { orderTasks, timePerTask } from "./tasks";

describe("orderTasks", () => {
  it("keeps every task, done or not, in the order added", () => {
    const tasks = [
      { id: "b", title: "second" },
      { id: "c", title: "ticked", done: true, done_at: 100 },
      { id: "a", title: "first", done: false },
    ];
    expect(orderTasks(tasks).map((x) => x.title)).toEqual(["first", "second", "ticked"]);
  });

  it("copes with nothing", () => {
    expect(orderTasks([])).toEqual([]);
  });
});

describe("timePerTask", () => {
  const f = (task: string | null | undefined, ms: number, kind: "focus" | "short_break" = "focus") => ({
    id: String(Math.random()),
    kind,
    started_at: 0,
    ended_at: ms,
    duration_ms: ms,
    task,
  });

  it("adds up focus time per task and ignores breaks and unlinked sessions", () => {
    const t = timePerTask([f("a", 1000), f("a", 500), f("b", 200), f(null, 900), f(undefined, 900), f("a", 5000, "short_break")] as never);
    expect([...t]).toEqual([["a", 1500], ["b", 200]]);
  });

  it("falls back to the span when there is no duration", () => {
    expect(timePerTask([{ id: "x", kind: "focus", started_at: 100, ended_at: 400, task: "a" }] as never).get("a")).toBe(300);
  });
});
