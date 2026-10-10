//! **The stamp dialog's state machine** (GitHub #770, #762, #740): every
//! step, transition, form check and background job of desktop kovan's
//! Stamp / Needs-fix dialog, without egui. The drawing is
//! `app/stamp_dialog.rs`; this file decides what it may show and do.
//!
//! ```text
//!  request (Stamp | NeedsFix, from kovan-web's review bar)
//!     │
//!     v
//!  Loading ── keystore list + kovan_root.toml (worker)
//!     ├─ no kovan_root.toml ──────────────> NoRoot ("run Index fresh"), stop
//!     ├─ no key ──> SetupKey ── Generate (worker: generate, save,
//!     │                │         register_key) ──> Loading again
//!     ├─ several ─> PickKey ─┐
//!     └─ one ────────────────┤
//!                            v
//!            key not in kovan_root.toml? ── Stamp ──> Register (worker) ──> Loading
//!                            │
//!            Stamp ──> Preparing (worker: prepare_stamp)
//!                            ├─ refused ──> Refused (verbatim + hint, "Try again";
//!                            │              since 2026-10-10 also bottom-up: a callee
//!                            │              without a valid stamp, #740)
//!                            v
//!                         Wizard ── live stamp_gate; "Mark as Needs fix instead?" ─┐
//!                            │ passphrase                                          │
//!                            v                                                     │
//!                         Signing (worker: load + unlock (argon2), draft_stamp     │
//!                            │     with the answers, sign_review, write_review)    │
//!                            ├─ wrong passphrase ──> Wizard (error, answers kept)  │
//!                            v                                                     │
//!            NeedsFix ──> NeedsFixForm <───────────────────────────────────────────┘
//!                            │ note
//!                            v
//!                         WritingNeedsFix (worker: draft_needs_fix_for, write_needs_fix)
//!                            v
//!                          Done ── Refresh (worker: stamp_states) ──> the review bar
//!
//!  Reconfirm (added 2026-10-10, #770, #740 decision 8) ── key as for Stamp ──>
//!  PreparingReconfirm (worker: reconfirm::prepare_reconfirm)
//!     ├─ refused ──> Refused (blocked by a failing test, not inherited stale, …)
//!     v
//!  Reconfirm (the changed callees' diffs; passphrase) ── "Mark for re-review" ──> Wizard
//!     v
//!  Signing (worker: unlock, reconfirm::sign_reconfirm) ──> Done
//! ```
//!
//! **No lag (root `CLAUDE.md`, HARD RULE).** Every step that touches the
//! disk, git or the KDF runs in a [`Worker`] thread; the UI thread calls
//! [`StampFlow::poll`], which takes a finished result with `try_write` and
//! never blocks.
//!
//! **Passphrases** live only in the forms' `String`s and in the worker that
//! uses them; they are zeroised after use and when a form is dropped, and
//! never logged or put in an error.
//!
//! **What is signed.** A stamp is signed by the key's owner after typing
//! the passphrase. A needs-fix entry carries no signature in the library
//! (`NeedsFixEntry` has none), so none is asked for. AI agents never stamp:
//! nothing here can sign without the passphrase.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use kovan_common::call_graph::split::StampState as WebStamp;
use kovan_common::review::root::ReviewRoot;
use kovan_common::review::signed_at::{date_of, now_local};
use kovan_common::review::signing::keystore::{generate, Keystore, KeystoreError};
use kovan_common::review::types::reviewer_id_kind;
use kovan_common::review::wizard::{
    format_answer, parse_answer, Applicability, GateResult, Question, ReviewWizard,
};
use kovan_common::review::ivv_view::AttestationChoice;
use zeroize::Zeroize;

use super::concepts::{self, ConceptChoice};
use super::organisations::attestation_choices_in;
use super::reconfirm::{prepare_reconfirm, sign_reconfirm, ReconfirmContext};
use super::review_mode::prefill::NoConceptForm;
use super::{
    draft_needs_fix_for, draft_stamp, prepare_stamp, register_key, stamp_states, write_needs_fix,
    write_review, KeyRegistration, StampContext, StampRequest, WrittenEntry, ROOT_FILE,
};

/// A background job: the closure runs on its own thread and leaves its
/// result in an `Arc<RwLock<Option<T>>>`, which the UI thread takes with
/// [`Worker::try_take`] (never blocking).
pub struct Worker<T> {
    slot: Arc<RwLock<Option<T>>>,
}

