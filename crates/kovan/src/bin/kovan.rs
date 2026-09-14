//! `kovan` — the GUI entry point: a thin wrapper around
//! [`kovan::digitiser::gui::run`], the digitiser window that is KOVAN's one
//! GUI surface.
//!
//! **Renamed from `kovan-gui` to plain `kovan` on 2026-08-21**, per the final
//! interface spec on GitHub issue #30: exactly three binaries —
//! `kovan` (this one, GUI), `kovan-cli` (the agent-facing CLI, formerly the
//! plain `kovan` binary — see `src/bin/kovan-cli.rs`), and `kovan-tui` (the
//! human-facing terminal UI, unchanged). Running `kovan` with no arguments is
//! meant to open the GUI; agents and scripts reach for `kovan-cli` instead.
//!
//! **The digitiser (engine + all three front ends) moved into this crate
//! from `kovan-literature` on 2026-08-21**, so that its PDF-native work
//! (GitHub issue #30) can depend on `kopitiam-pdf` without dragging
//! `kovan-literature` — used far beyond the GUI — into this crate's
//! AGPL-3.0-only relicense (see this crate's `NOTICE`). This binary used to
//! duplicate `kovan-literature`'s own `kovan-digitise-gui` bin exactly (both
//! were one-line wrappers around the same `digitiser::gui::run`); that
//! binary is retired and this is now the only name for it.
//!
//! Desktop-only by policy; Android gets a redirect message instead of a
//! window, handled inside `kovan::digitiser::gui::run` itself. `gui` is a
//! **default** feature (see `Cargo.toml`), so a plain `cargo run -p kovan
//! --bin kovan` builds it on desktop with no extra flag, while still
//! resolving to nothing on an Android target (no `--no-default-features`
//! needed there either).

fn main() -> std::process::ExitCode {
    // `kovan [image] [--root <dir>] [--paper <citekey>]`. The two flags open
    // the window directly on a library, and on one paper's PDF — the
    // scripted-startup path that makes this GUI reachable by a test or an
    // agent taking a screenshot, rather than only by hand.
    let mut image_arg = None;
    let mut startup = kovan::digitiser::gui::Startup::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => startup.root = args.next(),
            "--paper" => startup.paper = args.next(),
            other if other.starts_with("--") => {
                eprintln!("kovan: unknown option {other}");
                return std::process::ExitCode::FAILURE;
            }
            other => image_arg = Some(other.to_string()),
        }
    }
    match kovan::digitiser::gui::run(image_arg, startup) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kovan: error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
