// Ported from NJOY2016 `src/errorr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Self-contained numerical kernels from NJOY2016's ERRORR module.
//!
//! These are the small, side-effect-free helper routines that ERRORR uses while
//! reconstructing resonance cross sections and propagating their covariances:
//!
//! - Hard-sphere **phase shift** `phi_l` and **shift/penetration factors**
//!   `S_l`, `P_l` for orbital angular momentum `l = 0..5`
//!   ([`efacphi`], [`efacts`], [`eunfac`]).
//! - The **unresolved-resonance width-fluctuation (statistical) integral**
//!   evaluated with the ten-point Gauss-Laguerre-like quadrature tables NJOY
//!   ships ([`egnrl`]).
//! - Small **real/complex 3x3 matrix** helpers used by the resonance-matrix
//!   machinery ([`efrobns`], [`ethrinv`], [`eabcmat`]).
//! - A specialised **Clebsch-Gordan coefficient** `<i1 0, i2 0 | i3 0>` built
//!   from a hard-coded factorial mantissa/exponent table ([`cleb`]).
//!
//! **What belongs here:** faithful, line-traceable translations of the
//! self-contained numeric subroutines listed above. **What does not:** the
//! ERRORR driver loop, ENDF I/O, covariance assembly, or the `matrixin` /
//! `matrixej` transform drivers (owned elsewhere).
//!
//! # Faithfulness notes
//!
//! Fortran returns results through out-parameters; here each routine returns
//! them as a value or tuple instead (documented per function). Two upstream
//! quirks are reproduced deliberately, because a faithful port must:
//!
//! 1. **`efacts` / `eunfac` only cover the `l` values upstream handles.**
//!    `efacts` handles `l = 0..4`, `eunfac` handles `l = 0..2`. For higher `l`
//!    the Fortran leaves its out-parameters at whatever the caller passed in;
//!    this port returns zeros there and says so on each function.
//! 2. **`cleb` uses Fortran *integer* exponentiation `10**nept`.** In Fortran
//!    (and gfortran/ifort) `10**n` with `n < 0` is integer `0`, so `cleb`
//!    returns `0` whenever the factorial-exponent bookkeeping `nept` is
//!    negative — even for angular-momentum triples whose true Clebsch-Gordan
//!    coefficient is non-zero. This is reproduced exactly via [`ipow10`]; do
//!    not "fix" it, or the port stops matching NJOY bit-for-bit.

// These lints are relaxed deliberately: this module is a line-traceable port of
// Fortran, so it keeps explicit index loops (mirroring `do i=1,n`), the original
// multi-argument signature of `egnrl`, and small nested-array tuples in returns.
#![allow(clippy::too_many_arguments)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::assign_op_pattern)]
#![allow(clippy::type_complexity)]

use crate::NjoyError;

/// Fortran default-integer power `10**n`, reproduced exactly.
///
/// Physical/numeric role: a base-10 scale factor applied to a factorial
/// *mantissa* inside [`cleb`]. Fortran evaluates `10**n` in *integer*
/// arithmetic, so this is **not** `10f64.powi(n)`:
///
/// - `n < 0` returns `0.0` (Fortran integer `10**(-k) == 0` for `k > 0`).
/// - `0 <= n <= 9` returns the exact power.
/// - `n >= 10` overflows 32-bit integer and *wraps* (two's complement),
///   matching Fortran default `integer(4)`. In practice `nept` stays small,
///   so this branch is not exercised by realistic angular-momentum inputs.
///
/// Returns the scale factor as `f64`.
fn ipow10(n: i32) -> f64 {
    if n < 0 {
        0.0
    } else {
        // wrapping_pow reproduces default integer(4) overflow behaviour.
        10i32.wrapping_pow(n as u32) as f64
    }
}

/// Hard-sphere phase shift `phi_l` (dimensionless, radians).
///
/// errorr.f90:10335-10365
///
/// Computes the potential-scattering phase shift for orbital angular momentum
/// `l` at channel radius parameter `rho = k * a` (wavenumber times radius,
/// dimensionless). Valid for `l = 0..5`; `l >= 4` uses the general `l = 4`
/// closed form (as upstream does via its `else` branch). `rho` should be
/// positive (a physical `k*a`); at `l >= 2` a `phi/rho < 1e-6` result is
/// clamped to exactly `0`, matching NJOY.
///
/// Returns `phi` in radians.
pub fn efacphi(l: i32, rho: f64) -> f64 {
    const TEST: f64 = 1.0e-6;
    let r2 = rho * rho;
    if l == 0 {
        rho
    } else if l == 1 {
        rho - rho.atan()
    } else if l == 2 {
        let phi = rho - (3.0 * rho / (3.0 - r2)).atan();
        if phi / rho < TEST {
            0.0
        } else {
            phi
        }
    } else if l == 3 {
        let phi = rho - ((15.0 * rho - rho * r2) / (15.0 - 6.0 * r2)).atan();
        if phi / rho < TEST {
            0.0
        } else {
            phi
        }
    } else {
        let r4 = r2 * r2;
        let top = 105.0 * rho - 10.0 * r2 * rho;
        let bot = 105.0 - 45.0 * r2 + r4;
        let phi = rho - (top / bot).atan();
        if phi / rho < TEST {
            0.0
        } else {
            phi
        }
    }
}

