// SPDX-License-Identifier: AGPL-3.0-only
//
// Translated from OpenMC `openmc/data/vectfit.py` at commit
// 8f79d9d02483c90e4134f03a9df4b75ce6b062bf (2026-11-10), lines cited per
// function. OpenMC is MIT-licensed:
//
//   Copyright (c) 2011-2025 Massachusetts Institute of Technology, UChicago
//   Argonne LLC, and OpenMC contributors
//
//   Permission is hereby granted, free of charge, to any person obtaining a
//   copy of this software and associated documentation files (the
//   "Software"), to deal in the Software without restriction, including
//   without limitation the rights to use, copy, modify, merge, publish,
//   distribute, sublicense, and/or sell copies of the Software, and to permit
//   persons to whom the Software is furnished to do so, subject to the
//   following conditions: The above copyright notice and this permission
//   notice shall be included in all copies or substantial portions of the
//   Software. THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND.
//
// Upstream credits the algorithm to B. Gustavsen (VECTFIT, MATLAB) and
// J. Liang (mit-crpg/vectfit, C++); references [1]-[3] of the upstream module
// doc: Gustavsen & Semlyen 1999, Gustavsen 2006, Deschrijver et al. 2008.

//! Fast Relaxed Vector Fitting: `f(s) ~ sum_k r_k / (s - p_k)` with one pole
//! set shared by every response vector. A port of OpenMC's `vectfit.py`.
//!
//! Only the real-response case is ported: OpenMC's one caller
//! (`_vectfit_xs`) passes `f = sigma * E`, which is real. The polynomial
//! terms (`n_polys`) are ported but that caller never uses them.

use crate::wmp::Cf64;
use crate::NjoyError;

use super::npy::{cdiv, cisclose, eigvals, lstsq, HouseholderQr};

const TOL_LOW: f64 = 1e-18;
const TOL_HIGH: f64 = 1e18;

/// What one `vectfit` call returns (`vectfit.py:132-221`).
#[derive(Debug, Clone)]
pub struct VectfitResult {
    /// The relocated poles (eigenvalues), or the input poles when the pole
    /// update is skipped.
    pub poles: Vec<Cf64>,
    /// Residues, `[vector][pole]`.
    pub residues: Vec<Vec<Cf64>>,
    /// Polynomial coefficients, `[vector][term]`.
    pub poly: Vec<Vec<f64>>,
    /// The fitted response, `[vector][sample]`.
    pub fit: Vec<Vec<f64>>,
    /// Root-mean-square fitting error.
    pub rms: f64,
}

/// Evaluate `sum_k r_k / (s - p_k) + sum_j c_j s^j` (`vectfit.py:57-129`),
/// real part, `[vector][sample]`.
pub fn evaluate(s: &[f64], poles: &[Cf64], residues: &[Vec<Cf64>], poly: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let one = Cf64::new(1.0, 0.0);
    // 1 / (s - p), [pole][sample]
    let inv: Vec<Vec<Cf64>> = poles
        .iter()
        .map(|p| s.iter().map(|&x| cdiv(one, Cf64::new(x - p.re, 0.0 - p.im))).collect())
        .collect();
    let mut out = Vec::with_capacity(residues.len());
    for (v, res) in residues.iter().enumerate() {
        let mut row = vec![0.0_f64; s.len()];
        for (i, r) in row.iter_mut().enumerate() {
            let mut acc = Cf64::new(0.0, 0.0);
            for (k, rk) in res.iter().enumerate() {
                acc = acc.add(rk.mul(inv[k][i]));
            }
            *r = acc.re;
        }
        if let Some(c) = poly.get(v) {
            for (i, r) in row.iter_mut().enumerate() {
                let mut pw = 1.0_f64;
                let mut add = 0.0_f64;
                for &cj in c {
                    add += cj * pw;
                    pw *= s[i];
                }
                if !c.is_empty() {
                    *r += add;
                }
            }
        }
        out.push(row);
    }
    out
}

