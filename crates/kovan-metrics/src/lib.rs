//! # KOVAN metrics — repository accounting
//!
//! Per-commit **API-token accounting** and the pre-merge **historian report**,
//! for the OUTRAM PARK workspace. This replaced `docs/historian/token_usage.py`
//! and `docs/historian/historian.py` on 2026-08-13, and both were deleted the
//! same day — this crate is now the only implementation. It exists so the
//! toolchain needs no Python interpreter, which on Windows in particular is a
//! recurring failure mode (a `python3` that resolves to a Microsoft Store alias
//! stub silently turns the git hooks into no-ops).
//!
//! ## What belongs here
//!
//! Read-mostly accounting *about* a repository: token usage attributed to
//! commits, and lines/KLOC written over a window of history. It fits KOVAN's
//! stated focus on **traceability** and **engineering reproducibility**.
//!
//! ## What does not belong here
//!
//! Anything that modifies the repository's content, any network access, and any
//! estimation. This crate reports measurements or reports nothing.
//!
//! ## Module map
//!
//! | Module | Responsibility |
//! |---|---|
//! | [`date`] | `DDMMYY` window notation and civil-date arithmetic, dependency-free |
//! | [`git`] | The git queries used here; repo discovery reuses `kovan-discovery` |
//! | [`trailer`] | The `API-Usage-*` commit trailers — parse, format, token arithmetic |
//! | [`transcript`] | Reading token usage out of the Claude Code session transcripts |
//! | [`baseline`] | The per-clone baseline that makes "since the last commit" meaningful |
//! | [`tokens`] | Write side (git hooks) and query side (history) |
//! | [`historian`] | The pre-merge-to-`main` report generator |
//!
//! ## The two rules that govern this crate
//!
//! **Never block a commit.** The write-side entry points run inside
//! `prepare-commit-msg` and `post-commit`. They swallow their own errors and
//! degrade to a zero/`source=none` trailer rather than failing. A caller in the
//! hook path must preserve that.
//!
//! **Never invent a number.** Token figures come from the session transcripts
//! and, once recorded, from the commit trailers themselves. A commit made
//! outside a Claude session honestly reads `total=0 source=none`, and a commit
//! predating the hooks honestly has no trailer at all. Neither is a gap to be
//! filled with an estimate.
//!
//! ## Example
//!
//! ```no_run
//! use kovan_metrics::{date::Date, tokens};
//!
//! // Sum what the commit trailers on `develop` recorded for August 2026.
//! let result = tokens::query(
//!     Date::parse_ddmmyy("010826").ok(),
//!     Date::parse_ddmmyy("310826").ok(),
//!     "develop",
//! );
//! println!("{} tokens over {} commits", result.grand_total, result.commits_total);
//! ```

pub mod baseline;
pub mod date;
pub mod git;
pub mod historian;
pub mod kloc;
pub mod tokens;
pub mod trailer;
pub mod transcript;