/// Hard-sphere shift factor `S_l` and penetration factor `P_l` (both dimensionless).
///
/// errorr.f90:10367-10405
///
/// For orbital angular momentum `l` and channel radius parameter `rho = k * a`
/// (dimensionless), returns the R-matrix shift factor `se` (= `S_l`) and
/// penetration factor `pe` (= `P_l`). Upstream handles `l = 0..4` only; for any
/// other `l` the Fortran leaves the out-parameters untouched, so this port
/// returns `(0.0, 0.0)` there (documented divergence — no fabricated value).
///
/// Returns the tuple `(se, pe)` mapping the Fortran out-parameters
/// `(se, pe)` in that order.
pub fn efacts(l: i32, rho: f64) -> (f64, f64) {
    let r2 = rho * rho;
    if l == 0 {
        (0.0, rho)
    } else if l == 1 {
        let den = 1.0 + r2;
        let pe = r2 * rho / den;
        let se = -1.0 / den;
        (se, pe)
    } else if l == 2 {
        let r4 = r2 * r2;
        let den = 3.0 * r2 + r4 + 9.0;
        let pe = r4 * rho / den;
        let se = -(18.0 + 3.0 * r2) / den;
        (se, pe)
    } else if l == 3 {
        let r4 = r2 * r2;
        let r6 = r4 * r2;
        let den = 225.0 + 45.0 * r2 + 6.0 * r4 + r6;
        let pe = r6 * rho / den;
        let se = -(675.0 + 90.0 * r2 + 6.0 * r4) / den;
        (se, pe)
    } else if l == 4 {
        let r4 = r2 * r2;
        let r6 = r4 * r2;
        let r8 = r4 * r4;
        let den = 11025.0 + 1575.0 * r2 + 135.0 * r4 + 10.0 * r6 + r8;
        let pe = r8 * rho / den;
        let se = -(44100.0 + 4725.0 * r2 + 270.0 * r4 + 10.0 * r6) / den;
        (se, pe)
    } else {
        // Upstream leaves se, pe unset for l > 4; no fabricated value.
        (0.0, 0.0)
    }
}

/// Unresolved-region penetrability `vl` and phase shift `ps` (both dimensionless).
///
/// errorr.f90:10407-10430
///
/// For orbital angular momentum `l`, channel radius parameters `rho = k * a`
/// and `rhoc = k * a_c` (both dimensionless), and the degrees-of-freedom-style
/// prefactor `amun` (dimensionless), returns the penetrability factor `vl` and
/// the hard-sphere phase `ps` used in the unresolved-resonance region. Upstream
/// handles `l = 0..2` only; for higher `l` the Fortran leaves the
/// out-parameters untouched, so this port returns `(0.0, 0.0)` there
/// (documented divergence — no fabricated value).
///
/// Returns the tuple `(vl, ps)` mapping the Fortran out-parameters
/// `(vl, ps)` in that order.
pub fn eunfac(l: i32, rho: f64, rhoc: f64, amun: f64) -> (f64, f64) {
    let r2 = rho * rho;
    if l == 0 {
        (amun, rhoc)
    } else if l == 1 {
        let vl = amun * r2 / (1.0 + r2);
        let ps = rhoc - rhoc.atan();
        (vl, ps)
    } else if l == 2 {
        let r4 = r2 * r2;
        let vl = amun * r4 / (9.0 + 3.0 * r2 + r4);
        let ps = rhoc - (3.0 * rhoc / (3.0 - rhoc * rhoc)).atan();
        (vl, ps)
    } else {
        // Upstream leaves vl, ps unset for l > 2; no fabricated value.
        (0.0, 0.0)
    }
}

// --- egnrl quadrature tables (errorr.f90:10442-10471) ---------------------
//
// Fortran declares `qw(10,4)` and `qp(10,4)` column-major. Here they are stored
// as `[column][row]` = `[mu-1][j-1]`, so the Fortran access `qw(j,mu)` maps to
// `QW[mu - 1][j - 1]`. The four columns index the quadrature-order selector
// (`mu`, `nu`, `lamda` in Fortran, values 1..=4).

