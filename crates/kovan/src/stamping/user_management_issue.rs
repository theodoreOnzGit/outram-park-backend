//! **Issue a validated keyfile** (GitHub #1211, pathway (b); maintainer,
//! 2026-11-04: "i want both pathways, where i can validate keys, or give
//! other people a validated keyfile. Both should work").
//!
//! The founder generates a key pair for another person, admits it in the
//! same write, and saves it encrypted under a passphrase the founder types,
//! to hand over. Pathway (a), validating a key the person generated
//! themselves, is the existing admission ([`super::UserAction::Admit`]).
//! ~~Both are refused unless the signing key is the founder's
//! ([`super::founder_signer`]).~~ **CORRECTED 2026-11-05 (GitHub #1249):**
//! both are open to any admitted maintainer, a founder included
//! ([`super::maintainer_signer`]). "Founder" in the diagram below reads
//! "the signing maintainer".
//!
//! ```text
//!  founder's key (session) ─┐
//!  form: id, name, key id, role/scope, passphrase ×2, file (save picker)
//!        │ worker
//!        v
//!  issue_problems (re-read kovan_root.toml) ─refused─> nothing written
//!        v
//!  keystore::generate (argon2id + AES-256-GCM, the existing key file)
//!        ├─ the new UnlockedKey is dropped at once (zeroised): the founder never signs with it
//!        v
//!  founder.admit(reviewer) ── append_reviewer (read back) ──┐
//!        v                                                  │
//!  KeyFile::write_new_to(file)  (new file only, 0600) ──────┤ file first: a failed root
//!        v                                                  │ write removes the file, so no
//!  kovan_root.toml written ─────────────────────────────────┘ admitted key exists that nobody holds
//! ```
//!
//! **The keyfile is the ordinary `.kovankey` file** (`kovan-ed25519-key-v1`,
//! `kovan_common::review::signing::keystore`): the private seed is
//! encrypted at rest under the passphrase (argon2id, AES-256-GCM, header
//! bound in as AAD). It is never written unencrypted, and it imports
//! through the existing "Import a key" path ([`Keystore::import`]), which
//! finds it registered because the same write admitted it. The new owner
//! should change nothing: the passphrase is the founder's choice, handed
//! over apart from the file. (Changing a key file's passphrase is not a
//! kovan feature today.)
//!
//! **Secrets.** Both passphrases leave the form into the worker and are
//! zeroised there, and on drop of the form ([`IssueForm`]).
//!
//! [`Keystore::import`]: kovan_common::review::signing::keystore::Keystore::import

use std::path::{Path, PathBuf};

use kovan_common::review::root::{ReviewRoot, Role, RosterEventKind};
use kovan_common::review::root_append::{append_reviewer, append_roster_event, unadmitted_reviewer};
use kovan_common::review::signing::keystore::{generate, Keystore, UnlockedKey, KEY_FILE_EXT};
use kovan_common::review::signing::registry::Registry;
use kovan_common::review::types::reviewer_id_kind;
use zeroize::Zeroize;

use super::{globs, maintainer_signer, roster_event, ROOT_FILE};
use crate::stamping::write_text;

/// What the keyfile admits (no secret: safe to clone and print).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueRequest {
    /// `github:<handle>`, …: a reviewer not yet in `kovan_root.toml`.
    pub reviewer: String,
    pub name: Option<String>,
    pub key_id: String,
    pub role: Role,
    /// A reviewer's scope (globs); none for a maintainer.
    pub scope: Vec<String>,
    /// Admitted as a novice reviewer (a roster event in the same write).
    pub novice: bool,
}

/// The "Issue a validated keyfile" form.
#[derive(Debug)]
pub struct IssueForm {
    pub reviewer: String,
    pub name: String,
    pub key_id: String,
    pub role: Role,
    pub novice: bool,
    /// Globs, comma- or newline-separated.
    pub scope: String,
    /// The keyfile's passphrase, typed by the founder, to hand over.
    pub passphrase: String,
    pub confirm: String,
    /// Where the keyfile is saved (from the save picker).
    pub dest: Option<PathBuf>,
}

impl Default for IssueForm {
    fn default() -> Self {
        IssueForm {
            reviewer: String::new(),
            name: String::new(),
            key_id: "k1".into(),
            role: Role::Reviewer,
            novice: false,
            scope: String::new(),
            passphrase: String::new(),
            confirm: String::new(),
            dest: None,
        }
    }
}

impl Drop for IssueForm {
    fn drop(&mut self) {
        self.passphrase.zeroize();
        self.confirm.zeroize();
    }
}

impl IssueForm {
    /// The request the form states.
    pub fn request(&self) -> IssueRequest {
        IssueRequest {
            reviewer: self.reviewer.trim().to_string(),
            name: Some(self.name.trim()).filter(|n| !n.is_empty()).map(str::to_string),
            key_id: self.key_id.trim().to_string(),
            role: self.role,
            scope: if self.role == Role::Reviewer { globs(&self.scope) } else { Vec::new() },
            novice: self.novice && self.role == Role::Reviewer,
        }
    }