/// `label_conjugate_poles` (`vectfit.py:771-811`): 0 real, 1 first of a
/// conjugate pair, 2 second of a pair.
pub fn label_conjugate_poles(poles: &[Cf64]) -> Result<Vec<u8>, NjoyError> {
    let n = poles.len();
    let mut idx = vec![0u8; n];
    let is_complex: Vec<bool> = poles.iter().map(|p| p.im != 0.0).collect();
    let starts: Vec<bool> = (0..n.saturating_sub(1))
        .map(|i| is_complex[i] && cisclose(poles[i].conj(), poles[i + 1]))
        .collect();
    for (i, &st) in starts.iter().enumerate() {
        if st {
            idx[i] = 1;
        }
    }
    for (i, &st) in starts.iter().enumerate() {
        if st {
            idx[i + 1] = 2;
        }
    }
    if (0..n).any(|i| is_complex[i] && idx[i] == 0) {
        return Err(NjoyError::WmpData("vectfit: complex poles must appear in conjugate pairs".into()));
    }
    Ok(idx)
}

/// `compute_dk_matrix` (`vectfit.py:224-281`), row-major `[sample][col]`
/// with `ncols` columns.
fn compute_dk(
    s: &[f64],
    poles: &[Cf64],
    conj: &[u8],
    num_polys: usize,
    ncols: usize,
    tol_high: Option<f64>,
) -> Vec<Cf64> {
    let n = poles.len();
    let one = Cf64::new(1.0, 0.0);
    let jj = Cf64::new(0.0, 1.0);
    let mut dk = vec![Cf64::new(0.0, 0.0); s.len() * ncols];
    for (i, &x) in s.iter().enumerate() {
        for (k, p) in poles.iter().enumerate() {
            let d1 = Cf64::new(x - p.re, 0.0 - p.im);
            let d2 = Cf64::new(x - p.re, 0.0 + p.im);
            let v = match conj[k] {
                0 => cdiv(one, d1),
                1 => cdiv(one, d1).add(cdiv(one, d2)),
                _ => cdiv(jj, d2).sub(cdiv(jj, d1)),
            };
            dk[i * ncols + k] = v;
        }
        if let Some(th) = tol_high {
            for k in 0..ncols {
                let v = dk[i * ncols + k];
                if v.re.is_infinite() || v.im.is_infinite() {
                    dk[i * ncols + k] = Cf64::new(th, 0.0);
                }
            }
        }
        let mut pw = 1.0_f64;
        for j in 0..num_polys {
            dk[i * ncols + n + j] = Cf64::new(pw, 0.0);
            pw *= x;
        }
    }
    dk
}

/// `row_block_matrix` (`vectfit.py:284-332`) split into real and imaginary
/// parts, column-major `(2 n_s [+1]) x (2N + n_poly + 1)`, with `extra_rows`
/// zero rows appended.
fn row_block(dk: &[Cf64], ncols_dk: usize, w: &[f64], f: &[f64], n: usize, np: usize, extra_rows: usize) -> (usize, usize, Vec<f64>) {
    let ns = w.len();
    let cols = n + np + n + 1;
    let rows = 2 * ns + extra_rows;
    let mut a = vec![0.0_f64; rows * cols];
    for i in 0..ns {
        let wi = w[i];
        for c in 0..n + np {
            let d = dk[i * ncols_dk + c];
            a[c * rows + i] = wi * d.re;
            a[c * rows + ns + i] = wi * d.im;
        }
        let mw = -wi;
        for c in 0..n + 1 {
            let d = dk[i * ncols_dk + c];
            let col = n + np + c;
            a[col * rows + i] = (mw * d.re) * f[i];
            a[col * rows + ns + i] = (mw * d.im) * f[i];
        }
    }
    (rows, cols, a)
}

/// `wlstsq` (`vectfit.py:39-54`): scale columns to unit norm, solve by
/// `scipy.linalg.lstsq` (DGELSD, `cond = eps`), unscale.
fn wlstsq(m: usize, n: usize, a_rowmajor: &[f64], b: &[f64]) -> Result<Vec<f64>, NjoyError> {
    let mut scale = vec![1.0_f64; n];
    for (j, sc) in scale.iter_mut().enumerate() {
        let mut ss = 0.0_f64;
        for i in 0..m {
            let v = a_rowmajor[i * n + j];
            ss += v * v;
        }
        let cn = ss.sqrt();
        if cn > 0.0 {
            *sc = 1.0 / cn;
        }
        if !sc.is_finite() {
            *sc = 1.0;
        }
    }
    let mut colmajor = vec![0.0_f64; m * n];
    for i in 0..m {
        for j in 0..n {
            colmajor[j * m + i] = a_rowmajor[i * n + j] * scale[j];
        }
    }
    let sol = lstsq(m, n, &colmajor, &[b.to_vec()], f64::EPSILON)?;
    Ok(sol[0].iter().zip(scale.iter()).map(|(x, s)| x * s).collect())
}

