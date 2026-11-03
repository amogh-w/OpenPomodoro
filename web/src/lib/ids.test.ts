import { describe, expect, it } from "vitest";
import vectors from "../../../test-vectors/ids.json";
import { IdGen, formatTs, parseId } from "./ids";

type Op =
  | { op: "new"; device: string }
  | { op: "restore"; device: string; ids: string[] }
  | { op: "next"; at: number; expect: string };

const cases = (vectors as unknown as { cases: { name: string; ops: Op[] }[] }).cases;

describe("shared id vectors", () => {
  for (const c of cases) {
    it(c.name, () => {
      let idGen: IdGen | null = null;
      c.ops.forEach((op, i) => {
        if (op.op === "new") idGen = new IdGen(op.device);
        else if (op.op === "restore") idGen = IdGen.restore(op.device, op.ids);
        else {
          if (idGen === null) throw new Error("generator not created");
          expect((idGen as IdGen).next(op.at), `${c.name} op ${i}`).toBe(op.expect);
        }
      });
    });
  }
});

describe("ids", () => {
  it("formats known instants", () => {
    expect(formatTs(0)).toBe("19700101000000");
    expect(formatTs(1_000_000_000)).toBe("20010909014640");
    expect(formatTs(-5)).toBe("19700101000000");
  });

  it("parses what it formats and rejects junk", () => {
    expect(parseId(`${formatTs(951_782_400)}.00042@dev`)).toEqual({ secs: 951_782_400, seq: 42, device: "dev" });
    for (const bad of ["", "x", "20260101000000.00001", "20261301000000.00001@d", "20260101000000.1@d"]) {
      expect(parseId(bad), bad).toBeNull();
    }
  });

  it("is strictly increasing when the clock stalls or goes back", () => {
    const g = new IdGen("d");
    const ids = [5_000, 5_000, 5_500, 4_000, 9_000, 1_000, 10_000].map((t) => g.next(t));
    for (let i = 1; i < ids.length; i++) expect((ids[i - 1] as string) < (ids[i] as string)).toBe(true);
  });
});
