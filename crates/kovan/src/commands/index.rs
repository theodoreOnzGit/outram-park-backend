//! `kovan-cli index`: the code index (GitHub #767). One `rust-analyzer scip`
//! run (or `--scip <file>`) becomes every folder's `kovan.toml` and each
//! crate's `kovan_links.json`; `--refresh` updates the `kovan.toml` files
//! **without rust-analyzer**. The pure halves are
//! [`kovan_common::code_index`]; this module reads and writes files and
//! runs `rust-analyzer`, `cargo metadata` and `git`.
//!
//! # Full run
//!
//! 1. **rust-analyzer.** The installed version is compared with the pin in
//!    `kovan_root.toml` (`[code_review] rust_analyzer`; #739 D4): a
//!    mismatch warns (the output may differ from CI's) and the index is
//!    regenerated. `--pin-rust-analyzer` writes the installed version into
//!    an existing `kovan_root.toml`; nothing else ever changes the pin.
//!    With rust-analyzer absent and no
//!    `--scip` file, the run stops with [`IndexCmdError::RustAnalyzerMissing`]
//!    before reading or writing anything: rust-analyzer is needed only to
//!    regenerate the index, never to use it.
//! 2. **SCIP.** `rust-analyzer scip` into `target/kovan-scip/index.scip`
//!    (about 3-4 min for the workspace), or `--scip <file>`; an index
//!    written by another rust-analyzer than the installed one is
//!    regenerated (D4).
//! 3. **Links** per crate in scope, from the index ([`build_links`]).
//! 4. **Call graph** from the same decoded index
//!    ([`super::call_graph::build_from_scip`]).
//! 5. **Inputs read**: each module file's hashes, every `review.md` under the
//!    crates in scope (read only), every existing `kovan.toml` (the previous
//!    ids and the `[test_run]` to carry; a malformed one's `[test_run]` is
//!    recovered from `git show HEAD:<path>`, else left pending), and the
//!    deleted folders' history from `git log --diff-filter=D`.
//! 6. **Everything is computed in memory first**; only then are files
//!    written (a failure before that writes nothing). Each `kovan.toml` is
//!    written when it differs ([`kovan_common::code_index::heal`]: missing,
//!    conflicted, malformed, stale or hand-edited), a foreign one (a
//!    literature `kovan.toml`) is left alone and reported, and orphans this
//!    tool wrote for folders that no longer have indexed code are removed.
//!    `review.md` is never written (the `--fresh` flow of
//!    [`crate::index_fresh`], GitHub #780, adds missing skeletons as its own
//!    later step).
//! 7. **The rust-analyzer version used is recorded** (maintainer,
//!    2026-10-07: "just record the versions of rust analyzer that were used,
//!    never overwrite the comments based on the new versions"): after the
//!    writes, [`record_rust_analyzer_used`] appends a
//!    `[[code_review.rust_analyzer_used]]` entry (version, date, `HEAD` or
//!    `"none"`) to the end of an existing, readable `kovan_root.toml` when
//!    the version differs from the last one recorded. It appends text, so
//!    every existing byte and comment stays; the pin is not touched; a root
//!    that does not parse gets nothing (the corrupt-root flow applies).
//!    The version is the one that wrote the SCIP index. Not under
//!    `--check`, and not under `--refresh`, which runs no rust-analyzer.
//!
//! **Any workspace or crate (#780).** The same run works on a single crate
//! (a member at the workspace root, folder `""`) and on a repository with
//! no commit yet (ids minted with [`NO_COMMIT`]). [`run_controlled`] is the
//! entry with progress, cancel and `nice` ([`RunControl`]) that the
//! desktop app's "Index fresh" uses; [`run`] is it with the CLI defaults.
//!
//! `--check` does all of that but writes nothing, and fails when any file
//! would change (for CI). `--draft-upstream` also prints proposed
//! `[upstream]` entries from provenance headers for folders whose
//! `review.md` has none (never written).
//!
//! # `--refresh` (no rust-analyzer)
//!
//! Every existing code-folder `kovan.toml` in scope is refreshed from the
//! current source with the `syn` hasher
//! ([`kovan_common::code_index::refresh`]): hashes and lines now, ids kept,
//! callees and reaching tests kept for unchanged functions, and new or
//! edited functions marked `index_out_of_date`. Folders with `.rs` files
//! and no `kovan.toml` are listed for a full run. The link files are not
//! touched (each records its files' hashes, so edited files show as out of
//! date).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use kovan_common::code_index::folders::{build, parent, BuildInput};
use kovan_common::code_index::heal::{classify, is_own_orphan, Existing};
use kovan_common::code_index::links::{self, FileOccs, Occ, Site, LINKS_FILE};
use kovan_common::code_index::refresh::{all_claims, refresh_folder};
use kovan_common::code_index::{deleted, upstream_draft};
use kovan_common::review::hash::hash_functions;
use kovan_common::review::index::{FolderIndex, TestRun};
use kovan_common::review::review_md::{parse_review_md, ReviewDocument};
use kovan_common::review::root::{
    append_rust_analyzer_used, CodeReviewSettings, ReviewRoot, RustAnalyzerUsed,
};

