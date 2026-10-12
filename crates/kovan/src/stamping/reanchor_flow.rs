//! **The module re-anchor window's state machine** (GitHub #838): which key,
//! what is ready, the passphrase, the worker jobs. No egui; the drawing is
//! `app/need_you_view.rs`. The checks and the signing are [`super::reanchor`].
//!
//! ```text
//!  Loading ── worker: keystore list + kovan_root.toml, then
//!     │       prepare_reanchor_module for the first registered key whose
//!     │       reviewer has something to re-anchor in the module
//!     v
//!  Ready (ready and refused lists; passphrase) ── "Sign" ──> Signing
//!     ^── wrong passphrase ──┘         (worker: unlock, sign_reanchor_module)
//!     v
//!  Done(report)          Failed(message) from Loading
//! ```
//!
//! One passphrase entry signs every ready review of the module (maintainer,
//! 2026-10-12: re-anchoring is done "by modules, rather than by functions").

use std::path::PathBuf;

use kovan_common::review::signing::keystore::{Keystore, KeystoreError};
use zeroize::Zeroize;

use super::flow::{find_keys, KeyInfo, SignFailure, Worker};
use super::reanchor::{prepare_reanchor_module, sign_reanchor_module, ModuleReanchor};
use super::relocate::RelocateReport;

/// Where the window is.
#[derive(Debug, Clone, PartialEq)]
pub enum ReanchorStep {
    Loading,
    Ready,
    Signing,
    Done(RelocateReport),
    /// Nothing can be signed: no registered key, or the module is refused.
    Failed(String),
}

type Prepared = Result<(KeyInfo, ModuleReanchor), String>;

enum Job {
    Prepare(Worker<Prepared>),
    Sign(Worker<Result<RelocateReport, SignFailure>>),
}

/// The re-anchor window's state.
pub struct ReanchorFlow {
    root: PathBuf,
    keystore: Keystore,
    /// The module: a source file, workspace-relative.
    pub module: String,
    pub step: ReanchorStep,
    /// The key that will sign.
    pub key: Option<KeyInfo>,
    pub prepared: Option<ModuleReanchor>,
    pub passphrase: String,
    /// The last signing failure.
    pub error: Option<String>,
    job: Option<Job>,
}

impl Drop for ReanchorFlow {
    fn drop(&mut self) {
        self.passphrase.zeroize();
    }
}

/// The first registered key of `keystore` whose reviewer has a review to
/// re-anchor in `module`, with what was gathered; else the last refusal.
fn prepare(root: &std::path::Path, keystore: &Keystore, module: &str) -> Prepared {
    let found = find_keys(root, keystore)?;
    let mut last = "no key in your keystore is registered in kovan_root.toml: set one up with \
                    Stamp first"
        .to_string();
    for k in found.keys.into_iter().filter(|k| k.registered) {
        match prepare_reanchor_module(root, module, &k.reviewer) {
            Ok(m) => return Ok((k, m)),
            Err(e) => last = e,
        }
    }
    Err(last)
}

impl ReanchorFlow {
    /// Open the window for `module`; the first job starts on a worker.
    pub fn new(root: PathBuf, keystore: Keystore, module: String) -> ReanchorFlow {
        let (r, ks, m) = (root.clone(), keystore.clone(), module.clone());
        ReanchorFlow {
            root,
            keystore,
            module,
            step: ReanchorStep::Loading,
            key: None,
            prepared: None,
            passphrase: String::new(),
            error: None,
            job: Some(Job::Prepare(Worker::spawn(move || prepare(&r, &ks, &m)))),
        }
    }

    pub fn busy(&self) -> bool {
        self.job.is_some()
    }

    /// Take a finished job; never blocks.
    pub fn poll(&mut self) {
        match self.job.take() {
            None => {}
            Some(Job::Prepare(w)) => match w.try_take() {
                None => self.job = Some(Job::Prepare(w)),
                Some(Ok((k, m))) => {
                    self.key = Some(k);
                    self.prepared = Some(m);
                    self.step = ReanchorStep::Ready;
                }
                Some(Err(e)) => self.step = ReanchorStep::Failed(e),
            },
            Some(Job::Sign(w)) => match w.try_take() {
                None => self.job = Some(Job::Sign(w)),
                Some(Ok(rep)) => self.step = ReanchorStep::Done(rep),
                Some(Err(e)) => {
                    self.error = Some(match e {
                        SignFailure::WrongPassphrase => "Wrong passphrase.".into(),
                        SignFailure::Other(m) => m,
                    });
                    self.step = ReanchorStep::Ready;
                }
            },
        }
    }

