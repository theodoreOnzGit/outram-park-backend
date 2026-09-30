//! Several literature repositories per corpus tier (GitHub issue #458).
//!
//! Maintainer, 2026-09-30: *"kovan should be able to take on multiple
//! standard, multiple open and multiple propreitrary github repos in their
//! corpus"*. Each tier used to be exactly one repository: the standard
//! corpus at [`crate::root::RootPaths::standard_corpus`] from
//! [`crate::corpus::CORPUS_REPOSITORY_URL`], the open corpus at
//! [`crate::root::RootPaths::open_sources`] and the proprietary corpus at
//! [`crate::root::RootPaths::restricted_sources`]. A repository is to be
//! split by topic as it nears ~1 GB (GitHub's recommended size, #454), so
//! each tier now holds any number of them.
//!
//! What belongs here: the `[repos]` table of `kovan_root.toml`
//! ([`RepoTiers`]), resolving it together with the older single-repository
//! settings into one ordered list ([`resolve`], [`crate::root::KovanRoot::corpus_repos`]),
//! checking it ([`validate`]), the "is this remote public?" guard for
//! proprietary pushes ([`public_remote_reason`]), and the size warning
//! ([`size_warning`]). What does not: cloning, committing and pushing, which
//! stay in [`crate::corpus_repos`], [`crate::repository`] and
//! [`crate::save_push`] and now loop over this list.
//!
//! # `kovan_root.toml`
//!
//! ```toml
//! [[repos.standard]]
//! name = "htgr-standard"
//! remote = "https://github.com/example/htgr-standard-corpus.git"
//! path = "literature/standard-htgr"
//! branch = "main"
//!
//! [[repos.open]]
//! name = "open-thermal-hydraulics"
//! remote = "https://github.com/example/open-th.git"
//! path = "literature/open-th"
//! default = true          # where an open ingest goes unless the user picks another
//!
//! [[repos.proprietary]]
//! name = "proprietary-books"
//! remote = "https://github.com/example/private-books.git"
//! path = "literature/proprietary-books"
//!
//! [repos]
//! known_public = ["https://github.com/example/some-public-mirror.git"]
//! # builtin_standard = false   # drop Kovan's built-in standard repository
//! ```
//!
//! Per repository: `name` (unique across every tier; recorded in a paper's
//! `[source] repo`), `path` (relative to the Kovan folder), and optionally
//! `remote`, `branch`, `default` and, for a standard repository only,
//! `writable`.
//!
//! # Backward compatibility (no migration needed)
//!
//! A file without `[repos]` behaves exactly as before. The older settings
//! are each tier's **implicit first repository**:
//!
//! | tier | name | path | remote |
//! |---|---|---|---|
//! | standard | `kovan-standard` ([`BUILTIN_STANDARD_REPOS`]) | `[paths] standard_corpus` | [`crate::corpus::CORPUS_REPOSITORY_URL`] |
//! | open | `open` ([`LEGACY_OPEN`]) | `[paths] open_sources` | `[corpora] open_remote` |
//! | proprietary | `proprietary` ([`LEGACY_PROPRIETARY`]) | `[paths] restricted_sources` | `[corpora] proprietary_remote`, else `[private_submodule] remote` |
//!
//! `[[repos.<tier>]]` entries are appended after it. An entry with the same
//! `path` as the implicit one (or, for standard, the same remote as a
//! built-in) **replaces** it in place, which is how an existing repository
//! is renamed, made the default or given a branch without moving it.
//!
//! # Rules
//!
//! - **Standard repositories are read-only** to Kovan (never committed into
//!   or pushed) unless configured `writable = true`; the corpus maintainer
//!   maintains them. Open and proprietary repositories are the user's own.
//! - **A proprietary repository never shares or nests a path with another
//!   repository** ([`validate`], checked when the folder is opened): a
//!   proprietary PDF inside an open checkout would be published by its push.
//! - **A proprietary repository is never pushed to a public remote**
//!   ([`public_remote_reason`]): not to another tier's remote, not to the
//!   built-in standard corpus, not to a `known_public` URL, and not to an
//!   HTTPS remote that answers `git ls-remote` without credentials.

use crate::root::RootConfig;
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

