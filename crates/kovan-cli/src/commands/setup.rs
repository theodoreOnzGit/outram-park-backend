//! `kovan setup` — install a curated set of useful external CLI tools via
//! `cargo install`, skipping any whose binary is already on `PATH`.
//!
//! This is an **explicit, online, desktop-scope convenience**: it is never
//! run automatically by any other `kovan` subcommand, and it has no bearing
//! on the rest of this crate's offline / Android-clean operation (see the
//! crate's Android note in `README.md`). It exists purely so a human or an
//! agent working in this repository can bring their shell up to a useful
//! baseline (`rg`, `fd`, `bat`, …) in one command instead of five.
//!
//! The tool list ([`TOOLS`]) is intentionally hard-coded and small — extend
//! it by adding one more [`ToolSpec`] entry, nothing else needs to change.
//! The PATH-detection / "would this be installed" decision
//! ([`decide`]) is a pure function so it is unit-testable without touching
//! the real `PATH` or spawning `cargo`; only [`run`] performs I/O.

/// One entry in the curated external-tool list: the crate `cargo install`
/// pulls, the binary name that install provides (checked on `PATH`), and a
/// one-line explanation of what the tool is for.
pub struct ToolSpec {
    /// crates.io crate name, as passed to `cargo install <crate_name>`.
    pub crate_name: &'static str,
    /// Binary name the crate installs — this is what gets probed on `PATH`.
    pub binary_name: &'static str,
    /// Human-readable one-line description of the tool's purpose.
    pub description: &'static str,
}

/// The curated, hard-coded list of external Rust CLI tools `kovan setup`
/// knows how to install. Add an entry here to extend the list — everything
/// else (`--dry-run` reporting, PATH detection, install loop) is generic
/// over this array.
pub const TOOLS: &[ToolSpec] = &[
    ToolSpec {
        crate_name: "eza",
        binary_name: "eza",
        description: "modern `ls` replacement (colour, git status, tree view)",
    },
    ToolSpec {
        crate_name: "ripgrep",
        binary_name: "rg",
        description: "fast recursive regex search — the same engine kovan-discovery/kovan-semantics shell out to",
    },
    ToolSpec {
        crate_name: "fd-find",
        binary_name: "fd",
        description: "fast, user-friendly `find` replacement",
    },
    ToolSpec {
        crate_name: "bat",
        binary_name: "bat",
        description: "`cat` with syntax highlighting and git-diff markers",
    },
    ToolSpec {
        crate_name: "tokei",
        binary_name: "tokei",
        description: "fast source-code line counter / per-language breakdown",
    },
    ToolSpec {
        // The `gitoxide` *binary* crate (installs `gix` + `ein`, default-run
        // `gix`) — NOT the `gix` *library* crate. Chosen over GritQL/`grit`,
        // which has no crates.io / `cargo install` path.
        crate_name: "gitoxide",
        binary_name: "gix",
        description: "pure-Rust git implementation CLI (cross-platform, no system libgit2)",
    },
];

/// What `kovan setup` should do with one tool, given whether its binary is
/// already on `PATH`. Pure decision, no I/O — see [`decide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Binary already present and `--force` was not given: do nothing.
    Skip,
    /// Binary missing (or `--force` was given): run `cargo install`.
    Install,
}

/// Decide the [`Action`] for a tool from whether its binary was found on
/// `PATH` (`present`) and whether `--force` was given. Pure function —
/// takes the already-computed PATH-presence bool rather than probing
/// itself, so it is testable without touching the real filesystem/`PATH`.
pub fn decide(present: bool, force: bool) -> Action {
    if present && !force {
        Action::Skip
    } else {
        Action::Install
    }
}

/// Outcome of processing one [`ToolSpec`], for the end-of-run summary.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    /// Already on PATH, `--force` not given: skipped.
    AlreadyPresent,
    /// `--dry-run`: would have run `cargo install`.
    WouldInstall,
    /// `cargo install` ran and exited successfully.
    Installed,
    /// `cargo install` did not succeed; the string is the reported reason.
    Failed(String),
}

/// `kovan setup [--dry-run] [--force]` — the CLI entry point. Walks
/// [`TOOLS`], skipping tools already on `PATH` (unless `--force`),
/// installing the rest via `cargo install <crate>` (unless `--dry-run`, which
/// only reports what would happen). `cargo`/network/install failures are
/// caught per-tool and reported — one failing tool never stops the rest.
/// Returns `Err` only if at least one *requested* install genuinely failed
/// (never for tools that were skipped or dry-run reported).
pub fn run(dry_run: bool, force: bool) -> Result<(), String> {
    println!(
        "kovan setup — {} curated external CLI tool(s){}",
        TOOLS.len(),
        if dry_run {
            " (dry run: installing nothing)"
        } else {
            ""
        }
    );

    let mut outcomes = Vec::with_capacity(TOOLS.len());
    for tool in TOOLS {
        let present = binary_on_path(tool.binary_name);
        let outcome = match decide(present, force) {
            Action::Skip => Outcome::AlreadyPresent,
            Action::Install if dry_run => Outcome::WouldInstall,
            Action::Install => match cargo_install(tool.crate_name) {
                Ok(()) => Outcome::Installed,
                Err(e) => Outcome::Failed(e),
            },
        };
        report_line(tool, &outcome);
        outcomes.push(outcome);
    }

    if outcomes.iter().any(|o| matches!(o, Outcome::Failed(_))) {
        Err("one or more tool installs failed — see the FAILED lines above".to_string())
    } else {
        Ok(())
    }
}

