// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Cold-hydrogen / -deuterium Young-Koppel helpers (partial port).
//!
//! The full `coldh` orchestrator (leapr.f90:1936-2183) — which convolves the
//! solid/diffusive `S(alpha, beta)` with discrete Young-Koppel rotational modes
//! for ortho/para hydrogen and deuterium — is **not** ported (see [`coldh`]). Its
//! self-contained numerical helpers, which are the bulk of the physics content,
//! **are** ported and unit-tested here:
//!
//! - [`bt`] — rotational statistical-weight factor `P_j` (leapr.f90:2185-2209).
//! - [`sumh`] — sum over Bessel functions and Clebsch-Gordan coefficients
//!   (2211-2245).
//! - [`cn`] — Clebsch-Gordan / Wigner coefficient (2247-2340).
//! - [`sjbes`] — spherical Bessel function `j_n(x)` (2342-2442).
//! - [`terpk`] — interpolation in the static structure factor `S(kappa)`
//!   (2444-2466).
//!
//! ## Units
//! `x` in [`bt`] is `de/tev` (dimensionless rotational-energy ratio); `y` in
//! [`sumh`]/`sjbes` is the dimensionless argument `b' * wavenumber`; all outputs
//! are dimensionless.

use crate::NjoyError;

/// Rotational statistical-weight factor `P_j` (`bt`, leapr.f90:2185-2209).
///
/// For rotational level `j` at reduced temperature `x = de/tev`, returns the
/// Boltzmann population weight normalized over the first ten same-parity levels:
/// `P_j = (2j+1) e^{-j(j+1)x/2} / (2 * sum_k (2k+1) e^{-k(k+1)x/2})`. The sum over
/// same-parity `P_j` equals `1/2` by construction (see the unit test).
///
/// # Arguments
/// * `j` — rotational quantum number (`>= 0`).
/// * `x` — reduced rotational-energy ratio `de/tev` (dimensionless).
pub fn bt(j: i32, x: f64) -> f64 {
    let jf = j as f64;
    let yy = 0.5 * jf * (jf + 1.0);
    let a = (2.0 * jf + 1.0) * (-yy * x).exp();
    let mut b = 0.0_f64;
    for i in 1..=10 {
        let mut k = 2 * i - 2;
        if j % 2 == 1 {
            k += 1;
        }
        let kf = k as f64;
        let yyk = 0.5 * kf * (kf + 1.0);
        b += (2.0 * kf + 1.0) * (-yyk * x).exp();
    }
    a / (2.0 * b)
}

/// Clebsch-Gordan / Wigner `3-j`-type coefficient `C_n(jj, ll, nn)` (`cn`,
/// leapr.f90:2247-2340).
///
/// Returns zero unless `jj + ll + nn` is even (the triangle/parity rule); computes
/// the coefficient from log-factorials to avoid overflow.
///
/// # Arguments
/// * `jj`, `ll`, `nn` — non-negative integer angular-momentum indices.
pub fn cn(jj: i32, ll: i32, nn: i32) -> f64 {
    let kdet = (jj + ll + nn) / 2;
    let kdel = jj + ll + nn - 2 * kdet;
    if kdel != 0 {
        return 0.0;
    }
    let ka1 = jj + ll + nn;
    let ka2 = jj + ll - nn;
    let ka3 = jj - ll + nn;
    let ka4 = ll - jj + nn;
    let kb1 = ka1 / 2;
    let kb2 = ka2 / 2;
    let kb3 = ka3 / 2;
    let kb4 = ka4 / 2;

    // sqrt(factorial) for the a-terms, factorial for the b-terms, via log-sum
    let sqrt_fact = |k: i32| -> f64 {
        let mut s = 0.0_f64;
        for i in 1..=k {
            s += (i as f64).ln();
        }
        if s > 0.0 {
            s.exp().sqrt()
        } else {
            1.0
        }
    };
    let fact = |k: i32| -> f64 {
        let mut s = 0.0_f64;
        for i in 1..=k {
            s += (i as f64).ln();
        }
        if s > 0.0 {
            s.exp()
        } else {
            1.0
        }
    };

    let a1 = sqrt_fact(ka1);
    let a2 = sqrt_fact(ka2);
    let a3 = sqrt_fact(ka3);
    let a4 = sqrt_fact(ka4);
    let b1 = fact(kb1);
    let b2 = fact(kb2);
    let b3 = fact(kb3);
    let b4 = fact(kb4);

    let rat = (2 * nn + 1) as f64 / (jj + ll + nn + 1) as f64;
    let iwign = (jj + ll - nn) / 2;
    let sign = if iwign % 2 == 0 { 1.0 } else { -1.0 };
    sign * rat.sqrt() * b1 / a1 * a2 / b2 * a3 / b3 * a4 / b4
}