impl<T: Send + Sync + 'static> Worker<T> {
    /// Run `f` on a new thread.
    pub fn spawn(f: impl FnOnce() -> T + Send + 'static) -> Worker<T> {
        let slot: Arc<RwLock<Option<T>>> = Arc::new(RwLock::new(None));
        let w = slot.clone();
        std::thread::spawn(move || {
            let out = f();
            if let Ok(mut s) = w.write() {
                *s = Some(out);
            }
        });
        Worker { slot }
    }

    /// The result, once, if the job has finished; `None` while it runs or
    /// while the lock is held (tried again next frame).
    pub fn try_take(&self) -> Option<T> {
        self.slot.try_write().ok().and_then(|mut s| s.take())
    }
}

/// Stamp, Needs fix, or (added 2026-10-10, #770) Re-confirm an
/// inherited-stale review without the wizard ([`super::reconfirm`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Stamp,
    NeedsFix,
    Reconfirm,
}

/// The function the request names (from kovan-web's `FunctionRef`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    /// The call-graph id (`crates/x/src/a.rs::T::f`).
    pub function: String,
    /// The name the review bar shows.
    pub name: String,
}

/// One key file of the keystore, and whether `kovan_root.toml` lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInfo {
    pub reviewer: String,
    pub key: String,
    pub path: PathBuf,
    /// The root lists this reviewer with this public key.
    pub registered: bool,
}

/// The keystore and the root as the dialog opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeysFound {
    pub root_exists: bool,
    pub keys: Vec<KeyInfo>,
}

/// Read the keystore's key files (public headers only, nothing unlocked)
/// and mark the ones `<root>/kovan_root.toml` lists. `Err` when a key file
/// or the root does not parse.
pub fn find_keys(root: &Path, keystore: &Keystore) -> Result<KeysFound, String> {
    let text = std::fs::read_to_string(root.join(ROOT_FILE)).ok();
    let parsed = match &text {
        Some(t) => Some(ReviewRoot::parse(t).map_err(|e| format!("{ROOT_FILE}: {e}"))?),
        None => None,
    };
    let files = keystore.list().map_err(|e| e.to_string())?;
    let keys = files
        .into_iter()
        .map(|kf| {
            let registered = parsed
                .as_ref()
                .and_then(|r| r.reviewer(&kf.reviewer))
                .is_some_and(|r| r.keys.iter().any(|k| k.public == kf.public));
            KeyInfo {
                path: keystore.path_for(&kf.reviewer, &kf.key),
                reviewer: kf.reviewer,
                key: kf.key,
                registered,
            }
        })
        .collect();
    Ok(KeysFound {
        root_exists: text.is_some(),
        keys,
    })
}

/// The first `k<n>` (n = 1, 2, …) that is neither a key of `reviewer` in
/// the root nor a key file of `reviewer` in the keystore.
pub fn next_key_id(root: &Path, keystore: &Keystore, reviewer: &str) -> String {
    let mut used: Vec<String> = keystore
        .list()
        .unwrap_or_default()
        .into_iter()
        .filter(|k| k.reviewer == reviewer)
        .map(|k| k.key)
        .collect();
    if let Some(r) = std::fs::read_to_string(root.join(ROOT_FILE))
        .ok()
        .and_then(|t| ReviewRoot::parse(&t).ok())
    {
        if let Some(rv) = r.reviewer(reviewer) {
            used.extend(rv.keys.iter().map(|k| k.id.clone()));
        }
    }
    (1..)
        .map(|n| format!("k{n}"))
        .find(|k| !used.contains(k))
        .unwrap_or_else(|| "k1".into())
}

/// Plain words for a registration outcome.
pub fn describe_registration(r: &KeyRegistration) -> String {
    match r {
        KeyRegistration::Founder => "Registered you as the founding maintainer of this workspace \
             (trusted on first use)."
            .into(),
        KeyRegistration::AwaitingAdmission => "Registered you as a new reviewer, awaiting \
             admission: your stamps count for nothing until a maintainer admits you (they sign \
             your entry in kovan_root.toml)."
            .into(),
        KeyRegistration::KeyAdded {
            needs_endorsement: true,
        } => "Added this key to your reviewer entry. It needs an endorsement by one of your \
             earlier keys before its stamps count."
            .into(),
        KeyRegistration::KeyAdded {
            needs_endorsement: false,
        } => "Added this key to your reviewer entry.".into(),
    }
}

