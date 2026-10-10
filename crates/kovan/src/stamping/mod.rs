//! **Signed review stamps from desktop kovan** (GitHub #770, #762, #765):
//! the GUI-free layer the stamp dialog calls. Plain functions over a
//! workspace folder, no egui, safe to run on a worker thread.
//!
//! ```text
//!   prepare_stamp(root, function, by) ──> StampContext (the wizard's
//!        │   applicability and prefilled answers; the same refusals)
//!   draft_stamp(root, request) ──> ReviewEntry (unsigned)
//!        │   gathers: kovan.toml (refreshed in memory), HEAD, Cargo.lock,
//!        │   callee hashes, git's test authorship,
//!        │   the change's authorship (authorship.rs) ── pure part: kovan_common::review::draft
//!        v
//!   UnlockedKey::sign_review (the human, with their passphrase)
//!        v
//!   write_review(root, &entry, comments) ──> <folder>/review.md (spliced)
//!                                            ── pure part: kovan_common::review::review_md_write
//!   register_key(root, &key_file, …) ──> kovan_root.toml (appended text)
//!                                            ── pure part: kovan_common::review::root_append
//!   stamp_states(root) ──> Vec<StampState> for kovan-web's Facts::new
//!                          (states.rs: GitFacts from git, engine::evaluate)
//! ```
//!
//! The dialog's state machine (steps, form checks, worker jobs) is
//! [`flow`]; its drawing is `app/stamp_dialog.rs` (GUI feature).
//!
//! **The need-you queue and commit-and-push** (GitHub #771, #740 U1/U5):
//! [`queue`] builds the ⚑ rows from the engine's verdict and git
//! ([`new_code`] folds new functions per commit, [`authorship`] reads the
//! trailers), [`relocate`] acknowledges moves and records deletions,
//! [`recent`] lists the last places reviewed, and [`commit_push`] commits
//! only the files kovan wrote and pushes them, never to `main`. The
//! drawing is `app/need_you_view.rs`.
//!
//! # Reused, not rewritten
//!
//! - `kovan.toml` reading and the rust-analyzer-free refresh:
//!   [`kovan_common::review::index::FolderIndex`],
//!   [`kovan_common::code_index::refresh::refresh_folder`] and
//!   [`kovan_common::code_index::refresh::all_claims`];
//! - `review.md` parsing: [`kovan_common::review::review_md::parse_review_md`];
//! - git: [`git`] (`git`, `rev_parse`, `show`,
//!   `is_dirty`), the `Cargo.lock` hash as `kovan-cli test` takes it
//!   ([`kovan_common::review::hash::sha256_tagged`] of the file);
//! - file discovery: `kovan_discovery::discover` (honours `.gitignore`,
//!   skips hidden folders), as `commands::test_evidence` does.
//!
//! # The index is refreshed in memory, never written
//!
//! Every function's `hash`/`doc_hash` is recomputed from the working tree
//! with the `syn` hasher before anything is drafted or judged, so an edit
//! that was not re-indexed still shows (an edited function is marked
//! `index_out_of_date`). Nothing here writes `kovan.toml`: that is
//! `kovan-cli index`'s job.
//!
//! # AI agents never stamp
//!
//! Nothing here signs. [`write_review`] refuses an unsigned review, and a
//! signature needs a key unlocked by its human owner's passphrase.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use kovan_common::artifact::relation::RelationRecord;
use kovan_common::code_index::refresh::{all_claims, refresh_folder};
use kovan_common::review::draft::{draft_needs_fix, draft_review, DraftError, DraftInput};
use kovan_common::review::hash::sha256_tagged;
use kovan_common::review::index::{FolderIndex, FunctionIndex};
use kovan_common::review::review_md::{parse_review_md, NeedsFixEntry, ReviewDocument, ReviewEntry};
use kovan_common::review::review_md_write::{upsert_needs_fix, upsert_review};
use kovan_common::review::root::{ReviewRoot, Reviewer};
use kovan_common::review::root_append::{
    append_reviewer, append_reviewer_key, declare_founder, founding_reviewer, is_bare_key,
    unadmitted_reviewer,
};
use kovan_common::review::signed_at::{date_of, now_local};
use kovan_common::review::signing::keystore::KeyFile;
use kovan_common::review::types::ChangeAuthorship;
use kovan_common::review::wizard::{vv_case_author_prefill, Applicability, ReviewWizard};

