//! # kovan-discovery
//!
//! Repository indexing and file discovery for KOVAN. This is the layer beneath
//! semantics: before any language-native tooling runs, KOVAN needs to find the
//! files and grep their contents. It builds directly on two mature Rust
//! engines:
//!
//! - [`ignore`] — the `.gitignore`-aware directory walker behind `fd`/ripgrep.
//! - [`grep_searcher`] + [`grep_regex`] — the ripgrep search engine.
//!
//! Everything here is deterministic and offline: no index database, no
//! network access, no hidden state. Given the same file-system tree and the
//! same arguments, every function in this crate returns the same result on
//! every call, on every platform (Linux, Windows, macOS, Android/Termux) —
//! see "Determinism" on each function for the specific guarantee.
//!
//! ## What it provides
//!
//! - [`discover`] / [`discover_kind`] — enumerate files under a root, honouring
//!   `.gitignore`, optionally filtered to a [`FileKind`] (source, Markdown,
//!   PDF, metadata). Results are always sorted, so callers get a stable order
//!   regardless of the host filesystem's raw directory-entry order.
//! - [`search_file`] — ripgrep-style regex search of a single file, returning
//!   line number, 1-based character column, and text for every match.
//! - [`search_repository`] — the two combined: discover every file of a
//!   [`FileKind`] under a root, then search each one, in one deterministic
//!   pass. `kovan-semantics`'s `rough_definition_scan` implements this same
//!   discover-then-search loop today (see that crate); reach for
//!   [`search_repository`] directly when you don't need a language-specific
//!   heuristic on top.
//!
//! `kovan-semantics` starts from these primitives ("ripgrep first") before
//! escalating to language servers.
//!
//! ## What it does *not* do
//!
//! No index is persisted anywhere — every call re-walks the filesystem and
//! re-searches the requested files from scratch. That is intentional: KOVAN's
//! "Deterministic First" design principle (see `docs/kovan.md`) prefers
//! recomputation over a cache that can silently go stale. If a caller needs
//! caching, that is its own responsibility, layered on top of this crate.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::sinks::UTF8;
use grep_searcher::Searcher;
use ignore::WalkBuilder;

/// A category of file KOVAN cares about, with its associated extensions.
///
/// This is a closed, compile-time-known set (per the workspace's "enums over
/// trait objects" rule) — adding a new kind is a one-line match-exhaustiveness
/// error at every call site that needs updating, not a runtime surprise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    /// Source code (Rust, C++, Python, Fortran, …).
    Source,
    /// Markdown documents.
    Markdown,
    /// PDF literature.
    Pdf,
    /// Metadata / configuration (TOML, JSON, YAML).
    Metadata,
}

impl FileKind {
    /// The lowercase file extensions associated with this kind.
    ///
    /// Matching is case-insensitive: [`discover`] lowercases each candidate
    /// file's extension before comparing it against this list, so `FOO.RS`
    /// and `foo.rs` are both recognised as [`FileKind::Source`].
    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            FileKind::Source => &[
                "rs", "cpp", "cc", "cxx", "hpp", "hh", "h", "c", "py", "f90", "f", "for", "f95",
                "f03",
            ],
            FileKind::Markdown => &["md", "markdown"],
            FileKind::Pdf => &["pdf"],
            FileKind::Metadata => &["toml", "json", "yaml", "yml"],
        }
    }
}

/// A single search hit within a file.
///
/// Line and column are both 1-based, matching the convention used by editors,
/// compilers, and `ripgrep`'s own CLI output (`path:line:column: text`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// 1-based line number.
    pub line: u64,
    /// 1-based character (not byte) column of the start of the match on that
    /// line. Computed by counting `char`s before the match, so it stays
    /// correct for multi-byte UTF-8 text (e.g. a match after a non-ASCII
    /// identifier or comment). Defaults to `1` in the unexpected case where
    /// the regex that already matched this line cannot be re-located within
    /// it (see [`search_file`] for when that can happen).
    pub column: usize,
    /// The matching line, trailing newline trimmed.
    pub text: String,
}

