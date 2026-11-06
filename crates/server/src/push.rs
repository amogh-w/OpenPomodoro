//! Alerts while the app is closed (A-07): the browser registers a push subscription, and this module sends a short
//! message through the browser's push service when a phase ends or the daily reminder is due. It only speaks up when
//! the person has no app open (an open app alerts for itself), and it keeps to the device's quiet hours.

use std::collections::HashSet;
use std::time::Duration;

use op_core::{State, TimerState};
use p256::{SecretKey, ecdsa::SigningKey};
use serde::{Deserialize, Serialize};
use sha2::Digest;
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::DefaultConnector;

use crate::db::{Db, PushSub};
use crate::state::Shared;
use crate::webhooks::PublicOnly;
use crate::webpush;

/// Push services run by the browser makers. Anything else is refused, so a subscription cannot point this server at
/// an arbitrary address.
const HOST_SUFFIXES: [&str; 5] = [
    "fcm.googleapis.com",
    ".push.services.mozilla.com",
    ".push.apple.com",
    ".notify.windows.com",
    "web.push.apple.com",
];

/// A phase that ended longer ago than this is not announced (the server was down, or the person has been back since).
const PHASE_FRESH_MS: i64 = 90_000;
/// How long after its time the daily reminder may still be sent.
const DAILY_GRACE_MIN: i64 = 10;
const LOOP_EVERY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Texts {
    pub focus_title: String,
    pub focus_body: String,
    pub break_title: String,
    pub break_body: String,
    pub daily_title: String,
    pub daily_body: String,
    pub test_title: String,
    pub test_body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Daily {
    /// Minutes after local midnight.
    pub minutes: u16,
    /// Monday first.
    pub days: [bool; 7],
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Prefs {
    /// Say when a focus session or break ends.
    pub phase: bool,
    pub daily: Option<Daily>,
    /// Quiet hours as minutes after local midnight, (start, end); may cross midnight.
    pub quiet: Option<(u16, u16)>,
    /// Minutes east of UTC.
    pub tz: i32,
    pub texts: Texts,
}

const MAX_TEXT: usize = 120;

impl Prefs {
    /// Check what a browser sent: sane numbers and short texts.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(-14 * 60..=14 * 60).contains(&self.tz) {
            return Err("bad_tz");
        }
        if let Some(d) = &self.daily {
            if d.minutes >= 24 * 60 {
                return Err("bad_daily");
            }
        }
        if let Some((a, b)) = self.quiet {
            if a >= 24 * 60 || b >= 24 * 60 {
                return Err("bad_quiet");
            }
        }
        let t = &self.texts;
        let all = [
            &t.focus_title,
            &t.focus_body,
            &t.break_title,
            &t.break_body,
            &t.daily_title,
            &t.daily_body,
            &t.test_title,
            &t.test_body,
        ];
        if all
            .iter()
            .any(|s| s.chars().count() > MAX_TEXT || s.chars().any(char::is_control))
        {
            return Err("bad_text");
        }
        Ok(())
    }
}

/// Is `endpoint` an https address on one of the browser makers' push services?
pub fn endpoint_allowed(endpoint: &str) -> bool {
    if endpoint.len() > 1000 {
        return false;
    }
    let Some(rest) = endpoint.strip_prefix("https://") else {
        return false;
    };
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty() || authority.contains(['@', ' ', ':']) {
        return false;
    }
    let host = authority.to_ascii_lowercase();
    HOST_SUFFIXES.iter().any(|s| {
        if let Some(stripped) = s.strip_prefix('.') {
            host.ends_with(&format!(".{stripped}")) || host == stripped
        } else {
            host == *s
        }
    })
}

/// Is the keys part of a subscription well formed (a 65-byte uncompressed P-256 key and a 16-byte secret)?
pub fn keys_valid(p256dh: &str, auth: &str) -> bool {
    webpush::b64_decode(p256dh).is_some_and(|k| k.len() == 65 && k[0] == 4)
        && webpush::b64_decode(auth).is_some_and(|a| a.len() == 16)
}

fn minute_of_day(now_ms: i64, tz: i32) -> i64 {
    (now_ms.div_euclid(60_000) + i64::from(tz)).rem_euclid(24 * 60)
}

