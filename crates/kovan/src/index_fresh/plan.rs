//! The plan shown before "Index fresh" runs ([`plan_fresh`]: which files
//! will be created, overwritten or kept, and the rough cost), and the run
//! itself ([`run_fresh`]). Both do I/O (cargo metadata, file scans, git, the
//! keystore) and run on a worker thread in the app.

use std::path::{Path, PathBuf};

use crate::commands::index::{self, IndexCmdError, IndexOptions, IndexSummary, KOVAN_TOML};
use crate::commands::index_control::RunControl;
use kovan_common::code_index::folders::parent;
use kovan_common::code_index::links::LINKS_FILE;

use super::detect::{detect_project, DetectError, ProjectKind};
use super::root_file::{
    inspect_root, keystore_founders, last_good_committed_root, settle_root, CommittedRoot,
    CorruptAction, FounderChoice, RootOutcome, RootState, ROOT_FILE,
};
use super::skeleton::{create_missing_skeletons, missing_review_mds, SkeletonReport};

/// What will happen to one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    /// Does not exist; will be written.
    Create,
    /// Exists; regenerated when it differs (a disposable cache).
    Regenerate,
    /// Exists; left exactly as it is.
    Keep,
    /// Corrupt; kept aside as `.corrupt-<date>` once the user chooses.
    SetAside,
}

impl FileAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Regenerate => "overwrite if changed (cache)",
            Self::Keep => "keep as is",
            Self::SetAside => "keep aside as .corrupt-<date>",
        }
    }
}

/// One file of the plan, workspace-relative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub path: String,
    pub action: FileAction,
}

/// The rough cost, measured from the target's code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostEstimate {
    pub crates: usize,
    pub rs_files: usize,
    pub rs_lines: usize,
    pub message: String,
}

/// The cost message (#780: scaled honestly or said to be unknown). The
/// only measured reference is outram-park-backend's own run.
pub fn cost_message(crates: usize, rs_files: usize, rs_lines: usize) -> String {
    format!(
        "This target: {crates} crate(s), {rs_files} indexed .rs file(s), {rs_lines} lines. For scale, \
         the one measured run, outram-park-backend (48 crates), took about 4\u{2013}7 minutes with a \
         15\u{2013}16 GB memory peak in rust-analyzer. The cost grows with the code AND its dependencies \
         (rust-analyzer loads those too, and builds build scripts and proc macros first), so this \
         target's time and memory are not known in advance; a small crate is usually far quicker. \
         rust-analyzer runs at low priority (nice) and can be cancelled."
    )
}

/// Everything the confirm dialog shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshPlan {
    pub dir: PathBuf,
    pub kind: ProjectKind,
    /// (name, workspace-relative folder) of every member.
    pub crates: Vec<(String, String)>,
    pub root: RootState,
    /// For a corrupt root: the newest committed version that parses, or
    /// why there is none to offer.
    pub restore: Result<Option<CommittedRoot>, String>,
    pub founders: FounderChoice,
    /// The installed rust-analyzer, `None` when missing (the run is refused).
    pub rust_analyzer: Option<String>,
    pub files: Vec<PlannedFile>,
    pub cost: CostEstimate,
}

/// Build the plan for `dir` (no file is written).
pub fn plan_fresh(dir: &Path) -> Result<FreshPlan, String> {
    let dir = std::fs::canonicalize(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let kind = detect_project(&dir).map_err(|e| e.to_string())?;
    let crates = crate::commands::call_graph::member_dirs(&dir)?;
    let root = inspect_root(&dir);
    let restore = match &root {
        RootState::Corrupt { .. } => last_good_committed_root(&dir),
        _ => Ok(None),
    };
    let mut files = vec![PlannedFile {
        path: ROOT_FILE.into(),
        action: match &root {
            RootState::Missing => FileAction::Create,
            RootState::Valid { .. } => FileAction::Keep,
            RootState::Corrupt { .. } => FileAction::SetAside,
        },
    }];
    let exists = |rel: &str| dir.join(rel).exists();
    let (mut rs_files, mut rs_lines) = (0usize, 0usize);
    let mut folders = std::collections::BTreeSet::new();
    for m in &crates {
        let links = if m.1.is_empty() {
            LINKS_FILE.to_string()
        } else {
            format!("{}/{LINKS_FILE}", m.1)
        };
        let action = if exists(&links) {
            FileAction::Regenerate
        } else {
            FileAction::Create
        };
        files.push(PlannedFile {
            path: links,
            action,
        });
        for f in index::code_files(&dir, &crates, m) {
            rs_files += 1;
            rs_lines += std::fs::read_to_string(dir.join(&f))
                .map(|t| t.lines().count())
                .unwrap_or(0);
            folders.insert(parent(&f).to_string());
        }
    }
    let folders: Vec<String> = folders.into_iter().collect();
    for d in &folders {
        let rel = if d.is_empty() {
            KOVAN_TOML.to_string()
        } else {
            format!("{d}/{KOVAN_TOML}")
        };
        let action = if exists(&rel) {
            FileAction::Regenerate
        } else {
            FileAction::Create
        };
        files.push(PlannedFile { path: rel, action });
    }
    let missing = missing_review_mds(&dir, &folders);
    for d in &folders {
        let rel = if d.is_empty() {
            "review.md".to_string()
        } else {
            format!("{d}/review.md")
        };
        let action = if missing.contains(&rel) {
            FileAction::Create
        } else {
            FileAction::Keep
        };
        files.push(PlannedFile { path: rel, action });
    }
    let n = crates.len();
    Ok(FreshPlan {
        dir,
        kind,
        crates,
        root,
        restore,
        founders: keystore_founders(),
        rust_analyzer: index::installed_rust_analyzer(),
        files,
        cost: CostEstimate {
            crates: n,
            rs_files,
            rs_lines,
            message: cost_message(n, rs_files, rs_lines),
        },
    })
}

/// The user's choices from the confirm dialog (or the CLI flags).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FreshChoices {
    /// The founder for a new root; `None` leaves it UNSET.
    pub founder: Option<String>,
    /// Required when the root is corrupt.
    pub corrupt: Option<CorruptAction>,
    /// Use this SCIP index instead of running rust-analyzer (tests, CI).
    pub scip: Option<PathBuf>,
}

