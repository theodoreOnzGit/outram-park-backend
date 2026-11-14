// SPDX-License-Identifier: AGPL-3.0-only
//
// The numpy / scipy primitives OpenMC's windowed-multipole generator calls,
// written to numpy's own algorithms where those are reproducible:
//
// - `interp`       numpy `np.interp` (numpy/_core/src/multiarray/compiled_base.c,
//                  `arr_interp`), BSD-3-Clause, (c) NumPy Developers;
// - `linspace`     numpy `np.linspace` (numpy/_core/function_base.py);
// - `pairwise_sum` numpy `pairwise_sum` (numpy/_core/src/umath/loops_utils.h.src);
// - `cdiv`         numpy's complex `divide` loop (Smith's method,
//                  numpy/_core/src/umath/loops.c.src);
// - `count_peaks`  scipy `_local_maxima_1d` (scipy/signal/_peak_finding_utils.pyx),
//                  BSD-3-Clause, (c) SciPy Developers.
//
// The LAPACK-backed calls (`scipy.linalg.lstsq` = DGELSD, `np.linalg.lstsq`
// = DGELSD, `scipy.linalg.eigvals` = DGEEV) are NOT reproduced bit for bit:
// they are re-expressed on PETIR's GSL ports (Golub-Reinsch SVD, Francis QR),
// with the same truncation rule. See `docs/wmp-generation.md` for the measured
// consequence.

//! numpy/scipy primitives for the WMP generator (gh:#1390).

use std::cmp::Ordering;

use petir::linalg::{eigen_nonsymm, Matrix, Svd};
use petir::mathf::RealMath;

use crate::wmp::Cf64;
use crate::NjoyError;

/// `np.interp(x, xp, fp)` with numpy's default `left = fp[0]`, `right = fp[-1]`.
///
/// Per point: `j` is the largest index with `xp[j] <= x` (numpy's
/// `binary_search_with_guess`); an exact hit or the last node returns `fp[j]`,
/// otherwise `slope*(x - xp[j]) + fp[j]` with the NaN fall-backs numpy uses.
pub fn interp(x: &[f64], xp: &[f64], fp: &[f64]) -> Vec<f64> {
    let n = xp.len();
    let mut out = Vec::with_capacity(x.len());
    if n == 0 {
        return out;
    }
    let (lval, rval) = (fp[0], fp[n - 1]);
    for &xv in x {
        if xv.is_nan() {
            out.push(xv);
            continue;
        }
        // largest j with xp[j] <= xv, or -1
        let j = xp.partition_point(|&v| v <= xv) as isize - 1;
        let y = if j < 0 {
            lval
        } else if xv > xp[n - 1] {
            rval
        } else {
            let j = j as usize;
            if j == n - 1 || xp[j] == xv {
                fp[j]
            } else {
                let slope = (fp[j + 1] - fp[j]) / (xp[j + 1] - xp[j]);
                let mut r = slope * (xv - xp[j]) + fp[j];
                if r.is_nan() {
                    r = slope * (xv - xp[j + 1]) + fp[j + 1];
                    if r.is_nan() && fp[j] == fp[j + 1] {
                        r = fp[j];
                    }
                }
                r
            }
        };
        out.push(y);
    }
    out
}

/// `np.linspace(start, stop, num)` (endpoint included): `y[i] = i*step + start`,
/// last value set to `stop`; `num == 1` gives `[start]`.
pub fn linspace(start: f64, stop: f64, num: usize) -> Vec<f64> {
    if num == 0 {
        return Vec::new();
    }
    if num == 1 {
        return vec![start];
    }
    let div = (num - 1) as f64;
    let delta = stop - start;
    let step = delta / div;
    let mut y: Vec<f64> = if step == 0.0 {
        (0..num).map(|i| (i as f64 / div) * delta + start).collect()
    } else {
        (0..num).map(|i| i as f64 * step + start).collect()
    };
    y[num - 1] = stop;
    y
}

/// numpy's `pairwise_sum` for a contiguous `f64` run (the reduction `np.sum`
/// and `np.mean` use along a contiguous axis).
pub fn pairwise_sum(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = -0.0_f64;
        for &v in a {
            res += v;
        }
        res
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for (k, rk) in r.iter_mut().enumerate() {
                *rk += a[i + k];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise_sum(&a[..n2]) + pairwise_sum(&a[n2..])
    }
}

