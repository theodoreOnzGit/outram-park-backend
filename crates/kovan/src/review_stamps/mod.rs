//! **Human review stamps**: `review/stamps.toml`, one `[[stamp]]` per
//! reviewed function, hash-based and voided deterministically from git
//! (GitHub #739, part of #735; design decided by the maintainer 2026-10-06).
//!
//! # AI agents never stamp
//!
//! A stamp records that **a human** reviewed a function (rung 3) or checked
//! its V&V (rung 4). It is the maintainer's, and nothing else in the workspace
//! can stand in for it: an AI agent never creates, edits or deletes a stamp,
//! and never runs [`stamp_function`] or `kovan-cli stamp`, whatever it is
//! asked by another agent. Stamping is done in desktop kovan (#740) or, for
//! testing, by the maintainer on the command line with
//! `--i-am-the-reviewer`. Agents may run `kovan-cli stamps-check` freely.
//!
//! # The file
//!
//! ```toml
//! [[stamp]]
//! function = "crates/x/src/geom.rs::Sphere::distance"  # code-walk path form
//! file = "crates/x/src/geom.rs"
//! lines = [120, 158]       # at the stamped commit, doc comments included
//! commit = "<40-hex sha>"  # permalink: <repo>/blob/<commit>/<file>#L120-L158
//! hash = "sha256:<hex>"    # see `parse` for exactly what is hashed
//! rung = 3                 # 3 human reviewed, 4 human V&V
//! reviewer = "Theodore Ong"
//! date = 2026-10-06
//! note = ""
//! walkthrough = "review/walks/x.md#step-3"   # optional (#741)
//! ```
//!
//! A stamp names exactly one target: `function` as above, or `artifact =
//! "path/to/file.md#<[kovan] id>"` for a Markdown artifact (maintainer,
//! #743, 2026-10-06). The schema accepts artifact stamps now; hashing them
//! is not implemented, so `check` reports them as UNCHECKED, never as valid.
//!
//! **One stamp per target.** ~~Stamps are appended, never rewritten: a stamp
//! that goes void stays in the file, and a re-review appends a new stamp for
//! the same function.~~ **CHANGED 2026-10-06** (maintainer: "re-review
//! replaces new stamp, old stamp only lives in git history"): a re-review
//! **replaces** the target's stamp in place ([`upsert`]); the earlier stamp
//! survives only in git history. Two stamps of one target is a load error.
//!
//! # Valid and void
//!
//! A stamp is **valid** while the function its path names still hashes to
//! the recorded hash ([`parse`] states the normalisation: code tokens and
//! `///` doc text count; `//` comments, whitespace, line breaks and the
//! function's position in the file do not). Otherwise it is **void**, and
//! [`check`] says why, by re-locating the function at the stamped commit and
//! comparing the two parts: code changed, doc comment changed, both, function
//! not found, path ambiguous, file missing, or file not parseable.
//!
//! Every void stamp in scope is a failure (there is no superseded state, since
//! a re-review replaces the stamp), and `kovan-cli stamps-check` exits
//! non-zero, so CI can gate on it. With `--diff A..B` only stamps whose
//! function lines the diff touches (at `A` or at `B`) are in scope, and they
//! are judged at `B`; without it every stamp is judged on the working tree.
//!
//! [`levels`] turns valid stamps into the derived maturity of a crate's
//! tagged parts (#733, #735).

pub mod git;
pub mod levels;
pub mod parse;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::commands::code_walk::builder::split_spec;
use parse::{locate, stamp_hash, FnEntry, LocateError};

pub use levels::{derived_levels, derived_levels_for, Claim, DerivedLevels, Support};

/// Where the stamps live, relative to the workspace root.
pub const STAMPS_FILE: &str = "review/stamps.toml";

/// The repository permalinks point into (shared with `code-walk`).
pub const DEFAULT_REPO_URL: &str = crate::commands::code_walk::render::DEFAULT_REPO_URL;