use crate::scip::{PositionEncoding, ScipIndex, Sym};

use super::index_control::RunControl;

/// `ctl.say(format!(...))`: stderr and the run's progress log.
macro_rules! say {
    ($ctl:expr, $($t:tt)*) => {
        $ctl.say(format!($($t)*))
    };
}

/// The file name of a folder index.
pub const KOVAN_TOML: &str = "kovan.toml";

/// Why `kovan-cli index` stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexCmdError {
    /// rust-analyzer is not on `PATH` and no `--scip` index was given.
    RustAnalyzerMissing,
    /// `--check` found files that would change.
    CheckFailed(usize),
    Other(String),
}

impl std::fmt::Display for IndexCmdError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RustAnalyzerMissing => write!(
                f,
                "rust-analyzer is not installed (not on PATH). It is needed only to \
                 REGENERATE the code index; the committed kovan.toml files and \
                 kovan_links.json work without it. Nothing was read or written. \
                 Either run `kovan-cli index --refresh` (no rust-analyzer: updates \
                 hashes of edited files and marks what it cannot recompute \"index out \
                 of date\"), pass `--scip <file>` with an index written elsewhere, or \
                 install it: `rustup component add rust-analyzer`."
            ),
            Self::CheckFailed(n) => write!(f, "--check: {n} file(s) would change; run `kovan-cli index`"),
            Self::Other(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for IndexCmdError {}

impl From<String> for IndexCmdError {
    fn from(e: String) -> Self {
        Self::Other(e)
    }
}

/// The options of one run.
#[derive(Debug, Clone, Default)]
pub struct IndexOptions {
    /// Crate names; every member when empty.
    pub crates: Vec<String>,
    pub scip: Option<PathBuf>,
    pub refresh: bool,
    pub check: bool,
    pub draft_upstream: bool,
    pub pin_rust_analyzer: bool,
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// The installed rust-analyzer's version (`1.98.0`), or `None` when it is
/// not on `PATH`.
pub fn installed_rust_analyzer() -> Option<String> {
    let out = Command::new("rust-analyzer").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    s.split_whitespace().nth(1).map(str::to_string)
}

fn rel_path(root: &Path, p: &Path) -> String {
    p.strip_prefix(root)
        .unwrap_or(p)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn read(root: &Path, rel: &str) -> Option<String> {
    std::fs::read_to_string(root.join(rel)).ok()
}

fn kovan_toml_of(dir: &str) -> String {
    if dir.is_empty() {
        KOVAN_TOML.to_string()
    } else {
        format!("{dir}/{KOVAN_TOML}")
    }
}

/// The member owning workspace-relative `path`: the deepest member folder
/// holding it.
fn owner<'m>(members: &'m [(String, String)], path: &str) -> Option<&'m (String, String)> {
    members
        .iter()
        .filter(|(_, d)| d.is_empty() || path.starts_with(&format!("{d}/")))
        .max_by_key(|(_, d)| d.len())
}

/// Files named `name` under member `dir` that belong to that member (not
/// to a member nested inside it), workspace-relative.
fn files_named(root: &Path, members: &[(String, String)], krate: &(String, String), name: &str) -> Vec<String> {
    let ext = name.rsplit('.').next().unwrap_or("");
    kovan_discovery::discover(&root.join(&krate.1), &[ext])
        .into_iter()
        .filter(|p| p.file_name().and_then(|f| f.to_str()) == Some(name))
        .map(|p| rel_path(root, &p))
        .filter(|p| owner(members, p).map(|o| &o.0) == Some(&krate.0))
        .collect()
}

/// The last committed version of a `kovan.toml` whose text on disk is not
/// readable (its ids and `[test_run]` are carried), if HEAD has a readable
/// one written for that folder.
fn recover_committed(root: &Path, rel: &str) -> Option<FolderIndex> {
    let text = git(root, &["show", &format!("HEAD:{rel}")]).ok()?;
    FolderIndex::parse(&text).ok().filter(|fi| fi.dir == parent(rel))
}

/// Every review.md under the crates in scope, parsed (read only).
fn read_reviews(root: &Path, members: &[(String, String)], scope: &[(String, String)]) -> BTreeMap<String, ReviewDocument> {
    let mut out = BTreeMap::new();
    for m in scope {
        for p in files_named(root, members, m, "review.md") {
            if let Some(t) = read(root, &p) {
                out.insert(p, parse_review_md(&t));
            }
        }
    }
    out
}

/// The existing kovan.toml files under the crates in scope: readable ones,
/// and the `[test_run]` of every one (recovered from git when unreadable).
struct Existing0 {
    texts: BTreeMap<String, String>,
    previous: BTreeMap<String, FolderIndex>,
    test_runs: BTreeMap<String, TestRun>,
    recovered: Vec<String>,
    pending: Vec<String>,
}

fn read_existing(root: &Path, members: &[(String, String)], scope: &[(String, String)]) -> Existing0 {
    let mut e = Existing0 {
        texts: BTreeMap::new(),
        previous: BTreeMap::new(),
        test_runs: BTreeMap::new(),
        recovered: Vec::new(),
        pending: Vec::new(),
    };
    for m in scope {
        for p in files_named(root, members, m, KOVAN_TOML) {
            let Some(text) = read(root, &p) else { continue };
            let dir = parent(&p).to_string();
            match FolderIndex::parse(&text) {
                // A copy written for another folder (a test fixture): not
                // this folder's cache; never used, never removed.
                Ok(fi) if fi.dir != dir => {}
                Ok(fi) => {
                    if let Some(r) = &fi.test_run {
                        e.test_runs.insert(dir.clone(), r.clone());
                    }
                    e.previous.insert(dir, fi);
                }
                Err(kovan_common::review::index::IndexError::NotACodeFolder { kind }) if !kind.is_empty() => {}
                // Unreadable: the last committed version supplies the ids to
                // keep and the [test_run] to carry; without one, pending.
                Err(_) => match recover_committed(root, &p) {
                    Some(fi) => {
                        match &fi.test_run {
                            Some(r) => {
                                e.test_runs.insert(dir.clone(), r.clone());
                                e.recovered.push(p.clone());
                            }
                            None => e.pending.push(p.clone()),
                        }
                        e.previous.insert(dir, fi);
                    }
                    None => e.pending.push(p.clone()),
                },
            }
            e.texts.insert(p, text);
        }
    }
    e
}

/// The link index of crate `krate` (folder `dir`) from a decoded SCIP index:
/// every non-definition occurrence in the crate's own documents that has a
/// definition in the workspace.
pub fn build_links(
    root: &Path,
    ix: &ScipIndex,
    members: &[(String, String)],
    krate: &(String, String),
) -> links::LinkIndex {
    let mut files = Vec::new();
    let mut encoding = "utf8";
    for d in &ix.documents {
        if owner(members, &d.path).map(|o| &o.0) != Some(&krate.0) {
            continue;
        }
        let Some(text) = read(root, &d.path) else { continue };
        encoding = match d.encoding {
            PositionEncoding::Utf16 => "utf16",
            PositionEncoding::Utf32 => "utf32",
            _ => "utf8",
        };
        let mut occs = Vec::new();
        for o in d.occurrences.iter().filter(|o| !o.is_definition()) {
            let def = match o.symbol {
                Sym::Local(n) => d.local_definition(n).map(|l| Site {
                    path: d.path.clone(),
                    line: l.line,
                    col: l.start,
                }),
                g @ Sym::Global(_) => ix
                    .nearest_definition(g, &d.path, o.line, None, &[], None)
                    .map(|s| Site {
                        path: ix.documents[s.doc as usize].path.clone(),
                        line: s.line,
                        col: s.start,
                    }),
            };
            if let Some(def) = def {
                occs.push(Occ {
                    line: o.line,
                    col: o.start,
                    len: if o.end_line == o.line { o.end.saturating_sub(o.start) } else { 0 },
                    def,
                });
            }
        }
        files.push(FileOccs {
            path: d.path.clone(),
            text,
            occs,
        });
    }
    let generator = format!("{} {}", ix.tool_name, ix.tool_version).trim().to_string();
    links::build(&krate.0, &krate.1, &generator, encoding, &files)
}

fn scope_of(members: &[(String, String)], crates: &[String]) -> Result<Vec<(String, String)>, String> {
    if crates.is_empty() {
        return Ok(members.to_vec());
    }
    crates
        .iter()
        .map(|c| {
            members
                .iter()
                .find(|(n, _)| n == c)
                .cloned()
                .ok_or_else(|| format!("`{c}` is not a workspace member"))
        })
        .collect()
}

/// One planned file change.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Change {
    Write { rel: String, text: String, why: &'static str },
    Remove { rel: String },
}

fn apply(root: &Path, changes: &[Change], ctl: &RunControl) -> Result<(), String> {
    ctl.phase(format!("writing {} file(s)", changes.len()), changes.len());
    for (i, c) in changes.iter().enumerate() {
        ctl.step(i);
        match c {
            Change::Write { rel, text, .. } => {
                std::fs::write(root.join(rel), text).map_err(|e| format!("{rel}: {e}"))?
            }
            Change::Remove { rel } => std::fs::remove_file(root.join(rel)).map_err(|e| format!("{rel}: {e}"))?,
        }
    }
    ctl.step(changes.len());
    Ok(())
}

fn summarise(changes: &[Change], unchanged: usize, ctl: &RunControl) {
    let mut by: BTreeMap<&str, usize> = BTreeMap::new();
    for c in changes {
        let (k, rel) = match c {
            Change::Write { why, rel, .. } => (*why, rel),
            Change::Remove { rel } => ("orphan removed", rel),
        };
        say!(ctl, "index:   {rel}: {k}");
        *by.entry(k).or_default() += 1;
    }
    let parts: Vec<String> = by.iter().map(|(k, n)| format!("{n} {k}")).collect();
    say!(ctl,
        "index: {unchanged} unchanged{}{}",
        if parts.is_empty() { "" } else { ", " },
        parts.join(", ")
    );
}

/// Pin check (module doc, step 1). Returns the installed version.
fn check_pin(root: &Path, opts: &IndexOptions, ctl: &RunControl) -> Result<Option<String>, IndexCmdError> {
    let installed = installed_rust_analyzer();
    if installed.is_none() && opts.scip.is_none() {
        return Err(IndexCmdError::RustAnalyzerMissing);
    }
    let root_file = root.join("kovan_root.toml");
    let text = std::fs::read_to_string(&root_file).ok();
    let parsed = text.as_deref().map(ReviewRoot::parse);
    let pinned = match &parsed {
        Some(Ok(r)) => r.code_review.as_ref().and_then(|c| c.rust_analyzer.clone()),
        _ => None,
    };
    match (&pinned, &installed) {
        (Some(p), Some(i)) if p != i => say!(ctl,
            "index: WARNING: kovan_root.toml pins rust-analyzer {p}, the installed one is {i}; \
             the index is regenerated with {i} and may differ from CI's"
        ),
        // Only for a root that exists and reads: before "Index fresh"
        // creates one there is nothing to pin into (#780).
        (None, Some(i)) if !opts.pin_rust_analyzer && matches!(parsed, Some(Ok(_))) => say!(ctl,
            "index: kovan_root.toml pins no rust-analyzer; this run's version ({i}) is recorded under \
             [[code_review.rust_analyzer_used]] (`--pin-rust-analyzer` would also pin it)"
        ),
        _ => {}
    }
    if opts.pin_rust_analyzer {
        let (Some(i), Some(text), Some(Ok(mut r))) = (installed.clone(), text, parsed) else {
            return Err(IndexCmdError::Other(
                "--pin-rust-analyzer needs rust-analyzer installed and a readable kovan_root.toml at the workspace root".into(),
            ));
        };
        r.code_review
            .get_or_insert_with(CodeReviewSettings::default)
            .rust_analyzer = Some(i.clone());
        let new = r.write_into(&text).map_err(|e| IndexCmdError::Other(e.to_string()))?;
        std::fs::write(&root_file, new).map_err(|e| IndexCmdError::Other(e.to_string()))?;
        say!(ctl, "index: pinned rust-analyzer {i} in kovan_root.toml");
    }
    Ok(installed)
}

/// What one run did (GitHub #780: the app shows it, and the fresh-index
/// flow creates `review.md` skeletons in `folders`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexSummary {
    /// Workspace-relative folders that have (or, under `--check`, would
    /// have) a code-folder `kovan.toml` after the run. Under `--refresh`,
    /// the folders refreshed.
    pub folders: Vec<String>,
    /// Files written or removed (0 under `--check`).
    pub written: usize,
    /// Files already up to date.
    pub unchanged: usize,
    /// The rust-analyzer version that wrote the SCIP index this run used
    /// (`None` under `--refresh`, or an index that names no version).
    pub rust_analyzer: Option<String>,
    /// Crates in scope that have source files and no document in the SCIP
    /// index (GitHub #820): rust-analyzer did not index them, so their link
    /// index is empty and their functions have no callees. Empty under
    /// `--refresh`.
    pub not_in_scip: Vec<String>,
}