/// numpy's complex division `a / b` (Smith's method, `loops.c.src`).
pub fn cdiv(a: Cf64, b: Cf64) -> Cf64 {
    let (in1r, in1i, in2r, in2i) = (a.re, a.im, b.re, b.im);
    let in2r_abs = in2r.abs();
    let in2i_abs = in2i.abs();
    if in2r_abs >= in2i_abs {
        if in2r_abs == 0.0 && in2i_abs == 0.0 {
            Cf64::new(in1r / in2r_abs, in1i / in2r_abs)
        } else {
            let rat = in2i / in2r;
            let scl = 1.0 / (in2r + in2i * rat);
            Cf64::new((in1r + in1i * rat) * scl, (in1i - in1r * rat) * scl)
        }
    } else {
        let rat = in2r / in2i;
        let scl = 1.0 / (in2i + in2r * rat);
        Cf64::new((in1r * rat + in1i) * scl, (in1i * rat - in1r) * scl)
    }
}

/// numpy's ordering of complex values: by real part, then imaginary part.
pub fn ccmp(a: Cf64, b: Cf64) -> Ordering {
    match a.re.partial_cmp(&b.re) {
        Some(Ordering::Equal) => a.im.partial_cmp(&b.im).unwrap_or(Ordering::Equal),
        Some(o) => o,
        None => Ordering::Equal,
    }
}

/// `np.abs` of a complex value.
pub fn cabs(a: Cf64) -> f64 {
    a.re.r_hypot(a.im)
}

/// `np.isclose(a, b)` for complex values (`rtol = 1e-5`, `atol = 1e-8`).
pub fn cisclose(a: Cf64, b: Cf64) -> bool {
    cabs(a.sub(b)) <= 1e-8 + 1e-5 * cabs(b)
}

/// The number of peaks `scipy.signal.find_peaks(x)` returns with no
/// conditions: `_local_maxima_1d`, plateaus counted once.
pub fn count_peaks(x: &[f64]) -> usize {
    let n = x.len();
    if n < 3 {
        return 0;
    }
    let i_max = n - 1;
    let mut m = 0;
    let mut i = 1;
    while i < i_max {
        if x[i - 1] < x[i] {
            let mut i_ahead = i + 1;
            while i_ahead < i_max && x[i_ahead] == x[i] {
                i_ahead += 1;
            }
            if x[i_ahead] < x[i] {
                m += 1;
                i = i_ahead;
            }
        }
        i += 1;
    }
    m
}

/// `np.searchsorted(a, v, side='right')`.
pub fn searchsorted_right(a: &[f64], v: f64) -> usize {
    a.partition_point(|&x| x <= v)
}

/// `np.searchsorted(a, v, side='left')`.
pub fn searchsorted_left(a: &[f64], v: f64) -> usize {
    a.partition_point(|&x| x < v)
}

/// A Householder QR factorisation, column-major, LAPACK's `DGEQR2`
/// convention (`DLARFG`: `beta = -sign(alpha) * ||x||`). Unblocked, so it is
/// not OpenBLAS's blocked `DGEQRF` to the bit; the factors agree to
/// round-off and `R`'s row signs follow the same rule.
///
/// Written here rather than taken from `petir::linalg::QrDecomposition`
/// (GSL's, row-major) because the vector-fitting loop factors thousands of
/// tall `(2 n_s + 1) x (2 N + 1)` matrices and needs contiguous columns.
#[derive(Debug, Clone)]
pub struct HouseholderQr {
    m: usize,
    /// Column-major: `R` on and above the diagonal, reflectors below.
    a: Vec<f64>,
    tau: Vec<f64>,
}

impl HouseholderQr {
    /// Factor the column-major `m x n` matrix `a`.
    pub fn new(m: usize, n: usize, mut a: Vec<f64>) -> Self {
        let k = m.min(n);
        let mut tau = vec![0.0_f64; k];
        for i in 0..k {
            let (head, tail) = a.split_at_mut((i + 1) * m);
            let col = &mut head[i * m..];
            let alpha = col[i];
            let mut ss = 0.0_f64;
            for &x in &col[i + 1..m] {
                ss += x * x;
            }
            if ss == 0.0 {
                tau[i] = 0.0;
                continue;
            }
            let norm = alpha.r_hypot(ss.sqrt());
            let beta = if alpha >= 0.0 { -norm } else { norm };
            tau[i] = (beta - alpha) / beta;
            let scal = 1.0 / (alpha - beta);
            for x in &mut col[i + 1..m] {
                *x *= scal;
            }
            col[i] = beta;
            let t = tau[i];
            for j in 0..(n - i - 1) {
                let cj = &mut tail[j * m..(j + 1) * m];
                let mut w = cj[i];
                for r in i + 1..m {
                    w += col[r] * cj[r];
                }
                let tw = t * w;
                cj[i] -= tw;
                for r in i + 1..m {
                    cj[r] -= tw * col[r];
                }
            }
        }
        Self { m, a, tau }
    }

