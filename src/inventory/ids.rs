//! Deterministic record ID and source-hash helpers.
//!
//! See `docs/output-contract.md` section 6. IDs are stable across runs unless
//! the record content changes.

use sha2::{Digest, Sha256};

/// Full `sha256:<hex>` digest of the given text.
#[must_use]
pub fn source_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(7 + digest.len() * 2);
    out.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Short hash for IDs, computed from the stable identity inputs.
///
/// Hash input is `path + language + kind + line_start + normalized_text`.
#[must_use]
pub fn short_hash(
    path: &str,
    language: &str,
    kind: &str,
    line_start: usize,
    normalized_text: &str,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(path.as_bytes());
    hasher.update(b"\0");
    hasher.update(language.as_bytes());
    hasher.update(b"\0");
    hasher.update(kind.as_bytes());
    hasher.update(b"\0");
    hasher.update(line_start.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(normalized_text.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::with_capacity(8);
    for byte in digest.iter().take(4) {
        use std::fmt::Write;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_hash_is_stable_and_prefixed() {
        let a = source_hash("hello");
        let b = source_hash("hello");
        assert_eq!(a, b);
        assert!(a.starts_with("sha256:"));
        assert_eq!(a.len(), 7 + 64);
    }

    #[test]
    fn short_hash_changes_with_text() {
        let a = short_hash("p", "c", "comment", 1, "x");
        let b = short_hash("p", "c", "comment", 1, "y");
        assert_ne!(a, b);
        assert_eq!(a.len(), 8);
    }
}
