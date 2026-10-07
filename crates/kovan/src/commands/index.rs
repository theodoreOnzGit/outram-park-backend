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
//!    an existing `kovan_root.toml`. With rust-analyzer absent and no
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
//!    `review.md` is never written.
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
use kovan_common::review::root::{CodeReviewSettings, ReviewRoot};

use crate::scip::{PositionEncoding, ScipIndex, Sym};

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

fn apply(root: &Path, changes: &[Change]) -> Result<(), String> {
    for c in changes {
        match c {
            Change::Write { rel, text, .. } => {
                std::fs::write(root.join(rel), text).map_err(|e| format!("{rel}: {e}"))?
            }
            Change::Remove { rel } => std::fs::remove_file(root.join(rel)).map_err(|e| format!("{rel}: {e}"))?,
        }
    }
    Ok(())
}

fn summarise(changes: &[Change], unchanged: usize) {
    let mut by: BTreeMap<&str, usize> = BTreeMap::new();
    for c in changes {
        let (k, rel) = match c {
            Change::Write { why, rel, .. } => (*why, rel),
            Change::Remove { rel } => ("orphan removed", rel),
        };
        eprintln!("index:   {rel}: {k}");
        *by.entry(k).or_default() += 1;
    }
    let parts: Vec<String> = by.iter().map(|(k, n)| format!("{n} {k}")).collect();
    eprintln!(
        "index: {unchanged} unchanged{}{}",
        if parts.is_empty() { "" } else { ", " },
        parts.join(", ")
    );
}

/// Pin check (module doc, step 1). Returns the installed version.
fn check_pin(root: &Path, opts: &IndexOptions) -> Result<Option<String>, IndexCmdError> {
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
        (Some(p), Some(i)) if p != i => eprintln!(
            "index: WARNING: kovan_root.toml pins rust-analyzer {p}, the installed one is {i}; \
             the index is regenerated with {i} and may differ from CI's"
        ),
        (None, Some(i)) if !opts.pin_rust_analyzer => eprintln!(
            "index: rust-analyzer is not pinned (kovan_root.toml [code_review] rust_analyzer); \
             `--pin-rust-analyzer` pins {i}"
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
        eprintln!("index: pinned rust-analyzer {i} in kovan_root.toml");
    }
    Ok(installed)
}