    /// `R[i, j]` (zero below the diagonal).
    pub fn r(&self, i: usize, j: usize) -> f64 {
        if i > j { 0.0 } else { self.a[j * self.m + i] }
    }

    /// Overwrite `v` (length `m`) with `Q^T v`.
    pub fn apply_qt(&self, v: &mut [f64]) {
        for (i, &t) in self.tau.iter().enumerate() {
            if t == 0.0 {
                continue;
            }
            let col = &self.a[i * self.m..(i + 1) * self.m];
            let mut w = v[i];
            for r in i + 1..self.m {
                w += col[r] * v[r];
            }
            let tw = t * w;
            v[i] -= tw;
            for r in i + 1..self.m {
                v[r] -= tw * col[r];
            }
        }
    }
}

/// Minimum-norm least squares, the DGELSD contract: singular values
/// `s_i <= rcond * s_max` are treated as zero. `a` is `m x n` **column-major**
/// and `b` holds right-hand sides of length `m`. Returns one solution of
/// length `n` per right-hand side.
///
/// As DGELSD does for a tall matrix, `A` is first reduced by QR; the SVD of
/// the `n x n` triangle (PETIR's Golub-Reinsch, `gsl_linalg_SV_decomp`, in
/// place of LAPACK's divide and conquer) then gives the truncated solve. So
/// the result agrees with scipy/numpy to round-off, not bits. An `m < n`
/// system is padded with zero rows, which leaves the minimum-norm solution
/// unchanged.
pub fn lstsq(m: usize, n: usize, a: &[f64], b: &[Vec<f64>], rcond: f64) -> Result<Vec<Vec<f64>>, NjoyError> {
    let rows = m.max(n);
    let mut colmajor = vec![0.0_f64; rows * n];
    for j in 0..n {
        colmajor[j * rows..j * rows + m].copy_from_slice(&a[j * m..(j + 1) * m]);
    }
    let qr = HouseholderQr::new(rows, n, colmajor);
    let mut rdata = vec![0.0_f64; n * n];
    for i in 0..n {
        for j in i..n {
            rdata[i * n + j] = qr.r(i, j);
        }
    }
    let mat = Matrix::from_row_major(n, n, rdata).map_err(|e| NjoyError::WmpData(format!("lstsq: {e:?}")))?;
    let svd = Svd::new(&mat).map_err(|e| NjoyError::WmpData(format!("lstsq svd: {e:?}")))?;
    let s = svd.s();
    let smax = s.iter().cloned().fold(0.0_f64, f64::max);
    let thresh = rcond * smax;
    let u = svd.u();
    let v = svd.v();
    let mut out = Vec::with_capacity(b.len());
    for rhs in b {
        let mut c = vec![0.0_f64; rows];
        c[..m].copy_from_slice(&rhs[..m]);
        qr.apply_qt(&mut c);
        let mut x = vec![0.0_f64; n];
        for (k, &sk) in s.iter().enumerate() {
            if !(sk > thresh) {
                continue;
            }
            let mut ub = 0.0_f64;
            for (i, &ci) in c.iter().enumerate().take(n) {
                ub += u.get(i, k) * ci;
            }
            let coef = ub / sk;
            for (j, xj) in x.iter_mut().enumerate() {
                *xj += v.get(j, k) * coef;
            }
        }
        out.push(x);
    }
    Ok(out)
}

/// `scipy.linalg.eigvals` of a real square matrix (DGEEV, balanced), on
/// PETIR's balanced Francis double-shift QR.
///
/// DGEEV returns each complex-conjugate pair adjacent, the positive imaginary
/// part first; the generator relies on that (its conjugate-pair labelling and
/// merge read pairs as neighbours), so the pairs are put in that order here.
/// The order *between* eigenvalues follows PETIR's deflation order, which can
/// differ from LAPACK's; the generator's results do not depend on it beyond
/// round-off (poles are sorted before windowing).
pub fn eigvals(n: usize, a: &[f64]) -> Result<Vec<Cf64>, NjoyError> {
    let mat = Matrix::from_row_major(n, n, a.to_vec()).map_err(|e| NjoyError::WmpData(format!("eigvals: {e:?}")))?;
    let ev = eigen_nonsymm(&mat, true).map_err(|e| NjoyError::WmpData(format!("eigvals: {e:?}")))?;
    let mut vals: Vec<Cf64> = ev.iter().map(|z| Cf64::new(z.re, z.im)).collect();
    let mut i = 0;
    while i < vals.len() {
        if vals[i].im != 0.0 && i + 1 < vals.len() && vals[i + 1].re == vals[i].re && vals[i + 1].im == -vals[i].im {
            if vals[i].im < 0.0 {
                vals.swap(i, i + 1);
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(vals)
}
