//! R-matrix (level-matrix `Y = I - R*L`) inversion, dispatched by channel
//! count — ported from `samm.f90`'s `yinvrs`, `onech`, `twoch`, `threech`
//! (with helpers `scale3`/`unscale3`), `yfour`, and `zeror`.
//!
//! For 1, 2, or 3 channels there is a closed-form matrix inverse
//! (`onech`/`twoch`/`threech` respectively) — cheaper and more accurate
//! than a general factorization for these small, extremely common cases.
//! For 4 or more channels, [`invert`] falls back to the general
//! [`super::linpack`] complex-symmetric solver (`yfour`'s own approach:
//! factor once via `xspfa`, then solve once per unit column via `xspsl` to
//! build the inverse column by column).
//!
//! Matrices here use the same packed complex-symmetric storage as
//! [`super::linpack::PackedComplexMatrix`] — position `k` (1-indexed)
//! holds `A(i,j)` for `i<=j`, `k = i + j*(j-1)/2` — and, as in that module,
//! indices in these function bodies are kept numerically identical to the
//! Fortran flat offsets rather than translated to 0-indexed, for the same
//! reason: direct line-by-line checkability against the source matters
//! more here than idiomatic style.

use crate::samm::linpack::{xspfa, xspsl, PackedComplexMatrix};

#[inline]
fn g(v: &[f64], k: i64) -> f64 {
    v[(k - 1) as usize]
}
#[inline]
fn s(v: &mut [f64], k: i64, x: f64) {
    v[(k - 1) as usize] = x;
}
/// `v[k] *= factor` / `v[k] /= factor` in place -- avoids the borrow
/// conflict of writing `s(v, k, g(v, k) * factor)` (mutable + immutable
/// borrow of the same buffer within one call's argument list).
#[inline]
fn scale_at(v: &mut [f64], k: i64, factor: f64) {
    let x = g(v, k) * factor;
    s(v, k, x);
}
#[inline]
fn unscale_at(v: &mut [f64], k: i64, factor: f64) {
    let x = g(v, k) / factor;
    s(v, k, x);
}

/// Set a triangular matrix's real/imag parts to all-zero — ported from
/// `zeror` (`samm.f90:4998-5024`). (Trivial; kept as a named function
/// purely to mirror the Fortran call sites this port will eventually
/// reach in Phase 5's `crosss`.)
pub fn zero_triangular(n: usize) -> PackedComplexMatrix {
    PackedComplexMatrix::zeros(n)
}

/// Invert the `n`-channel level matrix `Y` — ported from `yinvrs`
/// (`samm.f90:5026-5044`), dispatching to the closed-form 1/2/3-channel
/// inverters or the general [`super::linpack`] solver for `n>=4`.
pub fn invert(y: &PackedComplexMatrix, n: i64) -> PackedComplexMatrix {
    match n {
        1 => onech(y),
        2 => twoch(y),
        3 => threech(y),
        _ => yfour(y, n),
    }
}

/// Invert a 1x1 complex matrix — ported from `onech` (`samm.f90:5046-5074`).
pub fn onech(rmat: &PackedComplexMatrix) -> PackedComplexMatrix {
    let mut rinv = PackedComplexMatrix::zeros(1);
    let (re, im) = (&rmat.re, &rmat.im);
    let r1 = g(re, 1);
    let i1 = g(im, 1);
    if r1 + i1 == r1 {
        s(&mut rinv.re, 1, 1.0 / r1);
        s(&mut rinv.im, 1, -(i1 / r1) / r1);
    } else if r1 + i1 == i1 {
        s(&mut rinv.re, 1, (r1 / i1) / i1);
        s(&mut rinv.im, 1, -1.0 / i1);
    } else if r1 == 0.0 {
        s(&mut rinv.re, 1, 0.0);
        s(&mut rinv.im, 1, -1.0 / i1);
    } else {
        let aa = r1 * r1 + i1 * i1;
        s(&mut rinv.re, 1, r1 / aa);
        s(&mut rinv.im, 1, -i1 / aa);
    }
    rinv
}