/// A corpus tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    /// Kovan's standard corpus: the documents compiled into
    /// [`crate::corpus::LITERATURE`], read-only to the user.
    Standard,
    /// The user's openly licensed literature.
    Open,
    /// The user's proprietary literature, in private repositories only.
    Proprietary,
}

impl Tier {
    /// Every tier, in the order they are listed.
    pub const ALL: [Tier; 3] = [Tier::Standard, Tier::Open, Tier::Proprietary];

    /// The label the UI shows, e.g. `Open corpus`.
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standard corpus",
            Self::Open => "Open corpus",
            Self::Proprietary => "Proprietary corpus",
        }
    }

    /// The `[repos]` key, e.g. `open`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Open => "open",
            Self::Proprietary => "proprietary",
        }
    }
}

/// One `[[repos.<tier>]]` entry of `kovan_root.toml`.
///
/// **Bare remote URLs only, never a credential or token**, as every remote in
/// `kovan_root.toml` (`DATA_POLICY.md`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorpusRepoConfig {
    /// Unique across every tier; recorded in a paper's `[source] repo`.
    pub name: String,
    /// Where the checkout is mounted, relative to the Kovan folder.
    pub path: PathBuf,
    /// The repository's remote. Without one the repository is initialised
    /// locally, to be given a remote later.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// The branch to check out and follow; else the remote's default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Whether this is the tier's default ingest target. The first flagged
    /// entry wins; with none flagged, the tier's first repository is the
    /// default.
    #[serde(default, skip_serializing_if = "is_false")]
    pub default: bool,
    /// Standard tier only: let Kovan commit into and push this repository
    /// (for the corpus maintainer). Ignored for open and proprietary
    /// repositories, which are always the user's own.
    #[serde(default, skip_serializing_if = "is_false")]
    pub writable: bool,
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_true(b: &bool) -> bool {
    *b
}

fn default_true() -> bool {
    true
}

/// The `[repos]` table of `kovan_root.toml`. Omitted from the file while
/// empty, so an older file is written back unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RepoTiers {
    /// Keep Kovan's built-in standard repositories ([`BUILTIN_STANDARD_REPOS`])
    /// ahead of any configured ones. On by default.
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub builtin_standard: bool,
    /// Remote URLs known to be public, to which no proprietary repository
    /// may push (in addition to every standard and open remote).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub known_public: Vec<String>,
    /// Extra standard repositories.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub standard: Vec<CorpusRepoConfig>,
    /// Extra open repositories.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub open: Vec<CorpusRepoConfig>,
    /// Extra proprietary repositories.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub proprietary: Vec<CorpusRepoConfig>,
}

impl Default for RepoTiers {
    fn default() -> Self {
        Self {
            builtin_standard: true,
            known_public: Vec::new(),
            standard: Vec::new(),
            open: Vec::new(),
            proprietary: Vec::new(),
        }
    }
}

impl RepoTiers {
    /// Whether the table says nothing (it is then omitted on save).
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The configured entries of `tier`.
    pub fn of(&self, tier: Tier) -> &[CorpusRepoConfig] {
        match tier {
            Tier::Standard => &self.standard,
            Tier::Open => &self.open,
            Tier::Proprietary => &self.proprietary,
        }
    }
}

/// A standard repository Kovan knows without configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinRepo {
    pub name: &'static str,
    pub remote: &'static str,
    pub branch: &'static str,
}

/// Kovan's built-in standard repositories, pulled by default into every
/// Kovan folder. The first is mounted at `[paths] standard_corpus`, any
/// later one at `literature/standard-<name>`. Extend the list per folder
/// with `[[repos.standard]]`; a topic split of the standard corpus is one
/// more entry here.
pub const BUILTIN_STANDARD_REPOS: &[BuiltinRepo] = &[BuiltinRepo {
    name: "kovan-standard",
    remote: crate::corpus::CORPUS_REPOSITORY_URL,
    branch: crate::corpus::CORPUS_REPOSITORY_BRANCH,
}];

/// The name of the open repository at `[paths] open_sources`, unless an
/// entry with that path renames it.
pub const LEGACY_OPEN: &str = "open";

/// The name of the proprietary repository at `[paths] restricted_sources`,
/// unless an entry with that path renames it.
pub const LEGACY_PROPRIETARY: &str = "proprietary";

