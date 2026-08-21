//! `kovan-cli agent-docs-gen` — bundle the workspace's API documentation into a
//! flat set of files for an external chat agent with a fixed context budget.
//!
//! The bundling logic lives in
//! [`kovan_semantics::agent_docs`](kovan_semantics::agent_docs); this module is
//! the `clap` surface, the console report, and the one opt-in path that shells
//! out to regenerate missing mirrors.
//!
//! # Why the output is flat
//!
//! The bundle exists to be uploaded to a web chat window, and those upload
//! dialogs take **files but not folders**. One file per crate, no
//! subdirectories, ever.
//!
//! # Regeneration is opt-in and is the one non-offline path
//!
//! `--regenerate-missing` calls [`super::api_docs`], which needs a nightly
//! toolchain (rustdoc's JSON output is nightly-only) and the `rustdoc-md`
//! binary, and compiles the crate. That is explicitly outside
//! KOVAN's offline/deterministic charter, so it is **off by default** and never
//! runs on its own — the same treatment [`super::setup`] gives its online
//! `cargo install` path. The default invocation reads files and writes files,
//! nothing more.

use std::io;
use std::path::{Path, PathBuf};

use kovan_semantics::agent_docs::{estimated_tokens, inventory, write_bundle, CrateEntry};

/// Default context budget, in estimated tokens.
///
/// 200,000 — the window the maintainer works against through NUS AI Know. It is
/// a *default*, overridable with `--budget`, and it is compared against an
/// **estimate** (four bytes per token), so treat a bundle that fits on paper as
/// "probably fits" rather than "fits".
const DEFAULT_BUDGET_TOKENS: u64 = 200_000;

/// Run `kovan-cli agent-docs-gen`.
///
/// `workspace_root` is the directory containing `crates/`; `out_dir` is where
/// the flat bundle is written (cleared of `*.md` first). `selected` names the
/// crate directories whose `<crate>-api.md` is copied in full — every crate with a
/// mirror still appears in the condensed `_INDEX.md` regardless.
///
/// Prints a per-file table with byte sizes and estimated tokens, a running
/// total against `budget_tokens`, and the crates that were omitted. Returns an
/// error only for genuine IO failures; being over budget is reported loudly but
/// is not an error, because the estimate is not precise enough to justify
/// refusing to write.
pub fn run(
    workspace_root: &Path,
    out_dir: &Path,
    selected: &[String],
    budget_tokens: Option<u64>,
    regenerate_missing: bool,
    list_only: bool,
) -> io::Result<()> {
    let budget = budget_tokens.unwrap_or(DEFAULT_BUDGET_TOKENS);
    let mut entries = inventory(workspace_root)?;

    if list_only {
        print_inventory(&entries);
        return Ok(());
    }

    // Existence first, so a typo is caught before anything expensive runs.
    validate_exists(&entries, selected)?;

    if regenerate_missing {
        regenerate(workspace_root, &entries, selected)?;
        // Sizes and presence changed underneath us, so re-read rather than
        // reporting stale figures.
        entries = inventory(workspace_root)?;
    }

    // The mirror check runs AFTER regeneration has had its chance -- doing it
    // before is what made `--regenerate-missing` unusable.
    validate_documented(&entries, selected)?;

    let report = write_bundle(workspace_root, out_dir, &entries, selected)?;

    println!("wrote {} ({} files)", out_dir.display(), report.files.len());
    println!();
    println!("CORE UPLOAD SET -- upload these every time:");
    println!("  {:<44} {:>12} {:>14}", "FILE", "BYTES", "EST. TOKENS");
    for (name, bytes) in &report.files {
        if name.ends_with(".index.md") {
            continue;
        }
        println!(
            "  {:<44} {:>12} {:>14}",
            name,
            bytes,
            estimated_tokens(*bytes)
        );
    }
    println!("  {}", "-".repeat(72));
    let core = report.core_estimated_tokens();
    println!(
        "  {:<44} {:>12} {:>14}",
        "core total",
        report.core_bytes(),
        core
    );
    println!();

    if report.exceeds_budget(budget) {
        println!(
            "OVER BUDGET: the core set alone is ~{core} estimated tokens against \
             a {budget} budget (over by ~{}).",
            core - budget
        );
        println!("  Drop a crate from --crates, or raise --budget if you know it fits.");
    } else {
        let headroom = budget - core;
        println!("Core set fits: ~{core} of {budget} estimated tokens, ~{headroom} headroom.");

        let optional = report.optional_files();
        if !optional.is_empty() {
            println!();
            println!(
                "OPTIONAL -- condensed indexes for the {} crates whose full docs are NOT",
                optional.len()
            );
            println!("in the core set. Add as many as the headroom allows, smallest first:");
            println!("  {:<44} {:>12} {:>14}", "FILE", "BYTES", "EST. TOKENS");
            let mut spent = 0_u64;
            let mut fit = 0_usize;
            for (name, bytes) in &optional {
                let cost = estimated_tokens(*bytes);
                let marker = if spent + cost <= headroom {
                    spent += cost;
                    fit += 1;
                    "  fits"
                } else {
                    "  over"
                };
                println!("  {name:<44} {bytes:>12} {cost:>14}{marker}");
            }
            println!("  {}", "-".repeat(72));
            println!(
                "  {fit} of {} optional files fit in the headroom (~{spent} tokens).",
                optional.len()
            );
        }
    }
    println!();
    println!(
        "  These are ESTIMATES at {} bytes/token, not measurements, and generated",
        kovan_semantics::agent_docs::BYTES_PER_ESTIMATED_TOKEN
    );
    println!("  API markdown tokenizes worse than prose -- expect the true figure to be higher.");

    if !report.missing_api_docs.is_empty() {
        println!();
        println!(
            "{} crates have no docs/<crate>-api.md and appear nowhere in the bundle:",
            report.missing_api_docs.len()
        );
        for name in &report.missing_api_docs {
            println!("  {name}");
        }
        println!("  AGENTS.md names them, so the agent is told not to invent their APIs.");
        println!("  Run with --regenerate-missing to generate them (needs nightly + rustdoc-md).");
    }

    Ok(())
}

