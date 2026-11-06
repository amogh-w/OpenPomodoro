import { describe, expect, it } from "vitest";
import { splitTasks } from "./tasks";

describe("splitTasks", () => {
  it("keeps open tasks in the order added and puts the latest win first", () => {
    const { open, done } = splitTasks([
      { id: "b", title: "second" },
      { id: "a", title: "first", done: false },
      { id: "c", title: "old", done: true, done_at: 100 },
      { id: "d", title: "new", done: true, done_at: 900 },
      { id: "e", title: "undated", done: true },
    ]);
    expect(open.map((x) => x.title)).toEqual(["first", "second"]);
    expect(done.map((x) => x.title)).toEqual(["new", "old", "undated"]);
  });

  it("copes with nothing", () => {
    expect(splitTasks([])).toEqual({ open: [], done: [] });
  });
});