/// A refusal message and the hint that goes with it.
pub fn refusal_hint(message: &str) -> &'static str {
    if message.contains("not committed") {
        "Commit or stash the changes in this file first (the stamp certifies HEAD), then Try again."
    } else if message.contains("no commit") {
        "Make a first commit, then Try again."
    } else if message.contains("kovan.toml")
        || message.contains("index hash")
        || message.contains("out of date")
    {
        "The index is out of date for this function: run Index fresh in the Code Map tab, then Try again."
    } else if message.starts_with("bottom-up") {
        "Bottom-up (#740): review the callees listed first, through the review bar's \"blocked by\" links or the review walk, then Try again."
    } else if message.contains("changed while") {
        "Look at the new diff in the review panel, then sign again: your answers and comments are kept."
    } else if message.contains(ROOT_FILE) {
        "kovan_root.toml cannot be read: run Index fresh in the Code Map tab to repair it."
    } else {
        "Nothing was written."
    }
}

/// The new-key form.
#[derive(Default)]
pub struct KeySetupForm {
    /// `github:<handle>`, `gitlab:<handle>`, an ORCID or an e-mail.
    pub reviewer: String,
    /// Display name (optional).
    pub name: String,
    pub passphrase: String,
    pub confirm: String,
}

impl Drop for KeySetupForm {
    fn drop(&mut self) {
        self.passphrase.zeroize();
        self.confirm.zeroize();
    }
}

impl KeySetupForm {
    /// What stops "Generate": an empty or malformed reviewer id (the
    /// registry's own check, [`reviewer_id_kind`]), an empty passphrase,
    /// or a confirmation that differs. Empty = may generate.
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        let id = self.reviewer.trim();
        if id.is_empty() {
            p.push("Enter your reviewer id, e.g. github:<handle>.".into());
        } else if let Err(e) = reviewer_id_kind(id) {
            p.push(format!("Reviewer id: {e}"));
        }
        if self.passphrase.is_empty() {
            p.push("Choose a passphrase.".into());
        } else if self.passphrase != self.confirm {
            p.push("The two passphrases differ.".into());
        }
        p
    }
}

/// The wizard's answers and the sign form.
pub struct WizardForm {
    pub ctx: StampContext,
    /// Question key -> chosen option key.
    pub options: BTreeMap<String, String>,
    /// Question key -> the text of an option that `requires_text`.
    pub texts: BTreeMap<String, String>,
    /// Free text written under the entry in `review.md`.
    pub comments: String,
    pub passphrase: String,
    /// The reviewer's own signed separation attestations (GitHub #810),
    /// read when the wizard opened.
    pub attestations: Vec<AttestationChoice>,
    /// The one this review relies on for rung 5; `None` (the default):
    /// the review claims no IV&V. Signed into the stamp.
    pub separation_attestation: Option<String>,
    /// Added 2026-10-10 (#770, #740 U3): the "no concept" reason, started
    /// from the context's prefill.
    pub no_concept: NoConceptForm,
    /// Added 2026-10-10 (#770, #740 decision 11): the linked concept's id
    /// (`concept:<path>` or `collection:<path>`), written as an
    /// `implements` relation. A concept and a no-concept reason exclude
    /// each other ([`Self::choose_concept`], [`Self::choose_reason`]).
    pub concept: Option<String>,
    /// The concept finder's query.
    pub concept_query: String,
}

impl Drop for WizardForm {
    fn drop(&mut self) {
        self.passphrase.zeroize();
    }
}

impl WizardForm {
    /// Start from the context's prefilled answers.
    pub fn new(ctx: StampContext) -> WizardForm {
        let mut options = BTreeMap::new();
        let mut texts = BTreeMap::new();
        for (q, raw) in &ctx.answers {
            let (o, t) = parse_answer(raw);
            options.insert(q.clone(), o.to_string());
            if let Some(t) = t {
                texts.insert(q.clone(), t.to_string());
            }
        }
        let mut no_concept = NoConceptForm::from_stored(ctx.no_concept.as_deref());
        let concept = ctx.concept.clone();
        if concept.is_some() {
            no_concept.choice = None;
        }
        WizardForm {
            concept,
            concept_query: String::new(),
            ctx,
            options,
            texts,
            comments: String::new(),
            passphrase: String::new(),
            attestations: Vec::new(),
            separation_attestation: None,
            no_concept,
        }
    }

    /// The applicability the gate and the draft use: a `units_documented`
    /// answer also turns `physical_interface` on, as `draft_review` does.
    pub fn applicability(&self) -> Applicability {
        let mut a = self.ctx.applicability;
        a.physical_interface |= self.options.contains_key("units_documented");
        a
    }