pub mod git;

pub mod authorship;
pub mod commit_push;
pub mod flow;
pub mod new_code;
/// Organisations and separation attestations (GitHub #810).
pub mod organisations;
pub mod queue;
pub mod recent;
pub mod relocate;
pub mod review_mode;
mod states;

pub use states::{
    evaluate_loaded, evaluate_loaded_with, evaluate_workspace, evaluate_workspace_with, git_facts, stamp_states,
    WorkspaceEvaluation,
};

/// `review.md`.
pub const REVIEW_MD: &str = "review.md";
/// `kovan.toml`.
pub const KOVAN_TOML: &str = "kovan.toml";
/// `kovan_root.toml`.
pub const ROOT_FILE: &str = "kovan_root.toml";

/// One folder's `review.md` as read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewMdFile {
    pub text: String,
    pub doc: ReviewDocument,
}

/// The workspace as the stamp dialog sees it.
#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    /// `HEAD`, or empty when there is no commit (or no git).
    pub head: String,
    /// Every code-folder `kovan.toml`, refreshed from the working tree.
    pub indexes: Vec<FolderIndex>,
    /// Workspace-relative folder -> its `review.md`.
    pub reviews: BTreeMap<String, ReviewMdFile>,
    /// `kovan_root.toml`'s review sections (empty when there is none).
    pub review_root: ReviewRoot,
}

/// Workspace-relative, `/`-separated.
fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .map(|r| r.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

/// `dir/name`, or `name` for the workspace folder itself.
pub(crate) fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// The `.rs` files directly in `dir` (workspace-relative), by file name.
fn rust_files(root: &Path, dir: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let Ok(rd) = std::fs::read_dir(root.join(dir)) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.extension().and_then(|x| x.to_str()) == Some("rs") && p.is_file() {
            if let (Some(name), Ok(text)) = (
                p.file_name().and_then(|n| n.to_str()),
                std::fs::read_to_string(&p),
            ) {
                out.insert(name.to_string(), text);
            }
        }
    }
    out
}

/// Read the workspace (module doc): every `review.md`, every code-folder
/// `kovan.toml` written for its own folder (refreshed in memory), and
/// `kovan_root.toml`. `Err` only when `kovan_root.toml` exists and does not
/// parse (the registry cannot be built then).
pub fn load_workspace(root: &Path) -> Result<Workspace, String> {
    let head = git::rev_parse(root, "HEAD").unwrap_or_default();
    let mut reviews = BTreeMap::new();
    for p in kovan_discovery::discover(root, &["md"]) {
        if p.file_name().and_then(|f| f.to_str()) != Some(REVIEW_MD) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        let dir = p.parent().map(|d| rel(root, d)).unwrap_or_default();
        let doc = parse_review_md(&text);
        reviews.insert(dir, ReviewMdFile { text, doc });
    }
    let by_path: BTreeMap<String, ReviewDocument> = reviews
        .iter()
        .map(|(d, f)| (join(d, REVIEW_MD), f.doc.clone()))
        .collect();
    let claims = all_claims(&by_path);
    let mint = if head.is_empty() {
        "none"
    } else {
        head.as_str()
    };
    let mut indexes = Vec::new();
    for p in kovan_discovery::discover(root, &["toml"]) {
        if p.file_name().and_then(|f| f.to_str()) != Some(KOVAN_TOML) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else {
            continue;
        };
        let Ok(fi) = FolderIndex::parse(&text) else {
            continue;
        };
        let dir = p.parent().map(|d| rel(root, d)).unwrap_or_default();
        if fi.dir != dir {
            continue; // a fixture or a copy, not this folder's index
        }
        let files = rust_files(root, &dir);
        let doc = reviews.get(&dir).map(|f| &f.doc);
        let (fresh, _) = refresh_folder(&fi.krate, &fi.dir, Some(&fi), &files, doc, &claims, mint);
        indexes.push(fresh);
    }
    let root_path = root.join(ROOT_FILE);
    let review_root = match std::fs::read_to_string(&root_path) {
        Ok(t) => ReviewRoot::parse(&t).map_err(|e| e.to_string())?,
        Err(_) => ReviewRoot::default(),
    };
    Ok(Workspace {
        root: root.to_path_buf(),
        head,
        indexes,
        reviews,
        review_root,
    })
}

