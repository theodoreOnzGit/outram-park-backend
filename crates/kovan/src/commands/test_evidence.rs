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
//! Counted runs replace `<workspace>/kovan_test_evidence.toml`; every other
//! run (partial, dirty, incomplete) goes to
//! `<workspace>/target/kovan/test_evidence_last.toml`, so it can never
//! overwrite counted evidence. `--out` overrides both.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kovan_common::review::evidence::args::plan;
use kovan_common::review::evidence::output::{LineKind, OutputParser};
use kovan_common::review::evidence::{dirty_build_inputs, TestEvidence};
use kovan_common::review::hash::sha256_tagged;

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
    Ok(sha256_tagged(&bytes))
}

/// Run `kovan-cli test` in workspace `root` with the user's `extra` cargo
/// arguments.
pub fn run(
    root: &Path,
    extra: &[String],
    allow_dirty: bool,
    out: Option<PathBuf>,
) -> Result<(), String> {
    let pl = plan(extra).map_err(|e| e.to_string())?;
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
    let scope = if pl.partial_reasons.is_empty() {
        "full suite".to_string()
    } else {
        format!("PARTIAL ({})", pl.partial_reasons.join(", "))
    };
    println!("kovan-cli test: {} [{scope}]", command.join(" "));

    // cargo's stderr (`Running …`) and the test binaries' stdout must reach
    // the parser in order, so both get the same pipe.
    let (reader, writer) = std::io::pipe().map_err(|e| format!("pipe: {e}"))?;
    let writer2 = writer.try_clone().map_err(|e| format!("pipe: {e}"))?;
    let mut child = Command::new("cargo")
        .args(&pl.args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(writer)
        .stderr(writer2)
        .spawn()
        .map_err(|e| format!("cargo: {e}"))?;
    // The parent's copies of the write end are moved into `Command` and
    // dropped with it, so the reader sees EOF when cargo exits.
    // cargo reports absolute source paths; `root` may be relative (`.`).
    let abs_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let mut parser = OutputParser::new(&abs_root.to_string_lossy());
    let stdout = std::io::stdout();
    for line in BufReader::new(reader).lines() {
        let line = line.map_err(|e| format!("reading cargo output: {e}"))?;
        if parser.feed(&line) == LineKind::Text {
            let mut o = stdout.lock();
            let _ = writeln!(o, "{line}");
            let _ = o.flush();
        }
    }
    let status = child.wait().map_err(|e| format!("cargo: {e}"))?;
    let run = parser.finish();

    dirty_paths.extend(dirty(root)?);
    let lock_after = lock_hash(root)?;
    if lock_after != lock_before {
        dirty_paths.push("Cargo.lock (changed during the run)".into());
    }
    let evidence = TestEvidence::new(
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
    let path = out.unwrap_or_else(|| root.join(evidence.destination()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let text = evidence.to_toml().map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;

    let t = evidence.totals;
    println!(
        "kovan-cli test: {} binaries, {} passed, {} failed, {} ignored at {} (Cargo.lock {})",
        evidence.binaries.len(),
        t.passed,
        t.failed,
        t.ignored,
        &evidence.commit[..evidence.commit.len().min(10)],
        &evidence.cargo_lock[..evidence.cargo_lock.len().min(19)],
    );
    match evidence.counted() {
        Ok(()) => println!("kovan-cli test: COUNTED as full-suite evidence"),
        Err(why) => {
            for w in why {
                println!("kovan-cli test: NOT counted: {w}");
            }
        }
    }
    println!("kovan-cli test: wrote {}", path.display());
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo test exited with {status}; the results are recorded in {}",
            path.display()
        ))
    }
}