    /// The questions asked for this function, in order.
    pub fn questions(&self) -> Vec<&'static Question> {
        ReviewWizard::embedded()
            .applicable(self.applicability())
            .collect()
    }

    /// The answers in their on-disk form (`option` or `option: text`), for
    /// the applicable questions answered. Exactly what is drafted and
    /// signed.
    pub fn answers(&self) -> BTreeMap<String, String> {
        let w = ReviewWizard::embedded();
        self.questions()
            .into_iter()
            .filter_map(|q| {
                let o = self.options.get(&q.key)?;
                let needs = w
                    .question(&q.key)
                    .and_then(|q| q.option(o))
                    .is_some_and(|o| o.requires_text);
                let text = needs.then(|| self.texts.get(&q.key).map(String::as_str).unwrap_or(""));
                Some((q.key.clone(), format_answer(o, text)))
            })
            .collect()
    }

    /// The relations the stamp carries: the linked concept's `implements`
    /// (#740 decision 11), then the suggested architecture node's
    /// `part_of`, when the reviewer accepted it (never silently).
    pub fn relations(&self) -> Vec<kovan_common::artifact::relation::RelationRecord> {
        let mut out: Vec<_> = self.concept.iter().map(|c| concepts::implements(c)).collect();
        if let (Some(r), true) = (&self.ctx.suggested_architecture, self.no_concept.link_architecture) {
            out.push(r.clone());
        }
        out
    }

    /// Link concept `id`: clears the no-concept reason (the two exclude
    /// each other, #740 U3).
    pub fn choose_concept(&mut self, id: &str) {
        self.concept = Some(id.to_string());
        self.no_concept.choice = None;
    }

    /// Choose a no-concept reason (`None`: no marker); a reason unlinks the
    /// concept.
    pub fn choose_reason(&mut self, key: Option<&str>) {
        self.no_concept.choice = key.map(str::to_string);
        if key.is_some() {
            self.concept = None;
        }
    }

    /// The finder's matches for the current query, best first.
    pub fn concept_matches(&self) -> Vec<&ConceptChoice> {
        concepts::rank(&self.ctx.concepts, &self.concept_query)
    }

    /// What stops the stamp on the concept part: neither a concept nor a
    /// no-concept reason ("A function with no concept link … carries a 'no
    /// concept' marker with a reason", #740 U3), or the reason's own
    /// problem (Other under 2 characters). `None`: fine.
    pub fn concept_problem(&self) -> Option<String> {
        if self.concept.is_none() && self.no_concept.choice.is_none() {
            return Some(
                "Concept: link the concept this function implements (the finder), or choose a \
                 no-concept reason"
                    .into(),
            );
        }
        self.no_concept.problem()
    }

    /// The live stamp gate over the answers.
    pub fn gate(&self) -> GateResult {
        ReviewWizard::embedded().stamp_gate(&self.answers(), self.applicability())
    }

    /// The note a "Mark as Needs fix instead?" starts from: each prompting
    /// answer's question and option label.
    pub fn needs_fix_note(&self) -> String {
        let w = ReviewWizard::embedded();
        self.gate()
            .prompts
            .iter()
            .filter_map(|c| {
                let q = w.question(&c.question)?;
                let o = q.option(&c.option)?;
                Some(format!("{}: {}", q.text.trim(), o.label.trim()))
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// Why signing failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignFailure {
    WrongPassphrase,
    Other(String),
}

/// A finished write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub what: Purpose,
    /// The `review.md` written, workspace-relative.
    pub review_md: String,
    /// An earlier entry was replaced.
    pub replaced: bool,
}

/// Where the dialog is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    Loading,
    /// No `kovan_root.toml`: run Index fresh first.
    NoRoot,
    SetupKey,
    Generating,
    PickKey,
    /// The chosen key is not in `kovan_root.toml`.
    Register,
    Registering,
    Preparing,
    Refused {
        message: String,
        hint: String,
    },
    Wizard,
    Signing,
    /// Added 2026-10-10 (#770): gathering the re-confirm (worker).
    PreparingReconfirm,
    /// The re-confirm step: the changed callees' diffs and the passphrase.
    Reconfirm,
    NeedsFixForm,
    WritingNeedsFix,
    Done(Outcome),
    /// The keystore or the root cannot be read.
    Failed(String),
}

/// A generated key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Generated {
    pub reviewer: String,
    pub key: String,
    pub path: PathBuf,
    pub registration: Result<KeyRegistration, String>,
}

/// The job in flight (one at a time).
enum Job {
    Keys(Worker<Result<KeysFound, String>>),
    Generate(Worker<Result<Generated, String>>),
    Register(Worker<Result<KeyRegistration, String>>),
    Prepare(Worker<Result<(StampContext, Vec<AttestationChoice>), String>>),
    PrepareReconfirm(Worker<Result<ReconfirmContext, String>>),
    Sign(Worker<Result<WrittenEntry, SignFailure>>),
    NeedsFix(Worker<Result<WrittenEntry, String>>),
}

