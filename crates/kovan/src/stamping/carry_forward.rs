//! **Writing and checking carry-forward records** (GitHub #838; design on
//! #1018): the git side of [`kovan_common::review::carry_forward`], whose
//! module doc holds the record format, why the signed stamp is never
//! edited, and why records live in `kovan_carry_forward/<to commit>.toml`.
//!
//! # `kovan-cli review carry-forward [--from <ref>] [--to <ref>]`
//!
//! One rule, run whenever a branch gains commits (by CI after a push to
//! `develop` or `main`, by an agent, or by a person after a merge):
//!
//! 1. `to` (default `HEAD`) is the commit that brought the code onto the
//!    branch: the squash commit, the merge commit, or the rebased or
//!    cherry-picked tip. It must be `HEAD` or an ancestor of it.
//! 2. For every **signed** stamp, resolve its key through the records
//!    already written ([`chain_of`]). A key that is `to` or an ancestor of
//!    it needs nothing (a regular merge keeps the original commit in
//!    history, so it writes no record: a record would verify nothing new
//!    and only add a file to merge).
//! 3. Otherwise look the function up at `to` (where the last hop, the
//!    review's `path`, or the certified location says it is) and hash it.
//!    **Equal to the signed `hash`:** write `{stamp, target, path, hash,
//!    from = key, to}`. **Not equal or not there:** write nothing and list
//!    it as refused; the stamp goes stale, as on any edit.
//!
//! `--from <ref>` names where the reviewed code came from (the review
//! branch, or `refs/pull/<n>/head`, which GitHub keeps after a squash and a
//! branch deletion). It is fetched from `origin` when this clone lacks it,
//! and the run is limited to stamps whose key is that commit or one of its
//! ancestors. Fetching it also brings the **original** commits back, so the
//! #783 signing-time check can run against them; without them it reads
//! "not checkable (squashed)" (never judged against `to`'s time).
//!
//! **Unsigned and keyless.** The records are derived facts anyone can
//! re-check (`--check`, [`check_records`]); `kovan-cli` writes no signed
//! entry and holds no key (maintainer, 2026-10-22: "kovan-cli should only
//! make unsigned reviews"). The command writes the file; committing it is
//! the caller's.

use std::collections::BTreeMap;
use std::path::Path;

use kovan_common::review::carry_forward::{
    check_record, file_name, resolve_chain, CarryForward, CarryForwardFile, Chain, ChainGit,
    HopCheck, CARRY_FORWARD_DIR,
};
use kovan_common::review::review_md::ReviewEntry;

use super::git;
use super::states::{certified_path, fn_at, FileCache};
use super::{load_workspace, ROOT_FILE};

/// Every record under `kovan_carry_forward/`, with the files that do not
/// parse (`(file, error)`): those are reported, never silently used.
pub fn load_records(root: &Path) -> (Vec<(String, CarryForward)>, Vec<(String, String)>) {
    let mut records = Vec::new();
    let mut bad = Vec::new();
    let Ok(rd) = std::fs::read_dir(root.join(CARRY_FORWARD_DIR)) else {
        return (records, bad);
    };
    let mut paths: Vec<_> = rd
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("toml"))
        .collect();
    paths.sort();
    for p in paths {
        let name = format!(
            "{CARRY_FORWARD_DIR}/{}",
            p.file_name().and_then(|n| n.to_str()).unwrap_or_default()
        );
        match std::fs::read_to_string(&p)
            .map_err(|e| e.to_string())
            .and_then(|t| CarryForwardFile::parse(&t))
        {
            Ok(f) => records.extend(f.records.into_iter().map(|r| (name.clone(), r))),
            Err(e) => bad.push((name, e)),
        }
    }
    (records, bad)
}

/// [`ChainGit`] over the `git` command line, answers kept per call.
pub(crate) struct GitChain<'a> {
    root: &'a Path,
    /// The history tip: `HEAD`, or the command's `to`.
    tip: String,
    files: &'a mut FileCache,
    history: BTreeMap<String, bool>,
    clone: BTreeMap<String, bool>,
}

impl<'a> GitChain<'a> {
    pub(crate) fn new(root: &'a Path, tip: &str, files: &'a mut FileCache) -> Self {
        Self {
            root,
            tip: tip.to_string(),
            files,
            history: BTreeMap::new(),
            clone: BTreeMap::new(),
        }
    }
}

