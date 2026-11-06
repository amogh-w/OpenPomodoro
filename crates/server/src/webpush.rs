//! Web Push: encrypting a message for a browser's push subscription (RFC 8291, `aes128gcm`) and signing the request
//! with the server's own VAPID key (RFC 8292), so the browser's push service accepts it. Pure functions; the sending
//! lives in `push.rs`.

use aes_gcm::{Aes128Gcm, KeyInit, aead::Aead};
use p256::{
    PublicKey, SecretKey,
    ecdh::diffie_hellman,
    ecdsa::{Signature, SigningKey, signature::Signer},
};
use sha2::{Digest, Sha256};

/// Largest message a push service must accept is 4096 bytes in all; keep ours well below.
pub const MAX_PAYLOAD: usize = 2000;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// URL-safe base64 without padding, the form push uses everywhere.
pub fn b64_encode(bytes: &[u8]) -> String {
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..=chunk.len() {
            out.push(char::from(B64[((n >> (18 - 6 * i)) & 63) as usize]));
        }
    }
    out
}

pub fn b64_decode(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let (mut buffer, mut bits) = (0u32, 0u32);
    for ch in text.trim_end_matches('=').bytes() {
        let value = u32::try_from(B64.iter().position(|&b| b == ch)?).ok()?;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            out.push(u8::try_from((buffer >> (bits - 8)) & 0xff).ok()?);
            bits -= 8;
        }
    }
    Some(out)
}