impl Workspace {
    /// The function `function` names: an `fn:` id, or a call-graph id
    /// (`file.rs::Type::f#2`, what kovan-web uses). Returns its folder
    /// index, file name and entry.
    pub fn find(&self, function: &str) -> Option<(&FolderIndex, &str, &FunctionIndex)> {
        self.indexes.iter().find_map(|idx| {
            idx.functions()
                .find(|(file, f)| f.id == function || call_graph_id(idx, file, f) == function)
                .map(|(file, f)| (idx, file, f))
        })
    }

    /// Function id -> its index hash now, over every folder.
    pub fn hashes(&self) -> BTreeMap<String, String> {
        self.indexes
            .iter()
            .flat_map(|i| i.functions().map(|(_, f)| (f.id.clone(), f.hash.clone())))
            .collect()
    }
}

/// The call-graph id of an index entry: `<file path>::<qual>` (the
/// index's `qual` is the call graph's id without the file prefix, `#k`
/// kept: `kovan_common::code_index::folders`).
pub fn call_graph_id(idx: &FolderIndex, file: &str, f: &FunctionIndex) -> String {
    format!("{}::{}", idx.file_path(file), f.qual)
}

/// `sha256:` of `<root>/Cargo.lock`, as `kovan-cli test` records it; `None`
/// when there is no lock file.
pub fn cargo_lock_hash(root: &Path) -> Option<String> {
    std::fs::read(root.join("Cargo.lock"))
        .ok()
        .map(|b| sha256_tagged(&b))
}

/// What the reviewer chose in the dialog.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StampRequest {
    /// An `fn:` id or a call-graph id.
    pub function: String,
    /// The reviewer id; must be the signing key's reviewer.
    pub by: String,
    /// Wizard answers by question key.
    pub checklist: BTreeMap<String, String>,
    /// The reviewed change's authorship; `None` fills it from git's
    /// commit trailers ([`authorship::change_authorship`], #771).
    pub authorship: Option<ChangeAuthorship>,
    pub no_concept: Option<String>,
    pub relations: Vec<RelationRecord>,
    /// The reviewer's `[[reviewer.separation]]` attestation the review
    /// relies on for rung 5 (GitHub #810); `None` by default. Signed into
    /// the stamp (v3 bytes).
    pub separation_attestation: Option<String>,
}

/// A drafted, unsigned stamp and where it will be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DraftedStamp {
    pub entry: ReviewEntry,
    /// The folder's `review.md` (absolute).
    pub review_md: PathBuf,
    /// The function's call-graph id (kovan-web's key).
    pub call_graph_id: String,
}

