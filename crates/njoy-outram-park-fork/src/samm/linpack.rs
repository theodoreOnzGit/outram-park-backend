//! Complex-symmetric packed linear solver — ported from `samm.f90`'s
//! `xspfa`/`xspsl` (and their BLAS-1 helpers `xaxpy`/`xdot`/`xswap`/
//! `ixamax`), the LINPACK `CSPFA`/`CSPSL` routines (Bunch-Kaufman
//! diagonal-pivoting factorization) adapted to complex-*symmetric*
//! (not Hermitian) matrices in packed upper-triangular storage.
//!
//! This is the `n>=4`-channel path of `yfour`: for 4 or more R-matrix
//! channels there is no closed-form inverse (unlike the 1-, 2-, and
//! 3-channel cases in [`super::onech`]/[`super::twoch`]/
//! [`super::threech`]), so the level matrix `Y = I - R*L` is factored once
//! via [`xspfa`] and then solved once per unit column via [`xspsl`] to
//! build up `Y^-1` column by column (see `yfour`, not yet ported).
//!
//! # Storage convention
//!
//! A packed complex-symmetric matrix of order `n` is stored as two
//! parallel `Vec<f64>` (real, imaginary), length `n*(n+1)/2`, holding the
//! upper triangle column by column — position `k` (1-indexed) holds
//! `A(i,j)` where `k = i + j*(j-1)/2`, `i<=j` (Fortran's own packed-storage
//! convention, documented in [`xspfa`]'s doc comment).
//!
//! **Indices in this module's bodies are kept numerically identical to the
//! Fortran source's 1-indexed flat offsets** (`ik`, `ij`, `kk`, `jk`, …)
//! rather than translated to a 0-indexed scheme, with every actual array
//! access going through the private `g`/`s` (get/set) helpers that apply
//! the `-1` — this is deliberately less idiomatic but far more directly
//! checkable line-by-line against the original, which matters more here
//! than anywhere else in this port: a single off-by-one in a pivoting
//! routine corrupts results silently rather than panicking.
//!
//! # Scope-matching simplification
//!
//! Upstream's `xaxpy`/`xdot`/`xswap` are generic BLAS-1 routines accepting
//! arbitrary strides (`incx`/`incy`) and use a manually unrolled loop for
//! the stride-1 case (a 1970s performance trick). Every call site in
//! `samm.f90` passes `incx=incy=1` (verified by grep across the whole
//! file) — so only the stride-1 path is ported, and the manual unrolling
//! is dropped (meaningless with a modern optimizing compiler; the loop
//! bodies are otherwise identical). `xdot` is additionally inlined at its
//! two call sites in [`xspsl`] rather than kept as a free function, since
//! Rust has no direct equivalent of Fortran's "function with an extra
//! output parameter" (`xdoti`) signature.

/// A packed complex-symmetric matrix (see this module's doc comment for
/// the storage convention).
pub struct PackedComplexMatrix {
    pub re: Vec<f64>,
    pub im: Vec<f64>,
}

impl PackedComplexMatrix {
    pub fn zeros(n: usize) -> Self {
        let len = n * (n + 1) / 2;
        Self {
            re: vec![0.0; len],
            im: vec![0.0; len],
        }
    }
}

#[inline]
fn g(v: &[f64], k: i64) -> f64 {
    v[(k - 1) as usize]
}
#[inline]
fn s(v: &mut [f64], k: i64, x: f64) {
    v[(k - 1) as usize] = x;
}
#[inline]
fn swap1(v: &mut [f64], a: i64, b: i64) {
    v.swap((a - 1) as usize, (b - 1) as usize);
}

/// `y[y0+1 ..= y0+n] += (sa + i*sai) * x[x0+1 ..= x0+n]` (upstream `xaxpy`,
/// stride-1 only), where `x0`/`y0` are 1-indexed Fortran offsets (so
/// `x[x0+1]` is the first element touched, matching a Fortran call like
/// `xaxpy(n, sa, sai, ap(1,x0+1), 1, ap(1,y0+1), 1)`).
fn xaxpy(n: i64, sa: f64, sai: f64, re: &mut [f64], im: &mut [f64], x0: i64, y0: i64) {
    if n <= 0 || (sa == 0.0 && sai == 0.0) {
        return;
    }
    for i in 1..=n {
        let xr = g(re, x0 + i);
        let xi = g(im, x0 + i);
        let aa = g(re, y0 + i) + sa * xr - sai * xi;
        let new_im = g(im, y0 + i) + sa * xi + sai * xr;
        s(im, y0 + i, new_im);
        s(re, y0 + i, aa);
    }
}