/// Errors produced by discovery / search.
#[derive(Debug)]
pub enum DiscoveryError {
    /// The regex pattern was invalid.
    BadPattern(String),
    /// An I/O error occurred while searching (includes the file not existing,
    /// a permissions failure, or the file not being valid UTF-8 — this crate
    /// searches decoded text, so a binary or non-UTF-8 file surfaces as an
    /// I/O-flavoured decoding error rather than a match list).
    Io(std::io::Error),
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiscoveryError::BadPattern(p) => write!(f, "invalid search pattern: {p}"),
            DiscoveryError::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for DiscoveryError {}

/// Discover all files under `root`, honouring `.gitignore` (and `.ignore`,
/// and global git excludes — anything the `ignore` crate's default walker
/// respects, the same rules `fd` and `ripgrep` use). If `exts` is non-empty,
/// only files whose (lowercased) extension is listed are returned; an empty
/// slice returns every non-ignored file.
///
/// Hidden files/directories (dotfiles, `.git/`) are skipped by default — this
/// is the `ignore` crate's standard behaviour, matching `fd`/`ripgrep`'s
/// defaults. Symlinks are not followed.
///
/// # `.gitignore` handling
///
/// `.gitignore` rules are honoured **even when `root` is not inside a git
/// repository** — a bare `.gitignore` in a plain directory (a downloaded
/// tarball, a vendored source tree, a literature staging directory) is still
/// respected, because that is what `.gitignore` is *supposed* to do. This is a
/// deliberate choice: the `ignore` crate's default (`require_git = true`) would
/// instead treat `.gitignore` as inert outside a real `.git` repo, silently
/// breaking this function's "honours `.gitignore`" contract for non-repo trees.
/// We call `.require_git(false)` so the contract holds everywhere. `.ignore`
/// files and global git excludes are honoured either way, and for a directory
/// that *is* inside a git repo the behaviour is unchanged.
///
/// # Determinism
///
/// The returned `Vec` is always sorted by path
/// ([`Ord`] on [`PathBuf`], i.e. lexicographic by path component), regardless
/// of the order the underlying filesystem's directory entries were returned
/// in. Two calls against the same tree — on the same machine or a different
/// one — always produce the same order.
///
/// # Error handling
///
/// This function does not return a `Result`: a root that does not exist, or
/// a subdirectory that cannot be read (permissions), is silently skipped
/// rather than surfaced as an error, matching `fd`'s default behaviour of
/// best-effort traversal. A non-existent or fully inaccessible `root` simply
/// yields an empty `Vec`. Callers that need to distinguish "no matching
/// files" from "root does not exist" should check `root.exists()` themselves
/// before calling.
pub fn discover(root: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    // `require_git(false)`: apply `.gitignore`/`.git/info/exclude`/global-git
    // rules even when `root` is not (or is not inside) an actual `.git`
    // repository. Without this, `ignore`'s default behaviour is to treat
    // those files as inert outside a real git repo, which would silently
    // break `.gitignore` honouring for non-repo trees (e.g. a literature
    // staging directory) — surprising given this function's documented
    // contract. `.ignore` files are always honoured either way.
    for entry in WalkBuilder::new(root).require_git(false).build().flatten() {
        let is_file = entry.file_type().is_some_and(|ft| ft.is_file());
        if !is_file {
            continue;
        }
        let path = entry.path();
        let keep = exts.is_empty()
            || path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .is_some_and(|e| exts.contains(&e.as_str()));
        if keep {
            out.push(path.to_path_buf());
        }
    }
    out.sort();
    out
}

/// Discover all files under `root` of a given [`FileKind`]. Equivalent to
/// calling [`discover`] with that kind's [`FileKind::extensions`].
///
/// See [`discover`] for the `.gitignore` and determinism (sorted-output)
/// guarantees, which apply identically here.
pub fn discover_kind(root: &Path, kind: FileKind) -> Vec<PathBuf> {
    discover(root, kind.extensions())
}

/// Search a single file for `pattern` (a regular expression), returning every
/// matching line with its line number and column. Uses the ripgrep engine
/// (`grep-searcher` + `grep-regex`), so the pattern accepts the same syntax
/// `rg` does (Rust's `regex` crate syntax).
///
/// # Determinism
///
/// Matches are returned in ascending line order (the order the file is read
/// top to bottom); re-running against an unchanged file always yields an
/// identical result.
///
/// # Errors
///
/// - [`DiscoveryError::BadPattern`] if `pattern` fails to compile as a regex.
/// - [`DiscoveryError::Io`] if `path` cannot be opened/read, or is not valid
///   UTF-8 (this function searches decoded text; binary files are rejected
///   rather than silently mis-decoded).
///
/// # Column computation
///
/// [`SearchMatch::column`] is derived by re-locating the pattern within the
/// matched line via [`grep_matcher::Matcher::find`]. This is a second, local
/// search over an already-matched line (not a full extra file pass), so it is
/// cheap. In the practically-unreachable case where that re-location fails —
/// the searcher matched a line but the matcher then reports no match on that
/// same line — the column falls back to `1` rather than panicking.
pub fn search_file(path: &Path, pattern: &str) -> Result<Vec<SearchMatch>, DiscoveryError> {
    let matcher =
        RegexMatcher::new(pattern).map_err(|e| DiscoveryError::BadPattern(e.to_string()))?;
    let mut matches = Vec::new();
    Searcher::new()
        .search_path(
            &matcher,
            path,
            UTF8(|lnum, line| {
                let column = matcher
                    .find(line.as_bytes())
                    .ok()
                    .flatten()
                    .map(|m| line[..m.start()].chars().count() + 1)
                    .unwrap_or(1);
                matches.push(SearchMatch {
                    line: lnum,
                    column,
                    text: line.trim_end().to_string(),
                });
                Ok(true)
            }),
        )
        .map_err(DiscoveryError::Io)?;
    Ok(matches)
}

/// Discover every file of `kind` under `root`, then [`search_file`] each one
/// for `pattern`. The combined "find files, then grep them" primitive —
/// `kovan-semantics`'s `rough_definition_scan` implements this same
/// discover-then-search loop by hand today; reach for this directly if you
/// just need "grep this whole repository for X" without a language-specific
/// heuristic on top.
///
/// # Determinism
///
/// Files are visited in the sorted order [`discover_kind`] returns, and each
/// file's matches are appended in ascending line order, so the overall
/// `Vec<(PathBuf, SearchMatch)>` is fully deterministic for a given tree.
///
/// # Errors
///
/// Stops at (and returns) the first [`DiscoveryError`] raised by
/// [`search_file`] on any discovered file — a [`DiscoveryError::BadPattern`]
/// on the first file means every subsequent file has the same bad pattern, so
/// failing fast avoids repeating the same compile error. A per-file I/O error
/// (e.g. one non-UTF-8 file among many source files) aborts the whole scan
/// rather than silently skipping that file; callers that want best-effort
/// behaviour should call [`discover_kind`] and [`search_file`] themselves and
/// decide how to handle a single file's error.
pub fn search_repository(
    root: &Path,
    kind: FileKind,
    pattern: &str,
) -> Result<Vec<(PathBuf, SearchMatch)>, DiscoveryError> {
    // Validate the pattern once, up front, so a bad pattern fails fast even
    // if `root` contains no matching files (rather than silently returning
    // an empty `Vec`).
    RegexMatcher::new(pattern).map_err(|e| DiscoveryError::BadPattern(e.to_string()))?;

    let mut hits = Vec::new();
    for file in discover_kind(root, kind) {
        for m in search_file(&file, pattern)? {
            hits.push((file.clone(), m));
        }
    }
    Ok(hits)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    fn crate_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
    }

