//! Uploaded alert sounds: which slots exist, and what counts as an audio file we accept.
//!
//! The type is decided from the first bytes of the file, never from what the client says it is, so a page or
//! script cannot be stored and served back under an audio name.

/// The alert slots a sound can replace. They match the tones the app plays on its own.
pub const SLOTS: [&str; 6] = [
    "warn",
    "focus_end",
    "break_end",
    "focus_reminder",
    "break_reminder",
    "nudge",
];

/// Largest upload (512 KiB): an alert is a second or two of sound.
pub const MAX_BYTES: usize = 512 * 1024;

/// The audio type of `bytes` by its signature, or `None` when it is not a format browsers play.
pub fn sniff(bytes: &[u8]) -> Option<&'static str> {
    let starts = |prefix: &[u8]| bytes.starts_with(prefix);
    if starts(b"ID3") || (bytes.len() > 2 && bytes[0] == 0xFF && bytes[1] & 0xE0 == 0xE0) {
        Some("audio/mpeg")
    } else if starts(b"OggS") {
        Some("audio/ogg")
    } else if starts(b"RIFF") && bytes.get(8..12) == Some(b"WAVE".as_slice()) {
        Some("audio/wav")
    } else if starts(b"fLaC") {
        Some("audio/flac")
    } else if starts(&[0x1A, 0x45, 0xDF, 0xA3]) {
        Some("audio/webm")
    } else if bytes.get(4..8) == Some(b"ftyp".as_slice()) {
        Some("audio/mp4")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_audio_signatures_are_recognised() {
        assert_eq!(sniff(b"ID3\x04rest"), Some("audio/mpeg"));
        assert_eq!(sniff(&[0xFF, 0xFB, 0x90, 0x00]), Some("audio/mpeg"));
        assert_eq!(sniff(b"OggS\0\x02"), Some("audio/ogg"));
        assert_eq!(sniff(b"RIFF\x24\0\0\0WAVEfmt "), Some("audio/wav"));
        assert_eq!(sniff(b"fLaC\0\0"), Some("audio/flac"));
        assert_eq!(sniff(&[0x1A, 0x45, 0xDF, 0xA3, 1]), Some("audio/webm"));
        assert_eq!(sniff(b"\0\0\0\x20ftypM4A "), Some("audio/mp4"));
    }

    #[test]
    fn everything_else_is_refused() {
        for bytes in [
            b"".as_slice(),
            b"<html><script>alert(1)</script>",
            b"#!/bin/sh\n",
            b"RIFF\x24\0\0\0AVI LIST",
            b"\x89PNG\r\n",
            &[0xFF],
        ] {
            assert_eq!(sniff(bytes), None, "{bytes:?}");
        }
    }
}