/// What [`record_rust_analyzer_used`] did to `kovan_root.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordUsed {
    /// There is no `kovan_root.toml`: nothing recorded.
    NoRoot,
    /// It does not parse (or cannot be read): left untouched.
    Unreadable(String),
    /// The last recorded version is this one already: nothing appended.
    AlreadyLast,
    /// A new entry was appended.
    Appended,
}

impl RecordUsed {
    /// One line for the run's output.
    pub fn describe(&self, used: &RustAnalyzerUsed) -> String {
        match self {
            Self::NoRoot => format!(
                "no kovan_root.toml yet, so rust-analyzer {} is not recorded by this step (\"Index fresh\" records it in the root it creates)",
                used.version
            ),
            Self::Unreadable(e) => format!(
                "WARNING: kovan_root.toml was left untouched ({e}); rust-analyzer {} not recorded",
                used.version
            ),
            Self::AlreadyLast => format!(
                "rust-analyzer {} is already the last version recorded in kovan_root.toml",
                used.version
            ),
            Self::Appended => format!(
                "rust-analyzer {} recorded in kovan_root.toml [[code_review.rust_analyzer_used]] (appended; nothing else changed)",
                used.version
            ),
        }
    }
}

/// Append `used` to `<root>/kovan_root.toml`'s rust-analyzer history
/// (module doc, step 7) by writing only the new text at the end of the
/// file, so every existing byte stays. `Err` only when that write fails.
pub fn record_rust_analyzer_used(root: &Path, used: &RustAnalyzerUsed) -> Result<RecordUsed, String> {
    use std::io::Write;
    let path = root.join("kovan_root.toml");
    if !path.exists() {
        return Ok(RecordUsed::NoRoot);
    }
    let old = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) => return Ok(RecordUsed::Unreadable(e.to_string())),
    };
    let new = match append_rust_analyzer_used(&old, used) {
        Ok(None) => return Ok(RecordUsed::AlreadyLast),
        Ok(Some(t)) => t,
        Err(e) => return Ok(RecordUsed::Unreadable(e.to_string())),
    };
    let tail = new.get(old.len()..).ok_or("the appended text does not extend the old")?;
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(tail.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(RecordUsed::Appended)
}