/// Where a resolved repository came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoOrigin {
    /// [`BUILTIN_STANDARD_REPOS`].
    Builtin,
    /// The older single-repository settings (`[paths]`, `[corpora]`,
    /// `[private_submodule]`).
    Legacy,
    /// A `[[repos.<tier>]]` entry.
    Configured,
}

/// One corpus repository of a Kovan folder, resolved: an absolute path and
/// every setting filled in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CorpusRepo {
    pub tier: Tier,
    pub name: String,
    /// Relative to the Kovan folder.
    pub rel: PathBuf,
    /// Absolute.
    pub dir: PathBuf,
    pub remote: Option<String>,
    pub branch: Option<String>,
    /// The tier's default ingest target.
    pub is_default: bool,
    /// Whether Kovan may commit into and push it: always for open and
    /// proprietary, only when configured for standard.
    pub writable: bool,
    pub origin: RepoOrigin,
}

impl CorpusRepo {
    /// Whether the checkout is here (has its own `.git`).
    pub fn is_downloaded(&self) -> bool {
        self.dir.join(".git").exists()
    }

    /// `Open corpus (name)`, for messages and reports.
    pub fn label(&self) -> String {
        format!("{} ({})", self.tier.label(), self.name)
    }
}

/// A reference to one repository, as the ingest form picks it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub tier: Tier,
    pub name: String,
}

/// Every corpus repository `config` declares for the Kovan folder at
/// `root_dir`, in tier order (standard, open, proprietary) and within a tier
/// in the order described in the module doc.
pub fn resolve(root_dir: &Path, config: &RootConfig) -> Vec<CorpusRepo> {
    let mut out = Vec::new();
    for tier in Tier::ALL {
        let mut list: Vec<(CorpusRepoConfig, RepoOrigin)> = Vec::new();
        match tier {
            Tier::Standard => {
                if config.repos.builtin_standard {
                    for (i, b) in BUILTIN_STANDARD_REPOS.iter().enumerate() {
                        let path = if i == 0 {
                            config.paths.standard_corpus.clone()
                        } else {
                            PathBuf::from(format!("literature/standard-{}", b.name))
                        };
                        list.push((
                            CorpusRepoConfig {
                                name: b.name.to_string(),
                                path,
                                remote: Some(b.remote.to_string()),
                                branch: Some(b.branch.to_string()),
                                default: false,
                                writable: false,
                            },
                            RepoOrigin::Builtin,
                        ));
                    }
                }
            }
            Tier::Open => list.push((
                CorpusRepoConfig {
                    name: LEGACY_OPEN.to_string(),
                    path: config.paths.open_sources.clone(),
                    remote: config.corpora.open_remote.clone(),
                    branch: None,
                    default: false,
                    writable: true,
                },
                RepoOrigin::Legacy,
            )),
            Tier::Proprietary => {
                list.push((
                    CorpusRepoConfig {
                        name: LEGACY_PROPRIETARY.to_string(),
                        path: config.paths.restricted_sources.clone(),
                        remote: config.corpora.proprietary_remote.clone().or_else(|| {
                            config.private_submodule.as_ref().map(|p| p.remote.clone())
                        }),
                        branch: None,
                        default: false,
                        writable: true,
                    },
                    RepoOrigin::Legacy,
                ))
            }
        }
        for entry in config.repos.of(tier) {
            let replaces = list.iter().position(|(c, origin)| {
                same_rel(&c.path, &entry.path)
                    || (*origin == RepoOrigin::Builtin
                        && matches!((&c.remote, &entry.remote),
                            (Some(a), Some(b)) if crate::save_push::normalize_url(a)
                                == crate::save_push::normalize_url(b)))
            });
            let mut entry = entry.clone();
            match replaces {
                Some(i) => {
                    // Keep the older setting's remote when the entry gives none.
                    if entry.remote.is_none() {
                        entry.remote = list[i].0.remote.clone();
                    }
                    let origin = list[i].1;
                    list[i] = (entry, origin);
                }
                None => list.push((entry, RepoOrigin::Configured)),
            }
        }
        let default_at = list.iter().position(|(c, _)| c.default).unwrap_or(0);
        for (i, (c, origin)) in list.into_iter().enumerate() {
            out.push(CorpusRepo {
                tier,
                dir: root_dir.join(&c.path),
                rel: c.path,
                name: c.name,
                remote: c.remote,
                branch: c.branch,
                is_default: i == default_at,
                writable: tier != Tier::Standard || c.writable,
                origin,
            });
        }
    }
    out
}