    /// Whether "Sign" is enabled: something is ready and a passphrase typed.
    pub fn can_sign(&self) -> bool {
        self.step == ReanchorStep::Ready
            && self.job.is_none()
            && !self.passphrase.is_empty()
            && self.prepared.as_ref().is_some_and(|m| !m.ready.is_empty())
    }

    /// Unlock the key and sign every ready review of the module (worker).
    pub fn sign(&mut self) -> bool {
        if !self.can_sign() {
            return false;
        }
        let (Some(k), Some(m)) = (self.key.clone(), self.prepared.clone()) else {
            return false;
        };
        let mut pass = std::mem::take(&mut self.passphrase);
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.error = None;
        self.step = ReanchorStep::Signing;
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
            sign_reanchor_module(&root, &m, &key).map_err(SignFailure::Other)
        })));
        true
    }

    /// Poll until no job runs (tests only; the UI never waits).
    #[cfg(test)]
    pub fn wait(&mut self) {
        let start = std::time::Instant::now();
        while self.busy() {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(120),
                "re-anchor job did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.poll();
        }
    }
}

#[cfg(test)]
mod tests {
    use kovan_common::review::state::StateKind;

    use super::super::queue::{build, QueueOptions, RowKind};
    use super::super::tests::{founder_key, Repo, LIB, PASS};
    use super::*;

    /// Methodology: on the stamping fixture, stamp `leaf` and `twice`, then
    /// squash the history so their certified commits leave it. Build the
    /// need-you queue, open the re-anchor window's flow for the module the
    /// row names (a temporary keystore), try a wrong passphrase, then the
    /// right one, commit, and rebuild the queue.
    ///
    /// Pass: the queue has one re-anchor row for `lib.rs` naming both
    /// functions; the flow reaches Ready with both ready; a wrong
    /// passphrase writes nothing and returns to Ready with the message; the
    /// right one is Done with both, the passphrase is cleared, both states
    /// are valid and the row is gone. With an empty keystore the flow fails
    /// naming the missing key.
    ///
    /// Result (2026-10-12): passes.
    #[test]
    fn the_queue_row_opens_a_flow_that_signs_the_module() {
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        r.stamp(&key, "leaf", "");
        r.stamp(&key, "twice", "");
        let first = r.git(&["rev-list", "--max-parents=0", "HEAD"]);
        r.git(&["reset", "-q", "--soft", first.trim()]);
        r.git(&["commit", "-q", "-m", "squashed"]);

        let rows = |r: &Repo| -> Vec<(String, usize)> {
            build(r.path(), &QueueOptions::default())
                .unwrap()
                .rows
                .iter()
                .filter_map(|row| match &row.kind {
                    RowKind::ReAnchor { module } => Some((module.clone(), row.functions.len())),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(rows(&r), [(LIB.to_string(), 2)]);

        let empty = tempfile::tempdir().unwrap();
        let mut none = ReanchorFlow::new(r.path().into(), Keystore::at(empty.path()), LIB.into());
        none.wait();
        assert!(
            matches!(&none.step, ReanchorStep::Failed(e) if e.contains("no key in your keystore")),
            "{:?}",
            none.step
        );

        let mut f = ReanchorFlow::new(r.path().into(), Keystore::at(store.path()), LIB.into());
        f.wait();
        assert_eq!(f.step, ReanchorStep::Ready);
        assert_eq!(f.prepared.as_ref().unwrap().ready.len(), 2);
        assert!(!f.can_sign(), "no passphrase yet");
        f.passphrase = "wrong passphrase".into();
        assert!(f.sign());
        f.wait();
        assert_eq!(f.step, ReanchorStep::Ready);
        assert_eq!(f.error.as_deref(), Some("Wrong passphrase."));
        assert!(!r.read(&format!("{}/review.md", super::super::tests::DIR)).contains("Re-anchored on"));

        f.passphrase = PASS.into();
        assert!(f.sign());
        f.wait();
        let ReanchorStep::Done(rep) = &f.step else {
            panic!("{:?} {:?}", f.step, f.error)
        };
        assert_eq!(rep.done.len(), 2, "{rep:?}");
        assert!(f.passphrase.is_empty());
        r.commit("re-anchor lib.rs");
        for q in ["leaf", "twice"] {
            assert_eq!(r.state(q).unwrap().state, Some(StateKind::Valid), "{q}");
        }
        assert_eq!(rows(&r), []);
    }
}
