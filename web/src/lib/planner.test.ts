import { describe, expect, it } from "vitest";
import vectors from "../../../test-vectors/planner.json";
import { idleState, plan } from "./planner";
import type { Action } from "./planner";
import type { Settings, TimerData } from "./types";

interface Step {
  at: number;
  action: Action;
  expect: { timer: TimerData; records: unknown[] } | { error: string };
}

interface Case {
  name: string;
  settings?: Partial<Settings>;
  steps: Step[];
}

const suite = vectors as unknown as { defaults: Settings; cases: Case[] };

describe("shared planner vectors", () => {
  it("has cases", () => expect(suite.cases.length).toBeGreaterThan(10));

  for (const c of suite.cases) {
    it(c.name, () => {
      const settings: Settings = { ...suite.defaults, ...c.settings };
      let state = idleState();
      c.steps.forEach((step, i) => {
        const result = plan(state, step.action, step.at, settings);
        const where = `${c.name} step ${i} (${step.action.name})`;
        if ("error" in step.expect) {
          expect(result.ok, `${where}: expected ${step.expect.error}`).toBe(false);
          if (!result.ok) expect(result.error, where).toBe(step.expect.error);
        } else {
          expect(result.ok, `${where}: ${result.ok ? "" : result.error}`).toBe(true);
          if (result.ok) {
            expect(result.next, `${where}: timer`).toEqual(step.expect.timer);
            expect(result.records, `${where}: records`).toEqual(step.expect.records);
            state = result.next;
          }
        }
      });
    });
  }
});