/// Invert a 2x2 complex-symmetric matrix (packed positions `1=(1,1)`,
/// `2=(2,1)`, `3=(2,2)`) — ported from `twoch` (`samm.f90:5076-5181`).
/// Rescales by the largest-magnitude entry first (numerical conditioning
/// for very large/small matrix elements) and undoes the scaling on the
/// result, matching upstream exactly.
pub fn twoch(rmat_in: &PackedComplexMatrix) -> PackedComplexMatrix {
    const K11: i64 = 1;
    const K21: i64 = 2;
    const K22: i64 = 3;

    let mut rmat = PackedComplexMatrix {
        re: rmat_in.re.clone(),
        im: rmat_in.im.clone(),
    };
    let mut rinv = PackedComplexMatrix::zeros(2);

    if g(&rmat.re, K11) != 0.0 || g(&rmat.re, K21) != 0.0 || g(&rmat.re, K22) != 0.0 {
        let mut a = 0.0_f64;
        let mut k = 0_i64;
        for i in 1..=3 {
            if g(&rmat.re, i).abs() > a {
                k = i;
                a = g(&rmat.re, i).abs();
            }
            if g(&rmat.im, i).abs() > a {
                k = i;
                a = g(&rmat.im, i).abs();
            }
        }
        if a > 0.0 {
            if k == 2 {
                for i in 1..=3 {
                    unscale_at(&mut rmat.re, i, a);
                    unscale_at(&mut rmat.im, i, a);
                }
            } else {
                unscale_at(&mut rmat.re, k, a);
                unscale_at(&mut rmat.im, k, a);
                unscale_at(&mut rmat.re, 2, a.sqrt());
                unscale_at(&mut rmat.im, 2, a.sqrt());
            }
        }

        let bbr = g(&rmat.re, K11) * g(&rmat.re, K22)
            - g(&rmat.im, K11) * g(&rmat.im, K22)
            - g(&rmat.re, K21).powi(2)
            + g(&rmat.im, K21).powi(2);
        let bbi = g(&rmat.re, K11) * g(&rmat.im, K22) + g(&rmat.im, K11) * g(&rmat.re, K22)
            - 2.0 * g(&rmat.re, K21) * g(&rmat.im, K21);

        let (aar, aai);
        if bbr + bbi != bbi {
            if bbr + bbi != bbr {
                let aa = 1.0 / (bbr * bbr + bbi * bbi);
                aar = bbr * aa;
                aai = -bbi * aa;
            } else {
                aar = 1.0 / bbr;
                aai = -(bbi / bbr) / bbr;
            }
        } else {
            aar = (bbr / bbi) / bbi;
            aai = -1.0 / bbi;
        }

        s(
            &mut rinv.re,
            K11,
            aar * g(&rmat.re, K22) - aai * g(&rmat.im, K22),
        );
        s(
            &mut rinv.im,
            K11,
            aar * g(&rmat.im, K22) + aai * g(&rmat.re, K22),
        );
        s(
            &mut rinv.re,
            K21,
            -aar * g(&rmat.re, K21) + aai * g(&rmat.im, K21),
        );
        s(
            &mut rinv.im,
            K21,
            -aar * g(&rmat.im, K21) - aai * g(&rmat.re, K21),
        );
        s(
            &mut rinv.re,
            K22,
            aar * g(&rmat.re, K11) - aai * g(&rmat.im, K11),
        );
        s(
            &mut rinv.im,
            K22,
            aar * g(&rmat.im, K11) + aai * g(&rmat.re, K11),
        );

        if a != 0.0 {
            if k == 2 {
                for i in 1..=3 {
                    unscale_at(&mut rinv.re, i, a);
                    unscale_at(&mut rinv.im, i, a);
                }
            } else {
                unscale_at(&mut rinv.re, k, a);
                unscale_at(&mut rinv.im, k, a);
                unscale_at(&mut rinv.re, 2, a.sqrt());
                unscale_at(&mut rinv.im, 2, a.sqrt());
            }
        }
    } else if g(&rmat.im, K21) != 0.0 {
        // Real part is zero everywhere; imaginary part is dense.
        let a = g(&rmat.im, K11) * g(&rmat.im, K22) - g(&rmat.im, K21).powi(2);
        s(&mut rinv.im, K11, -g(&rmat.im, K22) / a);
        s(&mut rinv.im, K21, g(&rmat.im, K21) / a);
        s(&mut rinv.im, K22, -g(&rmat.im, K11) / a);
    } else {
        // Real part is zero everywhere; imaginary part is zero off-diagonal.
        s(&mut rinv.im, K11, -1.0 / g(&rmat.im, K11));
        s(&mut rinv.im, K22, -1.0 / g(&rmat.im, K22));
    }

    rinv
}

