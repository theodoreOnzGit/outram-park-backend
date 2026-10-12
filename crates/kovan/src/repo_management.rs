//! **Repository management** for a Kovan folder (GitHub #840, #841;
//! maintainer, 2026-10-12): add a repository of any kind as a git submodule,
//! "as long as they are kovan compatible", and push a code repository only
//! by an explicit, named action.
//!
//! ```text
//!  add_repository(kovan folder, kind, name, remote, path)
//!     │ 1. the entry is valid beside the others (corpus_tiers::validate)
//!     │ 2. proprietary: the remote is not public (corpus_tiers guards)
//!     │ 3. probe the remote: is it kovan compatible for this kind?
//!     │ 4. mount it (corpus_repos::ensure_corpus: a submodule when the
//!     │    Kovan folder is a git repository)
//!     │ 5. record it in kovan_root.toml ([[repos.open|proprietary|code]])
//!     v
//!  AddedRepository
//!
//!  force_add_commit_push(code repository)   "Force add, commit and push"
//!     git add -A; git commit; push the current branch (never main, never
//!     a force push)
//! ```
//!
//! # The three kinds
//!
//! | kind | `kovan_root.toml` | kovan compatible means | Save Repository |
//! |---|---|---|---|
//! | open corpus | `[[repos.open]]` | not a Rust code repository | commits and pushes it |
//! | proprietary corpus | `[[repos.proprietary]]` | not a Rust code repository; remote not public | commits and pushes it |
//! | code | `[[repos.code]]` | a `Cargo.toml` at its root (Rust only for now) | **never touches it** |
//!
//! Several of each kind are allowed. The standard corpus is Kovan's own and
//! is not added here.
//!
//! # A code repository is never saved by Save Repository
//!
//! Maintainer, 2026-10-12: "save repository when code repos are submodules
//! should never push it. Rather, name it 'force add, commit and push' to
//! let the user know what they are doing. Warning them." A code repository
//! holds source code, and committing everything in it is rarely what a
//! person saving their literature means. [`force_add_commit_push`] is the
//! only way this module commits one; the GUI asks first.
//!
//! "Force" names the act (everything changed is added, without picking
//! files), not a git flag: ignored files stay ignored (`git add -A`, never
//! `--force`, which would add build output), and the push is the
//! fast-forward-or-merge push every other kovan push uses (never a force
//! push, GitHub #502). `main` is refused, as for kovan's review commits.
//!
//! # The probe
//!
//! Compatibility is checked **before** anything is mounted, on a
//! throwaway clone of the remote that fetches no file contents (`--depth 1
//! --no-checkout --filter=blob:none`) and is deleted afterwards, so an
//! incompatible repository never becomes a submodule that then has to be
//! removed.
//!
//! Plain functions, no egui; they run git over the network: call them off
//! the UI thread.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::corpus_repos::{ensure_corpus, RepoState};
use crate::corpus_tiers::{self, CorpusRepoConfig};
use crate::root::KovanRoot;
use crate::save_push::safe_push::{push_keeping_local, BranchRule, SafePushOk};

/// What a repository added here is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoKind {
    OpenCorpus,
    ProprietaryCorpus,
    /// A Rust source code repository.
    Code,
}

impl RepoKind {
    pub const ALL: [RepoKind; 3] = [RepoKind::OpenCorpus, RepoKind::ProprietaryCorpus, RepoKind::Code];

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenCorpus => "Open corpus",
            Self::ProprietaryCorpus => "Proprietary corpus",
            Self::Code => "Code repository (Rust)",
        }
    }

    /// The folder of the Kovan folder a new repository of this kind is
    /// mounted under.
    fn parent(self) -> &'static str {
        match self {
            Self::OpenCorpus | Self::ProprietaryCorpus => "literature",
            Self::Code => "code",
        }
    }
}

/// A repository to add.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewRepository {
    pub kind: RepoKind,
    /// Unique among the folder's repositories; letters, digits, `-`, `_`, `.`.
    pub name: String,
    /// The remote URL (bare: never a credential or token).
    pub remote: String,
    /// Where to mount it, relative to the Kovan folder; `None` is
    /// [`default_path`].
    pub path: Option<PathBuf>,
    pub branch: Option<String>,
}

/// `literature/<name>` for a corpus, `code/<name>` for a code repository.
pub fn default_path(kind: RepoKind, name: &str) -> PathBuf {
    Path::new(kind.parent()).join(name)
}

