// SPDX-License-Identifier: AGPL-3.0-only
//
// Translated from OpenMC `openmc/data/multipole.py` (`_windowing`, lines
// 541-730, and `WindowedMultipole.from_multipole`, lines 1048-1148) at commit
// 8f79d9d02483c90e4134f03a9df4b75ce6b062bf. OpenMC is MIT-licensed,
// Copyright (c) 2011-2025 Massachusetts Institute of Technology, UChicago
// Argonne LLC, and OpenMC contributors; the full permission notice is in this
// module's `vectfit.rs` header and in the crate's NOTICE.

//! Windowing: choose, per equal-`sqrt(E)` window, the fewest consecutive
//! poles plus a curve-fit background that reproduce the multipole cross
//! section; and the search over window counts and curve-fit orders.

use std::cmp::Ordering;

use petir::mathf::RealMath;

use crate::wmp::{Cf64, WindowedMultipole, WmpWindow};
use crate::NjoyError;

use super::npy::{ccmp, linspace, lstsq, pairwise_sum};
use super::vectfit::evaluate;
use super::{MultipoleData, K_BOLTZMANN, TEMPERATURE_LIMIT};

/// A windowed-multipole library as OpenMC's `WindowedMultipole` holds it
/// after generation (`multipole.py:718-730`): the attributes that
/// `export_to_hdf5` writes, in its layout.
#[derive(Debug, Clone)]
pub struct WmpGenerated {
    /// GNDS name, e.g. `Mg26`.
    pub name: String,
    /// Window width in `sqrt(E)` \[sqrt(eV)\].
    pub spacing: f64,
    /// `sqrt(AWR)`.
    pub sqrt_awr: f64,
    /// Lowest energy \[eV\].
    pub e_min: f64,
    /// Highest energy \[eV\].
    pub e_max: f64,
    /// Used poles, in library order.
    pub poles: Vec<Cf64>,
    /// Residues `[pole][reaction]` (2 or 3 reactions).
    pub residues: Vec<Vec<Cf64>>,
    /// `[first, last]` pole of each window, **1-based**, as stored.
    pub windows: Vec<[i64; 2]>,
    /// Curve-fit coefficients `[window][power][reaction]`.
    pub curvefit: Vec<Vec<Vec<f64>>>,
    /// Whether each window's curve fit is Doppler broadened.
    pub broaden_poly: Vec<bool>,
}

impl WmpGenerated {
    /// `fit_order`: curve-fit terms minus one.
    pub fn fit_order(&self) -> usize {
        self.curvefit.first().map_or(0, |w| w.len().saturating_sub(1))
    }

    /// Number of windows.
    pub fn n_windows(&self) -> usize {
        self.windows.len()
    }

    /// `poles_per_window`: mean of `last - first + 1`.
    pub fn poles_per_window(&self) -> f64 {
        let v: Vec<f64> = self.windows.iter().map(|w| (w[1] - w[0] + 1) as f64).collect();
        pairwise_sum(&v) / v.len() as f64
    }

    /// The crate's evaluator record, as [`WindowedMultipole::load_h5`] would
    /// build it from the file OpenMC would write.
    pub fn to_windowed_multipole(&self) -> WindowedMultipole {
        let fissionable = self.residues.first().is_some_and(|r| r.len() >= 3);
        let zero = Cf64::new(0.0, 0.0);
        let residues = self
            .residues
            .iter()
            .map(|r| [r[0], r[1], if fissionable { r[2] } else { zero }])
            .collect();
        let curvefit = self
            .curvefit
            .iter()
            .map(|w| w.iter().map(|c| [c[0], c[1], if fissionable { c[2] } else { 0.0 }]).collect())
            .collect();
        let windows = self
            .windows
            .iter()
            .zip(&self.broaden_poly)
            .map(|(w, &b)| {
                let (start, end) = if w[1] < w[0] { (1, 0) } else { ((w[0] - 1) as usize, (w[1] - 1) as usize) };
                WmpWindow { start, end, broaden_poly: b }
            })
            .collect();
        WindowedMultipole {
            name: self.name.clone(),
            awr: self.sqrt_awr * self.sqrt_awr,
            e_min: self.e_min,
            e_max: self.e_max,
            fissionable,
            poles: self.poles.clone(),
            residues,
            curvefit,
            windows,
            inv_spacing: 1.0 / self.spacing,
            fit_order: self.fit_order(),
        }
    }
}