/// The `commit` of a [`RustAnalyzerUsed`]: `HEAD`, or `"none"` when the
/// repository has no commit yet ([`NO_COMMIT`]).
pub fn used_commit(head: &str) -> String {
    if head == NO_COMMIT || head.is_empty() {
        "none".to_string()
    } else {
        head.to_string()
    }
}

/// `kovan-cli index` (module doc).
pub fn run(root: &Path, opts: &IndexOptions) -> Result<(), IndexCmdError> {
    run_controlled(root, opts, &RunControl::default()).map(|_| ())
}

/// [`run`] with progress, cancellation and priority (GitHub #780): the CLI
/// passes [`RunControl::default`], the desktop app's "Index fresh" a
/// background control from a worker thread. A cancel is honoured between
/// phases and during `rust-analyzer scip`, never once files are being
/// written.
pub fn run_controlled(root: &Path, opts: &IndexOptions, ctl: &RunControl) -> Result<IndexSummary, IndexCmdError> {
    let root = std::fs::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
    if opts.refresh {
        return run_refresh(&root, opts, ctl);
    }
    ctl.phase("checking rust-analyzer", 0);
    let installed = check_pin(&root, opts, ctl)?;
    ctl.phase("running cargo metadata", 0);
    let members = super::call_graph::member_dirs(&root)?;
    let scope = scope_of(&members, &opts.crates)?;
    ctl.check()?;
    // 2. SCIP.
    let scip_of = |root: &Path| -> Result<ScipIndex, IndexCmdError> {
        let path = super::call_graph::generate_scip_controlled(root, ctl)?;
        ctl.check()?;
        ctl.phase("reading the SCIP index", 0);
        Ok(super::call_graph::read_scip(&path)?)
    };
    let mut ix = match &opts.scip {
        Some(p) => {
            ctl.phase("reading the SCIP index", 0);
            super::call_graph::read_scip(p)?
        }
        None => scip_of(&root)?,
    };
    if let (Some(i), Some(_)) = (&installed, &opts.scip) {
        // The index says `1.98.0 (88d9e12 2026-08-18)`; compare the version.
        if ix.tool_version.split_whitespace().next() != Some(i.as_str()) {
            say!(ctl,
                "index: the index was written by rust-analyzer {}, the installed one is {i}: regenerating it",
                ix.tool_version
            );
            ix = scip_of(&root)?;
        }
    }
    ctl.check()?;
    // The version that wrote the index this run uses (step 7).
    let ra_used = ix
        .tool_version
        .split_whitespace()
        .next()
        .map(str::to_string);
    // A crate with indexed source files and no SCIP document was not
    // indexed by rust-analyzer: say so, and stop when that is every crate
    // (GitHub #820; before this an index matching nothing was written as
    // if the code had no calls).
    let mut with_code = 0usize;
    let mut not_in_scip: Vec<String> = Vec::new();
    for m in &scope {
        let files = code_files(&root, &members, m);
        if files.is_empty() {
            continue;
        }
        with_code += 1;
        if !files.iter().any(|f| ix.document(f).is_some()) {
            not_in_scip.push(m.0.clone());
        }
    }
    if with_code > 0 && not_in_scip.len() == with_code {
        return Err(IndexCmdError::Other(format!(
            "the SCIP index ({} document(s), written by rust-analyzer {}) holds no document of any of the {} \
             crate(s) to index, so links and callees would all be empty. Nothing was written. Was it made \
             for another workspace?",
            ix.documents.len(),
            ix.tool_version,
            with_code
        )));
    }
    for c in &not_in_scip {
        say!(ctl, "index: WARNING: {c} has source files and no document in the SCIP index: rust-analyzer did not index it, so its link index is empty and its functions have no callees or reaching tests");
    }
    // 3. Links.
    ctl.phase(format!("building the link index of {} crate(s)", scope.len()), scope.len());
    let mut changes = Vec::new();
    let mut unchanged = 0usize;
    let mut sizes = Vec::new();
    for (i, m) in scope.iter().enumerate() {
        ctl.step(i);
        let li = build_links(&root, &ix, &members, m);
        let text = li.to_json();
        let rel = if m.1.is_empty() { LINKS_FILE.to_string() } else { format!("{}/{LINKS_FILE}", m.1) };
        sizes.push((m.0.clone(), text.len(), li.files.len(), li.defs.len() / 3));
        if read(&root, &rel).as_deref() == Some(text.as_str()) {
            unchanged += 1;
        } else {
            changes.push(Change::Write { rel, text, why: "link index" });
        }
    }
    ctl.check()?;
    // 4. Call graph.
    ctl.phase("building the call graph", 0);
    let names: Vec<String> = scope.iter().map(|m| m.0.clone()).collect();
    let doc = super::call_graph::build_from_scip(&root, Some(&names), ix)?;
    ctl.check()?;
    // 5. Inputs.
    ctl.phase("hashing functions and reading review.md / kovan.toml", 0);
    let mut hashed = BTreeMap::new();
    let mut sources: BTreeMap<String, String> = BTreeMap::new();
    for (_, m, _) in doc.functions() {
        if sources.contains_key(&m.file) {
            continue;
        }
        let Some(text) = read(&root, &m.file) else { continue };
        match hash_functions(&text) {
            Ok(h) => {
                hashed.insert(m.file.clone(), h);
            }
            Err(e) => say!(ctl, "index: {}: {e}; its functions are left out", m.file),
        }
        sources.insert(m.file.clone(), text);
    }
    let mut quantities = kovan_common::code_index::physical::QuantityNames::default();
    quantities.learn(sources.values().map(String::as_str));
    let reviews = read_reviews(&root, &members, &scope);
    let existing = read_existing(&root, &members, &scope);
    for p in &existing.recovered {
        say!(ctl, "index: {p} is unreadable; its [test_run] was recovered from HEAD");
    }
    for p in &existing.pending {
        say!(ctl, "index: {p} is unreadable and HEAD has no readable copy; its test evidence is now PENDING");
    }
    let commit = head_commit(&root, ctl);
    let used_at = used_commit(&commit);
    let mut deleted_by_crate = BTreeMap::new();
    let still: BTreeSet<String> = reviews.keys().map(|p| parent(p).to_string()).collect();
    for m in &scope {
        let log = git(&root, &["log", "--diff-filter=D", "--name-only", "--format=%x00%H", "--", pathspec(&m.1)])
            .unwrap_or_default();
        let mut rows = Vec::new();
        for (dir, c) in deleted::deleted_review_mds(&log, &m.1, &still) {
            if let Ok(text) = git(&root, &["show", &format!("{c}^:{dir}/review.md")]) {
                rows.push(deleted::deleted_folder(&dir, &c, &text));
            }
        }
        if !rows.is_empty() {
            deleted_by_crate.insert(m.0.clone(), rows);
        }
    }
    ctl.check()?;
    ctl.phase("building the folder indexes", 0);
    let built = build(&BuildInput {
        doc: doc.clone(),
        hashed,
        reviews: reviews.clone(),
        previous: existing.previous.clone(),
        test_runs: existing.test_runs.clone(),
        deleted: deleted_by_crate,
        commit,
        quantities,
    });
    // 6. Plan the writes.
    for (dir, fi) in &built.folders {
        let rel = kovan_toml_of(dir);
        let text = fi.to_toml().map_err(|e| e.to_string())?;
        match classify(existing.texts.get(&rel).map(String::as_str), &text) {
            Existing::Unchanged => unchanged += 1,
            Existing::Foreign { kind } => {
                say!(ctl, "index: {rel} is a `{kind}` kovan.toml, not a code index: left alone, folder not indexed")
            }
            e => changes.push(Change::Write { rel, text, why: e.label() }),
        }
    }
    for (rel, text) in &existing.texts {
        let dir = parent(rel);
        if !built.folders.contains_key(dir) && is_own_orphan(text, dir) {
            changes.push(Change::Remove { rel: rel.clone() });
        }
    }
    let r = &built.report;
    if !r.unhashed.is_empty() {
        say!(ctl, "index: {} function(s) have no hash yet (nested in another fn or made by a macro) and are left out; their calls count for the enclosing function", r.unhashed.len());
    }
    for m in &r.moved {
        say!(ctl, "index: {} moved {} -> {} (matched by hash; acknowledge the move in kovan)", m.id, m.from.as_deref().unwrap_or("?"), m.to);
    }
    for u in &r.unmatched {
        say!(ctl, "index: {} ({}) names no function: {}", u.id, u.path.as_deref().unwrap_or("callee"), u.why);
    }
    if !r.outside_without_id.is_empty() {
        say!(ctl, "index: {} callee(s) outside the crates in scope have no known id and are left out of `callees`; index the whole workspace to include them", r.outside_without_id.len());
    }
    for (name, bytes, files, defs) in &sizes {
        say!(ctl, "index: {name}/{LINKS_FILE}: {bytes} bytes, {files} files, {defs} definitions");
    }
    if opts.draft_upstream {
        print_drafts(&doc, &reviews);
    }
    summarise(&changes, unchanged, ctl);
    let mut summary = IndexSummary {
        folders: built.folders.keys().cloned().collect(),
        written: 0,
        unchanged,
        rust_analyzer: ra_used.clone(),
        not_in_scip,
    };
    if opts.check {
        return if changes.is_empty() { Ok(summary) } else { Err(IndexCmdError::CheckFailed(changes.len())) };
    }
    ctl.check()?;
    apply(&root, &changes, ctl)?;
    summary.written = changes.len();
    if let Some(version) = ra_used {
        let date = crate::digitiser::dataset::utc_now_iso8601();
        let used = RustAnalyzerUsed {
            version,
            date: date.get(..10).unwrap_or(&date).to_string(),
            commit: used_at,
        };
        let rec = record_rust_analyzer_used(&root, &used).map_err(IndexCmdError::Other)?;
        say!(ctl, "index: {}", rec.describe(&used));
    }
    Ok(summary)
}