impl ChainGit for GitChain<'_> {
    fn in_history(&mut self, commit: &str) -> bool {
        let (root, tip) = (self.root, self.tip.clone());
        *self.history.entry(commit.to_string()).or_insert_with(|| {
            git::git(root, &["merge-base", "--is-ancestor", commit, &tip]).is_ok()
        })
    }
    fn in_clone(&mut self, commit: &str) -> bool {
        let root = self.root;
        *self
            .clone
            .entry(commit.to_string())
            .or_insert_with(|| git::git(root, &["cat-file", "-e", &format!("{commit}^{{commit}}")]).is_ok())
    }
    fn hash_at(&mut self, commit: &str, path: &str) -> Option<String> {
        let (file, qual) = path.split_once(".rs::")?;
        fn_at(self.files, self.root, commit, &format!("{file}.rs"), qual).map(|h| h.hashes.hash)
    }
}

/// `r`'s chain through `records` against `git`'s tip (the pure walk is
/// [`resolve_chain`]).
pub(crate) fn chain_of(r: &ReviewEntry, records: &[CarryForward], git: &mut GitChain) -> Chain {
    resolve_chain(r, records, git)
}

/// What one `carry-forward` run did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CarryForwardReport {
    /// The full id of `to`.
    pub to: String,
    /// The record file written (workspace-relative), when any was.
    pub file: Option<String>,
    /// The records written this run.
    pub written: Vec<CarryForward>,
    /// Signed stamps whose key is already `to` or an ancestor of it.
    pub in_history: usize,
    /// Signed stamps left out by `--from` (their key is not that commit or
    /// an ancestor of it).
    pub not_from: usize,
    /// `(function path, reason)` for stamps whose function at `to` is not
    /// the signed code: no record, the stamp goes stale.
    pub refused: Vec<(String, String)>,
    /// Record files that do not parse.
    pub unreadable: Vec<(String, String)>,
}

/// Resolve `rev`, fetching it from `origin` when this clone lacks it.
fn resolve_or_fetch(root: &Path, rev: &str) -> Result<String, String> {
    if let Ok(c) = git::rev_parse(root, rev) {
        return Ok(c);
    }
    git::git(root, &["fetch", "--quiet", "origin", rev])
        .map_err(|e| format!("{rev} is not in this clone and could not be fetched: {e}"))?;
    git::rev_parse(root, "FETCH_HEAD")
}

/// The places to look for `r`'s function at a new commit, first match
/// wins: the last hop's path, the review's `path`, the certified one.
fn candidate_paths(r: &ReviewEntry, chain: &Chain) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for p in [chain.path.clone(), r.path(), Some(certified_path(r))].into_iter().flatten() {
        if !p.is_empty() && !out.contains(&p) {
            out.push(p);
        }
    }
    out
}

