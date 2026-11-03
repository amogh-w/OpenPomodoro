//! A user's settings: names, defaults, validation, and the effective values the timer runs on.

use op_core::config::{MAX_PHASE_MS, MAX_ROUNDS, MIN_PHASE_MS};
use op_core::{Config, Rules};
use serde_json::{Value, json};

use crate::db::{Db, DbError};

/// Length of the optional breathing step. The bounds and default are guesses.
pub const DEFAULT_BREATHE_MS: i64 = 30_000;
const BREATHE_RANGE: std::ops::RangeInclusive<i64> = 5_000..=300_000;

/// Every setting the server accepts. Unknown names are refused so the table cannot be filled with junk.
pub const NAMES: [&str; 9] = [
    "focus_ms",
    "short_break_ms",
    "long_break_ms",
    "rounds_before_long",
    "breathe",
    "breathe_ms",
    "auto_start_break",
    "auto_start_focus",
    "strict",
];

pub struct Settings {
    pub config: Config,
    pub rules: Rules,
    pub breathe_ms: i64,
}

fn int_in(value: &Value, range: std::ops::RangeInclusive<i64>) -> bool {
    value.as_i64().is_some_and(|n| range.contains(&n))
}

/// Is `value` acceptable for the setting `name`?
pub fn valid_value(name: &str, value: &Value) -> bool {
    match name {
        "focus_ms" | "short_break_ms" | "long_break_ms" => {
            int_in(value, MIN_PHASE_MS..=MAX_PHASE_MS)
        }
        "rounds_before_long" => int_in(value, 1..=i64::from(MAX_ROUNDS)),
        "breathe_ms" => int_in(value, BREATHE_RANGE),
        "breathe" | "auto_start_break" | "auto_start_focus" | "strict" => value.is_boolean(),
        _ => false,
    }
}

impl Settings {
    pub fn load(db: &Db, user: &str) -> Result<Settings, DbError> {
        let defaults = Config::default();
        let int = |name: &str, default: i64| -> Result<i64, DbError> {
            Ok(db
                .setting(user, name)?
                .filter(|v| valid_value(name, v))
                .and_then(|v| v.as_i64())
                .unwrap_or(default))
        };
        let flag = |name: &str| -> Result<bool, DbError> {
            Ok(db
                .setting(user, name)?
                .and_then(|v| v.as_bool())
                .unwrap_or(false))
        };
        let config = Config::new(
            int("focus_ms", defaults.focus_ms())?,
            int(
                "short_break_ms",
                defaults.break_ms(op_core::BreakKind::Short),
            )?,
            int("long_break_ms", defaults.break_ms(op_core::BreakKind::Long))?,
            u32::try_from(int(
                "rounds_before_long",
                i64::from(defaults.rounds_before_long()),
            )?)
            .unwrap_or(4),
        )
        .unwrap_or(defaults);
        Ok(Settings {
            config,
            rules: Rules {
                breathe: flag("breathe")?,
                auto_start_break: flag("auto_start_break")?,
                auto_start_focus: flag("auto_start_focus")?,
                strict: flag("strict")?,
            },
            breathe_ms: int("breathe_ms", DEFAULT_BREATHE_MS)?,
        })
    }

    /// Every setting with its effective value (stored or default), for the client.
    pub fn to_json(&self) -> Value {
        json!({
            "focus_ms": self.config.focus_ms(),
            "short_break_ms": self.config.break_ms(op_core::BreakKind::Short),
            "long_break_ms": self.config.break_ms(op_core::BreakKind::Long),
            "rounds_before_long": self.config.rounds_before_long(),
            "breathe": self.rules.breathe,
            "breathe_ms": self.breathe_ms,
            "auto_start_break": self.rules.auto_start_break,
            "auto_start_focus": self.rules.auto_start_focus,
            "strict": self.rules.strict,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Role, User};
    use op_core::{Entity, EventRecord, Op};

    fn db_with_user() -> Db {
        let db = Db::open_in_memory().expect("db");
        db.insert_user(&User {
            id: "u".into(),
            username: "u".into(),
            password_hash: "h".into(),
            role: Role::User,
            disabled: false,
            created_at: 0,
        })
        .expect("user");
        db
    }

    fn set(db: &mut Db, id: &str, name: &str, value: Value) {
        let ev = EventRecord {
            id: id.into(),
            entity: Entity::Setting,
            op: Op::Add,
            key: name.into(),
            payload: json!({"value": value}),
        };
        db.apply_event("u", &ev).expect("set");
    }

    #[test]
    fn defaults_when_nothing_is_stored() {
        let db = db_with_user();
        let s = Settings::load(&db, "u").expect("load");
        assert_eq!(s.config, Config::default());
        assert_eq!(s.rules, Rules::default());
        assert_eq!(s.breathe_ms, DEFAULT_BREATHE_MS);
        assert_eq!(s.to_json()["focus_ms"], 25 * 60_000);
    }

    #[test]
    fn stored_values_override_defaults() {
        let mut db = db_with_user();
        set(&mut db, "1", "focus_ms", json!(50 * 60_000));
        set(&mut db, "2", "rounds_before_long", json!(3));
        set(&mut db, "3", "strict", json!(true));
        set(&mut db, "4", "breathe", json!(true));
        let s = Settings::load(&db, "u").expect("load");
        assert_eq!(s.config.focus_ms(), 50 * 60_000);
        assert_eq!(s.config.rounds_before_long(), 3);
        assert!(s.rules.strict && s.rules.breathe && !s.rules.auto_start_break);
    }

    #[test]
    fn an_invalid_stored_value_falls_back_to_the_default() {
        let mut db = db_with_user();
        set(&mut db, "1", "focus_ms", json!(5));
        set(&mut db, "2", "breathe_ms", json!("soon"));
        let s = Settings::load(&db, "u").expect("load");
        assert_eq!(s.config.focus_ms(), Config::default().focus_ms());
        assert_eq!(s.breathe_ms, DEFAULT_BREATHE_MS);
    }

    #[test]
    fn validation_rules() {
        assert!(valid_value("focus_ms", &json!(MIN_PHASE_MS)));
        assert!(!valid_value("focus_ms", &json!(MIN_PHASE_MS - 1)));
        assert!(!valid_value("focus_ms", &json!(1.5)));
        assert!(!valid_value("focus_ms", &json!("25")));
        assert!(valid_value("rounds_before_long", &json!(MAX_ROUNDS)));
        assert!(!valid_value("rounds_before_long", &json!(0)));
        assert!(valid_value("strict", &json!(false)));
        assert!(!valid_value("strict", &json!(0)));
        assert!(!valid_value("made_up", &json!(true)));
        for name in NAMES {
            assert!(
                valid_value(
                    name,
                    &Settings::load(&db_with_user(), "u")
                        .expect("load")
                        .to_json()[name]
                ),
                "{name}: the effective default must itself be valid"
            );
        }
    }
}