/// The commit `HEAD` names, to mint new function ids with. A repository
/// with no commit yet (a fresh `cargo new`), or a folder outside git, has
/// none: the all-zero id is used and the run says so (GitHub #780).
fn head_commit(root: &Path, ctl: &RunControl) -> String {
    match git(root, &["rev-parse", "--verify", "HEAD"]) {
        Ok(c) => c.trim().to_string(),
        Err(e) => {
            say!(ctl, "index: no HEAD commit ({e}); new function ids are minted with the all-zero commit");
            NO_COMMIT.to_string()
        }
    }
}

/// The commit new ids are minted with when there is no `HEAD`.
pub const NO_COMMIT: &str = "0000000000000000000000000000000000000000";

/// A git pathspec for workspace-relative folder `dir`: `.` for the root
/// (an empty pathspec is an error in git), else the folder.
fn pathspec(dir: &str) -> &str {
    if dir.is_empty() {
        "."
    } else {
        dir
    }
}

/// The workspace-relative `.rs` files of member `m` (name, folder) in the
/// folders a full run indexes: the lib and integration tests (`src/`,
/// `tests/`), not binaries (`src/bin/`, `src/main.rs`), which are not in
/// the call graph. Files of a member nested inside `m` are left out.
pub fn code_files(root: &Path, members: &[(String, String)], m: &(String, String)) -> Vec<String> {
    let pre = prefix(&m.1);
    let (src, tests, bin, main) =
        (format!("{pre}src/"), format!("{pre}tests/"), format!("{pre}src/bin/"), format!("{pre}src/main.rs"));
    kovan_discovery::discover(&root.join(&m.1), &["rs"])
        .into_iter()
        .map(|p| rel_path(root, &p))
        .filter(|p| owner(members, p).map(|o| &o.0) == Some(&m.0))
        .filter(|p| p.starts_with(&src) || p.starts_with(&tests))
        .filter(|p| !p.starts_with(&bin) && *p != main)
        .collect()
}

