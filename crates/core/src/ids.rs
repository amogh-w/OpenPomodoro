//! String-sortable ids: `<UTC yyyyMMddHHmmss>.<5-digit seq>@<device>` (D-02).
//!
//! Per device they are strictly increasing: a repeat of the same second, or a clock set back, bumps the
//! sequence instead of the time. Across devices they order by wall clock only; they do not prove causality.

/// Highest sequence number inside one second; past it the generator moves to the next second.
const MAX_SEQ: u32 = 99_999;
/// 9999-12-31T23:59:59Z. Later times are clamped so the timestamp stays 14 digits.
const MAX_SECS: i64 = 253_402_300_799;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedId<'a> {
    pub secs: i64,
    pub seq: u32,
    pub device: &'a str,
}

/// Days since 1970-01-01 for a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Civil date for days since 1970-01-01.
fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// `yyyyMMddHHmmss` in UTC for Unix seconds (clamped to the representable range).
pub fn format_ts(secs: i64) -> String {
    let secs = secs.clamp(0, MAX_SECS);
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{y:04}{m:02}{d:02}{:02}{:02}{:02}",
        rem / 3_600,
        rem % 3_600 / 60,
        rem % 60
    )
}

fn parse_ts(ts: &str) -> Option<i64> {
    if ts.len() != 14 || !ts.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = |range: std::ops::Range<usize>| ts[range].parse::<i64>().ok();
    let (y, mo, d, h, mi, s) = (
        n(0..4)?,
        n(4..6)?,
        n(6..8)?,
        n(8..10)?,
        n(10..12)?,
        n(12..14)?,
    );
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return None;
    }
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3_600 + mi * 60 + s)
}

pub fn parse(id: &str) -> Option<ParsedId<'_>> {
    let (left, device) = id.split_once('@')?;
    let (ts, seq) = left.split_once('.')?;
    if device.is_empty() || seq.len() != 5 || !seq.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(ParsedId {
        secs: parse_ts(ts)?,
        seq: seq.parse().ok()?,
        device,
    })
}

#[derive(Debug, Clone)]
pub struct IdGen {
    device: String,
    last: Option<(i64, u32)>,
}

impl IdGen {
    pub fn new(device: &str) -> Self {
        Self {
            device: device.to_string(),
            last: None,
        }
    }

    /// Continue after the highest id this device already issued. Ids from other devices are ignored, so a
    /// device with a fast clock cannot drag this one forward; unparseable ids are skipped.
    pub fn restore<'a>(device: &str, ids: impl IntoIterator<Item = &'a str>) -> Self {
        let last = ids
            .into_iter()
            .filter_map(parse)
            .filter(|p| p.device == device)
            .map(|p| (p.secs, p.seq))
            .max();
        Self {
            device: device.to_string(),
            last,
        }
    }

    pub fn next(&mut self, now_ms: i64) -> String {
        let now = now_ms.div_euclid(1_000).clamp(0, MAX_SECS);
        let (secs, seq) = match self.last {
            Some((ls, _)) if now > ls => (now, 0),
            Some((ls, sq)) if sq < MAX_SEQ => (ls, sq + 1),
            Some((ls, _)) => (ls + 1, 0),
            None => (now, 0),
        };
        self.last = Some((secs, seq));
        format!("{}.{seq:05}@{}", format_ts(secs), self.device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_known_instants() {
        assert_eq!(format_ts(0), "19700101000000");
        assert_eq!(format_ts(1_000_000_000), "20010909014640");
        assert_eq!(format_ts(951_782_400), "20000229000000", "leap day");
        assert_eq!(
            format_ts(-5),
            "19700101000000",
            "negative clamps to the epoch"
        );
        assert_eq!(format_ts(i64::MAX), "99991231235959");
    }

    #[test]
    fn parse_round_trips_format() {
        for secs in [0, 59, 86_399, 951_782_400, 1_793_725_000, MAX_SECS] {
            let id = format!("{}.00042@dev", format_ts(secs));
            assert_eq!(
                parse(&id),
                Some(ParsedId {
                    secs,
                    seq: 42,
                    device: "dev"
                })
            );
        }
    }

    #[test]
    fn rejects_malformed_ids() {
        for bad in [
            "",
            "x",
            "20260101000000.00001",
            "20260101000000@d",
            "20260101000000.1@d",
            "20261301000000.00001@d",
            "20260101000000.00001@",
        ] {
            assert_eq!(parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn strictly_increasing_even_when_the_clock_stalls_or_goes_back() {
        let mut id_gen = IdGen::new("d");
        let times = [5_000, 5_000, 5_500, 4_000, 9_000, 1_000, 9_999, 10_000];
        let ids: Vec<_> = times.iter().map(|&t| id_gen.next(t)).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]), "{ids:?}");
    }

    #[test]
    fn seq_overflow_moves_to_the_next_second() {
        let last = format!("{}.{MAX_SEQ:05}@d", format_ts(100));
        let mut id_gen = IdGen::restore("d", [last.as_str()]);
        let id = id_gen.next(100_000);
        assert_eq!(id, format!("{}.00000@d", format_ts(101)));
    }

    #[test]
    fn restore_ignores_other_devices_and_junk() {
        let mine = format!("{}.00003@me", format_ts(500));
        let theirs = format!("{}.00000@them", format_ts(9_999_999));
        let mut id_gen = IdGen::restore("me", [theirs.as_str(), "junk", mine.as_str()]);
        assert_eq!(id_gen.next(0), format!("{}.00004@me", format_ts(500)));
    }

    #[test]
    fn ids_sort_as_strings_in_time_order() {
        let mut id_gen = IdGen::new("d");
        let a = id_gen.next(1_000_000_000_000);
        let b = id_gen.next(1_793_725_000_000);
        assert!(a < b);
    }
}