/// Scale a 3x3 packed complex-symmetric matrix for numerical conditioning
/// before inversion, tracking per-block scale factors — ported from
/// `scale3` (`samm.f90:5327-5384`).
fn scale3(rmat: &mut PackedComplexMatrix) -> (f64, f64, f64) {
    const AA: f64 = 1.0e10;
    let mut a1 = 0.0_f64;
    let mut a2 = 0.0_f64;
    let mut a3 = 0.0_f64;
    let (re, im) = (&mut rmat.re, &mut rmat.im);

    if g(re, 1).abs() >= AA || g(im, 1).abs() >= AA {
        let bb = g(re, 1).abs().max(g(im, 1).abs());
        a1 = bb.sqrt();
        unscale_at(re, 1, bb);
        unscale_at(im, 1, bb);
        unscale_at(re, 2, a1);
        unscale_at(im, 2, a1);
        unscale_at(re, 4, a1);
        unscale_at(im, 4, a1);
    }
    if g(re, 3).abs() >= AA || g(im, 3).abs() >= AA {
        let bb = g(re, 3).abs().max(g(im, 3).abs());
        a2 = bb.sqrt();
        unscale_at(re, 2, a2);
        unscale_at(im, 2, a2);
        unscale_at(re, 3, bb);
        unscale_at(im, 3, bb);
        unscale_at(re, 5, a2);
        unscale_at(im, 5, a2);
    }
    if g(re, 6).abs() >= AA || g(im, 6).abs() >= AA {
        let bb = g(re, 6).abs().max(g(im, 6).abs());
        a3 = bb.sqrt();
        unscale_at(re, 4, a3);
        unscale_at(im, 4, a3);
        unscale_at(re, 5, a3);
        unscale_at(im, 5, a3);
        unscale_at(re, 6, bb);
        unscale_at(im, 6, bb);
    }
    (a1, a2, a3)
}