/// `dir/` for a member folder, empty for a crate at the workspace root.
fn prefix(dir: &str) -> String {
    if dir.is_empty() {
        String::new()
    } else {
        format!("{dir}/")
    }
}

fn print_drafts(doc: &kovan_common::call_graph::CallGraphDoc, reviews: &BTreeMap<String, ReviewDocument>) {
    let mut by_dir: BTreeMap<String, (BTreeMap<String, kovan_common::call_graph::upstream::Upstream>, Vec<String>)> =
        BTreeMap::new();
    for c in &doc.crates {
        for t in c.targets.iter().chain(c.tests.iter()) {
            for m in &t.modules {
                let dir = parent(&m.file).to_string();
                let name = m.file.rsplit('/').next().unwrap_or(&m.file).to_string();
                let e = by_dir.entry(dir).or_default();
                e.1.push(name.clone());
                if let Some(u) = &m.upstream {
                    e.0.insert(name, u.clone());
                }
            }
        }
    }
    let date = crate::digitiser::dataset::utc_now_iso8601();
    let date = date.get(..10).unwrap_or(&date).to_string();
    for (dir, (headers, mut all)) in by_dir {
        let md = format!("{dir}/review.md");
        if reviews.get(&md).is_some_and(|r| r.upstream().is_some()) {
            continue;
        }
        all.sort();
        all.dedup();
        if let Some(d) = upstream_draft::draft(&dir, &headers, &all, &date) {
            println!("{}", d.to_markdown());
        }
    }
}

