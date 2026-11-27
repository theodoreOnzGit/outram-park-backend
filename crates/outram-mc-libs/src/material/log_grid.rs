// SPDX-License-Identifier: AGPL-3.0-only
//
// Attribution (RESEARCH_INTEGRITY_AND_PROVENANCE.md): the logarithmic
// energy-grid index is a port of OpenMC's.
//   Upstream project: OpenMC, https://github.com/openmc-dev/openmc
//   Upstream files:   src/nuclide.cpp (`Nuclide::init_grid`, :499-530; the
//                     grid search of `Nuclide::calculate_xs`, :721-746),
//                     src/simulation.cpp (`log_spacing`, :905-915),
//                     src/settings.cpp (`n_log_bins {8000}`, :119)
//   Upstream commit:  8f79d9d0
//   Upstream licence: MIT, Copyright (c) 2011-2025 Massachusetts Institute of
//                     Technology, UChicago Argonne LLC, and OpenMC contributors
//   This port:        AGPL-3.0-only, with the MIT notice preserved here.
//! **One energy-grid search per nuclide per lookup** (gh:#1295): OpenMC's
//! logarithmic grid index over a nuclide's union energy grid.
//!
//! # Why
//!
//! A HIGH-tier nuclide's cross sections are RECONR/BROADR sections, one
//! pointwise table per reaction. Looking up a collision used to search each
//! reaction's own table (`eval_lin_lin`'s `partition_point`), some 70 searches
//! per U-238 collision. On Godiva that was ~70 % of transport (gh:#1292).
//!
//! # What the data guarantees, and what is checked
//!
//! RECONR and BROADR write every section on the material's union grid, a
//! threshold section starting later: each section's energies are a **suffix**
//! of the union grid (OpenMC's `threshold_idx` layout). Measured 2026-11-24 on
//! ENDF/B-VIII.0 at 293.6 K: U-235 87 of 87 sections, U-238 52 of 52, U-234 53
//! of 53, no repeated energies. [`UnionGrid::new`] **checks** this per section,
//! bit for bit; a section that is not a suffix gets no offset and keeps the
//! per-reaction search.
//!
//! # Bit-identical by construction
//!
//! [`UnionGrid::locate`] returns `j`, the number of union energies `<= e`, the
//! same count `partition_point(|x| x <= e)` gives. For a suffix section at
//! offset `o`, the section's own count is `j - o`, so the interval is the
//! one `eval_lin_lin` would find, and [`eval_section_located`] interpolates it
//! with `eval_lin_lin`'s own arithmetic (OpenMC's `(1-f) y0 + f y1` rounds
//! differently and is deliberately **not** used). Every other case (below or
//! above the section, thresholds, extrapolation, `legacy_xs_details`) is
//! handed to the existing `eval_section_x`, so the value is the same double in
//! every case. `locate` also verifies its bracket and falls back to a full
//! search if the log-bin guess were ever off.

#[cfg(not(any(feature = "std", test)))]
#[allow(unused_imports)]
use crate::nostd_prelude::*;

use njoy_outram_park_fork::reconr::ReconrResult;
use petir::mathf::RealMath;

/// `settings::n_log_bins` (`settings.cpp:119`): the number of equal-lethargy
/// bins of the index.
pub const N_LOG_BINS: usize = 8000;

/// A nuclide's union energy grid and OpenMC's logarithmic index into it.
#[derive(Debug, Clone, Default)]
pub struct UnionGrid {
    /// The union of every section's energies, eV, strictly ascending.
    energy: Vec<f64>,
    /// `grid_index[k]`: the last `j` with `ln(energy[j] / e_min) <= k *
    /// spacing` (`init_grid`), for `k = 0..=N_LOG_BINS`.
    grid_index: Vec<u32>,
    /// `energy[0]`, eV.
    e_min: f64,
    /// `ln(e_max / e_min) / N_LOG_BINS`.
    spacing: f64,
    /// `ln(e_min)` and `1 / spacing`, so `locate` takes the bin from a
    /// shared `ln(E)` with a subtraction and a multiply (gh:#1732).
    ln_e_min: f64,
    inv_spacing: f64,
    /// Per section: `Some(o)` when the section's energies are exactly
    /// `energy[o..]`, else `None` (searched per reaction, as before).
    offsets: Vec<Option<usize>>,
}