/// `sum(x[x0+1..=x0+n] * y[y0+1..=y0+n])` (complex, not conjugated) —
/// upstream `xdot`, stride-1 only. Returns `(real, imag)`.
fn xdot(n: i64, re: &[f64], im: &[f64], x0: i64, y0: i64) -> (f64, f64) {
    let mut stemp = 0.0_f64;
    let mut stempi = 0.0_f64;
    for i in 1..=n {
        let xr = g(re, x0 + i);
        let xi = g(im, x0 + i);
        let yr = g(re, y0 + i);
        let yi = g(im, y0 + i);
        stemp += xr * yr - xi * yi;
        stempi += xi * yr + xr * yi;
    }
    (stemp, stempi)
}

/// Swap `x[x0+1..=x0+n]` and `y[y0+1..=y0+n]` — upstream `xswap`,
/// stride-1 only.
fn xswap(n: i64, re: &mut [f64], im: &mut [f64], x0: i64, y0: i64) {
    for i in 1..=n {
        re.swap((x0 + i - 1) as usize, (y0 + i - 1) as usize);
        im.swap((x0 + i - 1) as usize, (y0 + i - 1) as usize);
    }
}

/// 1-indexed position (within `1..=n`, relative to `x0`) of the element of
/// largest squared modulus in `x[x0+1..=x0+n]` — upstream `ixamax`,
/// stride-1 only. Returns `0` if `n<1` (matching upstream's `ixamax=0`
/// default).
fn ixamax(n: i64, re: &[f64], im: &[f64], x0: i64) -> i64 {
    if n < 1 {
        return 0;
    }
    if n == 1 {
        return 1;
    }
    let mut imax = 1_i64;
    let mut smax = g(re, x0 + 1).powi(2) + g(im, x0 + 1).powi(2);
    for i in 2..=n {
        let aa = g(re, x0 + i).powi(2) + g(im, x0 + i).powi(2);
        if aa > smax {
            imax = i;
            smax = aa;
        }
    }
    imax
}

