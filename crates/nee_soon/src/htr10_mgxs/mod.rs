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

//! # HTR-10 MGXS generation cases (gh:#845)
//!
//! Named, fixed Monte Carlo runs that condense HTR-10 multigroup cross
//! sections, built so a long run can be handed to a machine with cores to
//! spare and nothing else: one command per case, a record written to disk,
//! nothing to decide while it runs. The runbook is
//! `crates/nee_soon/docs/htr10-mgxs-desktop-runbook.md`.
//!
//! Orchestration only, as everywhere in this crate: the transport is
//! `outram_mc_libs`' hybrid delta/surface tracker, the model is
//! [`crate::htr10_rmc`], and the condensation is [`crate::mgxs`].
//!
//! ## What a case does
//!
//! 1. Process the nuclear data at the case temperature
//!    ([`crate::htr10_rmc::data`], ENDF/B-VIII.0, every material at that
//!    temperature: an **isothermal** state point).
//! 2. Assemble the model ([`cases::CaseModel`]). Today that is the full core,
//!    Şeker & Çolak's 13-ball bed with explicit TRISO, the geometry every
//!    recorded HTR-10 `k` in this crate is on.
//! 3. Run two k-eigenvalue passes at one seed, the scalar and the
//!    scattering-matrix tallies of [`crate::mgxs`], per material, on the
//!    **70-group tally structure** of [`groups`] (WIMS-69 plus 10-20 MeV).
//! 4. Condense, then [`crate::mgxs::MgxsLibrary::collapse`] onto every
//!    structure of [`groups::Collapse`]. Collapsing is exact, so one run gives
//!    2, 4, 8 and 70 groups at once.
//! 5. Write [`record::CaseRecord`]: `record.json` (everything, including the
//!    fine library), `summary.csv` (one row) and `README.md` (methodology and
//!    results, generated from the record, so the V&V rule's two halves cannot
//!    drift apart).
//!
//! ## What a case does NOT do yet
//!
//! - **No multigroup `k_eff`.** The gate of #850 needs the converter to
//!   `physics_mg` (#848) and the scattering multiplicity (#682). The saved
//!   library is what that gate will read, so these runs are not repeated
//!   when it lands.
//! - **No uncertainty on the collapsed constants.** Tallies keep per-bin
//!   moments, not the covariance between the fine bins of one batch, so the
//!   σ of a merged group is not recoverable (see [`crate::mgxs`]). The
//!   replica-seed case measures the spread directly instead.
//! - **No separate fuel and moderator temperatures** (#852): every material
//!   is at the case temperature.

pub mod cases;
pub mod groups;
pub mod record;
pub mod run;

pub use cases::{CaseModel, MgxsCase};
pub use groups::Collapse;
pub use record::{write_index, CaseRecord, Provenance};
pub use run::{run_case, CaseError};
