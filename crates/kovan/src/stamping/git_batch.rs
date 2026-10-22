//! **The git questions of a review-state refresh, batched** (GitHub #1023).
//!
//! Before this, [`super::git_facts`] started one `git` process per
//! question: 316 on a refresh after a stamp in this workspace (31 stamps),
//! 4.1 s of a 5.4 s refresh on Linux, and process starts are much slower on
//! Windows. A [`GitBatch`] answers the same questions with few processes:
//!
//! | question | was | now |
//! |---|---|---|
//! | a file at a revision (`git show <rev>:./<file>`) | one `show` each | one long-lived `git cat-file --batch` per batch |
//! | the commit a revision names (`rev-parse --verify <rev>^{commit}`), "is it in this clone" (`cat-file -e`) | one process each | the same `cat-file --batch` |
//! | a commit's committer time (`show -s --format=%ct`) | one `show -s` each | parsed from the commit object (same `cat-file`) |
//! | `merge-base --is-ancestor <a> <b>` | one process each | one `git rev-list --parents <HEAD>`, walked in memory |
//! | the commit that added a stamp (`log -1 -S<value> -- review.md`) | one pickaxe `log` per stamp | one `log` per `review.md`, the occurrences counted here ([`super::states`]) |
//! | `git blame` of a reviewed function | one per function | one per file, every range in it (`-L` repeated) |
//!
//! **Same answers.** Each batched answer is the old command's, and where the
//! batch cannot give it exactly it runs the old command: `cat-file` not
//! starting or its stream breaking, an object that is not a blob (`show` of
//! a tree), a commit header that does not parse, an ancestry question about
//! a commit outside `HEAD`'s history, a multi-range `blame` that fails.
//! Failures stay values (`None`, `false`, empty), as before.
//!
//! **Kept per `HEAD`** for the life of the process (a refresh after a stamp
//! that was not committed sees the same `HEAD`): the history walk, each
//! `review.md`'s log, the stamp commits, the committer times of commits in
//! `HEAD`'s history, the last commit that changed a file, and the blame of a
//! file whose working-tree text (and `.mailmap`) is unchanged. All of these
//! are fixed by `HEAD` (and, for blame, the text): commits outside `HEAD`'s
//! history can be fetched or collected without `HEAD` moving, so what is
//! said about them is asked again each batch. A new `HEAD` starts afresh.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

use kovan_common::review::engine::StampCommit;
use kovan_common::review::hash::HashedFn;

use super::git;

/// One object as `cat-file --batch` gives it.
struct Object {
    id: String,
    kind: String,
    data: Vec<u8>,
}

/// One running `git cat-file --batch`.
struct CatFile {
    child: Child,
    stdin: Option<ChildStdin>,
    out: BufReader<ChildStdout>,
}

impl CatFile {
    fn start(root: &Path) -> Result<Self, String> {
        let args = ["cat-file", "--batch"];
        let started = Instant::now();
        let spawned = Command::new("git")
            .current_dir(root)
            .env("GIT_TERMINAL_PROMPT", "0")
            .args(["-c", "core.quotePath=false"])
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        git::count_spawn(&args, started.elapsed());
        let mut child = spawned.map_err(|e| format!("could not run git: {e}"))?;
        let (Some(stdin), Some(out)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("git cat-file: no pipes".into());
        };
        Ok(Self { child, stdin: Some(stdin), out: BufReader::new(out) })
    }

    /// `Ok(None)`: git says the name is missing (or ambiguous). `Err`: the
    /// stream is no longer usable.
    fn get(&mut self, name: &str) -> Result<Option<Object>, String> {
        let stdin = self.stdin.as_mut().ok_or("closed")?;
        stdin.write_all(name.as_bytes()).and_then(|()| stdin.write_all(b"\n")).and_then(|()| stdin.flush()).map_err(|e| e.to_string())?;
        let mut header = String::new();
        if self.out.read_line(&mut header).map_err(|e| e.to_string())? == 0 {
            return Err("git cat-file closed".into());
        }
        let header = header.trim_end_matches('\n');
        // `<name> missing` / `<name> ambiguous` (the name may be empty or
        // hold spaces): no content follows.
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            return Ok(None);
        }
        let mut parts = header.rsplitn(3, ' ');
        let (Some(size), Some(kind), Some(id)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!("git cat-file: {header}"));
        };
        let Ok(size) = size.parse::<usize>() else {
            return Err(format!("git cat-file: {header}"));
        };
        let mut data = vec![0; size + 1];
        self.out.read_exact(&mut data).map_err(|e| e.to_string())?;
        if data.pop() != Some(b'\n') {
            return Err("git cat-file: no newline after the object".into());
        }
        Ok(Some(Object { id: id.to_string(), kind: kind.to_string(), data }))
    }
}

