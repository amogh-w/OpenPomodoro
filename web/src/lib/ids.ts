// TypeScript port of crates/core/src/ids.rs. The Rust code is the source of truth; both pass
// test-vectors/ids.json. Format: `<UTC yyyyMMddHHmmss>.<5-digit seq>@<device>`. Strictly increasing per device.

const MAX_SEQ = 99_999;
/** 9999-12-31T23:59:59Z. Later times are clamped so the timestamp stays 14 digits. */
const MAX_SECS = 253_402_300_799;

const pad = (n: number, width: number): string => String(n).padStart(width, "0");

/** `yyyyMMddHHmmss` in UTC for Unix seconds (clamped to the representable range). */
export function formatTs(secs: number): string {
  const d = new Date(Math.min(Math.max(secs, 0), MAX_SECS) * 1000);
  return (
    pad(d.getUTCFullYear(), 4) +
    pad(d.getUTCMonth() + 1, 2) +
    pad(d.getUTCDate(), 2) +
    pad(d.getUTCHours(), 2) +
    pad(d.getUTCMinutes(), 2) +
    pad(d.getUTCSeconds(), 2)
  );
}

export interface ParsedId {
  secs: number;
  seq: number;
  device: string;
}

export function parseId(id: string): ParsedId | null {
  const m = /^(\d{4})(\d{2})(\d{2})(\d{2})(\d{2})(\d{2})\.(\d{5})@(.+)$/.exec(id);
  if (!m) return null;
  const [year, mo, d, h, mi, s] = m.slice(1, 7).map(Number) as [number, number, number, number, number, number];
  if (mo < 1 || mo > 12 || d < 1 || d > 31 || h > 23 || mi > 59 || s > 59) return null;
  return { secs: Date.UTC(year, mo - 1, d, h, mi, s) / 1000, seq: Number(m[7]), device: m[8] as string };
}

export class IdGen {
  private last: { secs: number; seq: number } | null;

  constructor(
    private readonly device: string,
    last: { secs: number; seq: number } | null = null,
  ) {
    this.last = last;
  }

  /**
   * Continue after the highest id this device already issued. Ids from other devices are ignored, so a
   * device with a fast clock cannot drag this one forward; unparseable ids are skipped.
   */
  static restore(device: string, ids: Iterable<string>): IdGen {
    let best: { secs: number; seq: number } | null = null;
    for (const id of ids) {
      const p = parseId(id);
      if (!p || p.device !== device) continue;
      if (best === null || p.secs > best.secs || (p.secs === best.secs && p.seq > best.seq)) {
        best = { secs: p.secs, seq: p.seq };
      }
    }
    return new IdGen(device, best);
  }

  next(nowMs: number): string {
    const now = Math.min(Math.max(Math.floor(nowMs / 1000), 0), MAX_SECS);
    let secs = now;
    let seq = 0;
    if (this.last !== null) {
      if (now > this.last.secs) {
        // new second
      } else if (this.last.seq < MAX_SEQ) {
        secs = this.last.secs;
        seq = this.last.seq + 1;
      } else {
        secs = this.last.secs + 1;
      }
    }
    this.last = { secs, seq };
    return `${formatTs(secs)}.${pad(seq, 5)}@${this.device}`;
  }
}
