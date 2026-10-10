//! **The "Organisations & IV&V" panel's state machine** (GitHub #810):
//! keys, forms, form checks and background jobs, without egui. The drawing
//! is `app/organisations_view.rs`.
//!
//! ```text
//!  open ──> Load (worker: keystore list + overview of kovan_root.toml)
//!            v
//!  forms ── as a maintainer: developing organisation, a reviewer's organisation
//!        └─ as a reviewer:   separation attestation (+ GitHub issue audit record)
//!            │ passphrase
//!            v
//!  Sign (worker: load key file, unlock (argon2), sign, append) ──> Load again
//!            └─ wrong passphrase / refused ──> the form, error shown, nothing written
//! ```
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** Reading the keystore, the KDF
//! and the write run on worker threads ([`super::super::flow::Worker`]);
//! [`OrgPanel::poll`] never blocks. The passphrase is taken out of the form
//! into the worker and zeroised there, and zeroised when the panel drops.

use std::path::{Path, PathBuf};

use kovan_common::review::root::Role;
use kovan_common::review::signed_at::{date_of, now_local};
use kovan_common::review::signing::keystore::{Keystore, KeystoreError};
use zeroize::Zeroize;

use super::{
    add_developing_organisation, add_reviewer_organisation, add_separation_attestation,
    check_audit_record, next_attestation_id, overview, AttestationInput, Overview,
};
use crate::stamping::flow::{find_keys, KeyInfo, SignFailure, Worker};

/// What the panel can sign.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// A `[[code_review.developing_organisation]]` record (maintainer).
    DevelopingOrganisation,
    /// A `[[reviewer.organisation]]` record (maintainer).
    ReviewerOrganisation,
    /// A `[[reviewer.separation]]` attestation (the reviewer).
    Attestation,
}

/// The panel's forms.
#[derive(Default)]
pub struct OrgForms {
    pub dev_name: String,
    /// Empty: workspace-wide; else the crate it overrides.
    pub dev_crate: String,
    pub rev_reviewer: String,
    pub rev_name: String,
    pub attestation: AttestationInput,
    pub passphrase: String,
}

impl Drop for OrgForms {
    fn drop(&mut self) {
        self.passphrase.zeroize();
    }
}

type Loaded = Result<(Vec<KeyInfo>, Overview), String>;

enum Job {
    Load(Worker<Loaded>),
    Sign(Worker<Result<String, SignFailure>>),
}

/// The panel's whole state (module doc).
pub struct OrgPanel {
    root: PathBuf,
    keystore: Keystore,
    pub keys: Vec<KeyInfo>,
    /// Index into `keys` of the key that signs.
    pub chosen: usize,
    /// `None` while loading.
    pub overview: Option<Result<Overview, String>>,
    pub forms: OrgForms,
    /// Plain-words outcomes (what was signed and written).
    pub notices: Vec<String>,
    /// The last refusal or wrong passphrase; nothing was written.
    pub error: Option<String>,
    job: Option<Job>,
    /// The action of the sign job running (or last run).
    signing: Option<Action>,
}

/// Today, `YYYY-MM-DD`, local.
fn today() -> String {
    date_of(&now_local()).unwrap_or_default()
}

impl OrgPanel {
    /// Open the panel over workspace `root`, reading keys from `keystore`
    /// (injected: tests pass a temporary one). Loading starts on a worker.
    pub fn new(root: PathBuf, keystore: Keystore) -> OrgPanel {
        let mut p = OrgPanel {
            root,
            keystore,
            keys: Vec::new(),
            chosen: 0,
            overview: None,
            forms: OrgForms::default(),
            notices: Vec::new(),
            error: None,
            job: None,
            signing: None,
        };
        p.load();
        p
    }

    /// The workspace.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// A job is running.
    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// The signing key, if any.
    pub fn key(&self) -> Option<&KeyInfo> {
        self.keys.get(self.chosen)
    }

    fn load(&mut self) {
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.job = Some(Job::Load(Worker::spawn(move || {
            let found = find_keys(&root, &ks)?;
            if !found.root_exists {
                return Err(
                    "No kovan_root.toml: run Index fresh in the Code Map tab first.".into(),
                );
            }
            Ok((found.keys, overview(&root)?))
        })));
    }

    /// Take a finished job's result; never blocks.
    pub fn poll(&mut self) {
        let Some(job) = self.job.take() else { return };
        match job {
            Job::Load(w) => match w.try_take() {
                None => self.job = Some(Job::Load(w)),
                Some(Err(e)) => self.overview = Some(Err(e)),
                Some(Ok((keys, o))) => {
                    let keep = self.key().map(|k| (k.reviewer.clone(), k.key.clone()));
                    self.keys = keys;
                    self.chosen = keep
                        .and_then(|(r, k)| {
                            self.keys.iter().position(|i| i.reviewer == r && i.key == k)
                        })
                        .unwrap_or(0);
                    self.overview = Some(Ok(o));
                    self.prefill_attestation();
                }
            },
            Job::Sign(w) => match w.try_take() {
                None => self.job = Some(Job::Sign(w)),
                Some(Err(SignFailure::WrongPassphrase)) => {
                    self.error = Some(
                        "Wrong passphrase (or the key file was altered). Nothing was written."
                            .into(),
                    );
                }
                Some(Err(SignFailure::Other(e))) => {
                    self.error = Some(format!("{e} Nothing was written."));
                }
                Some(Ok(notice)) => {
                    self.notices.push(notice);
                    if self.signing == Some(Action::Attestation) {
                        // Written: start the next one afresh (a new id).
                        self.forms.attestation = AttestationInput::default();
                    }
                    self.load();
                }
            },
        }
    }

