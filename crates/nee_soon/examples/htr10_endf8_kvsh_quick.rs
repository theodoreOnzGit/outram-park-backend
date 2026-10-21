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

//! # HTR-10 `k_eff` against loading height, ENDF/B-VIII.0 — the **quick** sweep (gh:#501)
//!
//! Şeker & Çolak (2003)'s 13-ball bed at N = 10 … 20 layers (gh:#472), each
//! height compared with Li, Yu & Wei (2014)'s RMC curve (the reference) and
//! the paper's MCNP Tables 3 (vacuum) and 4 (helium) (gauges), all read at the
//! height where Şeker's model holds as many balls as the built bed.
//! Statistics: **2000 particles × \[30 inactive + 70 active\]** (100 cycles),
//! those of the 2026-10-01 records. The heavy sweep is
//! `htr10_endf8_kvsh_heavy`.
//!
//! ```bash
//! # compile check only (both examples):
//! taskset -c 0-9 cargo check --release -j 8 -p nee_soon --example htr10_endf8_kvsh_quick --example htr10_endf8_kvsh_heavy
//! taskset -c 0-9 cargo build --release -j 8 -p nee_soon --example htr10_endf8_kvsh_quick
//! taskset -c 0-9 ./target/release/examples/htr10_endf8_kvsh_quick --threads 8 --out <dir>
//! # optional: --layers 10,12,20 runs a subset
//! ```
//!
//! Writes into `<dir>`: `logs/N<n>.log` per height, `keff_vs_height.py` (a
//! standalone matplotlib script with every number embedded; the same run gives
//! a byte-identical file), `results_table.md` and `RUN_PARAMETERS.md`, all
//! rewritten after every height; then runs the script to `keff_vs_height.png`
//! if `python3` (or `OUTRAM_PYTHON`) has matplotlib.
//!
//! Every setting is a literal: the statistics are
//! [`SweepStatistics::QUICK`], the data configuration is
//! `Htr10DataConfig::default()` (the correct-physics default: 30P graphite,
//! SiC and UO2 laws, natural carbon, helium coolant, full Ni/Fe rod metal),
//! and nothing physical is read from the environment. The driver is
//! `examples/common/htr10_kvsh.rs`; the run machinery and the emitters are
//! [`nee_soon::htr10_rmc::keff_vs_height`].
//!
//! ## Verification & validation
//!
//! **Methodology.** `k_eff` per height, against RMC at equal ball count; the
//! crate's pass band is 500-1000 pcm. MCNP is a gauge only (Şeker's
//! ENDF/B-VI runs on an independent model). One seed per point; the error bar
//! is the within-run 1σ, which does not contain seed-to-seed scatter.
//!
//! **Results (2026-10-02).** k − RMC over N = 10..20: mean +221, RMS 359,
//! max 674 pcm, σ 282-362 pcm; 11/11 within ±1000, 9/11 within ±500; slope
//! +7.8 pcm/cm. N = 10, 12, 20 equal the 2026-10-01 `htr10_rmc_keff` records
//! to every printed digit. 278-331 s transport per height, 58 min whole sweep,
//! i9-13900K pinned to CPUs 0-9, 8 threads, 62.5 GiB, shared machine. See
//! `crates/nee_soon/verification_and_validation/htr10_endf8_kvsh_quick_2026-10-02/README.md`
//! for the table with 1σ, the figure, timings with hardware, and the reading.

#[path = "common/htr10_kvsh.rs"]
mod htr10_kvsh;

use nee_soon::htr10_rmc::keff_vs_height::SweepStatistics;

fn main() {
    htr10_kvsh::run_sweep("htr10_endf8_kvsh_quick", SweepStatistics::QUICK);
}