    // -------------------------------------------------------------------
    // Original smoke tests against this crate's own source.
    // -------------------------------------------------------------------

    #[test]
    fn discovers_own_source() {
        let rs = discover_kind(&crate_dir(), FileKind::Source);
        assert!(rs.iter().any(|p| p.ends_with("src/lib.rs")));
    }

    #[test]
    fn searches_own_source() {
        let lib = crate_dir().join("src/lib.rs");
        let hits = search_file(&lib, r"kovan-discovery").expect("search ok");
        assert!(!hits.is_empty());
    }

    #[test]
    fn bad_pattern_errors() {
        let lib = crate_dir().join("src/lib.rs");
        assert!(matches!(
            search_file(&lib, "("),
            Err(DiscoveryError::BadPattern(_))
        ));
    }

    // -------------------------------------------------------------------
    // tempfile-fixture tests: a throwaway tree so discovery/search behaviour
    // is asserted against a controlled layout, independent of this crate's
    // own (changeable) source tree.
    // -------------------------------------------------------------------

    /// Build a small repository fixture:
    ///
    /// ```text
    /// <tmp>/
    /// ├── .gitignore          ("target/\n*.log\n")
    /// ├── src/
    /// │   ├── lib.rs          (contains "pub fn kept_one" and "pub fn kept_two")
    /// │   └── ignored.log     (would match, but excluded by .gitignore)
    /// ├── target/
    /// │   └── build.rs        (excluded: whole dir is gitignored)
    /// ├── README.md
    /// └── notes.txt            (not a tracked FileKind extension)
    /// ```
    fn fixture_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create tempdir");
        let root = dir.path();

        fs::write(root.join(".gitignore"), "target/\n*.log\n").unwrap();