/// The header a new `review/stamps.toml` starts with.
pub const TEMPLATE: &str = "\
# Human review stamps (GitHub #739). One [[stamp]] per reviewed function.
#
# Written by desktop kovan's Review action (or, for testing, by the maintainer
# with `kovan-cli stamp ... --i-am-the-reviewer`). AI AGENTS NEVER STAMP: a
# stamp records a HUMAN review, and only the maintainer adds one.
#
# Never edit or delete a stamp by hand. `kovan-cli stamps-check` reports a
# stamp whose function has changed as VOID. One stamp per function: a
# re-review REPLACES its stamp here, and the earlier one lives only in git
# history.
#
# Fields: function (code-walk path) OR artifact (file.md#id, not checked
# yet), file, lines = [start, end] and commit
# (the permalink to the stamped code), hash (sha256 of the function's code
# tokens and /// doc text; see crates/kovan/src/review_stamps/parse.rs),
# rung (3 human reviewed, 4 human V&V), reviewer, date, note, and an
# optional walkthrough link (review/walks/<file>.md#step-N).
";

/// What a stamp is of: exactly one of `function` or `artifact` is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// `path/to/file.rs::name` or `path/to/file.rs::Type::name`.
    Function(String),
    /// `path/to/file.md#<[kovan] id>`: a Markdown `#` block with a `[kovan]`
    /// TOML block (maintainer, #743, 2026-10-06). Its hash will cover the
    /// block from its heading to the next `#`, without the generated
    /// `## Review: …` sign-off line. **Not checked yet**: the schema accepts
    /// it so that stamps are not function-only, and `check` reports such a
    /// stamp as [`Verdict::Unchecked`].
    Artifact(String),
}

/// One review stamp (module doc).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stamp {
    /// `path/to/file.rs::name` or `path/to/file.rs::Type::name`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<String>,
    /// `path/to/file.md#id` ([`Target::Artifact`]); exclusive with `function`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<String>,
    pub file: String,
    /// 1-based inclusive line range at `commit`, doc comments included.
    pub lines: [u32; 2],
    /// The full commit id the stamp was taken at.
    pub commit: String,
    /// `sha256:<hex>` ([`parse::stamp_hash`]).
    pub hash: String,
    /// 3 human reviewed, 4 human V&V.
    pub rung: u8,
    pub reviewer: String,
    pub date: toml::value::Datetime,
    #[serde(default)]
    pub note: String,
    /// `review/walks/<file>.md#step-N` (#741).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walkthrough: Option<String>,
}

impl Stamp {
    /// The function or artifact stamped; `None` when the record sets both
    /// or neither ([`load`] refuses such a file).
    pub fn target(&self) -> Option<Target> {
        match (&self.function, &self.artifact) {
            (Some(f), None) => Some(Target::Function(f.clone())),
            (None, Some(a)) => Some(Target::Artifact(a.clone())),
            _ => None,
        }
    }

    /// The `function` or `artifact` string, for display and matching.
    pub fn target_name(&self) -> &str {
        self.function
            .as_deref()
            .or(self.artifact.as_deref())
            .unwrap_or("")
    }