/// The states job's output.
pub type StatesResult = Result<Vec<WebStamp>, String>;

/// The dialog's whole state (module doc).
pub struct StampFlow {
    root: PathBuf,
    keystore: Keystore,
    pub purpose: Purpose,
    pub target: Target,
    pub step: Step,
    pub keys: Vec<KeyInfo>,
    /// Index into `keys` of the key in use (or picked).
    pub chosen: usize,
    /// The key in use, kept across a reload of the keys (after a
    /// registration or a generation) so the flow goes on with it.
    remembered: Option<(String, String)>,
    pub setup: KeySetupForm,
    pub wizard: Option<WizardForm>,
    /// The needs-fix note.
    pub note: String,
    /// Plain-words outcomes to show (key generated, registered, …).
    pub notices: Vec<String>,
    /// The last error of a form (wrong passphrase, …), shown on it.
    pub error: Option<String>,
    job: Option<Job>,
    refresh: Option<Worker<StatesResult>>,
    /// The fresh states, until the host takes them.
    states: Option<Vec<WebStamp>>,
    /// The target's state after the last refresh (`None`: not refreshed,
    /// or nothing recorded for it).
    pub target_state: Option<WebStamp>,
    pub refresh_error: Option<String>,
    /// The files this dialog wrote, workspace-relative (`review.md`, and
    /// `kovan_root.toml` after a key registration): what commit-and-push
    /// may commit (GitHub #771, `super::commit_push`).
    pub wrote: BTreeSet<String>,
    /// Added 2026-10-10 (#770): comments the review panel's sidebar holds
    /// for this function, put into the wizard's Comments when it opens.
    pub comments_seed: String,
    /// Added 2026-10-10 (#770): the re-confirm being signed.
    pub reconfirm: Option<ReconfirmContext>,
    /// The re-confirm's passphrase (zeroised after use and on drop).
    pub reconfirm_passphrase: String,
}

impl Drop for StampFlow {
    fn drop(&mut self) {
        self.reconfirm_passphrase.zeroize();
    }
}

impl StampFlow {
    /// Open the dialog for `target` over the workspace `root`, reading keys
    /// from `keystore` (injected: tests pass a temporary one).
    pub fn new(root: PathBuf, keystore: Keystore, purpose: Purpose, target: Target) -> StampFlow {
        let mut f = StampFlow {
            root,
            keystore,
            purpose,
            target,
            step: Step::Loading,
            keys: Vec::new(),
            chosen: 0,
            remembered: None,
            setup: KeySetupForm::default(),
            wizard: None,
            note: String::new(),
            notices: Vec::new(),
            error: None,
            job: None,
            refresh: None,
            states: None,
            target_state: None,
            refresh_error: None,
            wrote: BTreeSet::new(),
            comments_seed: String::new(),
            reconfirm: None,
            reconfirm_passphrase: String::new(),
        };
        f.load_keys();
        f
    }

    /// The workspace.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The keystore folder (shown in the setup form).
    pub fn keystore_dir(&self) -> &Path {
        self.keystore.dir()
    }

    /// A step's job is running (Close waits for it; a refresh does not
    /// hold the dialog: [`Self::take_refresh`]).
    pub fn working(&self) -> bool {
        self.job.is_some()
    }

    /// A job (or a refresh) is running.
    pub fn busy(&self) -> bool {
        self.job.is_some() || self.refresh.is_some()
    }

    /// The key in use.
    pub fn key(&self) -> Option<&KeyInfo> {
        self.keys.get(self.chosen)
    }

    fn load_keys(&mut self) {
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.step = Step::Loading;
        self.job = Some(Job::Keys(Worker::spawn(move || find_keys(&root, &ks))));
    }