/// Spherical Bessel function `j_n(x)` (`sjbes`, leapr.f90:2342-2442).
///
/// Uses a small-argument series, the closed forms for `n = 0`, and downward
/// (Miller) recursion for higher orders. Returns `0.0` for out-of-range arguments
/// (`n >= 30000`, `x > 3e4`, `x < 0`, or `n < 0`) rather than aborting the way the
/// Fortran `error`/`mess` calls do.
///
/// # Arguments
/// * `n` — order (`>= 0`).
/// * `x` — argument (dimensionless, `>= 0`).
pub fn sjbes(n: i32, x: f64) -> f64 {
    const HUGE: f64 = 1e25;
    const SMALL: f64 = 2e-38;
    if n >= 30000 || x > 3e4 || x < 0.0 || n < 0 {
        return 0.0;
    }
    let bessel;
    if x <= 7e-4 {
        if n == 0 {
            bessel = 1.0;
        } else if n > 10 {
            bessel = 0.0;
        } else {
            let mut t1 = 3.0_f64;
            let mut t2 = 1.0_f64;
            let mut t3 = 0.0_f64;
            for _ in 1..=n {
                t3 = t2 * x / t1;
                t1 += 2.0;
                t2 = t3;
            }
            bessel = t3;
        }
    } else {
        let w = if x < 0.2 {
            let y = x * x;
            1.0 - y * (1.0 - y / 20.0) / 6.0
        } else {
            x.sin() / x
        };
        if n == 0 {
            bessel = w;
        } else {
            let l = if x >= 100.0 {
                (x / 50.0 + 18.0) as i32
            } else if x >= 10.0 {
                (x / 10.0 + 10.0) as i32
            } else if x > 1.0 {
                (x / 2.0 + 5.0) as i32
            } else {
                5
            };
            let iii = x as i32;
            let kmax = if iii > n { iii } else { n };
            let nm = kmax + l;
            let z = 1.0 / x;
            let mut t3 = 0.0_f64;
            let mut t2 = SMALL;
            let mut t1 = 0.0_f64;
            let mut sj = 0.0_f64;
            for i in 1..=nm {
                let k = nm - i;
                t1 = (2.0 * k as f64 + 3.0) * z * t2 - t3;
                if n == k {
                    sj = t1;
                }
                if t1.abs() >= HUGE {
                    t1 /= HUGE;
                    t2 /= HUGE;
                    sj /= HUGE;
                }
                t3 = t2;
                t2 = t1;
            }
            bessel = w * sj / t1;
        }
    }
    bessel
}

/// Sum over spherical Bessel functions and Clebsch-Gordan coefficients (`sumh`,
/// leapr.f90:2211-2245).
///
/// Evaluates `sum_n (j_n(y) C_n(j, jp))^2` over the allowed `n` for the rotational
/// transition `j -> jp`.
///
/// # Arguments
/// * `j`, `jp` — initial and final rotational quantum numbers (`>= 0`).
/// * `y` — dimensionless Bessel argument.
pub fn sumh(j: i32, jp: i32, y: f64) -> f64 {
    if j == 0 {
        let t = sjbes(jp, y) * cn(j, jp, jp);
        t * t
    } else if jp == 0 {
        let t = sjbes(j, y) * cn(j, 0, j);
        t * t
    } else {
        let imk = (j - jp).abs() + 1;
        let ipk1 = j + jp + 1;
        let mpk = ipk1 - imk;
        let ipk = if mpk <= 9 { ipk1 } else { imk + 9 };
        let mut sum1 = 0.0_f64;
        for nn in imk..=ipk {
            let n1 = nn - 1;
            let t = sjbes(n1, y) * cn(j, jp, n1);
            sum1 += t * t;
        }
        sum1
    }
}

