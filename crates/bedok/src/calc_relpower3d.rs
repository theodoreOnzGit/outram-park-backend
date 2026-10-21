//! Collapse a 3-D power-density vector to a normalised radial (x-y) map.
//!
//! # Provenance
//!
//! - **Original author:** Than Yan Ren, Singapore Nuclear Research and Safety
//!   Institute (SNRSI).
//! - **Source file:** `calc_relpower3d.m`, `main_exec_diff3d_standalone`
//!   snapshot.
//! - **Permission:** given by the author for open-source release under OUTRAM
//!   PARK; see the crate README, "Permission and attribution".
//! - **Licence:** GPL-3.0-only.

use crate::handle3dcoords::handle3dcoords;
use crate::matlab::Array2;
use crate::types::Params;

/// `pwrdens_out = calc_relpower3d(params, pwrdens)`.
///
/// Sums the power density over energy groups (when the input still carries
/// them) and over the axial direction, then normalises the resulting `x`-`y`
/// map so its **mean over the fuelled nodes is 1**. This is the
/// relative-power map `main_exec_diff3d.m` writes to `rel_power.csv`, and the
/// quantity the NEACRP and IAEA-3D benchmarks report assembly-wise.
///
/// # Arguments
///
/// - `params` — supplies `G` and the three extents via `handle3dcoords`.
/// - `pwrdens` — power density, flattened. Either `maxix*maxiy*maxiz` long
///   (already group-summed) or `G` times that. Units are whatever the solver
///   produced; the normalisation makes the output dimensionless.
///
/// # Returns
///
/// A `maxix`-by-`maxiy` map, dimensionless, scaled so the mean over non-zero
/// entries is 1.
///
/// # The normalisation, precisely
///
/// The reference computes `nzero = nnz(pwrdensxy)` and
/// `nsum = sum(pwrdensxy, "all")`, then scales by `nzero / nsum`. `nzero`
/// counts only **non-zero** nodes while `nsum` sums **all** of them, so the
/// result averages to 1 over the fuelled region rather than over the full
/// rectangle — reflector nodes are excluded from the average but not from the
/// sum. That is the convention benchmark relative-power maps use.
///
/// # Indexing
///
/// The reference's 1-based `(ix-1)*maxiy*maxiz + (iy-1)*maxiz + iz` is
/// converted to the 0-based `ix*maxiy*maxiz + iy*maxiz + iz`.
///
/// # Panics
///
/// If `pwrdens` is shorter than `maxix*maxiy*maxiz`, or shorter than
/// `G*maxix*maxiy*maxiz` when the group-collapse branch is taken.
///
/// # Division by zero
///
/// With an all-zero `pwrdens`, `nsum` is `0` and every output entry is `NaN`.
/// The reference does not guard this and neither does the translation.
pub fn calc_relpower3d(params: &Params, pwrdens: &[f64]) -> Array2<f64> {
    let g_count = params.g;
    let (maxix, maxiy, maxiz) = handle3dcoords(params);
    let es = maxix * maxiy * maxiz;

    // Group collapse: only when there is more than one group AND the vector is
    // longer than a single group's worth. A caller that has already summed the
    // groups passes an `es`-long vector and skips this.
    let mut working: Vec<f64> = pwrdens.to_vec();
    if g_count > 1 && pwrdens.len() > es {
        let mut collapsed = pwrdens[0..es].to_vec();
        for g in 1..g_count {
            for n in 0..es {
                collapsed[n] += pwrdens[g * es + n];
            }
        }
        working = collapsed;
    }

    let mut pwrdensxy = Array2::<f64>::zeros(maxix, maxiy);
    for ix in 0..maxix {
        for iy in 0..maxiy {
            for iz in 0..maxiz {
                let idx = ix * maxiy * maxiz + iy * maxiz + iz;
                let acc = pwrdensxy.get(ix, iy) + working[idx];
                pwrdensxy.set(ix, iy, acc);
            }
        }
    }

    let nzero = pwrdensxy.as_slice().iter().filter(|v| **v != 0.0).count() as f64;
    let nsum: f64 = pwrdensxy.as_slice().iter().sum();

    let mut out = Array2::<f64>::zeros(maxix, maxiy);
    for ix in 0..maxix {
        for iy in 0..maxiy {
            out.set(ix, iy, pwrdensxy.get(ix, iy) * nzero / nsum);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params_2x2x2(g: usize) -> Params {
        Params {
            maxix: Some(2),
            maxiy: Some(2),
            maxiz: Some(2),
            g,
            ..Default::default()
        }
    }

    /// A uniform map must normalise to exactly 1 everywhere.
    #[test]
    fn uniform_power_normalises_to_one() {
        let params = params_2x2x2(1);
        let out = calc_relpower3d(&params, &[1.0; 8]);
        for ix in 0..2 {
            for iy in 0..2 {
                assert_eq!(out.get(ix, iy), 1.0);
            }
        }
    }

    /// With a zeroed node, the mean is taken over the three non-zero nodes
    /// only — the behaviour described under "The normalisation, precisely".
    #[test]
    fn zero_nodes_are_excluded_from_the_average() {
        let params = params_2x2x2(1);
        // Axial pairs sum to (2, 2, 2, 0) over the four x-y positions.
        let out = calc_relpower3d(&params, &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
        // nzero = 3, nsum = 6, so each non-zero node scales by 3/6 = 0.5.
        assert_eq!(out.get(0, 0), 1.0);
        assert_eq!(out.get(1, 1), 0.0);
    }

    /// Two groups stacked in one vector are summed before the axial collapse.
    #[test]
    fn multigroup_input_is_collapsed_over_groups() {
        let params = params_2x2x2(2);
        let mut v = vec![1.0; 8];
        v.extend(vec![3.0; 8]);
        let out = calc_relpower3d(&params, &v);
        // Every node becomes 1+3 = 4, uniform, so normalises to 1.
        assert_eq!(out.get(0, 0), 1.0);
        assert_eq!(out.get(1, 1), 1.0);
    }
}