    /// Take a finished job's result and move on; never blocks.
    pub fn poll(&mut self) {
        if let Some(w) = &self.refresh {
            if let Some(r) = w.try_take() {
                self.refresh = None;
                match r {
                    Ok(states) => {
                        self.target_state = states
                            .iter()
                            .find(|s| s.function == self.target.function)
                            .cloned();
                        self.refresh_error = None;
                        self.states = Some(states);
                    }
                    Err(e) => self.refresh_error = Some(e),
                }
            }
        }
        let Some(job) = self.job.take() else { return };
        match job {
            Job::Keys(w) => match w.try_take() {
                None => self.job = Some(Job::Keys(w)),
                Some(Err(e)) => self.step = Step::Failed(e),
                Some(Ok(found)) => self.keys_loaded(found),
            },
            Job::Generate(w) => match w.try_take() {
                None => self.job = Some(Job::Generate(w)),
                Some(Err(e)) => {
                    self.error = Some(e);
                    self.step = Step::SetupKey;
                }
                Some(Ok(g)) => {
                    self.notices
                        .push(format!("Key generated and saved to {}.", g.path.display()));
                    match &g.registration {
                        Ok(r) => {
                            self.wrote.insert(ROOT_FILE.to_string());
                            self.notices.push(describe_registration(r));
                        }
                        Err(e) => self.notices.push(format!(
                            "The key was saved but not registered in {ROOT_FILE}: {e}"
                        )),
                    }
                    self.setup = KeySetupForm::default();
                    self.remembered = Some((g.reviewer, g.key));
                    self.load_keys();
                }
            },
            Job::Register(w) => match w.try_take() {
                None => self.job = Some(Job::Register(w)),
                Some(Err(e)) => {
                    self.error = Some(e);
                    self.step = Step::Register;
                }
                Some(Ok(r)) => {
                    self.wrote.insert(ROOT_FILE.to_string());
                    self.notices.push(describe_registration(&r));
                    self.load_keys();
                }
            },
            Job::Prepare(w) => match w.try_take() {
                None => self.job = Some(Job::Prepare(w)),
                Some(Err(message)) => {
                    let hint = refusal_hint(&message).to_string();
                    self.step = Step::Refused { message, hint };
                }
                Some(Ok((ctx, attestations))) => {
                    let mut w = WizardForm::new(ctx);
                    w.attestations = attestations;
                    w.comments = self.comments_seed.clone();
                    self.wizard = Some(w);
                    self.step = Step::Wizard;
                }
            },
            Job::PrepareReconfirm(w) => match w.try_take() {
                None => self.job = Some(Job::PrepareReconfirm(w)),
                Some(Err(message)) => {
                    let hint = refusal_hint(&message).to_string();
                    self.step = Step::Refused { message, hint };
                }
                Some(Ok(rc)) => {
                    self.reconfirm = Some(rc);
                    self.step = Step::Reconfirm;
                }
            },
            Job::Sign(w) => match w.try_take() {
                None => self.job = Some(Job::Sign(w)),
                Some(Err(e)) => {
                    let back = if self.purpose == Purpose::Reconfirm {
                        Step::Reconfirm
                    } else {
                        Step::Wizard
                    };
                    self.error = Some(match e {
                        SignFailure::WrongPassphrase => {
                            "Wrong passphrase (or the key file was altered). Nothing was written."
                                .into()
                        }
                        SignFailure::Other(e) => format!("{e}. Nothing was written."),
                    });
                    self.step = back;
                }
                Some(Ok(w)) => self.written(self.purpose, w),
            },
            Job::NeedsFix(w) => match w.try_take() {
                None => self.job = Some(Job::NeedsFix(w)),
                Some(Err(e)) => {
                    self.error = Some(e);
                    self.step = Step::NeedsFixForm;
                }
                Some(Ok(w)) => self.written(Purpose::NeedsFix, w),
            },
        }
    }

    fn keys_loaded(&mut self, found: KeysFound) {
        if !found.root_exists {
            self.step = Step::NoRoot;
            return;
        }
        self.keys = found.keys;
        self.step = match self.keys.len() {
            0 => Step::SetupKey,
            1 => {
                self.chosen = 0;
                return self.use_key();
            }
            _ => {
                let again = self.remembered.as_ref().and_then(|(r, k)| {
                    self.keys
                        .iter()
                        .position(|i| &i.reviewer == r && &i.key == k)
                });
                if let Some(i) = again {
                    self.chosen = i;
                    return self.use_key();
                }
                if self.chosen >= self.keys.len() {
                    self.chosen = 0;
                }
                Step::PickKey
            }
        };
    }

    /// Go on with the key `chosen` (the picker's "Use this key", or the
    /// only key): register it first when stamping with a key the root does
    /// not list; else prepare the stamp, or open the needs-fix form.
    pub fn use_key(&mut self) {
        let Some(k) = self.key() else { return };
        let (registered, id) = (k.registered, (k.reviewer.clone(), k.key.clone()));
        self.remembered = Some(id);
        self.error = None;
        match self.purpose {
            Purpose::Stamp | Purpose::Reconfirm if !registered => self.step = Step::Register,
            Purpose::Stamp | Purpose::Reconfirm => self.prepare(),
            Purpose::NeedsFix => self.step = Step::NeedsFixForm,
        }
    }

