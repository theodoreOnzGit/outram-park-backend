//! Reading committed state out of a git repository, without touching its
//! working directory.
//!
//! # Why committed state, and why not a checkout
//!
//! The measurement has to be reproducible by someone who is not the author, so
//! it reads a **named ref**, never a working tree — a repository the author has
//! open may carry uncommitted work, and counting that would make the figure
//! unreproducible the moment they saved a file.
//!
//! The Python this ports from ran `git archive` into a temporary directory and
//! walked that. This reads the tree directly instead, with `git ls-tree` for
//! the file list and a single `git cat-file --batch` for the contents. Two
//! processes per repository, no temporary directory, no tar dependency, and
//! nothing written to disk.
//!
//! # Which ref gets measured, and why it matters
//!
//! These repositories **squash-merge into `main`**, so `main` carries a handful
//! of release commits while the development history — and therefore every
//! active-day count — lives on `develop`. Measuring `main` understates
//! `thermal_hydraulics_rs` by 135 active days. [`PREFERRED_REFS`] encodes that
//! preference order.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use super::source::{is_rust_source, under_any, LineCount};

/// Which ref to measure, in order of preference.
///
/// `develop` first: see the module docs on squash-merging. `HEAD` last, as a
/// fallback for a repository following none of these conventions.
pub const PREFERRED_REFS: &[&str] = &[
    "develop",
    "origin/develop",
    "main",
    "origin/main",
    "master",
    "origin/master",
    "HEAD",
];

/// Run git in `repo` and return stdout, or an empty string if it fails.
///
/// Failure is deliberately quiet and empty rather than an error: every caller
/// here treats "no output" as "nothing to count", which is the correct reading
/// for a missing ref or a directory that is not a repository.
pub fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git").arg("-C").arg(repo).args(args).output();
    match output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => String::new(),
    }
}

/// Does `reference` resolve to a commit in `repo`?
pub fn ref_exists(repo: &Path, reference: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--verify", "--quiet"])
        .arg(format!("{reference}^{{commit}}"))
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// The first of [`PREFERRED_REFS`] that resolves in this checkout.
pub fn select_ref(repo: &Path) -> String {
    for reference in PREFERRED_REFS {
        if ref_exists(repo, reference) {
            return (*reference).to_string();
        }
    }
    "HEAD".to_string()
}

/// Calendar dates (`YYYY-MM-DD`) carrying at least one commit on `reference`.
///
/// A [`BTreeSet`] so the result is ordered and set operations across
/// repositories are deterministic. Counting **distinct dates** rather than
/// commits is the point: a day the author committed is a day worked, whether
/// that day carried one commit or thirty.
pub fn active_days(
    repo: &Path,
    reference: &str,
    since: Option<&str>,
    until: Option<&str>,
) -> BTreeSet<String> {
    let mut args: Vec<String> = vec![
        "log".to_string(),
        "--format=%ad".to_string(),
        "--date=short".to_string(),
        reference.to_string(),
    ];
    if let Some(since) = since {
        args.push(format!("--since={since}"));
    }
    if let Some(until) = until {
        args.push(format!("--until={until}"));
    }
    let borrowed: Vec<&str> = args.iter().map(String::as_str).collect();
    git(repo, &borrowed)
        .split_whitespace()
        .filter(|d| !d.is_empty())
        .map(str::to_string)
        .collect()
}

/// Commit date of `reference`, as `YYYY-MM-DD`.
pub fn head_date(repo: &Path, reference: &str) -> String {
    git(
        repo,
        &["log", "-1", "--format=%ad", "--date=short", reference],
    )
    .trim()
    .to_string()
}

/// Repository-relative paths of every Rust source file in the tree at
/// `reference`, excluding [`SKIP_DIRS`](super::source::SKIP_DIRS).
///
/// `prefix` restricts the listing to a subtree (e.g. `crates/tampines`); pass
/// an empty string for the whole tree.
pub fn rust_files_at(repo: &Path, reference: &str, prefix: &str) -> Vec<String> {
    let spec = if prefix.is_empty() {
        reference.to_string()
    } else {
        format!("{reference}:{}", prefix.trim_end_matches('/'))
    };
    git(repo, &["ls-tree", "-r", "--name-only", &spec])
        .lines()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .filter(|p| is_rust_source(p))
        .map(str::to_string)
        .collect()
}

/// Count the Rust source in the tree at `reference`, under `prefix`, excluding
/// any path under `exclude`.
///
/// `exclude` holds paths **relative to `prefix`**, matching how a crate's
/// differently-classified subtree is named.
///
/// # How the contents are read
///
/// One `git cat-file --batch` process for the whole listing. Requests are
/// written from a separate thread while this one reads responses — writing them
/// all first would fill the pipe buffer and deadlock against a backed-up stdout
/// on any repository of real size.
pub fn count_tree(
    repo: &Path,
    reference: &str,
    prefix: &str,
    exclude: &BTreeSet<String>,
) -> LineCount {
    let paths: Vec<String> = rust_files_at(repo, reference, prefix)
        .into_iter()
        .filter(|p| exclude.is_empty() || !under_any(p, exclude))
        .collect();

    if paths.is_empty() {
        return LineCount::default();
    }

    // The object spec is `<rev>:<path>`, with ONE colon. Joining a prefix with
    // a second colon (`<rev>:crates/foo:src/lib.rs`) is not a spec git
    // recognises, and it fails per-object rather than loudly -- every crate
    // silently counts zero while the listing above still looks right. The
    // baseline repositories hid this because their prefix is empty.
    let base = if prefix.is_empty() {
        format!("{reference}:")
    } else {
        format!("{reference}:{}/", prefix.trim_end_matches('/'))
    };

    let mut child = match Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return LineCount::default(),
    };

    // Feed requests from a worker thread; read responses here. Doing both from
    // one thread deadlocks once either pipe buffer fills.
    let mut stdin = child.stdin.take().expect("stdin was piped");
    let requests: Vec<String> = paths.iter().map(|p| format!("{base}{p}")).collect();
    let writer = std::thread::spawn(move || {
        for request in &requests {
            if writeln!(stdin, "{request}").is_err() {
                return;
            }
        }
        let _ = stdin.flush();
        // Dropping stdin closes it, ending the batch.
    });

    let stdout = child.stdout.take().expect("stdout was piped");
    let mut count = read_batch(stdout, paths.len());

    let _ = writer.join();
    let _ = child.wait();

    // `read_batch` counts files it actually received; a truncated stream leaves
    // the count short rather than silently reporting a wrong total as complete.
    count.files = count.files.min(paths.len() as u64);
    count
}