/// Bunch-Kaufman factorization `A = U*D*U^T` of a packed complex-symmetric
/// matrix (`U` a product of permutation and unit upper-triangular
/// matrices, `D` block-diagonal with 1x1/2x2 blocks) — ported from
/// `xspfa` (`samm.f90:5492-5775`, itself LINPACK's `CSPFA` adapted from
/// Hermitian to symmetric). `a` is factored **in place**.
///
/// Returns `(kpvt, info)`: `kpvt[k-1]` is the pivot index for step `k`
/// (negative encodes "this is part of a 2x2 block with the adjacent
/// index", matching upstream exactly); `info=0` normally, or `info=k` if
/// the `k`-th pivot block turned out to be (numerically) singular — not
/// fatal here, but [`xspsl`] may divide by zero if called with such a
/// factorization, matching upstream's own documented behavior.
pub fn xspfa(a: &mut PackedComplexMatrix, n: i64) -> (Vec<i64>, i64) {
    let alpha = (1.0 + 17.0_f64.sqrt()) / 8.0;
    let mut kpvt = vec![0_i64; n as usize];
    let mut info = 0_i64;
    let re = &mut a.re;
    let im = &mut a.im;

    let mut k = n;
    let mut ik = (n * (n - 1)) / 2;
    loop {
        if k == 0 {
            break;
        }
        if k <= 1 {
            kpvt[0] = 1;
            if g(re, 1) == 0.0 && g(im, 1) == 0.0 {
                info = 1;
            }
            break;
        }

        let km1 = k - 1;
        let kk = ik + k;
        let absakk = g(re, kk).powi(2) + g(im, kk).powi(2);

        let imax = ixamax(k - 1, re, im, ik);
        let imk = ik + imax;
        let colmax = g(re, imk).powi(2) + g(im, imk).powi(2);

        let kstep;
        let swap;
        let mut im_val = 0_i64; // upstream's `im` local -- only meaningful when swap=true.
        if absakk >= alpha * colmax {
            kstep = 1;
            swap = false;
        } else {
            let mut rowmax = 0.0_f64;
            let imaxp1 = imax + 1;
            im_val = (imax * (imax - 1)) / 2;
            let mut imj = im_val + 2 * imax;
            let mut j = imaxp1;
            while j <= k {
                let aa = g(re, imj).powi(2) + g(im, imj).powi(2);
                rowmax = rowmax.max(aa);
                imj += j;
                j += 1;
            }
            if imax != 1 {
                let jmax = ixamax(imax - 1, re, im, im_val);
                let jmim = jmax + im_val;
                let aa = g(re, jmim).powi(2) + g(im, jmim).powi(2);
                rowmax = rowmax.max(aa);
            }
            let imim = imax + im_val;
            let aa = g(re, imim).powi(2) + g(im, imim).powi(2);
            if aa >= alpha * rowmax {
                kstep = 1;
                swap = true;
            } else if absakk >= alpha * colmax * (colmax / rowmax) {
                kstep = 1;
                swap = false;
            } else {
                kstep = 2;
                swap = imax != km1;
            }
        }

        if absakk.max(colmax) == 0.0 {
            // Column k is (numerically) zero.
            kpvt[(k - 1) as usize] = k;
            info = k;
        } else if kstep != 2 {
            // 1x1 pivot block.
            if swap {
                xswap(imax, re, im, im_val, ik);
                let mut imj = ik + imax;
                let mut jj = imax;
                while jj <= k {
                    let j = k + imax - jj;
                    let jk = ik + j;
                    swap1(re, jk, imj);
                    swap1(im, jk, imj);
                    imj -= j - 1;
                    jj += 1;
                }
            }

            let mut ij = ik - (k - 1);
            for jj in 1..=km1 {
                let j = k - jj;
                let jk = ik + j;
                let aa = g(re, kk).powi(2) + g(im, kk).powi(2);
                let dmulk = -(g(re, jk) * g(re, kk) + g(im, jk) * g(im, kk)) / aa;
                let dmulki = (g(re, jk) * g(im, kk) - g(im, jk) * g(re, kk)) / aa;
                xaxpy(j, dmulk, dmulki, re, im, ik, ij);
                s(re, jk, dmulk);
                s(im, jk, dmulki);
                ij -= j - 1;
            }

            kpvt[(k - 1) as usize] = if swap { imax } else { k };
        } else {
            // 2x2 pivot block.
            let km1k = ik + k - 1;
            let ikm1 = ik - (k - 1);
            if swap {
                xswap(imax, re, im, im_val, ikm1);
                let mut imj = ikm1 + imax;
                let mut jj = imax;
                while jj <= km1 {
                    let j = km1 + imax - jj;
                    let jkm1 = ikm1 + j;
                    swap1(re, jkm1, imj);
                    swap1(im, jkm1, imj);
                    imj -= j - 1;
                    jj += 1;
                }
                swap1(re, km1k, imk);
                swap1(im, km1k, imk);
            }

            let km2 = k - 2;
            if km2 != 0 {
                let aa = g(re, km1k).powi(2) + g(im, km1k).powi(2);
                let ak = (g(re, kk) * g(re, km1k) + g(im, kk) * g(im, km1k)) / aa;
                let aki = (g(im, kk) * g(re, km1k) - g(re, kk) * g(im, km1k)) / aa;
                let km1km1 = ikm1 + k - 1;
                let akm1 = (g(re, km1km1) * g(re, km1k) + g(im, km1km1) * g(im, km1k)) / aa;
                let akm1i = (g(im, km1km1) * g(re, km1k) - g(re, km1km1) * g(im, km1k)) / aa;
                let denom = 1.0 - (ak * akm1 - aki * akm1i);
                let denomi = -(ak * akm1i + aki * akm1);
                let dd = denom * denom + denomi * denomi;
                let mut ij = ik - (k - 1) - (k - 2);
                for jj in 1..=km2 {
                    let j = km1 - jj;
                    let jk = ik + j;
                    let bk = (g(re, jk) * g(re, km1k) + g(im, jk) * g(im, km1k)) / aa;
                    let bki = (g(im, jk) * g(re, km1k) - g(re, jk) * g(im, km1k)) / aa;
                    let jkm1 = ikm1 + j;
                    let bkm1 = (g(re, jkm1) * g(re, km1k) + g(im, jkm1) * g(im, km1k)) / aa;
                    let bkm1i = (g(im, jkm1) * g(re, km1k) - g(re, jkm1) * g(im, km1k)) / aa;
                    let xx = akm1 * bk - akm1i * bki - bkm1;
                    let xxi = akm1 * bki + akm1i * bk - bkm1i;
                    let dmulk = (xx * denom + xxi * denomi) / dd;
                    let dmulki = (xxi * denom - xx * denomi) / dd;
                    let xx = ak * bkm1 - aki * bkm1i - bk;
                    let xxi = ak * bkm1i + aki * bkm1 - bki;
                    let dmlkm1 = (xx * denom + xxi * denomi) / dd;
                    let dmlkmi = (xxi * denom - xx * denomi) / dd;
                    xaxpy(j, dmulk, dmulki, re, im, ik, ij);
                    xaxpy(j, dmlkm1, dmlkmi, re, im, ikm1, ij);
                    s(re, jk, dmulk);
                    s(im, jk, dmulki);
                    s(re, jkm1, dmlkm1);
                    s(im, jkm1, dmlkmi);
                    ij -= j - 1;
                }
            }

            kpvt[(k - 1) as usize] = if swap { -imax } else { 1 - k };
            kpvt[(k - 2) as usize] = kpvt[(k - 1) as usize];
        }

        ik -= k - 1;
        if kstep == 2 {
            ik -= k - 2;
        }
        k -= kstep;
    }

    (kpvt, info)
}