    /// Generate a key from the setup form, save it and register it (worker).
    /// Refused (no change) while the form has problems or a job runs.
    pub fn generate_key(&mut self) -> bool {
        if self.job.is_some() || !self.setup.problems().is_empty() {
            return false;
        }
        let reviewer = self.setup.reviewer.trim().to_string();
        let name = self.setup.name.trim().to_string();
        let mut pass = std::mem::take(&mut self.setup.passphrase);
        self.setup.confirm.zeroize();
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.error = None;
        self.step = Step::Generating;
        self.job = Some(Job::Generate(Worker::spawn(move || {
            let today = date_of(&now_local()).unwrap_or_default();
            let key_id = next_key_id(&root, &ks, &reviewer);
            let made = generate(&reviewer, &key_id, &today, &pass);
            pass.zeroize();
            let (kf, _key) = made.map_err(|e| e.to_string())?;
            let path = ks.save(&kf).map_err(|e| e.to_string())?;
            let name = (!name.is_empty()).then_some(name.as_str());
            let registration = register_key(&root, &kf, name, &today);
            Ok(Generated {
                reviewer: kf.reviewer.clone(),
                key: kf.key.clone(),
                path,
                registration,
            })
        })));
        true
    }

    /// Register the chosen key in `kovan_root.toml` (worker).
    pub fn register_chosen(&mut self) -> bool {
        let Some(k) = self.key().cloned() else {
            return false;
        };
        if self.job.is_some() {
            return false;
        }
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.error = None;
        self.step = Step::Registering;
        self.job = Some(Job::Register(Worker::spawn(move || {
            let kf = ks.load(&k.reviewer, &k.key).map_err(|e| e.to_string())?;
            let today = date_of(&now_local()).unwrap_or_default();
            register_key(&root, &kf, None, &today)
        })));
        true
    }

    /// Check the function and gather the wizard's context (worker), or the
    /// re-confirm's for [`Purpose::Reconfirm`]; also "Try again" after a
    /// refusal.
    pub fn prepare(&mut self) {
        let Some(k) = self.key() else { return };
        if self.job.is_some() {
            return;
        }
        if self.purpose == Purpose::Reconfirm {
            let (root, function, by) = (
                self.root.clone(),
                self.target.function.clone(),
                k.reviewer.clone(),
            );
            self.step = Step::PreparingReconfirm;
            self.job = Some(Job::PrepareReconfirm(Worker::spawn(move || {
                prepare_reconfirm(&root, &function, &by)
            })));
            return;
        }
        let (root, function, by) = (
            self.root.clone(),
            self.target.function.clone(),
            k.reviewer.clone(),
        );
        self.step = Step::Preparing;
        self.job = Some(Job::Prepare(Worker::spawn(move || {
            let ctx = prepare_stamp(&root, &function, &by)?;
            Ok((ctx, attestation_choices_in(&root, &by)))
        })));
    }

    /// Whether "Sign and write review.md" is allowed: on the wizard, the
    /// gate open, a passphrase typed, no job running.
    pub fn can_sign(&self) -> bool {
        self.step == Step::Wizard
            && self.job.is_none()
            && self
                .wizard
                .as_ref()
                .is_some_and(|w| {
                    !w.passphrase.is_empty()
                        && w.gate().stampable()
                        && w.concept_problem().is_none()
                })
    }