/// Undo [`scale3`]'s scaling on both `rmat` and its inverse `rinv` — ported
/// from `unscale3` (`samm.f90:5386-5447`).
fn unscale3(
    a1: f64,
    a2: f64,
    a3: f64,
    rmat: &mut PackedComplexMatrix,
    rinv: &mut PackedComplexMatrix,
) {
    if a1 > 0.0 {
        let bb = a1 * a1;
        scale_at(&mut rmat.re, 1, bb);
        scale_at(&mut rmat.im, 1, bb);
        scale_at(&mut rmat.re, 2, a1);
        scale_at(&mut rmat.im, 2, a1);
        scale_at(&mut rmat.re, 4, a1);
        scale_at(&mut rmat.im, 4, a1);
        unscale_at(&mut rinv.re, 1, bb);
        unscale_at(&mut rinv.im, 1, bb);
        unscale_at(&mut rinv.re, 2, a1);
        unscale_at(&mut rinv.im, 2, a1);
        unscale_at(&mut rinv.re, 4, a1);
        unscale_at(&mut rinv.im, 4, a1);
    }
    if a2 > 0.0 {
        let bb = a2 * a2;
        scale_at(&mut rmat.re, 2, a2);
        scale_at(&mut rmat.im, 2, a2);
        scale_at(&mut rmat.re, 3, bb);
        scale_at(&mut rmat.im, 3, bb);
        scale_at(&mut rmat.re, 5, a2);
        scale_at(&mut rmat.im, 5, a2);
        unscale_at(&mut rinv.re, 2, a2);
        unscale_at(&mut rinv.im, 2, a2);
        unscale_at(&mut rinv.re, 3, bb);
        unscale_at(&mut rinv.im, 3, bb);
        unscale_at(&mut rinv.re, 5, a2);
        unscale_at(&mut rinv.im, 5, a2);
    }
    if a3 > 0.0 {
        let bb = a3 * a3;
        scale_at(&mut rmat.re, 4, a3);
        scale_at(&mut rmat.im, 4, a3);
        scale_at(&mut rmat.re, 5, a3);
        scale_at(&mut rmat.im, 5, a3);
        scale_at(&mut rmat.re, 6, bb);
        scale_at(&mut rmat.im, 6, bb);
        unscale_at(&mut rinv.re, 4, a3);
        unscale_at(&mut rinv.im, 4, a3);
        unscale_at(&mut rinv.re, 5, a3);
        unscale_at(&mut rinv.im, 5, a3);
        unscale_at(&mut rinv.re, 6, bb);
        unscale_at(&mut rinv.im, 6, bb);
    }
}