impl Drop for CatFile {
    fn drop(&mut self) {
        drop(self.stdin.take()); // end of input: cat-file exits
        let _ = self.child.wait();
    }
}

enum Cat {
    NotStarted,
    Running(CatFile),
    Broken,
}

/// `HEAD`'s history: each commit with its parents (`git rev-list
/// --parents`).
#[derive(Debug, Default)]
struct History {
    parents: HashMap<String, Vec<String>>,
}

impl History {
    fn parse(text: &str) -> Self {
        let parents = text
            .lines()
            .filter_map(|l| {
                let mut it = l.split_whitespace();
                let c = it.next()?;
                Some((c.to_string(), it.map(str::to_string).collect()))
            })
            .collect();
        Self { parents }
    }

    /// Whether `a` is `b` or an ancestor of it (`b` in this history).
    fn reaches(&self, b: &str, a: &str) -> bool {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut todo: VecDeque<&str> = VecDeque::from([b]);
        while let Some(c) = todo.pop_front() {
            if c == a {
                return true;
            }
            if seen.insert(c) {
                todo.extend(self.parents.get(c).into_iter().flatten().map(String::as_str));
            }
        }
        false
    }
}

/// One commit of a file's log ([`GitBatch::file_log`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogCommit {
    pub id: String,
    pub parents: Vec<String>,
    /// `%ct`.
    pub time: Option<i64>,
    /// `%B` and the newline `git log` ends each entry with.
    pub body: String,
}

impl LogCommit {
    /// `git log --format=%x00%H%x1f%P%x1f%ct%x1f%B` output, in order.
    fn parse_all(out: &str) -> Vec<Self> {
        out.split('\0')
            .skip(1)
            .filter_map(|rec| {
                let mut f = rec.splitn(4, '\x1f');
                let id = f.next()?.trim().to_string();
                let parents = f.next()?.split_whitespace().map(str::to_string).collect();
                let time = f.next().and_then(|t| t.trim().parse().ok());
                let body = f.next().unwrap_or("").to_string();
                (!id.is_empty()).then_some(Self { id, parents, time, body })
            })
            .collect()
    }
}

/// Blame of one file's ranges: the file, its working-tree text hash, the
/// `.mailmap` hash, the ranges.
type BlameKey = (String, u64, u64, Vec<[u32; 2]>);

/// What is kept per `HEAD` (module doc).
#[derive(Debug, Clone, Default)]
struct Kept {
    head: String,
    history: Option<Option<Arc<History>>>,
    logs: BTreeMap<String, Option<Arc<Vec<LogCommit>>>>,
    added: BTreeMap<(String, String, String), Option<StampCommit>>,
    times: BTreeMap<String, Option<i64>>,
    last_change: BTreeMap<String, Option<String>>,
    blame: BTreeMap<BlameKey, Arc<BTreeMap<[u32; 2], Vec<String>>>>,
}

/// More roots than this and the store starts again (tests use many).
const KEPT_ROOTS: usize = 8;

fn store() -> &'static Mutex<HashMap<PathBuf, Kept>> {
    static KEPT: OnceLock<Mutex<HashMap<PathBuf, Kept>>> = OnceLock::new();
    KEPT.get_or_init(Default::default)
}

/// (revision, file) -> the file's hashed functions there (`None`: absent or
/// not Rust that parses).
pub(crate) type FileCache = BTreeMap<(String, String), Option<Vec<HashedFn>>>;

