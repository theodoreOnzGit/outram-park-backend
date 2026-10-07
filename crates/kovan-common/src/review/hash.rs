//! The **function hash** of code review (maintainer, #739, 2026-10-07; data
//! walkthrough U5 on #740).
//!
//! - `hash` covers the **normalised tokens of the signature and body**:
//!   whitespace, formatting, plain `//` and `/* */` comments and the
//!   function's **own name** are excluded. Renaming a function alone keeps
//!   its hash (the review follows it); any other token change, a variable
//!   rename included, changes it.
//! - `doc_hash` covers the `///` doc comment separately, so editing it raises
//!   the lighter "doc changed" flag rather than a full re-review.
//!
//! Both are `sha256:` + 64 lowercase hex digits over a domain-separated
//! input, so a code text can never collide with a doc text:
//!
//! ```text
//! hash     = sha256("kovan-fn-code-v1\n" + code_without_name)
//! doc_hash = sha256("kovan-fn-doc-v1\n"  + normalised doc)
//! ```
//!
//! The normalisation itself (which tokens, how a doc paragraph is
//! re-wrapped) is [`super::rust_items`]' and is reused, not repeated: the
//! code text is [`FnEntry::code_without_name`], the doc text
//! [`FnEntry::doc`]. Pure Rust (`syn`, `sha2`), so it runs in web-kovan too.

use sha2::{Digest, Sha256};

use super::rust_items::{parse_file, FnEntry};
use super::types::HASH_PREFIX;

/// A function's two hashes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct FnHashes {
    /// Signature and body, name excluded.
    pub hash: String,
    /// The `///` doc comment.
    pub doc_hash: String,
}

/// `sha256:` + lowercase hex of SHA-256 over `bytes`.
pub fn sha256_tagged(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut s = String::with_capacity(HASH_PREFIX.len() + 64);
    s.push_str(HASH_PREFIX);
    for b in digest {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// `hash` of a normalised, name-free code text.
pub fn code_hash(code_without_name: &str) -> String {
    let mut input = b"kovan-fn-code-v1\n".to_vec();
    input.extend_from_slice(code_without_name.as_bytes());
    sha256_tagged(&input)
}

/// `doc_hash` of a normalised doc text.
pub fn doc_hash(doc: &str) -> String {
    let mut input = b"kovan-fn-doc-v1\n".to_vec();
    input.extend_from_slice(doc.as_bytes());
    sha256_tagged(&input)
}

/// Both hashes of one parsed function.
pub fn fn_hashes(f: &FnEntry) -> FnHashes {
    FnHashes {
        hash: code_hash(&f.code_without_name),
        doc_hash: doc_hash(&f.doc),
    }
}

/// One function of a file with its hashes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashedFn {
    pub entry: FnEntry,
    pub hashes: FnHashes,
}

/// The source text does not parse as Rust.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HashError(pub String);

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the file does not parse: {}", self.0)
    }
}

impl std::error::Error for HashError {}

/// Every function of a Rust source file, in source order, with its hashes.
pub fn hash_functions(source: &str) -> Result<Vec<HashedFn>, HashError> {
    let parsed = parse_file(source).map_err(HashError)?;
    Ok(parsed
        .fns
        .into_iter()
        .map(|entry| {
            let hashes = fn_hashes(&entry);
            HashedFn { entry, hashes }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> FnHashes {
        let mut v = hash_functions(src).unwrap();
        assert_eq!(v.len(), 1, "{src}");
        v.remove(0).hashes
    }

    /// Methodology: the maintainer's hash definition (#739, 2026-10-07) on
    /// one function. Reformatting, `//` and `/* */` comment edits and a
    /// rename keep `hash` and `doc_hash`; re-wrapping the doc keeps
    /// `doc_hash`; a doc word changes `doc_hash` only; a variable rename, a
    /// literal and a visibility change each change `hash`; a recursive call
    /// keeps its own name in the body, so renaming a recursive function
    /// changes the hash (its body changed).
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn hash_follows_the_maintainer_definition() {
        let base = one("/// Twice x,\n/// rounded.\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }");
        let same = [
            "/// Twice x,\n/// rounded.\npub fn f(x: f64) -> f64 {\n    let y = x * 2.0; // doubled\n    y.round()\n}\n",
            "/// Twice x,\n/// rounded.\npub fn renamed(x: f64) -> f64 { let y = x /* c */ * 2.0; y.round() }",
            "/// Twice x, rounded.\npub fn g(x:f64)->f64{let y=x*2.0;y.round()}",
        ];
        for s in same {
            assert_eq!(one(s), base, "{s}");
        }
        let doc_only = one("/// Twice x, rounded down.\npub fn f(x: f64) -> f64 { let y = x * 2.0; y.round() }");
        assert_eq!(doc_only.hash, base.hash);
        assert_ne!(doc_only.doc_hash, base.doc_hash);
        for s in [
            "/// Twice x,\n/// rounded.\npub fn f(x: f64) -> f64 { let z = x * 2.0; z.round() }",
            "/// Twice x,\n/// rounded.\npub fn f(x: f64) -> f64 { let y = x * 3.0; y.round() }",
            "/// Twice x,\n/// rounded.\nfn f(x: f64) -> f64 { let y = x * 2.0; y.round() }",
        ] {
            let h = one(s);
            assert_ne!(h.hash, base.hash, "{s}");
            assert_eq!(h.doc_hash, base.doc_hash, "{s}");
        }
        let rec = one("fn fact(n: u64) -> u64 { if n == 0 { 1 } else { n * fact(n - 1) } }");
        let rec2 = one("fn factorial(n: u64) -> u64 { if n == 0 { 1 } else { n * fact(n - 1) } }");
        assert_eq!(rec, rec2, "only the declaration's name is dropped");
        let rec3 = one("fn factorial(n: u64) -> u64 { if n == 0 { 1 } else { n * factorial(n - 1) } }");
        assert_ne!(rec.hash, rec3.hash);
        assert!(base.hash.starts_with("sha256:") && base.hash.len() == 71);
        assert_ne!(code_hash(""), doc_hash(""), "domain separated");
    }

    /// Methodology: methods, generics and trait declarations drop their
    /// name too (the identifier right after `fn`, before `<` or `(`).
    ///
    /// Result (2026-10-07): passes.
    #[test]
    fn name_is_dropped_for_methods_and_generics() {
        let a = hash_functions("struct S; impl S { fn a<T: Copy>(&self, t: T) -> T { t } }").unwrap();
        let b = hash_functions("struct S; impl S { fn b<T: Copy>(&self, t: T) -> T { t } }").unwrap();
        assert_eq!(a[0].hashes, b[0].hashes);
        assert_eq!(a[0].entry.code_without_name, "fn < T : Copy > ( & self , t : T ) - > T { t }");
        let t = hash_functions("trait Q { fn q(&self) -> u8; }").unwrap();
        assert_eq!(t[0].entry.code_without_name, "fn ( & self ) - > u8 ;");
        assert!(hash_functions("fn (").is_err());
    }
}