/// The function `function` names in `ws`, checked for stamping at
/// `HEAD`: refused when there is no commit, the function is not indexed,
/// its file differs from `HEAD` (the stamp certifies `HEAD`), or the
/// index's hash is not the hash of the function at `HEAD`.
fn locate<'w>(
    root: &Path,
    ws: &'w Workspace,
    function: &str,
) -> Result<(&'w FolderIndex, &'w str, &'w FunctionIndex), String> {
    if ws.head.is_empty() {
        return Err("no commit: a stamp certifies a commit".into());
    }
    let (idx, file, f) = ws
        .find(function)
        .ok_or_else(|| format!("{function} is not in any kovan.toml (run kovan-cli index)"))?;
    let path = idx.file_path(file);
    if git::is_dirty(root, &path)? {
        return Err(format!(
            "{path} has changes not committed: a stamp certifies HEAD, commit first"
        ));
    }
    let mut cache = states::FileCache::new();
    let at_head = states::fn_at(&mut cache, root, &ws.head, &path, &f.qual).map(|h| h.hashes.hash);
    if at_head.as_deref() != Some(f.hash.as_str()) {
        return Err(format!(
            "{path}::{}: the index hash is not the function's hash at HEAD",
            f.qual
        ));
    }
    Ok((idx, file, f))
}

/// What the stamp dialog needs before the wizard is shown (GitHub #770):
/// which questions apply, and the starting answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StampContext {
    /// The function's `fn:` id.
    pub fn_id: String,
    /// Its call-graph id (kovan-web's key).
    pub call_graph_id: String,
    /// `file.rs::qual`, workspace-relative.
    pub path: String,
    /// The folder's `review.md`, workspace-relative.
    pub review_md: String,
    /// The wizard's applicability, exactly as [`draft_stamp`] will judge
    /// it (`physical_interface` from the index; a `units_documented` answer
    /// also turns it on there, as in [`kovan_common::review::draft`]).
    pub applicability: Applicability,
    /// The starting answers: this reviewer's previous answers still valid
    /// ([`ReviewWizard::prefill`]), plus `vv_case_author` from git
    /// ([`vv_case_author_prefill`]) when not answered before.
    pub answers: BTreeMap<String, String>,
    /// This reviewer already has a review of the function (it is replaced).
    pub restamp: bool,
    /// Added 2026-10-10 (#770, #740 decision 7): the function's hash when
    /// the wizard opened. Signing refuses when the drafted hash differs
    /// (the code changed while it was being reviewed).
    pub hash: String,
    /// Added 2026-10-10 (#770, #760 q11): the starting no-concept reason,
    /// this reviewer's previous one, else "upstream control flow / solver
    /// structure" for a confirmed port
    /// ([`review_mode::prefill::no_concept_prefill`]).
    pub no_concept: Option<String>,
    /// Added 2026-10-10 (#770, #760 q11): the architecture node suggested
    /// as a `part_of` relation, off until the reviewer accepts it
    /// ([`review_mode::prefill::suggest_architecture`]).
    pub suggested_architecture: Option<RelationRecord>,
}