fn local_day(now_ms: i64, tz: i32) -> i64 {
    (now_ms.div_euclid(60_000) + i64::from(tz)).div_euclid(24 * 60)
}

/// Is it quiet hours (start included, end not; may cross midnight; equal means never)?
pub fn in_quiet(now_ms: i64, prefs: &Prefs) -> bool {
    let Some((start, end)) = prefs.quiet else {
        return false;
    };
    let (start, end, now) = (
        i64::from(start),
        i64::from(end),
        minute_of_day(now_ms, prefs.tz),
    );
    if start == end {
        false
    } else if start < end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

/// The day number (local) the daily reminder is due for now, if it is, and was not yet sent for that day.
pub fn daily_due(now_ms: i64, prefs: &Prefs, last_sent_day: Option<i64>) -> Option<i64> {
    let daily = prefs.daily.as_ref()?;
    let day = local_day(now_ms, prefs.tz);
    if last_sent_day.is_some_and(|d| d >= day) {
        return None;
    }
    // 1 January 1970 was a Thursday; Monday-first index.
    let weekday = usize::try_from((day + 3).rem_euclid(7)).ok()?;
    if !daily.days[weekday] {
        return None;
    }
    let now = minute_of_day(now_ms, prefs.tz);
    let at = i64::from(daily.minutes);
    (now >= at && now < at + DAILY_GRACE_MIN).then_some(day)
}

/// A focus session or break whose time ran out while nothing was watching: (its identity, is it a focus session).
pub fn phase_due(timer: &TimerState, now_ms: i64) -> Option<(String, bool)> {
    let focus = match timer.state {
        State::Session => true,
        State::Rest => false,
        _ => return None,
    };
    let t = timer.timer.as_ref()?;
    if t.is_paused() {
        return None;
    }
    let end = t.start() + t.total() + t.pause_buffer();
    (now_ms >= end && now_ms - end < PHASE_FRESH_MS)
        .then(|| (format!("{}:{}:{}", t.start(), t.total(), focus), focus))
}

#[derive(Debug, PartialEq, Eq)]
pub enum SendError {
    /// The push service says the subscription no longer exists (404 or 410): forget it.
    Gone,
    Failed(String),
}

/// The server's VAPID key, made on first use and kept in the database.
pub fn vapid_key(db: &Db) -> Result<SigningKey, String> {
    if let Some(text) = db.meta("vapid_key").map_err(|e| e.to_string())? {
        if let Some(key) = webpush::b64_decode(&text).and_then(|b| SigningKey::from_slice(&b).ok())
        {
            return Ok(key);
        }
    }
    let key = loop {
        let raw = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let bytes: Vec<u8> = (0..32)
            .map(|i| u8::from_str_radix(&raw[i * 2..i * 2 + 2], 16).unwrap_or(1))
            .collect();
        if let Ok(key) = SigningKey::from_slice(&bytes) {
            db.set_meta("vapid_key", &webpush::b64_encode(&bytes))
                .map_err(|e| e.to_string())?;
            break key;
        }
    };
    Ok(key)
}

fn random_bytes<const N: usize>() -> [u8; N] {
    let mut out = [0u8; N];
    let mut filled = 0;
    while filled < N {
        let id = uuid::Uuid::new_v4();
        for b in id.as_bytes() {
            if filled < N {
                out[filled] = *b;
                filled += 1;
            }
        }
    }
    out
}

/// Encrypt and send one message to one subscription (blocking: call from a blocking task).
pub fn send(
    sub: &PushSub,
    message: &str,
    vapid: &SigningKey,
    subject: &str,
    now_s: u64,
    allow_private: bool,
) -> Result<(), SendError> {
    if !endpoint_allowed(&sub.endpoint) {
        return Err(SendError::Failed("endpoint not allowed".into()));
    }
    let ua = webpush::b64_decode(&sub.p256dh).ok_or_else(|| SendError::Failed("bad key".into()))?;
    let auth =
        webpush::b64_decode(&sub.auth).ok_or_else(|| SendError::Failed("bad auth".into()))?;
    let ephemeral = loop {
        if let Ok(k) = SecretKey::from_slice(&random_bytes::<32>()) {
            break k;
        }
    };
    let body = webpush::encrypt(
        message.as_bytes(),
        &ua,
        &auth,
        &ephemeral,
        &random_bytes::<16>(),
    )
    .map_err(|e| SendError::Failed(format!("{e:?}")))?;
    let audience = {
        let rest = sub.endpoint.strip_prefix("https://").unwrap_or("");
        format!("https://{}", rest.split('/').next().unwrap_or(""))
    };
    let authorization = webpush::vapid_header(vapid, &audience, subject, now_s + 12 * 3600);

    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(10)))
        .max_redirects(0)
        .http_status_as_error(false)
        .build();
    let agent = ureq::Agent::with_parts(
        config,
        DefaultConnector::default(),
        PublicOnly {
            inner: DefaultResolver::default(),
            allow_private,
        },
    );
    let response = agent
        .post(&sub.endpoint)
        .header("content-encoding", "aes128gcm")
        .header("content-type", "application/octet-stream")
        .header("ttl", "60")
        .header("urgency", "high")
        .header("authorization", authorization)
        .send(body.as_slice())
        .map_err(|e| SendError::Failed(e.to_string()))?;
    match response.status().as_u16() {
        200..=299 => Ok(()),
        404 | 410 => Err(SendError::Gone),
        other => Err(SendError::Failed(format!("push service answered {other}"))),
    }
}

