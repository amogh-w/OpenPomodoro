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
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("configuration error: {message}");
            return ExitCode::from(2);
        }
    };

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
    println!(
        "OpenPomodoro on http://{addr} (database {}, web {})",
        config.db_path.display(),
        config.web_dir.display()
    );
    if let Some(public) = &config.public {
        println!("public URL {}", public.origin);
    }

    let state = AppState::new(
        db,
        Accounts::new(Hasher::new()),
        config,
        Arc::new(SystemClock),
    );
    let service = op_server::app(state).into_make_service_with_connect_info::<SocketAddr>();
    match axum::serve(listener, service).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("server error: {e}");
            ExitCode::FAILURE
        }
    }
}
