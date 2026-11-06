//! Turning a user's sessions into files other tools can read: CSV and iCalendar (ICS). Both are built here as
//! plain text so they can be tested without a server. Anything a person typed (intention, notes, category) is
//! escaped for the format, and for CSV also defused so a spreadsheet never runs it as a formula.

use std::collections::HashMap;

use op_core::{Row, ids::format_ts};
use serde_json::Value;

/// `2026-11-03T14:05:09Z` for Unix milliseconds.
pub fn iso_utc(ms: i64) -> String {
    let t = format_ts(ms.div_euclid(1000));
    format!(
        "{}-{}-{}T{}:{}:{}Z",
        &t[0..4],
        &t[4..6],
        &t[6..8],
        &t[8..10],
        &t[10..12],
        &t[12..14]
    )
}

/// `20261103T140509Z` for Unix milliseconds (the iCalendar form).
fn ics_utc(ms: i64) -> String {
    let t = format_ts(ms.div_euclid(1000));
    format!("{}T{}Z", &t[0..8], &t[8..14])
}

fn text(row: &Row, key: &str) -> Option<String> {
    row.data
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn int(row: &Row, key: &str) -> Option<i64> {
    row.data.get(key).and_then(Value::as_i64)
}

/// Sessions oldest first (ties by id), which is the order every export uses.
pub fn sorted(sessions: &[(String, Row)]) -> Vec<&(String, Row)> {
    let mut list: Vec<_> = sessions.iter().collect();
    list.sort_by(|a, b| {
        int(&a.1, "started_at")
            .cmp(&int(&b.1, "started_at"))
            .then_with(|| a.0.cmp(&b.0))
    });
    list
}

/// Names for the ids a session points at, so an export reads in words.
#[derive(Default)]
pub struct Names {
    pub categories: HashMap<String, String>,
    pub tasks: HashMap<String, String>,
}

/// A category's name from its id; an id that no longer exists is shown as it is.
fn category_name(row: &Row, names: &Names) -> Option<String> {
    let id = text(row, "category")?;
    Some(names.categories.get(&id).cloned().unwrap_or(id))
}

/// The title of the task a session worked on; a deleted task is shown by its id.
fn task_title(row: &Row, names: &Names) -> Option<String> {
    let id = text(row, "task")?;
    Some(names.tasks.get(&id).cloned().unwrap_or(id))
}

// ---- CSV ----

/// One CSV cell. Text is quoted when it needs to be, and a leading `=`, `+`, `-`, `@`, tab or carriage return is
/// neutralised with a quote mark, so opening the file in a spreadsheet cannot execute what someone typed.
fn csv_cell(value: &str) -> String {
    let defused = if value.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{value}")
    } else {
        value.to_string()
    };
    if defused.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", defused.replace('"', "\"\""))
    } else {
        defused
    }
}

pub fn csv(sessions: &[(String, Row)], names: &Names) -> String {
    let mut out = String::from(
        "id,kind,started_at,ended_at,duration_seconds,planned_seconds,intention,category,task,notes,mood\r\n",
    );
    for (id, row) in sorted(sessions) {
        let number = |key: &str, scale: i64| {
            int(row, key)
                .map(|v| (v / scale).to_string())
                .unwrap_or_default()
        };
        let time = |key: &str| int(row, key).map(iso_utc).unwrap_or_default();
        let cells = [
            csv_cell(id),
            csv_cell(&text(row, "kind").unwrap_or_default()),
            time("started_at"),
            time("ended_at"),
            number("duration_ms", 1000),
            number("planned_ms", 1000),
            csv_cell(&text(row, "intention").unwrap_or_default()),
            csv_cell(&category_name(row, names).unwrap_or_default()),
            csv_cell(&task_title(row, names).unwrap_or_default()),
            csv_cell(&text(row, "notes").unwrap_or_default()),
            csv_cell(&text(row, "mood").unwrap_or_default()),
        ];
        out.push_str(&cells.join(","));
        out.push_str("\r\n");
    }
    out
}

// ---- ICS ----

fn ics_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace("\r\n", "\\n")
        .replace(['\n', '\r'], "\\n")
}

/// Lines in iCalendar are at most 75 octets; longer ones continue on the next line after a space. Never splits
/// a multi-byte character.
fn fold(line: &str) -> String {
    const MAX_OCTETS: usize = 75;
    let mut out = String::new();
    // Octets on the current physical line. A continuation line starts with one space, which counts.
    let mut octets = 0;
    for ch in line.chars() {
        let width = ch.len_utf8();
        if octets + width > MAX_OCTETS {
            out.push_str("\r\n ");
            octets = 1;
        }
        out.push(ch);
        octets += width;
    }
    out
}

