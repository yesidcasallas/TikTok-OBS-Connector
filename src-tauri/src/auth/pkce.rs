//! PKCE (RFC 7636) con método `S256`.
//!
//! El `code_verifier` son 64 bytes del generador criptográfico del sistema
//! codificados en hexadecimal (128 caracteres), igual que Streamlabs Desktop.
//! Cumple el RFC: 43–128 caracteres del conjunto no reservado.
//! El `code_challenge` es `BASE64URL(SHA256(verifier))` sin relleno.

use crate::error::{AppError, AppResult};
use crate::security::Secret;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

const VERIFIER_BYTES: usize = 64;

pub struct Pkce {
    pub verifier: Secret,
    pub challenge: String,
}

impl Pkce {
    pub fn generate() -> AppResult<Self> {
        let mut bytes = [0u8; VERIFIER_BYTES];
        getrandom::fill(&mut bytes)
            .map_err(|e| AppError::Internal(format!("generador aleatorio no disponible: {e}")))?;
        let mut hex = String::with_capacity(VERIFIER_BYTES * 2);
        for b in bytes {
            hex.push(char::from_digit((b >> 4) as u32, 16).unwrap());
            hex.push(char::from_digit((b & 0x0f) as u32, 16).unwrap());
        }
        bytes.zeroize();
        let challenge = challenge_for(&hex);
        Ok(Pkce { verifier: Secret::new(hex), challenge })
    }
}

/// `BASE64URL-ENCODE(SHA256(ASCII(code_verifier)))` sin `=`.
pub fn challenge_for(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc7636_appendix_b_vector() {
        assert_eq!(
            challenge_for("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn verifier_has_valid_length_and_charset() {
        let p = Pkce::generate().unwrap();
        let v = p.verifier.expose();
        assert_eq!(v.len(), 128);
        assert!((43..=128).contains(&v.len()));
        assert!(v.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn challenge_is_base64url_without_padding() {
        let p = Pkce::generate().unwrap();
        assert_eq!(p.challenge.len(), 43);
        assert!(p
            .challenge
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_eq!(p.challenge, challenge_for(p.verifier.expose()));
    }

    #[test]
    fn verifiers_are_unique() {
        let a = Pkce::generate().unwrap();
        let b = Pkce::generate().unwrap();
        assert_ne!(a.verifier.expose(), b.verifier.expose());
        assert_ne!(a.challenge, b.challenge);
    }
}