/// Interpolate in the static structure factor `S(kappa)` for a required kappa
/// (`terpk`, leapr.f90:2444-2466). Unlike [`crate::leapr::continuous::terpt`],
/// this defaults to `1.0` (not `0.0`) outside the tabulated range.
///
/// # Arguments
/// * `ska` — the `S(kappa)` table (dimensionless).
/// * `delta` — kappa-grid spacing `dka` (inverse angstroms).
/// * `be` — the kappa value (inverse angstroms).
pub fn terpk(ska: &[f64], delta: f64, be: f64) -> f64 {
    let nka = ska.len();
    if be < 0.0 || be > nka as f64 * delta {
        return 1.0;
    }
    let i = (be / delta).floor() as usize;
    if i < nka - 1 {
        let bt = i as f64 * delta;
        let btp = bt + delta;
        ska[i] + (be - bt) * (ska[i + 1] - ska[i]) / (btp - bt)
    } else {
        1.0
    }
}

/// The full cold-hydrogen/deuterium Young-Koppel convolution — **not ported**.
///
/// The `coldh` orchestrator (leapr.f90:1936-2183) builds the asymmetric,
/// beta-non-symmetric `S(alpha, beta)` for ortho/para H2 or D2 by convolving the
/// solid/diffusive law with discrete rotational modes. Only its self-contained
/// helpers ([`bt`], [`sumh`], [`cn`], [`sjbes`], [`terpk`]) are ported; the
/// orchestrator itself returns [`crate::NjoyError::NotPorted`].
pub fn coldh() -> Result<(), NjoyError> {
    Err(NjoyError::NotPorted(
        "leapr coldh orchestrator (Young-Koppel rotational convolution; helpers ported)",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Methodology: the same-parity rotational weights sum to `1/2` exactly, by
    /// construction of [`bt`] (each `P_j = a_j / (2 * sum a_k)`, and the sum runs
    /// over the same ten same-parity levels used to build the denominator).
    /// Result (2026-07-15): sum over even `j = 0,2,...,18` equals `0.5` to
    /// `< 1e-12` at `x = 0.05`.
    #[test]
    fn bt_even_parity_weights_sum_to_half() {
        let x = 0.05;
        let sum: f64 = (0..10).map(|i| bt(2 * i, x)).sum();
        assert!((sum - 0.5).abs() < 1e-12, "sum P_j(even) = {sum}");
    }

    /// Methodology: `cn(0,0,0)` must equal `1` (trivial Clebsch-Gordan), and the
    /// parity rule must zero out `cn` when `jj+ll+nn` is odd.
    /// Result (2026-07-15): `cn(0,0,0) = 1.0` exactly; `cn(1,1,1) = 0`.
    #[test]
    fn cn_trivial_and_parity() {
        assert!((cn(0, 0, 0) - 1.0).abs() < 1e-12, "cn(0,0,0) = {}", cn(0, 0, 0));
        assert_eq!(cn(1, 1, 1), 0.0, "odd-sum cn should vanish");
    }

    /// Methodology: `sjbes` must reproduce the closed forms
    /// `j_0(x) = sin(x)/x` and `j_1(x) = sin(x)/x^2 - cos(x)/x`.
    /// Result (2026-07-15): at `x = 1`, `j_0 = 0.8414710`, `j_1 = 0.3011687`,
    /// both to `< 1e-5`.
    #[test]
    fn sjbes_closed_forms() {
        let x = 1.0_f64;
        let j0 = x.sin() / x;
        let j1 = x.sin() / (x * x) - x.cos() / x;
        assert!((sjbes(0, x) - j0).abs() < 1e-5, "j0(1) = {}", sjbes(0, x));
        assert!((sjbes(1, x) - j1).abs() < 1e-5, "j1(1) = {}", sjbes(1, x));
    }

    /// `terpk` linear interpolation + out-of-range default of 1.
    /// Result (2026-07-15): midpoint interpolates to the mean; beyond range -> 1.
    #[test]
    fn terpk_interpolation_and_default() {
        let ska = vec![2.0, 4.0, 6.0];
        // at be = 0.5*delta between ska[0]=2 and ska[1]=4 -> 3.0
        assert!((terpk(&ska, 1.0, 0.5) - 3.0).abs() < 1e-12);
        // beyond range -> 1.0
        assert_eq!(terpk(&ska, 1.0, 10.0), 1.0);
    }

    #[test]
    fn coldh_orchestrator_not_ported() {
        assert!(matches!(coldh(), Err(NjoyError::NotPorted(_))));
    }
}