/// Gauss quadrature *weights* `qw(j, col)` for the width-fluctuation integral,
/// stored as `QW[col-1][j-1]` (dimensionless). errorr.f90:10442-10456.
const QW: [[f64; 10]; 4] = [
    [
        1.1120413e-1,
        2.3546798e-1,
        2.8440987e-1,
        2.2419127e-1,
        0.10967668,
        0.030493789,
        0.0042930874,
        2.5827047e-4,
        4.9031965e-6,
        1.4079206e-8,
    ],
    [
        0.033773418,
        0.079932171,
        0.12835937,
        0.17652616,
        0.21347043,
        0.21154965,
        0.13365186,
        0.022630659,
        1.6313638e-5,
        2.745383e-31,
    ],
    [
        3.3376214e-4,
        0.018506108,
        0.12309946,
        0.29918923,
        0.33431475,
        0.17766657,
        0.042695894,
        4.0760575e-3,
        1.1766115e-4,
        5.0989546e-7,
    ],
    [
        1.7623788e-3,
        0.021517749,
        0.080979849,
        0.18797998,
        0.30156335,
        0.29616091,
        0.10775649,
        2.5171914e-3,
        8.9630388e-10,
        0.0,
    ],
];

/// Gauss quadrature *abscissae* `qp(j, col)` for the width-fluctuation integral,
/// stored as `QP[col-1][j-1]` (dimensionless). errorr.f90:10457-10471.
const QP: [[f64; 10]; 4] = [
    [
        3.0013465e-3,
        7.8592886e-2,
        4.3282415e-1,
        1.3345267,
        3.0481846,
        5.8263198,
        9.9452656,
        1.5782128e1,
        23.996824,
        36.216208,
    ],
    [
        1.3219203e-2,
        7.2349624e-2,
        0.19089473,
        0.39528842,
        0.74083443,
        1.3498293,
        2.5297983,
        5.2384894,
        13.821772,
        75.647525,
    ],
    [
        1.0004488e-3,
        0.026197629,
        0.14427472,
        0.44484223,
        1.0160615,
        1.9421066,
        3.3150885,
        5.2607092,
        7.9989414,
        12.072069,
    ],
    [
        0.013219203,
        0.072349624,
        0.19089473,
        0.39528842,
        0.74083443,
        1.3498293,
        2.5297983,
        5.2384894,
        13.821772,
        75.647525,
    ],
];

/// Unresolved-resonance width-fluctuation (statistical) integral `S` (dimensionless).
///
/// errorr.f90:10432-10578
///
/// Evaluates the Dresner/Hwang width-fluctuation integral that averages a
/// resonance cross section over the chi-squared distributions of the neutron,
/// fission, and competitive widths, using NJOY's ten-point quadrature
/// ([`QW`]/[`QP`]). Parameters (all reduced widths, same energy units as each
/// other, e.g. eV):
///
/// - `galpha` = neutron width `Gamma_n` (must be `> 0`, else `S = 0`).
/// - `gbeta`  = fission width `Gamma_f` (`>= 0`).
/// - `gamma`  = radiation (capture) width `Gamma_gamma` (must be `> 0`).
/// - `df`     = competitive-width parameter (`>= 0` when `gbeta > 0`).
/// - `mu, nu, lamda` = quadrature-order selectors for the neutron, fission, and
///   competitive channels respectively (Fortran column indices, valid `1..=4`).
/// - `id` = which moment to accumulate: `1` = capture-like (`xj^2` numerator),
///   `2` = elastic-like (`xj` numerator), `3` = fission-like (`xj * yk`
///   numerator). `id` values outside the set handled for a given branch yield
///   `S = 0`, matching upstream.
///
/// Preconditions: `mu, nu, lamda` in `1..=4` and `id` in `1..=3`; out-of-range
/// selectors would index past the quadrature table (documented precondition,
/// mirroring the Fortran array bounds).
///
/// Returns the accumulated integral `S` (Fortran out-parameter `s`).
pub fn egnrl(
    galpha: f64,
    gbeta: f64,
    gamma: f64,
    mu: usize,
    nu: usize,
    lamda: usize,
    df: f64,
    id: i32,
) -> f64 {
    let mut s = 0.0;
    if galpha <= 0.0 {
        return s;
    }
    if gamma <= 0.0 {
        return s;
    }
    if gbeta < 0.0 {
        return s;
    }
    if gbeta > 0.0 && df < 0.0 {
        return s;
    }

    let m = mu - 1; // 0-based neutron-channel column

    // Case 1: gbeta == 0, df == 0  -> single 10-point sum over the neutron grid.
    if gbeta == 0.0 && df == 0.0 {
        if id == 1 {
            for j in 0..10 {
                s += QW[m][j] * QP[m][j] * QP[m][j] / (galpha * QP[m][j] + gamma);
            }
        } else if id == 2 {
            for j in 0..10 {
                s += QW[m][j] * QP[m][j] / (galpha * QP[m][j] + gamma);
            }
        }
        return s;
    }

    // Case 2: gbeta == 0, df > 0  -> double sum over neutron and competitive grids.
    if gbeta == 0.0 && df > 0.0 {
        let l = lamda - 1;
        if id == 1 {
            for j in 0..10 {
                let xj = QP[m][j];
                for k in 0..10 {
                    s += QW[m][j] * QW[l][k] * xj * xj / (galpha * xj + gamma + df * QP[l][k]);
                }
            }
        } else if id == 2 {
            for j in 0..10 {
                let xj = QP[m][j];
                for k in 0..10 {
                    s += QW[m][j] * QW[l][k] * xj / (galpha * xj + gamma + df * QP[l][k]);
                }
            }
        }
        return s;
    }

    // Case 3: gbeta > 0, df == 0  -> double sum over neutron and fission grids.
    if gbeta > 0.0 && df == 0.0 {
        let n = nu - 1;
        if id == 1 {
            for j in 0..10 {
                let xj = QP[m][j];
                for k in 0..10 {
                    s += QW[m][j] * QW[n][k] * xj * xj / (galpha * xj + gbeta * QP[n][k] + gamma);
                }
            }
        } else if id == 2 {
            for j in 0..10 {
                let xj = QP[m][j];
                for k in 0..10 {
                    s += QW[m][j] * QW[n][k] * xj / (galpha * xj + gbeta * QP[n][k] + gamma);
                }
            }
        } else if id == 3 {
            for j in 0..10 {
                for k in 0..10 {
                    s += QW[m][j] * QW[n][k] * QP[m][j] * QP[n][k]
                        / (galpha * QP[m][j] + gbeta * QP[n][k] + gamma);
                }
            }
        }
        return s;
    }

    // Case 4: gbeta > 0, df > 0  -> triple sum over all three grids.
    if gbeta > 0.0 && df > 0.0 {
        let n = nu - 1;
        let lm = lamda - 1;
        if id == 1 {
            for j in 0..10 {
                let xj = QP[m][j];
                for k in 0..10 {
                    for ll in 0..10 {
                        s += QW[m][j] * QW[n][k] * QW[lm][ll] * xj * xj
                            / (galpha * xj + gbeta * QP[n][k] + gamma + df * QP[lm][ll]);
                    }
                }
            }
        } else if id == 2 {
            for j in 0..10 {
                for k in 0..10 {
                    for ll in 0..10 {
                        s += QW[m][j] * QW[n][k] * QW[lm][ll] * QP[m][j]
                            / (galpha * QP[m][j] + gbeta * QP[n][k] + gamma + df * QP[lm][ll]);
                    }
                }
            }
        } else if id == 3 {
            for j in 0..10 {
                for k in 0..10 {
                    for ll in 0..10 {
                        s += QW[m][j] * QW[n][k] * QW[lm][ll] * QP[m][j] * QP[n][k]
                            / (galpha * QP[m][j] + gbeta * QP[n][k] + gamma + df * QP[lm][ll]);
                    }
                }
            }
        }
        return s;
    }

    s
}

