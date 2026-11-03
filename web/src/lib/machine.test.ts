import { describe, expect, it } from "vitest";
import vectors from "../../../test-vectors/machine.json";
import { type EventName, type Effect, type Rejected, type State, defaultRules, transition } from "./machine";

interface Case {
  state: State;
  event: EventName;
  phase_ended?: boolean;
  rules: { breathe?: boolean; auto_start_break?: boolean; auto_start_focus?: boolean; strict?: boolean };
  expect: { state?: State; effect?: Effect | null; rejected?: Rejected };
}

const cases = (vectors as unknown as { cases: Case[] }).cases;

describe("shared machine vectors", () => {
  it("has the full set", () => expect(cases.length).toBe(792));

  it("matches every case", () => {
    cases.forEach((c, i) => {
      const rules = {
        ...defaultRules,
        breathe: c.rules.breathe ?? false,
        autoStartBreak: c.rules.auto_start_break ?? false,
        autoStartFocus: c.rules.auto_start_focus ?? false,
        strict: c.rules.strict ?? false,
      };
      const out = transition(c.state, { name: c.event, phaseEnded: c.phase_ended }, rules);
      const ctx = `case ${i}: ${c.state} + ${c.event} (ended ${String(c.phase_ended)})`;
      if (out.ok) {
        expect(c.expect.rejected, `${ctx}: expected a rejection`).toBeUndefined();
        expect(out.state, `${ctx}: state`).toBe(c.expect.state);
        expect(out.effect, `${ctx}: effect`).toBe(c.expect.effect ?? null);
      } else {
        expect(out.rejected, `${ctx}: rejection`).toBe(c.expect.rejected);
      }
    });
  });
});