fn push(out: &mut String, line: &str) {
    out.push_str(&fold(line));
    out.push_str("\r\n");
}

/// Sessions as calendar events, so they can be laid over a calendar. `now_ms` stamps the file.
pub fn ics(sessions: &[(String, Row)], names: &Names, now_ms: i64) -> String {
    let mut out = String::new();
    for line in [
        "BEGIN:VCALENDAR",
        "VERSION:2.0",
        "PRODID:-//OpenPomodoro//Sessions//EN",
        "CALSCALE:GREGORIAN",
    ] {
        push(&mut out, line);
    }
    for (id, row) in sorted(sessions) {
        let (Some(start), Some(end)) = (int(row, "started_at"), int(row, "ended_at")) else {
            continue;
        };
        let kind = text(row, "kind").unwrap_or_default();
        let summary = text(row, "intention").unwrap_or_else(|| {
            match kind.as_str() {
                "short_break" => "Short break",
                "long_break" => "Long break",
                _ => "Focus",
            }
            .to_string()
        });
        push(&mut out, "BEGIN:VEVENT");
        push(&mut out, &format!("UID:{}", ics_escape(id)));
        push(&mut out, &format!("DTSTAMP:{}", ics_utc(now_ms)));
        push(&mut out, &format!("DTSTART:{}", ics_utc(start)));
        push(&mut out, &format!("DTEND:{}", ics_utc(end.max(start))));
        push(&mut out, &format!("SUMMARY:{}", ics_escape(&summary)));
        if let Some(name) = category_name(row, names) {
            push(&mut out, &format!("CATEGORIES:{}", ics_escape(&name)));
        }
        let mut description = text(row, "notes").unwrap_or_default();
        if let Some(title) = task_title(row, names) {
            if !description.is_empty() {
                description.push('\n');
            }
            description.push_str(&format!("Task: {title}"));
        }
        if let Some(mood) = text(row, "mood") {
            if !description.is_empty() {
                description.push('\n');
            }
            description.push_str(&format!("Mood: {mood}"));
        }
        if !description.is_empty() {
            push(
                &mut out,
                &format!("DESCRIPTION:{}", ics_escape(&description)),
            );
        }
        push(&mut out, "END:VEVENT");
    }
    push(&mut out, "END:VCALENDAR");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn session(id: &str, data: Value) -> (String, Row) {
        let Value::Object(map) = data else {
            panic!("object")
        };
        (
            id.to_string(),
            Row {
                data: map,
                ..Row::default()
            },
        )
    }

    fn cats() -> Names {
        Names {
            categories: HashMap::from([("c1".to_string(), "Deep work".to_string())]),
            tasks: HashMap::from([("t1".to_string(), "Write the report".to_string())]),
        }
    }

    #[test]
    fn iso_and_ics_times_are_utc() {
        assert_eq!(iso_utc(1_000_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(ics_utc(1_000_000_000_000), "20010909T014640Z");
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn csv_has_a_header_and_one_row_per_session_oldest_first() {
        let sessions = vec![
            session(
                "b",
                json!({"kind": "short_break", "started_at": 2_000_000, "ended_at": 2_300_000, "duration_ms": 300_000, "planned_ms": 300_000}),
            ),
            session(
                "a",
                json!({"kind": "focus", "started_at": 1_000_000, "ended_at": 1_600_000, "duration_ms": 600_000, "planned_ms": 1_500_000, "intention": "write", "category": "c1", "task": "t1", "notes": "ok", "mood": "focused"}),
            ),
        ];
        let text = csv(&sessions, &cats());
        let lines: Vec<&str> = text.split("\r\n").collect();
        assert_eq!(
            lines[0],
            "id,kind,started_at,ended_at,duration_seconds,planned_seconds,intention,category,task,notes,mood"
        );
        assert_eq!(
            lines[1],
            "a,focus,1970-01-01T00:16:40Z,1970-01-01T00:26:40Z,600,1500,write,Deep work,Write the report,ok,focused"
        );
        assert_eq!(
            lines[2],
            "b,short_break,1970-01-01T00:33:20Z,1970-01-01T00:38:20Z,300,300,,,,,"
        );
        assert_eq!(lines[3], "", "ends with a line break");
    }

    #[test]
    fn csv_quotes_commas_quotes_and_line_breaks() {
        let s = vec![session(
            "a",
            json!({"kind": "focus", "started_at": 0, "ended_at": 1000, "intention": "plan, then \"do\"", "notes": "line one\nline two"}),
        )];
        let text = csv(&s, &cats());
        assert!(text.contains("\"plan, then \"\"do\"\"\""), "{text}");
        assert!(text.contains("\"line one\nline two\""), "{text}");
    }

    #[test]
    fn csv_defuses_spreadsheet_formulas() {
        for (typed, shown) in [
            ("=1+1", "'=1+1"),
            ("+cmd", "'+cmd"),
            ("-5", "'-5"),
            ("@SUM(A1)", "'@SUM(A1)"),
            ("\tx", "'\tx"),
        ] {
            assert_eq!(csv_cell(typed), shown, "{typed:?}");
        }
        assert_eq!(
            csv_cell("a=b"),
            "a=b",
            "only a leading character is dangerous"
        );
        let s = vec![session(
            "a",
            json!({"kind": "focus", "started_at": 0, "ended_at": 1, "intention": "=HYPERLINK(\"http://evil\")"}),
        )];
        assert!(!csv(&s, &cats()).contains(",=HYPERLINK"));
    }

    #[test]
    fn an_unknown_category_id_is_shown_as_it_is() {
        let s = vec![session(
            "a",
            json!({"kind": "focus", "started_at": 0, "ended_at": 1, "category": "gone"}),
        )];
        assert!(csv(&s, &cats()).contains(",gone,"));
    }

    #[test]
    fn ics_is_a_calendar_with_events() {
        let sessions = vec![session(
            "a@dev",
            json!({"kind": "focus", "started_at": 1_000_000_000_000_i64, "ended_at": 1_000_001_500_000_i64, "intention": "write", "category": "c1", "notes": "good", "mood": "focused"}),
        )];
        let text = ics(&sessions, &cats(), 1_700_000_000_000);
        for line in [
            "BEGIN:VCALENDAR",
            "VERSION:2.0",
            "BEGIN:VEVENT",
            "UID:a@dev",
            "DTSTAMP:20231114T221320Z",
            "DTSTART:20010909T014640Z",
            "DTEND:20010909T021140Z",
            "SUMMARY:write",
            "CATEGORIES:Deep work",
            "DESCRIPTION:good\\nMood: focused",
            "END:VEVENT",
            "END:VCALENDAR",
        ] {
            assert!(text.lines().any(|l| l == line), "missing {line}\n{text}");
        }
        assert!(text.ends_with("END:VCALENDAR\r\n"));
        assert!(
            text.lines().all(|l| !l.is_empty())
                && text.matches("\r\n").count() == text.lines().count()
        );
    }

    #[test]
    fn ics_escapes_what_people_type() {
        let s = vec![session(
            "a",
            json!({"kind": "focus", "started_at": 0, "ended_at": 1000, "intention": "a;b,c\\d", "notes": "x\ny"}),
        )];
        let text = ics(&s, &cats(), 0);
        assert!(text.contains("SUMMARY:a\\;b\\,c\\\\d"), "{text}");
        assert!(text.contains("DESCRIPTION:x\\ny"), "{text}");
    }

    #[test]
    fn ics_names_breaks_and_unlabelled_focus() {
        let s = vec![
            session(
                "a",
                json!({"kind": "long_break", "started_at": 0, "ended_at": 1000}),
            ),
            session(
                "b",
                json!({"kind": "focus", "started_at": 5000, "ended_at": 6000}),
            ),
        ];
        let text = ics(&s, &cats(), 0);
        assert!(text.contains("SUMMARY:Long break") && text.contains("SUMMARY:Focus"));
    }

    #[test]
    fn ics_skips_sessions_without_times() {
        let s = vec![session("a", json!({"kind": "focus", "started_at": 0}))];
        assert!(!ics(&s, &cats(), 0).contains("BEGIN:VEVENT"));
    }

    #[test]
    fn ics_folds_long_lines_without_splitting_characters() {
        let long = "é".repeat(100); // 200 octets
        let s = vec![session(
            "a",
            json!({"kind": "focus", "started_at": 0, "ended_at": 1000, "intention": long}),
        )];
        let text = ics(&s, &cats(), 0);
        for line in text.split("\r\n") {
            assert!(line.len() <= 75, "{} octets: {line}", line.len());
        }
        // Unfolding gives the original back.
        let unfolded = text.replace("\r\n ", "");
        assert!(unfolded.contains(&format!("SUMMARY:{}", "é".repeat(100))));
    }
}