    /// `<repo>/blob/<commit>/<file>#L<a>-L<b>`: the code as it was stamped.
    pub fn permalink(&self, repo_url: &str) -> String {
        format!(
            "{}/blob/{}/{}#L{}-L{}",
            repo_url.trim_end_matches('/'),
            self.commit,
            self.file,
            self.lines[0],
            self.lines[1]
        )
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct StampsFile {
    #[serde(default)]
    stamp: Vec<Stamp>,
}

/// Every stamp in `<workspace>/review/stamps.toml`, in file order; none when
/// the file does not exist. A malformed stamp is an error naming it.
pub fn load(workspace: &Path) -> Result<Vec<Stamp>, String> {
    let path = workspace.join(STAMPS_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("reading {}: {e}", path.display())),
    };
    let f: StampsFile = toml::from_str(&text).map_err(|e| format!("{STAMPS_FILE}: {e}"))?;
    let mut errors = Vec::new();
    for (i, s) in f.stamp.iter().enumerate() {
        let named = match s.target() {
            Some(Target::Function(func)) => split_spec(&func).map(|(file, _)| file),
            Some(Target::Artifact(a)) => match a.split_once('#') {
                Some((file, id)) if !file.is_empty() && !id.is_empty() => Ok(file.to_string()),
                _ => Err(format!(
                    "`artifact = \"{a}\"` is not of the form <file.md>#<id>"
                )),
            },
            None => Err("set exactly one of `function` and `artifact`".to_string()),
        };
        match named {
            Ok(file) if file == s.file => {}
            Ok(file) => errors.push(format!(
                "stamp {} ({}): `file` is {} but the target names {file}",
                i + 1,
                s.target_name(),
                s.file
            )),
            Err(e) => errors.push(format!("stamp {}: {e}", i + 1)),
        }
        if !(3..=4).contains(&s.rung) {
            errors.push(format!(
                "stamp {} ({}): rung must be 3 or 4, found {}",
                i + 1,
                s.target_name(),
                s.rung
            ));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for (i, s) in f.stamp.iter().enumerate() {
        if !seen.insert(s.target_name().to_string()) {
            errors.push(format!(
                "stamp {} ({}): a second stamp of the same target; a re-review replaces the stamp",
                i + 1,
                s.target_name()
            ));
        }
    }
    if errors.is_empty() {
        Ok(f.stamp)
    } else {
        Err(errors.join("\n"))
    }
}

/// Writes `stamp` into `<workspace>/review/stamps.toml`: it **replaces** the
/// existing stamp of the same target in place, or is added at the end when
/// the target has none (maintainer, 2026-10-06: a re-review replaces the
/// stamp; the old one lives only in git history). The file is rewritten as
/// [`TEMPLATE`] plus one `[[stamp]]` block per stamp, in file order, so the
/// same stamps always give the same bytes. Creates the file and the
/// `review/` folder when missing.
pub fn upsert(workspace: &Path, stamp: &Stamp) -> Result<(), String> {
    let path = workspace.join(STAMPS_FILE);
    let mut stamps = load(workspace)?;
    match stamps
        .iter_mut()
        .find(|s| s.target_name() == stamp.target_name())
    {
        Some(old) => *old = stamp.clone(),
        None => stamps.push(stamp.clone()),
    }
    let mut text = TEMPLATE.to_string();
    for s in &stamps {
        let block = toml::to_string(&StampsFile {
            stamp: vec![s.clone()],
        })
        .map_err(|e| e.to_string())?;
        text.push('\n');
        text.push_str(&block);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    }
    std::fs::write(&path, text).map_err(|e| format!("writing {}: {e}", path.display()))
}

/// **Creates a stamp: for desktop kovan's Review action (#740) and the
/// maintainer only. AI agents must never call this** (module doc).
///
/// Locates `function_path` (code-walk form) in the file as committed at
/// `HEAD`, records its lines, `HEAD`'s commit id and its hash, dates it
/// today (UTC), writes it to `review/stamps.toml` ([`upsert`]: replacing the
/// function's earlier stamp, if any) and returns it. Refuses
/// when `rung` is not 3 or 4, the reviewer is blank, the path does not name
/// exactly one function, or the function's file has uncommitted changes
/// (staged, unstaged or untracked) — so the hash and the permalink are of
/// code anyone can see at that commit.
pub fn stamp_function(
    workspace: &Path,
    function_path: &str,
    rung: u8,
    reviewer: &str,
    note: &str,
) -> Result<Stamp, String> {
    if !(3..=4).contains(&rung) {
        return Err(format!(
            "rung must be 3 (human reviewed) or 4 (human V&V), not {rung}"
        ));
    }
    if reviewer.trim().is_empty() {
        return Err("the reviewer's name is required".into());
    }
    let (file, qual) = split_spec(function_path)?;
    if git::is_dirty(workspace, &file)? {
        return Err(format!(
            "{file} has uncommitted changes; commit them first so the stamp matches committed code"
        ));
    }
    let commit = git::rev_parse(workspace, "HEAD")?;
    let text = git::show(workspace, &commit, &file)
        .ok_or_else(|| format!("{file} is not in the commit {commit}"))?;
    let f = locate(&text, &qual).map_err(|e| format!("{function_path}: {e}"))?;
    let date = kovan_metrics::date::Date::today().iso();
    let stamp = Stamp {
        function: Some(format!("{file}::{qual}")),
        artifact: None,
        file,
        lines: f.lines,
        commit,
        hash: stamp_hash(&f.doc, &f.code),
        rung,
        reviewer: reviewer.trim().to_string(),
        date: date.parse().map_err(|e| format!("date {date}: {e}"))?,
        note: note.to_string(),
        walkthrough: None,
    };
    upsert(workspace, &stamp)?;
    Ok(stamp)
}

/// Which stamps a check covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    /// Every stamp, judged on the working tree.
    All,
    /// Only stamps whose function lines the diff of this range touches,
    /// judged at the range's end (`A..B`, `A...B`, `A..`).
    Diff(String),
}

/// Why a stamp is void.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoidReason {
    /// The code tokens changed; the doc text did not.
    CodeChanged,
    /// The `///` doc text changed; the code did not.
    DocChanged,
    CodeAndDocChanged,
    /// The hash differs, and the stamped commit is not in this clone (or
    /// the function cannot be found there), so which part changed is unknown.
    Changed,
    /// The file no longer contains a function the path names.
    FunctionNotFound,
    /// The path now names several functions.
    Ambiguous(Vec<String>),
    FileMissing,
    FileDoesNotParse(String),
}

impl std::fmt::Display for VoidReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VoidReason::CodeChanged => write!(f, "code changed"),
            VoidReason::DocChanged => write!(f, "doc comment changed"),
            VoidReason::CodeAndDocChanged => write!(f, "code and doc comment changed"),
            VoidReason::Changed => {
                write!(
                    f,
                    "hash changed (stamped commit not available to say which part)"
                )
            }
            VoidReason::FunctionNotFound => write!(f, "function not found"),
            VoidReason::Ambiguous(m) => write!(f, "path now ambiguous: {}", m.join(", ")),
            VoidReason::FileMissing => write!(f, "file not found"),
            VoidReason::FileDoesNotParse(e) => write!(f, "file does not parse: {e}"),
        }
    }
}