/// `identify_poles` (`vectfit.py:486-633`): relaxed pole relocation.
fn identify_poles(poles: &[Cf64], s: &[f64], w: &[Vec<f64>], f: &[Vec<f64>], np: usize) -> Result<Vec<Cf64>, NjoyError> {
    let n = poles.len();
    let ns = s.len();
    let nv = f.len();
    let conj = label_conjugate_poles(poles)?;
    let ncols_dk = n + np.max(1);
    let mut dk = compute_dk(s, poles, &conj, np, ncols_dk, Some(TOL_HIGH));
    if np == 0 {
        for i in 0..ns {
            dk[i * ncols_dk + n] = Cf64::new(1.0, 0.0);
        }
    }
    // scale_factor = sqrt(sum_m ||w_m f_m||^2) / n_s
    let mut tot = 0.0_f64;
    for m in 0..nv {
        let mut ss = 0.0_f64;
        for i in 0..ns {
            let v = w[m][i] * f[m][i];
            ss += v * v;
        }
        let nm = ss.sqrt();
        tot += nm * nm;
    }
    let scale_factor = tot.sqrt() / ns as f64;

    // constrained blocks (vectfit.py:335-418)
    let nb = n + 1;
    let mut lhs = vec![0.0_f64; nv * nb * nb];
    let mut rhs = vec![0.0_f64; nv * nb];
    for v in 0..nv {
        let (rows, cols, mut a) = row_block(&dk, ncols_dk, &w[v], &f[v], n, np, 1);
        if v == nv - 1 {
            for c in 0..nb {
                let mut sum = 0.0_f64;
                for i in 0..ns {
                    sum += dk[i * ncols_dk + c].re;
                }
                a[(n + np + c) * rows + 2 * ns] = scale_factor * sum;
            }
        }
        if rows < cols {
            return Err(NjoyError::WmpData(format!("vectfit: {rows} rows < {cols} columns")));
        }
        let qr = HouseholderQr::new(rows, cols, a);
        let off = n + np;
        for r in 0..nb {
            for c in 0..nb {
                lhs[(v * nb + r) * nb + c] = qr.r(off + r, off + c);
            }
        }
        if v == nv - 1 {
            // Q[-1, off..off+nb] = (Q^T e_last)[off..]
            let mut e = vec![0.0_f64; rows];
            e[rows - 1] = 1.0;
            qr.apply_qt(&mut e);
            let k = ns as f64 * scale_factor;
            for r in 0..nb {
                rhs[v * nb + r] = k * e[off + r];
            }
        }
    }
    let sol = wlstsq(nv * nb, nb, &lhs, &rhs)?;
    let mut coeffs = sol[..n].to_vec();
    let mut denom = sol[n];

    if denom.abs() < TOL_LOW || denom.abs() > TOL_HIGH {
        if denom == 0.0 {
            denom = 1.0;
        } else if denom.abs() < TOL_LOW {
            denom = denom.signum() * TOL_LOW;
        } else if denom.abs() > TOL_HIGH {
            denom = denom.signum() * TOL_HIGH;
        }
        // unconstrained blocks (vectfit.py:421-483)
        let mut lhs = vec![0.0_f64; nv * n * n];
        let mut rhs = vec![0.0_f64; nv * n];
        for v in 0..nv {
            let (rows, cols, a) = row_block(&dk, ncols_dk, &w[v], &f[v], n, np, 0);
            if rows < cols {
                return Err(NjoyError::WmpData(format!("vectfit: {rows} rows < {cols} columns")));
            }
            let mut b = vec![0.0_f64; rows];
            for i in 0..ns {
                b[i] = denom * w[v][i] * f[v][i];
            }
            let qr = HouseholderQr::new(rows, cols, a);
            qr.apply_qt(&mut b);
            let off = n + np;
            for r in 0..n {
                for c in 0..n {
                    lhs[(v * n + r) * n + c] = qr.r(off + r, off + c);
                }
                rhs[v * n + r] = b[off + r];
            }
        }
        coeffs = wlstsq(nv * n, n, &lhs, &rhs)?;
    }

    // lambda - outer(scale, coeffs) / denom  (vectfit.py:604-633)
    let mut lam = vec![0.0_f64; n * n];
    let mut sv = vec![1.0_f64; n];
    for k in 0..n {
        match conj[k] {
            0 => lam[k * n + k] = poles[k].re,
            1 => {
                let (re, im) = (poles[k].re, poles[k].im);
                lam[k * n + k] = re;
                lam[(k + 1) * n + k + 1] = re;
                lam[k * n + k + 1] = im;
                lam[(k + 1) * n + k] = -im;
                sv[k] = 2.0;
                sv[k + 1] = 0.0;
            }
            _ => {}
        }
    }
    let mut h = vec![0.0_f64; n * n];
    for r in 0..n {
        for c in 0..n {
            h[r * n + c] = lam[r * n + c] - (sv[r] * coeffs[c]) / denom;
        }
    }
    eigvals(n, &h)
}

