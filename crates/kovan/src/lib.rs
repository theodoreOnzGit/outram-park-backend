//! # kovan (library)
//!
//! Shared modules behind KOVAN's three front ends, which live as separate
//! binaries in this same crate:
//!
//! - **`kovan`** (agent-facing CLI, [`commands`]) — deterministic,
//!   line-oriented subcommands for a coding agent.
//! - **`kovan-tui`** (human-facing terminal UI, [`tui`]) — a `ratatui` browser
//!   over the same sibling `kovan-*` crates. Desktop-only; the whole module
//!   tree is gated off Android, and the binary compiles to a redirect stub
//!   there instead.
//! - **`kovan-gui`** — does not live here as a module: it reuses
//!   [`kovan_literature::digitiser::gui::run`] directly, since the digitiser
//!   GUI already covers this crate's one GUI surface and duplicating it would
//!   just be a second copy to keep in sync.
//!
//! This crate was consolidated from the former separate `kovan-cli` and
//! `kovan-tui` crates on 2026-08-21 so one crate carries all three interfaces
//! over the same knowledge-layer libraries — see `DECISIONS.md` for the
//! merge rationale and each front end's original design history.

pub mod commands;

#[cfg(not(target_os = "android"))]
pub mod tui;
