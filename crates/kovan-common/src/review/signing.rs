//! **Stamp signatures**: the stored field and the exact bytes that get
//! signed. The cryptography itself is GitHub #762 and is **not here**:
//! [`verify_review`] and [`verify_architecture`] always answer
//! [`SignatureCheck::Unverified`], with the reason, until #762 lands. Nothing
//! reads a stored signature as valid.
//!
//! # The signed bytes
//!
//! A UTF-8 text, one `key=value` line per field, in a fixed order, each
//! value written as a JSON string (so a newline or `=` inside a value cannot
//! forge a line), maps sorted by key:
//!
//! ```text
//! kovan-review-signature-v1
//! kind="review"
//! target="fn:3f2a9c0d1e4b5a67"     (the stable id, #764 hybrid id)
//! path="crates/x/src/a.rs::f"      (current location)
//! by="github:theodoreOnzGit"
//! rung="3"
//! date="2026-10-07"
//! commit="<sha>"
//! hash="sha256:…"
//! doc_hash="sha256:…"
//! cargo_lock="sha256:…"          (or "" when absent)
//! authorship="agent"              (or "" when absent)
//! session="https://…"            (one line per session, sorted)
//! callee="<id>" "<hash>"          (one line per callee, sorted by id)
//! checklist="<q>" "<answer>"      (one line per answer, sorted)
//! no_concept="…"                  (or "")
//! moved="<from>" "<to>" "<commit>"  (one line per move, in order;
//!                                    "" for a first-version record's to)
//! relation="<kind>" "<target>"    (one line per relation, in order)
//! ```
//!
//! The artifact id, `created`/`modified`, the Markdown comments and the
//! signature itself are not signed (the signing comment on #739 lists the
//! fields: target, hash, commit, `by`, date, checklist; the rest above are
//! the review's other certifying fields). The authorship of the reviewed
//! change is signed (maintainer, #764, 2026-10-07).

use serde::{Deserialize, Serialize};

use super::review_md::{ArchitectureEntry, ReviewEntry};

/// `[review.signature]` / `[architecture.signature]`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    /// The reviewer key id (`[[reviewer.key]] id` in `kovan_root.toml`).
    pub key: String,
    /// `ed25519`.
    pub alg: String,
    /// Base64 signature over [`signed_bytes`].
    pub value: String,
}

/// Why a signature does not (yet) count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnverifiedReason {
    /// The entry carries no signature.
    NoSignature,
    /// A signature is present, but signature checking is GitHub #762 and is
    /// not implemented: nothing is verified.
    CryptoNotImplemented,
}

/// The result of checking an entry's signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignatureCheck {
    Unverified(UnverifiedReason),
}

fn q(s: &str) -> String {
    serde_json::Value::String(s.to_string()).to_string()
}

fn line(out: &mut String, key: &str, values: &[&str]) {
    out.push_str(key);
    out.push('=');
    let quoted: Vec<String> = values.iter().map(|v| q(v)).collect();
    out.push_str(&quoted.join(" "));
    out.push('\n');
}

/// The bytes a review's signature is taken over (module doc).
pub fn signed_bytes(r: &ReviewEntry) -> Vec<u8> {
    let b = &r.review;
    let mut s = String::from("kovan-review-signature-v1\n");
    line(&mut s, "kind", &["review"]);
    line(&mut s, "target", &[&r.function_id()]);
    line(&mut s, "path", &[&r.path().unwrap_or_default()]);
    line(&mut s, "by", &[&b.by]);
    line(&mut s, "rung", &[&b.rung.to_string()]);
    line(&mut s, "date", &[&b.date]);
    line(&mut s, "commit", &[&b.commit]);
    line(&mut s, "hash", &[&b.hash]);
    line(&mut s, "doc_hash", &[&b.doc_hash]);
    line(&mut s, "cargo_lock", &[b.cargo_lock.as_deref().unwrap_or("")]);
    let (kind, sessions) = match &b.authorship {
        Some(a) => (a.kind.as_str(), a.sessions.clone()),
        None => ("", Vec::new()),
    };
    line(&mut s, "authorship", &[kind]);
    let mut sessions = sessions;
    sessions.sort();
    for sess in &sessions {
        line(&mut s, "session", &[sess]);
    }
    for (id, h) in &b.callees {
        line(&mut s, "callee", &[id, h]);
    }
    for (k, v) in &b.checklist {
        line(&mut s, "checklist", &[k, v]);
    }
    line(&mut s, "no_concept", &[b.no_concept.as_deref().unwrap_or("")]);
    for m in &b.moved {
        line(&mut s, "moved", &[&m.from, m.to.as_deref().unwrap_or(""), &m.commit]);
    }
    for rel in &r.relations {
        line(&mut s, "relation", &[rel.kind.as_str(), &rel.target]);
    }
    s.into_bytes()
}

/// The bytes an architecture node's signature is taken over: the same
/// line format, `kind="architecture"`, then by, date, commit, each member
/// (sorted), the upstream's repository and commit, the pattern, and the
/// relations.
pub fn architecture_signed_bytes(a: &ArchitectureEntry) -> Vec<u8> {
    let b = &a.architecture;
    let mut s = String::from("kovan-review-signature-v1\n");
    line(&mut s, "kind", &["architecture"]);
    line(&mut s, "id", &[&a.kovan.id]);
    line(&mut s, "by", &[&b.by]);
    line(&mut s, "date", &[&b.date]);
    line(&mut s, "commit", &[&b.commit]);
    let mut members = b.members.clone();
    members.sort();
    for m in &members {
        line(&mut s, "member", &[m]);
    }
    let (repo, commit) = match &b.upstream {
        Some(u) => (
            u.repository.clone().unwrap_or_default(),
            u.commit.clone().unwrap_or_default(),
        ),
        None => (String::new(), String::new()),
    };
    line(&mut s, "upstream", &[&repo, &commit]);
    line(&mut s, "pattern", &[b.pattern.as_deref().unwrap_or("")]);
    for rel in &a.relations {
        line(&mut s, "relation", &[rel.kind.as_str(), &rel.target]);
    }
    s.into_bytes()
}

fn stub(sig: Option<&Signature>) -> SignatureCheck {
    SignatureCheck::Unverified(match sig {
        None => UnverifiedReason::NoSignature,
        Some(_) => UnverifiedReason::CryptoNotImplemented,
    })
}

/// Check a review's signature. **Stub until #762**: never verified.
pub fn verify_review(r: &ReviewEntry) -> SignatureCheck {
    stub(r.review.signature.as_ref())
}

/// Check an architecture node's signature. **Stub until #762**.
pub fn verify_architecture(a: &ArchitectureEntry) -> SignatureCheck {
    stub(a.architecture.signature.as_ref())
}