/// Write the carry-forward records `to` calls for (module doc). `Err` when
/// the workspace cannot be read, `to` is not on this branch, `--from`
/// cannot be resolved or fetched, or the record file for `to` exists and
/// does not parse (it is never overwritten then).
pub fn carry_forward(root: &Path, from: Option<&str>, to: Option<&str>) -> Result<CarryForwardReport, String> {
    let ws = load_workspace(root)?;
    let head = git::rev_parse(root, "HEAD")?;
    let to = git::rev_parse(root, to.unwrap_or("HEAD"))?;
    if git::git(root, &["merge-base", "--is-ancestor", &to, &head]).is_err() {
        return Err(format!("{to} is not HEAD or an ancestor of it: carry forward on the branch that holds it"));
    }
    let from = from.map(|f| resolve_or_fetch(root, f)).transpose()?;
    let (loaded, unreadable) = load_records(root);
    let records: Vec<CarryForward> = loaded.into_iter().map(|(_, r)| r).collect();
    let mut files = FileCache::new();
    let mut report = CarryForwardReport {
        to: to.clone(),
        unreadable,
        ..CarryForwardReport::default()
    };
    for m in ws.reviews.values() {
        for r in m.doc.reviews() {
            if r.review.signature.is_none() {
                continue;
            }
            let mut g = GitChain::new(root, &to, &mut files);
            let chain = chain_of(r, &records, &mut g);
            if g.in_history(&chain.key) {
                report.in_history += 1;
                continue;
            }
            if let Some(f) = &from {
                let reachable = g.in_clone(&chain.key)
                    && git::git(root, &["merge-base", "--is-ancestor", &chain.key, f]).is_ok();
                if !reachable {
                    report.not_from += 1;
                    continue;
                }
            }
            let paths = candidate_paths(r, &chain);
            let mut seen = None;
            let mut found = None;
            for p in &paths {
                match g.hash_at(&to, p) {
                    Some(h) if h == r.review.hash => {
                        found = Some(p.clone());
                        break;
                    }
                    Some(h) => seen = seen.or(Some(h)),
                    None => {}
                }
            }
            match found {
                Some(path) => report.written.push(CarryForward {
                    stamp: r.kovan.id.clone(),
                    target: r.function_id(),
                    path,
                    hash: r.review.hash.clone(),
                    from: chain.key.clone(),
                    to: to.clone(),
                }),
                None => report.refused.push((
                    paths.first().cloned().unwrap_or_else(|| r.function_id()),
                    match seen {
                        Some(h) => format!(
                            "at {} it hashes to {h}, not the signed {}: it changed, so no record; review it again",
                            short(&to),
                            r.review.hash
                        ),
                        None => format!("it is not found at {}: no record", short(&to)),
                    },
                )),
            }
        }
    }
    report.written.sort();
    if !report.written.is_empty() {
        let rel = format!("{CARRY_FORWARD_DIR}/{}", file_name(&to));
        let path = root.join(&rel);
        let mut file = match std::fs::read_to_string(&path) {
            Ok(t) => CarryForwardFile::parse(&t).map_err(|e| format!("{rel} does not parse ({e}); not overwritten"))?,
            Err(_) => CarryForwardFile::new(),
        };
        file.merge(report.written.iter().cloned());
        std::fs::create_dir_all(root.join(CARRY_FORWARD_DIR)).map_err(|e| e.to_string())?;
        std::fs::write(&path, file.to_toml()).map_err(|e| format!("writing {rel}: {e}"))?;
        report.file = Some(rel);
    }
    Ok(report)
}

/// The first 10 characters of a commit id.
fn short(commit: &str) -> String {
    commit.chars().take(10).collect()
}

/// One record, re-checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordCheck {
    /// The file it is in (workspace-relative).
    pub file: String,
    pub record: CarryForward,
    /// `None` when no signed stamp in the workspace has the record's id.
    pub verdict: Option<HopCheck>,
}

/// Re-check every record (module doc, `--check`): the record against its
/// stamp's signed fields, and the function at `to` against the signed
/// hash. Needs no key. Returns the verdicts and the files that do not
/// parse.
pub fn check_records(root: &Path) -> Result<(Vec<RecordCheck>, Vec<(String, String)>), String> {
    let ws = load_workspace(root)?;
    let (records, bad) = load_records(root);
    let stamps: BTreeMap<&str, &ReviewEntry> = ws
        .reviews
        .values()
        .flat_map(|m| m.doc.reviews())
        .filter(|r| r.review.signature.is_some())
        .map(|r| (r.kovan.id.as_str(), r))
        .collect();
    let mut files = FileCache::new();
    let mut g = GitChain::new(root, "HEAD", &mut files);
    let mut out = Vec::new();
    for (file, record) in records {
        let verdict = stamps.get(record.stamp.as_str()).map(|e| {
            let in_clone = g.in_clone(&record.to);
            let hash = if in_clone { g.hash_at(&record.to, &record.path) } else { None };
            check_record(&record, e, hash.as_deref(), in_clone)
        });
        out.push(RecordCheck { file, record, verdict });
    }
    Ok((out, bad))
}

