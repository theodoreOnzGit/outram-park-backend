// SPDX-License-Identifier: AGPL-3.0-only
//
// Translated from OpenMC `openmc/data/multipole.py` (`_vectfit_xs`, lines
// 144-385, and the fitting half of `vectfit_nuclide`, lines 481-524) at
// commit 8f79d9d02483c90e4134f03a9df4b75ce6b062bf. OpenMC is MIT-licensed,
// Copyright (c) 2011-2025 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors; the full permission notice is in this
// module's `vectfit.rs` header and in the crate's NOTICE.

//! Pole fitting of pointwise cross sections (`_vectfit_xs`) and the
//! piecewise driver (`vectfit_nuclide`).

use crate::wmp::Cf64;
use crate::NjoyError;

use super::npy::{ccmp, cdiv, count_peaks, interp, linspace, searchsorted_left, searchsorted_right};
use super::vectfit::{evaluate, vectfit};
use super::{MultipoleData, PointwiseXs, K_BOLTZMANN, TEMPERATURE_LIMIT};

/// `_vectfit_xs`'s keyword arguments, upstream defaults in [`Default`].
#[derive(Debug, Clone)]
pub struct VectfitOptions {
    /// Relative error tolerance (`rtol = 1e-3`).
    pub rtol: f64,
    /// Absolute error tolerance in barn (`atol = 1e-5`).
    pub atol: f64,
    /// Orders (pole counts) to search; `None` searches upstream's range.
    pub orders: Option<Vec<i64>>,
    /// Maximum vector-fitting iterations per order (`n_vf_iter = 30`).
    pub n_vf_iter: usize,
}

impl Default for VectfitOptions {
    fn default() -> Self {
        Self { rtol: 1e-3, atol: 1e-5, orders: None, n_vf_iter: 30 }
    }
}

/// The multipole poles and residues of one energy piece, in OpenMC's
/// multipole convention (residues divided by `i` relative to vector fitting).
#[derive(Debug, Clone)]
pub struct PieceFit {
    /// Poles in `sqrt(E)` space: real poles first, then one of each pair.
    pub poles: Vec<Cf64>,
    /// Residues `[reaction][pole]`.
    pub residues: Vec<Vec<Cf64>>,
}

