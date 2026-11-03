//! Server configuration from environment variables (X-07).

use std::net::IpAddr;
use std::path::PathBuf;

/// How the outside world reaches this server (`OP_PUBLIC_URL`), for example `https://pomodoro.example.com`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicUrl {
    /// `scheme://host[:port]`, lower-cased, no trailing slash.
    pub origin: String,
    /// `host[:port]`, lower-cased: what a browser sends in the `Host` header.
    pub authority: String,
    pub https: bool,
}

impl PublicUrl {
    pub fn parse(raw: &str) -> Result<Self, String> {
        let raw = raw.trim();
        let (scheme, rest) = raw
            .split_once("://")
            .map(|(s, rest)| (s.to_ascii_lowercase(), rest))
            .filter(|(s, _)| matches!(s.as_str(), "http" | "https"))
            .ok_or("OP_PUBLIC_URL must start with http:// or https://")?;
        let authority = rest.strip_suffix('/').unwrap_or(rest).to_ascii_lowercase();
        let valid = !authority.is_empty()
            && !authority.contains(['/', '?', '#', '@', ' '])
            && !authority.starts_with(':');
        if !valid {
            return Err("OP_PUBLIC_URL must be just scheme, host and optional port (no path, query or user)".into());
        }
        Ok(Self {
            origin: format!("{scheme}://{authority}"),
            authority,
            https: scheme == "https",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub bind: IpAddr,
    pub port: u16,
    pub public: Option<PublicUrl>,
    pub db_path: PathBuf,
    pub web_dir: PathBuf,
    /// Trust the `X-Forwarded-For` header from a reverse proxy in front of this server.
    pub trust_proxy: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let bind: IpAddr = match get("OP_BIND") {
            Some(v) => v
                .trim()
                .parse()
                .map_err(|_| format!("OP_BIND is not an IP address: {v}"))?,
            None => IpAddr::from([127, 0, 0, 1]),
        };
        let port: u16 = match get("OP_PORT") {
            Some(v) => v
                .trim()
                .parse()
                .map_err(|_| format!("OP_PORT is not a port number: {v}"))?,
            None => 3000,
        };
        let public = get("OP_PUBLIC_URL")
            .filter(|v| !v.trim().is_empty())
            .map(|v| PublicUrl::parse(&v))
            .transpose()?;
        if !bind.is_loopback() && public.is_none() {
            return Err("OP_PUBLIC_URL is required when OP_BIND is not a loopback address".into());
        }
        let flag = |key: &str| {
            get(key).is_some_and(|v| {
                matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes")
            })
        };
        Ok(Self {
            bind,
            port,
            public,
            db_path: get("OP_DB")
                .map_or_else(|| PathBuf::from("data/openpomodoro.db"), PathBuf::from),
            web_dir: get("OP_WEB_DIR").map_or_else(|| PathBuf::from("web/dist"), PathBuf::from),
            trust_proxy: flag("OP_TRUST_PROXY"),
        })
    }

    /// True when browsers reach the app over https, so cookies can be `Secure`.
    pub fn https(&self) -> bool {
        self.public.as_ref().is_some_and(|p| p.https)
    }

    /// `__Host-` cookies must be `Secure`, so the prefix is only used over https.
    pub fn session_cookie_name(&self) -> &'static str {
        if self.https() {
            "__Host-op_session"
        } else {
            "op_session"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(pairs: &[(&str, &str)]) -> Result<Config, String> {
        Config::from_lookup(|k| {
            pairs
                .iter()
                .find(|(key, _)| *key == k)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn defaults_are_loopback_port_3000() {
        let c = cfg(&[]).expect("defaults");
        assert_eq!(c.bind, IpAddr::from([127, 0, 0, 1]));
        assert_eq!(c.port, 3000);
        assert_eq!(c.public, None);
        assert_eq!(c.db_path, PathBuf::from("data/openpomodoro.db"));
        assert!(!c.trust_proxy && !c.https());
        assert_eq!(c.session_cookie_name(), "op_session");
    }

    #[test]
    fn public_url_is_normalised() {
        let p = PublicUrl::parse(" HTTPS://Pomodoro.Example.com/ ").expect("parse");
        assert_eq!(
            p,
            PublicUrl {
                origin: "https://pomodoro.example.com".into(),
                authority: "pomodoro.example.com".into(),
                https: true
            }
        );
        assert_eq!(
            PublicUrl::parse("http://localhost:8080")
                .expect("parse")
                .authority,
            "localhost:8080"
        );
    }

    #[test]
    fn public_url_rejects_junk() {
        for bad in [
            "",
            "example.com",
            "ftp://example.com",
            "https://",
            "https://example.com/app",
            "https://a@example.com",
            "https://example.com?x=1",
            "https://:80",
        ] {
            assert!(PublicUrl::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn exposing_the_server_requires_a_public_url() {
        assert!(cfg(&[("OP_BIND", "0.0.0.0")]).is_err());
        let c = cfg(&[
            ("OP_BIND", "0.0.0.0"),
            ("OP_PUBLIC_URL", "https://pomodoro.example.com"),
            ("OP_TRUST_PROXY", "true"),
        ])
        .expect("ok");
        assert!(c.https() && c.trust_proxy);
        assert_eq!(c.session_cookie_name(), "__Host-op_session");
    }

    #[test]
    fn bad_values_are_reported() {
        assert!(cfg(&[("OP_BIND", "not-an-ip")]).is_err());
        assert!(cfg(&[("OP_PORT", "99999")]).is_err());
        assert!(cfg(&[("OP_PUBLIC_URL", "nonsense")]).is_err());
    }
}
