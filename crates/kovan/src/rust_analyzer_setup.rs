//! Is **rust-analyzer** installed, and how to install it (GitHub #820).
//!
//! The Code Map view asks this when it is first shown
//! (`app::rust_analyzer_view`), because "Index fresh" (#780) and
//! `kovan-cli index` cannot build an index without it. [`probe`] runs
//! `rust-analyzer --version` and says what it found; [`install`] runs
//! `rustup component add rust-analyzer` and probes again. Both spawn a
//! process, so the app calls them on a worker thread.
//!
//! Nothing here runs unless it is called, and [`install`] only when the
//! user asks for it: it is the one function that reaches the network.
//!
//! **A rustup proxy is not an installed rust-analyzer.** rustup puts a
//! `rust-analyzer` proxy in `~/.cargo/bin` whether or not the component is
//! in the active toolchain, so "is it on `PATH`" is not the question: the
//! proxy exits non-zero with "is not installed for the toolchain". The
//! probe therefore runs it and reads the result ([`classify`]).

use std::path::Path;
use std::process::Command;

/// The program probed.
pub const PROGRAM: &str = "rust-analyzer";

/// The install command, as run and as shown to the user.
pub const INSTALL_COMMAND: [&str; 4] = ["rustup", "component", "add", "rust-analyzer"];

/// [`INSTALL_COMMAND`] as one line of text.
pub fn install_command_line() -> String {
    INSTALL_COMMAND.join(" ")
}

/// What running `<program> --version` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunOutcome {
    /// The program is not on `PATH`.
    NotFound,
    /// It could not be started for another reason.
    NotStarted(String),
    /// It ran.
    Exited {
        success: bool,
        stdout: String,
        stderr: String,
    },
}

/// What the probe found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// Installed, with its version (`1.98.0`).
    Installed { version: String },
    /// Not on `PATH` at all.
    NotOnPath,
    /// rustup's proxy is on `PATH` but the component is not in the active
    /// toolchain; `detail` is what the proxy printed.
    NotInToolchain { detail: String },
    /// It is there but `--version` failed or printed no version.
    Failed { detail: String },
}

impl Probe {
    /// The version when installed.
    pub fn version(&self) -> Option<&str> {
        match self {
            Self::Installed { version } => Some(version),
            _ => None,
        }
    }

    /// Whether an index can be built.
    pub fn is_installed(&self) -> bool {
        matches!(self, Self::Installed { .. })
    }

    /// One sentence for the user.
    pub fn describe(&self) -> String {
        match self {
            Self::Installed { version } => format!("rust-analyzer {version} is installed."),
            Self::NotOnPath => "rust-analyzer is not installed (it is not on PATH).".into(),
            Self::NotInToolchain { detail } => format!(
                "rust-analyzer is not installed in the active Rust toolchain. rustup said: {detail}"
            ),
            Self::Failed { detail } => {
                format!("rust-analyzer is on PATH but did not report a version: {detail}")
            }
        }
    }
}

/// Read a [`RunOutcome`] (module doc). Pure, so the cases are tested without
/// a rust-analyzer or a rustup.
pub fn classify(outcome: &RunOutcome) -> Probe {
    match outcome {
        RunOutcome::NotFound => Probe::NotOnPath,
        RunOutcome::NotStarted(e) => Probe::Failed { detail: e.clone() },
        RunOutcome::Exited {
            success: true,
            stdout,
            ..
        } => match stdout.split_whitespace().nth(1) {
            // `rust-analyzer 1.98.0 (88d9e12 2026-08-18)`.
            Some(v) => Probe::Installed {
                version: v.to_string(),
            },
            None => Probe::Failed {
                detail: format!("unexpected --version output: {}", stdout.trim()),
            },
        },
        RunOutcome::Exited { stderr, .. } => {
            let detail = stderr.trim().to_string();
            if detail.contains("is not installed for the toolchain") {
                Probe::NotInToolchain { detail }
            } else {
                Probe::Failed { detail }
            }
        }
    }
}

fn run_version(program: &str, dir: Option<&Path>) -> RunOutcome {
    let mut cmd = Command::new(program);
    cmd.arg("--version");
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    match cmd.output() {
        Ok(out) => RunOutcome::Exited {
            success: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).to_string(),
            stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => RunOutcome::NotFound,
        Err(e) => RunOutcome::NotStarted(e.to_string()),
    }
}

/// Probe `program` by running `<program> --version` in `dir` (the process's
/// own directory when `None`). `dir` matters under rustup: a folder's
/// `rust-toolchain.toml` chooses the toolchain the proxy answers for.
pub fn probe_program(program: &str, dir: Option<&Path>) -> Probe {
    classify(&run_version(program, dir))
}

