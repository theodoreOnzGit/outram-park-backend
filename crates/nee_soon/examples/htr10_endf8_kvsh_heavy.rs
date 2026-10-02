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

//! # HTR-10 `k_eff` against loading height, ENDF/B-VIII.0 — the **heavy** sweep (gh:#501)
//!
//! The same sweep as `htr10_endf8_kvsh_quick` (Şeker's bed, N = 10 … 20,
//! RMC reference and MCNP Tables 3/4 gauges at equal ball count), at the
//! reference paper's own statistics: **10 000 particles × \[5 inactive + 135
//! active\]** (140 cycles), [`SweepStatistics::HEAVY`]. **Multi-hour**: an
//! example, not a test, so nothing gates it.
//!
//! ```bash
//! # compile check only (both examples):
//! taskset -c 0-9 cargo check --release -j 8 -p nee_soon --example htr10_endf8_kvsh_quick --example htr10_endf8_kvsh_heavy
//! taskset -c 0-9 cargo build --release -j 8 -p nee_soon --example htr10_endf8_kvsh_heavy
//! taskset -c 0-9 ./target/release/examples/htr10_endf8_kvsh_heavy --threads 8 --out <dir>
//! ```
//!
//! Outputs, arguments and the shared driver are as documented on
//! `htr10_endf8_kvsh_quick`; the record is rewritten after every height, so a
//! stopped run keeps every finished height.
//!
//! ## Verification & validation
//!
//! **Methodology.** As for the quick sweep. The same statistics and seed were
//! run on 2026-10-01/02 through `htr10_rmc_keff` on 5 threads
//! (`verification_and_validation/htr10_seker_2026_10_01_10k/`); this sweep
//! re-runs them on 8 threads through the shared machinery, which compares
//! the code before and after the #486 geometry move and the thread count.
//!
//! **Results.** See
//! `crates/nee_soon/verification_and_validation/htr10_endf8_kvsh_heavy_<date>/README.md`.

#[path = "common/htr10_kvsh.rs"]
mod htr10_kvsh;

use nee_soon::htr10_rmc::keff_vs_height::SweepStatistics;

fn main() {
    htr10_kvsh::run_sweep("htr10_endf8_kvsh_heavy", SweepStatistics::HEAVY);
}