/// Multiply two real `3x3` matrices, `C = A * B` (dimensionless entries).
///
/// errorr.f90:10666-10684
///
/// `a` and `b` are indexed `[row][col]`; returns the product `[row][col]`.
/// Used by [`efrobns`]. No failure mode.
pub fn eabcmat(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    let mut c = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            let mut acc = 0.0;
            for k in 0..3 {
                acc += a[i][k] * b[k][j];
            }
            c[i][j] = acc;
        }
    }
    c
}

/// In-place inverse of a real symmetric `n x n` matrix (`n <= 3`), returned as
/// a new `3x3` array (dimensionless entries).
///
/// errorr.f90:10616-10664
///
/// Inverts the leading `n x n` block of `d` (indexed `[row][col]`) with NJOY's
/// symmetric Gauss-Jordan sweep. Only the leading `n x n` block is meaningful in
/// the result; entries outside it are carried through untouched. `n` must be in
/// `1..=3`.
///
/// # Errors
///
/// Returns [`NjoyError::NotConvergent`] `{ routine: "errorr::ethrinv" }` if a
/// pivot vanishes (`1 - d[lr][lr] == 0`), i.e. the matrix is singular for this
/// method — upstream signals this via its `kimerr = 1` out-flag.
pub fn ethrinv(d: [[f64; 3]; 3], n: usize) -> Result<[[f64; 3]; 3], NjoyError> {
    let mut d = d;

    // Form I - D on the leading n x n block (symmetric).
    for j in 0..n {
        for i in 0..=j {
            d[i][j] = -d[i][j];
            d[j][i] = d[i][j];
        }
        d[j][j] = 1.0 + d[j][j];
    }

    for lr in 0..n {
        let fooey = 1.0 - d[lr][lr];
        // Exact zero test reproduces the Fortran `fooey.eq.zero` singularity gate.
        if fooey == 0.0 {
            return Err(NjoyError::NotConvergent {
                routine: "errorr::ethrinv",
            });
        }
        d[lr][lr] = 1.0 / fooey;
        let mut s = [0.0f64; 3];
        for j in 0..n {
            s[j] = d[lr][j];
            if j != lr {
                d[j][lr] *= d[lr][lr];
                d[lr][j] = d[j][lr];
            }
        }
        for j in 0..n {
            if j != lr {
                for i in 0..=j {
                    if i != lr {
                        d[i][j] += d[i][lr] * s[j];
                        d[j][i] = d[i][j];
                    }
                }
            }
        }
    }
    Ok(d)
}

