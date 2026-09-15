//! `kovan-cli kloc` — the paper's productivity accounting.
//!
//! The measurement lives in
//! [`kovan_metrics::kloc`](kovan_metrics::kloc); this module is the `clap`
//! surface plus the one thing that crate deliberately does not do: **clone
//! repositories from GitHub**.
//!
//! # Why cloning is here and not in `kovan-metrics`
//!
//! `kovan-metrics` is offline by charter. Fetching from the network belongs in
//! the CLI layer, opt-in behind a flag, exactly as [`super::setup`] and
//! [`super::api_docs`] handle their own non-offline paths.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use kovan_metrics::kloc::{self, Options};

/// Run `kovan-cli kloc`.
pub fn run(
    out_dir: PathBuf,
    clone: bool,
    from_github: bool,
    fetch: bool,
    check: bool,
    no_figure: bool,
) -> io::Result<()> {
    let mut options = Options::new(out_dir);
    options.github_only = from_github;
    options.check = check;
    options.figure = !no_figure;

    if clone || from_github || fetch {
        ensure_clones(&options, fetch)?;
    }
    if from_github {
        println!(
            "Measuring GitHub clones under {} (github.com/{}).\n",
            options.vendor_dir.display(),
            kloc::GITHUB_USER
        );
    }

    let unresolved: Vec<&str> = kloc::all_repo_keys()
        .into_iter()
        .filter(|key| kloc::find_repo(&options, key).is_none())
        .collect();
    if !unresolved.is_empty() {
        println!("Repositories not found locally: {}", unresolved.join(", "));
        if !clone {
            println!(
                "Re-run with --clone to fetch them into {} , then measure.\n",
                options.vendor_dir.display()
            );
        }
    }

    println!("Measuring. This reads every .rs file in each checkout and may take a minute.\n");
    let outcome = kloc::run(&options)?;
    println!("{}", outcome.report);
    println!();
    for path in &outcome.written {
        println!("  wrote {}", path.display());
    }

    // A missing repository does not merely shrink the baseline -- an extension
    // crate subtracts its pre-agentic original, so the lines move into the
    // agentic total instead. Say so last, where it will be read.
    if !outcome.missing.is_empty() {
        println!();
        println!(
            "  !! {} repositories were missing: {}",
            outcome.missing.len(),
            outcome.missing.join(", ")
        );
        println!("  !! Their pre-agentic lines are counted as AGENTIC, inflating that");
        println!("  !! total by what the baseline loses. These numbers are not the");
        println!("  !! paper's. Re-run with --from-github.");
    }
    Ok(())
}

/// Clone any repository not already present into the vendor directory.
///
/// A **full** clone, never `--depth 1`: every active-day count comes from the
/// commit history, so a shallow clone would silently report a handful of days
/// for a repository with years of work.
fn ensure_clones(options: &Options, fetch: bool) -> io::Result<()> {
    std::fs::create_dir_all(&options.vendor_dir)?;
    let gitignore = options.vendor_dir.join(".gitignore");
    if !gitignore.exists() {
        std::fs::write(
            &gitignore,
            "# Source repositories cloned by `kovan-cli kloc`.\n\
             # Measurement inputs, not project content, and large. Do not commit them.\n\
             *\n",
        )?;
    }

    for key in kloc::all_repo_keys() {
        let target = options.vendor_dir.join(key);
        match kloc::find_repo(options, key) {
            None => {
                let url = kloc::repo_url(key);
                println!("  cloning {url}");
                let status = Command::new("git")
                    .args(["clone", "--quiet", &url])
                    .arg(&target)
                    .status();
                match status {
                    Ok(status) if status.success() => {}
                    _ => eprintln!("  ! clone failed for {key}"),
                }
            }
            Some(path) if fetch && path == target => {
                // Only ever fetch into our own clones. A checkout of the
                // author's that we merely found is never written to.
                println!("  fetching {key}");
                let _ = Command::new("git")
                    .arg("-C")
                    .arg(&target)
                    .args(["fetch", "--quiet", "--all", "--tags"])
                    .status();
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Default output directory.
pub fn default_out_dir(root: &Path) -> PathBuf {
    root.join("docs").join("kloc-accounting")
}
