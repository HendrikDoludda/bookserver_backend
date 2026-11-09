//! Encrypting TOTP secrets at rest.

use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{
    aead::{Aead, Payload},
    Key, KeyInit, XChaCha20Poly1305, XNonce,
};
use rand::{rngs::SysRng, TryRng};

use crate::error_types::AuthenticationError;

// BLAKE3 derives an unrelated key for every context string, so no other use of the signing key
// can ever end up with this one. Changing the string makes every stored secret unreadable.
const KEY_CONTEXT: &str = "bookserver_backend 2026-11-09 TOTP secrets at rest";
const NONCE_BYTES: usize = 24;
const TAG_BYTES: usize = 16;

/// Encrypts and decrypts TOTP secrets with XChaCha20-Poly1305, under a key derived from the
/// server's signing key. No `Debug`: it holds a key.
#[derive(Clone)]
pub struct TotpCipher(XChaCha20Poly1305);

impl TotpCipher {
    pub fn from_signing_key(signing_key: &[u8; 32]) -> Self {
        let key = blake3::derive_key(KEY_CONTEXT, signing_key);
        TotpCipher(XChaCha20Poly1305::new(&Key::from(key)))
    }

    /// The secret as it is stored: base64 of nonce, ciphertext and tag.
    pub fn encrypt(&self, user_id: i64, secret: &[u8]) -> Result<String, AuthenticationError> {
        let mut nonce = [0u8; NONCE_BYTES];
        SysRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| AuthenticationError::RngCreatorFailed)?;
        let aad = associated_data(user_id);
        let sealed = self
            .0
            .encrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: secret,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| AuthenticationError::TotpFailure)?;
        let mut stored = nonce.to_vec();
        stored.extend_from_slice(&sealed);
        Ok(STANDARD.encode(stored))
    }

    /// The secret behind a stored value. Fails for a value that was changed, belongs to another
    /// user, or was encrypted under another signing key.
    pub fn decrypt(&self, user_id: i64, stored: &str) -> Result<Vec<u8>, AuthenticationError> {
        let unreadable = || {
            tracing::error!(
                "the TOTP secret of user {user_id} does not decrypt; was secrets.toml replaced?"
            );
            AuthenticationError::TotpFailure
        };
        let bytes = STANDARD.decode(stored).map_err(|_| unreadable())?;
        if bytes.len() < NONCE_BYTES + TAG_BYTES {
            return Err(unreadable());
        }
        let (nonce, sealed) = bytes.split_at(NONCE_BYTES);
        let nonce: [u8; NONCE_BYTES] = nonce.try_into().map_err(|_| unreadable())?;
        let aad = associated_data(user_id);
        self.0
            .decrypt(
                &XNonce::from(nonce),
                Payload {
                    msg: sealed,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| unreadable())
    }
}

/// Authenticated along with the secret, so a secret copied into another user's row doesn't
/// decrypt there.
fn associated_data(user_id: i64) -> String {
    format!("totp_secret:{user_id}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_cipher as cipher;

    const SECRET: &[u8; 20] = b"twenty bytes secret!";

    fn is_unreadable(result: Result<Vec<u8>, AuthenticationError>) -> bool {
        matches!(result, Err(AuthenticationError::TotpFailure))
    }

    #[test]
    fn a_secret_decrypts_to_itself() {
        let stored = cipher().encrypt(42, SECRET).unwrap();
        assert_eq!(cipher().decrypt(42, &stored).unwrap(), SECRET);
    }

    #[test]
    fn the_stored_value_is_nonce_ciphertext_and_tag_in_base64() {
        let stored = cipher().encrypt(42, SECRET).unwrap();
        let bytes = STANDARD.decode(&stored).unwrap();
        assert_eq!(bytes.len(), NONCE_BYTES + SECRET.len() + TAG_BYTES);
        assert!(
            !bytes.windows(SECRET.len()).any(|window| window == SECRET),
            "the plain secret must not appear in the stored value"
        );
    }

    #[test]
    fn every_encryption_uses_a_fresh_nonce() {
        let first = cipher().encrypt(42, SECRET).unwrap();
        let second = cipher().encrypt(42, SECRET).unwrap();
        assert_ne!(first, second);
        assert_eq!(cipher().decrypt(42, &second).unwrap(), SECRET);
    }

    #[test]
    fn a_changed_byte_fails() {
        let stored = cipher().encrypt(42, SECRET).unwrap();
        let mut bytes = STANDARD.decode(&stored).unwrap();
        for position in [0, NONCE_BYTES, bytes.len() - 1] {
            bytes[position] ^= 1;
            assert!(
                is_unreadable(cipher().decrypt(42, &STANDARD.encode(&bytes))),
                "a change at byte {position} must be noticed"
            );
            bytes[position] ^= 1;
        }
    }

    #[test]
    fn another_users_secret_fails() {
        let stored = cipher().encrypt(42, SECRET).unwrap();
        assert!(is_unreadable(cipher().decrypt(43, &stored)));
    }

    #[test]
    fn another_signing_key_fails() {
        let stored = cipher().encrypt(42, SECRET).unwrap();
        let other = TotpCipher::from_signing_key(&[8; 32]);
        assert!(is_unreadable(other.decrypt(42, &stored)));
    }

    #[test]
    fn garbage_fails_without_a_panic() {
        for stored in ["", "not base64!", "c2hvcnQ=", &STANDARD.encode([0u8; 39])] {
            assert!(is_unreadable(cipher().decrypt(42, stored)), "{stored:?}");
        }
    }
}