/// Inverse of a complex `3x3` matrix by the Frobenius-Schur method.
///
/// errorr.f90:10580-10614
///
/// Given the real and imaginary parts `a` (= `Re M`) and `b` (= `Im M`) of a
/// complex matrix `M = A + iB` (entries dimensionless, indexed `[row][col]`),
/// returns `(c, d)` = the real and imaginary parts of `M^{-1}`, i.e.
/// `M^{-1} = C + iD`. The method computes `C = (A + B A^{-1} B)^{-1}` and
/// `D = -A^{-1} B C`.
///
/// # Errors
///
/// Returns [`NjoyError::NotConvergent`] `{ routine: "errorr::ethrinv" }`
/// (propagated from the inner [`ethrinv`]) if either the real part `A` or the
/// intermediate `A + B A^{-1} B` is singular for that method. Upstream's
/// Fortran instead degrades silently (it copies `A` into `c` and skips the
/// computation when the first inversion fails, and ignores the second
/// inversion's error flag); this port surfaces the failure as an error rather
/// than returning a partially-computed / uninitialised result.
///
/// Returns the tuple `(c, d)` mapping the Fortran out-parameters `(c, d)`.
pub fn efrobns(
    a: [[f64; 3]; 3],
    b: [[f64; 3]; 3],
) -> Result<([[f64; 3]; 3], [[f64; 3]; 3]), NjoyError> {
    let c0 = a; // Fortran seeds c := a before inverting a in place.
    let a_inv = ethrinv(a, 3)?; // a := A^{-1}
    let q = eabcmat(a_inv, b); // q = A^{-1} B
    let mut d = eabcmat(b, q); // d = B A^{-1} B

    // c = A + B A^{-1} B
    let mut c = c0;
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] += d[i][j];
        }
    }
    let c_inv = ethrinv(c, 3)?; // c := (A + B A^{-1} B)^{-1}  == C
    d = eabcmat(q, c_inv); // d = A^{-1} B C
    for row in d.iter_mut() {
        for x in row.iter_mut() {
            *x = -*x; // D = -A^{-1} B C
        }
    }
    Ok((c_inv, d))
}

// --- cleb factorial tables (errorr.f90:10801-10833) -----------------------
//
// Each factorial (k-1)! is stored as FAC[k-1] * 10^NFAC[k-1], with FAC the
// mantissa in [1,10) and NFAC the base-10 exponent (e.g. 4! = 24 = 2.4 * 10^1
// -> FAC[4] = 2.4, NFAC[4] = 1).

/// Factorial mantissas: `(k-1)! = FAC[k-1] * 10^NFAC[k-1]`. errorr.f90:10801-10826.
const FAC: [f64; 101] = [
    1.0, 1.0, 2.0, 6.0, 2.4, 1.2, 7.2, 5.04, 4.032, 3.6288, 3.6288, 3.99168, 4.790016, 6.2270208,
    8.7178291, 1.3076744, 2.0922790, 3.5568743, 6.4023737, 1.2164510, 2.4329020, 5.1090942,
    1.1240007, 2.5852017, 6.2044840, 1.5511210, 4.0329146, 1.0888869, 3.0488834, 8.8417620,
    2.6525286, 8.2228387, 2.6313084, 8.6833176, 2.9523280, 1.0333148, 3.7199333, 1.3763753,
    5.2302262, 2.0397882, 8.1591528, 3.3452527, 1.4050061, 6.0415263, 2.6582716, 1.1962222,
    5.5026222, 2.5862324, 1.2413916, 6.0828186, 3.0414093, 1.5511188, 8.0658175, 4.2748833,
    2.3084370, 1.2696403, 7.1099859, 4.0526920, 2.3505613, 1.3868312, 8.3209871, 5.0758021,
    3.1469973, 1.9826083, 1.2688693, 8.2476506, 5.4434494, 3.6471111, 2.4800355, 1.7112245,
    1.1978572, 8.5047859, 6.1234458, 4.4701155, 3.3078854, 2.4809141, 1.8854947, 1.4518309,
    1.1324281, 8.9461821, 7.1569457, 5.7971260, 4.7536433, 3.9455240, 3.3142401, 2.8171041,
    2.4227095, 2.1077573, 1.8548264, 1.6507955, 1.4857160, 1.3520015, 1.2438414, 1.1567725,
    1.0873662, 1.0329978, 9.9167793, 9.6192760, 9.4268904, 9.3326215, 9.3326215,
];