/// The JSON a service worker shows as a notification.
pub fn message(title: &str, body: &str, tag: &str) -> String {
    serde_json::json!({ "title": title, "body": body, "tag": tag }).to_string()
}

fn subject(state: &Shared) -> String {
    match &state.config.public {
        Some(p) if p.https => p.origin.clone(),
        _ => "mailto:noreply@openpomodoro.invalid".to_string(),
    }
}

fn digest_key(s: &str) -> String {
    sha2::Sha256::digest(s.as_bytes())
        .iter()
        .take(6)
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Send one message to one subscription. True when the push service took it. A subscription the service says is
/// gone is forgotten; other failures are noted in the server log.
pub async fn deliver(state: &Shared, sub: &PushSub, message: String) -> bool {
    let Ok(vapid) = vapid_key(&state.db()) else {
        return false;
    };
    let subject = subject(state);
    let now_s = u64::try_from(state.now_ms() / 1000).unwrap_or(0);
    let target = sub.clone();
    let result = tokio::task::spawn_blocking(move || {
        send(&target, &message, &vapid, &subject, now_s, false)
    })
    .await;
    match result {
        Ok(Ok(())) => true,
        Ok(Err(SendError::Gone)) => {
            let _ = state.db().delete_push_endpoint(&sub.endpoint);
            crate::serverlog::record(
                "push",
                format!(
                    "subscription {} is gone, removed",
                    digest_key(&sub.endpoint)
                ),
            );
            false
        }
        Ok(Err(SendError::Failed(why))) => {
            crate::serverlog::record(
                "push",
                format!("send to {} failed: {why}", digest_key(&sub.endpoint)),
            );
            false
        }
        Err(e) => {
            crate::serverlog::record("push", format!("send crashed: {e}"));
            false
        }
    }
}

/// Send to every subscription of `user` for which `build` makes a message; returns how many were delivered.
pub async fn send_to_user(
    state: &Shared,
    user: &str,
    build: impl Fn(&PushSub, &Prefs) -> Option<String>,
) -> usize {
    let subs = state.db().push_subs_of(user).unwrap_or_default();
    let mut delivered = 0;
    for sub in subs {
        let Ok(prefs) = serde_json::from_str::<Prefs>(&sub.prefs) else {
            continue;
        };
        if let Some(message) = build(&sub, &prefs) {
            if deliver(state, &sub, message).await {
                delivered += 1;
            }
        }
    }
    delivered
}

/// The background loop: every few seconds, look at people who have subscribed and nobody watching, and send what is due.
pub async fn run(state: Shared) {
    let mut sent_phases: HashSet<(String, String)> = HashSet::new();
    let mut tick = tokio::time::interval(LOOP_EVERY);
    loop {
        tick.tick().await;
        let now = state.now_ms();
        let users = state.db().push_users().unwrap_or_default();
        for user in users {
            if state.hub.connections(&user) > 0 {
                continue; // the app is open somewhere and alerts for itself
            }
            let Ok(timer) = crate::timer_service::load_state(&state.db(), &user) else {
                continue;
            };
            if let Some((key, focus)) = phase_due(&timer, now) {
                if sent_phases.insert((user.clone(), key)) {
                    send_to_user(&state, &user, |_, p| {
                        if !p.phase || in_quiet(now, p) {
                            return None;
                        }
                        let t = &p.texts;
                        Some(if focus {
                            message(&t.focus_title, &t.focus_body, "op-phase")
                        } else {
                            message(&t.break_title, &t.break_body, "op-phase")
                        })
                    })
                    .await;
                }
                if sent_phases.len() > 5000 {
                    sent_phases.clear();
                }
            }
            if timer.state == State::Idle {
                let subs = state.db().push_subs_of(&user).unwrap_or_default();
                for sub in subs {
                    let Ok(prefs) = serde_json::from_str::<Prefs>(&sub.prefs) else {
                        continue;
                    };
                    if in_quiet(now, &prefs) {
                        continue;
                    }
                    let key = format!("push_daily:{}", sub.id);
                    let last = state
                        .db()
                        .meta(&key)
                        .ok()
                        .flatten()
                        .and_then(|v| v.parse::<i64>().ok());
                    if let Some(day) = daily_due(now, &prefs, last) {
                        let _ = state.db().set_meta(&key, &day.to_string());
                        let t = &prefs.texts;
                        deliver(
                            &state,
                            &sub,
                            message(&t.daily_title, &t.daily_body, "op-daily"),
                        )
                        .await;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_core::{PhaseKind, Timer};

    fn prefs(quiet: Option<(u16, u16)>, daily: Option<Daily>, tz: i32) -> Prefs {
        Prefs {
            phase: true,
            daily,
            quiet,
            tz,
            texts: Texts::default(),
        }
    }

    const HOUR: i64 = 3_600_000;
    // Friday 6 November 2026, 12:00 UTC.
    const NOON: i64 = 1_793_966_400_000;

    #[test]
    fn only_the_browser_makers_push_services_are_allowed() {
        for ok in [
            "https://fcm.googleapis.com/fcm/send/abc",
            "https://updates.push.services.mozilla.com/wpush/v2/abc",
            "https://web.push.apple.com/abc",
            "https://wns2-par02p.notify.windows.com/w/?token=x",
            "https://api.push.apple.com/3/device/abc",
        ] {
            assert!(endpoint_allowed(ok), "{ok}");
        }
        for bad in [
            "http://fcm.googleapis.com/x",
            "https://evil.example/fcm.googleapis.com",
            "https://fcm.googleapis.com.evil.example/x",
            "https://notfcm.googleapis.com/x",
            "https://user@fcm.googleapis.com/x",
            "https://127.0.0.1/x",
            "https://fcm.googleapis.com:8080/x",
            "",
            "ftp://fcm.googleapis.com/x",
        ] {
            assert!(!endpoint_allowed(bad), "{bad}");
        }
    }

    #[test]
    fn keys_must_be_a_real_key_and_a_16_byte_secret() {
        let good = webpush::b64_encode(
            &SecretKey::from_slice(&[3u8; 32])
                .expect("k")
                .public_key()
                .to_sec1_bytes(),
        );
        let auth = webpush::b64_encode(&[1u8; 16]);
        assert!(keys_valid(&good, &auth));
        assert!(!keys_valid("AAAA", &auth));
        assert!(!keys_valid(&good, "AAAA"));
        assert!(!keys_valid("!!", "!!"));
    }

    #[test]
    fn quiet_hours_follow_the_devices_clock_and_may_cross_midnight() {
        // 12:00 UTC is 14:00 at +120.
        let p = prefs(Some((13 * 60, 15 * 60)), None, 120);
        assert!(in_quiet(NOON, &p));
        assert!(!in_quiet(NOON + 2 * HOUR, &p), "16:00 local is after");
        let night = prefs(Some((22 * 60, 7 * 60)), None, 0);
        assert!(in_quiet(NOON + 11 * HOUR, &night), "23:00");
        assert!(in_quiet(NOON - 6 * HOUR, &night), "06:00");
        assert!(!in_quiet(NOON, &night));
        assert!(!in_quiet(NOON, &prefs(Some((300, 300)), None, 0)));
        assert!(!in_quiet(NOON, &prefs(None, None, 0)));
    }

    #[test]
    fn the_daily_reminder_comes_on_the_chosen_days_at_the_chosen_time_once() {
        let weekdays = Daily {
            minutes: 12 * 60 + 5,
            days: [true, true, true, true, true, false, false],
        };
        let p = prefs(None, Some(weekdays.clone()), 0);
        let friday_1205 = NOON + 5 * 60_000;
        let day = daily_due(friday_1205, &p, None).expect("due on a Friday at 12:05");
        assert_eq!(daily_due(NOON, &p, None), None, "not yet");
        assert_eq!(
            daily_due(friday_1205 + 10 * 60_000, &p, None),
            None,
            "grace is over"
        );
        assert_eq!(
            daily_due(friday_1205, &p, Some(day)),
            None,
            "already sent today"
        );
        assert!(daily_due(friday_1205, &p, Some(day - 1)).is_some());
        let saturday = friday_1205 + 24 * HOUR;
        assert_eq!(daily_due(saturday, &p, None), None, "not on a Saturday");
        // The same instant is a different local day at +14 hours (02:05 on Saturday).
        let far = prefs(
            None,
            Some(Daily {
                minutes: 2 * 60 + 5,
                days: weekdays.days,
            }),
            14 * 60,
        );
        assert_eq!(daily_due(friday_1205, &far, None), None, "Saturday there");
    }

    #[test]
    fn a_phase_is_announced_once_it_has_ended_unseen_and_only_while_fresh() {
        let mut timer = TimerState::idle();
        timer.state = State::Session;
        timer.kind = Some(PhaseKind::Focus);
        timer.timer = Some(Timer::from_parts(NOON, 25 * 60_000, 0, None));
        let end = NOON + 25 * 60_000;
        assert_eq!(phase_due(&timer, end - 1), None);
        let (key, focus) = phase_due(&timer, end + 1000).expect("due");
        assert!(focus);
        assert_eq!(
            phase_due(&timer, end + 1000).map(|d| d.0),
            Some(key),
            "the same identity each time"
        );
        assert_eq!(phase_due(&timer, end + 120_000), None, "too old");
        timer.timer = Some(Timer::from_parts(NOON, 25 * 60_000, 0, Some(NOON + 1000)));
        assert_eq!(phase_due(&timer, end + 1000), None, "paused");
        timer.state = State::Paused(op_core::PausedFrom::Session);
        assert_eq!(phase_due(&timer, end + 1000), None);
        timer.state = State::Rest;
        timer.timer = Some(Timer::from_parts(NOON, 5 * 60_000, 0, None));
        assert_eq!(
            phase_due(&timer, NOON + 5 * 60_000 + 10).map(|d| d.1),
            Some(false)
        );
        timer.state = State::Idle;
        assert_eq!(phase_due(&timer, NOON + 5 * 60_000 + 10), None);
    }

    #[test]
    fn prefs_are_checked() {
        let ok = prefs(
            None,
            Some(Daily {
                minutes: 540,
                days: [true; 7],
            }),
            60,
        );
        assert_eq!(ok.validate(), Ok(()));
        assert_eq!(prefs(None, None, 15 * 60).validate(), Err("bad_tz"));
        assert_eq!(
            prefs(
                None,
                Some(Daily {
                    minutes: 1440,
                    days: [true; 7]
                }),
                0
            )
            .validate(),
            Err("bad_daily")
        );
        assert_eq!(prefs(Some((1440, 0)), None, 0).validate(), Err("bad_quiet"));
        let mut long = prefs(None, None, 0);
        long.texts.focus_body = "x".repeat(121);
        assert_eq!(long.validate(), Err("bad_text"));
        long.texts.focus_body = "bell\u{7}".into();
        assert_eq!(long.validate(), Err("bad_text"));
    }

    #[test]
    fn the_message_is_small_json() {
        let m: serde_json::Value =
            serde_json::from_str(&message("Focus complete", "Time for a break.", "op-phase"))
                .expect("json");
        assert_eq!(
            m,
            serde_json::json!({"title": "Focus complete", "body": "Time for a break.", "tag": "op-phase"})
        );
    }
}