/// `kovan-cli index` (module doc).
pub fn run(root: &Path, opts: &IndexOptions) -> Result<(), IndexCmdError> {
    let root = std::fs::canonicalize(root).map_err(|e| format!("{}: {e}", root.display()))?;
    if opts.refresh {
        return run_refresh(&root, opts);
    }
    let installed = check_pin(&root, opts)?;
    let members = super::call_graph::member_dirs(&root)?;
    let scope = scope_of(&members, &opts.crates)?;
    // 2. SCIP.
    let mut ix = match &opts.scip {
        Some(p) => super::call_graph::read_scip(p)?,
        None => super::call_graph::read_scip(&super::call_graph::generate_scip(&root)?)?,
    };
    if let (Some(i), Some(_)) = (&installed, &opts.scip) {
        // The index says `1.98.0 (88d9e12 2026-08-18)`; compare the version.
        if ix.tool_version.split_whitespace().next() != Some(i.as_str()) {
            eprintln!(
                "index: the index was written by rust-analyzer {}, the installed one is {i}: regenerating it",
                ix.tool_version
            );
            ix = super::call_graph::read_scip(&super::call_graph::generate_scip(&root)?)?;
        }
    }
    // 3. Links.
    let mut changes = Vec::new();
    let mut unchanged = 0usize;
    let mut sizes = Vec::new();
    for m in &scope {
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
    // 4. Call graph.
    let names: Vec<String> = scope.iter().map(|m| m.0.clone()).collect();
    let doc = super::call_graph::build_from_scip(&root, Some(&names), ix)?;
    // 5. Inputs.
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
            Err(e) => eprintln!("index: {}: {e}; its functions are left out", m.file),
        }
        sources.insert(m.file.clone(), text);
    }
    let mut quantities = kovan_common::code_index::physical::QuantityNames::default();
    quantities.learn(sources.values().map(String::as_str));
    let reviews = read_reviews(&root, &members, &scope);
    let existing = read_existing(&root, &members, &scope);
    for p in &existing.recovered {
        eprintln!("index: {p} is unreadable; its [test_run] was recovered from HEAD");
    }
    for p in &existing.pending {
        eprintln!("index: {p} is unreadable and HEAD has no readable copy; its test evidence is now PENDING");
    }
    let commit = git(&root, &["rev-parse", "HEAD"])?.trim().to_string();
    let mut deleted_by_crate = BTreeMap::new();
    let still: BTreeSet<String> = reviews.keys().map(|p| parent(p).to_string()).collect();
    for m in &scope {
        let log = git(&root, &["log", "--diff-filter=D", "--name-only", "--format=%x00%H", "--", &m.1]).unwrap_or_default();
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
                eprintln!("index: {rel} is a `{kind}` kovan.toml, not a code index: left alone, folder not indexed")
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
        eprintln!("index: {} function(s) have no hash yet (nested in another fn or made by a macro) and are left out; their calls count for the enclosing function", r.unhashed.len());
    }
    for m in &r.moved {
        eprintln!("index: {} moved {} -> {} (matched by hash; acknowledge the move in kovan)", m.id, m.from.as_deref().unwrap_or("?"), m.to);
    }
    for u in &r.unmatched {
        eprintln!("index: {} ({}) names no function: {}", u.id, u.path.as_deref().unwrap_or("callee"), u.why);
    }
    if !r.outside_without_id.is_empty() {
        eprintln!("index: {} callee(s) outside the crates in scope have no known id and are left out of `callees`; index the whole workspace to include them", r.outside_without_id.len());
    }
    for (name, bytes, files, defs) in &sizes {
        eprintln!("index: {name}/{LINKS_FILE}: {bytes} bytes, {files} files, {defs} definitions");
    }
    if opts.draft_upstream {
        print_drafts(&doc, &reviews);
    }
    summarise(&changes, unchanged);
    if opts.check {
        return if changes.is_empty() { Ok(()) } else { Err(IndexCmdError::CheckFailed(changes.len())) };
    }
    apply(&root, &changes)?;
    Ok(())
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
fn run_refresh(root: &Path, opts: &IndexOptions) -> Result<(), IndexCmdError> {
    let members = super::call_graph::member_dirs(root)?;
    let scope = scope_of(&members, &opts.crates)?;
    let reviews = read_reviews(root, &members, &scope);
    let claims = all_claims(&reviews);
    let commit = git(root, &["rev-parse", "HEAD"])?.trim().to_string();
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
                eprintln!("index --refresh: {u} does not parse; its entries are kept and marked out of date");
            }
            let new = fi.to_toml().map_err(|e| e.to_string())?;
            match classify(Some(&text), &new) {
                Existing::Unchanged => unchanged += 1,
                e => changes.push(Change::Write { rel, text: new, why: e.label() }),
            }
        }
        let rs_dirs: BTreeSet<String> = kovan_discovery::discover(&root.join(&m.1), &["rs"])
            .into_iter()
            .map(|p| rel_path(root, &p))
            .filter(|p| owner(&members, p).map(|o| &o.0) == Some(&m.0))
            // The folders a full run indexes (lib and integration tests;
            // binaries are not in the call graph).
            .filter(|p| p.starts_with(&format!("{}/src/", m.1)) || p.starts_with(&format!("{}/tests/", m.1)))
            .filter(|p| !p.contains("/src/bin/") && !p.ends_with("/src/main.rs"))
            .map(|p| parent(&p).to_string())
            .collect();
        for d in rs_dirs.difference(&indexed_dirs) {
            eprintln!("index --refresh: {d} has .rs files and no kovan.toml: run `kovan-cli index`");
        }
    }
    if flagged > 0 {
        eprintln!("index --refresh: {flagged} function(s) marked \"index out of date: run kovan-cli index\" (new or edited; callees and reaching tests not recomputed)");
    }
    summarise(&changes, unchanged);
    if opts.check {
        return if changes.is_empty() { Ok(()) } else { Err(IndexCmdError::CheckFailed(changes.len())) };
    }
    apply(root, &changes)?;
    Ok(())
}