/// Probe rust-analyzer ([`probe_program`]).
pub fn probe(dir: Option<&Path>) -> Probe {
    probe_program(PROGRAM, dir)
}

/// Whether kovan can install rust-analyzer itself: only through rustup.
pub fn rustup_on_path() -> bool {
    which::which(INSTALL_COMMAND[0]).is_ok()
}

/// Run `installer args…` in `dir`, then probe `program` again. `Ok` only
/// when the probe then finds it installed; every other outcome is an `Err`
/// that says what happened.
pub fn install_with(
    installer: &str,
    args: &[&str],
    program: &str,
    dir: Option<&Path>,
) -> Result<Probe, String> {
    let line = format!("{installer} {}", args.join(" "));
    let mut cmd = Command::new(installer);
    cmd.args(args);
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("could not run `{line}`: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "`{line}` failed ({}): {}",
            out.status,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    match probe_program(program, dir) {
        p @ Probe::Installed { .. } => Ok(p),
        p => Err(format!(
            "`{line}` succeeded, but {program} still does not run: {}",
            p.describe()
        )),
    }
}

/// Install rust-analyzer with [`INSTALL_COMMAND`] and probe again
/// ([`install_with`]). Uses the network; called only when the user asks.
pub fn install(dir: Option<&Path>) -> Result<Probe, String> {
    install_with(INSTALL_COMMAND[0], &INSTALL_COMMAND[1..], PROGRAM, dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exited(success: bool, stdout: &str, stderr: &str) -> RunOutcome {
        RunOutcome::Exited {
            success,
            stdout: stdout.into(),
            stderr: stderr.into(),
        }
    }

    #[test]
    fn a_version_line_is_installed_and_a_rustup_proxy_without_the_component_is_not() {
        let p = classify(&exited(
            true,
            "rust-analyzer 1.98.0 (88d9e12 2026-08-18)\n",
            "",
        ));
        assert_eq!(p.version(), Some("1.98.0"));
        assert!(p.is_installed());

        // What rustup's proxy prints when the component is missing.
        let proxy = "error: 'rust-analyzer.exe' is not installed for the toolchain 'stable-x86_64-pc-windows-msvc'.";
        let p = classify(&exited(false, "", proxy));
        assert!(matches!(p, Probe::NotInToolchain { .. }), "{p:?}");
        assert!(!p.is_installed() && p.describe().contains("not installed for the toolchain"));

        assert_eq!(classify(&RunOutcome::NotFound), Probe::NotOnPath);
        assert!(matches!(
            classify(&exited(false, "", "boom")),
            Probe::Failed { .. }
        ));
        assert!(matches!(
            classify(&exited(true, "garbage", "")),
            Probe::Failed { .. }
        ));
        assert!(matches!(
            classify(&RunOutcome::NotStarted("denied".into())),
            Probe::Failed { .. }
        ));
    }

    /// A program that is certainly on this machine under `cargo test`.
    fn cargo() -> String {
        std::env::var("CARGO").unwrap_or_else(|_| "cargo".into())
    }

    const MISSING: &str = "kovan-no-such-program-for-the-probe-test";

    #[test]
    fn the_probe_runs_the_program_and_reports_a_missing_one() {
        assert_eq!(probe_program(MISSING, None), Probe::NotOnPath);
        // `cargo 1.90.0 (…)` has the same shape as rust-analyzer's line.
        let here = std::env::current_dir().unwrap();
        assert!(probe_program(&cargo(), Some(&here)).is_installed());
        // The real probe answers one way or the other without panicking.
        let real = probe(None);
        assert_eq!(real.is_installed(), real.version().is_some());
        assert_eq!(install_command_line(), "rustup component add rust-analyzer");
        let _ = rustup_on_path();
    }

    #[test]
    fn an_install_counts_only_when_the_program_then_runs() {
        // No network: `cargo --version` stands in for the installer.
        let c = cargo();
        let ok = install_with(&c, &["--version"], &c, None).unwrap();
        assert!(ok.is_installed());
        let e = install_with(&c, &["--version"], MISSING, None).unwrap_err();
        assert!(e.contains("still does not run"), "{e}");
        let e = install_with(&c, &["no-such-subcommand-kovan"], &c, None).unwrap_err();
        assert!(e.contains("failed"), "{e}");
        let e = install_with(MISSING, &[], &c, None).unwrap_err();
        assert!(e.contains("could not run"), "{e}");
    }
}