/// `_vectfit_xs` (`multipole.py:144-385`): fit `sigma(E) * E` as a rational
/// function of `sqrt(E)`, searching the order, and return multipole poles
/// and residues. `ce_xs` is `[reaction][energy]`.
///
/// The figure output (`path_out`) and logging are not ported.
pub fn vectfit_xs(energy: &[f64], ce_xs: &[Vec<f64>], opts: &VectfitOptions) -> Result<PieceFit, NjoyError> {
    let ne = energy.len();
    let nmt = ce_xs.len();
    if ce_xs.iter().any(|r| r.len() != ne) || ne < 2 || nmt < 2 {
        return Err(NjoyError::WmpData("vectfit_xs: inconsistent cross section data".into()));
    }
    let (rtol, atol) = (opts.rtol, opts.atol);

    // test grid, 10x finer (multipole.py:182-190)
    let n_finer = 10usize;
    let ne_test = (ne - 1) * n_finer + 1;
    let xt: Vec<f64> = (0..ne_test).map(|i| i as f64).collect();
    let xp: Vec<f64> = (0..ne_test).step_by(n_finer).map(|i| i as f64).collect();
    let mut test_energy = interp(&xt, &xp, energy);
    test_energy[0] = energy[0];
    test_energy[ne_test - 1] = energy[ne - 1];
    let test_xs_ref: Vec<Vec<f64>> = ce_xs.iter().map(|r| interp(&test_energy, energy, r)).collect();

    let f: Vec<Vec<f64>> = ce_xs.iter().map(|r| r.iter().zip(energy).map(|(x, e)| x * e).collect()).collect();
    let s: Vec<f64> = energy.iter().map(|e| e.sqrt()).collect();
    let test_s: Vec<f64> = test_energy.iter().map(|e| e.sqrt()).collect();

    // inverse weighting (multipole.py:204-214)
    let min_cross_section = 1e-7;
    let mut weight: Vec<Vec<f64>> = f.iter().map(|r| r.iter().map(|x| 1.0 / x).collect()).collect();
    for i in 0..nmt {
        if ce_xs[i].iter().all(|&x| x <= min_cross_section) {
            weight[i].iter_mut().for_each(|w| *w = 1.0);
        } else if ce_xs[i].iter().any(|&x| x <= min_cross_section) {
            let mut mx = f64::NEG_INFINITY;
            let mut first = true;
            for (k, &x) in ce_xs[i].iter().enumerate() {
                if x > min_cross_section {
                    let w = weight[i][k];
                    // Python's builtin max: keep the first unless a later one is greater
                    if first || w > mx {
                        mx = w;
                        first = false;
                    }
                }
            }
            for (k, &x) in ce_xs[i].iter().enumerate() {
                if x <= min_cross_section {
                    weight[i][k] = mx;
                }
            }
        }
    }

    // peaks and order search range (multipole.py:217-225)
    let sum01: Vec<f64> = ce_xs[0].iter().zip(&ce_xs[1]).map(|(a, b)| a + b).collect();
    let n_peaks = count_peaks(&sum01);
    let orders: Vec<usize> = match &opts.orders {
        // upstream builds a `set`; its iteration order for these small
        // non-negative ints is ascending, which is what is used here
        Some(o) => {
            let mut v: Vec<usize> = o.iter().filter(|&&i| i >= 2).map(|&i| ((i / 2) * 2) as usize).collect();
            v.sort_unstable();
            v.dedup();
            v
        }
        None => {
            let lowest = 2.max(2 * n_peaks);
            let highest = 200.max(4 * n_peaks);
            (lowest..=highest).step_by(2).collect()
        }
    };

    let mut found_ideal = false;
    let mut n_discarded = 0usize;
    let mut best_quality = f64::NEG_INFINITY;
    let mut best_ratio = f64::NEG_INFINITY;
    let mut best: Option<(Vec<Cf64>, Vec<Vec<Cf64>>)> = None;
    let s0 = s[0];
    let s_last = s[ne - 1];

    for &order in &orders {
        let poles_r = linspace(s0, s_last, order / 2);
        let mut poles: Vec<Cf64> = Vec::with_capacity(order);
        for &pr in &poles_r {
            let p = Cf64::new(pr, pr * 0.01);
            poles.push(p);
            poles.push(p.conj());
        }
        poles.sort_by(|a, b| ccmp(*a, *b));

        let mut found_better = false;
        for _ in 0..opts.n_vf_iter {
            let r = vectfit(&f, &s, &poles, &weight, 0, false, false)?;
            poles = r.poles;
            let mut residues = r.residues;

            // real poles inside the range become conjugate pairs
            let mut n_real = 0usize;
            let mut new_poles = Vec::with_capacity(poles.len());
            for p in &poles {
                if s0 <= p.re && p.re <= s_last && p.im == 0.0 {
                    new_poles.push(Cf64::new(p.re, p.re * 0.01));
                    new_poles.push(Cf64::new(p.re, -(p.re * 0.01)));
                    n_real += 1;
                } else {
                    new_poles.push(*p);
                }
            }
            if n_real > 0 {
                let r2 = vectfit(&f, &s, &new_poles, &weight, 0, true, false)?;
                new_poles = r2.poles;
                residues = r2.residues;
            }

            // assess on the test grid (multipole.py:271-282)
            let ev = evaluate(&test_s, &new_poles, &residues, &[]);
            let test_xs: Vec<Vec<f64>> =
                ev.iter().map(|r| r.iter().zip(&test_energy).map(|(x, e)| x / e).collect()).collect();
            let mut any_nan = false;
            let mut all_le_atol = true;
            let mut maxre = f64::NEG_INFINITY;
            let mut c1 = 0usize;
            let mut c2 = 0usize;
            let total = nmt * ne_test;
            for i in 0..nmt {
                for k in 0..ne_test {
                    let ab = (test_xs[i][k] - test_xs_ref[i][k]).abs();
                    let re = ab / test_xs_ref[i][k];
                    if ab.is_nan() {
                        any_nan = true;
                    }
                    if !(ab <= atol) {
                        all_le_atol = false;
                    }
                    if ab > atol {
                        // np.max propagates NaN
                        maxre = if re.is_nan() || maxre.is_nan() { f64::NAN } else { maxre.max(re) };
                    }
                    if re < rtol || ab < atol {
                        c1 += 1;
                    }
                    if re < 10.0 * rtol || ab < atol {
                        c2 += 1;
                    }
                }
            }
            let (maxre, ratio, ratio2) = if any_nan {
                (f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY)
            } else if all_le_atol {
                (0.0, 1.0, 1.0)
            } else {
                (maxre, c1 as f64 / total as f64, c2 as f64 / total as f64)
            };
            let pen = 0.1 * maxre;
            // Python's min(pen, 1)
            let pen = if 1.0 < pen { 1.0 } else { pen };
            let mut quality = ratio + ratio2 - pen - 0.001 * new_poles.len() as f64;
            if test_xs.iter().any(|r| r.iter().any(|&x| x < -atol)) {
                quality = f64::NEG_INFINITY;
            }

            if quality > best_quality {
                found_better = true;
                best_quality = quality;
                best_ratio = ratio;
                best = Some((new_poles.clone(), residues.clone()));
                if best_ratio >= 1.0 {
                    found_ideal = true;
                    break;
                }
            }
        }
        if found_ideal {
            break;
        }
        if found_better {
            n_discarded = 0;
        } else if order > (2 * n_peaks).max(50) && best_ratio > 0.7 {
            n_discarded += 1;
            if n_discarded >= 10 || (n_discarded >= 5 && best_ratio > 0.9) {
                break;
            }
        }
    }

    let (best_poles, best_residues) =
        best.ok_or_else(|| NjoyError::WmpData("vectfit_xs: no fit was ever accepted".into()))?;

    // merge conjugate poles (multipole.py:330-350)
    let mut real_idx = Vec::new();
    let mut conj_idx = Vec::new();
    let mut found_conj = false;
    for (i, p) in best_poles.iter().enumerate() {
        if found_conj {
            found_conj = false;
            continue;
        }
        if p.im == 0.0 {
            real_idx.push(i);
        } else if i + 1 < best_poles.len() && p.conj() == best_poles[i + 1] {
            found_conj = true;
            conj_idx.push(i);
        } else {
            return Err(NjoyError::WmpData("vectfit_xs: complex poles are not conjugate".into()));
        }
    }
    let jj = Cf64::new(0.0, 1.0);
    let mut poles = Vec::new();
    for &i in real_idx.iter().chain(conj_idx.iter()) {
        poles.push(best_poles[i]);
    }
    let residues: Vec<Vec<Cf64>> = best_residues
        .iter()
        .map(|row| {
            let mut out = Vec::new();
            for &i in &real_idx {
                out.push(cdiv(row[i], jj));
            }
            for &i in &conj_idx {
                out.push(cdiv(Cf64::new(row[i].re * 2.0, row[i].im * 2.0), jj));
            }
            out
        })
        .collect();
    Ok(PieceFit { poles, residues })
}