impl UnionGrid {
    /// Build the union grid of `recon`'s sections and its index
    /// (`Nuclide::init_grid`, `nuclide.cpp:499-530`, with this nuclide's own
    /// `E_min`/`E_max` in place of the library-wide ones: the bracket is
    /// verified on every lookup, so the bounds only decide the bin width).
    ///
    /// An evaluation with fewer than two distinct positive energies gets an
    /// empty grid, and every lookup then takes the per-reaction search.
    pub fn new(recon: &ReconrResult) -> Self {
        let mut energy: Vec<f64> = recon.sections.iter().flat_map(|s| s.pairs.iter().map(|p| p.0)).collect();
        energy.sort_by(f64::total_cmp);
        energy.dedup();
        let n = energy.len();
        if n < 2 || !(energy[0] > 0.0) || !energy[n - 1].is_finite() {
            return Self::default();
        }
        let offsets = recon
            .sections
            .iter()
            .map(|s| {
                let m = s.pairs.len();
                if m == 0 || m > n {
                    return None;
                }
                let o = n - m;
                let suffix = energy[o..].iter().zip(&s.pairs).all(|(g, p)| g.to_bits() == p.0.to_bits());
                suffix.then_some(o)
            })
            .collect();
        let e_min = energy[0];
        let spacing = RealMath::r_ln(energy[n - 1] / e_min) / N_LOG_BINS as f64;
        let mut grid_index = Vec::with_capacity(N_LOG_BINS + 1);
        let mut j = 0usize;
        for k in 0..=N_LOG_BINS {
            let umesh = k as f64 * spacing;
            while RealMath::r_ln(energy[j + 1] / e_min) <= umesh {
                if j + 2 == n {
                    break;
                }
                j += 1;
            }
            grid_index.push(j as u32);
        }
        Self { energy, grid_index, e_min, spacing, ln_e_min: RealMath::r_ln(e_min), inv_spacing: 1.0 / spacing, offsets }
    }

    /// `j`, the number of union energies `<= e`, when `energy[0] <= e <
    /// energy[last]` (so `energy[j - 1] <= e < energy[j]`); `None` outside,
    /// for `NaN`, or when the grid is empty. One bracketed search inside a
    /// log bin (`calculate_xs`, `nuclide.cpp:729-736`), verified, with a full
    /// search as the fallback.
    ///
    /// **One `ln` per energy, not per nuclide (gh:#1732).** OpenMC computes
    /// the bin once per particle energy (`i_log_union`) and hands it to every
    /// nuclide. Here the bin comes from `ln(E)` ([`shared_ln`], computed once
    /// per energy per thread and reused by every nuclide asked about the same
    /// energy) minus this grid's `ln(E_min)`, times `1 / spacing`. That can
    /// round to a neighbouring bin, so the bracket is checked, then the bins
    /// either side, then the full search: `j` is the same count in every case.
    pub fn locate(&self, e: f64) -> Option<usize> {
        let n = self.energy.len();
        if n < 2 || !(e >= self.energy[0]) || !(e < self.energy[n - 1]) {
            return None;
        }
        let u = ((shared_ln(e) - self.ln_e_min) * self.inv_spacing).max(0.0) as usize;
        let u = u.min(N_LOG_BINS - 1);
        let bracket = |lo: usize, hi: usize| -> Option<usize> {
            if lo >= hi {
                return None;
            }
            let j = lo + self.energy[lo..hi].partition_point(|&g| g <= e);
            (j >= 1 && j < n && self.energy[j - 1] <= e && e < self.energy[j]).then_some(j)
        };
        let bin = |u: usize| (self.grid_index[u] as usize, (self.grid_index[u + 1] as usize + 2).min(n));
        let (lo, hi) = bin(u);
        if let Some(j) = bracket(lo, hi) {
            return Some(j);
        }
        // a rounding of the shared `ln` can land one bin off
        let (lo, hi) = (bin(u.saturating_sub(1)).0, bin((u + 1).min(N_LOG_BINS - 1)).1);
        if let Some(j) = bracket(lo, hi) {
            return Some(j);
        }
        Some(self.energy.partition_point(|&g| g <= e))
    }

    /// The suffix offset of section `idx`, if it has one.
    pub fn offset(&self, idx: usize) -> Option<usize> {
        self.offsets.get(idx).copied().flatten()
    }

    /// Number of points of the union grid.
    pub fn len(&self) -> usize {
        self.energy.len()
    }

    /// `true` when the grid is empty (every lookup is searched per reaction).
    pub fn is_empty(&self) -> bool {
        self.energy.is_empty()
    }
}

/// `ln(e)` (PETIR's), computed once per energy and reused while the same
/// thread keeps asking about the same `e` (every nuclide of a material at one
/// collision, gh:#1732). A pure memo: it returns the very double
/// `RealMath::r_ln(e)` gives, so nothing that reads it can change. Without
/// `std` (no thread-local storage) it computes `ln` every time.
pub fn shared_ln(e: f64) -> f64 {
    #[cfg(feature = "std")]
    {
        use core::cell::Cell;
        std::thread_local! {
            static LAST: Cell<(u64, f64)> = const { Cell::new((u64::MAX, f64::NAN)) };
        }
        let bits = e.to_bits();
        LAST.with(|c| {
            let (k, v) = c.get();
            if k == bits {
                return v;
            }
            let v = RealMath::r_ln(e);
            c.set((bits, v));
            v
        })
    }
    #[cfg(not(feature = "std"))]
    {
        RealMath::r_ln(e)
    }
}