/// The verdict on one stamp.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Valid,
    Void(VoidReason),
    /// Not judged, with the reason: an artifact stamp, whose hashing is not
    /// implemented yet (#743). Neither valid nor a failure; counted apart.
    Unchecked(String),
}

/// One stamp, checked.
#[derive(Debug, Clone, PartialEq)]
pub struct StampCheck {
    /// 0-based position in `stamps.toml`.
    pub index: usize,
    pub stamp: Stamp,
    pub verdict: Verdict,
    /// Where the function is now, when it is found.
    pub now_lines: Option<[u32; 2]>,
    /// `Type::name` / `name` of the function found, when found.
    pub qualname: Option<String>,
}

impl StampCheck {
    /// Void: what makes `stamps-check` fail.
    pub fn is_failure(&self) -> bool {
        matches!(self.verdict, Verdict::Void(_))
    }
}

/// The result of [`check`].
#[derive(Debug, Clone, PartialEq)]
pub struct CheckReport {
    /// Stamps in `stamps.toml`.
    pub recorded: usize,
    /// What the stamps were judged against: `working tree`, or a commit id.
    pub judged_at: String,
    /// `None` for [`Scope::All`]; the resolved `old..new` for a diff.
    pub range: Option<(String, String)>,
    /// The stamps in scope, in file order.
    pub checked: Vec<StampCheck>,
}

impl CheckReport {
    pub fn failures(&self) -> impl Iterator<Item = &StampCheck> {
        self.checked.iter().filter(|c| c.is_failure())
    }
}

/// Re-hash the stamped functions in `scope` (module doc). Never writes.
pub fn check(workspace: &Path, scope: &Scope) -> Result<CheckReport, String> {
    let stamps = load(workspace)?;
    let recorded = stamps.len();
    let (range, judged_at) = match scope {
        Scope::All => (None, "working tree".to_string()),
        Scope::Diff(r) => {
            let (old, new) = git::resolve_range(workspace, r)?;
            (Some((old, new.clone())), new)
        }
    };
    let read = |file: &str| -> Option<String> {
        match &range {
            None => std::fs::read_to_string(workspace.join(file)).ok(),
            Some((_, new)) => git::show(workspace, new, file),
        }
    };
    let in_scope: Vec<bool> = match &range {
        None => vec![true; stamps.len()],
        Some((old, new)) => {
            let hunks = git::diff_hunks(workspace, old, new)?;
            stamps
                .iter()
                .map(|s| {
                    let Some(hs) = hunks.get(&s.file) else {
                        return false;
                    };
                    let Some(func) = &s.function else {
                        // An artifact stamp: any change to its file.
                        return true;
                    };
                    let (_, qual) = split_spec(func).unwrap_or_default();
                    let at = |rev: &str| {
                        git::show(workspace, rev, &s.file).and_then(|t| locate(&t, &qual).ok())
                    };
                    let old_hit = at(old)
                        .is_some_and(|f| hs.iter().any(|h| h.touches_old(f.lines[0], f.lines[1])));
                    let new_hit = at(new)
                        .is_some_and(|f| hs.iter().any(|h| h.touches_new(f.lines[0], f.lines[1])));
                    old_hit || new_hit
                })
                .collect()
        }
    };
    let mut checked = Vec::new();
    for (index, s) in stamps.iter().enumerate() {
        if !in_scope[index] {
            continue;
        }
        let Some(func) = &s.function else {
            checked.push(StampCheck {
                index,
                stamp: s.clone(),
                verdict: Verdict::Unchecked("artifact stamps are not checked yet (#743)".into()),
                now_lines: None,
                qualname: None,
            });
            continue;
        };
        let (_, qual) = split_spec(func)?;
        let (verdict, found) = match read(&s.file) {
            None => (Verdict::Void(VoidReason::FileMissing), None),
            Some(text) => match locate(&text, &qual) {
                Ok(f) if stamp_hash(&f.doc, &f.code) == s.hash => (Verdict::Valid, Some(f)),
                Ok(f) => (Verdict::Void(why_changed(workspace, s, &qual, &f)), Some(f)),
                Err(LocateError::NotFound) => (Verdict::Void(VoidReason::FunctionNotFound), None),
                Err(LocateError::Ambiguous(m)) => (Verdict::Void(VoidReason::Ambiguous(m)), None),
                Err(LocateError::Parse(e)) => {
                    (Verdict::Void(VoidReason::FileDoesNotParse(e)), None)
                }
            },
        };
        checked.push(StampCheck {
            index,
            stamp: s.clone(),
            verdict,
            now_lines: found.as_ref().map(|f| f.lines),
            qualname: found.as_ref().map(FnEntry::qualname),
        });
    }
    Ok(CheckReport {
        recorded,
        judged_at,
        range,
        checked,
    })
}