    /// What the form itself lacks: the passphrase twice, alike, and a file.
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if self.passphrase.is_empty() {
            p.push("Choose the keyfile's passphrase (you hand it over apart from the file).".into());
        } else if self.passphrase != self.confirm {
            p.push("The two keyfile passphrases differ.".into());
        }
        if self.dest.is_none() {
            p.push("Choose where to save the keyfile.".into());
        }
        p
    }

    /// The save picker's default name: `<reviewer>--<key>.kovankey`, as the
    /// keystore names it.
    pub fn default_file_name(&self) -> String {
        Keystore::at("").path_for(self.reviewer.trim(), self.key_id.trim())
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("key.{KEY_FILE_EXT}"))
    }
}

/// What stops issuing `req`, signed by `signer` (reviewer, key) on `date`,
/// against `root` (empty: it may go ahead). The same check runs again on
/// the file just before writing.
pub fn issue_problems(root: &ReviewRoot, signer: Option<(&str, &str)>, req: &IssueRequest, date: &str) -> Vec<String> {
    let mut p = Vec::new();
    match signer {
        None => p.push("No signing key: set one up from a Stamp, or import one below.".into()),
        Some(s) => {
            if let Err(e) = maintainer_signer(&Registry::build(root), s, date) {
                p.push(format!("{e} A founder or an admitted maintainer issues a validated keyfile."));
            }
        }
    }
    if req.reviewer.is_empty() {
        p.push("Enter the person's reviewer id, e.g. github:<handle>.".into());
    } else if let Err(e) = reviewer_id_kind(&req.reviewer) {
        p.push(format!("Reviewer id: {e}"));
    } else if root.reviewer(&req.reviewer).is_some() {
        p.push(format!(
            "{} is in {ROOT_FILE} already: validate their own key (Admit on their page) instead.",
            req.reviewer
        ));
    }
    let id_ok = !req.key_id.is_empty()
        && req.key_id.len() <= 64
        && req.key_id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    if !id_ok {
        p.push("The key id is letters, digits, '-' or '_' (e.g. k1).".into());
    }
    if req.role == Role::Reviewer && req.scope.is_empty() {
        p.push("A reviewer needs a scope (e.g. crates/njoy-*/**).".into());
    }
    p
}

/// What [`issue_keyfile`] wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issued {
    pub path: PathBuf,
    pub reviewer: String,
    pub key: String,
    pub fingerprint: String,
    pub role: Role,
    pub novice: bool,
}

impl Issued {
    /// The notice to show.
    pub fn notice(&self) -> String {
        let role = match (self.role, self.novice) {
            (Role::Maintainer, _) => "maintainer",
            (Role::Reviewer, true) => "novice reviewer",
            (Role::Reviewer, false) => "reviewer",
        };
        format!(
            "Issued and admitted {} as {role}: key {} ({}) saved encrypted to {}. Hand the file \
             and its passphrase over separately; they import it with \"Import a key\".",
            self.reviewer,
            self.key,
            self.fingerprint,
            self.path.display()
        )
    }
}

/// Issue a validated keyfile (module doc): re-check [`issue_problems`]
/// against the file, generate a key for `req.reviewer` encrypted under
/// `passphrase`, admit it signed by `founder`, write the keyfile to the new
/// file `dest`, then `<root>/kovan_root.toml`. Nothing is written on any
/// refusal; if the root cannot be written, the keyfile just written is
/// removed again.
pub fn issue_keyfile(
    root: &Path,
    founder: &UnlockedKey,
    req: &IssueRequest,
    passphrase: &str,
    dest: &Path,
    date: &str,
) -> Result<Issued, String> {
    let path = root.join(ROOT_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{ROOT_FILE}: {e}"))?;
    let parsed = ReviewRoot::parse(&text).map_err(|e| format!("{ROOT_FILE}: {e}"))?;
    let problems = issue_problems(&parsed, Some((founder.reviewer(), founder.key_id())), req, date);
    if !problems.is_empty() {
        return Err(problems.join(" "));
    }
    if passphrase.is_empty() {
        return Err("Choose the keyfile's passphrase.".into());
    }
    if dest.exists() {
        return Err(format!("{} exists already: choose a new file.", dest.display()));
    }
    let (kf, new_key) = generate(&req.reviewer, &req.key_id, date, passphrase).map_err(|e| e.to_string())?;
    // The founder never signs as the new person: their key is dropped
    // (and zeroised) here, unused.
    drop(new_key);
    let mut r = unadmitted_reviewer(&req.reviewer, req.name.as_deref(), kf.reviewer_key());
    r.role = req.role;
    r.scope = req.scope.clone();
    r.admitted = Some(date.to_string());
    founder.admit(&mut r).map_err(|e| e.to_string())?;
    let mut t = append_reviewer(&text, &r).map_err(|e| e.to_string())?;
    if req.novice {
        let e = roster_event(founder, &req.reviewer, RosterEventKind::Novice, date, None, &[])?;
        t = append_roster_event(&t, &e).map_err(|e| e.to_string())?;
    }
    let fingerprint = kf.fingerprint().map_err(|e| e.to_string())?;
    kf.write_new_to(dest).map_err(|e| e.to_string())?;
    if let Err(e) = write_text(&path, &t) {
        let removed = std::fs::remove_file(dest).is_ok();
        return Err(format!(
            "{e} The keyfile {} {}.",
            dest.display(),
            if removed { "was removed again" } else { "could NOT be removed: delete it, it is admitted nowhere" }
        ));
    }
    Ok(Issued {
        path: dest.to_path_buf(),
        reviewer: req.reviewer.clone(),
        key: req.key_id.clone(),
        fingerprint,
        role: req.role,
        novice: req.novice,
    })
}