/// The git questions of one refresh (or one stamp, one command), answered
/// with few processes (module doc). Not shared between threads; create one
/// per job. Dropping it ends its `cat-file` and keeps the per-`HEAD`
/// answers for the next one.
pub(crate) struct GitBatch {
    root: PathBuf,
    cat: Cat,
    head: Option<Option<String>>,
    kept: Kept,
    blobs: BTreeMap<(String, String), Option<String>>,
    resolved: BTreeMap<String, Option<String>>,
    ancestry: BTreeMap<(String, String), bool>,
    /// The functions of a file at a revision ([`super::states::fn_at`]).
    pub(crate) files: FileCache,
}

impl GitBatch {
    pub(crate) fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            cat: Cat::NotStarted,
            head: None,
            kept: Kept::default(),
            blobs: BTreeMap::new(),
            resolved: BTreeMap::new(),
            ancestry: BTreeMap::new(),
            files: FileCache::new(),
        }
    }

    /// Whether the `cat-file` is running (tests: nothing fell back).
    #[cfg(test)]
    fn cat_running(&self) -> bool {
        matches!(self.cat, Cat::Running(_))
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    /// The object `name` names: `Some(None)` when git says missing, `None`
    /// when the batch cannot ask (the caller runs the old command).
    fn object(&mut self, name: &str) -> Option<Option<Object>> {
        if name.contains(['\n', '\r']) {
            return None;
        }
        if matches!(self.cat, Cat::NotStarted) {
            self.cat = CatFile::start(&self.root).map_or(Cat::Broken, Cat::Running);
        }
        let Cat::Running(c) = &mut self.cat else {
            return None;
        };
        match c.get(name) {
            Ok(o) => Some(o),
            Err(_) => {
                self.cat = Cat::Broken;
                None
            }
        }
    }

    /// `file` (relative to the root) at `rev`: [`git::show`]'s answer.
    pub(crate) fn show(&mut self, rev: &str, file: &str) -> Option<String> {
        let key = (rev.to_string(), file.to_string());
        if let Some(t) = self.blobs.get(&key) {
            return t.clone();
        }
        let text = match self.object(&format!("{rev}:./{file}")) {
            Some(Some(o)) if o.kind == "blob" => Some(String::from_utf8_lossy(&o.data).into_owned()),
            Some(None) => None,
            _ => git::show(&self.root, rev, file),
        };
        self.blobs.insert(key, text.clone());
        text
    }

    /// The full commit id `rev` names: [`git::rev_parse`]'s answer, `None`
    /// for its error.
    pub(crate) fn resolve(&mut self, rev: &str) -> Option<String> {
        if let Some(c) = self.resolved.get(rev) {
            return c.clone();
        }
        let id = match self.object(&format!("{rev}^{{commit}}")) {
            Some(Some(o)) if o.kind == "commit" => Some(o.id),
            Some(Some(_)) | None => git::rev_parse(&self.root, rev).ok(),
            Some(None) => None,
        };
        self.resolved.insert(rev.to_string(), id.clone());
        id
    }

    /// Whether `commit` is a commit in this clone (`git cat-file -e
    /// <commit>^{commit}`).
    pub(crate) fn in_clone(&mut self, commit: &str) -> bool {
        self.resolve(commit).is_some()
    }

    /// `HEAD`'s commit id; the per-`HEAD` answers are taken from the store
    /// the first time.
    pub(crate) fn head(&mut self) -> Option<String> {
        if let Some(h) = &self.head {
            return h.clone();
        }
        let h = self.resolve("HEAD");
        if let Some(id) = &h {
            let kept = store().lock().ok().and_then(|s| s.get(&self.root).filter(|k| k.head == *id).cloned());
            self.kept = kept.unwrap_or_else(|| Kept { head: id.clone(), ..Kept::default() });
        }
        self.head = Some(h.clone());
        h
    }

    fn history(&mut self) -> Option<Arc<History>> {
        let head = self.head()?;
        if let Some(h) = &self.kept.history {
            return h.clone();
        }
        let h = git::git(&self.root, &["rev-list", "--parents", &head]).ok().map(|t| Arc::new(History::parse(&t)));
        self.kept.history = Some(h.clone());
        h
    }

    /// Whether `commit` is in `HEAD`'s history, from the walk alone
    /// (`None`: not known, no `HEAD` or the walk failed).
    fn in_head_history(&mut self, commit: &str) -> Option<bool> {
        Some(self.history()?.parents.contains_key(commit))
    }

    /// `git merge-base --is-ancestor <a> <b>` succeeds.
    pub(crate) fn is_ancestor(&mut self, a: &str, b: &str) -> bool {
        let key = (a.to_string(), b.to_string());
        if let Some(&v) = self.ancestry.get(&key) {
            return v;
        }
        let v = match (self.resolve(a), self.resolve(b)) {
            (Some(a_id), Some(b_id)) => match self.history() {
                Some(h) if h.parents.contains_key(&b_id) => h.reaches(&b_id, &a_id),
                _ => git::git(&self.root, &["merge-base", "--is-ancestor", a, b]).is_ok(),
            },
            // Not a commit in this clone: merge-base fails.
            _ => false,
        };
        self.ancestry.insert(key, v);
        v
    }

    /// `git show -s --format=%ct <rev>`, parsed.
    pub(crate) fn committer_time(&mut self, rev: &str) -> Option<i64> {
        let id = self.resolve(rev);
        let stable = match &id {
            Some(id) => self.in_head_history(id).unwrap_or(false),
            None => false,
        };
        if let (true, Some(t)) = (stable, id.as_ref().and_then(|i| self.kept.times.get(i))) {
            return *t;
        }
        let parsed = match self.object(rev) {
            Some(Some(o)) if o.kind == "commit" => committer_seconds(&o.data),
            Some(None) => Some(None),
            _ => None,
        };
        let t = parsed.unwrap_or_else(|| {
            git::git(&self.root, &["show", "-s", "--format=%ct", rev]).ok().and_then(|s| s.trim().parse().ok())
        });
        if let (true, Some(id)) = (stable, id) {
            self.kept.times.insert(id, t);
        }
        t
    }

    /// `git log` of `file` from `HEAD` (history simplified for that path,
    /// as `git log -- <file>` does), newest first.
    pub(crate) fn file_log(&mut self, file: &str) -> Option<Arc<Vec<LogCommit>>> {
        let head = self.head()?;
        if let Some(l) = self.kept.logs.get(file) {
            return l.clone();
        }
        let l = git::git(&self.root, &["log", "--format=%x00%H%x1f%P%x1f%ct%x1f%B", &head, "--", file])
            .ok()
            .map(|out| Arc::new(LogCommit::parse_all(&out)));
        self.kept.logs.insert(file.to_string(), l.clone());
        l
    }

    /// A stamp commit kept for this `HEAD` (`Some(answer)`), or `None`.
    pub(crate) fn kept_added(&mut self, key: &(String, String, String)) -> Option<Option<StampCommit>> {
        self.head()?;
        self.kept.added.get(key).cloned()
    }

    /// Keep a stamp commit for this `HEAD` (it depends on `HEAD` alone).
    pub(crate) fn keep_added(&mut self, key: (String, String, String), v: Option<StampCommit>) {
        if self.head().is_some() {
            self.kept.added.insert(key, v);
        }
    }

    /// `git log -1 --format=%H -- <file>`, trimmed.
    pub(crate) fn last_change(&mut self, file: &str) -> Option<String> {
        let known = self.head().is_some();
        if let (true, Some(c)) = (known, self.kept.last_change.get(file)) {
            return c.clone();
        }
        let c = git::git(&self.root, &["log", "-1", "--format=%H", "--", file]).ok().map(|s| s.trim().to_string());
        if known {
            self.kept.last_change.insert(file.to_string(), c.clone());
        }
        c
    }

    /// The `git blame` author e-mails of each of `ranges` of `file` in the
    /// working tree (each as [`blame_range`] gives it alone), with one
    /// `blame` for all of them.
    pub(crate) fn blame(&mut self, file: &str, ranges: &BTreeSet<[u32; 2]>) -> BTreeMap<[u32; 2], Vec<String>> {
        let ranges: Vec<[u32; 2]> = ranges.iter().copied().collect();
        let key = match (self.head(), std::fs::read(self.root.join(file))) {
            (Some(_), Ok(bytes)) => {
                let mailmap = std::fs::read(self.root.join(".mailmap")).unwrap_or_default();
                Some((file.to_string(), bytes_hash(&bytes), bytes_hash(&mailmap), ranges.clone()))
            }
            _ => None,
        };
        if let Some(b) = key.as_ref().and_then(|k| self.kept.blame.get(k)) {
            return (**b).clone();
        }
        let mut args: Vec<String> = vec!["blame".into(), "--line-porcelain".into()];
        for [a, b] in &ranges {
            args.push("-L".into());
            args.push(format!("{a},{b}"));
        }
        args.extend(["--".into(), file.into()]);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let out: BTreeMap<[u32; 2], Vec<String>> = match git::git(&self.root, &refs) {
            Ok(out) => {
                let by_line = porcelain_mails(&out);
                ranges
                    .iter()
                    .map(|&[a, b]| ([a, b], tidy_mails((a..=b).filter_map(|l| by_line.get(&l).cloned()).collect())))
                    .collect()
            }
            // One range git refuses (past the end of the file, say) fails
            // the whole call: ask range by range, as before.
            Err(_) => ranges.iter().map(|&[a, b]| ([a, b], blame_range(&self.root, file, a, b))).collect(),
        };
        if let Some(k) = key {
            self.kept.blame.insert(k, Arc::new(out.clone()));
        }
        out
    }
}

