//! **Per-function stamp states from `review.md`** (GitHub #765, #770): the
//! native side of the staleness engine. Builds [`GitFacts`] with `git`
//! subprocess calls (nothing in the workspace built them before
//! 2026-10-10), runs [`kovan_common::review::engine::evaluate`] with the
//! registry of `kovan_root.toml` and signatures enforced, and maps the
//! result onto the [`StampState`] rows kovan-web's `Facts::new` reads, keyed
//! by call-graph id.
//!
//! # What git gives, field by field
//!
//! | `GitFacts` field | how |
//! |---|---|
//! | `head` | `git rev-parse HEAD` |
//! | `cargo_lock` | `sha256:` of `Cargo.lock` in the working tree |
//! | `lock_fingerprints` | per-crate dependency fingerprints ([`kovan_common::review::lock_scope`], #1048) of the working tree's `Cargo.lock` and of each lock a review or `[test_run]` recorded, read with `git show <commit>:Cargo.lock` and kept only when it hashes to the recorded value |
//! | `stamps[..].added_in` | the newest commit whose diff changes the number of occurrences of the stamp's signature value in its `review.md` (`git log -1 -S<value>`), taken only when `HEAD`'s `review.md` holds it (else not committed); `after_certified` is "not the certified commit and a descendant of it" (`git merge-base --is-ancestor`); `certified_in_history` is "the certified commit is an ancestor of `HEAD`" (the same command against `HEAD`; false after a squash or rebase, and in a shallow clone, #838); the trailer check is [`agent_trailer`] on its message |
//! | `stamps[..].hash_at_commit` | the function at the review's recorded `path`, re-hashed from `git show <commit>:<file>` with the same `syn` hasher |
//! | `stamps[..].carried_forward` | only when the certified commit is not in `HEAD`'s history: the stamp's chain through the verified `kovan_carry_forward/` records ([`super::carry_forward`], #838, #1018), when it ends in a commit in history. Then `certified_in_history` is true, `hash_at_commit`, the permalink and (when the original commit is not in this clone) `tests_at_review` are read at that effective key, and `reviewed_commit_time` stays the original commit's (`None` when absent: the #783 check is not checkable) |
//! | `stamps[..].tests_at_review` | `reached_by` of the function in the folder's `kovan.toml` **as committed at the review commit**, and for each test the messages of the commits up to it that touched the test's file (file-level: coarser than the function, so an agent commit anywhere in the file counts, the conservative side) |
//! | `hashes_at_test_run` | every function of a folder with `[test_run]`, re-hashed at the run's commit |
//! | `hashes_at_targeted_run` | the same for `[targeted_run]` (GitHub #1052) |
//! | `code_authors` | for reviewed functions only: `git blame` author e-mails of the function's current lines, plus the reviewer ids they map to (the same e-mail, or `github:<user>` for a `users.noreply.github.com` address; matched case-insensitively against the registry) |
//! | `previous_root` | `HEAD`'s `kovan_root.toml` when the working copy differs from it, else the version before the last commit that changed it |
//! | `deleted_in`, `publish_records`, `tag_commits` | left empty: deleted functions show without a deleting commit, there is no #773 record yet, and tags show as unchecked (offline) |
//!
//! **Batched (GitHub #1023).** The commands above say what each answer
//! *is*; since 2026-10-22 they are not run one process per question but put
//! to a [`GitBatch`](super::git_batch) (one `cat-file --batch`, one
//! `rev-list --parents HEAD`, one `log` per `review.md`, one `blame` per
//! file, answers kept per `HEAD`), which gives the same answers or runs the
//! command itself. The tests' `log` per test file is unchanged (kept per
//! process for a full commit id, #856). Measured on this workspace (806
//! folders, Linux): a refresh after a stamp started 316 `git` processes
//! before and 2 after; its git phase went from 4.1 s to 0.45 s.
//!
//! ~~[`ConceptAreas`] is passed **empty** (resolving a function's concept
//! areas from its reviews' `implements` relations is not built): no
//! function reaches rung 5 here~~ **CORRECTED 2026-10-10** (#770, #810):
//! [`evaluate_workspace`] and [`evaluate_loaded`] (and so
//! [`stamp_states`], the need-you queue, `kovan-cli review ivv` and
//! `call-graph --split-dir`) resolve [`ConceptAreas`] from the reviews'
//! `implements` concept links ([`super::concepts::concept_areas`]; what an
//! "area" is taken to mean is in that module's doc). A function whose
//! reviews link no concept still has no known area and cannot reach rung 5,
//! never a silent upgrade.
//!
//! A stamp without a signature has no pickaxe string and is never found
//! in git (unverified, "no git facts"); with signatures enforced it would be
//! unverified anyway.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use kovan_common::call_graph::split::{StampState as WebStamp, StampVerdict};
use kovan_common::review::carry_forward::CarryForward;
use kovan_common::review::engine::{
    evaluate, CarriedForward, ConceptAreas, Evaluation, FolderReviews, GitFacts, ReviewKey,
    same_deps, SignaturePolicy, StampCommit, StampFacts, StampState, TestsAtReview,
    UnverifiedReason,
};
use kovan_common::review::hash::{hash_all, HashedFn};
use kovan_common::review::index::FolderIndex;
use kovan_common::review::ivv_view::summarise;
use kovan_common::review::review_md::ReviewEntry;
use kovan_common::review::root::ReviewRoot;
use kovan_common::review::state::StateKind;
use kovan_common::review::types::agent_trailer;

use super::{
    call_graph_id, cargo_lock_hash, join, load_workspace, Workspace, KOVAN_TOML, REVIEW_MD,
    ROOT_FILE,
};
use super::git;
use super::git_batch::GitBatch;
/// (revision, file) -> commit messages up to the revision touching it.
pub(crate) type MessageCache = BTreeMap<(String, String), Vec<String>>;

