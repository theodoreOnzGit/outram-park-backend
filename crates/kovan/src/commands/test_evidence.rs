//! `kovan-cli test`: run the workspace test suite and record the result as
//! test evidence (GitHub #766; #739 D4).
//!
//! The pure half (argument plan, output parser, evidence file, what counts)
//! is [`kovan_common::review::evidence`]; this module runs the processes:
//! `git` for the commit and the dirty check, `rustc`/`cargo` for their
//! versions, and `cargo test` itself, whose output is echoed line by line
//! as it arrives (cargo's JSON build messages excepted) and parsed on the
//! way.
//!
//! # Dirty trees
//!
//! A run on a tree with uncommitted build inputs (`.rs`, `Cargo.toml`,
//! `Cargo.lock`, cargo config; tracked or untracked) does not describe its
//! commit. `kovan-cli test` **refuses to start** on such a tree, so a
//! 2.5-hour run is not wasted on results that cannot count; `--allow-dirty`
//! runs anyway and records `dirty = true`, which never counts. The tree is
//! checked again after the run (an edit or a `Cargo.lock` rewrite during the
//! run also makes it dirty).
//!
//! # Where it writes
//!
//! ~~Counted runs replace `<workspace>/kovan_test_evidence.toml`~~
//! **CORRECTED 2026-10-07** (maintainer, #766: test evidence lives in the
//! committed `kovan.toml`): a counted run is written into every
//! code-folder `kovan.toml`'s `[test_run]` ([`write_into_kovan_tomls`]);
//! every run's raw record, counted or not, goes to
//! `<workspace>/target/kovan/test_evidence_last.toml` (`--out` overrides
//! it), so a partial, dirty or incomplete run never touches `kovan.toml`.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kovan_common::review::evidence::args::{plan, Plan};
use kovan_common::review::index::FolderIndex;
use kovan_common::review::state::StateKind;
use kovan_common::review::evidence::output::{LineKind, OutputParser};
use kovan_common::review::evidence::{dirty_build_inputs, TestEvidence};

use super::index_control::{RunControl, CANCELLED};

