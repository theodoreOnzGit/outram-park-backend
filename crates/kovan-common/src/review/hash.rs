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

/// `hash` of a non-function item's name-free text (#843). A separate domain
/// from [`code_hash`], so an item and a function never share a hash.
pub fn item_code_hash(code_without_name: &str) -> String {
    let mut input = b"kovan-item-code-v1\n".to_vec();
    input.extend_from_slice(code_without_name.as_bytes());
    sha256_tagged(&input)
}

/// Every other reviewable item of a Rust source file (structs, enums,
/// unions, traits, type aliases, constants and statics:
/// [`super::rust_items`], "Which other items exist"), in source order, with
/// its hashes. A struct, enum or union's `hash` also covers the methods
/// written for it in the same file.
pub fn hash_items(source: &str) -> Result<Vec<HashedFn>, HashError> {
    let parsed = parse_file(source).map_err(HashError)?;
    Ok(parsed
        .items
        .into_iter()
        .map(|entry| {
            let hashes = FnHashes {
                hash: item_code_hash(&entry.code_without_name),
                doc_hash: doc_hash(&entry.doc),
            };
            HashedFn { entry, hashes }
        })
        .collect())
}

/// Everything reviewable in a Rust source file: its functions
/// ([`hash_functions`]), then its other items ([`hash_items`]), leaving out
/// an item whose name a free function of the file also has (the index
/// leaves it out too: [`crate::code_index::folders`], "Other items"). For
/// looking an index entry up by its `qual` in a file's text.
pub fn hash_all(source: &str) -> Result<Vec<HashedFn>, HashError> {
    let mut all = hash_functions(source)?;
    let taken: std::collections::BTreeSet<String> = all.iter().map(|h| h.entry.qualname()).collect();
    all.extend(hash_items(source)?.into_iter().filter(|h| !taken.contains(&h.entry.name)));
    Ok(all)
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

    /// Methodology: an item's hashes (#843) on one struct with a method.
    /// The hash is in the item domain (it differs from the function hash of
    /// the same text), a method edit changes the struct's `hash` and leaves
    /// its `doc_hash`, a doc edit does the opposite, and functions are not
    /// listed among the items.
    ///
    /// Result (2026-10-13): passes.
    #[test]
    fn item_hash_covers_definition_and_methods() {
        let src = "/// A ball.\nstruct Ball { r: f64 }\nimpl Ball { fn r(&self) -> f64 { self.r } }";
        let a = hash_items(src).unwrap();
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].hashes.hash, item_code_hash(&a[0].entry.code_without_name));
        assert_ne!(a[0].hashes.hash, code_hash(&a[0].entry.code_without_name));
        let method = hash_items(&src.replace("self.r }", "self.r * 2.0 }")).unwrap();
        assert_ne!(method[0].hashes.hash, a[0].hashes.hash);
        assert_eq!(method[0].hashes.doc_hash, a[0].hashes.doc_hash);
        let doc = hash_items(&src.replace("A ball.", "A sphere.")).unwrap();
        assert_eq!(doc[0].hashes.hash, a[0].hashes.hash);
        assert_ne!(doc[0].hashes.doc_hash, a[0].hashes.doc_hash);
        assert!(hash_items("fn (").is_err());
        // Functions first, then items; an item named as a free function is
        // left out.
        let all = hash_all("const f: u8 = 1; fn f() {} struct Ball; impl Ball { fn r(&self) {} }").unwrap();
        let names: Vec<String> = all.iter().map(|h| h.entry.qualname()).collect();
        assert_eq!(names, ["f", "Ball::r", "Ball"]);
        assert!(all[0].entry.shape.is_none() && all[2].entry.shape.is_some());
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