/// Invert a 3x3 complex-symmetric matrix (packed positions `1=(1,1)`,
/// `2=(2,1)`, `3=(2,2)`, `4=(3,1)`, `5=(3,2)`, `6=(3,3)`) via the adjugate/
/// determinant formula — ported from `threech` (`samm.f90:5183-5325`).
pub fn threech(rmat_in: &PackedComplexMatrix) -> PackedComplexMatrix {
    const K11: i64 = 1;
    const K21: i64 = 2;
    const K31: i64 = 4;
    const K22: i64 = 3;
    const K32: i64 = 5;
    const K33: i64 = 6;

    let mut rmat = PackedComplexMatrix {
        re: rmat_in.re.clone(),
        im: rmat_in.im.clone(),
    };
    let (a1, a2, a3) = scale3(&mut rmat);
    let mut rinv = PackedComplexMatrix::zeros(3);

    let mut izz = false;
    for i in 1..=6 {
        if g(&rmat.re, i) != 0.0 {
            izz = true;
        }
    }
    if g(&rmat.im, 2) != 0.0 || g(&rmat.im, 4) != 0.0 || g(&rmat.im, 5) != 0.0 {
        izz = true;
    }

    if !izz {
        // Only imaginary parts of diagonal terms are non-zero.
        s(&mut rinv.im, 1, -1.0 / g(&rmat.im, 1));
        s(&mut rinv.im, 3, -1.0 / g(&rmat.im, 3));
        s(&mut rinv.im, 6, -1.0 / g(&rmat.im, 6));
    } else {
        let (rr, ri) = (&rmat.re, &rmat.im);
        let fc1r = g(rr, K22) * g(rr, K33) - g(ri, K22) * g(ri, K33) - g(rr, K32).powi(2)
            + g(ri, K32).powi(2);
        let fc1i =
            g(ri, K22) * g(rr, K33) + g(rr, K22) * g(ri, K33) - 2.0 * g(rr, K32) * g(ri, K32);
        let fc2r = g(rr, K21) * g(rr, K33) - g(ri, K21) * g(ri, K33) - g(rr, K31) * g(rr, K32)
            + g(ri, K31) * g(ri, K32);
        let fc2i = g(ri, K21) * g(rr, K33) + g(rr, K21) * g(ri, K33)
            - g(rr, K31) * g(ri, K32)
            - g(ri, K31) * g(rr, K32);
        let fc3r = g(rr, K21) * g(rr, K32) - g(ri, K21) * g(ri, K32) - g(rr, K31) * g(rr, K22)
            + g(ri, K31) * g(ri, K22);
        let fc3i = g(ri, K21) * g(rr, K32) + g(rr, K21) * g(ri, K32)
            - g(ri, K31) * g(rr, K22)
            - g(rr, K31) * g(ri, K22);

        let aar = g(rr, K11) * fc1r - g(rr, K21) * fc2r + g(rr, K31) * fc3r
            - (g(ri, K11) * fc1i - g(ri, K21) * fc2i + g(ri, K31) * fc3i);
        let aai = g(rr, K11) * fc1i - g(rr, K21) * fc2i
            + g(rr, K31) * fc3i
            + (g(ri, K11) * fc1r - g(ri, K21) * fc2r + g(ri, K31) * fc3r);

        let (dcr, dci);
        if aar + aai != aai {
            if aar + aai != aar {
                let aa = aar * aar + aai * aai;
                dcr = aar / aa;
                dci = -aai / aa;
            } else {
                dcr = 1.0 / aar;
                dci = -(aai / aar) / aar;
            }
        } else {
            dcr = (aar / aai) / aai;
            dci = -1.0 / aai;
        }

        s(&mut rinv.re, K11, fc1r * dcr - fc1i * dci);
        s(&mut rinv.im, K11, fc1r * dci + fc1i * dcr);
        s(&mut rinv.re, K21, -fc2r * dcr + fc2i * dci);
        s(&mut rinv.im, K21, -fc2r * dci - fc2i * dcr);
        s(
            &mut rinv.re,
            K22,
            (g(rr, K11) * g(rr, K33) - g(rr, K31).powi(2) - g(ri, K11) * g(ri, K33)
                + g(ri, K31).powi(2))
                * dcr
                - (g(ri, K11) * g(rr, K33) - 2.0 * g(rr, K31) * g(ri, K31)
                    + g(rr, K11) * g(ri, K33))
                    * dci,
        );
        s(
            &mut rinv.im,
            K22,
            (g(rr, K11) * g(rr, K33) - g(rr, K31).powi(2) - g(ri, K11) * g(ri, K33)
                + g(ri, K31).powi(2))
                * dci
                + (g(ri, K11) * g(rr, K33) - 2.0 * g(rr, K31) * g(ri, K31)
                    + g(rr, K11) * g(ri, K33))
                    * dcr,
        );
        s(&mut rinv.re, K31, fc3r * dcr - fc3i * dci);
        s(&mut rinv.im, K31, fc3r * dci + fc3i * dcr);
        s(
            &mut rinv.re,
            K32,
            -(g(rr, K11) * g(rr, K32) - g(rr, K21) * g(rr, K31) - g(ri, K11) * g(ri, K32)
                + g(ri, K21) * g(ri, K31))
                * dcr
                + (g(rr, K11) * g(ri, K32) - g(rr, K21) * g(ri, K31) + g(ri, K11) * g(rr, K32)
                    - g(ri, K21) * g(rr, K31))
                    * dci,
        );
        s(
            &mut rinv.im,
            K32,
            -(g(rr, K11) * g(rr, K32) - g(rr, K21) * g(rr, K31) - g(ri, K11) * g(ri, K32)
                + g(ri, K21) * g(ri, K31))
                * dci
                - (g(rr, K11) * g(ri, K32) - g(rr, K21) * g(ri, K31) + g(ri, K11) * g(rr, K32)
                    - g(ri, K21) * g(rr, K31))
                    * dcr,
        );
        s(
            &mut rinv.re,
            K33,
            (g(rr, K11) * g(rr, K22) - g(rr, K21).powi(2) - g(ri, K11) * g(ri, K22)
                + g(ri, K21).powi(2))
                * dcr
                - (g(ri, K11) * g(rr, K22) - 2.0 * g(rr, K21) * g(ri, K21)
                    + g(rr, K11) * g(ri, K22))
                    * dci,
        );
        s(
            &mut rinv.im,
            K33,
            (g(rr, K11) * g(rr, K22) - g(rr, K21).powi(2) - g(ri, K11) * g(ri, K22)
                + g(ri, K21).powi(2))
                * dci
                + (g(ri, K11) * g(rr, K22) - 2.0 * g(rr, K21) * g(ri, K21)
                    + g(rr, K11) * g(ri, K22))
                    * dcr,
        );

        unscale3(a1, a2, a3, &mut rmat, &mut rinv);
    }

    rinv
}

