//! `kovan-gui` — reuses [`kovan_literature::digitiser::gui::run`] rather than
//! implementing a second GUI: the digitiser window is KOVAN's one GUI
//! surface, so this binary is a thin wrapper exposing it from the
//! consolidated `kovan` crate alongside `kovan` (CLI) and `kovan-tui`.
//!
//! Desktop-only by policy; Android gets a redirect message instead of a
//! window, handled inside `kovan_literature::digitiser::gui::run` itself.
//! Requires the non-default `gui` feature (`cargo run -p kovan --features
//! gui --bin kovan-gui`), which enables `kovan-literature`'s `digitise-gui`
//! feature in turn.

fn main() -> std::process::ExitCode {
    let image_arg = std::env::args().nth(1);
    match kovan_literature::digitiser::gui::run(image_arg) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kovan-gui: error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
