//! Lifecycle hooks (I-04, I-05): when the timer moves between phases, run the executables the server admin listed
//! in the hooks file, handing each a JSON description of what happened on stdin.
//!
//! Hooks run programs on the server, so they are configured only by the admin, in a file named by `OP_HOOKS_FILE`
//! (never from the web app), and a person's data only reaches a hook the admin chose to run.

use std::{path::Path, process::Stdio, sync::Arc, time::Duration};

use op_core::{PausedFrom, State, TimerState};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::{io::AsyncWriteExt, process::Command, sync::Semaphore, sync::mpsc};

/// Every event a hook can listen for. `*` in the file means all of them.
pub const EVENTS: [&str; 9] = [
    "focus_started",
    "focus_ended",
    "focus_stopped",
    "focus_abandoned",
    "paused",
    "resumed",
    "break_started",
    "break_ended",
    "break_stopped",
];

/// Events waiting for the worker. A full queue drops new events (and says so on stderr) rather than slowing the
/// timer down: hooks are a courtesy, never a gate.
const QUEUE: usize = 64;
/// Hooks running at the same moment, so a stuck program cannot pile up processes.
const CONCURRENT: usize = 8;
const DEFAULT_TIMEOUT_S: u64 = 10;
const MAX_TIMEOUT_S: u64 = 60;

fn yes() -> bool {
    true
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_S
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HookDef {
    /// One of [`EVENTS`], or `*`.
    pub event: String,
    /// The program and its arguments. Run directly, never through a shell.
    pub command: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_s: u64,
    #[serde(default = "yes")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HooksFile {
    #[serde(default)]
    pub hooks: Vec<HookDef>,
}

impl HooksFile {
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let file: Self = serde_json::from_str(text).map_err(|e| e.to_string())?;
        for (i, hook) in file.hooks.iter().enumerate() {
            let n = i + 1;
            if hook.event != "*" && !EVENTS.contains(&hook.event.as_str()) {
                return Err(format!("hook {n}: unknown event \"{}\"", hook.event));
            }
            if hook
                .command
                .first()
                .is_none_or(|program| program.trim().is_empty())
            {
                return Err(format!("hook {n}: command must name a program"));
            }
            if !(1..=MAX_TIMEOUT_S).contains(&hook.timeout_s) {
                return Err(format!("hook {n}: timeout_s must be 1 to {MAX_TIMEOUT_S}"));
            }
        }
        Ok(file)
    }
}

fn pauses(state: State) -> Option<PausedFrom> {
    match state {
        State::Paused(from) => Some(from),
        _ => None,
    }
}

fn is_focus(state: State) -> bool {
    matches!(
        state,
        State::Session
            | State::SessionEnd
            | State::Paused(PausedFrom::Session | PausedFrom::SessionEnd)
    )
}

fn is_break(state: State) -> bool {
    matches!(
        state,
        State::Rest | State::RestEnd | State::Paused(PausedFrom::Rest | PausedFrom::RestEnd)
    )
}

fn starts_focus(state: State) -> bool {
    matches!(state, State::Session | State::Breathe)
}

/// What happened between two timer states, oldest first. Pure, so every rule is easy to test.
pub fn events_between(prev: &TimerState, next: &TimerState) -> Vec<&'static str> {
    let (a, b) = (prev.state, next.state);
    let mut out = Vec::new();
    // Time ran out on its own (an auto-started break or focus follows without a stop).
    let focus_time_up = a == State::Session && (b == State::SessionEnd || b == State::Rest);
    let break_time_up = a == State::Rest && (b == State::RestEnd || starts_focus(b));

    if is_focus(a) && b == State::Reflect {
        out.push("focus_stopped");
    }
    if is_focus(a) && b == State::Idle {
        out.push("focus_abandoned");
    }
    if focus_time_up {
        out.push("focus_ended");
    }
    if break_time_up {
        out.push("break_ended");
    }
    if is_break(a)
        && !break_time_up
        && (b == State::Idle || (starts_focus(b) && a != b))
        && !is_break(b)
    {
        out.push("break_stopped");
    }
    if pauses(b).is_some() && pauses(a).is_none() {
        out.push("paused");
    }
    if pauses(a).is_some() && pauses(b).is_none() {
        out.push("resumed");
    }
    if starts_focus(b) && !is_focus(a) && a != State::Breathe {
        out.push("focus_started");
    }
    if b == State::Rest && !is_break(a) {
        out.push("break_started");
    }
    out
}