/// Parse `git cat-file --batch` output: `<sha> <type> <size>\n<contents>\n`,
/// repeated, accumulating line counts.
///
/// Reads raw bytes because a blob need not be UTF-8; contents are decoded
/// lossily, matching the Python's `errors="replace"`.
fn read_batch(stdout: std::process::ChildStdout, expected: usize) -> LineCount {
    use std::io::Read;

    let mut reader = std::io::BufReader::new(stdout);
    let mut count = LineCount::default();
    let mut header = Vec::new();
    let mut seen = 0_usize;

    while seen < expected {
        header.clear();
        // Read the header line byte by byte: the body that follows is binary
        // and length-prefixed, so a line-buffered read would overshoot.
        let mut byte = [0_u8; 1];
        loop {
            match reader.read_exact(&mut byte) {
                Ok(()) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    header.push(byte[0]);
                }
                Err(_) => return count,
            }
        }
        let line = String::from_utf8_lossy(&header).into_owned();
        let fields: Vec<&str> = line.split_whitespace().collect();

        // `<sha> missing` for an object that is not there.
        if fields.len() < 3 {
            seen += 1;
            continue;
        }
        let size: usize = match fields[2].parse() {
            Ok(size) => size,
            Err(_) => {
                seen += 1;
                continue;
            }
        };

        let mut body = vec![0_u8; size];
        if reader.read_exact(&mut body).is_err() {
            return count;
        }
        // Trailing newline git appends after each object.
        let mut trailing = [0_u8; 1];
        let _ = reader.read_exact(&mut trailing);

        count.add_file(&String::from_utf8_lossy(&body));
        seen += 1;
    }

    count
}