/// Invert a `n>=4`-channel level matrix via the general LINPACK-style
/// solver — ported from `yfour` (`samm.f90:5449-5490`): factor once via
/// [`xspfa`], then solve for each unit column via [`xspsl`] to build up
/// the inverse column by column.
pub fn yfour(rmat: &PackedComplexMatrix, n: i64) -> PackedComplexMatrix {
    let mut a = PackedComplexMatrix {
        re: rmat.re.clone(),
        im: rmat.im.clone(),
    };
    let (kpvt, info) = xspfa(&mut a, n);
    if info != 0 {
        log::error!("samm yfour: xspfa reported a singular pivot block (info={info})");
    }

    let mut rinv = PackedComplexMatrix::zeros(n as usize);
    for k in 1..=n {
        let mut b_re = vec![0.0_f64; n as usize];
        let mut b_im = vec![0.0_f64; n as usize];
        b_re[(k - 1) as usize] = 1.0;
        xspsl(&a, n, &kpvt, &mut b_re, &mut b_im);
        for j in 1..=k {
            let kj = (j + k * (k - 1) / 2) as usize - 1;
            rinv.re[kj] = b_re[(j - 1) as usize];
            rinv.im[kj] = b_im[(j - 1) as usize];
        }
    }
    rinv
}

#[cfg(test)]
mod tests {
    //! `Y·Y^-1 = I` for every inverter, the `n >= 4` path above all
    //! (GitHub #339).
    //!
    //! **Methodology.** Build complex-symmetric matrices in this module's
    //! packed storage, invert with [`invert`] (`yfour` for `n >= 4`, the
    //! closed forms below that), and form the dense product `Y·Y^-1`. The
    //! oracle is the identity, which needs no reference code. Pass: every
    //! element within `1e-12` of the identity. The cases reach each branch of
    //! `xspfa`'s Bunch-Kaufman pivoting: diagonally dominant (1x1 pivots, no
    //! swap), a small diagonal with a large far coupling (a swap), and a zero
    //! trailing 2x2 diagonal (a 2x2 pivot).
    //!
    //! **Results, 2026-10-01.** Every case passes at round-off. Re-run
    //! against the pre-fix `xdot` in `linpack::xspsl`, all three tests fail
    //! at O(1): `|Y·Y^-1 - I|` = 1.49 on the coupled `n = 4` case, 40.5 on
    //! the Fe-57 block, and 0.71 on the `n = 4` 2x2-pivot case.
    use super::*;

    /// 0-indexed packed position of `(i, j)`.
    fn pk(i: usize, j: usize) -> usize {
        if i <= j {
            i + j * (j + 1) / 2
        } else {
            j + i * (i + 1) / 2
        }
    }

    fn from_fn(n: usize, f: impl Fn(usize, usize) -> (f64, f64)) -> PackedComplexMatrix {
        let mut m = PackedComplexMatrix::zeros(n);
        for j in 0..n {
            for i in 0..=j {
                let (re, im) = f(i, j);
                m.re[pk(i, j)] = re;
                m.im[pk(i, j)] = im;
            }
        }
        m
    }