/// `qual` split into the hasher's qualified name and the call graph's
/// 1-based `#k` duplicate index.
fn split_qual(qual: &str) -> (String, Option<usize>) {
    let squash = |s: &str| s.split_whitespace().collect::<String>();
    match qual.rsplit_once('#') {
        Some((base, k)) if !k.is_empty() && k.bytes().all(|b| b.is_ascii_digit()) => {
            (squash(base), k.parse().ok())
        }
        _ => (squash(qual), None),
    }
}

/// The function, type or constant (#843) `qual` of `file` at `rev`, hashed as the index hashes it
/// (the same `#k` rule as `kovan_common::call_graph::function_ids`).
pub(crate) fn fn_at(git: &mut GitBatch, rev: &str, file: &str, qual: &str) -> Option<HashedFn> {
    let key = (rev.to_string(), file.to_string());
    if !git.files.contains_key(&key) {
        let parsed = git.show(rev, file).and_then(|t| hash_all(&t).ok());
        git.files.insert(key.clone(), parsed);
    }
    let fns = git.files.get(&key)?.as_ref()?;
    pick(fns.iter().map(|h| h.entry.qualname()), qual).map(|i| fns[i].clone())
}

/// Which of a file's items (their qualified names, in file order) `qual`
/// names: the one with that name, or the `#k`-th of several (the call
/// graph's rule). Shared by [`fn_at`] and the kept test-run hashes
/// ([`super::hash_store`]), so the two cannot choose differently.
pub(crate) fn pick(names: impl Iterator<Item = String>, qual: &str) -> Option<usize> {
    let (base, k) = split_qual(qual);
    let same: Vec<usize> = names
        .enumerate()
        .filter(|(_, n)| n.split_whitespace().collect::<String>() == base)
        .map(|(i, _)| i)
        .collect();
    match (k, same.as_slice()) {
        (None, [one]) => Some(*one),
        (Some(k), many) if k >= 1 && many.len() > 1 => many.get(k - 1).copied(),
        _ => None,
    }
}

