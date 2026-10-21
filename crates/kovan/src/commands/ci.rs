//! `kovan-cli ci` — the push CI's compile gate and test selection
//! (GitHub #314, #414, #416; maintainer decisions 2026-09-29).
//!
//! # Why this exists
//!
//! On CI, **compiling costs far more than running tests** (measured in
//! [`super::affected`]: over 150 minutes for the fast tier, almost all of it
//! release compilation). So the push job is built around compiling each crate
//! **once**:
//!
//! - **`top-crates`** prints the members no other member depends on. A single
//!   `cargo check --release --lib --tests` over them builds every library in
//!   the workspace exactly once, as a dependency (measured 2026-09-29: 15 of 46
//!   members are top-level, and with `--tests` they reach 46/46). The list is
//!   **computed on every run**, so a new top crate cannot be silently left
//!   out.
//! - **`smoke`** checks, and with `--run` runs, the short test selection in
//!   `ci/smoke-tests.toml`: copies of chosen tests placed in **top** crates, so
//!   running them reuses libraries the compile gate already built instead of
//!   rebuilding a low-level crate as a test program. The check fails if a
//!   listed test target, or the source it was copied from, no longer exists,
//!   and if a host is not a top crate. The list cannot drift silently.
//! - **`known-failures`** runs the `[[known_failure]]` entries (tests left
//!   failing on purpose, marked `#[ignore = "known failing, #NNN"]`) **by exact
//!   name**. A bare `--ignored` would also start the multi-hour tests that
//!   `#[ignore]` marks elsewhere in this workspace. CI runs this in a job that
//!   is allowed to fail, so the main job can be green again **without editing
//!   any assertion**.
//!
//! # What the compile gate does not cover (accepted, #414)
//!
//! Low-level crates' own tests, examples and benches, and features no
//! dependent enables, are not compiled on push.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command as Process;

use clap::Subcommand;

use super::affected::{members, reverse_edges};

/// Default location of the smoke list, relative to the workspace root.
pub const DEFAULT_LIST: &str = "ci/smoke-tests.toml";

/// `kovan-cli ci <subcommand>`.
#[derive(Subcommand)]
pub enum CiCommand {
    /// The workspace members no other member depends on. Prints `-p a -p b`
    /// (paste onto one `cargo check --release --lib --tests`), or one name
    /// per line with `--format list`.
    TopCrates {
        /// Workspace root (default: discovered).
        #[arg(long)]
        root: Option<PathBuf>,
        /// `cargo-args` (default) or `list`.
        #[arg(long, default_value = "cargo-args")]
        format: String,
    },
    /// Check `ci/smoke-tests.toml` against the tree, and with `--run` run
    /// every `[[smoke]]` entry (`cargo test --release -p <crate> --test
    /// <test>`).
    Smoke {
        /// Workspace root (default: discovered).
        #[arg(long)]
        root: Option<PathBuf>,
        /// The list (default: `ci/smoke-tests.toml` under the root).
        #[arg(long)]
        list: Option<PathBuf>,
        /// Run the entries after checking them.
        #[arg(long)]
        run: bool,
    },
    /// Run every `[[known_failure]]` entry by exact name, with `--ignored`.
    /// Exits non-zero if any fails. CI allows that job to fail.
    KnownFailures {
        /// Workspace root (default: discovered).
        #[arg(long)]
        root: Option<PathBuf>,
        /// The list (default: `ci/smoke-tests.toml` under the root).
        #[arg(long)]
        list: Option<PathBuf>,
    },
}

/// One `[[smoke]]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmokeEntry {
    /// Short label, e.g. `"godiva"`.
    pub name: String,
    /// Host crate. Must be a top crate.
    pub krate: String,
    /// Test target in the host (`tests/<test>.rs` or `tests/<test>/main.rs`).
    pub test: String,
    /// Repo-relative paths of the originals the copy was taken from.
    pub sources: Vec<String>,
}

/// One `[[known_failure]]` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFailure {
    /// Host crate.
    pub krate: String,
    /// Test target holding the test.
    pub test: String,
    /// The test's full path inside the target, e.g. `module::test_name`,
    /// passed to libtest with `--exact`.
    pub name: String,
    /// The GitHub issue that tracks the failure.
    pub issue: u64,
}

