//! The target's `kovan_root.toml` under **Leak Before Break**
//! (`docs/kovan.md`; maintainer, #780): it holds keys and an append-only
//! history, so it is never silently overwritten.
//!
//! | state on disk | what happens |
//! |---|---|
//! | missing | a fresh one is written ([`fresh_root_text`]) |
//! | valid ([`ReviewRoot::parse`] reads it) | kept, byte for byte; a rust-analyzer pin that differs is only reported |
//! | corrupt | nothing, until the user chooses a [`CorruptAction`]; then the bad file is kept as `kovan_root.toml.corrupt-<date>` ([`quarantine`]) and the last committed version that parses is restored ([`last_good_committed_root`]) or a fresh one written |
//!
//! A fresh root is a literature-library marker (`schema_version`,
//! `[library]`, from [`crate::root::RootConfig`], so the app can open the
//! folder) with the code-review sections written by the existing
//! [`ReviewRoot::write_into`]: `[code_review]` with the founder and the
//! rust-analyzer version, and no `[[reviewer]]` (an empty registry, so an
//! empty key history). A founder that is not known is left out and the
//! header says **UNSET** in words; one is never invented.

use std::path::{Path, PathBuf};
use std::process::Command;

use kovan_common::review::root::{CodeReviewSettings, ReviewRoot};
use kovan_common::review::types::reviewer_id_kind;

use crate::root::RootConfig;

/// The file name.
pub const ROOT_FILE: &str = "kovan_root.toml";

/// What the target holds now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootState {
    Missing,
    /// Readable: kept as it is.
    Valid {
        text: String,
        rust_analyzer: Option<String>,
    },
    /// Present but unreadable (not TOML, or a review section breaks the
    /// schema), with the parse error to show.
    Corrupt {
        text: String,
        error: String,
    },
}

/// Read and classify `<dir>/kovan_root.toml`.
pub fn inspect_root(dir: &Path) -> RootState {
    let path = dir.join(ROOT_FILE);
    if !path.exists() {
        return RootState::Missing;
    }
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            return RootState::Corrupt {
                text: String::new(),
                error: format!("cannot read it: {e}"),
            }
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(t) => t,
        Err(e) => {
            let text = String::from_utf8_lossy(e.as_bytes()).to_string();
            return RootState::Corrupt {
                text,
                error: format!("not UTF-8: {e}"),
            };
        }
    };
    match ReviewRoot::parse(&text) {
        Ok(r) => RootState::Valid {
            rust_analyzer: r.code_review.and_then(|c| c.rust_analyzer),
            text,
        },
        Err(e) => RootState::Corrupt {
            text,
            error: e.to_string(),
        },
    }
}

/// The founder, from the user's sign-off identity: the reviewer ids that
/// have a key in kovan's keystore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FounderChoice {
    /// No identity: the founder is left UNSET.
    Unset,
    /// Exactly one identity: it is the founder (the user can still unset it).
    One(String),
    /// Several identities: the user picks one, or leaves it UNSET.
    Several(Vec<String>),
}

impl FounderChoice {
    /// From the reviewer ids of the keystore's key files (sorted, deduped,
    /// invalid ids dropped).
    pub fn from_ids(ids: impl IntoIterator<Item = String>) -> FounderChoice {
        let mut v: Vec<String> = ids
            .into_iter()
            .filter(|i| reviewer_id_kind(i).is_ok())
            .collect();
        v.sort();
        v.dedup();
        match v.len() {
            0 => Self::Unset,
            1 => Self::One(v.remove(0)),
            _ => Self::Several(v),
        }
    }

    /// The founder used when the user does not choose.
    pub fn default_founder(&self) -> Option<String> {
        match self {
            Self::One(id) => Some(id.clone()),
            _ => None,
        }
    }

    /// Every id the user may choose.
    pub fn ids(&self) -> Vec<String> {
        match self {
            Self::Unset => Vec::new(),
            Self::One(id) => vec![id.clone()],
            Self::Several(v) => v.clone(),
        }
    }
}

/// The reviewer ids in kovan's default keystore (empty when there is none
/// or it cannot be listed). Reads only the key files' public headers.
/// Under `cargo test` the user's keystore is never read: `Unset`.
pub fn keystore_founders() -> FounderChoice {
    if cfg!(test) {
        return FounderChoice::Unset;
    }
    let ids = kovan_common::review::signing::keystore::Keystore::default_location()
        .and_then(|k| k.list())
        .map(|files| files.iter().map(|f| f.reviewer.clone()).collect::<Vec<_>>())
        .unwrap_or_default();
    FounderChoice::from_ids(ids)
}

