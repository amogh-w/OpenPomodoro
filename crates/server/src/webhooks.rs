//! Outgoing webhooks (I-06): the same events as the hooks, sent as an HTTP POST to a URL the server admin listed.
//!
//! The body is the hook's JSON. With a `secret`, each request carries `X-OpenPomodoro-Signature: sha256=<hex>`,
//! an HMAC-SHA256 of the body, so the receiver can tell it came from this server. Addresses that are private,
//! loopback or link-local are refused unless the admin allowed them, checked on the addresses actually connected
//! to (so a name that points inward is refused too), and redirects are never followed.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::{DefaultConnector, NextTimeout};

fn yes() -> bool {
    true
}

fn default_timeout() -> u64 {
    10
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebhookDef {
    /// One of the hook events, or `*`.
    pub event: String,
    /// `http://` or `https://`.
    pub url: String,
    /// Signs each request when set.
    #[serde(default)]
    pub secret: Option<String>,
    #[serde(default = "default_timeout")]
    pub timeout_s: u64,
    #[serde(default = "yes")]
    pub enabled: bool,
}

/// Why a delivery did not work.
#[derive(Debug, PartialEq, Eq)]
pub enum Failure {
    /// The server answered with a status outside 200 to 299.
    Status(u16),
    /// Could not connect, the address was refused, or it timed out.
    Network(String),
}

/// An address a webhook may call without the admin's say-so: not this machine and not the local network.
pub fn is_public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => public_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => public_v4(v4),
            None => public_v6(v6),
        },
    }
}

fn public_v4(ip: Ipv4Addr) -> bool {
    let [a, b, c, _] = ip.octets();
    !(ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || ip.is_documentation()
        || a == 0
        || (a == 100 && (64..=127).contains(&b)) // shared address space (carrier-grade NAT)
        || (a == 192 && b == 0 && c == 0) // protocol assignments
        || (a == 198 && (b == 18 || b == 19)) // benchmarking
        || a >= 240) // reserved
}

fn public_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    !(ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00 // unique local
        || (first & 0xffc0) == 0xfe80 // link-local
        || (first == 0x2001 && ip.segments()[1] == 0x0db8)) // documentation
}

/// HMAC-SHA256 (RFC 2104), hex.
pub fn sign(secret: &str, body: &[u8]) -> String {
    hmac_sha256_hex(secret.as_bytes(), body)
}

