//! `kovan-digitise-gui` — hybrid graph digitiser (egui, graphreader-style).
//!
//! The app itself lives in [`kovan_literature::digitiser::gui`] as a reusable
//! library function; this binary is a thin wrapper around it so the same
//! window is reusable from other binaries — the consolidated `kovan` crate's
//! `kovan-gui` does exactly that instead of duplicating the app. See that
//! module's doc comment for the interaction model and provenance guarantees.

fn main() -> std::process::ExitCode {
    let image_arg = std::env::args().nth(1);
    match kovan_literature::digitiser::gui::run(image_arg) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("kovan-digitise-gui: error: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