/// Factorial base-10 exponents: `(k-1)! = FAC[k-1] * 10^NFAC[k-1]`. errorr.f90:10827-10833.
const NFAC: [i32; 101] = [
    0, 0, 0, 0, 1, 2, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14, 15, 17, 18, 19, 21, 22, 23, 25, 26,
    28, 29, 30, 32, 33, 35, 36, 38, 40, 41, 43, 44, 46, 47, 49, 51, 52, 54, 56, 57, 59, 61, 62, 64,
    66, 67, 69, 71, 73, 74, 76, 78, 80, 81, 83, 85, 87, 89, 90, 92, 94, 96, 98, 100, 101, 103, 105,
    107, 109, 111, 113, 115, 116, 118, 120, 122, 124, 126, 128, 130, 132, 134, 136, 138, 140, 142,
    144, 146, 148, 149, 151, 153, 155, 157,
];

/// Specialised Clebsch-Gordan coefficient `<i1 0, i2 0 | i3 0>` (dimensionless).
///
/// errorr.f90:10792-10863
///
/// Returns the Clebsch-Gordan coefficient coupling three *integer* angular
/// momenta `i1`, `i2`, `i3` with all magnetic quantum numbers zero, evaluated
/// from the hard-coded factorial table ([`FAC`]/[`NFAC`]). Inputs are the
/// integer angular momenta themselves (not `2j`), each `>= 0`.
///
/// Returns `0.0` when the triangle rule fails (`i1 + i2 - i3 < 0`, etc.) or when
/// `i1 + i2 + i3` is odd, matching upstream.
///
/// **Faithfulness caveat (deliberate):** the coefficient is scaled by Fortran
/// *integer* powers of ten via [`ipow10`]. Because Fortran `10**n` is `0` for
/// `n < 0`, this routine returns exactly `0.0` for any triple whose
/// factorial-exponent bookkeeping `nept` is negative — even when the true
/// Clebsch-Gordan coefficient is non-zero (e.g. `cleb(2, 2, 2)` and
/// `cleb(2, 0, 2)` both return `0`). This reproduces NJOY bit-for-bit and must
/// not be "corrected".
///
/// No failure mode (returns `0.0` for invalid couplings rather than erroring).
pub fn cleb(i1: i32, i2: i32, i3: i32) -> f64 {
    let mut cleb = 0.0;

    let n1 = i1 + i2 - i3 + 1;
    if n1 <= 0 {
        return cleb;
    }
    let n2 = i1 - i2 + i3 + 1;
    if n2 <= 0 {
        return cleb;
    }
    let n3 = -i1 + i2 + i3 + 1;
    if n3 <= 0 {
        return cleb;
    }
    let it = i1 + i2 + i3;
    if it.rem_euclid(2) != 0 {
        return cleb;
    }
    let n4 = it + 2;
    let mut nept = NFAC[(n1 - 1) as usize] + NFAC[(n2 - 1) as usize] + NFAC[(n3 - 1) as usize]
        - NFAC[(n4 - 1) as usize];
    // Fortran: `if (nept.lt.33) go to 50` else return 0.
    if nept >= 33 {
        return cleb;
    }

    let arg = FAC[(n1 - 1) as usize] * FAC[(n2 - 1) as usize] * FAC[(n3 - 1) as usize]
        / FAC[(n4 - 1) as usize];
    let z1 = arg * ipow10(nept);
    let d123 = z1.sqrt();

    let is = it / 2;
    let mut signx = 1.0;
    if (is + i3).rem_euclid(2) == 1 {
        signx = -1.0;
    }
    let ia = is + 1;
    let ib = is - i1 + 1;
    let ic = is - i2 + 1;
    let idx = is - i3 + 1;
    nept = NFAC[(ia - 1) as usize]
        - NFAC[(ib - 1) as usize]
        - NFAC[(ic - 1) as usize]
        - NFAC[(idx - 1) as usize];
    let arg = FAC[(ia - 1) as usize]
        / (FAC[(ib - 1) as usize] * FAC[(ic - 1) as usize] * FAC[(idx - 1) as usize]);
    let z1 = (2 * i3 + 1) as f64;
    cleb = signx * z1.sqrt() * d123 * arg * ipow10(nept);
    cleb
}

#[cfg(test)]
mod tests {
    use super::*;

    /// METHODOLOGY: `efacphi` at `l = 0` is defined by NJOY (errorr.f90:10348)
    /// as `phi = rho` exactly. INPUT: rho = 0.37. PASS: bit-exact equality.
    /// DERIVATION: the `l == 0` branch returns `rho` unmodified.
    #[test]
    fn efacphi_l0_is_rho() {
        assert_eq!(efacphi(0, 0.37), 0.37);
    }

    /// METHODOLOGY: `efacphi` at `l = 1` is `phi = rho - atan(rho)`
    /// (errorr.f90:10350). INPUT: rho = 0.8. PASS: matches the closed form to
    /// 1e-15. DERIVATION: rho - atan(rho) = 0.8 - atan(0.8).
    #[test]
    fn efacphi_l1_closed_form() {
        let rho = 0.8f64;
        let expect = rho - rho.atan();
        assert!((efacphi(1, rho) - expect).abs() < 1e-15);
    }

