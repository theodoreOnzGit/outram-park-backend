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

//! # HTR-10 with the FULL rod metal: real nickel and real iron
//!
//! ```bash
//! git submodule update --init reference-data/ace   # the Ni tapes
//! cargo run --release -p nee_soon --example htr10_rod_metal_full
//! ```
//!
//! **BLOCKED on gh:#339 (2026-10-01).** This case loads ENDF/B-VIII.0 Fe-57,
//! whose reconstruction (LRF=7, three particle pairs) exhausted 13.4 GB and was
//! OOM-killed. Until #339 is fixed the shared loader refuses the case before it
//! reads any tape and this example prints `REFUSED: BLOCKED on gh:#339 ...`.
//! It does **not** fall back to Fe-56: that is the simplified case, which is a
//! separate example ([`htr10_rod_metal_simplified`](../htr10_rod_metal_simplified/index.html)).
//! Whoever fixes #339 flips `nee_soon::htr10_rmc::data::FE57_RECONSTRUCTION_FIXED`.
//!
//! ## Treatment (maintainer decision 2026-10-01, gh:#329)
//!
//! *"Have a full case with all iron and nickel, and a simplified case. Both
//! should live as examples."* This is the full case, and it is also the
//! library default (`RodMetalTreatment::Full`):
//!
//! - rod sleeve steel, TECDOC-1382 § 4.1.1.5 (7.9 g/cm3; Cr 18, Fe 68.1, Ni 10,
//!   Si 1, Mn 2, C 0.1, Ti 0.8 wt.%), every element split into its natural
//!   isotopes (IUPAC, Meija et al. 2016): **Ni-58/60/61/62/64** and
//!   **Fe-54/56/57/58** as themselves;
//! - joint and end iron, 0.04 atoms/(b cm), Fe-54/56/57/58;
//! - Ni tapes from the `reference-data/ace` submodule
//!   (`ace/endf/endf-b-viii.0/`, listed in `ace/endf/MANIFEST.tsv`); every other
//!   tape from `reference-data/endf/`.
//!
//! Everything else is the correct-physics default: ENDF/B-VIII.0, natural carbon
//! (C-12 / C-13, gh:#425), helium coolant (gh:#426), every bound thermal law.
//! The shared settings (N = 12, 14 rings, 10 000 x [30 + 100], one seed) are in
//! `examples/common/htr10_rod_metal_case.rs`.
//!
//! ## Verification & validation
//!
//! **Methodology.** `k_eff` of the HTR-10 first-criticality core at N = 12
//! against Li, Yu & Wei (2014)'s RMC curve read at the equal-ball-count height
//! (`rmc_keff_at_ball_count`). The quantity of interest is the difference
//! against `htr10_rod_metal_simplified` (same seed, same everything but the
//! rod metal), i.e. the worth of representing nickel and Fe-57 as themselves.
//! The rods are withdrawn, so the expected worth is small; a single-seed pair
//! cannot resolve much below ~300 pcm (seed-to-seed sd ~180-210 pcm), so pool
//! seeds before quoting it.
//!
//! **Results.** None. Not run: no eigenvalue is computed until the HTR-10
//! geometry and materials are complete (maintainer rule), and this case is
//! blocked by gh:#339 in any event. Record the pooled difference here when it
//! is measured.
//!
//! Verified without transport (2026-10-01): the composition this case builds
//! is pinned by `htr10_rmc::data::tests` (Ni and Fe atoms conserved per
//! element; carbon, helium), and the five Ni tapes were loaded and
//! reconstructed at 300.15 K under a 9 GB memory cap (each under 2 s).

#[path = "common/htr10_rod_metal_case.rs"]
mod htr10_rod_metal_case;

use nee_soon::htr10_rmc::data::RodMetalTreatment;

fn main() {
    htr10_rod_metal_case::run("htr10-rod-metal-full", RodMetalTreatment::Full);
}
