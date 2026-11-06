//! Time-based one-time passwords (RFC 6238) for two-factor sign-in, and the recovery codes that stand in for them.
//! Pure functions; the database and the sign-in rules live elsewhere.

use sha1::{Digest, Sha1};

/// Seconds a code is valid for, and how many codes before and after the current one are accepted (clock drift).
pub const STEP_SECS: u64 = 30;
const DRIFT_STEPS: i64 = 1;
const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// RFC 4648 base32 without padding, the form authenticator apps take.
pub fn base32_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            out.push(char::from(ALPHABET[((buffer >> (bits - 5)) & 31) as usize]));
            bits -= 5;
        }
    }
    if bits > 0 {
        out.push(char::from(ALPHABET[((buffer << (5 - bits)) & 31) as usize]));
    }
    out
}

/// The reverse; spaces, dashes and lower case are tolerated, anything else is refused.
pub fn base32_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for ch in text.chars().filter(|c| !matches!(c, ' ' | '-' | '=')) {
        let value = ALPHABET.iter().position(|&a| char::from(a) == ch.to_ascii_uppercase())?;
        buffer = (buffer << 5) | u32::try_from(value).ok()?;
        bits += 5;
        if bits >= 8 {
            out.push(u8::try_from((buffer >> (bits - 8)) & 0xff).ok()?);
            bits -= 8;
        }
    }
    Some(out)
}

fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    const BLOCK: usize = 64;
    let mut key = key.to_vec();
    if key.len() > BLOCK {
        key = Sha1::digest(&key).to_vec();
    }
    key.resize(BLOCK, 0);
    let pad = |byte: u8| key.iter().map(|k| k ^ byte).collect::<Vec<u8>>();
    let mut inner = Sha1::new();
    inner.update(pad(0x36));
    inner.update(message);
    let mut outer = Sha1::new();
    outer.update(pad(0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

/// The six-digit code for one time step (RFC 4226 dynamic truncation).
pub fn code_for_step(secret: &[u8], step: u64) -> String {
    let mac = hmac_sha1(secret, &step.to_be_bytes());
    let offset = usize::from(mac[19] & 0x0f);
    let number = u32::from_be_bytes([mac[offset] & 0x7f, mac[offset + 1], mac[offset + 2], mac[offset + 3]]);
    format!("{:06}", number % 1_000_000)
}

/// Which step `code` belongs to at `now_s`, allowing for a little clock drift, and never a step at or before
/// `last_used` (so a code cannot be used twice). `None` when it matches nothing allowed.
pub fn verify(secret: &[u8], code: &str, now_s: u64, last_used: Option<u64>) -> Option<u64> {
    let code = code.trim().replace(' ', "");
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let now_step = i64::try_from(now_s / STEP_SECS).ok()?;
    (-DRIFT_STEPS..=DRIFT_STEPS)
        .filter_map(|d| u64::try_from(now_step + d).ok())
        .filter(|step| last_used.is_none_or(|last| *step > last))
        .find(|step| {
            let expected = code_for_step(secret, *step);
            // Compare every digit so the time taken does not say how many were right.
            expected.bytes().zip(code.bytes()).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
        })
}

/// The address an authenticator app reads (usually from a QR code; people can also type the key).
pub fn provisioning_uri(issuer: &str, account: &str, secret_base32: &str) -> String {
    let enc = |s: &str| {
        s.bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
                    char::from(b).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect::<String>()
    };
    format!(
        "otpauth://totp/{}:{}?secret={secret_base32}&issuer={}&algorithm=SHA1&digits=6&period={STEP_SECS}",
        enc(issuer),
        enc(account),
        enc(issuer)
    )
}

/// One recovery code as shown (`ABCDE-FGHJK`), made from ten random base32 characters.
pub fn recovery_code(random: &[u8; 10]) -> String {
    let text: String = random.iter().map(|b| char::from(ALPHABET[usize::from(b & 31)])).collect();
    format!("{}-{}", &text[..5], &text[5..])
}

/// A recovery code as typed (any case, with or without the dash, spaces ignored) in the form that is hashed.
pub fn normalize_recovery(code: &str) -> String {
    code.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_ascii_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RFC_SECRET: &[u8] = b"12345678901234567890";

    #[test]
    fn hotp_matches_the_rfc_4226_values() {
        let expected = ["755224", "287082", "359152", "969429", "338314", "254676", "287922", "162583", "399871", "520489"];
        for (counter, want) in expected.iter().enumerate() {
            assert_eq!(&code_for_step(RFC_SECRET, counter as u64), want, "counter {counter}");
        }
    }

    #[test]
    fn totp_matches_the_rfc_6238_time_points() {
        // The RFC's eight-digit values end in these six digits.
        for (time, want) in [(59u64, "287082"), (1_111_111_109, "081804"), (1_111_111_111, "050471"), (1_234_567_890, "005924"), (2_000_000_000, "279037")] {
            assert_eq!(code_for_step(RFC_SECRET, time / STEP_SECS), want, "time {time}");
        }
    }

    #[test]
    fn base32_round_trips_and_is_forgiving() {
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(base32_decode("MZXW6YTBOI").as_deref(), Some(b"foobar".as_slice()));
        assert_eq!(base32_decode("mzxw 6ytb-oi").as_deref(), Some(b"foobar".as_slice()));
        assert_eq!(base32_decode("not base32!"), None);
        for len in 0..12usize {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + 11) as u8).collect();
            assert_eq!(base32_decode(&base32_encode(&bytes)), Some(bytes));
        }
    }

    #[test]
    fn verify_allows_a_little_drift_and_refuses_reuse_and_junk() {
        let now = 1_700_000_000u64;
        let step = now / STEP_SECS;
        let code = code_for_step(RFC_SECRET, step);
        assert_eq!(verify(RFC_SECRET, &code, now, None), Some(step));
        assert_eq!(verify(RFC_SECRET, &code_for_step(RFC_SECRET, step - 1), now, None), Some(step - 1), "one step early");
        assert_eq!(verify(RFC_SECRET, &code_for_step(RFC_SECRET, step + 1), now, None), Some(step + 1), "one step late");
        assert_eq!(verify(RFC_SECRET, &code_for_step(RFC_SECRET, step - 3), now, None), None, "too old");
        assert_eq!(verify(RFC_SECRET, &code, now, Some(step)), None, "already used");
        assert_eq!(verify(RFC_SECRET, &code, now, Some(step - 1)), Some(step));
        for junk in ["", "12345", "1234567", "abcdef", "12 34 5x"] {
            assert_eq!(verify(RFC_SECRET, junk, now, None), None, "{junk}");
        }
        // Spaces in the middle are fine, as authenticator apps often show codes in two groups.
        let spaced = format!("{} {}", &code[..3], &code[3..]);
        assert_eq!(verify(RFC_SECRET, &spaced, now, None), Some(step));
    }

    #[test]
    fn the_provisioning_address_is_escaped() {
        let uri = provisioning_uri("Open Pomodoro", "a b@c", "ABCD");
        assert_eq!(uri, "otpauth://totp/Open%20Pomodoro:a%20b%40c?secret=ABCD&issuer=Open%20Pomodoro&algorithm=SHA1&digits=6&period=30");
    }

    #[test]
    fn recovery_codes_read_back_however_they_are_typed() {
        let code = recovery_code(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
        assert_eq!(code.len(), 11);
        assert_eq!(normalize_recovery(&code.to_lowercase()), normalize_recovery(&code));
        assert_eq!(normalize_recovery(" ab-cd ef "), "ABCDEF");
    }
}
