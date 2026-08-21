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
//! - **`kovan-gui`**, **`kovan-digitise`**, **`kovan-digitise-tui`** — three
//!   front ends over [`digitiser`], KOVAN's one GUI surface plus its
//!   automatic/reviewed CLI and TUI companions. `kovan-gui`'s binary is a
//!   one-line wrapper around [`digitiser::gui::run`]; the other two carry
//!   their own logic in `src/bin/`.
//!
//! This crate was consolidated from the former separate `kovan-cli` and
//! `kovan-tui` crates on 2026-08-21 so one crate carries all three interfaces
//! over the same knowledge-layer libraries — see `DECISIONS.md` for the
//! merge rationale and each front end's original design history. The
//! [`digitiser`] module joined the same day, moved here from
//! `kovan-literature` so it can depend on `kopitiam-pdf` (this crate's own
//! AGPL-3.0-only relicense, see `NOTICE`) without dragging
//! `kovan-literature` — used well beyond the GUI — into that relicense too.

pub mod commands;
pub mod digitiser;

#[cfg(not(target_os = "android"))]
pub mod tui;