/// `git log` for one file up to `rev`: the commit messages, newest first.
fn log_messages(root: &Path, rev: &str, file: &str) -> Vec<String> {
    git::git(root, &["log", "--format=%B%x1e", rev, "--", file])
        .map(|out| {
            out.split('\x1e')
                .map(str::trim)
                .filter(|m| !m.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// A full commit id: what it names never changes, so what git says about
/// it can be kept ~~for the life of the process~~ across runs
/// ([`super::message_store`]).
fn is_full_sha(rev: &str) -> bool {
    rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit())
}


/// Fill `cache` with the messages of the commits up to `rev` that touched
/// each of `files`. One `git log` per file, as before (the history of each
/// file is simplified on its own, which one log over all of them would not
/// reproduce); what is new (#856) is that the logs run side by side, two
/// cores left free, and that the answer for a full commit id is kept ~~for
/// the life of the process~~ on disk across runs (2026-11-03,
/// [`super::message_store`]). Measured 2026-10-13 on this workspace: the
/// reaching tests of one reviewed function took 1.08 s one after another.
fn fetch_messages(cache: &mut MessageCache, root: &Path, rev: &str, files: &BTreeSet<String>) {
    let keep = is_full_sha(rev);
    let mut missing: Vec<&String> = Vec::new();
    for f in files {
        if cache.contains_key(&(rev.to_string(), f.clone())) {
            continue;
        }
        let kept = keep.then(|| super::message_store::kept(root, rev, f)).flatten();
        match kept {
            Some(m) => {
                cache.insert((rev.to_string(), f.clone()), m);
            }
            None => missing.push(f),
        }
    }
    if missing.is_empty() {
        return;
    }
    let threads = std::thread::available_parallelism()
        .map_or(1, |n| n.get().saturating_sub(2))
        .max(1);
    let per = missing.len().div_ceil(threads).max(1);
    let got: Vec<(String, Vec<String>)> = std::thread::scope(|scope| {
        let handles: Vec<_> = missing
            .chunks(per)
            .map(|part| scope.spawn(move || part.iter().map(|f| ((*f).clone(), log_messages(root, rev, f))).collect::<Vec<_>>()))
            .collect();
        handles.into_iter().flat_map(|h| h.join().expect("git log does not panic")).collect()
    });
    for (f, m) in got {
        if keep {
            super::message_store::keep(root, rev, &f, m.clone());
        }
        cache.insert((rev.to_string(), f), m);
    }
}

/// Messages of the commits up to `rev` that touched `file`.
fn messages(cache: &mut MessageCache, root: &Path, rev: &str, file: &str) -> Vec<String> {
    fetch_messages(cache, root, rev, &BTreeSet::from([file.to_string()]));
    cache.get(&(rev.to_string(), file.to_string())).cloned().unwrap_or_default()
}

/// The tests that reached function `id` at `commit`, from the folder's
/// `kovan.toml` as committed then (module doc); `None` when it was not
/// committed or does not list the function.
pub(crate) fn tests_at(
    git: &mut GitBatch,
    commit: &str,
    dir: &str,
    id: &str,
    cache: &mut MessageCache,
) -> Option<TestsAtReview> {
    let reached_by = reached_by_at(git, commit, dir)?.get(id)?.clone();
    let root = git.root();
    let test_files: BTreeSet<String> =
        reached_by.iter().filter_map(|t| t.split_once(".rs::").map(|(file, _)| format!("{file}.rs"))).collect();
    fetch_messages(cache, root, commit, &test_files);
    let mut commit_messages = BTreeMap::new();
    for t in &reached_by {
        if let Some((file, _)) = t.split_once(".rs::") {
            commit_messages.insert(
                t.clone(),
                messages(cache, root, commit, &format!("{file}.rs")),
            );
        }
    }
    Some(TestsAtReview {
        reached_by,
        commit_messages,
    })
}

/// Each function's `reached_by` in `dir`'s `kovan.toml` as committed at
/// `commit`; `None` when it was not committed or does not parse.
///
/// Parsed once per process for a full commit id (#1198: re-parsing it for
/// every stamp was 1.55 s of each refresh, 2026-11-04), since what a fixed
/// commit holds never changes; asked afresh for any other revision.
/// [`prefetch_reached_by`] parses a refresh's files side by side first.
type ReachedBy = Arc<HashMap<String, Vec<String>>>;
type ReachedByKey = (PathBuf, String, String);
fn kept_reached_by() -> &'static Mutex<HashMap<ReachedByKey, Option<ReachedBy>>> {
    static KEPT: OnceLock<Mutex<HashMap<ReachedByKey, Option<ReachedBy>>>> = OnceLock::new();
    KEPT.get_or_init(Default::default)
}

fn parse_reached_by(text: &str) -> Option<ReachedBy> {
    let idx = FolderIndex::parse(text).ok()?;
    let mut m = HashMap::new();
    for (_, f) in idx.functions() {
        // The first entry with an id, as a search through the file finds.
        m.entry(f.id.clone()).or_insert_with(|| f.reached_by.clone());
    }
    Some(Arc::new(m))
}

fn reached_by_at(git: &mut GitBatch, commit: &str, dir: &str) -> Option<ReachedBy> {
    let key = (git.root().to_path_buf(), commit.to_string(), dir.to_string());
    let keep = is_full_sha(commit);
    if keep {
        if let Some(v) = kept_reached_by().lock().ok().and_then(|k| k.get(&key).cloned()) {
            return v;
        }
    }
    let parsed = git.show(commit, &join(dir, KOVAN_TOML)).and_then(|t| parse_reached_by(&t));
    if keep {
        if let Ok(mut k) = kept_reached_by().lock() {
            k.insert(key, parsed.clone());
        }
    }
    parsed
}

/// Parse the committed `kovan.toml`s of `pairs` (commit, folder) not yet
/// kept, side by side, and keep them ([`reached_by_at`]): the same parse,
/// so the same answers (#1198).
fn prefetch_reached_by(git: &mut GitBatch, pairs: &BTreeSet<(String, String)>) {
    let root = git.root().to_path_buf();
    let mut todo = Vec::new();
    for (commit, dir) in pairs.iter().filter(|(c, _)| is_full_sha(c)) {
        let key = (root.clone(), commit.clone(), dir.clone());
        if kept_reached_by().lock().is_ok_and(|k| k.contains_key(&key)) {
            continue;
        }
        todo.push((key, git.show(commit, &join(dir, KOVAN_TOML))));
    }
    let parsed = super::git_batch::side_by_side(todo, |(_, text)| text.as_deref().and_then(parse_reached_by));
    if let Ok(mut k) = kept_reached_by().lock() {
        for ((key, _), v) in parsed {
            k.insert(key, v);
        }
    }
}

/// The folder of a `<file>.rs::<qual>` path.
fn dir_of(p: &str) -> String {
    p.split_once(".rs::").map_or(String::new(), |(f, _)| f.rsplit_once('/').map_or(String::new(), |(d, _)| d.to_string()))
}

/// The stamp's pickaxe string: its signature value.
fn needle(r: &ReviewEntry) -> Option<&str> {
    r.review.signature.as_ref().map(|s| s.value.as_str())
}

/// The commit that added `r` to `review_md` (module doc), if committed:
/// what `git log -1 -S<value> -- <review_md>` finds (GitHub #1023: one
/// `log` per `review.md` instead of one pickaxe per stamp, kept per `HEAD`).
///
/// The pickaxe, as git runs it: walking the file's log (history simplified
/// for the path) newest first, the first commit whose diff changes the
/// number of occurrences of the value. A merge has no diff in `git log`
/// (no `-m`), so it never matches; a root commit is diffed against nothing.
/// Checked against git's own pickaxe on 116 values over the four signed
/// `review.md` files of `human-audit-and-coding-theodore` (0 differ) and
/// by `git_batch::tests`.
fn added_in(git: &mut GitBatch, review_md: &str, r: &ReviewEntry) -> Option<StampCommit> {
    let needle = needle(r)?;
    let certified = &r.review.commit;
    let key = (review_md.to_string(), needle.to_string(), certified.clone());
    if let Some(kept) = git.kept_added(&key) {
        return kept;
    }
    let found = stamp_commit(git, review_md, needle, certified);
    git.keep_added(key, found.clone());
    found
}

/// [`added_in`], computed.
pub(crate) fn stamp_commit(git: &mut GitBatch, review_md: &str, needle: &str, certified: &str) -> Option<StampCommit> {
    if needle.is_empty() || !git.show("HEAD", review_md)?.contains(needle) {
        return None;
    }
    let log = git.file_log(review_md)?;
    let mut count = |rev: &str| git.show(rev, review_md).map_or(0, |t| t.matches(needle).count());
    let hit = log.iter().find(|c| match c.parents.as_slice() {
        [] => count(&c.id) != 0,
        [p] => count(&c.id) != count(p),
        _ => false,
    })?;
    let commit = hit.id.clone();
    let descendant = git.is_ancestor(certified, &commit);
    let same = git.resolve(certified).is_some_and(|c| c == commit);
    let in_history = git.is_ancestor(certified, "HEAD");
    Some(StampCommit {
        commit,
        after_certified: descendant && !same,
        certified_in_history: in_history,
        agent_trailer: agent_trailer(&hit.body).0,
        committer_time: hit.time,
    })
}

/// The stamp's carry-forward (#838, #1018), when its certified commit is
/// not in `HEAD`'s history and verified records lead to a commit that is.
/// Then `added` is marked as in history (the effective key is), and whether
/// the stamp arrived in a hop's own commit is recorded.
fn carried_forward(
    git: &mut GitBatch,
    r: &ReviewEntry,
    records: &[CarryForward],
    added: Option<&mut StampCommit>,
) -> Option<CarriedForward> {
    let added = added?;
    if added.certified_in_history || records.is_empty() {
        return None;
    }
    let mut g = super::carry_forward::GitChain::new(git, "HEAD");
    let chain = super::carry_forward::chain_of(r, records, &mut g);
    use kovan_common::review::carry_forward::ChainGit;
    if chain.hops.is_empty() || !g.in_history(&chain.key) {
        return None;
    }
    added.certified_in_history = true;
    Some(CarriedForward {
        key: chain.key.clone(),
        path: chain.path.clone().unwrap_or_default(),
        stamp_arrived_in_hop: chain.hops.iter().any(|h| h.to == added.commit),
        hops: chain.hops.iter().map(|h| (h.from.clone(), h.to.clone())).collect(),
    })
}

/// Reviewer ids a git author e-mail stands for (module doc).
fn author_ids(email: &str, root: &ReviewRoot) -> BTreeSet<String> {
    let mut out = BTreeSet::from([email.to_string()]);
    let lower = email.to_ascii_lowercase();
    let mut candidates = vec![lower.clone()];
    if let Some(local) = lower.strip_suffix("@users.noreply.github.com") {
        let user = local.rsplit('+').next().unwrap_or(local);
        candidates.push(format!("github:{user}"));
    }
    for c in &candidates {
        match root
            .reviewers
            .iter()
            .find(|r| r.id.to_ascii_lowercase() == *c)
        {
            Some(r) => out.insert(r.id.clone()),
            None => out.insert(c.clone()),
        };
    }
    out
}

/// Where the review's function was **at the commit it certifies**: the
/// first acknowledged move's `from` (GitHub #771: an acknowledged move
/// rewrites `path` to the new location, but the certified commit still
/// holds the function at the old one), else `path`. The hash at the
/// certified commit, the tests at review time and the permalink are all
/// read there.
pub(crate) fn certified_path(r: &ReviewEntry) -> String {
    r.review
        .moved
        .first()
        .map(|m| m.from.clone())
        .or_else(|| r.path())
        .unwrap_or_default()
}

/// The previous committed `kovan_root.toml` (module doc).
fn previous_root(git: &mut GitBatch) -> Option<ReviewRoot> {
    let head = git.show("HEAD", ROOT_FILE)?;
    let now = std::fs::read_to_string(git.root().join(ROOT_FILE)).unwrap_or_default();
    let text = if now != head {
        head
    } else {
        let last = git.last_change(ROOT_FILE)?;
        git.show(&format!("{last}^"), ROOT_FILE)?
    };
    ReviewRoot::parse(&text).ok()
}

/// Lock hash -> crate -> dependency fingerprint (#1048) for the current
/// `Cargo.lock` and every other lock a review or `[test_run]` recorded,
/// read at that entry's commit. A commit's lock is used only when it
/// hashes to the recorded value (a review made over an uncommitted lock
/// does not), and each recorded hash is resolved once.
fn lock_fingerprints(
    git: &mut GitBatch,
    ws: &Workspace,
    current: &str,
) -> BTreeMap<String, BTreeMap<String, String>> {
    use kovan_common::review::lock_scope::crate_fingerprints;
    let mut out = BTreeMap::new();
    let now = std::fs::read(git.root().join("Cargo.lock")).ok();
    if let Some(fp) = now.and_then(|b| crate_fingerprints(&String::from_utf8_lossy(&b))) {
        out.insert(current.to_string(), fp);
    }
    // (recorded hash, commit) pairs, deduplicated.
    let mut wanted: BTreeSet<(String, String)> = BTreeSet::new();
    for idx in &ws.indexes {
        if let Some(run) = &idx.test_run {
            wanted.insert((run.cargo_lock.clone(), run.commit.clone()));
        }
    }
    for m in ws.reviews.values() {
        for r in m.doc.reviews() {
            if let Some(l) = &r.review.cargo_lock {
                wanted.insert((l.clone(), r.review.commit.clone()));
            }
        }
    }
    for (lock, commit) in wanted {
        if out.contains_key(&lock) {
            continue;
        }
        let Some(text) = git.show(&commit, "Cargo.lock") else { continue };
        if super::lock_bytes_hash(text.as_bytes()) != lock {
            continue;
        }
        if let Some(fp) = crate_fingerprints(&text) {
            out.insert(lock, fp);
        }
    }
    out
}

/// The crates a test run is due for (GitHub #1049, Code Review's "Test
/// changed crates"): every crate with a code folder whose `[test_run]` is
/// missing, not the full suite, or at a lock where the crate's dependency
/// closure differs from now ([`same_deps`]). Reads git for the old locks;
/// call it off the UI thread.
pub fn crates_needing_test(root: &Path) -> Result<Vec<String>, String> {
    let ws = load_workspace(root)?;
    let mut git = GitBatch::new(root);
    let current = cargo_lock_hash(root).unwrap_or_default();
    let facts = GitFacts {
        lock_fingerprints: lock_fingerprints(&mut git, &ws, &current),
        cargo_lock: current,
        ..GitFacts::default()
    };
    Ok(needing_test(&ws.indexes, &facts))
}

/// [`crates_needing_test`]'s decision over the indexes, sorted.
pub(crate) fn needing_test(indexes: &[FolderIndex], git: &GitFacts) -> Vec<String> {
    use kovan_common::review::index::Suite;
    let mut out: BTreeSet<String> = BTreeSet::new();
    for idx in indexes {
        let due = match &idx.test_run {
            None => true,
            Some(r) => r.suite != Suite::Full || !same_deps(&idx.krate, &r.cargo_lock, git),
        };
        if due {
            out.insert(idx.krate.clone());
        }
    }
    out.into_iter().collect()
}

/// The workspace's [`GitFacts`] (module doc).
pub fn git_facts(root: &Path, ws: &Workspace) -> GitFacts {
    git_facts_in(&mut GitBatch::new(root), ws)
}

/// [`git_facts`] over a [`GitBatch`] (GitHub #1023), which the permalinks
/// then reuse.
pub(crate) fn git_facts_in(git: &mut GitBatch, ws: &Workspace) -> GitFacts {
    let root = git.root().to_path_buf();
    let root = root.as_path();
    let mut msgs = MessageCache::new();
    let mut g = GitFacts {
        head: ws.head.clone(),
        cargo_lock: cargo_lock_hash(root).unwrap_or_default(),
        previous_root: previous_root(git),
        ..GitFacts::default()
    };
    g.lock_fingerprints = lock_fingerprints(git, ws, &g.cargo_lock);
    // Each function's hash at its folder's last test run, and at the
    // targeted run (GitHub #1052) for the functions all of whose reaching
    // tests it targeted (`targeted_verdict`): no extra work for the rest.
    // Kept by blob and parsed in parallel (#1198, [`super::hash_store`]).
    let covered = |run: &kovan_common::review::index::TestRun, f: &kovan_common::review::index::FunctionIndex| {
        !f.reached_by.is_empty() && f.reached_by.iter().all(|t| run.targets.contains(t))
    };
    let mut wanted = BTreeSet::new();
    for idx in &ws.indexes {
        for (file, f) in idx.functions() {
            for run in [idx.test_run.as_ref(), idx.targeted_run.as_ref().filter(|r| covered(r, f))].into_iter().flatten() {
                wanted.insert((run.commit.clone(), idx.file_path(file)));
            }
        }
    }
    let items = super::hash_store::items_at(git, &wanted);
    let hash_at = |run: &kovan_common::review::index::TestRun, path: String, qual: &str| {
        items.get(&(run.commit.clone(), path)).and_then(|i| super::hash_store::hash_of(i, qual))
    };
    for idx in &ws.indexes {
        for (file, f) in idx.functions() {
            if let Some(h) = idx.test_run.as_ref().and_then(|run| hash_at(run, idx.file_path(file), &f.qual)) {
                g.hashes_at_test_run.insert(f.id.clone(), h);
            }
            if let Some(h) = idx.targeted_run.as_ref().filter(|r| covered(r, f)).and_then(|run| hash_at(run, idx.file_path(file), &f.qual)) {
                g.hashes_at_targeted_run.insert(f.id.clone(), h);
            }
        }
    }
    // Carry-forward records (#838, #1018), consulted only for a stamp whose
    // certified commit is not in HEAD's history.
    let records: Vec<CarryForward> = super::carry_forward::load_records(root)
        .0
        .into_iter()
        .map(|(_, r)| r)
        .collect();
    let current: BTreeMap<&str, (&FolderIndex, &str)> = ws
        .indexes
        .iter()
        .flat_map(|i| {
            i.functions()
                .map(move |(file, f)| (f.id.as_str(), (i, file)))
        })
        .collect();
    // Reviewed function ranges by file, blamed together after the loop.
    let mut blames: BTreeMap<String, Vec<(String, [u32; 2])>> = BTreeMap::new();
    // The logs `added_in` reads, run side by side first (#1198).
    let signed: BTreeSet<String> = ws
        .reviews
        .iter()
        .filter(|(_, m)| m.doc.reviews().any(|r| needle(r).is_some_and(|n| !n.is_empty())))
        .map(|(dir, _)| join(dir, REVIEW_MD))
        .collect();
    git.prefetch_file_logs(&signed);
    // The committed `kovan.toml`s `tests_at` reads, parsed side by side.
    let at_review: BTreeSet<(String, String)> = ws
        .reviews
        .values()
        .flat_map(|m| m.doc.reviews().map(|r| (r.review.commit.clone(), dir_of(&certified_path(r)))).collect::<Vec<_>>())
        .collect();
    prefetch_reached_by(git, &at_review);
    for (dir, m) in &ws.reviews {
        let review_md = join(dir, REVIEW_MD);
        for r in m.doc.reviews() {
            let mut added = added_in(git, &review_md, r);
            let carried = carried_forward(git, r, &records, added.as_mut());
            // Read at the effective key when carried forward, else at the
            // certified commit (#838, #1018).
            let (key, path) = match &carried {
                Some(c) => (c.key.clone(), c.path.clone()),
                None => (r.review.commit.clone(), certified_path(r)),
            };
            let at = path.split_once(".rs::").and_then(|(file, qual)| {
                fn_at(git, &key, &format!("{file}.rs"), qual)
            });
            // The tests at the original commit when this clone holds it,
            // else at the effective key.
            let tests_at_review =
                tests_at(git, &r.review.commit, &dir_of(&certified_path(r)), &r.function_id(), &mut msgs)
                    .or_else(|| {
                        carried.as_ref().and_then(|c| tests_at(git, &c.key, &dir_of(&c.path), &r.function_id(), &mut msgs))
                    });
            let facts = StampFacts {
                hash_at_commit: at.map(|h| h.hashes.hash),
                added_in: added,
                // Always the original commit's time (#783, #1018): `None`
                // when it is not in this clone, never `to`'s.
                reviewed_commit_time: git.committer_time(&r.review.commit),
                tests_at_review,
                carried_forward: carried,
            };
            g.stamps.insert(
                ReviewKey {
                    function: r.function_id(),
                    by: r.review.by.clone(),
                },
                facts,
            );
            if let Some((idx, file)) = current.get(r.function_id().as_str()) {
                let f = idx
                    .functions()
                    .find(|(_, f)| f.id == r.function_id())
                    .map(|(_, f)| f);
                if let Some(f) = f {
                    blames.entry(idx.file_path(file)).or_default().push((f.id.clone(), f.lines));
                }
            }
        }
    }
    // `git blame` author e-mails of each reviewed function's current lines:
    // one `blame` per file (#1023).
    let ranges_of = |wanted: &Vec<(String, [u32; 2])>| -> BTreeSet<[u32; 2]> { wanted.iter().map(|(_, l)| *l).collect() };
    git.prefetch_blames(&blames.iter().map(|(f, w)| (f.clone(), ranges_of(w))).collect());
    for (file, wanted) in &blames {
        let ranges = ranges_of(wanted);
        let mails = git.blame(file, &ranges);
        for (id, lines) in wanted {
            let ids: BTreeSet<String> = mails
                .get(lines)
                .into_iter()
                .flatten()
                .flat_map(|e| author_ids(e, &ws.review_root))
                .collect();
            g.code_authors.entry(id.clone()).or_default().extend(ids);
        }
    }
    // Keep the new test commit messages for the next run. A failed write
    // costs the next run time, never a different answer (module doc of
    // `message_store`), so it is not an error of the refresh.
    let _ = super::message_store::save(root);
    let _ = super::hash_store::save(root);
    g
}

/// The engine's verdict on the workspace, with the call-graph id of every
/// current function.
#[derive(Debug, Clone)]
pub struct WorkspaceEvaluation {
    pub evaluation: Evaluation,
    /// Function id -> call-graph id (`file.rs::Type::f#2`).
    pub call_graph_ids: BTreeMap<String, String>,
    /// Review key -> permalink of the reviewed code at the review commit.
    pub permalinks: BTreeMap<ReviewKey, String>,
    /// `kovan_root.toml`'s review sections as judged (for the IV&V views,
    /// #810: the audit records as written).
    pub review_root: ReviewRoot,
}

/// Load the workspace, build the git facts and run the engine with
/// signatures enforced (module doc), with the concept areas the reviews'
/// concept links give ([`super::concepts::concept_areas`]).
pub fn evaluate_workspace(root: &Path) -> Result<WorkspaceEvaluation, String> {
    let ws = load_workspace(root)?;
    Ok(evaluate_loaded(root, &ws))
}

/// [`evaluate_workspace`] with the function -> concept-area map given
/// instead of resolved (GitHub #810; tests). ~~Nothing in kovan resolves
/// concept areas yet, so every caller but a test passes none~~
/// **CORRECTED 2026-10-10**: [`evaluate_workspace`] resolves them; with an
/// empty map rung 5 is unreachable
/// ([`kovan_common::review::ivv::Rung5Miss::NoConceptArea`]).
pub fn evaluate_workspace_with(
    root: &Path,
    concepts: &ConceptAreas,
) -> Result<WorkspaceEvaluation, String> {
    let ws = load_workspace(root)?;
    Ok(evaluate_loaded_with(root, &ws, concepts))
}

/// [`evaluate_workspace`] over a workspace already loaded (the need-you
/// queue reads the same [`Workspace`] for crates and line ranges), with the
/// concept areas resolved from its reviews (~~with no concept areas~~
/// **CORRECTED 2026-10-10**, #810).
pub fn evaluate_loaded(root: &Path, ws: &Workspace) -> WorkspaceEvaluation {
    evaluate_loaded_with(root, ws, &super::concepts::concept_areas(ws))
}

/// [`evaluate_loaded`] with the function -> concept-area map given (#810).
pub fn evaluate_loaded_with(root: &Path, ws: &Workspace, concepts: &ConceptAreas) -> WorkspaceEvaluation {
    evaluate_loaded_timed(root, ws, concepts).0
}

/// The phases of one state refresh, timed (GitHub #1018: "for windows,
/// reviews do take long"; measure before optimising). Logged by kovan
/// after each background refresh ([`RefreshTimings::line`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RefreshTimings {
    pub load: super::LoadPhases,
    /// Resolving the reviews' concept areas.
    pub concepts: std::time::Duration,
    /// [`git_facts`]: every `git` call.
    pub git: std::time::Duration,
    /// The staleness engine.
    pub engine: std::time::Duration,
    /// The permalinks (re-hashing reviewed code at its commit).
    pub permalinks: std::time::Duration,
    /// Mapping to the web's rows.
    pub states: std::time::Duration,
    pub total: std::time::Duration,
    /// `git` processes started during the refresh (GitHub #1023;
    /// [`super::git::spawns`], process-wide, so another thread's `git`
    /// calls in the same window count too).
    pub git_spawns: usize,
}

impl RefreshTimings {
    /// The total less every phase named: what no phase covers.
    pub fn other(&self) -> std::time::Duration {
        let named = self.load.cache_check
            + self.load.folder_refresh
            + self.load.cache_copy
            + self.concepts
            + self.git
            + self.engine
            + self.permalinks
            + self.states;
        self.total.saturating_sub(named)
    }

    /// One line for the log, seconds to the millisecond. The same line on
    /// every system (kovan's log and the "Refresh states" tooltip), so a
    /// Windows refresh reads against a Linux one phase by phase; "git
    /// processes" is how many `git` processes the refresh started (GitHub
    /// #1023), the cost that is much higher on Windows.
    pub fn line(&self) -> String {
        let s = |d: std::time::Duration| format!("{:.3} s", d.as_secs_f64());
        format!(
            "review-state refresh {}: cache check {}, folder refresh {} ({} folders), cache copy {}, concepts {}, git {} ({} git processes), engine {}, permalinks {}, states {}, other {}",
            s(self.total),
            s(self.load.cache_check),
            s(self.load.folder_refresh),
            self.load.folders_refreshed,
            s(self.load.cache_copy),
            s(self.concepts),
            s(self.git),
            self.git_spawns,
            s(self.engine),
            s(self.permalinks),
            s(self.states),
            s(self.other())
        )
    }
}

/// [`evaluate_loaded_with`], with its phases timed (`git`, `engine`,
/// `permalinks` of [`RefreshTimings`]).
pub fn evaluate_loaded_timed(root: &Path, ws: &Workspace, concepts: &ConceptAreas) -> (WorkspaceEvaluation, RefreshTimings) {
    let mut t = RefreshTimings::default();
    let clock = std::time::Instant::now();
    let mut batch = GitBatch::new(root);
    let git = git_facts_in(&mut batch, ws);
    t.git = clock.elapsed();
    let krate_of = |dir: &str| {
        ws.indexes
            .iter()
            .find(|i| i.dir == dir)
            .map(|i| i.krate.clone())
            .unwrap_or_default()
    };
    let folders: Vec<FolderReviews> = ws
        .reviews
        .iter()
        .map(|(dir, m)| FolderReviews {
            krate: krate_of(dir),
            dir: dir.clone(),
            doc: m.doc.clone(),
        })
        .collect();
    let clock = std::time::Instant::now();
    let evaluation = evaluate(
        &folders,
        &ws.indexes,
        &ws.review_root,
        &git,
        concepts,
        SignaturePolicy::Enforce,
    );
    t.engine = clock.elapsed();
    let clock = std::time::Instant::now();
    let call_graph_ids = ws
        .indexes
        .iter()
        .flat_map(|i| {
            i.functions()
                .map(move |(file, f)| (f.id.clone(), call_graph_id(i, file, f)))
        })
        .collect();
    // The same batch: every file read for the facts is read again here.
    let mut permalinks = BTreeMap::new();
    for m in ws.reviews.values() {
        for r in m.doc.reviews() {
            let key = ReviewKey {
                function: r.function_id(),
                by: r.review.by.clone(),
            };
            // At the effective key when carried forward (#838, #1018).
            let (commit, path) = match git.stamps.get(&key).and_then(|f| f.carried_forward.as_ref()) {
                Some(c) => (c.key.clone(), c.path.clone()),
                None => (r.review.commit.clone(), certified_path(r)),
            };
            if let Some((file, qual)) = path.split_once(".rs::") {
                let file = format!("{file}.rs");
                let lines =
                    fn_at(&mut batch, &commit, &file, qual).map(|h| h.entry.lines);
                let anchor = lines
                    .map(|[a, b]| format!("#L{a}-L{b}"))
                    .unwrap_or_default();
                permalinks.insert(
                    ReviewKey {
                        function: r.function_id(),
                        by: r.review.by.clone(),
                    },
                    format!(
                        "{}/blob/{}/{file}{anchor}",
                        crate::commands::code_walk::render::DEFAULT_REPO_URL.trim_end_matches('/'),
                        commit
                    ),
                );
            }
        }
    }
    t.permalinks = clock.elapsed();
    (
        WorkspaceEvaluation {
            evaluation,
            call_graph_ids,
            permalinks,
            review_root: ws.review_root.clone(),
        },
        t,
    )
}

/// A one-line reason for a state that is not valid.
fn describe(s: &StampState) -> String {
    match s {
        StampState::Valid => String::new(),
        StampState::DirectlyStale(w) => format!("code changed since the review: {w:?}"),
        StampState::InheritedStale { cause, blocked, .. } => {
            format!(
                "a callee changed: {cause:?}{}",
                if *blocked {
                    " (re-confirm blocked until tests pass)"
                } else {
                    ""
                }
            )
        }
        StampState::NeedsFixOpen { note, .. } => format!("needs fix: {note}"),
        StampState::Fixed { note, .. } => format!("fixed, awaiting re-review: {note}"),
        StampState::Unverified(UnverifiedReason::CertifiedCommitNotInHistory) => {
            "unverified: the commit this review certifies is not in this branch's history (a \
             squash or rebase replaced it, or the clone is shallow); after a squash, run \
             `kovan-cli review carry-forward` (or re-anchor it in desktop kovan)"
                .into()
        }
        StampState::Unverified(why) => format!("unverified: {why:?}"),
        StampState::Unreadable { message } => format!("unreadable: {message}"),
        StampState::Invalid(why) => format!("invalid: {why:?}"),
        StampState::Moved { to, .. } => format!("moved to {}::{}", to.file, to.qual),
        StampState::PendingWorkspaceTest { .. } => {
            "pending a test run: this crate's dependencies changed in Cargo.lock".into()
        }
        other => other.kind().label().to_string(),
    }
}

/// Every reviewed (or flagged) function's state, in the form kovan-web
/// reads (`Facts::new`), keyed by call-graph id. Functions with nothing
/// recorded are left out (the web shows them as unreviewed). Plain
/// function, no UI: call it off the UI thread.
pub fn stamp_states(root: &Path) -> Result<Vec<WebStamp>, String> {
    stamp_states_timed(root).map(|(s, _)| s)
}

/// [`stamp_states`], with every phase timed ([`RefreshTimings`]).
pub fn stamp_states_timed(root: &Path) -> Result<(Vec<WebStamp>, RefreshTimings), String> {
    let start = std::time::Instant::now();
    let spawned = super::git::spawns();
    let (ws, load) = super::load_workspace_timed(root)?;
    let clock = std::time::Instant::now();
    let concepts = super::concepts::concept_areas(&ws);
    let concepts_took = clock.elapsed();
    let (we, mut t) = evaluate_loaded_timed(root, &ws, &concepts);
    t.load = load;
    t.concepts = concepts_took;
    let clock = std::time::Instant::now();
    let mut out = Vec::new();
    for (id, fr) in &we.evaluation.functions {
        if fr.reviews.is_empty() && fr.state.kind() == StateKind::New {
            continue;
        }
        let Some(cg) = we.call_graph_ids.get(id) else {
            continue;
        };
        let chosen = fr
            .reviews
            .iter()
            .find(|r| r.state == fr.state)
            .or_else(|| fr.reviews.first());
        let kind = fr.state.kind();
        let note = match &fr.state {
            StampState::NeedsFixOpen { note, .. } | StampState::Fixed { note, .. } => note.clone(),
            _ => String::new(),
        };
        let key = chosen.map(|r| ReviewKey {
            function: id.clone(),
            by: r.by.clone(),
        });
        out.push(WebStamp {
            function: cg.clone(),
            verdict: if kind == StateKind::Valid {
                StampVerdict::Valid
            } else {
                StampVerdict::Stale
            },
            reason: describe(&fr.state),
            rung: fr.rung.or_else(|| chosen.and_then(|r| r.rung)).unwrap_or(0),
            reviewer: chosen.map(|r| r.by.clone()).unwrap_or_default(),
            date: chosen.and_then(|r| r.date.clone()).unwrap_or_default(),
            note,
            permalink: key
                .and_then(|k| we.permalinks.get(&k).cloned())
                .unwrap_or_default(),
            state: Some(kind),
            ivv: summarise(fr, &we.review_root, &we.evaluation.ivv_warnings)
                .map(std::sync::Arc::new),
        });
    }
    out.sort_by(|a, b| a.function.cmp(&b.function));
    t.states = clock.elapsed();
    t.total = start.elapsed();
    t.git_spawns = super::git::spawns().saturating_sub(spawned);
    Ok((out, t))
}

#[cfg(test)]
mod message_store_tests {
    use super::*;

    /// Methodology (2026-11-03): a throwaway repository where `t.rs` is
    /// changed by a commit with the agent trailer and a human one. The messages
    /// up to HEAD for `t.rs` are asked once (from git, then saved); the
    /// process's copy is dropped, as a new kovan run starts, and they are
    /// asked again. Pass: the second answer equals the first and equals
    /// `git log` itself, and it starts no `git` process.
    #[test]
    fn kept_messages_survive_a_restart_and_match_git() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        let run = |a: &[&str]| git::git(p, a).unwrap();
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "t@example.org"]);
        run(&["config", "user.name", "T"]);
        run(&["config", "commit.gpgsign", "false"]);
        for (text, msg) in [("1", "add t\n\nCo-Authored-By: Claude <noreply@anthropic.com>"), ("2", "edit t by hand")] {
            std::fs::write(p.join("t.rs"), text).unwrap();
            run(&["add", "-A"]);
            run(&["commit", "-q", "-m", msg]);
        }
        let head = run(&["rev-parse", "HEAD"]).trim().to_string();
        let first = messages(&mut MessageCache::new(), p, &head, "t.rs");
        assert_eq!(first, log_messages(p, &head, "t.rs"));
        assert_eq!(first.len(), 2);
        assert!(first.iter().any(|m| agent_trailer(m).0), "{first:?}");
        super::super::message_store::save(p).unwrap();
        super::super::message_store::forget(p);
        // Counted on this thread: other tests' git calls run beside it.
        let spawned = git::spawns_on_this_thread();
        let again = messages(&mut MessageCache::new(), p, &head, "t.rs");
        assert_eq!(git::spawns_on_this_thread(), spawned, "read from the kept file, not git");
        assert_eq!(again, first);
    }
}

