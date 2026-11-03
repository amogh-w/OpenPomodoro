use std::{net::SocketAddr, path::PathBuf};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let port: u16 = std::env::var("OP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let web_dir: PathBuf =
        std::env::var_os("OP_WEB_DIR").map_or_else(|| PathBuf::from("web/dist"), PathBuf::from);

    // Loopback only; LAN listening is an explicit later opt-in (PLAN.md, Security).
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!(
        "OpenPomodoro on http://{addr} (serving {})",
        web_dir.display()
    );
    axum::serve(listener, op_server::app(&web_dir)).await
}