/// The parsed list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SmokeList {
    pub smoke: Vec<SmokeEntry>,
    pub known_failures: Vec<KnownFailure>,
}

/// Dispatch a `ci` subcommand.
pub fn run(command: CiCommand) -> Result<(), String> {
    match command {
        CiCommand::TopCrates { root, format } => {
            let root = resolve_root(root)?;
            let tops = compute_top_crates(&root)?;
            eprintln!("{} top crate(s)", tops.len());
            if format == "list" {
                for t in &tops {
                    println!("{t}");
                }
            } else {
                let args: Vec<String> = tops.iter().map(|c| format!("-p {c}")).collect();
                println!("{}", args.join(" "));
            }
            Ok(())
        }
        CiCommand::Smoke { root, list, run } => {
            let root = resolve_root(root)?;
            let parsed = load_list(&root, list.as_deref())?;
            check_list(&root, &parsed)?;
            println!(
                "smoke list OK: {} smoke entr(y/ies), {} known failure(s)",
                parsed.smoke.len(),
                parsed.known_failures.len()
            );
            if run {
                run_smoke(&root, &parsed)
            } else {
                Ok(())
            }
        }
        CiCommand::KnownFailures { root, list } => {
            let root = resolve_root(root)?;
            let parsed = load_list(&root, list.as_deref())?;
            check_list(&root, &parsed)?;
            run_known_failures(&root, &parsed)
        }
    }
}

fn resolve_root(root: Option<PathBuf>) -> Result<PathBuf, String> {
    let (root, how) = super::workspace::resolve(root.as_deref()).map_err(|e| e.to_string())?;
    eprintln!("workspace: {} ({how})", root.display());
    Ok(root)
}

/// The members of the workspace at `root` that no other member depends on.
pub fn compute_top_crates(root: &Path) -> Result<Vec<String>, String> {
    let members = members(root).map_err(|e| e.to_string())?;
    let rev = reverse_edges(root, &members).map_err(|e| e.to_string())?;
    Ok(top_crates(&members, &rev))
}

/// Pure core of [`compute_top_crates`]: a member is top-level when nothing
/// depends on it, i.e. it has no entry in the reverse-edge map (every table,
/// target-specific ones included; see [`super::affected::dependency_names`]).
pub fn top_crates(members: &[String], rev: &BTreeMap<String, BTreeSet<String>>) -> Vec<String> {
    let mut tops: Vec<String> = members
        .iter()
        .filter(|m| rev.get(m.as_str()).map_or(true, BTreeSet::is_empty))
        .cloned()
        .collect();
    tops.sort();
    tops
}

fn load_list(root: &Path, list: Option<&Path>) -> Result<SmokeList, String> {
    let path = list.map_or_else(|| root.join(DEFAULT_LIST), Path::to_path_buf);
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    parse_list(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// Parse `ci/smoke-tests.toml`. Pure, so malformed lists are testable.
pub fn parse_list(text: &str) -> Result<SmokeList, String> {
    let doc: toml::Value = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let string = |t: &toml::Value, key: &str, what: &str| -> Result<String, String> {
        t.get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("{what}: missing string `{key}`"))
    };

    let mut out = SmokeList::default();
    for (i, t) in array(&doc, "smoke").iter().enumerate() {
        let what = format!("[[smoke]] #{}", i + 1);
        let sources = t
            .get("sources")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{what}: missing array `sources`"))?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or_else(|| format!("{what}: `sources` must hold strings"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if sources.is_empty() {
            return Err(format!(
                "{what}: `sources` is empty; a smoke copy must name its original"
            ));
        }
        // `why` is required so every entry justifies its CI cost, but is not
        // carried further.
        string(t, "why", &what)?;
        out.smoke.push(SmokeEntry {
            name: string(t, "name", &what)?,
            krate: string(t, "crate", &what)?,
            test: string(t, "test", &what)?,
            sources,
        });
    }
    for (i, t) in array(&doc, "known_failure").iter().enumerate() {
        let what = format!("[[known_failure]] #{}", i + 1);
        let issue = t
            .get("issue")
            .and_then(toml::Value::as_integer)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| format!("{what}: missing issue number `issue`"))?;
        out.known_failures.push(KnownFailure {
            krate: string(t, "crate", &what)?,
            test: string(t, "test", &what)?,
            name: string(t, "name", &what)?,
            issue,
        });
    }
    Ok(out)
}