fn hmac_sha256(key: &[u8], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut key = key.to_vec();
    if key.len() > BLOCK {
        key = Sha256::digest(&key).to_vec();
    }
    key.resize(BLOCK, 0);
    let pad = |byte: u8| key.iter().map(|k| k ^ byte).collect::<Vec<u8>>();
    let mut inner = Sha256::new();
    inner.update(pad(0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(pad(0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

/// HKDF (RFC 5869) with SHA-256: extract then expand to `len` bytes (at most 32 here).
fn hkdf(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    let prk = hmac_sha256(salt, ikm);
    let mut block = info.to_vec();
    block.push(1);
    hmac_sha256(&prk, &block)[..len].to_vec()
}

#[derive(Debug, PartialEq, Eq)]
pub enum PushError {
    /// The subscription's key or auth secret is not valid.
    BadSubscription,
    /// The message is too long.
    TooLong,
    Crypto,
}

/// The encrypted request body for one message. `ephemeral` is a fresh private key and `salt` 16 fresh random bytes for
/// every message (they are parameters so the RFC's own example can be checked).
pub fn encrypt(
    plaintext: &[u8],
    ua_public: &[u8],
    auth_secret: &[u8],
    ephemeral: &SecretKey,
    salt: &[u8; 16],
) -> Result<Vec<u8>, PushError> {
    if plaintext.len() > MAX_PAYLOAD {
        return Err(PushError::TooLong);
    }
    if auth_secret.len() != 16 {
        return Err(PushError::BadSubscription);
    }
    let ua = PublicKey::from_sec1_bytes(ua_public).map_err(|_| PushError::BadSubscription)?;
    let as_public = ephemeral.public_key().to_sec1_bytes();
    let shared = diffie_hellman(ephemeral.to_nonzero_scalar(), ua.as_affine());

    let mut key_info = b"WebPush: info\0".to_vec();
    key_info.extend_from_slice(ua_public);
    key_info.extend_from_slice(&as_public);
    let ikm = hkdf(
        auth_secret,
        shared.raw_secret_bytes().as_slice(),
        &key_info,
        32,
    );
    let cek = hkdf(salt, &ikm, b"Content-Encoding: aes128gcm\0", 16);
    let nonce = hkdf(salt, &ikm, b"Content-Encoding: nonce\0", 12);

    let mut padded = plaintext.to_vec();
    padded.push(0x02); // the last (and only) record
    let cipher = Aes128Gcm::new_from_slice(&cek).map_err(|_| PushError::Crypto)?;
    let nonce: [u8; 12] = nonce.try_into().map_err(|_| PushError::Crypto)?;
    let sealed = cipher
        .encrypt(&nonce.into(), padded.as_slice())
        .map_err(|_| PushError::Crypto)?;

    let mut body = Vec::with_capacity(21 + as_public.len() + sealed.len());
    body.extend_from_slice(salt);
    body.extend_from_slice(&4096u32.to_be_bytes());
    body.push(u8::try_from(as_public.len()).map_err(|_| PushError::Crypto)?);
    body.extend_from_slice(&as_public);
    body.extend_from_slice(&sealed);
    Ok(body)
}

/// The `Authorization` header for a push request: a short-lived signed token naming the push service and this server
/// (RFC 8292), plus the server's public key.
pub fn vapid_header(key: &SigningKey, audience: &str, subject: &str, expires_at_s: u64) -> String {
    let header = b64_encode(br#"{"typ":"JWT","alg":"ES256"}"#);
    let claims = b64_encode(
        format!(r#"{{"aud":"{audience}","exp":{expires_at_s},"sub":"{subject}"}}"#).as_bytes(),
    );
    let signing_input = format!("{header}.{claims}");
    let signature: Signature = key.sign(signing_input.as_bytes());
    let public = key.verifying_key().to_sec1_point(false);
    format!(
        "vapid t={signing_input}.{}, k={}",
        b64_encode(&signature.to_bytes()),
        b64_encode(public.as_bytes())
    )
}

/// The public key browsers need to subscribe (the `applicationServerKey`).
pub fn public_key_b64(key: &SigningKey) -> String {
    b64_encode(key.verifying_key().to_sec1_point(false).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_url_round_trips() {
        for len in 0..40usize {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 53 + 7) as u8).collect();
            assert_eq!(b64_decode(&b64_encode(&bytes)), Some(bytes));
        }
        assert_eq!(b64_encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(b64_decode("not valid!"), None);
    }

    /// The worked example in RFC 8291, section 5 and appendix A.
    #[test]
    fn the_rfc_8291_example_encrypts_to_the_documented_body() {
        let as_private = SecretKey::from_slice(
            &b64_decode("yfWPiYE-n46HLnH0KqZOF1fJJU3MYrct3AELtAQ-oRw").expect("key"),
        )
        .expect("secret");
        let ua_public = b64_decode("BCVxsr7N_eNgVRqvHtD0zTZsEc6-VV-JvLexhqUzORcxaOzi6-AYWXvTBHm4bjyPjs7Vd8pZGH6SRpkNtoIAiw4").expect("ua");
        let auth = b64_decode("BTBZMqHH6r4Tts7J_aSIgg").expect("auth");
        let salt: [u8; 16] = b64_decode("DGv6ra1nlYgDCS1FRnbzlw")
            .expect("salt")
            .try_into()
            .expect("16");
        let body = encrypt(
            b"When I grow up, I want to be a watermelon",
            &ua_public,
            &auth,
            &as_private,
            &salt,
        )
        .expect("encrypt");
        assert_eq!(
            b64_encode(&body),
            "DGv6ra1nlYgDCS1FRnbzlwAAEABBBP4z9KsN6nGRTbVYI_c7VJSPQTBtkgcy27mlmlMoZIIgDll6e3vCYLocInmYWAmS6TlzAC8wEqKK6PBru3jl7A_yl95bQpu6cVPTpK4Mqgkf1CXztLVBSt2Ks3oZwbuwXPXLWyouBWLVWGNWQexSgSxsj_Qulcy4a-fN"
        );
    }

    #[test]
    fn bad_subscriptions_and_long_messages_are_refused() {
        let key = SecretKey::from_slice(&[7u8; 32]).expect("key");
        let ua = SecretKey::from_slice(&[9u8; 32])
            .expect("ua")
            .public_key()
            .to_sec1_bytes();
        let salt = [1u8; 16];
        assert_eq!(
            encrypt(b"x", &[1, 2, 3], &[0; 16], &key, &salt),
            Err(PushError::BadSubscription)
        );
        assert_eq!(
            encrypt(b"x", &ua, &[0; 15], &key, &salt),
            Err(PushError::BadSubscription)
        );
        assert_eq!(
            encrypt(&vec![b'a'; MAX_PAYLOAD + 1], &ua, &[0; 16], &key, &salt),
            Err(PushError::TooLong)
        );
        assert!(encrypt(b"fine", &ua, &[0; 16], &key, &salt).is_ok());
    }

    #[test]
    fn the_vapid_header_names_the_service_and_verifies() {
        use p256::ecdsa::signature::Verifier;
        let key = SigningKey::from_slice(&[5u8; 32]).expect("key");
        let header = vapid_header(
            &key,
            "https://push.example",
            "mailto:admin@example.com",
            1_700_000_000,
        );
        let (token, public) = header
            .strip_prefix("vapid t=")
            .and_then(|r| r.split_once(", k="))
            .expect("shape");
        assert_eq!(public, public_key_b64(&key));
        let (signing_input, sig) = token.rsplit_once('.').expect("jwt");
        let claims = String::from_utf8(
            b64_decode(signing_input.split('.').nth(1).expect("claims")).expect("b64"),
        )
        .expect("utf8");
        assert_eq!(
            claims,
            r#"{"aud":"https://push.example","exp":1700000000,"sub":"mailto:admin@example.com"}"#
        );
        let signature = Signature::from_slice(&b64_decode(sig).expect("sig")).expect("64 bytes");
        assert!(
            key.verifying_key()
                .verify(signing_input.as_bytes(), &signature)
                .is_ok()
        );
    }
}