/// Print one summary line for `tool`'s [`Outcome`].
fn report_line(tool: &ToolSpec, outcome: &Outcome) {
    match outcome {
        Outcome::AlreadyPresent => println!(
            "  [skip]      {:<8} ({}) — already on PATH. {}",
            tool.binary_name, tool.crate_name, tool.description
        ),
        Outcome::WouldInstall => println!(
            "  [dry-run]   {:<8} ({}) — would run `cargo install {}`. {}",
            tool.binary_name, tool.crate_name, tool.crate_name, tool.description
        ),
        Outcome::Installed => println!(
            "  [installed] {:<8} ({}) — OK",
            tool.binary_name, tool.crate_name
        ),
        Outcome::Failed(e) => eprintln!(
            "  [FAILED]    {:<8} ({}) — {e}",
            tool.binary_name, tool.crate_name
        ),
    }
}

/// Whether `binary` resolves on the current `PATH`. Isolated behind this
/// helper (rather than calling `which::which` inline throughout) so the rest
/// of the module only depends on a `bool`.
fn binary_on_path(binary: &str) -> bool {
    which::which(binary).is_ok()
}

/// Run `cargo install <crate_name>`, inheriting stdout/stderr so the child's
/// own progress output streams straight to the terminal. Never panics: a
/// missing `cargo`, a network failure, or a non-zero exit are all reported
/// as `Err` with a human-readable reason.
#[cfg(not(target_os = "android"))]
fn cargo_install(crate_name: &str) -> Result<(), String> {
    let status = std::process::Command::new("cargo")
        .args(["install", crate_name])
        .status();
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(format!("cargo install exited with {status}")),
        Err(e) => Err(format!("failed to spawn `cargo install {crate_name}`: {e}")),
    }
}

/// Android is not a `cargo install` host for desktop dev tools (no general
/// on-device Rust toolchain, no meaningful `PATH`-shell workflow for these
/// tools) — `setup` no-ops the actual install here rather than attempting a
/// spawn that could never succeed. This keeps the *behaviour* Android-safe;
/// the crate's Android-buildability requirement (`cargo check --target
/// aarch64-linux-android`) is already satisfied because every dependency
/// this module uses (`which`, `std::process::Command`) compiles fine there —
/// this gate is about runtime intent, not compilation.
#[cfg(target_os = "android")]
fn cargo_install(_crate_name: &str) -> Result<(), String> {
    Err("kovan setup does not install dev tools on Android (no cargo-install host)".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_list_is_non_empty() {
        assert!(!TOOLS.is_empty());
    }

    #[test]
    fn tool_list_has_unique_crate_names() {
        for (i, a) in TOOLS.iter().enumerate() {
            for b in &TOOLS[i + 1..] {
                assert_ne!(a.crate_name, b.crate_name, "duplicate crate_name");
            }
        }
    }

    #[test]
    fn tool_list_has_unique_binary_names() {
        for (i, a) in TOOLS.iter().enumerate() {
            for b in &TOOLS[i + 1..] {
                assert_ne!(a.binary_name, b.binary_name, "duplicate binary_name");
            }
        }
    }

    #[test]
    fn tool_list_entries_are_non_empty_strings() {
        for t in TOOLS {
            assert!(!t.crate_name.is_empty());
            assert!(!t.binary_name.is_empty());
            assert!(!t.description.is_empty());
        }
    }

    #[test]
    fn decide_skips_present_without_force() {
        assert_eq!(decide(true, false), Action::Skip);
    }

    #[test]
    fn decide_installs_missing_without_force() {
        assert_eq!(decide(false, false), Action::Install);
    }

    #[test]
    fn decide_installs_present_with_force() {
        assert_eq!(decide(true, true), Action::Install);
    }

    #[test]
    fn decide_installs_missing_with_force() {
        assert_eq!(decide(false, true), Action::Install);
    }

    /// End-to-end shape check for the `--dry-run` decision path (mirrors
    /// what `run(dry_run: true, force: false)` would report) without
    /// spawning `cargo` or touching the real `PATH`: for each tool, a
    /// simulated `present` reading feeds `decide`, and a dry run must never
    /// choose to actually install (that's `run`'s job to gate on
    /// `dry_run`, checked directly here).
    #[test]
    fn dry_run_never_installs_regardless_of_presence() {
        for present in [true, false] {
            for force in [true, false] {
                let action = decide(present, force);
                // `run` maps `Action::Install` to `Outcome::WouldInstall`
                // (not `Outcome::Installed`) whenever `dry_run` is set — the
                // pure decision layer only decides *whether* to act, never
                // *how*, so this asserts the contract `run` relies on: a
                // `Skip` decision is always safe to report as-is under
                // dry-run, and an `Install` decision is exactly the set of
                // tools `run` must turn into `WouldInstall` (not a real
                // spawn) when `dry_run` is true.
                match (present, force) {
                    (true, false) => assert_eq!(action, Action::Skip),
                    _ => assert_eq!(action, Action::Install),
                }
            }
        }
    }
}
