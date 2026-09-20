//! Institutional signatures (spec Section 3: "a simple registered-keypair
//! allowlist," not real-world KYC — but the signature check itself is real
//! Ed25519, not a placeholder "is this field non-empty" check). A
//! `VerifiedInstitution` publishes under a signature over its content
//! hash; the server verifies it against the institution's registered
//! public key before granting `institutionally_verified` status.

use ed25519_dalek::{Signer, SigningKey, Verifier, VerifyingKey};
use rand::rngs::OsRng;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignatureError {
    #[error("invalid public key encoding")]
    InvalidPublicKey,
    #[error("invalid signature encoding")]
    InvalidSignatureEncoding,
    #[error("signature does not verify against the given public key and message")]
    VerificationFailed,
}

/// Generates a fresh Ed25519 keypair, hex-encoded, for onboarding a demo
/// institution (used by the admin onboarding flow and the seed script —
/// the private key is handed to the institution, never stored server-side).
pub fn generate_keypair() -> (String, String) {
    let signing_key = SigningKey::generate(&mut OsRng);
    let private_hex = hex::encode(signing_key.to_bytes());
    let public_hex = hex::encode(signing_key.verifying_key().to_bytes());
    (private_hex, public_hex)
}

pub fn sign_hex(private_key_hex: &str, message: &[u8]) -> Result<String, SignatureError> {
    let key_bytes = hex::decode(private_key_hex).map_err(|_| SignatureError::InvalidPublicKey)?;
    let key_arr: [u8; 32] = key_bytes.try_into().map_err(|_| SignatureError::InvalidPublicKey)?;
    let signing_key = SigningKey::from_bytes(&key_arr);
    let signature = signing_key.sign(message);
    Ok(hex::encode(signature.to_bytes()))
}

pub fn is_valid_public_key_hex(public_key_hex: &str) -> bool {
    hex::decode(public_key_hex)
        .map(|bytes| bytes.len() == 32)
        .unwrap_or(false)
}

pub fn verify_hex(public_key_hex: &str, message: &[u8], signature_hex: &str) -> Result<(), SignatureError> {
    let pk_bytes = hex::decode(public_key_hex).map_err(|_| SignatureError::InvalidPublicKey)?;
    let pk_arr: [u8; 32] = pk_bytes.try_into().map_err(|_| SignatureError::InvalidPublicKey)?;
    let verifying_key = VerifyingKey::from_bytes(&pk_arr).map_err(|_| SignatureError::InvalidPublicKey)?;

    let sig_bytes = hex::decode(signature_hex).map_err(|_| SignatureError::InvalidSignatureEncoding)?;
    let sig_arr: [u8; 64] = sig_bytes.try_into().map_err(|_| SignatureError::InvalidSignatureEncoding)?;
    let signature = ed25519_dalek::Signature::from_bytes(&sig_arr);

    verifying_key
        .verify(message, &signature)
        .map_err(|_| SignatureError::VerificationFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_signature_verifies() {
        let (sk, pk) = generate_keypair();
        let sig = sign_hex(&sk, b"content-hash-bytes").unwrap();
        assert!(verify_hex(&pk, b"content-hash-bytes", &sig).is_ok());
    }

    #[test]
    fn tampered_message_fails_verification() {
        let (sk, pk) = generate_keypair();
        let sig = sign_hex(&sk, b"original message").unwrap();
        assert_eq!(
            verify_hex(&pk, b"tampered message", &sig),
            Err(SignatureError::VerificationFailed)
        );
    }

    #[test]
    fn wrong_public_key_fails_verification() {
        let (sk, _pk) = generate_keypair();
        let (_other_sk, other_pk) = generate_keypair();
        let sig = sign_hex(&sk, b"message").unwrap();
        assert_eq!(
            verify_hex(&other_pk, b"message", &sig),
            Err(SignatureError::VerificationFailed)
        );
    }

    #[test]
    fn validates_public_key_length() {
        let (_sk, pk) = generate_keypair();
        assert!(is_valid_public_key_hex(&pk));
        assert!(!is_valid_public_key_hex("not-hex"));
        assert!(!is_valid_public_key_hex("aabb"));
    }

    #[test]
    fn malformed_public_key_is_rejected() {
        let (sk, _pk) = generate_keypair();
        let sig = sign_hex(&sk, b"message").unwrap();
        assert_eq!(
            verify_hex("not-hex", b"message", &sig),
            Err(SignatureError::InvalidPublicKey)
        );
    }
}