/// `kovan-cli review carry-forward` as text: with `check`, every record's
/// verdict and `Err` when any is rejected or a file does not parse;
/// otherwise the run's report. Plain CLI glue over [`carry_forward`] and
/// [`check_records`], reached by the tests below.
pub fn run(root: &Path, from: Option<&str>, to: Option<&str>, check: bool) -> Result<String, String> {
    if !root.join(ROOT_FILE).exists() && !root.join(".git").exists() {
        return Err(format!("{} is not a workspace root", root.display()));
    }
    let mut out = String::new();
    if check {
        let (rows, bad) = check_records(root)?;
        let mut failed = bad.len();
        for r in &rows {
            let what = match &r.verdict {
                Some(HopCheck::Verified) => "verified".to_string(),
                Some(HopCheck::NotCheckable) => "not checkable here (`to` is not in this clone)".to_string(),
                Some(HopCheck::Rejected(why)) => {
                    failed += 1;
                    format!("REJECTED: {}", why.describe())
                }
                None => "no signed stamp has this id (it was replaced or removed); ignored".to_string(),
            };
            out.push_str(&format!(
                "{} {} {} -> {}: {what}\n",
                r.file,
                r.record.stamp,
                short(&r.record.from),
                short(&r.record.to)
            ));
        }
        for (f, e) in &bad {
            out.push_str(&format!("{f}: does not parse: {e}\n"));
        }
        if rows.is_empty() && bad.is_empty() {
            out.push_str("No carry-forward records.\n");
        }
        return if failed > 0 {
            Err(format!("{out}{failed} record(s) or file(s) do not verify"))
        } else {
            Ok(out)
        };
    }
    let rep = carry_forward(root, from, to)?;
    out.push_str(&format!(
        "Carry forward to {}: {} written, {} already in history, {} refused{}.\n",
        short(&rep.to),
        rep.written.len(),
        rep.in_history,
        rep.refused.len(),
        if from.is_some() { format!(", {} not from --from", rep.not_from) } else { String::new() }
    ));
    if let Some(f) = &rep.file {
        out.push_str(&format!("Wrote {f} (unsigned; commit it).\n"));
    }
    for r in &rep.written {
        out.push_str(&format!("  carried {} {} -> {}\n", r.path, short(&r.from), short(&r.to)));
    }
    for (p, why) in &rep.refused {
        out.push_str(&format!("  refused {p}: {why}\n"));
    }
    for (f, e) in &rep.unreadable {
        out.push_str(&format!("  {f} does not parse, ignored: {e}\n"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use kovan_common::review::carry_forward::Rejection;
    use kovan_common::review::engine::ReviewedCommitTiming;
    use kovan_common::review::review_md::parse_review_md;
    use kovan_common::review::signing::registry::Registry;
    use kovan_common::review::signing::verify_review;
    use kovan_common::review::state::StateKind;

    use super::super::tests::{founder_key, Repo, DIR, LIB};
    use super::super::{evaluate_workspace, REVIEW_MD};
    use super::*;

    const SQUASH_MSG: &str = "Squash review into main\n\n* edit other\n\nCo-Authored-By: Claude <noreply@anthropic.com>\n";

    fn review_md(r: &Repo) -> String {
        r.read(&format!("{DIR}/{REVIEW_MD}"))
    }

    /// The signed stamp of `twice`.
    fn twice_entry(r: &Repo) -> ReviewEntry {
        parse_review_md(&review_md(r))
            .reviews()
            .find(|e| e.path().is_some_and(|p| p.ends_with("::twice")))
            .expect("twice is stamped")
            .clone()
    }

    /// Its signature verifies against the fixture's `kovan_root.toml`.
    fn signature_verifies(r: &Repo) -> bool {
        let ws = load_workspace(r.path()).unwrap();
        verify_review(&twice_entry(r), &Registry::build(&ws.review_root)).is_verified()
    }

    fn state(r: &Repo) -> (Option<StateKind>, u8, String) {
        let s = r.state("twice").expect("twice has a state");
        (s.state, s.rung, s.reason)
    }

    /// Delete `branch` and collect every object only it held, as GitHub does
    /// after a merge and a clone's `git gc` eventually does.
    fn delete_and_gc(r: &Repo, branch: &str) {
        r.git(&["branch", "-D", branch]);
        r.git(&["reflog", "expire", "--expire=now", "--all"]);
        r.git(&["gc", "-q", "--prune=now"]);
    }

    fn in_clone(r: &Repo, commit: &str) -> bool {
        git::git(r.path(), &["cat-file", "-e", &format!("{commit}^{{commit}}")]).is_ok()
    }

    /// The fixture with a founder key on `main`, then a `review` branch
    /// that edits `other`, re-indexes and stamps `twice` (the commit it
    /// certifies is returned), checked out back on `main` with one more
    /// commit there so no merge is a fast-forward.
    fn reviewed_branch() -> (Repo, tempfile::TempDir, String) {
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        r.git(&["checkout", "-q", "-b", "review"]);
        r.edit("    1\n", "    2\n");
        r.commit("edit other");
        r.reindex();
        r.stamp(&key, "twice", "Checked against the doc.");
        assert_eq!(state(&r).0, Some(StateKind::Valid));
        let certified = twice_entry(&r).review.commit;
        r.git(&["checkout", "-q", "main"]);
        r.write("NOTES.txt", "main moved on\n");
        r.commit("main moves on");
        (r, store, certified)
    }

    /// `git merge --squash <branch>` and one commit with `msg`.
    fn squash_merge(r: &Repo, branch: &str, msg: &str) -> String {
        r.git(&["merge", "-q", "--squash", branch]);
        r.git(&["commit", "-q", "-m", msg]);
        r.git(&["rev-parse", "HEAD"]).trim().to_string()
    }

    /// Methodology (#838, #1018 test 1, and the #838 shallow clone): stamp
    /// `twice` on a review branch, which was also pushed to a bare `origin`
    /// as `refs/pull/1/head`; squash-merge it into `main` with a message
    /// that, as GitHub's does, concatenates the branch's commits and their
    /// agent trailer; delete the branch and gc, so the certified commit is
    /// gone. Run carry-forward and commit its file; judge with signatures
    /// enforced. Then re-run it, `--check` it, clone at depth 2 (holds the
    /// squash commit) and depth 1 (does not), and run it with `--from
    /// refs/pull/1/head`.
    ///
    /// Pass: before the record the stamp is unverified (not in history);
    /// one record `certified -> squash` is written, `review.md` is byte for
    /// byte unchanged and the signature still verifies; the stamp is valid
    /// at rung 3 with the squash commit as its key and the #783 timing
    /// "not checkable (squashed)", and the permalink points at the squash
    /// commit; a second run writes nothing; `--check` verifies the record;
    /// the depth-2 clone shows it valid and the depth-1 clone unverified
    /// (the record's commit is not there: no record, no carry); `--from
    /// refs/pull/1/head` fetches the original commit back, and the timing
    /// check then runs.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn a_squash_merge_is_carried_forward_and_the_signature_kept() {
        let (r, _store, certified) = reviewed_branch();
        let origin = tempfile::tempdir().unwrap();
        r.git(&["init", "-q", "--bare", origin.path().to_str().unwrap()]);
        r.git(&["remote", "add", "origin", origin.path().to_str().unwrap()]);
        r.git(&["push", "-q", "origin", "review:refs/pull/1/head"]);
        let squash = squash_merge(&r, "review", SQUASH_MSG);
        delete_and_gc(&r, "review");
        assert!(!in_clone(&r, &certified), "the certified commit is collected");
        let (s, _, why) = state(&r);
        assert_eq!(s, Some(StateKind::Unverified));
        assert!(why.contains("not in this branch's history"), "{why}");

        let md_before = review_md(&r);
        let rep = carry_forward(r.path(), None, None).unwrap();
        assert_eq!(rep.written.len(), 1, "{rep:?}");
        assert!(rep.refused.is_empty(), "{rep:?}");
        let rec = &rep.written[0];
        assert_eq!((rec.from.as_str(), rec.to.as_str()), (certified.as_str(), squash.as_str()));
        assert_eq!(rec.path, format!("{LIB}::twice"));
        assert_eq!(rep.file.as_deref(), Some(format!("{CARRY_FORWARD_DIR}/{squash}.toml").as_str()));
        r.commit("carry forward");
        assert_eq!(review_md(&r), md_before, "the signed entry is never edited");
        assert!(signature_verifies(&r));

        let (s, rung, why) = state(&r);
        assert_eq!((s, rung), (Some(StateKind::Valid), 3), "{why}");
        let we = evaluate_workspace(r.path()).unwrap();
        let fr = we.evaluation.functions.values().find(|f| !f.reviews.is_empty()).unwrap();
        let cf = fr.reviews[0].carried_forward.as_ref().expect("carried forward");
        assert_eq!((cf.key.as_str(), cf.hops.len(), cf.stamp_arrived_in_hop), (squash.as_str(), 1, true));
        assert_eq!(fr.reviews[0].reviewed_commit_timing, ReviewedCommitTiming::NotCheckableSquashed);
        assert!(r.state("twice").unwrap().permalink.contains(&squash));

        let again = carry_forward(r.path(), None, None).unwrap();
        assert!(again.written.is_empty() && again.in_history == 1, "{again:?}");
        let (rows, bad) = check_records(r.path()).unwrap();
        assert!(bad.is_empty());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].verdict, Some(HopCheck::Verified));
        let out = run(r.path(), None, None, true).unwrap();
        assert!(out.contains("verified"), "{out}");

        // #838: a shallow clone that holds the record's commit is fixed.
        let url = format!("file://{}", r.path().display());
        for (depth, valid) in [("2", true), ("1", false)] {
            let c = tempfile::tempdir().unwrap();
            r.git(&["clone", "-q", "--depth", depth, &url, c.path().to_str().unwrap()]);
            let states = super::super::stamp_states(c.path()).unwrap();
            let s = states.iter().find(|s| s.function.ends_with("::twice")).unwrap();
            assert_eq!(s.state == Some(StateKind::Valid), valid, "depth {depth}: {}", s.reason);
        }

        // --from refs/pull/<n>/head brings the original back for #783.
        let out = run(r.path(), Some("refs/pull/1/head"), None, false).unwrap();
        assert!(out.contains("0 written"), "{out}");
        assert!(in_clone(&r, &certified));
        let we = evaluate_workspace(r.path()).unwrap();
        let fr = we.evaluation.functions.values().find(|f| !f.reviews.is_empty()).unwrap();
        assert_eq!(fr.reviews[0].reviewed_commit_timing, ReviewedCommitTiming::Checked);
        assert_eq!(state(&r).0, Some(StateKind::Valid));
    }

    /// Methodology (#1018 test 2, and a forged record): as the first test,
    /// but `twice` is edited in the squash commit itself; re-index. Run
    /// carry-forward; then write by hand a record claiming the squash
    /// commit with the signed hash (forged: the code there differs).
    ///
    /// Pass: no record is written, `twice` is listed as refused because it
    /// changed, and its state is directly stale; the forged record is
    /// rejected by `--check` (the function at `to` hashes otherwise) and
    /// the state stays directly stale.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn an_edit_in_the_squash_gets_no_record_and_goes_stale() {
        let (r, _store, certified) = reviewed_branch();
        r.git(&["merge", "-q", "--squash", "review"]);
        r.edit("leaf(x) + 1.0", "leaf(x) + 5.0");
        r.commit("squash review, with an edit");
        let squash = r.git(&["rev-parse", "HEAD"]).trim().to_string();
        delete_and_gc(&r, "review");
        r.reindex();

        let rep = carry_forward(r.path(), None, Some(&squash)).unwrap();
        assert!(rep.written.is_empty() && rep.file.is_none(), "{rep:?}");
        assert_eq!(rep.refused.len(), 1, "{rep:?}");
        assert!(rep.refused[0].1.contains("changed"), "{}", rep.refused[0].1);
        assert_eq!(state(&r).0, Some(StateKind::DirectlyStale));

        let e = twice_entry(&r);
        let mut forged = CarryForwardFile::new();
        forged.records.push(CarryForward {
            stamp: e.kovan.id.clone(),
            target: e.function_id(),
            path: format!("{LIB}::twice"),
            hash: e.review.hash.clone(),
            from: certified,
            to: squash.clone(),
        });
        r.write(&format!("{CARRY_FORWARD_DIR}/{squash}.toml"), &forged.to_toml());
        r.commit("forged record");
        let (rows, _) = check_records(r.path()).unwrap();
        assert!(matches!(rows[0].verdict, Some(HopCheck::Rejected(Rejection::ChangedAtTo { found: Some(_) }))), "{rows:?}");
        let err = run(r.path(), None, None, true).unwrap_err();
        assert!(err.contains("REJECTED"), "{err}");
        assert_eq!(state(&r).0, Some(StateKind::DirectlyStale));
    }

    /// Methodology (#1018 test 4): squash as in the first test, then write
    /// by hand a record for `twice` whose `hash` is not the signed one, and
    /// a second whose `target` is another function; no genuine record.
    ///
    /// Pass: both are rejected by `--check` (hash not signed; another
    /// function), the command fails, and the stamp stays unverified (not in
    /// history): a forged record carries nothing. A record file that does
    /// not parse is reported too.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn a_forged_record_is_rejected() {
        let (r, _store, certified) = reviewed_branch();
        let squash = squash_merge(&r, "review", "squash");
        delete_and_gc(&r, "review");
        let e = twice_entry(&r);
        let good = CarryForward {
            stamp: e.kovan.id.clone(),
            target: e.function_id(),
            path: format!("{LIB}::twice"),
            hash: e.review.hash.clone(),
            from: certified,
            to: squash.clone(),
        };
        let mut wrong_hash = good.clone();
        wrong_hash.hash = format!("sha256:{}", "0".repeat(64));
        let mut wrong_target = good;
        wrong_target.target = "fn:0000000000000000".into();
        let mut f = CarryForwardFile::new();
        f.records = vec![wrong_hash, wrong_target];
        r.write(&format!("{CARRY_FORWARD_DIR}/{squash}.toml"), &f.to_toml());
        r.write(&format!("{CARRY_FORWARD_DIR}/broken.toml"), "format = \"kovan-carry-forward-v1\"\n[[carry_forward]]\nstamp = 1\n");
        r.commit("forged records");

        let (rows, bad) = check_records(r.path()).unwrap();
        assert_eq!(bad.len(), 1, "{bad:?}");
        let why: Vec<_> = rows.iter().filter_map(|x| match &x.verdict {
            Some(HopCheck::Rejected(w)) => Some(w.clone()),
            _ => None,
        }).collect();
        assert_eq!(why.len(), 2, "{rows:?}");
        assert!(why.iter().any(|w| matches!(w, Rejection::HashNotSigned { .. })));
        assert!(why.iter().any(|w| matches!(w, Rejection::TargetMismatch { .. })));
        let err = run(r.path(), None, None, true).unwrap_err();
        assert!(err.contains("REJECTED") && err.contains("does not parse"), "{err}");
        let (s, _, why) = state(&r);
        assert_eq!(s, Some(StateKind::Unverified), "{why}");
        assert!(signature_verifies(&r));
    }

    /// Methodology (#1018 test 3): `main`, a `develop` branched from it,
    /// and a review branch from `develop` that edits `other`, re-indexes
    /// and stamps `twice` (so the certified commit is the branch's). Squash the
    /// review branch into `develop` and carry forward (hop 1); squash
    /// `develop` into `main` and carry forward (hop 2); delete both
    /// branches and gc, so the certified commit and hop 1's commit are both
    /// gone from the clone.
    ///
    /// Pass: one record per hop, in two files; the stamp is valid at rung 3
    /// on `main` with two hops (the first unchecked here: its commit was
    /// collected, while the second verifies the same signed hash at a
    /// commit this clone holds); the signature verifies.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn two_squash_hops_give_two_records_and_a_valid_stamp() {
        let r = Repo::new();
        let store = tempfile::tempdir().unwrap();
        let key = founder_key(&r, store.path());
        r.git(&["checkout", "-q", "-b", "develop"]);
        r.git(&["checkout", "-q", "-b", "review"]);
        r.edit("    1\n", "    2\n");
        r.commit("edit other");
        r.reindex();
        r.stamp(&key, "twice", "");
        let certified = twice_entry(&r).review.commit;
        r.git(&["checkout", "-q", "develop"]);
        let s1 = squash_merge(&r, "review", "squash review into develop");
        let hop1 = carry_forward(r.path(), None, None).unwrap();
        assert_eq!(hop1.written.len(), 1);
        assert_eq!((hop1.written[0].from.as_str(), hop1.written[0].to.as_str()), (certified.as_str(), s1.as_str()));
        r.commit("carry forward to develop");
        assert_eq!(state(&r).0, Some(StateKind::Valid));

        r.git(&["checkout", "-q", "main"]);
        let s2 = squash_merge(&r, "develop", "squash develop into main");
        assert_eq!(state(&r).0, Some(StateKind::Unverified), "hop 1 does not reach main's history");
        let hop2 = carry_forward(r.path(), None, None).unwrap();
        assert_eq!(hop2.written.len(), 1);
        assert_eq!((hop2.written[0].from.as_str(), hop2.written[0].to.as_str()), (s1.as_str(), s2.as_str()));
        r.commit("carry forward to main");
        delete_and_gc(&r, "review");
        delete_and_gc(&r, "develop");
        assert!(!in_clone(&r, &certified) && !in_clone(&r, &s1));

        let (records, _) = load_records(r.path());
        assert_eq!(records.len(), 2);
        let (s, rung, why) = state(&r);
        assert_eq!((s, rung), (Some(StateKind::Valid), 3), "{why}");
        let we = evaluate_workspace(r.path()).unwrap();
        let fr = we.evaluation.functions.values().find(|f| !f.reviews.is_empty()).unwrap();
        let cf = fr.reviews[0].carried_forward.as_ref().unwrap();
        assert_eq!((cf.key.as_str(), cf.hops.len()), (s2.as_str(), 2));
        assert!(signature_verifies(&r));
    }

    /// Methodology (#1018 test 5): the same reviewed branch reaches `main`
    /// by a regular merge (`--no-ff`) in one repository and by a rebase
    /// then fast-forward in another; each branch is deleted and gc'd, and
    /// carry-forward is run.
    ///
    /// Pass: identical outcome, valid at rung 3 with the signature
    /// verifying. The merge keeps the certified commit in history, so it
    /// writes no record (nothing to carry); the rebase replaced it, so it
    /// writes one, and is unverified until then.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn a_regular_merge_and_a_rebase_give_the_same_outcome() {
        let (m, _s1, certified) = reviewed_branch();
        m.git(&["merge", "-q", "--no-ff", "-m", "merge review", "review"]);
        delete_and_gc(&m, "review");
        assert!(in_clone(&m, &certified));
        let rep = carry_forward(m.path(), None, None).unwrap();
        assert!(rep.written.is_empty() && rep.in_history == 1, "{rep:?}");

        let (b, _s2, certified_b) = reviewed_branch();
        b.git(&["checkout", "-q", "review"]);
        b.git(&["rebase", "-q", "main"]);
        b.git(&["checkout", "-q", "main"]);
        b.git(&["merge", "-q", "--ff-only", "review"]);
        delete_and_gc(&b, "review");
        assert!(!in_clone(&b, &certified_b), "the rebase replaced the certified commit");
        assert_eq!(state(&b).0, Some(StateKind::Unverified));
        let rep = carry_forward(b.path(), None, None).unwrap();
        assert_eq!(rep.written.len(), 1, "{rep:?}");
        b.commit("carry forward");

        for r in [&m, &b] {
            let (s, rung, why) = state(r);
            assert_eq!((s, rung), (Some(StateKind::Valid), 3), "{why}");
            assert!(signature_verifies(r));
        }
    }

    /// Methodology: `--to` must be on the branch, `--from` must resolve or
    /// be fetchable, and an unparsable record file for `to` is never
    /// overwritten.
    ///
    /// Pass: each is an `Err` naming the cause.
    ///
    /// Result (2026-10-22): passes.
    #[test]
    fn bad_arguments_are_refused() {
        let (r, _store, _) = reviewed_branch();
        let e = carry_forward(r.path(), None, Some("review")).unwrap_err();
        assert!(e.contains("not HEAD or an ancestor"), "{e}");
        let e = carry_forward(r.path(), Some("refs/pull/9/head"), None).unwrap_err();
        assert!(e.contains("could not be fetched"), "{e}");
        let squash = squash_merge(&r, "review", "squash");
        delete_and_gc(&r, "review");
        r.write(&format!("{CARRY_FORWARD_DIR}/{squash}.toml"), "not toml [");
        let e = carry_forward(r.path(), None, None).unwrap_err();
        assert!(e.contains("not overwritten"), "{e}");
        let e = run(Path::new("/nonexistent-kovan-root"), None, None, false).unwrap_err();
        assert!(e.contains("not a workspace root"), "{e}");
    }
}