/// The text of a fresh `kovan_root.toml` for library `name` (module doc).
/// `founder` must be a valid reviewer id when given.
pub fn fresh_root_text(
    name: &str,
    founder: Option<&str>,
    rust_analyzer: Option<&str>,
    date: &str,
) -> Result<String, String> {
    if let Some(f) = founder {
        reviewer_id_kind(f).map_err(|e| format!("founder {f}: {e}"))?;
    }
    // The library marker without `[paths]`: the conventional literature
    // layout is the default when it is absent, and a code repository has
    // no use for it spelled out.
    let mut base: toml::Table =
        toml::from_str(&RootConfig::new(name, name).to_toml()?).map_err(|e| e.to_string())?;
    base.remove("paths");
    let base = toml::to_string_pretty(&base).map_err(|e| e.to_string())?;
    let review = ReviewRoot {
        code_review: Some(CodeReviewSettings {
            rust_analyzer: rust_analyzer.map(str::to_string),
            founder: founder.map(str::to_string),
        }),
        reviewers: Vec::new(),
        deleted_crates: Vec::new(),
    };
    let body = review.write_into(&base).map_err(|e| e.to_string())?;
    let founder_line = match founder {
        Some(f) => format!("# founder: {f} (the sign-off identity in kovan's keystore)."),
        None => "# founder: UNSET. No sign-off identity was found in kovan's keystore, so none was\n\
                 # written (one is never invented). Set [code_review] founder = \"github:<you>\"\n\
                 # (or gitlab:, orcid:, an email) before anyone stamps; until then no key is trusted."
            .to_string(),
    };
    let ra_line = match rust_analyzer {
        Some(v) => format!("# rust-analyzer: {v}, the version this index was built with ([code_review] rust_analyzer)."),
        None => "# rust-analyzer: not recorded (the index was built from a given SCIP file).".to_string(),
    };
    Ok(format!(
        "# kovan_root.toml: created by kovan \"Index fresh\" on {date} (GitHub #780).\n\
         # It holds this repository's code-review registry: reviewers, their keys and\n\
         # each key's append-only history. kovan never commits it; you do.\n\
         {founder_line}\n\
         # reviewers: none yet (an empty registry, so an empty key history).\n\
         {ra_line}\n\n{body}"
    ))
}

/// The name a corrupt root is kept under: `kovan_root.toml.corrupt-<date>`,
/// then `-2`, `-3`… so an earlier one is never overwritten.
pub fn quarantine_path(dir: &Path, date: &str) -> PathBuf {
    let base = format!("{ROOT_FILE}.corrupt-{date}");
    let mut p = dir.join(&base);
    let mut n = 2;
    while p.exists() {
        p = dir.join(format!("{base}-{n}"));
        n += 1;
    }
    p
}

/// Keep the corrupt root aside (renamed, never deleted). Returns where.
pub fn quarantine(dir: &Path, date: &str) -> Result<PathBuf, String> {
    let to = quarantine_path(dir, date);
    std::fs::rename(dir.join(ROOT_FILE), &to)
        .map_err(|e| format!("keeping {ROOT_FILE} as {}: {e}", to.display()))?;
    Ok(to)
}

/// A committed `kovan_root.toml` that parses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedRoot {
    pub commit: String,
    /// The commit date, `YYYY-MM-DD`.
    pub date: String,
    pub text: String,
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(args)
        .current_dir(dir)
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

/// The newest committed version of `<dir>/kovan_root.toml` that
/// [`ReviewRoot::parse`] reads, walking `git log -- kovan_root.toml`
/// newest first. `Ok(None)`: committed, but no version parses, or never
/// committed. `Err`: not a git repository (the error says so).
pub fn last_good_committed_root(dir: &Path) -> Result<Option<CommittedRoot>, String> {
    git(dir, &["rev-parse", "--git-dir"]).map_err(|e| {
        format!("not a git repository, so there is no history to restore from ({e})")
    })?;
    // A repository with no commit yet has no log: nothing to restore.
    let Ok(log) = git(dir, &["log", "--format=%H %cs", "--", ROOT_FILE]) else {
        return Ok(None);
    };
    for line in log.lines() {
        let Some((commit, date)) = line.trim().split_once(' ') else {
            continue;
        };
        let Ok(text) = git(dir, &["show", &format!("{commit}:./{ROOT_FILE}")]) else {
            continue;
        };
        if ReviewRoot::parse(&text).is_ok() {
            return Ok(Some(CommittedRoot {
                commit: commit.to_string(),
                date: date.to_string(),
                text,
            }));
        }
    }
    Ok(None)
}