    /// Unlock the key, draft the stamp with the wizard's answers at `HEAD`,
    /// sign and write it (worker). The passphrase is taken out of the form
    /// and zeroised in the worker.
    pub fn sign(&mut self) -> bool {
        if !self.can_sign() {
            return false;
        }
        let (Some(k), Some(w)) = (self.key().cloned(), self.wizard.as_mut()) else {
            return false;
        };
        let mut pass = std::mem::take(&mut w.passphrase);
        let answers = w.answers();
        let comments = w.comments.clone();
        let separation_attestation = w.separation_attestation.clone();
        let no_concept = w.no_concept.stored();
        let relations = w.relations();
        let seen_hash = w.ctx.hash.clone();
        let (root, ks, function) = (
            self.root.clone(),
            self.keystore.clone(),
            self.target.function.clone(),
        );
        self.error = None;
        self.step = Step::Signing;
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
            let req = StampRequest {
                function,
                by: k.reviewer.clone(),
                checklist: answers.clone(),
                separation_attestation,
                no_concept,
                relations,
                ..StampRequest::default()
            };
            let mut d = draft_stamp(&root, &req).map_err(SignFailure::Other)?;
            // #740 decision 7: never certify code the reviewer was not shown.
            if !seen_hash.is_empty() && d.entry.review.hash != seen_hash {
                return Err(SignFailure::Other(
                    "the function changed while you were reviewing it (a pull, a checkout or a \
                     quick fix): the stamp is refused"
                        .into(),
                ));
            }
            if d.entry.review.checklist != answers {
                return Err(SignFailure::Other(
                    "the drafted checklist is not the answers given".into(),
                ));
            }
            key.sign_review(&mut d.entry)
                .map_err(|e| SignFailure::Other(e.to_string()))?;
            write_review(&root, &d.entry, comments.trim()).map_err(SignFailure::Other)
        })));
        true
    }

    /// Whether "Sign the re-confirmation" is allowed: on the re-confirm
    /// step, a passphrase typed, no job running.
    pub fn can_reconfirm(&self) -> bool {
        self.step == Step::Reconfirm
            && self.job.is_none()
            && self.reconfirm.is_some()
            && !self.reconfirm_passphrase.is_empty()
    }

    /// Unlock the key and sign the re-confirmation (worker): the previous
    /// review's answers re-signed at `HEAD` ([`super::reconfirm`]).
    pub fn sign_reconfirm(&mut self) -> bool {
        if !self.can_reconfirm() {
            return false;
        }
        let (Some(k), Some(rc)) = (self.key().cloned(), self.reconfirm.clone()) else {
            return false;
        };
        let mut pass = std::mem::take(&mut self.reconfirm_passphrase);
        let (root, ks) = (self.root.clone(), self.keystore.clone());
        self.error = None;
        self.step = Step::Signing;
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
            sign_reconfirm(&root, &rc, &key).map_err(SignFailure::Other)
        })));
        true
    }

    /// "Mark for re-review" (#740 decision 8): leave the re-confirm for the
    /// full wizard, as a Stamp.
    pub fn switch_to_review(&mut self) {
        if self.job.is_some() {
            return;
        }
        self.reconfirm_passphrase.zeroize();
        self.reconfirm = None;
        self.purpose = Purpose::Stamp;
        self.error = None;
        self.prepare();
    }

    /// "Mark as Needs fix instead?": switch to the needs-fix form, the note
    /// started from the prompting answers.
    pub fn switch_to_needs_fix(&mut self) {
        if self.job.is_some() {
            return;
        }
        if let Some(w) = &self.wizard {
            if self.note.trim().is_empty() {
                self.note = w.needs_fix_note();
            }
        }
        self.purpose = Purpose::NeedsFix;
        self.error = None;
        self.step = Step::NeedsFixForm;
    }

    /// Whether "Write needs fix" is allowed.
    pub fn can_write_needs_fix(&self) -> bool {
        self.step == Step::NeedsFixForm
            && self.job.is_none()
            && self.note.trim().chars().count() >= 2
            && self.key().is_some()
    }

    /// Draft and write the needs-fix entry (worker).
    pub fn write_needs_fix(&mut self) -> bool {
        if !self.can_write_needs_fix() {
            return false;
        }
        let Some(k) = self.key() else { return false };
        let (root, function, by, note) = (
            self.root.clone(),
            self.target.function.clone(),
            k.reviewer.clone(),
            self.note.trim().to_string(),
        );
        self.error = None;
        self.step = Step::WritingNeedsFix;
        self.job = Some(Job::NeedsFix(Worker::spawn(move || {
            let n = draft_needs_fix_for(&root, &function, &by, &note)?;
            write_needs_fix(&root, &n, "")
        })));
        true
    }

    fn written(&mut self, what: Purpose, w: WrittenEntry) {
        let review_md = w
            .review_md
            .strip_prefix(&self.root)
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_else(|_| w.review_md.display().to_string());
        if let Some(f) = self.wizard.as_mut() {
            f.passphrase.zeroize();
        }
        self.wrote.insert(review_md.clone());
        self.step = Step::Done(Outcome {
            what,
            review_md,
            replaced: w.replaced,
        });
        self.refresh();
    }

    /// Recompute every function's state from `review.md` (worker); the
    /// result goes to the review bar through [`Self::take_states`].
    pub fn refresh(&mut self) {
        if self.refresh.is_none() {
            let root = self.root.clone();
            self.refresh = Some(Worker::spawn(move || stamp_states(&root)));
        }
    }

    /// The fresh states, once.
    pub fn take_states(&mut self) -> Option<Vec<WebStamp>> {
        self.states.take()
    }

    /// The refresh still running, for the host to keep polling after the
    /// dialog closes.
    pub fn take_refresh(&mut self) -> Option<Worker<StatesResult>> {
        self.refresh.take()
    }

    /// Poll until no job runs (tests only; the UI never waits).
    #[cfg(test)]
    pub fn wait(&mut self) {
        let start = std::time::Instant::now();
        while self.busy() {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(120),
                "stamp flow job did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
            self.poll();
        }
    }
}