/// `_windowing`'s settings.
#[derive(Debug, Clone, Copy)]
pub struct WindowingOptions {
    /// Curve-fit order.
    pub n_cf: usize,
    /// Maximum relative error (`rtol = 1e-3`).
    pub rtol: f64,
    /// Absolute error floor in barn (`atol = 1e-5`).
    pub atol: f64,
    /// Number of windows; `None` uses `spacing`, or 1000.
    pub n_win: Option<usize>,
    /// Inner window spacing in `sqrt(E)`.
    pub spacing: Option<f64>,
}

impl WindowingOptions {
    /// Upstream defaults for a given curve-fit order.
    pub fn new(n_cf: usize) -> Self {
        Self { n_cf, rtol: 1e-3, atol: 1e-5, n_win: None, spacing: None }
    }
}

/// `_windowing` (`multipole.py:541-730`).
pub fn windowing(mp: &MultipoleData, o: &WindowingOptions) -> Result<WmpGenerated, NjoyError> {
    let n_pieces = mp.poles.len();
    let e_min = mp.e_min;
    let mut e_max = mp.e_max;
    let piece_width = (e_max.sqrt() - e_min.sqrt()) / n_pieces as f64;
    let alpha = mp.awr / (K_BOLTZMANN * TEMPERATURE_LIMIT);
    let sqrt_alpha = alpha.sqrt();
    let n_cf = o.n_cf;

    let n_win = match (o.n_win, o.spacing) {
        (Some(n), _) => n,
        (None, Some(sp)) => {
            let n = ((e_max.sqrt() - e_min.sqrt()) / sp) as usize;
            let t = e_min.sqrt() + n as f64 * sp;
            e_max = t * t;
            n
        }
        (None, None) => 1000,
    };
    let spacing = (e_max.sqrt() - e_min.sqrt()) / n_win as f64;
    if spacing > piece_width {
        return Err(NjoyError::WmpData("windowing: window spacing cannot be larger than piece spacing".into()));
    }

    // sort poles (and residues) by the pole, numpy's complex order
    let mut poles: Vec<Vec<Cf64>> = Vec::with_capacity(n_pieces);
    let mut residues: Vec<Vec<Vec<Cf64>>> = Vec::with_capacity(n_pieces);
    for ip in 0..n_pieces {
        let mut idx: Vec<usize> = (0..mp.poles[ip].len()).collect();
        idx.sort_by(|&a, &b| ccmp(mp.poles[ip][a], mp.poles[ip][b]));
        poles.push(idx.iter().map(|&k| mp.poles[ip][k]).collect());
        residues.push(mp.residues[ip].iter().map(|r| idx.iter().map(|&k| r[k]).collect()).collect());
    }
    let mut unused: Vec<Vec<bool>> = poles.iter().map(|p| vec![true; p.len()]).collect();
    let jj = Cf64::new(0.0, 1.0);

    let mut win_data: Vec<(usize, usize, usize, Vec<Vec<f64>>)> = Vec::with_capacity(n_win);
    for iw in 0..n_win {
        let inbegin = e_min.sqrt() + spacing * iw as f64;
        let inend = inbegin + spacing;
        let incenter = (inbegin + inend) / 2.0;
        let e_start = if iw == 0 || sqrt_alpha * inbegin < 4.0 {
            inbegin * inbegin
        } else {
            let t = sqrt_alpha * inbegin - 4.0;
            e_min.max(t * t / alpha)
        };
        let t = sqrt_alpha * inend + 4.0;
        let e_end = e_max.min(t * t / alpha);

        let i_piece = (n_pieces - 1).min(((inbegin - e_min.sqrt()) / piece_width + 0.5) as usize);
        let pp = &poles[i_piece];
        let rr: Vec<Vec<Cf64>> = residues[i_piece].iter().map(|r| r.iter().map(|x| x.mul(jj)).collect()).collect();
        let n_poles = pp.len();
        let nmt = rr.len();

        let n_points = (100usize.max(((e_end - e_start) * 4.0) as usize)).min(10000);
        let energy_sqrt = linspace(e_start.sqrt(), e_end.sqrt(), n_points);
        let energy: Vec<f64> = energy_sqrt.iter().map(|x| x * x).collect();
        let div_e = |rows: Vec<Vec<f64>>| -> Vec<Vec<f64>> {
            rows.into_iter().map(|r| r.into_iter().zip(&energy).map(|(x, e)| x / e).collect()).collect()
        };
        let xs_ref = div_e(evaluate(&energy_sqrt, pp, &rr, &[]));

        // curve-fit matrix, column-major n_points x (n_cf+1)
        let ncoef = n_cf + 1;
        let mut matrix = vec![0.0_f64; n_points * ncoef];
        for i in 0..ncoef {
            let p = 0.5 * i as f64 - 1.0;
            for (k, &e) in energy.iter().enumerate() {
                matrix[i * n_points + k] = numpy_power(e, p);
            }
        }

        // start from the pole nearest the window centre
        let mut center = 0usize;
        let mut best = f64::INFINITY;
        for (k, p) in pp.iter().enumerate() {
            let d = (p.re - incenter).abs();
            if d < best {
                best = d;
                center = k;
            }
        }
        let (mut lp, mut rp) = (center, center);
        let coefs = loop {
            let xs_wp = if rp > lp {
                let sub: Vec<Vec<Cf64>> = rr.iter().map(|r| r[lp..rp].to_vec()).collect();
                div_e(evaluate(&energy_sqrt, &pp[lp..rp], &sub, &[]))
            } else {
                vec![vec![0.0; n_points]; nmt]
            };
            let rhs: Vec<Vec<f64>> =
                (0..nmt).map(|m| (0..n_points).map(|k| xs_ref[m][k] - xs_wp[m][k]).collect()).collect();
            let rcond = f64::EPSILON * n_points.max(ncoef) as f64;
            let coefs = lstsq(n_points, ncoef, &matrix, &rhs, rcond)?; // [reaction][power]

            let mut abserr = vec![vec![0.0_f64; n_points]; nmt];
            let mut relerr = vec![vec![0.0_f64; n_points]; nmt];
            let mut any_nan = false;
            for m in 0..nmt {
                for k in 0..n_points {
                    let mut fit = 0.0_f64;
                    for i in 0..ncoef {
                        fit += matrix[i * n_points + k] * coefs[m][i];
                    }
                    let a = (fit + xs_wp[m][k] - xs_ref[m][k]).abs();
                    abserr[m][k] = a;
                    relerr[m][k] = a / xs_ref[m][k];
                    any_nan |= a.is_nan();
                }
            }
            if !any_nan {
                let total = (nmt * n_points) as f64;
                let mut re_size = 0usize;
                let mut all_le = true;
                let mut re_max = f64::NEG_INFINITY;
                let mut n_gt = 0usize;
                for m in 0..nmt {
                    for k in 0..n_points {
                        if abserr[m][k] > o.atol {
                            let r = relerr[m][k];
                            re_size += 1;
                            all_le &= r <= o.rtol;
                            re_max = if r.is_nan() || re_max.is_nan() { f64::NAN } else { re_max.max(r) };
                            if r > o.rtol {
                                n_gt += 1;
                            }
                        }
                    }
                }
                let mean_ok = iw == 0 && relerr.iter().all(|r| pairwise_sum(r) / n_points as f64 <= o.rtol);
                if re_size == 0
                    || all_le
                    || (re_max <= 2.0 * o.rtol && (n_gt as f64) <= 0.01 * total)
                    || mean_ok
                {
                    break coefs;
                }
            }
            if iw == 0 {
                return Err(NjoyError::WmpData("windowing: pure curvefit failed for the first window".into()));
            }
            // include the next centre-nearest pole (numpy's complex <=)
            if rp >= n_poles {
                if lp == 0 {
                    return Err(NjoyError::WmpData("windowing: every pole used and tolerance not met".into()));
                }
                lp -= 1;
            } else if lp == 0 || {
                let a = Cf64::new(pp[rp].re - incenter, pp[rp].im);
                let b = Cf64::new(incenter - pp[lp - 1].re, 0.0 - pp[lp - 1].im);
                ccmp(a, b) != Ordering::Greater
            } {
                rp += 1;
            } else {
                lp -= 1;
            }
        };
        // store [power][reaction]
        let cf: Vec<Vec<f64>> = (0..ncoef).map(|i| (0..nmt).map(|m| coefs[m][i]).collect()).collect();
        win_data.push((i_piece, lp, rp, cf));
        for u in &mut unused[i_piece][lp..rp] {
            *u = false;
        }
    }

    // flatten, dropping unused poles (multipole.py:696-716)
    let mut out_poles = Vec::new();
    let mut out_res = Vec::new();
    for ip in 0..n_pieces {
        for k in 0..poles[ip].len() {
            if !unused[ip][k] {
                out_poles.push(poles[ip][k]);
                out_res.push(residues[ip].iter().map(|r| r[k]).collect::<Vec<_>>());
            }
        }
    }
    let mut windows = Vec::with_capacity(n_win);
    let mut curvefit = Vec::with_capacity(n_win);
    for (ip, lp, rp, cf) in win_data {
        let n_prev: usize = (0..ip).map(|i| unused[i].len()).sum();
        let n_unused: usize =
            (0..ip).map(|i| unused[i].iter().filter(|&&u| u).count()).sum::<usize>() + unused[ip][..lp].iter().filter(|&&u| u).count();
        let l = lp as i64 + n_prev as i64 - n_unused as i64 + 1;
        let r = rp as i64 + n_prev as i64 - n_unused as i64;
        windows.push([l, r]);
        curvefit.push(cf);
    }
    Ok(WmpGenerated {
        name: mp.name.clone(),
        spacing,
        sqrt_awr: mp.awr.sqrt(),
        e_min,
        e_max,
        poles: out_poles,
        residues: out_res,
        broaden_poly: vec![true; windows.len()],
        windows,
        curvefit,
    })
}