#[cfg(test)]
mod needing_test_tests {
    use super::*;
    use kovan_common::review::index::{Suite, TestRun};

    /// Methodology (#1049): four crates — no run, a quick run, a full run
    /// at a lock where the crate's dependencies are the same as now, and
    /// one where they differ. Pass: all but the unchanged crate are due.
    #[test]
    fn crates_due_a_test_run() {
        let h = |c: char| format!("sha256:{}", c.to_string().repeat(64));
        let with = |krate: &str, run: Option<(Suite, char)>| {
            let mut i = FolderIndex::new(krate, &format!("crates/{krate}/src"));
            i.test_run = run.map(|(suite, l)| TestRun {
                commit: "c".repeat(40),
                cargo_lock: h(l),
                suite,
                passed: Vec::new(),
                failed: Vec::new(),
                edited: Vec::new(),
                targets: Vec::new(),
            });
            i
        };
        let idx = [
            with("none", None),
            with("quick", Some((Suite::Quick, '2'))),
            with("same", Some((Suite::Full, '1'))),
            with("moved", Some((Suite::Full, '1'))),
        ];
        let fp = |same: &str, moved: &str| {
            BTreeMap::from([("same".to_string(), same.to_string()), ("moved".to_string(), moved.to_string())])
        };
        let git = GitFacts {
            cargo_lock: h('2'),
            lock_fingerprints: BTreeMap::from([(h('1'), fp("s", "m0")), (h('2'), fp("s", "m1"))]),
            ..GitFacts::default()
        };
        assert_eq!(needing_test(&idx, &git), ["moved", "none", "quick"]);
    }
}