fn hmac_sha256_hex(key: &[u8], body: &[u8]) -> String {
    const BLOCK: usize = 64;
    let mut key = key.to_vec();
    if key.len() > BLOCK {
        key = Sha256::digest(&key).to_vec();
    }
    key.resize(BLOCK, 0);
    let pad = |byte: u8| key.iter().map(|k| k ^ byte).collect::<Vec<u8>>();
    let mut inner = Sha256::new();
    inner.update(pad(0x36));
    inner.update(body);
    let mut outer = Sha256::new();
    outer.update(pad(0x5c));
    outer.update(inner.finalize());
    outer
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Resolves like the default resolver, then drops every address the policy forbids.
#[derive(Debug)]
pub(crate) struct PublicOnly {
    pub(crate) inner: DefaultResolver,
    pub(crate) allow_private: bool,
}

impl Resolver for PublicOnly {
    fn resolve(
        &self,
        uri: &ureq::http::Uri,
        config: &ureq::config::Config,
        timeout: NextTimeout,
    ) -> Result<ResolvedSocketAddrs, ureq::Error> {
        let all = self.inner.resolve(uri, config, timeout)?;
        if self.allow_private {
            return Ok(all);
        }
        let mut kept = self.empty();
        for addr in all
            .iter()
            .copied()
            .filter(|a: &SocketAddr| is_public_ip(a.ip()))
        {
            kept.push(addr);
        }
        if kept.is_empty() {
            return Err(ureq::Error::HostNotFound);
        }
        Ok(kept)
    }
}

/// Send one webhook now (blocking; call from a blocking task). One retry for a network error or a 5xx answer.
pub fn deliver(
    hook: &WebhookDef,
    event: &str,
    body: &str,
    allow_private: bool,
) -> Result<u16, Failure> {
    let first = attempt(hook, event, body, allow_private);
    match first {
        Err(Failure::Network(_) | Failure::Status(500..=599)) => {
            std::thread::sleep(Duration::from_secs(2));
            attempt(hook, event, body, allow_private)
        }
        other => other,
    }
}

fn attempt(
    hook: &WebhookDef,
    event: &str,
    body: &str,
    allow_private: bool,
) -> Result<u16, Failure> {
    let config = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(hook.timeout_s)))
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
    let mut request = agent
        .post(&hook.url)
        .header("content-type", "application/json")
        .header(
            "user-agent",
            concat!("OpenPomodoro/", env!("CARGO_PKG_VERSION")),
        )
        .header("x-openpomodoro-event", event);
    if let Some(secret) = &hook.secret {
        request = request.header(
            "x-openpomodoro-signature",
            format!("sha256={}", sign(secret, body.as_bytes())),
        );
    }
    match request.send(body) {
        Ok(response) => {
            let status = response.status().as_u16();
            if (200..300).contains(&status) {
                Ok(status)
            } else {
                Err(Failure::Status(status))
            }
        }
        Err(e) => Err(Failure::Network(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn the_signature_matches_the_rfc_4231_test_vector() {
        let key = "\u{b}".repeat(20);
        assert_eq!(
            sign(&key, b"Hi There"),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // A key longer than the block is hashed first (RFC 4231 test case 6).
        assert_eq!(
            hmac_sha256_hex(
                &[0xaa; 131],
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            ),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn only_public_addresses_pass() {
        let public = [
            "8.8.8.8",
            "1.1.1.1",
            "93.184.216.34",
            "2606:4700:4700::1111",
        ];
        let not = [
            "127.0.0.1",
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.1",
            "192.168.1.1",
            "169.254.169.254",
            "0.0.0.0",
            "100.64.0.1",
            "224.0.0.1",
            "255.255.255.255",
            "198.18.0.1",
            "240.0.0.1",
            "::1",
            "::",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "ff02::1",
            "::ffff:127.0.0.1",
            "::ffff:10.0.0.1",
            "2001:db8::1",
        ];
        for ip in public {
            assert!(is_public_ip(ip.parse().expect("ip")), "{ip}");
        }
        for ip in not {
            assert!(!is_public_ip(ip.parse().expect("ip")), "{ip}");
        }
    }

    /// A one-shot receiver on localhost: answers `status` and hands back the raw request it saw.
    fn receiver(status: u16) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let url = format!("http://{}/hook", listener.local_addr().expect("addr"));
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .expect("timeout");
            let mut seen = Vec::new();
            let mut buf = [0u8; 4096];
            loop {
                let n = stream.read(&mut buf).expect("read");
                seen.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&seen);
                if let Some((head, rest)) = text.split_once("\r\n\r\n") {
                    let length = head
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length: ")
                                .and_then(|v| v.trim().parse::<usize>().ok())
                        })
                        .unwrap_or(0);
                    if rest.len() >= length {
                        break;
                    }
                }
                if n == 0 {
                    break;
                }
            }
            let _ = write!(
                stream,
                "HTTP/1.1 {status} X\r\ncontent-length: 0\r\nconnection: close\r\n\r\n"
            );
            String::from_utf8_lossy(&seen).into_owned()
        });
        (url, handle)
    }

    fn def(url: &str, secret: Option<&str>) -> WebhookDef {
        WebhookDef {
            event: "*".into(),
            url: url.into(),
            secret: secret.map(str::to_string),
            timeout_s: 5,
            enabled: true,
        }
    }

    #[test]
    fn a_signed_post_arrives_with_the_event_and_body() {
        let (url, seen) = receiver(200);
        let body = r#"{"event":"paused"}"#;
        assert_eq!(
            deliver(&def(&url, Some("s3cret")), "paused", body, true),
            Ok(200)
        );
        let request = seen.join().expect("receiver").to_ascii_lowercase();
        assert!(request.starts_with("post /hook"), "{request}");
        assert!(request.contains("x-openpomodoro-event: paused"));
        assert!(request.contains(&format!(
            "x-openpomodoro-signature: sha256={}",
            sign("s3cret", body.as_bytes())
        )));
        assert!(request.ends_with(body));
    }

    #[test]
    fn private_addresses_are_refused_unless_allowed() {
        let (url, seen) = receiver(200);
        let refused = deliver(&def(&url, None), "paused", "{}", false);
        assert!(matches!(refused, Err(Failure::Network(_))), "{refused:?}");
        // Nothing was sent; let the receiver finish by letting it be called once.
        assert_eq!(deliver(&def(&url, None), "paused", "{}", true), Ok(200));
        seen.join().expect("receiver");
    }

    #[test]
    fn a_redirect_is_not_followed_and_an_error_status_is_reported() {
        let (url, seen) = receiver(302);
        assert_eq!(
            attempt(&def(&url, None), "paused", "{}", true),
            Err(Failure::Status(302))
        );
        seen.join().expect("receiver");
        let (url, seen) = receiver(404);
        assert_eq!(
            attempt(&def(&url, None), "paused", "{}", true),
            Err(Failure::Status(404))
        );
        seen.join().expect("receiver");
    }
}