/// Check that `function` can be stamped by `by` at `HEAD` now and gather
/// the wizard's context ([`StampContext`]). Refused for every reason
/// [`draft_stamp`] refuses before the answers matter (no commit, not
/// indexed, uncommitted changes in its file, index hash not the hash at
/// `HEAD`, index entry out of date, a callee with no hash), with the same
/// messages, and (added 2026-10-10, #740: "bottom-up is an enforced rule")
/// while a workspace callee has no valid stamp
/// ([`review_mode::blockers_of`]; a recursion partner never blocks).
/// Reads git and runs the staleness engine; call it off the UI thread.
pub fn prepare_stamp(root: &Path, function: &str, by: &str) -> Result<StampContext, String> {
    let ws = load_workspace(root)?;
    let (idx, file, f) = locate(root, &ws, function)?;
    let path = format!("{}::{}", idx.file_path(file), f.qual);
    if f.index_out_of_date {
        return Err(DraftError::IndexOutOfDate { path }.to_string());
    }
    let hashes = ws.hashes();
    if let Some(c) = f.callees.iter().find(|c| !hashes.contains_key(*c)) {
        return Err(DraftError::MissingCalleeHash(c.clone()).to_string());
    }
    let snap = review_mode::Snapshot::load(root)?;
    let blocked = review_mode::blockers_of(&snap, &f.id);
    if !blocked.is_empty() {
        let names: Vec<&str> = blocked.iter().map(|(_, n)| n.as_str()).collect();
        return Err(format!(
            "bottom-up: {path} calls {} without a valid stamp; review {} first",
            names.join(", "),
            if names.len() == 1 { "it" } else { "them" }
        ));
    }
    let folder = ws.reviews.get(&idx.dir);
    let mut msgs = states::MessageCache::new();
    let tests = states::tests_at(root, &ws.head, &idx.dir, &f.id, &mut msgs);
    let applicability = Applicability {
        is_port: folder
            .and_then(|m| m.doc.upstream())
            .is_some_and(|u| u.is_port),
        physical_interface: f.physical_interface,
        tests: tests.as_ref().map(|t| t.authorship()).unwrap_or_default(),
    };
    let previous = folder.and_then(|m| {
        m.doc
            .reviews()
            .find(|r| r.function_id() == f.id && r.review.by == by)
    });
    let wizard = ReviewWizard::embedded();
    let mut answers = previous
        .map(|r| wizard.prefill(&r.review.checklist, applicability))
        .unwrap_or_default();
    let messages: Vec<String> = tests
        .map(|t| t.commit_messages.into_values().flatten().collect())
        .unwrap_or_default();
    if let Some(a) = vv_case_author_prefill(&messages) {
        answers
            .entry("vv_case_author".to_string())
            .or_insert_with(|| a.to_string());
    }
    let upstream = folder.and_then(|m| m.doc.upstream());
    let architectures = ws
        .reviews
        .values()
        .flat_map(|m| m.doc.architectures());
    Ok(StampContext {
        fn_id: f.id.clone(),
        call_graph_id: call_graph_id(idx, file, f),
        path,
        review_md: join(&idx.dir, REVIEW_MD),
        applicability,
        answers,
        restamp: previous.is_some(),
        hash: f.hash.clone(),
        no_concept: review_mode::prefill::no_concept_prefill(
            previous.and_then(|r| r.review.no_concept.as_deref()),
            applicability.is_port,
        ),
        suggested_architecture: review_mode::prefill::suggest_architecture(
            &f.id,
            upstream,
            architectures,
        ),
    })
}

/// Draft the stamp for `req` at `HEAD` (module doc). Refused when there is
/// no commit, the function is not indexed, its file differs from `HEAD`
/// (the stamp certifies `HEAD`), the index's hash is not the hash of the
/// function at `HEAD`, the pure draft refuses
/// ([`kovan_common::review::draft::DraftError`]), or (#810) the request
/// names a separation attestation the reviewer does not have.
pub fn draft_stamp(root: &Path, req: &StampRequest) -> Result<DraftedStamp, String> {
    let ws = load_workspace(root)?;
    let (idx, file, f) = locate(root, &ws, &req.function)?;
    let path = idx.file_path(file);
    let hashes = ws.hashes();
    let folder = ws.reviews.get(&idx.dir);
    let previous = folder.and_then(|m| {
        m.doc
            .reviews()
            .find(|r| r.function_id() == f.id && r.review.by == req.by)
            .map(|r| r.kovan.clone())
    });
    let now = now_local();
    let mut msgs = states::MessageCache::new();
    let tests = states::tests_at(root, &ws.head, &idx.dir, &f.id, &mut msgs)
        .map(|t| t.authorship())
        .unwrap_or_default();
    // The reviewed change's authorship, from the commit trailers (#771,
    // #764): since this reviewer's previous review, else all of it.
    let since = folder.and_then(|m| {
        m.doc
            .reviews()
            .find(|r| r.function_id() == f.id && r.review.by == req.by)
            .map(|r| r.review.commit.clone())
    });
    let authorship = req.authorship.clone().or_else(|| {
        authorship::change_authorship(root, &ws.head, &path, f.lines, since.as_deref())
    });
    let input = DraftInput {
        function: f.clone(),
        file: path,
        callee_hashes: f
            .callees
            .iter()
            .filter_map(|c| hashes.get(c).map(|h| (c.clone(), h.clone())))
            .collect(),
        by: req.by.clone(),
        date: date_of(&now).unwrap_or_default(),
        now,
        commit: ws.head.clone(),
        cargo_lock: cargo_lock_hash(root),
        checklist: req.checklist.clone(),
        tests,
        is_port: folder
            .and_then(|m| m.doc.upstream())
            .is_some_and(|u| u.is_port),
        authorship,
        no_concept: req.no_concept.clone(),
        relations: req.relations.clone(),
        previous,
    };
    let mut entry = draft_review(&input).map_err(|e| e.to_string())?;
    if let Some(id) = &req.separation_attestation {
        let own = ws
            .review_root
            .reviewer(&req.by)
            .is_some_and(|r| r.separations.iter().any(|a| &a.id == id));
        if !own {
            return Err(format!(
                "{} has no separation attestation {id:?} in {ROOT_FILE}",
                req.by
            ));
        }
        entry.review.separation_attestation = Some(id.clone());
    }
    Ok(DraftedStamp {
        entry,
        review_md: root.join(join(&idx.dir, REVIEW_MD)),
        call_graph_id: call_graph_id(idx, file, f),
    })
}