/// A name for a repository from its remote URL: the last path segment
/// without `.git` (`https://host/me/my-code.git` -> `my-code`), with any
/// character a name may not hold replaced by `-`.
pub fn name_from_remote(remote: &str) -> String {
    let last = remote
        .trim()
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or("");
    last.strip_suffix(".git")
        .unwrap_or(last)
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// What was added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedRepository {
    pub kind: RepoKind,
    pub name: String,
    /// Relative to the Kovan folder.
    pub path: PathBuf,
    /// How it was mounted (a submodule when the Kovan folder is a git
    /// repository).
    pub state: RepoState,
    /// Things the user should know (a code repository with no
    /// `kovan_root.toml` yet, say).
    pub notes: Vec<String>,
}

fn run(dir: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    // The tests' stand-in remotes are local paths.
    #[cfg(test)]
    cmd.args(["-c", "protocol.file.allow=always"]);
    let out = cmd
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// The names at the top of `remote`'s default (or `branch`'s) tree, from a
/// throwaway clone that fetches no file contents (module doc, "The probe").
/// An empty repository has none.
pub fn remote_top_level(remote: &str, branch: Option<&str>) -> Result<Vec<String>, String> {
    // A folder of our own under the system temporary folder, removed when
    // done (the `tempfile` crate is a dev-dependency only).
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp = std::env::temp_dir().join(format!("kovan-probe-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
    let dest = tmp.join("probe");
    let dest_str = dest.to_string_lossy().into_owned();
    let mut args = vec!["clone", "-q", "--depth", "1", "--no-checkout", "--filter=blob:none"];
    if let Some(b) = branch {
        args.extend(["-b", b]);
    }
    args.extend([remote, dest_str.as_str()]);
    let cloned = run(&tmp, &args).map_err(|e| format!("{remote} could not be read: {e}"));
    // An empty repository has no HEAD commit: no names.
    let names = cloned.map(|_| run(&dest, &["ls-tree", "--name-only", "HEAD"]).unwrap_or_default());
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(names?.lines().map(str::to_string).collect())
}

/// Whether a repository with `top` at its root is kovan compatible as
/// `kind` (module doc, "The three kinds"); the notes on success.
pub fn compatible(kind: RepoKind, top: &[String]) -> Result<Vec<String>, String> {
    let has = |name: &str| top.iter().any(|t| t == name);
    match kind {
        RepoKind::Code if !has("Cargo.toml") => Err(
            "not kovan compatible as a code repository: it has no Cargo.toml at its root (Rust \
             repositories only, for now)"
                .into(),
        ),
        RepoKind::Code if !has(crate::stamping::ROOT_FILE) => Ok(vec![format!(
            "It has no {} yet: open it in Code Map and run \"Index fresh\" before reviewing.",
            crate::stamping::ROOT_FILE
        )]),
        RepoKind::Code => Ok(Vec::new()),
        RepoKind::OpenCorpus | RepoKind::ProprietaryCorpus if has("Cargo.toml") => Err(
            "not kovan compatible as a corpus: it is a Rust code repository (a Cargo.toml at its \
             root). Add it as a code repository instead"
                .into(),
        ),
        RepoKind::OpenCorpus | RepoKind::ProprietaryCorpus => Ok(Vec::new()),
    }
}

/// Add `new` to the Kovan folder at `root_dir` (module doc). Nothing is
/// mounted or recorded unless every check passes; a mount that fails leaves
/// `kovan_root.toml` unchanged.
pub fn add_repository(root_dir: &Path, new: &NewRepository) -> Result<AddedRepository, String> {
    let mut root = KovanRoot::open(root_dir).map_err(|e| e.to_string())?;
    let remote = new.remote.trim();
    if remote.is_empty() {
        return Err("a repository is added from its remote URL".into());
    }
    let name = new.name.trim().to_string();
    let path = new
        .path
        .clone()
        .unwrap_or_else(|| default_path(new.kind, &name));
    let entry = CorpusRepoConfig {
        name: name.clone(),
        path: path.clone(),
        remote: Some(remote.to_string()),
        branch: new.branch.clone().filter(|b| !b.trim().is_empty()),
        default: false,
        writable: false,
    };
    // 1. Valid beside the others.
    let mut config = root.config().clone();
    match new.kind {
        RepoKind::OpenCorpus => config.repos.open.push(entry),
        RepoKind::ProprietaryCorpus => config.repos.proprietary.push(entry),
        RepoKind::Code => config.repos.code.push(entry),
    }
    let problems = corpus_tiers::validate(&config);
    if !problems.is_empty() {
        return Err(problems.join("; "));
    }
    // 2. A proprietary corpus never goes to a public remote.
    if new.kind == RepoKind::ProprietaryCorpus {
        let all = root.corpus_repos();
        if let Some(why) = corpus_tiers::public_remote_reason(root.config(), &all, remote) {
            return Err(format!("{remote} cannot hold a proprietary corpus: {why}"));
        }
        if corpus_tiers::anonymously_readable(remote) == Some(true) {
            return Err(format!(
                "{remote} cannot hold a proprietary corpus: it can be read without credentials \
                 (a public repository)"
            ));
        }
    }
    // 3. Kovan compatible?
    let top = remote_top_level(remote, new.branch.as_deref())?;
    let notes = compatible(new.kind, &top)?;
    // 4. Mount, 5. record.
    let dir = root.path().join(&path);
    let state = ensure_corpus(&root, &dir, Some(remote), new.branch.as_deref())
        .map_err(|e| e.to_string())?;
    root.set_repos(config.repos)?;
    Ok(AddedRepository {
        kind: new.kind,
        name,
        path,
        state,
        notes,
    })
}

/// What [`force_add_commit_push`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForcePushed {
    pub branch: String,
    pub remote: String,
    /// The commit made; `None` when there was nothing to commit and only
    /// the push ran.
    pub commit: Option<String>,
    /// The remote had moved on and its tip was merged in first.
    pub merged: Option<String>,
}

/// What the confirmation prompt shows before [`force_add_commit_push`]:
/// the branch and the files `git add -A` would stage (`git status
/// --porcelain`, one line each).
pub fn force_preview(code_dir: &Path) -> Result<(String, Vec<String>), String> {
    let branch = run(code_dir, &["symbolic-ref", "-q", "--short", "HEAD"])
        .map(|b| b.trim().to_string())
        .map_err(|_| "the repository is on no branch (a detached HEAD): check out a branch first".to_string())?;
    let status = run(code_dir, &["status", "--porcelain", "--untracked-files=all"])?;
    Ok((branch, status.lines().map(str::to_string).collect()))
}

/// **Force add, commit and push** the code repository at `code_dir` (module
/// doc): `git add -A`, commit with `message`, push the current branch to
/// its remote. Refused on a detached `HEAD` and on `main`, before anything
/// is staged. Never a force push.
pub fn force_add_commit_push(code_dir: &Path, message: &str) -> Result<ForcePushed, String> {
    let (branch, changes) = force_preview(code_dir)?;
    if branch == "main" {
        return Err(
            "the repository is on main: kovan never commits and pushes main. Check out another \
             branch first"
                .into(),
        );
    }
    let remote = run(code_dir, &["config", "--get", &format!("branch.{branch}.remote")])
        .ok()
        .map(|r| r.trim().to_string())
        .filter(|r| !r.is_empty() && r != ".")
        .unwrap_or_else(|| "origin".to_string());
    let commit = if changes.is_empty() {
        None
    } else {
        let message = if message.trim().is_empty() {
            "kovan: force add, commit and push"
        } else {
            message.trim()
        };
        run(code_dir, &["add", "-A"])?;
        run(code_dir, &["commit", "-q", "-m", message])?;
        Some(run(code_dir, &["rev-parse", "HEAD"])?.trim().to_string())
    };
    match push_keeping_local(code_dir, &remote, &branch, BranchRule::NeverMain) {
        Ok(ok) => Ok(ForcePushed {
            branch,
            remote,
            commit,
            merged: match ok {
                SafePushOk::Pushed { merged } => merged,
                SafePushOk::UpToDate => None,
            },
        }),
        Err(e) => Err(format!(
            "{}the push did not go through: {e:?}",
            if commit.is_some() { "committed locally, but " } else { "" }
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::root::RootConfig;

    fn g(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(["-c", "protocol.file.allow=always"])
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    /// A bare repository standing in for a remote, holding `files` on
    /// branch `branch`.
    fn remote(base: &Path, name: &str, branch: &str, files: &[(&str, &str)]) -> String {
        let work = base.join(format!("{name}-work"));
        std::fs::create_dir_all(&work).unwrap();
        g(&work, &["init", "-q", "-b", branch]);
        for (f, text) in files {
            let p = work.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        }
        g(&work, &["add", "-A"]);
        g(&work, &["commit", "-q", "-m", "first"]);
        let bare = base.join(format!("{name}.git"));
        g(base, &["clone", "-q", "--bare", work.to_str().unwrap(), bare.to_str().unwrap()]);
        bare.to_string_lossy().into_owned()
    }

    fn kovan_folder(base: &Path) -> PathBuf {
        let dir = base.join("kovan");
        std::fs::create_dir_all(&dir).unwrap();
        KovanRoot::create(&dir, RootConfig::new("t", "t"), true).unwrap();
        dir
    }

    fn new(kind: RepoKind, name: &str, remote: &str) -> NewRepository {
        NewRepository {
            kind,
            name: name.into(),
            remote: remote.into(),
            path: None,
            branch: None,
        }
    }

    /// Methodology: a Kovan folder (a git repository) in a temporary
    /// directory, and three local bare repositories standing in for
    /// remotes: a Rust code repository (a `Cargo.toml` at its root), a
    /// second one, and a corpus (a PDF, no `Cargo.toml`). Add them in every
    /// kind. `KOVAN_SKIP_VISIBILITY_PROBE` is not needed: the remotes are
    /// local paths, which the visibility probe does not apply to.
    ///
    /// Pass: the code repository is mounted at `code/<name>` as a submodule
    /// and recorded under `[[repos.code]]`, with a note that it has no
    /// `kovan_root.toml`; a second code repository is added beside it;
    /// the corpus is refused as code and the code repository refused as a
    /// corpus, each naming the reason, with nothing mounted or recorded;
    /// the corpus is added as open under `literature/`, and then refused as
    /// proprietary because its remote is by then an open repository's (so
    /// public); a duplicate name is refused. The code repositories are not among the
    /// corpus repositories Save Repository loops over.
    ///
    /// Result (2026-10-12): passes.
    #[test]
    fn repositories_of_every_kind_are_added_when_compatible() {
        let base = tempfile::tempdir().unwrap();
        let folder = kovan_folder(base.path());
        let code = remote(base.path(), "rust-a", "develop", &[("Cargo.toml", "[workspace]\n"), ("src/lib.rs", "")]);
        let code2 = remote(base.path(), "rust-b", "develop", &[("Cargo.toml", "[package]\n")]);
        let corpus = remote(base.path(), "papers", "main", &[("a-open-corpus/a.pdf", "%PDF")]);

        let e = add_repository(&folder, &new(RepoKind::Code, "papers", &corpus)).unwrap_err();
        assert!(e.contains("no Cargo.toml"), "{e}");
        let e = add_repository(&folder, &new(RepoKind::OpenCorpus, "rust-a", &code)).unwrap_err();
        assert!(e.contains("Rust code repository"), "{e}");
        let root = KovanRoot::open(&folder).unwrap();
        assert!(root.code_repos().is_empty() && root.config().repos.is_empty());
        assert!(!folder.join("code").exists() && !folder.join("literature/rust-a/.git").exists());

        let added = add_repository(&folder, &new(RepoKind::Code, "rust-a", &code)).unwrap();
        assert_eq!(added.path, Path::new("code/rust-a"));
        assert_eq!(added.state, RepoState::SubmoduleAdded);
        assert!(added.notes.iter().any(|n| n.contains("kovan_root.toml")), "{:?}", added.notes);
        assert!(folder.join("code/rust-a/Cargo.toml").is_file());
        add_repository(&folder, &new(RepoKind::Code, "rust-b", &code2)).unwrap();
        let e = add_repository(&folder, &new(RepoKind::Code, "rust-a", &code2)).unwrap_err();
        assert!(e.contains("already used"), "{e}");

        add_repository(&folder, &new(RepoKind::OpenCorpus, "papers", &corpus)).unwrap();
        let e = add_repository(&folder, &new(RepoKind::ProprietaryCorpus, "private-papers", &corpus)).unwrap_err();
        assert!(e.contains("cannot hold a proprietary corpus"), "{e}");
        let root = KovanRoot::open(&folder).unwrap();
        let names: Vec<String> = root.code_repos().into_iter().map(|c| c.name).collect();
        assert_eq!(names, ["rust-a", "rust-b"]);
        assert!(root.code_repos().iter().all(|c| c.is_downloaded()));
        assert!(folder.join("literature/papers/a-open-corpus/a.pdf").is_file());
        let corpora: Vec<String> = root.corpus_repos().into_iter().map(|r| r.name).collect();
        assert!(corpora.contains(&"papers".to_string()), "{corpora:?}");
        assert!(!corpora.iter().any(|n| n.starts_with("rust-")), "Save never sees a code repository: {corpora:?}");
        let text = std::fs::read_to_string(folder.join("kovan_root.toml")).unwrap();
        assert!(text.contains("[[repos.code]]"), "{text}");
    }

    /// Methodology: a clone of a local bare code repository on `develop`.
    /// Edit a tracked file, add an untracked one and an ignored one, preview,
    /// then force add, commit and push. Repeat on `main`, and on a detached
    /// `HEAD`.
    ///
    /// Pass: the preview names the branch and the two changes, never the
    /// ignored file; the push lands on the remote's `develop` with both
    /// files and without the ignored one; with nothing changed it only
    /// pushes; `main` and a detached `HEAD` are refused before anything is
    /// staged.
    ///
    /// Result (2026-10-12): passes.
    #[test]
    fn force_add_commit_push_takes_everything_but_ignored_files_and_never_main() {
        let base = tempfile::tempdir().unwrap();
        let url = remote(
            base.path(),
            "rust-a",
            "develop",
            &[("Cargo.toml", "[workspace]\n"), (".gitignore", "target/\n")],
        );
        let clone = base.path().join("clone");
        g(base.path(), &["clone", "-q", &url, clone.to_str().unwrap()]);
        g(&clone, &["config", "user.name", "t"]);
        g(&clone, &["config", "user.email", "t@t"]);
        g(&clone, &["config", "commit.gpgsign", "false"]);
        std::fs::write(clone.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        std::fs::write(clone.join("new.rs"), "fn f() {}\n").unwrap();
        std::fs::create_dir_all(clone.join("target")).unwrap();
        std::fs::write(clone.join("target/built"), "x").unwrap();

        let (branch, changes) = force_preview(&clone).unwrap();
        assert_eq!(branch, "develop");
        assert_eq!(changes.len(), 2, "{changes:?}");
        assert!(!changes.iter().any(|c| c.contains("target")));

        let done = force_add_commit_push(&clone, "review prep").unwrap();
        assert_eq!((done.branch.as_str(), done.remote.as_str()), ("develop", "origin"));
        let tip = g(Path::new(&url), &["rev-parse", "develop"]);
        assert_eq!(Some(tip.trim()), done.commit.as_deref());
        let files = g(Path::new(&url), &["ls-tree", "-r", "--name-only", "develop"]);
        assert!(files.contains("new.rs") && !files.contains("target"), "{files}");
        let again = force_add_commit_push(&clone, "").unwrap();
        assert_eq!(again.commit, None);

        g(&clone, &["checkout", "-q", "-b", "main"]);
        std::fs::write(clone.join("new.rs"), "fn g() {}\n").unwrap();
        let e = force_add_commit_push(&clone, "x").unwrap_err();
        assert!(e.contains("never commits and pushes main"), "{e}");
        assert!(g(&clone, &["diff", "--cached", "--name-only"]).trim().is_empty(), "nothing staged");
        g(&clone, &["checkout", "-q", "--detach"]);
        let e = force_add_commit_push(&clone, "x").unwrap_err();
        assert!(e.contains("detached"), "{e}");
    }

    /// The name suggested from a remote URL, in the forms a user pastes.
    #[test]
    fn a_name_is_suggested_from_the_remote() {
        assert_eq!(name_from_remote("https://github.com/me/my-code.git"), "my-code");
        assert_eq!(name_from_remote("git@github.com:me/my_code.git"), "my_code");
        assert_eq!(name_from_remote("https://host/me/papers/"), "papers");
        assert_eq!(name_from_remote("/tmp/some repo.git"), "some-repo");
        assert_eq!(default_path(RepoKind::Code, "x"), Path::new("code/x"));
        assert_eq!(default_path(RepoKind::OpenCorpus, "x"), Path::new("literature/x"));
    }
}
