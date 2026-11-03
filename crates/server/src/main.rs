use std::{net::SocketAddr, process::ExitCode, sync::Arc};

use op_core::SystemClock;
use op_server::{
    accounts::{Accounts, Hasher},
    config::Config,
    db::Db,
    state::AppState,
};

#[tokio::main]
async fn main() -> ExitCode {
    // `op-server healthcheck`: for container health checks. Exits 0 only if the running server is healthy.
    let healthcheck = std::env::args().nth(1).as_deref() == Some("healthcheck");

    let config = match Config::from_env() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("configuration error: {message}");
            return ExitCode::from(2);
        }
    };

    if healthcheck {
        return if op_server::healthcheck::check(config.port) {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        };
    }

    if let Some(dir) = config
        .db_path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
    {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("cannot create {}: {e}", dir.display());
            return ExitCode::FAILURE;
        }
    }
    let db = match Db::open(&config.db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("cannot open database {}: {e}", config.db_path.display());
            return ExitCode::FAILURE;
        }
    };

    let addr = SocketAddr::new(config.bind, config.port);
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("cannot listen on {addr}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let web = match (&config.web_dir, op_server::assets::embedded_available()) {
        (Some(dir), _) => format!("folder {}", dir.display()),
        (None, true) => "built into this program".to_string(),
        (None, false) => "folder web/dist".to_string(),
    };
    println!(
        "OpenPomodoro on http://{addr} (database {}, web app {web})",
        config.db_path.display()
    );
    if let Some(public) = &config.public {
        println!("public URL {}", public.origin);
    }

    let state = match AppState::new(
        db,
        Accounts::new(Hasher::new()),
        config,
        Arc::new(SystemClock),
    ) {
        Ok(state) => state,
        Err(e) => {
            eprintln!("cannot start: {e}");
            return ExitCode::FAILURE;
        }
    };
    let service = op_server::app(state).into_make_service_with_connect_info::<SocketAddr>();
    match axum::serve(listener, service).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("server error: {e}");
            ExitCode::FAILURE
        }
    }
}
