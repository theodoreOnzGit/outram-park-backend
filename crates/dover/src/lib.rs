// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! **DOVER** — *Deck-based Open-source Visualisation Engine for Reactors*, the
//! low-fidelity counterpart of `dhoby-ghaut`.
//!
//! # What is here
//!
//! One worked low-fidelity case, run from a TOML deck, headless:
//! **steam methane reforming in a continuous stirred-tank reactor.**
//!
//! ```text
//!   TOML deck  ──►  deck::Deck (validated)  ──►  smr::SmrCase
//!                                                     │
//!                        dwsim-libs Cstr  ◄───────────┘
//!                          (Newton on the reaction extents)
//!                                │
//!                                ▼
//!                       headless::run_deck  ──►  CSV on stdout
//! ```
//!
//! ```no_run
//! use dover::headless::run_toml;
//!
//! let csv = run_toml(&std::fs::read_to_string("decks/smr_cstr.toml").unwrap()).unwrap();
//! print!("{csv}");
//! ```
//!
//! # What was reused rather than written
//!
//! Almost all of it. The workspace's "search before building" rule turned up
//! a complete reactor stack in `outram-park-fork-dwsim-libs` that this crate
//! composes rather than duplicates:
//!
//! | needed | already existed |
//! |---|---|
//! | the CSTR itself | `reactors::Cstr` — damped Newton on reaction extents, tested against `X = kτ/(1+kτ)` |
//! | reversible rate law | `reactions::Reaction::net_rate` |
//! | `K_eq(T)` from `ΔH°`/`ΔS°` | `reactions::EquilibriumConstant::GibbsVantHoff` |
//! | feed/outcome plumbing | `reactors::{ReactorFeed, ReactorOutcome}` |
//! | TOML + serde | already in the root `[workspace.dependencies]` |
//!
//! What is genuinely new here is the **deck reader** (DOVER's own
//! `CLAUDE.md` identified it as the missing piece), the **species
//! thermochemistry** ([`species`] — the workspace had critical constants for
//! three of the five species and no formation data at all), and the
//! **thermodynamic-consistency derivation** ([`smr::consistent_reverse`])
//! that `net_rate`'s independent forward/reverse pairs leave to the caller.
//!
//! # Status
//!
//! **Untrusted AI-assisted draft. No human V&V. Not declared mature.**
//! Education, research and V&V only per the workspace `RESPONSIBLE_USE.md` —
//! never process design, plant operation or a safety case.
//!
//! The model's scope and its approximations are set out in [`smr`], and
//! should be read before quoting any number it produces. In particular it is
//! **isothermal** and holds volumetric flow constant, neither of which a real
//! reformer does.

#![forbid(unsafe_code)]

pub mod deck;
pub mod headless;
pub mod smr;
pub mod species;

#[cfg(test)]
mod tests {
    /// The crate builds and links.
    #[test]
    fn builds_and_links() {
        assert_eq!(crate::deck::SCHEMA_VERSION, 1);
    }
}
