//! Shared by the tests that drive a real rust-analyzer.

/// Whether `rust-analyzer` can actually be run from a directory OUTSIDE this
/// repository, which is where every one of these tests runs it (in a
/// `tempfile` crate).
///
/// **Why not `which::which("rust-analyzer")`.** With rustup, `rust-analyzer`
/// is a proxy in `~/.cargo/bin` whether or not the active toolchain has the
/// component, so `which` finds it and the test then fails ("Unknown binary
/// 'rust-analyzer'", or kovan's own "rust-analyzer is not installed") instead
/// of skipping as its Gate promises. That is the case on a machine whose
/// default toolchain lacks the component while the repo's `rust-toolchain.toml`
/// toolchain has it (a probe from inside the repo would pass, the temp-dir run
/// would not). Running `--version` from the temp dir asks the question the
/// tests need answered. Reached by every test that calls it.
pub fn rust_analyzer_runs() -> bool {
    std::process::Command::new("rust-analyzer")
        .arg("--version")
        .current_dir(std::env::temp_dir())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
