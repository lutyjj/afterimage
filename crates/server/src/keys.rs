//! Random tokens (device access keys, session and sign-in cookies), the hash the
//! database holds in their place, and friendly IDs.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use sha2::{Digest, Sha256};

/// Where randomness comes from, so that tests can script it.
pub trait Entropy: Send + Sync + 'static {
    fn fill(&self, buffer: &mut [u8]);
}

/// The operating system's random number generator.
pub struct OsEntropy;

impl Entropy for OsEntropy {
    fn fill(&self, buffer: &mut [u8]) {
        getrandom::fill(buffer).expect("the operating system provides randomness");
    }
}

/// A new token: 256 random bits as unpadded base64url.
pub fn new_token(entropy: &dyn Entropy) -> String {
    let mut bytes = [0; 32];
    entropy.fill(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// What the server stores in place of a token.
#[must_use]
pub fn token_hash(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// Symbols for friendly IDs: digits and uppercase letters without 0, 1, I and O,
/// which read alike on a small screen. 32 symbols, so a random byte maps without bias.
const FRIENDLY_ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// Length of a friendly ID: 30 bits, typed by hand to claim a device.
pub const FRIENDLY_ID_LENGTH: usize = 6;

/// A new friendly ID, such as `K7QM2X`.
pub fn new_friendly_id(entropy: &dyn Entropy) -> String {
    let mut bytes = [0; FRIENDLY_ID_LENGTH];
    entropy.fill(&mut bytes);
    bytes
        .iter()
        .map(|byte| char::from(FRIENDLY_ALPHABET[usize::from(byte % 32)]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed(u8);

    impl Entropy for Fixed {
        fn fill(&self, buffer: &mut [u8]) {
            buffer.fill(self.0);
        }
    }

    #[test]
    fn tokens_carry_256_bits_in_url_safe_text() {
        let key = new_token(&OsEntropy);
        assert_eq!(key.len(), 43);
        assert!(
            key.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        );
        assert_ne!(key, new_token(&OsEntropy));
    }

    #[test]
    fn hashes_tokens_with_sha256() {
        // SHA-256 of "abc", from FIPS 180-2 appendix B.1.
        assert_eq!(
            token_hash("abc"),
            [
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
                0xf2, 0x00, 0x15, 0xad
            ]
        );
    }

    #[test]
    fn friendly_ids_use_unambiguous_symbols() {
        assert_eq!(new_friendly_id(&Fixed(0)), "222222");
        assert_eq!(new_friendly_id(&Fixed(31)), "ZZZZZZ");
        assert_eq!(new_friendly_id(&Fixed(32)), "222222");
        let id = new_friendly_id(&OsEntropy);
        assert_eq!(id.len(), FRIENDLY_ID_LENGTH);
        assert!(id.bytes().all(|b| FRIENDLY_ALPHABET.contains(&b)));
    }
}