fn array<'a>(doc: &'a toml::Value, key: &str) -> Vec<&'a toml::Value> {
    doc.get(key)
        .and_then(toml::Value::as_array)
        .map(|a| a.iter().collect())
        .unwrap_or_default()
}

/// The files of test target `test` in `krate`: `tests/<test>.rs`, or every
/// `.rs` file under `tests/<test>/` when it is a directory target.
fn target_files(root: &Path, krate: &str, test: &str) -> Vec<PathBuf> {
    let tests = root.join("crates").join(krate).join("tests");
    let single = tests.join(format!("{test}.rs"));
    if single.is_file() {
        return vec![single];
    }
    let dir = tests.join(test);
    if !dir.join("main.rs").is_file() {
        return Vec::new();
    }
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "rs"))
                .collect()
        })
        .unwrap_or_default();
    files.sort();
    files
}

/// Check the list against the tree. Every problem is reported, not just the
/// first.
fn check_list(root: &Path, list: &SmokeList) -> Result<(), String> {
    let members: BTreeSet<String> = members(root)
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();
    let tops: BTreeSet<String> = compute_top_crates(root)?.into_iter().collect();
    let mut problems = Vec::new();

    for e in &list.smoke {
        let at = format!("[[smoke]] `{}`", e.name);
        if !members.contains(&e.krate) {
            problems.push(format!("{at}: `{}` is not a workspace member", e.krate));
            continue;
        }
        if !tops.contains(&e.krate) {
            problems.push(format!(
                "{at}: `{}` is not a top crate, so running its tests would rebuild it \
                 as a test program; host the copy in a top crate (#414)",
                e.krate
            ));
        }
        if target_files(root, &e.krate, &e.test).is_empty() {
            problems.push(format!(
                "{at}: no test target `{}` in crates/{}/tests/",
                e.test, e.krate
            ));
        }
        for s in &e.sources {
            if !root.join(s).exists() {
                problems.push(format!(
                    "{at}: source `{s}` no longer exists; the copy has lost its original"
                ));
            }
        }
    }

    for k in &list.known_failures {
        let at = format!("[[known_failure]] `{}` (#{})", k.name, k.issue);
        let files = target_files(root, &k.krate, &k.test);
        if files.is_empty() {
            problems.push(format!(
                "{at}: no test target `{}` in crates/{}/tests/",
                k.test, k.krate
            ));
            continue;
        }
        let text: String = files
            .iter()
            .filter_map(|f| std::fs::read_to_string(f).ok())
            .collect::<Vec<_>>()
            .join("\n");
        let func = k.name.rsplit("::").next().unwrap_or(&k.name);
        if !text.contains(&format!("fn {func}(")) {
            problems.push(format!("{at}: no `fn {func}` in the target"));
        }
        let marker = format!("known failing, #{}", k.issue);
        if !text.contains(&marker) {
            problems.push(format!(
                "{at}: no `#[ignore = \"{marker}\"]` in the target, so an ordinary run \
                 would still execute it"
            ));
        }
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "ci/smoke-tests.toml does not match the tree:\n  - {}",
            problems.join("\n  - ")
        ))
    }
}

/// `cargo test --release -p <crate> --test <test> [-- <extra>]`, inheriting
/// stdio. `Ok(true)` when it passed.
fn cargo_test(root: &Path, krate: &str, test: &str, extra: &[&str]) -> Result<bool, String> {
    let mut cmd = Process::new("cargo");
    cmd.current_dir(root)
        .args(["test", "--release", "-p", krate, "--test", test]);
    if !extra.is_empty() {
        cmd.arg("--").args(extra);
    }
    eprintln!(
        "$ cargo test --release -p {krate} --test {test} {}",
        extra.join(" ")
    );
    let status = cmd.status().map_err(|e| format!("cargo: {e}"))?;
    Ok(status.success())
}

