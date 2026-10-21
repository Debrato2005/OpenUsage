//! Passphrase protection for a random database key. The caller controls fallback eligibility and file storage.
//! Version 1 authenticates its version, Argon2id parameters, salt, nonce, and encrypted key.
//! The caller must clear its passphrase buffer after this module returns.

use argon2::{Algorithm, Argon2, Block, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use std::fmt;
use zeroize::Zeroizing;

const MAGIC: &[u8; 8] = b"OUKEYWRP";
const FORMAT_VERSION: u16 = 1;
const MEMORY_KIB: u32 = 64 * 1024;
const PASSES: u32 = 3;
const LANES: u32 = 1;
const HEADER_SIZE: usize = 64;
pub const WRAPPED_KEY_SIZE: usize = HEADER_SIZE + 32 + 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeywrapError {
    InvalidPassphrase,
    InvalidEnvelope,
    UnsupportedFormat,
    AuthenticationFailed,
    RandomnessUnavailable,
    KeyDerivationFailed,
}

impl fmt::Display for KeywrapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPassphrase => "Use a passphrase between 1 and 1024 bytes.",
            Self::InvalidEnvelope => "The protected database key file is invalid or incomplete.",
            Self::UnsupportedFormat => "The protected database key format or parameters are unsupported.",
            Self::AuthenticationFailed => "The database key could not be unlocked. Check the passphrase and preserve the existing files.",
            Self::RandomnessUnavailable => "Secure random bytes are unavailable.",
            Self::KeyDerivationFailed => "The passphrase encryption key could not be derived.",
        })
    }
}

impl std::error::Error for KeywrapError {}

pub fn generate_database_key() -> Result<Zeroizing<[u8; 32]>, KeywrapError> {
    let mut key = Zeroizing::new([0u8; 32]);
    getrandom::fill(key.as_mut()).map_err(|_| KeywrapError::RandomnessUnavailable)?;
    Ok(key)
}

/// Return encrypted file bytes. This function never saves a key or passphrase.
pub fn wrap_key(key: &[u8; 32], passphrase: &[u8]) -> Result<Vec<u8>, KeywrapError> {
    validate_passphrase(passphrase)?;
    let mut header = [0u8; HEADER_SIZE];
    header[..8].copy_from_slice(MAGIC);
    header[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    header[10] = 1; // Argon2id algorithm identifier.
    header[11] = 0x13; // Argon2 version 1.3.
    header[12..16].copy_from_slice(&MEMORY_KIB.to_le_bytes());
    header[16..20].copy_from_slice(&PASSES.to_le_bytes());
    header[20..24].copy_from_slice(&LANES.to_le_bytes());
    getrandom::fill(&mut header[24..]).map_err(|_| KeywrapError::RandomnessUnavailable)?;
    let wrapping_key = derive_key(passphrase, &header[24..40])?;
    let cipher = XChaCha20Poly1305::new_from_slice(wrapping_key.as_ref())
        .map_err(|_| KeywrapError::KeyDerivationFailed)?;
    let nonce = XNonce::from(<[u8; 24]>::try_from(&header[40..64]).unwrap());
    let encrypted = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: key,
                aad: &header,
            },
        )
        .map_err(|_| KeywrapError::AuthenticationFailed)?;
    let mut envelope = Vec::with_capacity(WRAPPED_KEY_SIZE);
    envelope.extend_from_slice(&header);
    envelope.extend_from_slice(&encrypted);
    Ok(envelope)
}

/// Reject unknown formats and KDF costs before allocation or key derivation.
/// A failure never changes the wrapped key, database, or caller-owned files.
pub fn unwrap_key(envelope: &[u8], passphrase: &[u8]) -> Result<Zeroizing<[u8; 32]>, KeywrapError> {
    validate_passphrase(passphrase)?;
    if envelope.len() != WRAPPED_KEY_SIZE {
        return Err(KeywrapError::InvalidEnvelope);
    }
    if &envelope[..8] != MAGIC
        || envelope[8..10] != FORMAT_VERSION.to_le_bytes()
        || envelope[10] != 1
        || envelope[11] != 0x13
        || envelope[12..16] != MEMORY_KIB.to_le_bytes()
        || envelope[16..20] != PASSES.to_le_bytes()
        || envelope[20..24] != LANES.to_le_bytes()
    {
        return Err(KeywrapError::UnsupportedFormat);
    }
    let wrapping_key = derive_key(passphrase, &envelope[24..40])?;
    let cipher = XChaCha20Poly1305::new_from_slice(wrapping_key.as_ref())
        .map_err(|_| KeywrapError::KeyDerivationFailed)?;
    let nonce = XNonce::from(<[u8; 24]>::try_from(&envelope[40..64]).unwrap());
    let plaintext = Zeroizing::new(
        cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: &envelope[HEADER_SIZE..],
                    aad: &envelope[..HEADER_SIZE],
                },
            )
            .map_err(|_| KeywrapError::AuthenticationFailed)?,
    );
    if plaintext.len() != 32 {
        return Err(KeywrapError::InvalidEnvelope);
    }
    let mut key = Zeroizing::new([0u8; 32]);
    key.copy_from_slice(&plaintext);
    Ok(key)
}

