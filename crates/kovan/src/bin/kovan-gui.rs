//! `kovan-gui` — a thin wrapper around [`kovan::digitiser::gui::run`], the
//! digitiser window that is KOVAN's one GUI surface.
//!
//! **The digitiser (engine + all three front ends) moved into this crate
//! from `kovan-literature` on 2026-08-21**, so that its PDF-native work
//! (GitHub issue #30) can depend on `kopitiam-pdf` without dragging
//! `kovan-literature` — used far beyond the GUI — into this crate's
//! AGPL-3.0-only relicense (see this crate's `NOTICE`). This binary used to
//! duplicate `kovan-literature`'s own `kovan-digitise-gui` bin exactly (both
//! were one-line wrappers around the same `digitiser::gui::run`); that
//! binary is retired and `kovan-gui` is now the only name for it.
//!
//! Desktop-only by policy; Android gets a redirect message instead of a
//! window, handled inside `kovan::digitiser::gui::run` itself. Requires the
//! non-default `gui` feature (`cargo run -p kovan --features gui --bin
//! kovan-gui`).

fn main() -> std::process::ExitCode {
    let image_arg = std::env::args().nth(1);
    match kovan::digitiser::gui::run(image_arg) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kovan-gui: error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
