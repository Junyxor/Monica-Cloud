//! Server-side primitives only: password hashing, tokens and digests.
//! Vault encryption happens exclusively on clients.

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use data_encoding::{BASE64URL_NOPAD, HEXLOWER};
use rand::RngCore;
use sha2::{Digest, Sha256};

pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut bytes = [0u8; N];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    bytes
}

/// URL-safe random token with `N` bytes of entropy.
pub fn random_token<const N: usize>() -> String {
    BASE64URL_NOPAD.encode(&random_bytes::<N>())
}

pub fn sha256_hex(data: &[u8]) -> String {
    HEXLOWER.encode(&Sha256::digest(data))
}

/// Argon2id with the crate defaults (19 MiB, t=2, p=1), the OWASP baseline.
pub fn hash_password(secret: &str) -> String {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .expect("argon2 hashing with default parameters cannot fail")
        .to_string()
}

pub fn verify_password(secret: &str, phc: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(parsed) => Argon2::default().verify_password(secret.as_bytes(), &parsed).is_ok(),
        Err(_) => false,
    }
}

/// Burn the same time as a real verification so missing accounts are not observable.
pub fn dummy_verify() {
    static DUMMY: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| hash_password("monica-cloud-dummy"));
    let _ = verify_password("not-the-password", &DUMMY);
}

/// Checks that an authHash / send passwordHash is base64 of exactly 32 bytes.
pub fn is_valid_auth_hash(value: &str) -> bool {
    data_encoding::BASE64.decode(value.as_bytes()).map(|b| b.len() == 32).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argon2_round_trip() {
        let phc = hash_password("secret");
        assert!(verify_password("secret", &phc));
        assert!(!verify_password("other", &phc));
    }

    #[test]
    fn auth_hash_shape() {
        assert!(is_valid_auth_hash(&data_encoding::BASE64.encode(&[7u8; 32])));
        assert!(!is_valid_auth_hash(&data_encoding::BASE64.encode(&[7u8; 31])));
        assert!(!is_valid_auth_hash("password"));
    }
}
