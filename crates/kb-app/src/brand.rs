//! The browser's name and logo.

use std::sync::OnceLock;

/// The browser's name, as shown to the reader.
pub const NAME: &str = "Primitive Browser";

/// The logo (PNG). It is part of the binary, so it shows offline.
const LOGO_PNG: &[u8] = include_bytes!("../../../assets/primitive-icon.png");

/// The logo as a data URL, for the browser's own pages.
pub fn logo_data_url() -> &'static str {
    static URL: OnceLock<String> = OnceLock::new();
    URL.get_or_init(|| format!("data:image/png;base64,{}", base64(LOGO_PNG)))
}

/// Standard base64 with padding.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |acc, (i, b)| acc | (u32::from(*b) << (16 - 8 * i)));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(ALPHABET[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_encoding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"h"), "aA==");
        assert_eq!(base64(b"hi"), "aGk=");
        assert_eq!(base64(b"hi!"), "aGkh");
        assert!(logo_data_url().starts_with("data:image/png;base64,iVBORw0KGgo"));
    }
}