fn capture(root: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("{program}: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "{program} {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    // `trim_end` only: a `git status --porcelain` line starts with a space.
    Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
}

fn dirty(root: &Path) -> Result<Vec<String>, String> {
    let s = capture(
        root,
        "git",
        &["status", "--porcelain", "--untracked-files=all"],
    )?;
    Ok(dirty_build_inputs(&s))
}

fn lock_hash(root: &Path) -> Result<String, String> {
    let bytes = std::fs::read(root.join("Cargo.lock"))
        .map_err(|e| format!("{}: {e}", root.join("Cargo.lock").display()))?;
    // The same hash the stamps compare against (CRLF read as LF).
    Ok(crate::stamping::lock_bytes_hash(&bytes))
}

/// Write a counted run into every code-folder `kovan.toml` of the workspace
/// (`[test_run]`, restricted to each folder's tests:
/// [`kovan_common::code_index::test_run::write_counted`]). Returns how many
/// `kovan.toml` files hold it. Every file is computed before any is written.
pub fn write_into_kovan_tomls(root: &Path, evidence: &TestEvidence) -> Result<usize, String> {
    use kovan_common::review::index::FolderIndex;
    let mut found: Vec<(PathBuf, FolderIndex)> = Vec::new();
    for p in kovan_discovery::discover(root, &["toml"]) {
        if p.file_name().and_then(|f| f.to_str()) != Some(super::index::KOVAN_TOML) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let Ok(fi) = FolderIndex::parse(&text) else { continue };
        // Only an index written for the folder it is in (not a fixture copy).
        let dir = p
            .parent()
            .and_then(|d| d.strip_prefix(root).ok())
            .map(|d| d.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        if fi.dir == dir {
            found.push((p, fi));
        }
    }
    // A `-p` run (#1049) rewrites only its crates' files.
    found.retain(|(_, f)| kovan_common::code_index::test_run::in_scope(evidence, f));
    let mut folders: Vec<FolderIndex> = found.iter().map(|(_, f)| f.clone()).collect();
    let unmapped = kovan_common::code_index::test_run::write_counted(evidence, &mut folders)
        .map_err(|why| why.iter().map(|w| w.to_string()).collect::<Vec<_>>().join("; "))?;
    if !unmapped.is_empty() {
        println!(
            "kovan-cli test: {} test name(s) did not map to one test id (not in any kovan.toml, or ambiguous); run `kovan-cli index` if tests were added",
            unmapped.len()
        );
    }
    let mut texts = Vec::new();
    for ((p, _), fi) in found.iter().zip(&folders) {
        texts.push((p.clone(), fi.to_toml().map_err(|e| e.to_string())?));
    }
    for (p, t) in &texts {
        std::fs::write(p, t).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    println!("kovan-cli test: recorded in {} kovan.toml file(s)", texts.len());
    Ok(texts.len())
}

/// `cargo <args>` in `root`, with colour off: its output is parsed, and a
/// `CARGO_TERM_COLOR=always` (as CI runners set) wrapped cargo's `Running`
/// lines in escape codes, so no test was mapped to a binary and the run was
/// recorded with nothing passed (kovan CI, macOS runner, 2026-10-22).
fn cargo_command(args: &[String], root: &Path) -> Command {
    let mut c = Command::new("cargo");
    c.args(args)
        .current_dir(root)
        .env("CARGO_TERM_COLOR", "never");
    c
}

/// Run `kovan-cli test` in workspace `root` with the user's `extra` cargo
/// arguments, echoing to stdout.
pub fn run(
    root: &Path,
    extra: &[String],
    allow_dirty: bool,
    out: Option<PathBuf>,
) -> Result<(), String> {
    run_with(root, extra, allow_dirty, out, &RunControl::default(), print_line)
}

/// [`run`], with every line it would print handed to `say` (the CLI prints
/// it; Code Review's "Run workspace test" logs it, `app::workspace_test`).
/// Cancel on `ctl` kills cargo at its next output line and records
/// nothing: a cancelled run is not evidence.
pub fn run_with<F: FnMut(&str)>(
    root: &Path,
    extra: &[String],
    allow_dirty: bool,
    out: Option<PathBuf>,
    ctl: &RunControl,
    mut say: F,
) -> Result<(), String> {
    let pl = plan(extra).map_err(|e| e.to_string())?;
    execute(root, pl, None, allow_dirty, out, ctl, &mut say)
}

/// Prints a line to stdout (the CLI's `say`).
fn print_line(l: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{l}");
    let _ = o.flush();
}

/// `kovan-cli test --pending-stamps` (GitHub #1052): evaluate the
/// workspace's stamps, then run only the reaching tests of those pending a
/// workspace test ([`targeted_plan`]) and record the run as targeted.
/// `extra` must be empty: a targeted run chooses its own arguments.
pub fn run_pending_stamps(
    root: &Path,
    extra: &[String],
    allow_dirty: bool,
    out: Option<PathBuf>,
) -> Result<(), String> {
    run_pending_stamps_with(root, extra, allow_dirty, out, &RunControl::default(), print_line)
}

/// [`run_pending_stamps`], with every line handed to `say` and cancel on
/// `ctl`, as [`run_with`] (Code Review's "Run pending stamps' tests",
/// `app::workspace_test`).
pub fn run_pending_stamps_with<F: FnMut(&str)>(
    root: &Path,
    extra: &[String],
    allow_dirty: bool,
    out: Option<PathBuf>,
    ctl: &RunControl,
    mut say: F,
) -> Result<(), String> {
    if !extra.is_empty() {
        return Err(format!(
            "--pending-stamps takes no cargo arguments (got: {})",
            extra.join(" ")
        ));
    }
    let ws = crate::stamping::load_workspace(root)?;
    let eval = crate::stamping::evaluate_loaded(root, &ws);
    let pending: BTreeSet<String> = eval
        .evaluation
        .functions
        .values()
        .filter(|f| {
            f.reviews
                .iter()
                .any(|r| r.state.kind() == StateKind::PendingWorkspaceTest)
        })
        .map(|f| f.id.clone())
        .collect();
    let Some(tp) = targeted_plan(&ws.indexes, &pending) else {
        say("kovan-cli test --pending-stamps: no stamp is pending a workspace test with a reaching test; nothing to run");
        return Ok(());
    };
    say(&format!(
        "kovan-cli test --pending-stamps: {} pending function(s), {} reaching test(s) in {} package(s)",
        pending.len(),
        tp.targets.len(),
        tp.packages.len()
    ));
    // No `packages`: its `-p`s pick where the reaching tests live, while
    // the pending functions they speak for may be in other crates, so the
    // run is written to every folder (#1049's per-crate scope is for a
    // full run of chosen crates).
    let pl = Plan {
        args: tp.args(),
        partial_reasons: Vec::new(),
        packages: Vec::new(),
    };
    execute(root, pl, Some(tp.targets), allow_dirty, out, ctl, &mut say)
}

/// What a targeted run runs (GitHub #1052): the test ids to cover, the
/// packages holding them and the libtest name filters (each test's
/// function name, so the run is a superset of the targets, never fewer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetedPlan {
    pub targets: Vec<String>,
    pub packages: Vec<String>,
    pub filters: Vec<String>,
}

impl TargetedPlan {
    /// The arguments after `cargo`: the full run's flags over the
    /// packages, filtered by name.
    pub fn args(&self) -> Vec<String> {
        let mut a: Vec<String> = [
            "test",
            "--lib",
            "--tests",
            "--release",
            "--no-fail-fast",
            "--message-format=json-render-diagnostics",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for p in &self.packages {
            a.push("-p".into());
            a.push(p.clone());
        }
        a.push("--".into());
        a.extend(self.filters.iter().cloned());
        a
    }
}

/// The reaching tests of the functions in `pending` (by function id), with
/// the package and libtest name filter of each, from the folder indexes;
/// `None` when they have none. A reaching test no index lists as a test
/// function stays a target but adds no filter: it is not run, so the
/// engine sees it not run.
pub fn targeted_plan(indexes: &[FolderIndex], pending: &BTreeSet<String>) -> Option<TargetedPlan> {
    let mut targets: BTreeSet<String> = BTreeSet::new();
    let mut tests: BTreeMap<String, (String, String)> = BTreeMap::new();
    for idx in indexes {
        for (file, f) in idx.functions() {
            if pending.contains(&f.id) {
                targets.extend(f.reached_by.iter().cloned());
            }
            if f.test {
                let name = f.qual.rsplit("::").next().unwrap_or(&f.qual);
                let name = name.split('#').next().unwrap_or(name);
                tests.insert(
                    format!("{}::{}", idx.file_path(file), f.qual),
                    (idx.krate.clone(), name.to_string()),
                );
            }
        }
    }
    if targets.is_empty() {
        return None;
    }
    let mut packages = BTreeSet::new();
    let mut filters = BTreeSet::new();
    for t in &targets {
        if let Some((k, n)) = tests.get(t) {
            packages.insert(k.clone());
            filters.insert(n.clone());
        }
    }
    Some(TargetedPlan {
        targets: targets.into_iter().collect(),
        packages: packages.into_iter().collect(),
        filters: filters.into_iter().collect(),
    })
}

/// Run the planned `cargo test` and record it (module doc); `targets` makes
/// it a targeted run (GitHub #1052).
fn execute<F: FnMut(&str)>(
    root: &Path,
    pl: Plan,
    targets: Option<Vec<String>>,
    allow_dirty: bool,
    out: Option<PathBuf>,
    ctl: &RunControl,
    say: &mut F,
) -> Result<(), String> {
    let mut dirty_paths = dirty(root)?;
    if !dirty_paths.is_empty() && !allow_dirty {
        return Err(format!(
            "refusing to run: uncommitted build inputs ({}). The results would not \
             describe a commit. Commit them, or pass --allow-dirty to run anyway \
             (recorded as dirty, never counted).",
            dirty_paths.join(", ")
        ));
    }
    let commit = capture(root, "git", &["rev-parse", "HEAD"])?;
    let lock_before = lock_hash(root)?;
    let rustc = capture(root, "rustc", &["--version"])?;
    let cargo = capture(root, "cargo", &["--version"])?;
    let date = crate::digitiser::dataset::utc_now_iso8601();
    let mut command = vec!["cargo".to_string()];
    command.extend(pl.args.iter().cloned());
    let scope = if targets.is_some() {
        "targeted: pending stamps' reaching tests".to_string()
    } else if !pl.partial_reasons.is_empty() {
        format!("PARTIAL ({})", pl.partial_reasons.join(", "))
    } else if !pl.packages.is_empty() {
        format!("full suite of {}", pl.packages.join(", "))
    } else {
        "full suite".to_string()
    };
    say(&format!("kovan-cli test: {} [{scope}]", command.join(" ")));

    // cargo's stderr (`Running …`) and the test binaries' stdout must reach
    // the parser in order, so both get the same pipe.
    let (reader, writer) = std::io::pipe().map_err(|e| format!("pipe: {e}"))?;
    let writer2 = writer.try_clone().map_err(|e| format!("pipe: {e}"))?;
    // cargo reports absolute source paths; `root` may be relative (`.`),
    // or on Windows hold an 8.3 short name (`RUNNER~1`) that cargo would
    // echo back while the canonical root is long. Run cargo in the
    // canonical root so its paths and the parser's root are spelled alike.
    let abs_root = dunce::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let mut child = cargo_command(&pl.args, &abs_root)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(writer2)
        .spawn()
        .map_err(|e| format!("cargo: {e}"))?;
    // The parent's copies of the write end are moved into `Command` and
    // dropped with it, so the reader sees EOF when cargo exits.
    let mut parser = OutputParser::new(&abs_root.to_string_lossy());
    for line in BufReader::new(reader).lines() {
        let line = line.map_err(|e| format!("reading cargo output: {e}"))?;
        if ctl.cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CANCELLED.to_string());
        }
        if parser.feed(&line) == LineKind::Text {
            say(&line);
        }
    }
    let status = child.wait().map_err(|e| format!("cargo: {e}"))?;
    let run = parser.finish();

    dirty_paths.extend(dirty(root)?);
    let lock_after = lock_hash(root)?;
    if lock_after != lock_before {
        dirty_paths.push("Cargo.lock (changed during the run)".into());
    }
    let mut evidence = TestEvidence::new(
        commit,
        dirty_paths,
        lock_after,
        date,
        rustc,
        cargo,
        command,
        pl.partial_reasons,
        status.code(),
        run.build_ok,
        run.binaries,
    );
    evidence.packages = pl.packages.clone();
    let evidence = match targets {
        Some(t) => evidence.into_targeted(t),
        None => evidence,
    };
    let path = out.unwrap_or_else(|| root.join(evidence.destination()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = evidence.to_toml().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;

    let t = evidence.totals;
    say(&format!(
        "kovan-cli test: {} binaries, {} passed, {} failed, {} ignored at {} (Cargo.lock {})",
        evidence.binaries.len(),
        t.passed,
        t.failed,
        t.ignored,
        &evidence.commit[..evidence.commit.len().min(10)],
        &evidence.cargo_lock[..evidence.cargo_lock.len().min(19)],
    ));
    match evidence.counted() {
        Ok(()) => {
            say(&format!(
                "kovan-cli test: COUNTED as {} evidence",
                if evidence.targets.is_empty() { "full-suite" } else { "targeted" }
            ));
            let n = write_into_kovan_tomls(root, &evidence)?;
            if n == 0 {
                say(&format!(
                    "kovan-cli test: no code-folder kovan.toml found to record it in: run \
                     `kovan-cli index` first; the raw record is in {}",
                    path.display()
                ));
            }
        }
        Err(why) => {
            for w in why {
                say(&format!("kovan-cli test: NOT counted: {w}"));
            }
        }
    }
    say(&format!("kovan-cli test: wrote {}", path.display()));
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo test exited with {status}; the results are recorded in {}",
            path.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kovan_common::review::index::{FunctionIndex, ItemKind, ModuleIndex};

    fn func(id: &str, qual: &str, reached_by: &[&str], test: bool) -> FunctionIndex {
        FunctionIndex {
            id: id.into(),
            name: qual.rsplit("::").next().unwrap().into(),
            qual: qual.into(),
            item: ItemKind::Fn,
            shape: None,
            lines: [1, 2],
            hash: format!("sha256:{}", "a".repeat(64)),
            doc_hash: format!("sha256:{}", "d".repeat(64)),
            callees: Vec::new(),
            uses: Vec::new(),
            reached_by: reached_by.iter().map(|s| s.to_string()).collect(),
            test,
            index_out_of_date: false,
            physical_interface: false,
        }
    }

    fn folder(krate: &str, dir: &str, file: &str, fns: Vec<FunctionIndex>) -> FolderIndex {
        let mut idx = FolderIndex::new(krate, dir);
        idx.modules.insert(file.into(), ModuleIndex { path: "crate".into(), functions: fns });
        idx
    }

    /// GitHub #1052: the targeted plan runs the union of the pending
    /// functions' reaching tests, over the packages that hold them, filtered
    /// by each test's function name (`#k` stripped); a function not pending
    /// adds nothing; a reaching test no index knows stays a target with no
    /// filter; nothing pending gives no plan.
    #[test]
    fn the_targeted_plan_covers_the_pending_functions_reaching_tests() {
        let it = "crates/b/tests/it.rs::checks_flash";
        let unit = "crates/a/src/lib.rs::tests::unit#2";
        let gone = "crates/z/tests/gone.rs::gone";
        let indexes = vec![
            folder(
                "a",
                "crates/a/src",
                "lib.rs",
                vec![
                    func("fn:p", "flash", &[it, unit, gone], false),
                    func("fn:q", "other", &["crates/c/tests/c.rs::c"], false),
                    func("fn:u", "tests::unit#2", &[], true),
                ],
            ),
            folder("b-pkg", "crates/b/tests", "it.rs", vec![func("fn:t", "checks_flash", &[], true)]),
            folder("c", "crates/c/tests", "c.rs", vec![func("fn:c", "c", &[], true)]),
        ];
        let pending: BTreeSet<String> = ["fn:p".to_string()].into();
        let tp = targeted_plan(&indexes, &pending).unwrap();
        assert_eq!(tp.targets, vec![unit.to_string(), it.to_string(), gone.to_string()]);
        assert_eq!(tp.packages, vec!["a", "b-pkg"]);
        assert_eq!(tp.filters, vec!["checks_flash", "unit"]);
        let args = tp.args();
        assert_eq!(&args[..6], ["test", "--lib", "--tests", "--release", "--no-fail-fast", "--message-format=json-render-diagnostics"]);
        assert_eq!(&args[6..], ["-p", "a", "-p", "b-pkg", "--", "checks_flash", "unit"]);
        assert!(targeted_plan(&indexes, &BTreeSet::new()).is_none());
    }

    /// The parsed cargo run never gets colour, whatever the caller's
    /// environment says.
    #[test]
    fn the_cargo_run_has_colour_off() {
        let c = cargo_command(&["test".into(), "--release".into()], Path::new("."));
        let color = c
            .get_envs()
            .find(|(k, _)| *k == "CARGO_TERM_COLOR")
            .and_then(|(_, v)| v);
        assert_eq!(color, Some(std::ffi::OsStr::new("never")));
        assert_eq!(c.get_args().collect::<Vec<_>>(), ["test", "--release"]);
    }
}