    fn max_dev_from_identity(y: &PackedComplexMatrix, yi: &PackedComplexMatrix, n: usize) -> f64 {
        let mut worst = 0.0_f64;
        for i in 0..n {
            for j in 0..n {
                let (mut re, mut im) = (0.0, 0.0);
                for k in 0..n {
                    let (ar, ai) = (y.re[pk(i, k)], y.im[pk(i, k)]);
                    let (br, bi) = (yi.re[pk(k, j)], yi.im[pk(k, j)]);
                    re += ar * br - ai * bi;
                    im += ar * bi + ai * br;
                }
                let t = if i == j { 1.0 } else { 0.0 };
                worst = worst.max((re - t).abs()).max(im.abs());
            }
        }
        worst
    }

    #[test]
    fn every_inverter_inverts_coupled_matrices() {
        for n in 1..=6 {
            let y = from_fn(n, |i, j| {
                if i == j {
                    (3.0 + i as f64, -1.0 - 0.5 * i as f64)
                } else {
                    (0.3 / (1.0 + (i + j) as f64), 0.1 * (i as f64 - j as f64))
                }
            });
            let yi = invert(&y, n as i64);
            let d = max_dev_from_identity(&y, &yi, n);
            assert!(d < 1e-12, "n={n}: |Y Y^-1 - I| = {d:e}");
        }
    }

    /// Fe-57's J=1⁻ level matrix at 110.3688 keV (ENDF/B-VIII.0), printed to
    /// 5 figures: two coupled channels among four, the case that exposed the
    /// defect.
    #[test]
    fn yfour_inverts_the_fe57_block_structure() {
        let y = from_fn(4, |i, j| match (i, j) {
            (0, 0) => (1.0440e1, -2.2409e0),
            (0, 2) => (9.7609e0, -2.8765e-2),
            (2, 2) => (1.0584e1, -2.4098e0),
            (1, 1) | (3, 3) => (0.0, -1.0),
            _ => (0.0, 0.0),
        });
        let yi = invert(&y, 4);
        let d = max_dev_from_identity(&y, &yi, 4);
        assert!(d < 1e-12, "|Y Y^-1 - I| = {d:e}");
    }

    /// `xspfa`'s swap and 2x2-pivot branches.
    #[test]
    fn yfour_inverts_matrices_needing_swaps_and_2x2_pivots() {
        for n in 4..=6 {
            // Zero trailing 2x2 diagonal with a large coupling: a 2x2 pivot.
            let two_by_two = from_fn(n, |i, j| {
                if i == j {
                    if j + 2 >= n {
                        (0.0, 0.0)
                    } else {
                        (2.0 + i as f64, 0.5)
                    }
                } else if i + 2 == n && j + 1 == n {
                    (5.0, -1.0)
                } else {
                    (0.2, 0.05 * (i + j) as f64)
                }
            });
            // Small last diagonal, largest coupling on row 0: a swap.
            let swap = from_fn(n, |i, j| {
                if i == j {
                    if j + 1 == n {
                        (1e-3, 0.0)
                    } else {
                        (4.0 + i as f64, -0.3)
                    }
                } else if i == 0 && j + 1 == n {
                    (2.0, 0.7)
                } else {
                    (0.1, -0.02 * (i + j) as f64)
                }
            });
            for (name, y) in [("2x2", two_by_two), ("swap", swap)] {
                let mut f = PackedComplexMatrix {
                    re: y.re.clone(),
                    im: y.im.clone(),
                };
                let (kpvt, _) = xspfa(&mut f, n as i64);
                let yi = invert(&y, n as i64);
                let d = max_dev_from_identity(&y, &yi, n);
                assert!(d < 1e-12, "{name} n={n} kpvt={kpvt:?}: |Y Y^-1 - I| = {d:e}");
                // The case must reach the branch it is named for.
                let k = kpvt[n - 1];
                match name {
                    "2x2" => assert!(k < 0, "{name} n={n}: kpvt={kpvt:?} has no 2x2 pivot"),
                    _ => assert!(k != n as i64, "{name} n={n}: kpvt={kpvt:?} has no swap"),
                }
            }
        }
    }
}