/// The fitting half of `vectfit_nuclide` (`multipole.py:481-524`): split the
/// range into pieces equal in `sqrt(E)` (overlapping by the Doppler reach at
/// 3000 K) and fit each with [`vectfit_xs`].
pub fn vectfit_pointwise(
    xs: &PointwiseXs,
    vf_pieces: Option<usize>,
    opts: &VectfitOptions,
) -> Result<MultipoleData, NjoyError> {
    let energy = &xs.energy;
    let n_points = energy.len();
    let e_min = energy[0];
    let e_max = energy[n_points - 1];
    let mut ce_xs = vec![xs.elastic.clone(), xs.absorption.clone()];
    let mut mts = vec![2, 27];
    if let Some(fis) = &xs.fission {
        ce_xs.push(fis.clone());
        mts.push(18);
    }

    let vf_pieces = match vf_pieces {
        Some(v) => v,
        None => {
            let n_peaks = count_peaks(&xs.total);
            if n_peaks > 200 || n_points > 30000 || n_peaks * n_points > 100 * 10000 {
                5.max(n_peaks / 50).max(n_points / 2000)
            } else {
                1
            }
        }
    };
    let piece_width = (e_max.sqrt() - e_min.sqrt()) / vf_pieces as f64;
    let alpha = xs.awr / (K_BOLTZMANN * TEMPERATURE_LIMIT);

    let mut poles = Vec::with_capacity(vf_pieces);
    let mut residues = Vec::with_capacity(vf_pieces);
    for i_piece in 0..vf_pieces {
        let b = e_min.sqrt() + piece_width * (i_piece as f64 - 0.5);
        let e_bound = b * b;
        let e_start_idx = if i_piece == 0 || (alpha * e_bound).sqrt() < 4.0 {
            0
        } else {
            let t = (alpha * e_bound).sqrt() - 4.0;
            let e_start = e_min.max(t * t / alpha);
            searchsorted_right(energy, e_start) as isize - 1
        };
        let b = e_min.sqrt() + piece_width * (i_piece as f64 + 1.0);
        let e_bound = b * b;
        let t = (alpha * e_bound).sqrt() + 4.0;
        let e_end = e_max.min(t * t / alpha);
        let e_end_idx = searchsorted_left(energy, e_end) + 1;
        let lo = e_start_idx.max(0) as usize;
        let hi = (e_end_idx + 1).min(n_points);
        let en = energy[lo..hi].to_vec();
        let cx: Vec<Vec<f64>> = ce_xs.iter().map(|r| r[lo..hi].to_vec()).collect();
        let fit = vectfit_xs(&en, &cx, opts)?;
        poles.push(fit.poles);
        residues.push(fit.residues);
    }
    Ok(MultipoleData { name: xs.name.clone(), awr: xs.awr, e_min, e_max, mts, poles, residues })
}