/// Print the crate inventory without writing anything (`--list`).
///
/// This is the command to run first: it shows which crates have a mirror and
/// what each would cost, so a selection can be made against the budget before
/// any file is written.
fn print_inventory(entries: &[CrateEntry]) {
    println!(
        "  {:<44} {:>12} {:>14}",
        "CRATE", "API BYTES", "EST. TOKENS"
    );
    let mut documented = 0_usize;
    let mut total = 0_u64;
    for entry in entries {
        if entry.has_api_docs() {
            documented += 1;
            total += entry.api_bytes;
            println!(
                "  {:<44} {:>12} {:>14}",
                entry.directory,
                entry.api_bytes,
                estimated_tokens(entry.api_bytes)
            );
        } else {
            println!("  {:<44} {:>12} {:>14}", entry.directory, "-", "-");
        }
    }
    println!("  {}", "-".repeat(72));
    println!(
        "  {} crates, {} with API docs, {} bytes, ~{} estimated tokens if bundled whole",
        entries.len(),
        documented,
        total,
        estimated_tokens(total)
    );
}

/// Reject a selection naming a crate that does not exist under `crates/`.
///
/// Failing here rather than silently skipping matters: a typo in `--crates`
/// would otherwise produce a bundle quietly missing the very crate the user
/// wanted, and they would not find out until the agent started guessing.
///
/// **Existence only** — this deliberately does not check for a mirror, because
/// it runs before `--regenerate-missing` has had its chance to create one. See
/// [`validate_documented`], which is the half that runs afterwards.
fn validate_exists(entries: &[CrateEntry], selected: &[String]) -> io::Result<()> {
    for name in selected {
        if !entries.iter().any(|e| &e.directory == name) {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "no crate directory `{name}` under crates/ -- run \
                     `kovan-cli agent-docs-gen --list` to see the available names"
                ),
            ));
        }
    }
    Ok(())
}

/// Reject a selection naming a crate that still has no `docs/<crate>-api.md`.
///
/// # Why this is separate from [`validate_exists`]
///
/// The two checks were originally one function running before regeneration,
/// which made `--regenerate-missing` **impossible to use**: it rejected the
/// crate for lacking exactly the file the flag exists to create, and returned
/// before `regenerate` was ever reached. The flag was dead on arrival and
/// shipped that way on 2026-08-14, because it was described as untestable on
/// this host when in fact nightly, `rustdoc-md` and python3 were all installed.
/// Splitting the check around regeneration is the fix; running the command is
/// what found it.
fn validate_documented(entries: &[CrateEntry], selected: &[String]) -> io::Result<()> {
    for name in selected {
        let missing = entries
            .iter()
            .any(|e| &e.directory == name && !e.has_api_docs());
        if missing {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "crate `{name}` has no docs/<crate>-api.md -- generate it with \
                     `--regenerate-missing` (needs a nightly toolchain and \
                     rustdoc-md), or drop it from --crates"
                ),
            ));
        }
    }
    Ok(())
}

/// Generate `docs/<crate>-api.md` for selected crates that lack one.
///
/// **Not offline and not deterministic** — see this module's header. Restricted
/// to the crates named in `selected`, not every crate missing a mirror:
/// regenerating all of them would add megabytes to a corpus already several
/// times over budget, and nothing would use them.
///
/// Calls [`super::api_docs::generate`] **in process**. It used to spawn
/// `python3 scripts/gen_api_docs.py`; that script was ported to Rust and retired
/// on 2026-08-14, so there is no longer an interpreter in the chain.
fn regenerate(
    workspace_root: &Path,
    entries: &[CrateEntry],
    selected: &[String],
) -> io::Result<()> {
    for name in selected {
        let Some(entry) = entries.iter().find(|e| &e.directory == name) else {
            continue;
        };
        if entry.has_api_docs() {
            continue;
        }
        println!("regenerating docs/{name}-api.md for {name} (nightly rustdoc + rustdoc-md)...");
        let path = super::api_docs::generate(workspace_root, name, false)?;
        println!("  wrote {}", path.display());
    }
    Ok(())
}

/// Where the bundle is written, in order of preference.
///
/// Delegates to [`super::workspace::output_dir`]: an explicit `--out` wins,
/// then the workspace (`<workspace>/agent-docs`, which the repository's
/// `.gitignore` already covers), then `~/Documents/agent-docs`, then
/// `~/agent-docs`.
pub fn resolve_out_dir(explicit: Option<&Path>) -> io::Result<(PathBuf, String)> {
    super::workspace::output_dir(explicit, "agent-docs")
}