/// Which part changed: the function as stamped (at its commit) against now.
fn why_changed(workspace: &Path, s: &Stamp, qual: &str, now: &FnEntry) -> VoidReason {
    let Some(then) = git::show(workspace, &s.commit, &s.file).and_then(|t| locate(&t, qual).ok())
    else {
        return VoidReason::Changed;
    };
    if stamp_hash(&then.doc, &then.code) != s.hash {
        return VoidReason::Changed;
    }
    match (then.code != now.code, then.doc != now.doc) {
        (true, true) => VoidReason::CodeAndDocChanged,
        (true, false) => VoidReason::CodeChanged,
        (false, true) => VoidReason::DocChanged,
        (false, false) => VoidReason::Changed,
    }
}

/// The report `kovan-cli stamps-check` prints: one line per stamp in scope,
/// then a summary line. Deterministic for a given repository state.
pub fn render_report(r: &CheckReport, repo_url: &str) -> String {
    let mut out = String::new();
    let scope = match &r.range {
        None => "all stamps".to_string(),
        Some((a, b)) => format!("stamps touched by {}..{}", short(a), short(b)),
    };
    out.push_str(&format!(
        "stamps-check: {scope}, judged at {}: {} in scope of {} recorded\n",
        if r.range.is_some() {
            short(&r.judged_at)
        } else {
            &r.judged_at
        },
        r.checked.len(),
        r.recorded
    ));
    for c in &r.checked {
        let s = &c.stamp;
        let head = format!(
            "#{} {} (rung {}, {} {})",
            c.index + 1,
            s.target_name(),
            s.rung,
            s.reviewer,
            s.date
        );
        let now = c
            .now_lines
            .map(|l| {
                if l == s.lines {
                    format!("lines {}-{}", l[0], l[1])
                } else {
                    format!(
                        "now lines {}-{} (stamped {}-{})",
                        l[0], l[1], s.lines[0], s.lines[1]
                    )
                }
            })
            .unwrap_or_default();
        match &c.verdict {
            Verdict::Valid => out.push_str(&format!("VALID  {head}  {now}\n")),
            Verdict::Unchecked(why) => out.push_str(&format!("UNCHECKED {head}  {why}\n")),
            Verdict::Void(why) => {
                out.push_str(&format!("VOID   {head}  {why}"));
                if !now.is_empty() {
                    out.push_str(&format!("  {now}"));
                }
                out.push_str(&format!("\n       stamped: {}\n", s.permalink(repo_url)));
            }
        }
    }
    let fails = r.failures().count();
    let unchecked = r
        .checked
        .iter()
        .filter(|c| matches!(c.verdict, Verdict::Unchecked(_)))
        .count();
    let valid = r
        .checked
        .iter()
        .filter(|c| c.verdict == Verdict::Valid)
        .count();
    out.push_str(&format!(
        "{valid} valid, {fails} void, {unchecked} unchecked\n"
    ));
    out
}

fn short(sha: &str) -> &str {
    &sha[..sha.len().min(10)]
}

#[cfg(test)]
mod tests;