impl Drop for GitBatch {
    fn drop(&mut self) {
        if self.kept.head.is_empty() {
            return;
        }
        if let Ok(mut s) = store().lock() {
            if s.len() >= KEPT_ROOTS && !s.contains_key(&self.root) {
                s.clear();
            }
            s.insert(self.root.clone(), std::mem::take(&mut self.kept));
        }
    }
}

fn bytes_hash(b: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

/// The committer time of a raw commit object, as `%ct` prints it, as
/// `Some(Some(seconds))`; `None` for a header this does not read (the caller
/// then asks git).
fn committer_seconds(data: &[u8]) -> Option<Option<i64>> {
    let text = std::str::from_utf8(data).ok()?;
    let header = text.split("\n\n").next()?;
    let line = header.lines().find_map(|l| l.strip_prefix("committer "))?;
    let (_, after) = line.rsplit_once('>')?;
    let mut it = after.split_whitespace();
    let secs = it.next()?.parse::<i64>().ok()?;
    let tz = it.next()?;
    (it.next().is_none() && tz.len() == 5 && (tz.starts_with('+') || tz.starts_with('-'))).then_some(Some(secs))
}

/// Final line number -> `author-mail` of `git blame --line-porcelain`.
fn porcelain_mails(out: &str) -> BTreeMap<u32, String> {
    let mut map = BTreeMap::new();
    let mut line: Option<u32> = None;
    for l in out.lines() {
        if l.starts_with('\t') {
            continue;
        }
        let parts: Vec<&str> = l.split(' ').collect();
        if (3..=4).contains(&parts.len())
            && parts[0].len() >= 40
            && parts[0].bytes().all(|b| b.is_ascii_hexdigit())
            && parts[1..].iter().all(|p| p.parse::<u32>().is_ok())
        {
            line = parts[2].parse().ok();
        } else if let (Some(m), Some(n)) = (l.strip_prefix("author-mail "), line) {
            map.insert(n, m.to_string());
        }
    }
    map
}

/// The e-mails as the refresh records them: `<>` removed, empty and
/// "not committed yet" dropped, sorted, once each.
fn tidy_mails(raw: Vec<String>) -> Vec<String> {
    let mut v: Vec<String> = raw
        .iter()
        .map(|m| m.trim().trim_start_matches('<').trim_end_matches('>').to_string())
        .filter(|m| !m.is_empty() && m != "not.committed.yet")
        .collect();
    v.sort();
    v.dedup();
    v
}

/// `git blame` author e-mails of `file` lines `a..=b` in the working tree,
/// one `blame` (the call before #1023, and the fallback).
pub(crate) fn blame_range(root: &Path, file: &str, a: u32, b: u32) -> Vec<String> {
    let range = format!("{a},{b}");
    let Ok(out) = git::git(root, &["blame", "--line-porcelain", "-L", &range, "--", file]) else {
        return Vec::new();
    };
    tidy_mails(out.lines().filter_map(|l| l.strip_prefix("author-mail ")).map(str::to_string).collect())
}

#[cfg(test)]
#[path = "git_batch_tests.rs"]
mod tests;