/// Run every smoke target in ONE cargo invocation that names **every top
/// crate**, exactly the package set the compile gate
/// (`cargo test --release --no-run --lib --tests <top crates>`) builds.
///
/// # Why every top crate and not just the hosts
///
/// Cargo unifies a dependency's features across the packages named in one
/// invocation. Naming only the two hosts could resolve a shared dependency
/// with fewer features than the gate did, and that dependency would then be
/// **compiled again**, the repeat #414 exists to avoid. With the same package
/// set the features match and every artifact is reused. Cargo skips the named
/// packages that have no such `--test` target (checked 2026-09-29).
fn run_smoke(root: &Path, list: &SmokeList) -> Result<(), String> {
    let tops = compute_top_crates(root)?;
    let mut tests: Vec<&str> = Vec::new();
    for e in &list.smoke {
        if !tests.contains(&e.test.as_str()) {
            tests.push(&e.test);
        }
    }
    let mut cmd = Process::new("cargo");
    cmd.current_dir(root).args(["test", "--release"]);
    for t in &tops {
        cmd.args(["-p", t]);
    }
    for t in &tests {
        cmd.args(["--test", t]);
    }
    eprintln!(
        "$ cargo test --release -p <{} top crates> {}",
        tops.len(),
        tests
            .iter()
            .map(|t| format!("--test {t}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let status = cmd.status().map_err(|e| format!("cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("smoke tests FAILED (see the cargo output above)".to_string())
    }
}

fn run_known_failures(root: &Path, list: &SmokeList) -> Result<(), String> {
    if list.known_failures.is_empty() {
        println!("no known failures listed");
        return Ok(());
    }
    let mut still_failing = Vec::new();
    for k in &list.known_failures {
        let passed = cargo_test(root, &k.krate, &k.test, &["--ignored", "--exact", &k.name])?;
        if passed {
            println!(
                "NOW PASSING: {} (#{}): propose closing the issue and removing the \
                 `known failing` ignore, with the evidence",
                k.name, k.issue
            );
        } else {
            still_failing.push(format!("{} (#{})", k.name, k.issue));
        }
    }
    if still_failing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "known failures still failing: {}",
            still_failing.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A top crate is one nothing depends on. `mc` is depended on by `twin`,
    /// `njoy` by `mc`; `twin` and `kovan` are the tops.
    #[test]
    fn top_crates_are_the_ones_nothing_depends_on() {
        let members: Vec<String> = ["njoy", "mc", "twin", "kovan"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let mut rev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        rev.entry("njoy".into()).or_default().insert("mc".into());
        rev.entry("mc".into()).or_default().insert("twin".into());
        assert_eq!(top_crates(&members, &rev), vec!["kovan", "twin"]);
    }

    #[test]
    fn the_list_parses_both_tables() {
        let list = parse_list(
            r#"
            [[smoke]]
            name = "godiva"
            crate = "twin"
            test = "ci_smoke"
            sources = ["crates/mc/examples/godiva.rs"]
            why = "ICSBEP"

            [[known_failure]]
            crate = "twin"
            test = "ci_smoke"
            name = "godiva::slow_thing"
            issue = 301
            "#,
        )
        .expect("parses");
        assert_eq!(list.smoke.len(), 1);
        assert_eq!(list.smoke[0].krate, "twin");
        assert_eq!(list.known_failures[0].issue, 301);
    }

    /// A smoke copy that does not name its original, or does not say why it
    /// costs CI time, is rejected at parse time.
    #[test]
    fn an_entry_without_sources_or_why_is_rejected() {
        let no_sources = r#"
            [[smoke]]
            name = "x"
            crate = "twin"
            test = "ci_smoke"
            sources = []
            why = "y"
        "#;
        assert!(parse_list(no_sources).unwrap_err().contains("sources"));
        let no_why = r#"
            [[smoke]]
            name = "x"
            crate = "twin"
            test = "ci_smoke"
            sources = ["a"]
        "#;
        assert!(parse_list(no_why).unwrap_err().contains("why"));
    }
}