    /// METHODOLOGY: `efacts` at `l = 0` gives shift factor S_0 = 0 and
    /// penetration factor P_0 = rho (errorr.f90:10379-10380). INPUT: rho = 0.42.
    /// PASS: bit-exact. DERIVATION: the `l == 0` branch returns `(0, rho)`.
    #[test]
    fn efacts_l0_penetration_is_rho() {
        let (se, pe) = efacts(0, 0.42);
        assert_eq!(se, 0.0);
        assert_eq!(pe, 0.42);
    }

    /// METHODOLOGY: `efacts` at `l = 1` (errorr.f90:10382-10384) has
    /// den = 1 + rho^2, P_1 = rho^3/den, S_1 = -1/den. INPUT: rho = 0.5.
    /// PASS: matches closed form to 1e-15. DERIVATION: den = 1.25,
    /// P_1 = 0.125/1.25 = 0.1, S_1 = -1/1.25 = -0.8.
    #[test]
    fn efacts_l1_closed_form() {
        let (se, pe) = efacts(1, 0.5);
        assert!((pe - 0.1).abs() < 1e-15);
        assert!((se - (-0.8)).abs() < 1e-15);
    }

    /// METHODOLOGY: `eunfac` at `l = 0` returns (vl, ps) = (amun, rhoc)
    /// (errorr.f90:10419-10420). INPUT: rho = 0.3, rhoc = 0.31, amun = 2.0.
    /// PASS: bit-exact. DERIVATION: the `l == 0` branch returns `(amun, rhoc)`.
    #[test]
    fn eunfac_l0_identity() {
        let (vl, ps) = eunfac(0, 0.3, 0.31, 2.0);
        assert_eq!(vl, 2.0);
        assert_eq!(ps, 0.31);
    }