        fs::create_dir(root.join("src")).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "pub fn kept_one() {}\nfn private() {}\npub fn kept_two() {}\n",
        )
        .unwrap();
        fs::write(
            root.join("src/ignored.log"),
            "pub fn should_not_be_found() {}\n",
        )
        .unwrap();

        fs::create_dir(root.join("target")).unwrap();
        fs::write(root.join("target/build.rs"), "pub fn also_ignored() {}\n").unwrap();

        fs::write(root.join("README.md"), "# Fixture\n").unwrap();
        fs::write(root.join("notes.txt"), "not a tracked kind\n").unwrap();

        dir
    }

    #[test]
    fn gitignore_excludes_matching_files_and_directories() {
        let repo = fixture_repo();
        let all = discover(repo.path(), &[]);

        assert!(
            all.iter().any(|p| p.ends_with("src/lib.rs")),
            "src/lib.rs should be discovered: {all:?}"
        );
        assert!(
            !all.iter().any(|p| p.ends_with(".gitignore")),
            ".gitignore is a dotfile, hidden by the walker's default `hidden(true)`, \
             independent of its own ignore rules: {all:?}"
        );
        assert!(
            !all.iter().any(|p| p.ends_with("src/ignored.log")),
            "*.log must be excluded by .gitignore: {all:?}"
        );
        assert!(
            !all.iter().any(|p| p.to_string_lossy().contains("target")),
            "target/ must be excluded by .gitignore: {all:?}"
        );
    }

    #[test]
    fn discover_kind_filters_by_extension_and_respects_gitignore() {
        let repo = fixture_repo();

        let sources = discover_kind(repo.path(), FileKind::Source);
        assert_eq!(sources.len(), 1, "only src/lib.rs qualifies: {sources:?}");
        assert!(sources[0].ends_with("src/lib.rs"));

        let markdown = discover_kind(repo.path(), FileKind::Markdown);
        assert_eq!(markdown.len(), 1);
        assert!(markdown[0].ends_with("README.md"));
    }

    #[test]
    fn discover_returns_sorted_stable_order() {
        let repo = fixture_repo();
        let first = discover(repo.path(), &[]);
        let second = discover(repo.path(), &[]);
        assert_eq!(
            first, second,
            "two calls against an unchanged tree must agree"
        );

        let mut sorted = first.clone();
        sorted.sort();
        assert_eq!(first, sorted, "discover() output must already be sorted");
    }

    #[test]
    fn discover_nonexistent_root_returns_empty_not_panic() {
        let repo = fixture_repo();
        let missing = repo.path().join("does-not-exist");
        assert!(discover(&missing, &[]).is_empty());
    }

    #[test]
    fn search_file_reports_correct_line_and_column() {
        let repo = fixture_repo();
        let lib_rs = repo.path().join("src/lib.rs");

        let hits = search_file(&lib_rs, r"pub fn \w+").expect("search ok");
        assert_eq!(
            hits.len(),
            2,
            "two pub fns, private() must not match: {hits:?}"
        );

        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[0].column, 1, "match starts at column 1 on line 1");
        assert_eq!(hits[0].text, "pub fn kept_one() {}");

        assert_eq!(hits[1].line, 3);
        assert_eq!(hits[1].column, 1);
        assert_eq!(hits[1].text, "pub fn kept_two() {}");
    }

    #[test]
    fn search_file_column_accounts_for_leading_offset() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let file = dir.path().join("indented.rs");
        fs::write(&file, "    let x = needle;\n").unwrap();

        let hits = search_file(&file, "needle").expect("search ok");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 1);
        // "    let x = " is 12 characters, so "needle" starts at 1-based column 13.
        assert_eq!(hits[0].column, 13);
    }

    #[test]
    fn search_file_missing_path_is_io_error() {
        let dir = tempfile::tempdir().expect("create tempdir");
        let missing = dir.path().join("nope.rs");
        assert!(matches!(
            search_file(&missing, "anything"),
            Err(DiscoveryError::Io(_))
        ));
    }

    #[test]
    fn search_repository_combines_discovery_and_search_deterministically() {
        let repo = fixture_repo();
        let hits =
            search_repository(repo.path(), FileKind::Source, r"pub fn \w+").expect("search ok");

        assert_eq!(hits.len(), 2);
        assert!(hits.iter().all(|(p, _)| p.ends_with("src/lib.rs")));
        assert_eq!(hits[0].1.line, 1);
        assert_eq!(hits[1].1.line, 3);

        // *.log is gitignored, so the (otherwise-matching) ignored.log content
        // must never appear even though FileKind::Source doesn't cover .log —
        // this asserts the gitignore filtering, not just the extension filter.
        assert!(!hits
            .iter()
            .any(|(p, _)| p.to_string_lossy().contains("ignored")));
    }

    #[test]
    fn search_repository_bad_pattern_fails_before_touching_files() {
        let repo = fixture_repo();
        assert!(matches!(
            search_repository(repo.path(), FileKind::Source, "("),
            Err(DiscoveryError::BadPattern(_))
        ));
    }
}
