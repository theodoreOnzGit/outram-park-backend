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

//! # HTR-10 with the SIMPLIFIED rod metal: nickel as iron, Fe-57 as Fe-56
//!
//! ```bash
//! cargo run --release -p nee_soon --example htr10_rod_metal_simplified
//! ```
//!
//! ## Treatment (maintainer decision 2026-10-01, gh:#329)
//!
//! *"Have a full case with all iron and nickel, and a simplified case. Both
//! should live as examples."* This is the simplified case
//! (`RodMetalTreatment::Simplified`), two stated modelling assumptions:
//!
//! - **Ni -> Fe, atom for atom** (maintainer 2026-09-26, "just ignore it and
//!   replace w iron"): Ni-58/60 -> Fe-56, Ni-61 -> Fe-57, Ni-62 -> Fe-54,
//!   Ni-64 -> Fe-58, which puts the replaced atoms close to natural iron's
//!   isotopics. Ni is ~9 % of the sleeve steel's atoms. No Ni tape is loaded.
//! - **Fe-57 -> Fe-56** (~2 % of natural iron, 2026-09-26), because
//!   reconstructing ENDF/B-VIII.0 Fe-57 exhausts memory (gh:#339). So Ni-61
//!   ends up as Fe-56 too. The Fe-57 tape is never read.
//!
//! Atoms are conserved: the steel and joint iron carry the same number of atoms
//! per cm3 as the full case, only the nuclide each atom is drawn from changes
//! (pinned by `htr10_rmc::data::tests`). This is equivalent to
//! `OUTRAM_HTR10_NI_AS_FE=1 OUTRAM_HTR10_FE57_AS_FE56=1 htr10_rmc_keff`.
//!
//! Everything else is the correct-physics default: ENDF/B-VIII.0, natural carbon
//! (C-12 / C-13, gh:#425), helium coolant (gh:#426), every bound thermal law.
//! The shared settings (N = 12, 14 rings, 10 000 x [30 + 100], one seed) are in
//! `examples/common/htr10_rod_metal_case.rs`; the full case
//! ([`htr10_rod_metal_full`](../htr10_rod_metal_full/index.html)) differs only
//! in the rod metal.
//!
//! ## Verification & validation
//!
//! **Methodology.** As `htr10_rod_metal_full`: `k_eff` at N = 12 against RMC at
//! the equal-ball-count height, and, once #339 is fixed, the full-minus-
//! simplified difference over pooled seeds.
//!
//! **Results.** None. Not run: no eigenvalue is computed until the HTR-10
//! geometry and materials are complete (maintainer rule). Record the result
//! here when it is measured.

#[path = "common/htr10_rod_metal_case.rs"]
mod htr10_rod_metal_case;

use nee_soon::htr10_rmc::data::RodMetalTreatment;

fn main() {
    htr10_rod_metal_case::run("htr10-rod-metal-simplified", RodMetalTreatment::Simplified);
}
