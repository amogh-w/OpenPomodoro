// The 52-week heatmap: one cell per day, a column per week (Monday first), shaded by how much focus the day had.
import { localDayKey } from "./days";
import type { DayTotal } from "./types";

export interface HeatCell {
  /** Local `YYYY-MM-DD`. */
  day: string;
  focusMs: number;
  /** 0 for no focus, then 1 (lightest) to 4 (strongest). */
  level: 0 | 1 | 2 | 3 | 4;
  /** After today: drawn as an empty slot, never shaded. */
  future: boolean;
}

export const HEATMAP_WEEKS = 53;

/** Cut points between levels 1..4, from the days that have any focus, so the shading adapts to the person. */
export function levelCuts(values: readonly number[]): [number, number, number] {
  const sorted = values.filter((v) => v > 0).sort((a, b) => a - b);
  if (sorted.length === 0) return [0, 0, 0];
  const at = (p: number): number => sorted[Math.max(0, Math.ceil(p * sorted.length) - 1)] ?? 0;
  return [at(0.25), at(0.5), at(0.75)];
}

function levelOf(ms: number, cuts: readonly number[]): HeatCell["level"] {
  if (ms <= 0) return 0;
  if (ms <= (cuts[0] ?? 0)) return 1;
  if (ms <= (cuts[1] ?? 0)) return 2;
  if (ms <= (cuts[2] ?? 0)) return 3;
  return 4;
}

/**
 * The weeks to draw, oldest first, ending with the week that holds `todayKey`. Each week has seven cells, Monday to
 * Sunday; days after today are marked `future`.
 */
export function buildHeatmap(totals: readonly DayTotal[], todayKey: string, weeks = HEATMAP_WEEKS): HeatCell[][] {
  const focus = new Map(totals.map((d) => [d.day, d.focus_ms]));
  const [y, m, d] = todayKey.split("-").map(Number) as [number, number, number];
  const today = new Date(y, m - 1, d);
  const mondayOffset = (today.getDay() + 6) % 7;
  const firstDay = new Date(y, m - 1, d - mondayOffset - (weeks - 1) * 7);
  const out: HeatCell[][] = [];
  const shown = new Set<string>();
  const dates: Date[][] = [];
  for (let w = 0; w < weeks; w++) {
    dates.push(Array.from({ length: 7 }, (_, i) => new Date(firstDay.getFullYear(), firstDay.getMonth(), firstDay.getDate() + w * 7 + i)));
  }
  for (const date of dates.flat()) shown.add(localDayKey(date.getTime()));
  // The shading adapts to the days that are drawn (not older history, not days after today).
  const cuts = levelCuts([...focus].filter(([day]) => shown.has(day) && day <= todayKey).map(([, ms]) => ms));
  for (let w = 0; w < weeks; w++) {
    const column: HeatCell[] = [];
    for (let i = 0; i < 7; i++) {
      const date = dates[w]?.[i] ?? firstDay;
      const day = localDayKey(date.getTime());
      const future = day > todayKey;
      const ms = future ? 0 : (focus.get(day) ?? 0);
      column.push({ day, focusMs: ms, level: levelOf(ms, cuts), future });
    }
    out.push(column);
  }
  return out;
}