/// Draft an open needs-fix on `function` at `HEAD` by `by`.
pub fn draft_needs_fix_for(
    root: &Path,
    function: &str,
    by: &str,
    note: &str,
) -> Result<NeedsFixEntry, String> {
    let ws = load_workspace(root)?;
    if ws.head.is_empty() {
        return Err("no commit: a needs-fix is raised against a commit".into());
    }
    let (idx, file, f) = ws
        .find(function)
        .ok_or_else(|| format!("{function} is not in any kovan.toml"))?;
    let now = now_local();
    draft_needs_fix(
        f,
        &idx.file_path(file),
        by,
        &date_of(&now).unwrap_or_default(),
        &now,
        &ws.head,
        note,
    )
    .map_err(|e| e.to_string())
}

/// The folder `review.md` of an entry whose `path` is `file.rs::item`.
fn review_md_for(root: &Path, path: Option<String>) -> Result<PathBuf, String> {
    let path = path.ok_or("the entry has no path")?;
    let (file, _) = path
        .split_once(".rs::")
        .ok_or_else(|| format!("{path} is not file.rs::item"))?;
    let dir = file.rsplit_once('/').map_or("", |(d, _)| d);
    Ok(root.join(join(dir, REVIEW_MD)))
}

/// Where a write went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenEntry {
    pub review_md: PathBuf,
    /// An earlier entry was replaced (else appended).
    pub replaced: bool,
}

fn write_text(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Write a **signed** review into its folder's `review.md` (replacing this
/// reviewer's earlier review of the function; every other byte kept:
/// [`kovan_common::review::review_md_write`]). An unsigned entry is refused.
pub fn write_review(
    root: &Path,
    entry: &ReviewEntry,
    comments: &str,
) -> Result<WrittenEntry, String> {
    if entry.review.signature.is_none() {
        return Err(
            "refusing to write an unsigned review: sign it with the reviewer's key first".into(),
        );
    }
    let path = review_md_for(root, entry.path())?;
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let w = upsert_review(&old, entry, comments).map_err(|e| e.to_string())?;
    write_text(&path, &w.text)?;
    Ok(WrittenEntry {
        review_md: path,
        replaced: w.replaced,
    })
}

/// Write a needs-fix entry into its folder's `review.md` (replacing the
/// entry with the same id, else appending).
pub fn write_needs_fix(
    root: &Path,
    entry: &NeedsFixEntry,
    comments: &str,
) -> Result<WrittenEntry, String> {
    let path = review_md_for(root, entry.path())?;
    let old = std::fs::read_to_string(&path).unwrap_or_default();
    let w = upsert_needs_fix(&old, entry, comments).map_err(|e| e.to_string())?;
    write_text(&path, &w.text)?;
    Ok(WrittenEntry {
        review_md: path,
        replaced: w.replaced,
    })
}

/// What [`register_key`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyRegistration {
    /// The root had no reviewer and no founder: this reviewer was
    /// registered as the founding maintainer, its key trusted on first use.
    Founder,
    /// A new reviewer, not admitted: counts for nothing until a maintainer
    /// admits it (`UnlockedKey::admit`).
    AwaitingAdmission,
    /// A key added to an existing reviewer; `needs_endorsement` when it is
    /// not the reviewer's first key and carries no endorsement yet.
    KeyAdded { needs_endorsement: bool },
}