/// What a hook is told. Kept small: the event, when, whose timer, and the timer's own fields.
pub fn payload(
    event: &str,
    at_ms: i64,
    user_id: &str,
    username: &str,
    timer: &TimerState,
) -> Value {
    json!({
        "event": event,
        "at": at_ms,
        "user": { "id": user_id, "username": username },
        "timer": timer.to_json(),
    })
}

struct Job {
    event: &'static str,
    body: String,
}

/// The sending side: cheap to call from a request. The worker does the running.
pub struct HookRunner {
    tx: mpsc::Sender<Job>,
    file: HooksFile,
}

impl HookRunner {
    /// The runner and the worker that must be spawned on the runtime (`tokio::spawn(worker.run())`).
    pub fn new(file: HooksFile) -> (Arc<Self>, Worker) {
        let (tx, rx) = mpsc::channel(QUEUE);
        let runner = Arc::new(Self {
            tx,
            file: file.clone(),
        });
        (runner, Worker { rx, file })
    }

    pub fn hooks(&self) -> &[HookDef] {
        &self.file.hooks
    }

    /// Queue an event for every enabled hook that wants it. Never waits.
    pub fn emit(&self, event: &'static str, body: &Value) {
        let wanted = self
            .file
            .hooks
            .iter()
            .any(|h| h.enabled && (h.event == "*" || h.event == event));
        if !wanted {
            return;
        }
        if self
            .tx
            .try_send(Job {
                event,
                body: body.to_string(),
            })
            .is_err()
        {
            eprintln!("hooks: queue full, dropped the {event} event");
        }
    }
}

pub struct Worker {
    rx: mpsc::Receiver<Job>,
    file: HooksFile,
}

/// How one hook ended, for the admin's Test button and the log.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Exited(i32),
    TimedOut,
    Failed(String),
}

/// Run one hook: the program with `body` on stdin and the event name in `OP_EVENT`, killed after its timeout.
pub async fn run_hook(hook: &HookDef, event: &str, body: &str) -> Outcome {
    let mut command = Command::new(&hook.command[0]);
    command
        .args(&hook.command[1..])
        .env("OP_EVENT", event)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    let work = async {
        if let Some(mut stdin) = child.stdin.take() {
            // A program that never reads its input just gets a broken pipe; that is not our failure.
            let _ = stdin.write_all(body.as_bytes()).await;
        }
        child.wait().await
    };
    match tokio::time::timeout(Duration::from_secs(hook.timeout_s), work).await {
        Ok(Ok(status)) => Outcome::Exited(status.code().unwrap_or(-1)),
        Ok(Err(e)) => Outcome::Failed(e.to_string()),
        Err(_) => Outcome::TimedOut,
    }
}