/// The remote setup and pull clone `repo` from when it is not downloaded:
/// its resolved remote, except that the proprietary repository at
/// `[paths] restricted_sources` is fetched from `[corpora]
/// proprietary_remote` only, as before #458 (a `[private_submodule] remote`
/// alone names where Save may push, not something setup clones).
pub fn setup_remote(config: &RootConfig, repo: &CorpusRepo) -> Option<String> {
    let private = config.private_submodule.as_ref().map(|p| p.remote.clone());
    if repo.tier == Tier::Proprietary
        && repo.origin == RepoOrigin::Legacy
        && config.corpora.proprietary_remote.is_none()
        && repo.remote == private
    {
        return None;
    }
    repo.remote.clone()
}

/// Whether two relative paths name the same place (ignoring `.` and a
/// trailing `/`).
fn same_rel(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| -> Vec<String> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect()
    };
    norm(a) == norm(b)
}

/// Whether relative path `inner` is `outer` or inside it.
fn rel_within(inner: &Path, outer: &Path) -> bool {
    let norm = |p: &Path| -> Vec<String> {
        p.components()
            .filter(|c| !matches!(c, Component::CurDir))
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect()
    };
    let (i, o) = (norm(inner), norm(outer));
    i.len() >= o.len() && i[..o.len()] == o[..]
}

/// What is wrong with the repositories `config` declares, one message each;
/// empty when nothing is. [`crate::root::KovanRoot::open`] refuses a folder
/// with any problem, so nothing can ingest into, commit or push a
/// misconfigured repository.
///
/// - every name is non-empty, of letters, digits, `-`, `_` or `.`, and
///   unique across all tiers;
/// - every path is relative, non-empty and has no `..`;
/// - two repositories of one tier do not share a path;
/// - **a proprietary repository shares or nests a path with no other
///   repository**, of any tier.
///
/// A standard and an open repository may share a path (an older layout
/// mounted one checkout as both); it is then pulled once.
pub fn validate(config: &RootConfig) -> Vec<String> {
    let repos = resolve(Path::new(""), config);
    let mut problems = Vec::new();
    for (i, r) in repos.iter().enumerate() {
        let where_ = format!("{} repository {:?}", r.tier.key(), r.name);
        if r.name.is_empty()
            || !r
                .name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        {
            problems.push(format!(
                "{where_}: a name may only use letters, digits, '-', '_' and '.'"
            ));
        }
        if r.rel.as_os_str().is_empty()
            || r.rel.is_absolute()
            || r.rel
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::Prefix(_)))
        {
            problems.push(format!(
                "{where_}: path {} must be relative to the Kovan folder, without '..'",
                r.rel.display()
            ));
        }
        for other in &repos[..i] {
            if other.name == r.name {
                problems.push(format!(
                    "{where_}: the name is already used by a {} repository",
                    other.tier.key()
                ));
            }
            let shared = same_rel(&other.rel, &r.rel);
            let nested = rel_within(&other.rel, &r.rel) || rel_within(&r.rel, &other.rel);
            let proprietary_involved =
                (r.tier == Tier::Proprietary) != (other.tier == Tier::Proprietary);
            if other.tier == r.tier && shared {
                problems.push(format!(
                    "{where_}: path {} is already used by {:?}",
                    r.rel.display(),
                    other.name
                ));
            } else if proprietary_involved && nested {
                problems.push(format!(
                    "{where_}: path {} overlaps the {} repository {:?} at {}; a proprietary \
                     repository must not share or nest a folder with another repository, or its \
                     documents could be published",
                    r.rel.display(),
                    other.tier.key(),
                    other.name,
                    other.rel.display()
                ));
            }
        }
    }
    problems
}