/// Solve `A*x = b` using the factorization from [`xspfa`] — ported from
/// `xspsl` (`samm.f90:6038-6229`, LINPACK's `CSPSL`). `b` is overwritten
/// with the solution `x`.
///
/// A division by zero may occur if `a`'s factorization has a singular
/// pivot block (`info != 0` from [`xspfa`]) — matching upstream's own
/// documented behavior; this port does not add a guard upstream doesn't
/// have.
pub fn xspsl(a: &PackedComplexMatrix, n: i64, kpvt: &[i64], b_re: &mut [f64], b_im: &mut [f64]) {
    let re = &a.re;
    let im = &a.im;

    // Loop backward applying the transformations and D^-1 to b.
    let mut k = n;
    let mut ik = (n * (n - 1)) / 2;
    while k != 0 {
        let kk = ik + k;
        if kpvt[(k - 1) as usize] >= 0 {
            // 1x1 pivot block.
            if k != 1 {
                let kp = kpvt[(k - 1) as usize];
                if kp != k {
                    b_re.swap((k - 1) as usize, (kp - 1) as usize);
                    b_im.swap((k - 1) as usize, (kp - 1) as usize);
                }
                let bk_re = b_re[(k - 1) as usize];
                let bk_im = b_im[(k - 1) as usize];
                xaxpy_b(k - 1, bk_re, bk_im, re, im, ik, b_re, b_im);
            }
            let aa = g(re, kk).powi(2) + g(im, kk).powi(2);
            let ab = (b_re[(k - 1) as usize] * g(re, kk) + b_im[(k - 1) as usize] * g(im, kk)) / aa;
            let new_bi =
                (b_im[(k - 1) as usize] * g(re, kk) - b_re[(k - 1) as usize] * g(im, kk)) / aa;
            b_im[(k - 1) as usize] = new_bi;
            b_re[(k - 1) as usize] = ab;
            k -= 1;
            ik -= k;
        } else {
            // 2x2 pivot block.
            let ikm1 = ik - (k - 1);
            if k != 2 {
                let kp = kpvt[(k - 1) as usize].abs();
                if kp != k - 1 {
                    b_re.swap((k - 2) as usize, (kp - 1) as usize);
                    b_im.swap((k - 2) as usize, (kp - 1) as usize);
                }
                let bk_re = b_re[(k - 1) as usize];
                let bk_im = b_im[(k - 1) as usize];
                xaxpy_b(k - 2, bk_re, bk_im, re, im, ik, b_re, b_im);
                let bkm1_re = b_re[(k - 2) as usize];
                let bkm1_im = b_im[(k - 2) as usize];
                xaxpy_b(k - 2, bkm1_re, bkm1_im, re, im, ikm1, b_re, b_im);
            }
            let km1k = ik + k - 1;
            let kk = ik + k;
            let aa = g(re, km1k).powi(2) + g(im, km1k).powi(2);
            let ak = (g(re, kk) * g(re, km1k) + g(im, kk) * g(im, km1k)) / aa;
            let aki = (g(im, kk) * g(re, km1k) - g(re, kk) * g(im, km1k)) / aa;
            let km1km1 = ikm1 + k - 1;
            let akm1 = (g(re, km1km1) * g(re, km1k) + g(im, km1km1) * g(im, km1k)) / aa;
            let akm1i = (g(im, km1km1) * g(re, km1k) - g(re, km1km1) * g(im, km1k)) / aa;
            let bk =
                (b_re[(k - 1) as usize] * g(re, km1k) + b_im[(k - 1) as usize] * g(im, km1k)) / aa;
            let bki =
                (b_im[(k - 1) as usize] * g(re, km1k) - b_re[(k - 1) as usize] * g(im, km1k)) / aa;
            let bkm1 =
                (b_re[(k - 2) as usize] * g(re, km1k) + b_im[(k - 2) as usize] * g(im, km1k)) / aa;
            let bkm1i =
                (b_im[(k - 2) as usize] * g(re, km1k) - b_re[(k - 2) as usize] * g(im, km1k)) / aa;
            let denom = ak * akm1 - aki * akm1i - 1.0;
            let denomi = ak * akm1i + aki * akm1;
            let dd = denom * denom + denomi * denomi;
            let ab = akm1 * bk - akm1i * bki - bkm1;
            let abi = akm1i * bk + akm1 * bki - bkm1i;
            b_re[(k - 1) as usize] = (ab * denom + abi * denomi) / dd;
            b_im[(k - 1) as usize] = (abi * denom - ab * denomi) / dd;
            let ab = ak * bkm1 - aki * bkm1i - bk;
            let abi = aki * bkm1 + ak * bkm1i - bki;
            b_re[(k - 2) as usize] = (ab * denom + abi * denomi) / dd;
            b_im[(k - 2) as usize] = (abi * denom - ab * denomi) / dd;
            k -= 2;
            ik -= (k + 1) + k;
        }
    }

    // Loop forward applying the transformations.
    let mut k = 1_i64;
    let mut ik = 0_i64;
    while k <= n {
        if kpvt[(k - 1) as usize] >= 0 {
            // 1x1 pivot block.
            if k != 1 {
                let (dr, di) = xdot(k - 1, re, im, ik, 0);
                b_re[(k - 1) as usize] += dr;
                b_im[(k - 1) as usize] += di;
                let kp = kpvt[(k - 1) as usize];
                if kp != k {
                    b_re.swap((k - 1) as usize, (kp - 1) as usize);
                    b_im.swap((k - 1) as usize, (kp - 1) as usize);
                }
            }
            ik += k;
            k += 1;
        } else {
            // 2x2 pivot block.
            if k != 1 {
                let (dr, di) = xdot(k - 1, re, im, ik, 0);
                b_re[(k - 1) as usize] += dr;
                b_im[(k - 1) as usize] += di;
                let ikp1 = ik + k;
                let (dr2, di2) = xdot(k - 1, re, im, ikp1, 0);
                b_re[k as usize] += dr2;
                b_im[k as usize] += di2;
                let kp = kpvt[(k - 1) as usize].abs();
                if kp != k {
                    b_re.swap((k - 1) as usize, (kp - 1) as usize);
                    b_im.swap((k - 1) as usize, (kp - 1) as usize);
                }
            }
            ik += k + k + 1;
            k += 2;
        }
    }
}

/// `y[y0+1..=y0+n] += (sa+i*sai) * x[x0+1..=x0+n]` where `x` lives in `a`'s
/// packed storage (`re`/`im`) and `y` is the RHS vector `b` — a thin
/// `xaxpy` variant for [`xspsl`], whose "vector `y`" argument is always
/// `b(1,1)` (i.e. `y0=0`, unlike [`xspfa`]'s in-place `a`-to-`a` calls).
fn xaxpy_b(
    n: i64,
    sa: f64,
    sai: f64,
    x_re: &[f64],
    x_im: &[f64],
    x0: i64,
    y_re: &mut [f64],
    y_im: &mut [f64],
) {
    if n <= 0 || (sa == 0.0 && sai == 0.0) {
        return;
    }
    for i in 1..=n {
        let xr = g(x_re, x0 + i);
        let xi = g(x_im, x0 + i);
        let aa = y_re[(i - 1) as usize] + sa * xr - sai * xi;
        y_im[(i - 1) as usize] += sa * xi + sai * xr;
        y_re[(i - 1) as usize] = aa;
    }
}