impl Worker {
    pub async fn run(mut self) {
        let slots = Arc::new(Semaphore::new(CONCURRENT));
        while let Some(job) = self.rx.recv().await {
            for hook in self
                .file
                .hooks
                .iter()
                .filter(|h| h.enabled && (h.event == "*" || h.event == job.event))
            {
                let Ok(permit) = slots.clone().acquire_owned().await else {
                    return;
                };
                let (hook, event, body) = (hook.clone(), job.event, job.body.clone());
                tokio::spawn(async move {
                    match run_hook(&hook, event, &body).await {
                        Outcome::Exited(0) => {}
                        other => eprintln!(
                            "hooks: {} for {event} ended with {other:?}",
                            hook.command[0]
                        ),
                    }
                    drop(permit);
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use op_core::PhaseKind;

    fn at(state: State) -> TimerState {
        let mut t = TimerState::idle();
        t.state = state;
        t.kind = match state {
            State::Rest
            | State::RestEnd
            | State::Paused(PausedFrom::Rest | PausedFrom::RestEnd) => Some(PhaseKind::ShortBreak),
            State::Idle | State::Reflect => None,
            _ => Some(PhaseKind::Focus),
        };
        t
    }

    fn between(a: State, b: State) -> Vec<&'static str> {
        events_between(&at(a), &at(b))
    }

    #[test]
    fn a_focus_session_start_to_finish() {
        assert_eq!(between(State::Idle, State::Session), ["focus_started"]);
        assert_eq!(
            between(State::Session, State::Paused(PausedFrom::Session)),
            ["paused"]
        );
        assert_eq!(
            between(State::Paused(PausedFrom::Session), State::Session),
            ["resumed"]
        );
        assert_eq!(between(State::Session, State::SessionEnd), ["focus_ended"]);
        assert_eq!(
            between(State::SessionEnd, State::Reflect),
            ["focus_stopped"]
        );
        assert_eq!(between(State::Session, State::Reflect), ["focus_stopped"]);
        assert_eq!(between(State::Session, State::Idle), ["focus_abandoned"]);
    }

    #[test]
    fn breathing_before_focus_starts_the_session_once() {
        assert_eq!(between(State::Idle, State::Breathe), ["focus_started"]);
        assert_eq!(between(State::Breathe, State::Session), Vec::<&str>::new());
    }

    #[test]
    fn breaks_start_end_and_stop() {
        assert_eq!(between(State::Reflect, State::Rest), ["break_started"]);
        assert_eq!(between(State::Idle, State::Rest), ["break_started"]);
        assert_eq!(between(State::Rest, State::RestEnd), ["break_ended"]);
        assert_eq!(between(State::Rest, State::Idle), ["break_stopped"]);
        assert_eq!(between(State::RestEnd, State::Idle), ["break_stopped"]);
        assert_eq!(
            between(State::Rest, State::Paused(PausedFrom::Rest)),
            ["paused"]
        );
    }

    #[test]
    fn auto_start_chains_report_each_step_in_order() {
        assert_eq!(
            between(State::Session, State::Rest),
            ["focus_ended", "break_started"]
        );
        assert_eq!(
            between(State::Rest, State::Session),
            ["break_ended", "focus_started"]
        );
    }

    #[test]
    fn leaving_a_break_for_a_new_focus_stops_the_break_and_starts_the_focus() {
        assert_eq!(
            between(State::RestEnd, State::Session),
            ["break_stopped", "focus_started"]
        );
        assert_eq!(
            between(State::Rest, State::Breathe),
            ["break_ended", "focus_started"]
        );
    }

    #[test]
    fn nothing_that_stays_put_makes_an_event() {
        for s in [
            State::Idle,
            State::Session,
            State::Rest,
            State::Reflect,
            State::SessionEnd,
        ] {
            assert!(between(s, s).is_empty(), "{s:?}");
        }
    }

    #[test]
    fn the_hooks_file_is_checked() {
        let ok = HooksFile::parse(r#"{"hooks":[{"event":"focus_started","command":["/bin/true"]},{"event":"*","command":["x","y"],"timeout_s":5,"enabled":false}]}"#).expect("ok");
        assert_eq!(ok.hooks.len(), 2);
        assert_eq!(ok.hooks[0].timeout_s, 10);
        assert!(ok.hooks[0].enabled);
        for bad in [
            r#"{"hooks":[{"event":"nope","command":["x"]}]}"#,
            r#"{"hooks":[{"event":"paused","command":[]}]}"#,
            r#"{"hooks":[{"event":"paused","command":[" "]}]}"#,
            r#"{"hooks":[{"event":"paused","command":["x"],"timeout_s":0}]}"#,
            r#"{"hooks":[{"event":"paused","command":["x"],"timeout_s":61}]}"#,
            r#"{"hooks":[{"event":"paused","command":["x"],"shell":true}]}"#,
            "not json",
        ] {
            assert!(HooksFile::parse(bad).is_err(), "{bad}");
        }
    }

    fn hook(command: &[&str], timeout_s: u64) -> HookDef {
        HookDef {
            event: "*".into(),
            command: command.iter().map(|s| s.to_string()).collect(),
            timeout_s,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn a_hook_gets_the_json_on_stdin_and_the_event_in_its_environment() {
        let dir = crate::test_util::TempDir::new();
        let out = dir.path().join("out.txt");
        let script = format!(
            "cat > {p}; printf %s \"$OP_EVENT\" >> {p}.event",
            p = out.display()
        );
        let result = run_hook(
            &hook(&["/bin/sh", "-c", &script], 5),
            "paused",
            r#"{"event":"paused"}"#,
        )
        .await;
        assert_eq!(result, Outcome::Exited(0));
        assert_eq!(
            std::fs::read_to_string(&out).expect("stdin"),
            r#"{"event":"paused"}"#
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("out.txt.event")).expect("env"),
            "paused"
        );
    }

    #[tokio::test]
    async fn a_slow_hook_is_killed_a_failing_one_reported_and_a_missing_one_does_not_crash() {
        assert_eq!(
            run_hook(&hook(&["/bin/sh", "-c", "sleep 30"], 1), "paused", "{}").await,
            Outcome::TimedOut
        );
        assert_eq!(
            run_hook(&hook(&["/bin/sh", "-c", "exit 3"], 5), "paused", "{}").await,
            Outcome::Exited(3)
        );
        assert!(matches!(
            run_hook(&hook(&["/no/such/program"], 5), "paused", "{}").await,
            Outcome::Failed(_)
        ));
    }

    #[tokio::test]
    async fn the_runner_only_queues_events_a_hook_wants_and_a_full_queue_never_blocks() {
        let file = HooksFile {
            hooks: vec![HookDef {
                event: "paused".into(),
                command: vec!["x".into()],
                timeout_s: 5,
                enabled: true,
            }],
        };
        let (runner, mut worker) = HookRunner::new(file);
        runner.emit("resumed", &json!({}));
        assert!(worker.rx.try_recv().is_err(), "nobody listens for resumed");
        for _ in 0..(QUEUE + 10) {
            runner.emit("paused", &json!({}));
        }
        let mut queued = 0;
        while worker.rx.try_recv().is_ok() {
            queued += 1;
        }
        assert_eq!(queued, QUEUE);
    }

    async fn signed_in(app: &crate::test_util::TestApp, name: &str, admin: Option<&str>) -> String {
        let pw = "correct horse battery staple";
        let cookie = |res: &crate::test_util::Res| {
            res.header("set-cookie")
                .expect("cookie")
                .split(';')
                .next()
                .expect("pair")
                .split_once('=')
                .expect("kv")
                .1
                .to_string()
        };
        match admin {
            None => cookie(
                &app.post(
                    "/api/setup",
                    Some(json!({"username": name, "password": pw})),
                )
                .send()
                .await,
            ),
            Some(boss) => {
                app.post(
                    "/api/admin/users",
                    Some(json!({"username": name, "password": pw})),
                )
                .cookie(boss)
                .send()
                .await;
                cookie(
                    &app.post(
                        "/api/signin",
                        Some(json!({"username": name, "password": pw})),
                    )
                    .send()
                    .await,
                )
            }
        }
    }

    #[tokio::test]
    async fn a_command_that_starts_focus_runs_the_hook_with_the_event_and_the_user() {
        let app = crate::test_util::TestApp::new();
        let cookie = signed_in(&app, "admin", None).await;
        let dir = crate::test_util::TempDir::new();
        let out = dir.path().join("seen.json");
        let script = format!("cat > {}", out.display());
        let file = HooksFile {
            hooks: vec![HookDef {
                event: "focus_started".into(),
                command: vec!["/bin/sh".into(), "-c".into(), script],
                timeout_s: 5,
                enabled: true,
            }],
        };
        let (runner, worker) = HookRunner::new(file);
        tokio::spawn(worker.run());
        assert!(app.state.hooks.set(runner).is_ok());

        let id = op_core::IdGen::new("t").next(app.state.now_ms());
        let res = app
            .post(
                "/api/commands",
                Some(json!({"id": id, "name": "init_focus", "intention": "hooked"})),
            )
            .cookie(&cookie)
            .send()
            .await;
        assert!(res.status.is_success(), "{}", res.text);

        let mut seen = None;
        for _ in 0..50 {
            if let Ok(text) = std::fs::read_to_string(&out) {
                if !text.is_empty() {
                    seen = Some(text);
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let body: Value = serde_json::from_str(&seen.expect("the hook ran")).expect("json");
        assert_eq!(body["event"], "focus_started");
        assert_eq!(body["user"]["username"], "admin");
        assert_eq!(body["timer"]["intention"], "hooked");
    }

    #[tokio::test]
    async fn only_the_admin_sees_and_tests_hooks() {
        let app = crate::test_util::TestApp::new();
        let boss = signed_in(&app, "admin", None).await;
        let bob = signed_in(&app, "bob", Some(&boss)).await;
        let file = HooksFile {
            hooks: vec![HookDef {
                event: "*".into(),
                command: vec!["/bin/sh".into(), "-c".into(), "exit 0".into()],
                timeout_s: 5,
                enabled: true,
            }],
        };
        let (runner, worker) = HookRunner::new(file);
        tokio::spawn(worker.run());
        assert!(app.state.hooks.set(runner).is_ok());

        assert_eq!(
            app.get("/api/admin/hooks").cookie(&bob).send().await.status,
            axum::http::StatusCode::FORBIDDEN
        );
        assert_eq!(
            app.post("/api/admin/hooks/0/test", None)
                .cookie(&bob)
                .send()
                .await
                .status,
            axum::http::StatusCode::FORBIDDEN
        );
        let list = app
            .get("/api/admin/hooks")
            .cookie(&boss)
            .send()
            .await
            .json();
        assert_eq!(list["hooks"][0]["event"], "*");
        let tested = app
            .post("/api/admin/hooks/0/test", None)
            .cookie(&boss)
            .send()
            .await
            .json();
        assert_eq!(
            (tested["result"].clone(), tested["detail"].clone()),
            (json!("exited"), json!(0))
        );
        assert_eq!(
            app.post("/api/admin/hooks/9/test", None)
                .cookie(&boss)
                .send()
                .await
                .status,
            axum::http::StatusCode::NOT_FOUND
        );
    }
}