/// Why pushing a proprietary repository to `url` must be refused, if it
/// must: `url` is (after [`crate::save_push::normalize_url`]) a remote of a
/// standard or open repository, a built-in standard repository, or a
/// `known_public` entry; or it contains `outram-park-backend`. Offline and
/// deterministic; [`anonymously_readable`] is the network half.
pub fn public_remote_reason(config: &RootConfig, all: &[CorpusRepo], url: &str) -> Option<String> {
    let want = crate::save_push::normalize_url(url);
    if url.contains("outram-park-backend") {
        return Some("it is the public outram-park-backend repository".into());
    }
    for r in all.iter().filter(|r| r.tier != Tier::Proprietary) {
        if let Some(remote) = &r.remote {
            if crate::save_push::normalize_url(remote) == want {
                return Some(format!(
                    "it is the remote of the {} repository {:?}, which is public",
                    r.tier.key(),
                    r.name
                ));
            }
        }
    }
    for b in BUILTIN_STANDARD_REPOS {
        if crate::save_push::normalize_url(b.remote) == want {
            return Some(format!(
                "it is Kovan's built-in standard corpus {:?}, which is public",
                b.name
            ));
        }
    }
    config
        .repos
        .known_public
        .iter()
        .find(|k| crate::save_push::normalize_url(k) == want)
        .map(|k| format!("it is listed in [repos] known_public ({k})"))
}

/// Whether `url` can be read **without any credentials**: `Some(true)` when
/// `git ls-remote` succeeds with every credential helper, prompt, `.netrc`
/// and user or system Git config out of reach (a public repository),
/// `Some(false)` when it fails, `None` when the check does not apply.
///
/// Applies only to `http(s)://` URLs and GitHub-style `git@host:owner/repo`
/// ones (checked as `https://host/owner/repo`); a local path or an `ssh://`
/// URL to a non-GitHub host is `None`. Set `KOVAN_SKIP_VISIBILITY_PROBE` to
/// skip it (`None`). A network failure reads as `Some(false)`: the push
/// that follows would fail anyway, and the offline checks in
/// [`public_remote_reason`] have already run.
pub fn anonymously_readable(url: &str) -> Option<bool> {
    if std::env::var_os("KOVAN_SKIP_VISIBILITY_PROBE").is_some() {
        return None;
    }
    let https = https_form(url)?;
    // No credential of the user's may take part: not a generic or a
    // per-URL helper (`gh auth setup-git` writes
    // `credential.https://github.com.helper`, which `-c credential.helper=`
    // does not clear), not `~/.netrc` (curl reads it), not a repository's
    // own config. So the probe runs with an empty home, no global or system
    // config, from an empty directory.
    let empty = std::env::temp_dir().join("kovan-visibility-probe");
    let _ = std::fs::create_dir_all(&empty);
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let out = std::process::Command::new("git")
        .current_dir(&empty)
        .env("HOME", &empty)
        .env("USERPROFILE", &empty)
        .env("XDG_CONFIG_HOME", &empty)
        .env("GIT_CONFIG_GLOBAL", null)
        .env("GIT_CONFIG_SYSTEM", null)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .args([
            "-c",
            "credential.helper=",
            "-c",
            "core.askPass=",
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=15",
            "ls-remote",
            "--heads",
            &https,
        ])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("SSH_ASKPASS", "")
        .output()
        .ok()?;
    Some(out.status.success())
}

/// `url` as an anonymous `https://` URL, when it is an `http(s)://` URL or
/// an scp-style `user@host:owner/repo` one.
fn https_form(url: &str) -> Option<String> {
    let u = url.trim();
    if let Some(rest) = u
        .strip_prefix("https://")
        .or_else(|| u.strip_prefix("http://"))
    {
        // Drop any user part: the check must not use one.
        let rest = match rest.split_once('@') {
            Some((user, after)) if !user.contains('/') => after,
            _ => rest,
        };
        return Some(format!("https://{rest}"));
    }
    if u.contains("://") {
        return None;
    }
    let (user_host, path) = u.split_once(':')?;
    let (_, host) = user_host.split_once('@')?;
    if host.is_empty() || host.contains('/') || path.is_empty() {
        return None;
    }
    Some(format!("https://{host}/{path}"))
}