    /// Whether the signing key's reviewer is a maintainer in the root.
    pub fn is_maintainer(&self) -> bool {
        let (Some(k), Some(Ok(o))) = (self.key(), &self.overview) else {
            return false;
        };
        o.reviewers
            .iter()
            .any(|(id, role)| *id == k.reviewer && *role == Role::Maintainer)
    }

    /// Start the attestation form from what the root records for the
    /// signing reviewer: their organisation and the workspace developing
    /// organisation (the last recorded of each), and a fresh id. Fields
    /// already typed are kept.
    pub fn prefill_attestation(&mut self) {
        let (Some(k), Some(Ok(o))) = (self.key().cloned(), &self.overview) else {
            return;
        };
        let a = &mut self.forms.attestation;
        if a.organisation.trim().is_empty() {
            if let Some(r) = o
                .reviewer_organisations
                .iter()
                .rev()
                .find(|r| r.scope == k.reviewer)
            {
                a.organisation = r.name.clone();
            }
        }
        if a.developing_organisation.trim().is_empty() {
            if let Some(d) = o.developing.iter().rev().find(|d| d.scope == "workspace") {
                a.developing_organisation = d.name.clone();
            }
        }
        if a.id.trim().is_empty() {
            let root = std::fs::read_to_string(self.root.join(super::super::ROOT_FILE))
                .ok()
                .and_then(|t| kovan_common::review::root::ReviewRoot::parse(&t).ok())
                .unwrap_or_default();
            a.id = next_attestation_id(&root, &k.reviewer, &today());
        }
    }

    /// What stops `action` (empty: may sign).
    pub fn problems(&self, action: Action) -> Vec<String> {
        let mut p = Vec::new();
        if self.key().is_none() {
            p.push("No key in the keystore: set one up from a Stamp first.".into());
        }
        if !self.key().is_some_and(|k| k.registered) {
            p.push("This key is not registered in kovan_root.toml.".into());
        }
        let f = &self.forms;
        match action {
            Action::DevelopingOrganisation | Action::ReviewerOrganisation => {
                if !self.is_maintainer() {
                    p.push("Only a maintainer signs organisation records.".into());
                }
            }
            Action::Attestation => {}
        }
        match action {
            Action::DevelopingOrganisation => {
                if f.dev_name.trim().is_empty() {
                    p.push("Enter the developing organisation.".into());
                }
            }
            Action::ReviewerOrganisation => {
                if f.rev_reviewer.trim().is_empty() {
                    p.push("Choose the reviewer.".into());
                }
                if f.rev_name.trim().is_empty() {
                    p.push("Enter the reviewer's organisation.".into());
                }
            }
            Action::Attestation => {
                let a = &f.attestation;
                if a.id.trim().is_empty() {
                    p.push("Enter an id for the attestation.".into());
                }
                if a.organisation.trim().is_empty() {
                    p.push("Enter your organisation.".into());
                }
                if a.developing_organisation.trim().is_empty() {
                    p.push("Enter the developing organisation.".into());
                }
                if let Err(e) = check_audit_record(&a.audit_record) {
                    p.push(e);
                }
            }
        }
        if f.passphrase.is_empty() {
            p.push("Type your passphrase.".into());
        }
        p
    }

    /// Unlock the key, sign `action`'s record and append it (worker).
    /// Refused (nothing starts) while it has problems or a job runs.
    pub fn submit(&mut self, action: Action) -> bool {
        if self.job.is_some() || !self.problems(action).is_empty() {
            return false;
        }
        let Some(k) = self.key().cloned() else {
            return false;
        };
        let mut pass = std::mem::take(&mut self.forms.passphrase);
        let f = &self.forms;
        let (dev_name, dev_crate) = (f.dev_name.clone(), f.dev_crate.clone());
        let (rev_reviewer, rev_name) = (f.rev_reviewer.clone(), f.rev_name.clone());
        let att = AttestationInput {
            date: today(),
            ..f.attestation.clone()
        };
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.error = None;
        self.job = Some(Job::Sign(Worker::spawn(move || {
            let kf = ks
                .load(&k.reviewer, &k.key)
                .map_err(|e| SignFailure::Other(e.to_string()))?;
            let unlocked = kf.unlock(&pass);
            pass.zeroize();
            let key = unlocked.map_err(|e| match e {
                KeystoreError::WrongPassphrase => SignFailure::WrongPassphrase,
                other => SignFailure::Other(other.to_string()),
            })?;
            let date = today();
            let done = match action {
                Action::DevelopingOrganisation => {
                    let krate = Some(dev_crate.trim()).filter(|c| !c.is_empty());
                    add_developing_organisation(&root, &key, &dev_name, krate, &date).map(|e| {
                        format!(
                            "Signed and recorded the developing organisation {} ({}) from {}.",
                            e.name,
                            e.krate.map_or("workspace".into(), |c| format!("crate {c}")),
                            e.date
                        )
                    })
                }
                Action::ReviewerOrganisation => {
                    add_reviewer_organisation(&root, &key, &rev_reviewer, &rev_name, &date).map(
                        |e| {
                            format!(
                                "Signed and recorded {}'s organisation {} from {}.",
                                rev_reviewer.trim(),
                                e.name,
                                e.date
                            )
                        },
                    )
                }
                Action::Attestation => add_separation_attestation(&root, &key, &att).map(|a| {
                    format!(
                        "Signed and recorded separation attestation {}: {} is separate from {}.",
                        a.id, a.organisation, a.developing_organisation
                    )
                }),
            };
            done.map_err(SignFailure::Other)
        })));
        self.signing = Some(action);
        true
    }

    /// Poll until no job runs (tests only; the UI never waits).
    #[cfg(test)]
    pub fn wait(&mut self) {
        let start = std::time::Instant::now();
        while self.busy() {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(120),
                "organisations panel job did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.poll();
        }
    }
}