/// `--refresh` (module doc).
fn run_refresh(root: &Path, opts: &IndexOptions, ctl: &RunControl) -> Result<IndexSummary, IndexCmdError> {
    let members = super::call_graph::member_dirs(root)?;
    let scope = scope_of(&members, &opts.crates)?;
    let reviews = read_reviews(root, &members, &scope);
    let claims = all_claims(&reviews);
    ctl.phase("refreshing kovan.toml files", 0);
    let commit = head_commit(root, ctl);
    let mut changes = Vec::new();
    let mut unchanged = 0usize;
    let mut flagged = 0usize;
    let mut indexed_dirs = BTreeSet::new();
    for m in &scope {
        for rel in files_named(root, &members, m, KOVAN_TOML) {
            let Some(text) = read(root, &rel) else { continue };
            let dir = parent(&rel).to_string();
            let previous = match FolderIndex::parse(&text) {
                Ok(fi) => Some(fi),
                Err(kovan_common::review::index::IndexError::NotACodeFolder { kind }) if !kind.is_empty() => continue,
                Err(_) => git(root, &["show", &format!("HEAD:{rel}")])
                    .ok()
                    .and_then(|t| FolderIndex::parse(&t).ok()),
            };
            if previous.as_ref().is_some_and(|p| p.dir != dir) {
                continue; // a copy written for another folder (a fixture)
            }
            indexed_dirs.insert(dir.clone());
            let mut files = BTreeMap::new();
            if let Ok(rd) = std::fs::read_dir(root.join(&dir)) {
                for e in rd.flatten() {
                    let name = e.file_name().to_string_lossy().to_string();
                    if name.ends_with(".rs") && e.path().is_file() {
                        if let Ok(t) = std::fs::read_to_string(e.path()) {
                            files.insert(name, t);
                        }
                    }
                }
            }
            let krate = previous.as_ref().map_or(m.0.clone(), |p| p.krate.clone());
            let rd = reviews.get(&format!("{dir}/review.md"));
            let (fi, report) = refresh_folder(&krate, &dir, previous.as_ref(), &files, rd, &claims, &commit);
            flagged += report.out_of_date.len();
            for u in &report.unparsed {
                say!(ctl, "index --refresh: {u} does not parse; its entries are kept and marked out of date");
            }
            let new = fi.to_toml().map_err(|e| e.to_string())?;
            match classify(Some(&text), &new) {
                Existing::Unchanged => unchanged += 1,
                e => changes.push(Change::Write { rel, text: new, why: e.label() }),
            }
        }
        let rs_dirs: BTreeSet<String> =
            code_files(root, &members, m).iter().map(|p| parent(p).to_string()).collect();
        for d in rs_dirs.difference(&indexed_dirs) {
            say!(ctl, "index --refresh: {d} has .rs files and no kovan.toml: run `kovan-cli index`");
        }
    }
    if flagged > 0 {
        say!(ctl, "index --refresh: {flagged} function(s) marked \"index out of date: run kovan-cli index\" (new or edited; callees and reaching tests not recomputed)");
    }
    summarise(&changes, unchanged, ctl);
    let mut summary =
        IndexSummary { folders: indexed_dirs.into_iter().collect(), written: 0, unchanged, rust_analyzer: None, not_in_scip: Vec::new() };
    if opts.check {
        return if changes.is_empty() { Ok(summary) } else { Err(IndexCmdError::CheckFailed(changes.len())) };
    }
    apply(root, &changes, ctl)?;
    summary.written = changes.len();
    Ok(summary)
}