/// Every PDF under `dir`, recursively, skipping Git's own folder, sorted.
pub fn pdfs_in(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if e.file_name() != ".git" {
                    walk(&path, out);
                }
            } else if path
                .extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("pdf"))
            {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

/// GitHub's recommended maximum repository size, 1 GiB.
pub const RECOMMENDED_MAX_BYTES: u64 = 1 << 30;

/// The size at which [`size_warning`] warns by default: 90 % of
/// [`RECOMMENDED_MAX_BYTES`].
pub const SIZE_WARN_BYTES: u64 = RECOMMENDED_MAX_BYTES / 10 * 9;

/// Total bytes of the files in a checkout, excluding `.git` (for PDFs the
/// history is roughly the files again, so this is what nears the limit).
pub fn checkout_size(dir: &Path) -> u64 {
    fn walk(dir: &Path) -> u64 {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return 0;
        };
        let mut total = 0;
        for e in entries.flatten() {
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                if e.file_name() != ".git" {
                    total += walk(&e.path());
                }
            } else if kind.is_file() {
                total += e.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
        total
    }
    walk(dir)
}

/// A warning when `repo`'s checkout holds `threshold` bytes or more
/// (normally [`SIZE_WARN_BYTES`]), suggesting a new repository in the same
/// tier; `None` below it or when the checkout is absent.
pub fn size_warning(repo: &CorpusRepo, threshold: u64) -> Option<String> {
    if !repo.dir.is_dir() {
        return None;
    }
    let size = checkout_size(&repo.dir);
    (size >= threshold).then(|| {
        format!(
            "{} holds {:.2} GB, near GitHub's recommended 1 GB per repository; consider adding \
             another {} repository (a [[repos.{}]] entry in kovan_root.toml) and filing new \
             documents there",
            repo.label(),
            size as f64 / 1e9,
            repo.tier.key(),
            repo.tier.key()
        )
    })
}

#[cfg(test)]
mod multi_repo_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> RootConfig {
        toml::from_str(text).unwrap()
    }

    const LEGACY: &str = r#"
schema_version = 1
[library]
id = "teddy-kovan-repo"
name = "Teddy's Kovan"
[private_submodule]
remote = "https://github.com/o/private.git"
[corpora]
open_remote = "https://github.com/o/reactor-literature.git"
proprietary_remote = "https://github.com/o/private.git"
"#;

    /// A file without `[repos]` resolves to exactly the three repositories
    /// it always had, and writes back without a `[repos]` table.
    #[test]
    fn a_legacy_file_resolves_to_one_repository_per_tier() {
        let config = parse(LEGACY);
        let repos = resolve(Path::new("/lib"), &config);
        let summary: Vec<(Tier, &str, &str, bool, bool)> = repos
            .iter()
            .map(|r| {
                (
                    r.tier,
                    r.name.as_str(),
                    r.rel.to_str().unwrap(),
                    r.is_default,
                    r.writable,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    Tier::Standard,
                    "kovan-standard",
                    "literature/standard-corpus",
                    true,
                    false
                ),
                (Tier::Open, "open", "literature/open-corpus", true, true),
                (
                    Tier::Proprietary,
                    "proprietary",
                    "literature/proprietary",
                    true,
                    true
                ),
            ]
        );
        assert_eq!(
            repos[0].remote.as_deref(),
            Some(crate::corpus::CORPUS_REPOSITORY_URL)
        );
        assert_eq!(
            repos[2].remote.as_deref(),
            Some("https://github.com/o/private.git")
        );
        assert!(validate(&config).is_empty());
        assert!(!config.to_toml().unwrap().contains("repos"));
    }

    /// `[[repos.*]]` entries extend each tier; one at an older path renames
    /// it in place; `default = true` moves the default; the file round-trips.
    #[test]
    fn configured_repositories_extend_and_replace() {
        let text = format!(
            "{LEGACY}{}",
            r#"
[repos]
known_public = ["https://github.com/o/mirror.git"]

[[repos.standard]]
name = "htgr-standard"
remote = "https://github.com/o/htgr-standard.git"
path = "literature/standard-htgr"

[[repos.open]]
name = "reactor-literature"
path = "literature/open-corpus"

[[repos.open]]
name = "open-th"
remote = "https://github.com/o/open-th.git"
path = "literature/open-th"
default = true

[[repos.proprietary]]
name = "books"
remote = "git@github.com:o/private-books.git"
path = "literature/proprietary-books"
branch = "main"
"#
        );
        let config = parse(&text);
        assert!(validate(&config).is_empty(), "{:?}", validate(&config));
        let repos = resolve(Path::new("/lib"), &config);
        let names: Vec<&str> = repos.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "kovan-standard",
                "htgr-standard",
                "reactor-literature",
                "open-th",
                "proprietary",
                "books"
            ]
        );
        let open: Vec<&CorpusRepo> = repos.iter().filter(|r| r.tier == Tier::Open).collect();
        // Renamed in place, keeping the [corpora] remote.
        assert_eq!(
            open[0].remote.as_deref(),
            Some("https://github.com/o/reactor-literature.git")
        );
        assert_eq!(open[0].origin, RepoOrigin::Legacy);
        assert!(!open[0].is_default && open[1].is_default);
        assert_eq!(repos[5].dir, Path::new("/lib/literature/proprietary-books"));
        assert!(!repos[1].writable, "standard is read-only by default");

        let back: RootConfig = toml::from_str(&config.to_toml().unwrap()).unwrap();
        assert_eq!(back, config);
    }

    /// Overlapping a proprietary repository with any other, reusing a
    /// name, or escaping the folder is a problem; a standard and an open
    /// repository may share a checkout.
    #[test]
    fn validation_rejects_unsafe_layouts() {
        let with = |extra: &str| parse(&format!("{LEGACY}{extra}"));
        let nested = with(
            "[[repos.proprietary]]\nname = \"p2\"\npath = \"literature/open-corpus/secret\"\n",
        );
        let problems = validate(&nested);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("must not share or nest")),
            "{problems:?}"
        );
        let dup = with("[[repos.open]]\nname = \"proprietary\"\npath = \"literature/x\"\n");
        assert!(validate(&dup).iter().any(|p| p.contains("already used")));
        let escape = with("[[repos.open]]\nname = \"x\"\npath = \"../elsewhere\"\n");
        assert!(validate(&escape).iter().any(|p| p.contains("without '..'")));
        let shared =
            with("[[repos.open]]\nname = \"same\"\npath = \"literature/standard-corpus\"\n");
        assert!(validate(&shared).is_empty(), "{:?}", validate(&shared));
    }

    /// The offline public-remote guard, over every URL spelling.
    #[test]
    fn public_remotes_are_recognised_offline() {
        let config = parse(&format!(
            "{LEGACY}[repos]\nknown_public = [\"https://github.com/o/mirror.git\"]\n"
        ));
        let all = resolve(Path::new("/lib"), &config);
        for url in [
            "git@github.com:o/reactor-literature.git",
            "https://github.com/theodoreOnzGit/reactor-literature",
            "ssh://git@github.com/o/mirror.git",
            "https://github.com/x/outram-park-backend.git",
        ] {
            assert!(
                public_remote_reason(&config, &all, url).is_some(),
                "{url} should be public"
            );
        }
        assert!(public_remote_reason(&config, &all, "https://github.com/o/private.git").is_none());
    }

    #[test]
    fn https_forms() {
        assert_eq!(
            https_form("git@github.com:o/r.git").as_deref(),
            Some("https://github.com/o/r.git")
        );
        assert_eq!(
            https_form("https://user@github.com/o/r").as_deref(),
            Some("https://github.com/o/r")
        );
        assert_eq!(https_form("/tmp/remote.git"), None);
        assert_eq!(https_form("ssh://git@host/o/r"), None);
        assert_eq!(https_form("C:/x/y"), None);
    }

    /// The size warning fires at the threshold and names the tier to add a
    /// repository to.
    #[test]
    fn a_large_checkout_is_warned_about() {
        let tmp = tempfile::tempdir().unwrap();
        let config = parse(LEGACY);
        let repos = resolve(tmp.path(), &config);
        let open = repos.iter().find(|r| r.tier == Tier::Open).unwrap();
        std::fs::create_dir_all(open.dir.join(".git")).unwrap();
        std::fs::write(open.dir.join("a.pdf"), vec![0u8; 3000]).unwrap();
        std::fs::write(open.dir.join(".git/pack"), vec![0u8; 9000]).unwrap();
        assert_eq!(checkout_size(&open.dir), 3000, ".git is not counted");
        assert!(size_warning(open, 3001).is_none());
        let w = size_warning(open, 3000).unwrap();
        assert!(w.contains("[[repos.open]]"), "{w}");
    }
}