/// The committed `kovan_root.toml` of `commit`, if it parses.
pub fn committed_root_at(dir: &Path, commit: &str) -> Result<String, String> {
    let text = git(dir, &["show", &format!("{commit}:./{ROOT_FILE}")])?;
    ReviewRoot::parse(&text)
        .map_err(|e| format!("{ROOT_FILE} at {commit} does not parse either: {e}"))?;
    Ok(text)
}

/// What the user chose for a corrupt root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorruptAction {
    /// Restore the version committed at `commit` (it must parse).
    Restore { commit: String },
    /// Write a fresh root (confirmed by the user).
    StartFresh,
}

/// What happened to `kovan_root.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootOutcome {
    /// There was none; a fresh one was written.
    Created { founder: Option<String> },
    /// A valid one was kept untouched. `pin_differs`: it pins another
    /// rust-analyzer than the one used (reported, not rewritten).
    Kept {
        pinned: Option<String>,
        pin_differs: bool,
    },
    /// A corrupt one was kept aside and the committed version restored.
    Restored { commit: String, kept_as: PathBuf },
    /// A corrupt one was kept aside and a fresh one written.
    Replaced {
        kept_as: PathBuf,
        founder: Option<String>,
    },
}

impl RootOutcome {
    /// One line for the report.
    pub fn describe(&self) -> String {
        match self {
            Self::Created { founder } => format!(
                "kovan_root.toml created (founder {})",
                founder.as_deref().unwrap_or("UNSET")
            ),
            Self::Kept { pinned, pin_differs } => match (pinned, pin_differs) {
                (Some(p), true) => format!(
                    "kovan_root.toml kept as it was; it pins rust-analyzer {p}, which differs from the one used (not changed)"
                ),
                _ => "kovan_root.toml kept as it was".to_string(),
            },
            Self::Restored { commit, kept_as } => format!(
                "kovan_root.toml restored from commit {}; the corrupt file is kept as {}",
                commit.get(..10).unwrap_or(commit),
                kept_as.display()
            ),
            Self::Replaced { kept_as, founder } => format!(
                "a fresh kovan_root.toml written (founder {}); the corrupt file is kept as {}",
                founder.as_deref().unwrap_or("UNSET"),
                kept_as.display()
            ),
        }
    }
}

/// Settle the root (module doc table). `state` is what [`inspect_root`]
/// saw; it is checked again so a file changed meanwhile is not clobbered.
pub fn settle_root(
    dir: &Path,
    action: Option<&CorruptAction>,
    founder: Option<&str>,
    rust_analyzer: Option<&str>,
    date: &str,
) -> Result<RootOutcome, String> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "workspace".to_string());
    match inspect_root(dir) {
        RootState::Missing => {
            let text = fresh_root_text(&name, founder, rust_analyzer, date)?;
            write_new(&dir.join(ROOT_FILE), &text)?;
            Ok(RootOutcome::Created { founder: founder.map(str::to_string) })
        }
        RootState::Valid { rust_analyzer: pinned, .. } => {
            let pin_differs = matches!((&pinned, rust_analyzer), (Some(p), Some(u)) if p != u);
            Ok(RootOutcome::Kept { pinned, pin_differs })
        }
        RootState::Corrupt { error, .. } => match action {
            None => Err(format!(
                "{ROOT_FILE} is corrupt ({error}); it was not touched. Choose: restore the last committed \
                 version that parses, or start fresh (the bad file is kept as {ROOT_FILE}.corrupt-<date>)"
            )),
            Some(CorruptAction::Restore { commit }) => {
                let text = committed_root_at(dir, commit)?;
                let kept_as = quarantine(dir, date)?;
                write_new(&dir.join(ROOT_FILE), &text)?;
                Ok(RootOutcome::Restored { commit: commit.clone(), kept_as })
            }
            Some(CorruptAction::StartFresh) => {
                let text = fresh_root_text(&name, founder, rust_analyzer, date)?;
                let kept_as = quarantine(dir, date)?;
                write_new(&dir.join(ROOT_FILE), &text)?;
                Ok(RootOutcome::Replaced { kept_as, founder: founder.map(str::to_string) })
            }
        },
    }
}

/// Write a file that must not exist yet (never overwrites).
pub fn write_new(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    f.write_all(text.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))
}