/// Read one blob out of the tree at `reference`, or `None` if it is absent.
///
/// Decoded lossily: a manifest need not be valid UTF-8, and a replacement
/// character in a description is preferable to dropping the crate.
pub fn read_blob(repo: &Path, reference: &str, path: &str) -> Option<String> {
    let spec = format!("{reference}:{path}");
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["cat-file", "-p", &spec])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Immediate entry names under `path` in the tree at `reference`.
pub fn list_dir(repo: &Path, reference: &str, path: &str) -> Vec<String> {
    let spec = if path.is_empty() {
        reference.to_string()
    } else {
        format!("{reference}:{}", path.trim_end_matches('/'))
    };
    let mut names: Vec<String> = git(repo, &["ls-tree", "--name-only", &spec])
        .lines()
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string)
        .collect();
    names.sort();
    names
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Build a throwaway repository with two commits and some Rust.
    fn fixture() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let run = |args: &[&str]| {
            Command::new("git")
                .arg("-C")
                .arg(root)
                .args(args)
                .output()
                .unwrap();
        };
        run(&["init", "--quiet", "-b", "develop"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);

        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("src/lib.rs"),
            "// a comment\nfn a() {}\n\nfn b() {}\n",
        )
        .unwrap();
        // Must be excluded by SKIP_DIRS even though it is committed.
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("target/gen.rs"), "fn generated() {}\n").unwrap();
        fs::write(root.join("notes.txt"), "not rust\n").unwrap();

        run(&["add", "-A"]);
        run(&[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "first",
            "--date=2026-01-02T00:00:00",
        ]);
        tmp
    }

    #[test]
    fn the_preferred_ref_is_develop_not_main() {
        // Pins the ordering, because measuring `main` on a squash-merging
        // repository collapses years of history into a few commits.
        assert_eq!(PREFERRED_REFS[0], "develop");
        assert!(
            PREFERRED_REFS.iter().position(|r| *r == "develop").unwrap()
                < PREFERRED_REFS.iter().position(|r| *r == "main").unwrap(),
            "develop must be preferred over main"
        );
    }

    #[test]
    fn a_tree_is_counted_from_the_commit_not_the_working_directory() {
        let tmp = fixture();
        let root = tmp.path();

        // Dirty the working directory: the count must not move.
        fs::write(root.join("src/lib.rs"), "fn a() {}\nfn b() {}\nfn c() {}\n").unwrap();

        let count = count_tree(root, "develop", "", &BTreeSet::new());
        assert_eq!(
            count.files, 1,
            "target/ must be skipped, notes.txt is not Rust"
        );
        assert_eq!(count.total, 4, "the COMMITTED file has four lines");
        assert_eq!(count.code, 2, "the comment and the blank line are not code");
    }

    #[test]
    fn skipped_directories_are_excluded_even_when_committed() {
        let tmp = fixture();
        let listed = rust_files_at(tmp.path(), "develop", "");
        assert_eq!(listed, vec!["src/lib.rs".to_string()]);
    }

    #[test]
    fn active_days_are_distinct_dates() {
        let tmp = fixture();
        let days = active_days(tmp.path(), "develop", None, None);
        assert_eq!(days.len(), 1);
        assert!(days.contains("2026-01-02"), "got {days:?}");
        assert_eq!(head_date(tmp.path(), "develop"), "2026-01-02");
    }

    #[test]
    fn a_missing_ref_yields_nothing_rather_than_an_error() {
        let tmp = fixture();
        assert!(!ref_exists(tmp.path(), "no-such-ref"));
        let count = count_tree(tmp.path(), "no-such-ref", "", &BTreeSet::new());
        assert_eq!(count, LineCount::default());
    }

    /// **A prefixed subtree must actually count.** The object spec is
    /// `<rev>:<path>` with one colon; joining a prefix with a second colon
    /// yields a spec git rejects per-object, so every crate counted zero while
    /// the file listing still looked correct. The baseline repositories could
    /// not catch it, because their prefix is empty.
    #[test]
    fn counting_under_a_prefix_reads_the_files_rather_than_reporting_zero() {
        let tmp = fixture();
        let count = count_tree(tmp.path(), "develop", "src", &BTreeSet::new());
        assert_eq!(count.files, 1, "the prefixed listing must resolve to blobs");
        assert!(
            count.code > 0,
            "a prefixed subtree counted zero -- the object spec is malformed"
        );
        // And it agrees with the same files counted from the root.
        let whole = count_tree(tmp.path(), "develop", "", &BTreeSet::new());
        assert_eq!(count.code, whole.code);
        assert_eq!(count.total, whole.total);
    }

    #[test]
    fn an_excluded_subtree_is_not_counted() {
        let tmp = fixture();
        let mut exclude = BTreeSet::new();
        exclude.insert("src".to_string());
        let count = count_tree(tmp.path(), "develop", "", &exclude);
        assert_eq!(count.files, 0);
    }
}
