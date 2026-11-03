//! `op-server healthcheck`: asks a running server whether it is healthy, for container health checks (the image
//! has no `curl`). Exits successfully only if `/api/health` answers 200.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::time::Duration;

/// Does this HTTP response say 200?
fn is_ok(response: &str) -> bool {
    response
        .lines()
        .next()
        .is_some_and(|status| status.split_whitespace().nth(1) == Some("200"))
}

/// Ask the server on this machine (loopback, `port`) and say whether it is healthy.
pub fn check(port: u16) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(3)) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let request =
        format!("GET /api/health HTTP/1.0\r\nHost: localhost:{port}\r\nConnection: close\r\n\r\n");
    let mut response = String::new();
    stream.write_all(request.as_bytes()).is_ok()
        && stream.read_to_string(&mut response).is_ok()
        && is_ok(&response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn serve_once(reply: &'static str) -> u16 {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            if let Ok((mut conn, _)) = listener.accept() {
                let mut buf = [0u8; 512];
                let _ = conn.read(&mut buf);
                let _ = conn.write_all(reply.as_bytes());
            }
        });
        port
    }

    #[test]
    fn reads_the_status_line() {
        assert!(is_ok("HTTP/1.1 200 OK\r\ncontent-type: x\r\n\r\n{}"));
        assert!(!is_ok("HTTP/1.1 403 Forbidden\r\n\r\n"));
        assert!(!is_ok("HTTP/1.1 500 Internal Server Error\r\n\r\n"));
        assert!(!is_ok(""));
        assert!(!is_ok("garbage"));
    }

    #[test]
    fn healthy_when_the_server_answers_200() {
        assert!(check(serve_once(
            "HTTP/1.0 200 OK\r\n\r\n{\"status\":\"ok\"}"
        )));
    }

    #[test]
    fn unhealthy_on_an_error_answer_or_when_nothing_is_listening() {
        assert!(!check(serve_once(
            "HTTP/1.0 500 Internal Server Error\r\n\r\n"
        )));
        let free = TcpListener::bind("127.0.0.1:0")
            .expect("bind")
            .local_addr()
            .expect("addr")
            .port();
        assert!(!check(free), "the port is free again, so nothing answers");
    }
}