/// Section `idx` of `recon` at `e`, eV, given `j = grid.locate(e)`: the same
/// double as `nuclide.rs`'s `eval_section_x(recon, idx, e)` (see the module
/// doc), without the per-reaction search when `e` is strictly inside a suffix
/// section. `fallback` is `eval_section_x`, passed in so this module does not
/// duplicate it.
pub fn eval_section_located(
    recon: &ReconrResult,
    grid: &UnionGrid,
    idx: usize,
    e: f64,
    j: Option<usize>,
    fallback: fn(&ReconrResult, usize, f64) -> f64,
) -> f64 {
    if let (Some(j), Some(o)) = (j, grid.offset(idx)) {
        let p = &recon.sections[idx].pairs;
        let n = p.len();
        if n >= 2 && e > p[0].0 && e < p[n - 1].0 && j > o {
            // `eval_lin_lin`'s interval and arithmetic, `reconr/mod.rs:304-324`
            let k = j - o;
            let (x0, y0) = p[k - 1];
            let (x1, y1) = p[k];
            return if (x1 - x0).abs() < 1e-30 { y0 } else { y0 + (y1 - y0) * (e - x0) / (x1 - x0) };
        }
    }
    fallback(recon, idx, e)
}

#[cfg(test)]
mod tests {
    use super::*;
    use njoy_outram_park_fork::reconr::eval_lin_lin;

    fn recon_with(sections: Vec<Vec<(f64, f64)>>) -> ReconrResult {
        use njoy_outram_park_fork::endf::mt::MtReaction;
        use njoy_outram_park_fork::reconr::ReconrSection;
        ReconrResult {
            material: njoy_outram_park_fork::reconr::mf1::MaterialInfo {
                za: 92238.0,
                awr: 236.0,
                lrp: 0,
                lfi: 0,
                nlib: 0,
                elis: 0.0,
                nfor: 6,
                emax: 2.0e7,
            },
            sections: sections
                .into_iter()
                .enumerate()
                .map(|(i, pairs)| ReconrSection { lr: 0, mt: MtReaction::from_any(51 + i as i32), qi: 0.0, pairs })
                .collect(),
            resonance_upper_limit: None,
            unresolved_table: None,
            temperature_k: 293.6,
        }
    }

    fn plain(recon: &ReconrResult, idx: usize, e: f64) -> f64 {
        eval_lin_lin(&recon.sections[idx].pairs, e)
    }

    /// `locate` gives `partition_point(<= e)` on the union grid at grid
    /// points, between them and at the ends; suffix sections get offsets, a
    /// section off the grid does not; the located value is the searched one
    /// to the bit (gh:#1295).
    #[test]
    fn locate_and_located_values_match_the_search() {
        let grid_e: Vec<f64> = (0..400).map(|i| 1e-5 * 1.07_f64.powi(i)).collect();
        let full: Vec<(f64, f64)> = grid_e.iter().map(|&x| (x, 1.0 / x.sqrt())).collect();
        let threshold: Vec<(f64, f64)> = grid_e[250..].iter().map(|&x| (x, x.sqrt())).collect();
        // on the grid but not contiguous: not a suffix, so searched per reaction
        let off_grid: Vec<(f64, f64)> = vec![(grid_e[10], 1.0), (grid_e[200], 2.0), (grid_e[399], 3.0)];
        let r = recon_with(alloc::vec![full, threshold, off_grid]);
        let g = UnionGrid::new(&r);
        assert_eq!(g.offset(0), Some(0));
        assert_eq!(g.offset(1), Some(g.len() - 150));
        assert_eq!(g.offset(2), None);
        let mut probes: Vec<f64> = grid_e.clone();
        probes.extend(grid_e.windows(2).map(|w| 0.5 * (w[0] + w[1])));
        probes.extend([1e-7, grid_e[399] * 2.0, 2e-5, 3.3e-3]);
        for &e in &probes {
            let want = if e >= g.energy[0] && e < g.energy[g.len() - 1] {
                Some(g.energy.partition_point(|&x| x <= e))
            } else {
                None
            };
            assert_eq!(g.locate(e), want, "locate({e:e})");
            for idx in 0..3 {
                let a = eval_section_located(&r, &g, idx, e, g.locate(e), plain);
                assert_eq!(a.to_bits(), plain(&r, idx, e).to_bits(), "section {idx} at {e:e}");
            }
        }
        assert_eq!(g.locate(f64::NAN), None);
        // the memo returns PETIR's `ln` to the bit, on a miss and on a hit (gh:#1732)
        for &e in &[1.0e-5, 0.0253, 2.0e6, 0.0253] {
            assert_eq!(shared_ln(e).to_bits(), RealMath::r_ln(e).to_bits());
        }
        assert!(UnionGrid::new(&recon_with(alloc::vec![])).is_empty());
    }
}