/// What a fresh index did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreshReport {
    pub dir: PathBuf,
    pub kind: ProjectKind,
    pub index: IndexSummary,
    pub root: RootOutcome,
    pub skeletons: SkeletonReport,
}

impl FreshReport {
    /// The report as lines, for the CLI and the app.
    pub fn lines(&self) -> Vec<String> {
        let s = &self.skeletons;
        let mut v = vec![
            format!("{} ({})", self.dir.display(), self.kind.label()),
            format!(
                "index: {} folder(s) indexed, {} file(s) written, {} unchanged",
                self.index.folders.len(),
                self.index.written,
                self.index.unchanged
            ),
            self.root.describe(),
            format!(
                "review.md: {} skeleton(s) created, {} kept as they were",
                s.created.len(),
                s.kept.len()
            ),
        ];
        for (p, n) in &s.unreadable {
            v.push(format!("review.md: {p} has {n} unreadable entr(y/ies): they count as no review; redo them in the review wizard"));
        }
        v.push("Nothing was committed: review the new files and commit them yourself.".into());
        v
    }
}

/// Why a fresh index stopped. Nothing was written unless the variant says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreshError {
    Detect(DetectError),
    /// The root is corrupt and no [`CorruptAction`] was chosen.
    CorruptRoot {
        error: String,
    },
    /// The index run failed (rust-analyzer missing, SCIP failed, cancelled).
    Index(IndexCmdError),
    /// After the index was written: settling the root or the skeletons.
    AfterIndex(String),
}

impl std::fmt::Display for FreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Detect(e) => write!(f, "{e}"),
            Self::CorruptRoot { error } => write!(
                f,
                "{ROOT_FILE} is corrupt and was not touched: {error}. Choose to restore the last committed \
                 version that parses, or to start fresh (the bad file is kept as {ROOT_FILE}.corrupt-<date>)."
            ),
            Self::Index(e) => write!(f, "{e}"),
            Self::AfterIndex(e) => write!(f, "the index was written, then: {e}"),
        }
    }
}

impl std::error::Error for FreshError {}

/// Run the fresh index of `dir` (module doc of [`super`]). Order, so that a
/// failure leaves the last good state: the checks (project, root, a
/// restore that must parse) write nothing; the index run computes
/// everything before it writes, and a missing rust-analyzer, a failed SCIP
/// run or a cancel stops it with nothing written; then the root, then the
/// skeletons.
pub fn run_fresh(
    dir: &Path,
    choices: &FreshChoices,
    ctl: &RunControl,
) -> Result<FreshReport, FreshError> {
    ctl.phase("checking the target", 0);
    let dir = std::fs::canonicalize(dir)
        .map_err(|e| FreshError::Index(IndexCmdError::Other(format!("{}: {e}", dir.display()))))?;
    let kind = detect_project(&dir).map_err(FreshError::Detect)?;
    ctl.say(format!(
        "index-fresh: {} is a {}",
        dir.display(),
        kind.label()
    ));
    if let RootState::Corrupt { error, .. } = inspect_root(&dir) {
        match &choices.corrupt {
            None => return Err(FreshError::CorruptRoot { error }),
            Some(CorruptAction::Restore { commit }) => {
                super::root_file::committed_root_at(&dir, commit)
                    .map_err(|e| FreshError::Index(IndexCmdError::Other(e)))?;
            }
            Some(CorruptAction::StartFresh) => {}
        }
    }
    let rust_analyzer = index::installed_rust_analyzer();
    let opts = IndexOptions {
        scip: choices.scip.clone(),
        ..IndexOptions::default()
    };
    let summary = index::run_controlled(&dir, &opts, ctl).map_err(FreshError::Index)?;
    ctl.phase("settling kovan_root.toml", 0);
    let date = super::today();
    let root = settle_root(
        &dir,
        choices.corrupt.as_ref(),
        choices.founder.as_deref(),
        rust_analyzer.as_deref(),
        &date,
    )
    .map_err(FreshError::AfterIndex)?;
    ctl.say(format!("index-fresh: {}", root.describe()));
    ctl.phase(
        "creating missing review.md skeletons",
        summary.folders.len(),
    );
    let created = crate::digitiser::dataset::utc_now_iso8601();
    let skeletons = create_missing_skeletons(&dir, &summary.folders, &created)
        .map_err(FreshError::AfterIndex)?;
    let report = FreshReport {
        dir,
        kind,
        index: summary,
        root,
        skeletons,
    };
    for l in report.lines() {
        ctl.say(format!("index-fresh: {l}"));
    }
    ctl.phase("done", 0);
    Ok(report)
}