/// `energy ** p` as numpy evaluates it for the exponents the curve fit
/// uses: `-1` is a reciprocal, `0` is one, `0.5` a square root and `1` the
/// value itself (numpy's scalar-exponent fast paths); anything else is `pow`.
fn numpy_power(e: f64, p: f64) -> f64 {
    if p == -1.0 {
        1.0 / e
    } else if p == 0.0 {
        1.0
    } else if p == 0.5 {
        e.sqrt()
    } else if p == 1.0 {
        e
    } else if p == 2.0 {
        e * e
    } else {
        e.r_powf(p)
    }
}

/// `WindowedMultipole.from_multipole`'s search settings.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Number of window counts in the search grid (`search_n_win = 20`).
    pub search_n_win: usize,
    /// Curve-fit orders to try, in order (`range(10, 1, -1)`).
    pub search_cf_orders: Vec<usize>,
    /// `rtol` passed to every windowing.
    pub rtol: f64,
    /// `atol` passed to every windowing.
    pub atol: f64,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self { search_n_win: 20, search_cf_orders: (2..=10).rev().collect(), rtol: 1e-3, atol: 1e-5 }
    }
}

/// `from_multipole(search=True)` (`multipole.py:1104-1148`): window the
/// multipole data over a grid of window counts and curve-fit orders and keep
/// the library with the best `-(10 poles/window + order + 0.01 windows)`.
pub fn search_windowing(mp: &MultipoleData, s: &SearchOptions) -> Result<WmpGenerated, NjoyError> {
    let n_poles: usize = mp.poles.iter().map(Vec::len).sum();
    let n_win_min = 5.max(n_poles / 20);
    let n_win_max = if n_poles < 2000 { 2000 } else { 8000 };
    let mut grid: Vec<usize> = linspace(n_win_min as f64, n_win_max as f64, s.search_n_win)
        .into_iter()
        .map(|x| x.floor() as usize)
        .collect();
    grid.sort_unstable();
    grid.dedup();
    let mut best: Option<(WmpGenerated, f64)> = None;
    for &n_w in &grid {
        for &n_cf in &s.search_cf_orders {
            let o = WindowingOptions { n_cf, rtol: s.rtol, atol: s.atol, n_win: Some(n_w), spacing: None };
            let wmp = match windowing(mp, &o) {
                Ok(w) => w,
                Err(_) => break,
            };
            let metric = -(wmp.poles_per_window() * 10.0 + wmp.fit_order() as f64 * 1.0 + wmp.n_windows() as f64 * 0.01);
            let better = match &best {
                None => true,
                Some((_, m)) => metric > *m,
            };
            if better {
                best = Some((wmp, metric));
            }
        }
    }
    best.map(|b| b.0).ok_or_else(|| NjoyError::WmpData("from_multipole: no windowing succeeded".into()))
}