/// `identify_residues` (`vectfit.py:636-768`).
fn identify_residues(
    poles: &[Cf64],
    s: &[f64],
    w: &[Vec<f64>],
    f: &[Vec<f64>],
    np: usize,
) -> Result<(Vec<Vec<Cf64>>, Vec<Vec<f64>>, Vec<Vec<f64>>, f64), NjoyError> {
    let n = poles.len();
    let ns = s.len();
    let nv = f.len();
    let conj = label_conjugate_poles(poles)?;
    let ncols = n + np;
    let dk = compute_dk(s, poles, &conj, np, ncols, None);
    let mut residues = vec![vec![Cf64::new(0.0, 0.0); n]; nv];
    let mut poly = vec![vec![0.0_f64; np]; nv];
    for v in 0..nv {
        let m = 2 * ns;
        let mut a = vec![0.0_f64; m * ncols];
        for i in 0..ns {
            for c in 0..ncols {
                let d = dk[i * ncols + c];
                a[i * ncols + c] = d.re * w[v][i];
                a[(ns + i) * ncols + c] = d.im * w[v][i];
            }
        }
        let mut b = vec![0.0_f64; m];
        for i in 0..ns {
            b[i] = w[v][i] * f[v][i];
        }
        let x = wlstsq(m, ncols, &a, &b)?;
        for k in 0..n {
            match conj[k] {
                0 => residues[v][k] = Cf64::new(x[k], 0.0),
                1 => {
                    residues[v][k] = Cf64::new(x[k], x[k + 1]);
                    residues[v][k + 1] = Cf64::new(x[k], -x[k + 1]);
                }
                _ => {}
            }
        }
        poly[v].copy_from_slice(&x[n..n + np]);
    }
    let fit = evaluate(s, poles, &residues, &poly);
    let mut ss = 0.0_f64;
    for v in 0..nv {
        for i in 0..ns {
            let d = fit[v][i] - f[v][i];
            ss += d * d;
        }
    }
    let rms = ss.sqrt() / ((nv * ns) as f64).sqrt();
    Ok((residues, poly, fit, rms))
}

/// `vectfit` (`vectfit.py:132-221`). `f` and `weights` are `[vector][sample]`.
pub fn vectfit(
    f: &[Vec<f64>],
    s: &[f64],
    initial_poles: &[Cf64],
    weights: &[Vec<f64>],
    n_polys: usize,
    skip_pole_update: bool,
    skip_residue_update: bool,
) -> Result<VectfitResult, NjoyError> {
    if n_polys > 11 {
        return Err(NjoyError::WmpData("vectfit: n_polys must be in [0, 11]".into()));
    }
    let nv = f.len();
    let ns = s.len();
    let n = initial_poles.len();
    let mut out = VectfitResult {
        poles: initial_poles.to_vec(),
        residues: vec![vec![Cf64::new(0.0, 0.0); n]; nv],
        poly: vec![vec![0.0; n_polys]; nv],
        fit: vec![vec![0.0; ns]; nv],
        rms: 0.0,
    };
    if n == 0 && n_polys == 0 {
        let mut ss = 0.0_f64;
        for row in f {
            for &x in row {
                ss += x * x;
            }
        }
        out.rms = ss.sqrt() / ((nv * ns) as f64).sqrt();
        return Ok(out);
    }
    if !skip_pole_update && n > 0 {
        out.poles = identify_poles(initial_poles, s, weights, f, n_polys)?;
    }
    if !skip_residue_update {
        let (r, p, fit, rms) = identify_residues(&out.poles, s, weights, f, n_polys)?;
        out.residues = r;
        out.poly = p;
        out.fit = fit;
        out.rms = rms;
    }
    Ok(out)
}
