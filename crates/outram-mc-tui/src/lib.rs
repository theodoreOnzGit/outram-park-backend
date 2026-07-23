//! Library surface behind the `outram-mc-tui` binary, split out purely so the
//! headless smoke test (`tests/smoke.rs`) can drive the app state machine and
//! render path with ratatui's `TestBackend` — there is no interactive
//! terminal to click through in CI. `src/main.rs` is the real entry point
//! (terminal setup/teardown + the crossterm event loop); see its module docs
//! for how to run the app and the Android/Termux notes.

pub mod app;
pub mod presets;
pub mod settings;
pub mod spectrum;
pub mod transport;
pub mod ui;
