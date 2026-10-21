//! **Stable function ids** (maintainer, #764, 2026-10-07: "the stable id is
//! a hybrid").
//!
//! - `target = "fn:<opaque>"` in a `review.md` entry's `[kovan]` table is the
//!   **join key**. It never changes: not on a rename, a file move or a
//!   folder move.
//! - `path` (`file.rs::Type::name`) is an attribute holding the current
//!   location; kovan updates it when the maintainer acknowledges a move, and
//!   the move is recorded as `[[review.moved]]` (from, to, commit).
//!
//! # Minting
//!
//! At first index (or first review) a function's id is minted from three
//! things that are fixed at that moment:
//!
//! ```text
//! fn:<first 16 hex digits of sha256("kovan-fn-id-v1\n" + first path + "\n"
//!                                    + first hash + "\n" + first-seen commit)>
//! ```
//!
//! 16 hex digits (64 bits) keep ids short in `review.md` while a collision
//! among a workspace's ~10^5 functions stays below 10^-9. The inputs are
//! recorded nowhere else; the id is opaque afterwards.
//!
//! # The first-version form
//!
//! Before 2026-10-07 an entry named its function by the call-graph key
//! (`[review] function = "file.rs::Type::name"`, `[kovan] target =
//! "code:…"`). ~~Nothing on disk uses it yet, but it still reads:
//! `parse_review_md` migrates it in memory (`ReviewDocument::migrated`),
//! minting the id from the key, the review's hash and its commit, so the
//! next save writes the new form.~~ **CORRECTED 2026-10-10** (#825): nothing
//! on disk ever used it, and the migration was dropped; such an entry is
//! unreadable ([`super::review_md::Unreadable`]).

use super::hash::sha256_tagged;

/// The prefix of a function id.
pub const FN_ID_PREFIX: &str = "fn:";

/// Mint an id (module doc).
pub fn mint_fn_id(first_path: &str, first_hash: &str, first_commit: &str) -> String {
    let input = format!("kovan-fn-id-v1\n{first_path}\n{first_hash}\n{first_commit}");
    let tagged = sha256_tagged(input.as_bytes());
    let hex = tagged.trim_start_matches(super::types::HASH_PREFIX);
    format!("{FN_ID_PREFIX}{}", &hex[..16])
}

/// Whether `s` is `fn:` + 16 lowercase hex digits.
pub fn is_fn_id(s: &str) -> bool {
    s.strip_prefix(FN_ID_PREFIX).is_some_and(|h| {
        h.len() == 16 && h.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

/// Whether `path` looks like `file.rs::item` (a current location).
pub fn is_fn_path(path: &str) -> bool {
    path.split_once(".rs::")
        .is_some_and(|(f, i)| !f.is_empty() && !i.is_empty() && !path.contains(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: minting is deterministic, depends on each input, and
    /// gives the documented shape.
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn mint_is_deterministic_and_shaped() {
        let a = mint_fn_id("crates/x/src/a.rs::f", "sha256:aa", "c1");
        assert_eq!(a, mint_fn_id("crates/x/src/a.rs::f", "sha256:aa", "c1"));
        assert!(is_fn_id(&a), "{a}");
        assert_ne!(a, mint_fn_id("crates/x/src/a.rs::g", "sha256:aa", "c1"));
        assert_ne!(a, mint_fn_id("crates/x/src/a.rs::f", "sha256:ab", "c1"));
        assert_ne!(a, mint_fn_id("crates/x/src/a.rs::f", "sha256:aa", "c2"));
        assert!(!is_fn_id("fn:XYZ") && !is_fn_id("crates/x/src/a.rs::f"));
        assert!(is_fn_path("crates/x/src/a.rs::Type::f") && !is_fn_path("fn:0123"));
    }
}