fn validate_passphrase(passphrase: &[u8]) -> Result<(), KeywrapError> {
    if passphrase.is_empty() || passphrase.len() > 1024 {
        return Err(KeywrapError::InvalidPassphrase);
    }
    Ok(())
}

fn derive_key(passphrase: &[u8], salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, KeywrapError> {
    let params = Params::new(MEMORY_KIB, PASSES, LANES, Some(32))
        .map_err(|_| KeywrapError::KeyDerivationFailed)?;
    // Wipe caller-owned KDF memory. The library's allocating helper only deallocates that memory.
    let mut memory = Zeroizing::new(Vec::new());
    memory
        .try_reserve_exact(params.block_count())
        .map_err(|_| KeywrapError::KeyDerivationFailed)?;
    memory.resize_with(params.block_count(), Block::default);
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into_with_memory(passphrase, salt, key.as_mut(), memory.as_mut_slice())
        .map_err(|_| KeywrapError::KeyDerivationFailed)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PASSPHRASE: &[u8] = b"synthetic database passphrase";

    #[test]
    fn random_database_key_round_trips_with_unique_salt_and_nonce() {
        let key = generate_database_key().unwrap();
        let first = wrap_key(&key, PASSPHRASE).unwrap();
        let second = wrap_key(&key, PASSPHRASE).unwrap();
        assert_ne!(&first[24..40], &second[24..40]);
        assert_ne!(&first[40..64], &second[40..64]);
        assert!(unwrap_key(&first, PASSPHRASE).unwrap().as_ref() == key.as_ref());
        assert!(unwrap_key(&second, PASSPHRASE).unwrap().as_ref() == key.as_ref());
    }

    #[test]
    fn wrong_passphrase_and_tampered_salt_nonce_or_ciphertext_reject_without_changing_envelope() {
        let envelope = wrap_key(&[0x47; 32], PASSPHRASE).unwrap();
        let original = envelope.clone();
        assert!(matches!(
            unwrap_key(&envelope, b"wrong passphrase"),
            Err(KeywrapError::AuthenticationFailed)
        ));
        for offset in [24, 40, HEADER_SIZE, WRAPPED_KEY_SIZE - 1] {
            let mut tampered = envelope.clone();
            tampered[offset] ^= 1;
            assert!(matches!(
                unwrap_key(&tampered, PASSPHRASE),
                Err(KeywrapError::AuthenticationFailed)
            ));
        }
        assert_eq!(envelope, original);
    }

    #[test]
    fn invalid_formats_costs_lengths_and_passphrases_reject_before_expensive_derivation() {
        let envelope = wrap_key(&[0x17; 32], PASSPHRASE).unwrap();
        for offset in [0, 8, 10, 11, 12, 16, 20] {
            let mut invalid = envelope.clone();
            invalid[offset] ^= 0xff;
            assert!(matches!(
                unwrap_key(&invalid, PASSPHRASE),
                Err(KeywrapError::UnsupportedFormat)
            ));
        }
        assert!(matches!(
            unwrap_key(&envelope[..envelope.len() - 1], PASSPHRASE),
            Err(KeywrapError::InvalidEnvelope)
        ));
        let mut extended = envelope.clone();
        extended.push(0);
        assert!(matches!(
            unwrap_key(&extended, PASSPHRASE),
            Err(KeywrapError::InvalidEnvelope)
        ));
        assert!(matches!(
            wrap_key(&[0x17; 32], b""),
            Err(KeywrapError::InvalidPassphrase)
        ));
        assert!(matches!(
            unwrap_key(&envelope, &vec![0x61; 1025]),
            Err(KeywrapError::InvalidPassphrase)
        ));
    }
}