fn read_root(root: &Path) -> Result<(PathBuf, String), String> {
    let path = root.join(ROOT_FILE);
    let text = std::fs::read_to_string(&path).map_err(|e| {
        format!(
            "{}: {e} (open the workspace in kovan first)",
            path.display()
        )
    })?;
    Ok((path, text))
}

/// Register the key in `kf` in `<root>/kovan_root.toml` (appended text,
/// comments kept: [`kovan_common::review::root_append`]). Trust on first
/// use is given only to the first reviewer of a root with no reviewer and
/// either no founder or `[code_review] founder` already naming this
/// reviewer (what "Index fresh" writes when the keystore holds one
/// identity; added 2026-10-10 for the stamp dialog, #770). Any other root
/// registers the reviewer as awaiting admission (conservative: a root
/// whose founder is someone else trusts this reviewer only once a
/// maintainer admits it). `date` is
/// `YYYY-MM-DD`; `name` is display only. The file must exist.
pub fn register_key(
    root: &Path,
    kf: &KeyFile,
    name: Option<&str>,
    date: &str,
) -> Result<KeyRegistration, String> {
    let (path, text) = read_root(root)?;
    let parsed = ReviewRoot::parse(&text).map_err(|e| e.to_string())?;
    let key = kf.reviewer_key();
    let founder = parsed.code_review.as_ref().and_then(|c| c.founder.clone());
    let (new, what) = match parsed.reviewer(&kf.reviewer) {
        Some(r) => {
            let first = r.keys.is_empty();
            let t = append_reviewer_key(&text, &kf.reviewer, &key).map_err(|e| e.to_string())?;
            (
                t,
                KeyRegistration::KeyAdded {
                    needs_endorsement: !first && is_bare_key(&key),
                },
            )
        }
        None if parsed.reviewers.is_empty()
            && founder.as_ref().is_none_or(|f| *f == kf.reviewer) =>
        {
            let t = match founder {
                // Named already (by "Index fresh" from the keystore, or by
                // hand): only the founding reviewer entry is added.
                Some(_) => text.clone(),
                None => declare_founder(&text, &kf.reviewer).map_err(|e| e.to_string())?,
            };
            let r = founding_reviewer(&kf.reviewer, name, key, date);
            (
                append_reviewer(&t, &r).map_err(|e| e.to_string())?,
                KeyRegistration::Founder,
            )
        }
        None => {
            let r = unadmitted_reviewer(&kf.reviewer, name, key);
            (
                append_reviewer(&text, &r).map_err(|e| e.to_string())?,
                KeyRegistration::AwaitingAdmission,
            )
        }
    };
    write_text(&path, &new)?;
    Ok(what)
}

/// Append a reviewer prepared (and, when not the founder, admitted) by
/// the caller to `<root>/kovan_root.toml`.
pub fn register_reviewer(root: &Path, reviewer: &Reviewer) -> Result<(), String> {
    let (path, text) = read_root(root)?;
    let new = append_reviewer(&text, reviewer).map_err(|e| e.to_string())?;
    write_text(&path, &new)
}

#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod need_you_tests;
#[cfg(test)]
mod tests;