    /// METHODOLOGY: `eabcmat` multiplies 3x3 matrices; A * I = A.
    /// INPUT: A = [[1,2,3],[4,5,6],[7,8,10]], B = identity. PASS: bit-exact
    /// equality with A. DERIVATION: matrix times identity is the matrix.
    #[test]
    fn eabcmat_times_identity() {
        let a = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0], [7.0, 8.0, 10.0]];
        let id = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(eabcmat(a, id), a);
    }

    /// METHODOLOGY: `ethrinv` inverts a real symmetric matrix; a diagonal
    /// matrix inverts to the reciprocal diagonal. INPUT: diag(2,4,5).
    /// PASS: result == diag(0.5, 0.25, 0.2) to 1e-15. DERIVATION: hand-tracing
    /// the NJOY sweep on diag(2,4,5) yields exactly diag(1/2,1/4,1/5); the
    /// inverse of a diagonal matrix is the elementwise reciprocal.
    #[test]
    fn ethrinv_diagonal() {
        let d = [[2.0, 0.0, 0.0], [0.0, 4.0, 0.0], [0.0, 0.0, 5.0]];
        let inv = ethrinv(d, 3).expect("nonsingular");
        assert!((inv[0][0] - 0.5).abs() < 1e-15);
        assert!((inv[1][1] - 0.25).abs() < 1e-15);
        assert!((inv[2][2] - 0.2).abs() < 1e-15);
        // off-diagonals stay zero
        assert!(inv[0][1].abs() < 1e-15);
        assert!(inv[1][2].abs() < 1e-15);
    }

    /// METHODOLOGY: `ethrinv` signals singularity when a pivot
    /// `1 - d[lr][lr]` vanishes. INPUT: the zero matrix, whose I - D transform
    /// makes every diagonal 1 so the first pivot is 1 - 1 = 0. PASS: returns
    /// `Err(NotConvergent)`. DERIVATION: hand-tracing the first sweep gives
    /// fooey = 0, triggering the kimerr = 1 branch (errorr.f90:10640-10642).
    #[test]
    fn ethrinv_singular_errors() {
        let d = [[0.0; 3]; 3];
        assert!(ethrinv(d, 3).is_err());
    }

    /// METHODOLOGY: verify `efrobns` produces a genuine complex inverse by
    /// forming M = A + iB, computing (C, D) = efrobns(A, B), then checking the
    /// complex product M * (C + iD) = I via the real building blocks:
    /// Re = A*C - B*D, Im = A*D + B*C. INPUT: A = diag(2,3,4) and the *symmetric*
    /// B = [[0,1,0],[1,0,1],[0,1,0]] (the routine assumes symmetric matrices, as
    /// in R-matrix theory). PASS: Re == identity and Im == 0 to 1e-12.
    /// DERIVATION: the Frobenius-Schur identity C = (A + B A^{-1} B)^{-1},
    /// D = -A^{-1} B C is the exact inverse of A + iB, so the reconstructed
    /// product must be the identity.
    #[test]
    fn efrobns_reconstructs_identity() {
        let a = [[2.0, 0.0, 0.0], [0.0, 3.0, 0.0], [0.0, 0.0, 4.0]];
        let b = [[0.0, 1.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 0.0]];
        let (c, d) = efrobns(a, b).expect("nonsingular");

        // Re(M * inv) = A*C - B*D ; Im(M * inv) = A*D + B*C
        let ac = eabcmat(a, c);
        let bd = eabcmat(b, d);
        let ad = eabcmat(a, d);
        let bc = eabcmat(b, c);
        for i in 0..3 {
            for j in 0..3 {
                let re = ac[i][j] - bd[i][j];
                let im = ad[i][j] + bc[i][j];
                let expect_re = if i == j { 1.0 } else { 0.0 };
                assert!((re - expect_re).abs() < 1e-12, "re[{i}][{j}] = {re}");
                assert!(im.abs() < 1e-12, "im[{i}][{j}] = {im}");
            }
        }
    }

    /// METHODOLOGY: `cleb(0,0,0)` = <0 0, 0 0 | 0 0> = 1 by definition.
    /// PASS: within 1e-12. DERIVATION: all factorial terms are 1, nept = 0 in
    /// both places so ipow10(0) = 1, signx = +1, sqrt(2*0+1) = 1.
    #[test]
    fn cleb_trivial_is_one() {
        assert!((cleb(0, 0, 0) - 1.0).abs() < 1e-12);
    }

    /// METHODOLOGY: `cleb(1,1,0)` = <1 0, 1 0 | 0 0> = -1/sqrt(3), a standard
    /// closed-form Clebsch-Gordan value. PASS: within 1e-6 (table mantissas
    /// carry ~7 sig figs). DERIVATION: <l 0, l 0 | 0 0> = (-1)^l / sqrt(2l+1);
    /// for l = 1 this is -1/sqrt(3) = -0.57735. Hand-tracing the routine gives
    /// d123 = sqrt(1/3), signx = -1, so cleb = -1/sqrt(3).
    #[test]
    fn cleb_1_1_0_known_value() {
        let expect = -1.0 / 3.0f64.sqrt();
        assert!(
            (cleb(1, 1, 0) - expect).abs() < 1e-6,
            "cleb = {}",
            cleb(1, 1, 0)
        );
    }

    /// METHODOLOGY: triangle-rule / parity gates return 0. INPUTS:
    /// `cleb(1,1,3)` violates the triangle rule (i1+i2-i3 < 0) and `cleb(1,1,1)`
    /// has odd i1+i2+i3. PASS: both return exactly 0. DERIVATION: n1 <= 0 and
    /// the odd-sum guard respectively (errorr.f90:10837, 10843).
    #[test]
    fn cleb_selection_rules_zero() {
        assert_eq!(cleb(1, 1, 3), 0.0);
        assert_eq!(cleb(1, 1, 1), 0.0);
    }

    /// METHODOLOGY: reproduce the deliberate `10**nept` integer-arithmetic
    /// quirk. For `cleb(2,2,2)` the first exponent nept = -3, so Fortran
    /// `10**(-3) = 0` (integer) forces the result to 0 despite the true
    /// coefficient <2 0, 2 0 | 2 0> = -sqrt(2/7) being non-zero. PASS: returns
    /// exactly 0. DERIVATION: nfac(3)*3 - nfac(8) = 0 - 3 = -3 < 0, and
    /// ipow10(-3) = 0 (see [`ipow10`]).
    #[test]
    fn cleb_integer_pow_quirk_is_zero() {
        assert_eq!(cleb(2, 2, 2), 0.0);
    }

    /// METHODOLOGY: `egnrl` with gbeta = 0, df = 0, id = 2 reduces to the single
    /// 10-point sum over the neutron grid. INPUT: galpha = 1, gamma = 1, mu = 1.
    /// PASS: matches an independent reevaluation of the same quadrature sum to
    /// 1e-15. DERIVATION: s = sum_j QW[0][j] * QP[0][j] / (QP[0][j] + 1),
    /// recomputed directly here from the same table (self-consistency check on
    /// the table transcription and the branch selection).
    #[test]
    fn egnrl_single_sum_matches_manual() {
        let got = egnrl(1.0, 0.0, 1.0, 1, 1, 1, 0.0, 2);
        let mut want = 0.0;
        for j in 0..10 {
            want += QW[0][j] * QP[0][j] / (1.0 * QP[0][j] + 1.0);
        }
        assert!((got - want).abs() < 1e-15);
        assert!(got > 0.0);
    }

    /// METHODOLOGY: `egnrl` guard clauses return 0 for non-physical widths.
    /// INPUT: galpha = 0 (no neutron width). PASS: returns exactly 0.
    /// DERIVATION: the `galpha <= 0` guard (errorr.f90:10475) returns s = 0.
    #[test]
    fn egnrl_zero_neutron_width() {
        assert_eq!(egnrl(0.0, 0.0, 1.0, 1, 1, 1, 0.0, 2), 0.0);
    }
}
