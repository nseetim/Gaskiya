//! Per-listing hash chaining: each `ListingVersion.content_hash` is the
//! SHA-256 of the version's canonicalized content, and each version's
//! `prev_version_hash` must equal the previous version's `content_hash`.
//! This is what makes a "verify-then-pivot" edit structurally visible —
//! the chain either recomputes cleanly or it doesn't.

use serde::Serialize;
use sha2::{Digest, Sha256};

pub fn sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

/// Canonicalizes `content` to a JSON string with deterministically ordered
/// object keys, then returns its SHA-256 hex digest.
///
/// Relies on `serde_json::Value`'s `Object` variant being a `BTreeMap`
/// (true as long as the `preserve_order` feature is not enabled), which
/// sorts keys on serialization without extra work.
pub fn content_hash<T: Serialize>(content: &T) -> Result<String, serde_json::Error> {
    let value = serde_json::to_value(content)?;
    let canonical = serde_json::to_string(&value)?;
    Ok(sha256_hex(canonical.as_bytes()))
}

#[derive(Debug, Clone)]
pub struct VersionLink {
    pub content_hash: String,
    pub prev_version_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    #[error("version at index {0} is not the first version but has no prev_version_hash")]
    MissingPrevHash(usize),
    #[error("first version must not have a prev_version_hash")]
    FirstVersionHasPrev,
    #[error("chain broken at index {index}: prev_version_hash does not match the previous version's content_hash")]
    BrokenLink { index: usize },
}

/// Verifies that `versions` (in version-number order) form an unbroken
/// hash chain. Does not re-derive `content_hash` from raw content — callers
/// with the original content should also check via [`content_hash`].
pub fn verify_chain(versions: &[VersionLink]) -> Result<(), ChainError> {
    for (i, v) in versions.iter().enumerate() {
        if i == 0 {
            if v.prev_version_hash.is_some() {
                return Err(ChainError::FirstVersionHasPrev);
            }
        } else {
            let expected = &versions[i - 1].content_hash;
            match &v.prev_version_hash {
                None => return Err(ChainError::MissingPrevHash(i)),
                Some(actual) if actual != expected => {
                    return Err(ChainError::BrokenLink { index: i })
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Recomputes `content_hash` from live content and checks it against the
/// hash stored on the version row — this is the "don't trust the server's
/// word for it" half of verification.
pub fn matches_content<T: Serialize>(content: &T, expected_hash: &str) -> bool {
    content_hash(content)
        .map(|h| h == expected_hash)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Serialize;

    #[derive(Serialize)]
    struct Content {
        title: String,
        application_url: String,
    }

    #[test]
    fn same_content_same_hash_regardless_of_field_order() {
        #[derive(Serialize)]
        struct A {
            b: i32,
            a: i32,
        }
        #[derive(Serialize)]
        struct B {
            a: i32,
            b: i32,
        }
        let h1 = content_hash(&A { b: 2, a: 1 }).unwrap();
        let h2 = content_hash(&B { a: 1, b: 2 }).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn detects_pivot_via_content_hash_change() {
        let v1 = Content {
            title: "Scholarship".into(),
            application_url: "https://real.edu/apply".into(),
        };
        let v2 = Content {
            title: "Scholarship".into(),
            application_url: "https://phishing.example/apply".into(),
        };
        assert_ne!(content_hash(&v1).unwrap(), content_hash(&v2).unwrap());
    }

    #[test]
    fn verifies_unbroken_chain() {
        let h1 = sha256_hex(b"v1");
        let h2 = sha256_hex(b"v2");
        let versions = vec![
            VersionLink {
                content_hash: h1.clone(),
                prev_version_hash: None,
            },
            VersionLink {
                content_hash: h2,
                prev_version_hash: Some(h1),
            },
        ];
        assert!(verify_chain(&versions).is_ok());
    }

    #[test]
    fn rejects_broken_chain() {
        let versions = vec![
            VersionLink {
                content_hash: sha256_hex(b"v1"),
                prev_version_hash: None,
            },
            VersionLink {
                content_hash: sha256_hex(b"v2"),
                prev_version_hash: Some(sha256_hex(b"tampered")),
            },
        ];
        assert_eq!(
            verify_chain(&versions),
            Err(ChainError::BrokenLink { index: 1 })
        );
    }
}
