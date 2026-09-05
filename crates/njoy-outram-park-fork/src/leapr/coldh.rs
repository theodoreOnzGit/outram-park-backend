// Ported from NJOY2016 `src/leapr.f90` (git commit ac5adf5f33d893e42f2eed7fb286b0d51c7580da).
// NJOY2016 is under a modified BSD 3-Clause (LANL/DOE) licence, GPL-compatible;
// this derivative file is distributed under GPL-3.0-only. This is a modified,
// non-LANL version, not endorsed by LANL/DOE. See crate root LICENSE.njoy + NOTICE.

//! Cold-hydrogen / -deuterium Young-Koppel scattering.
//!
//! The `coldh` orchestrator (leapr.f90:1936-2183) — which convolves the
//! solid/diffusive `S(alpha, beta)` with discrete Young-Koppel rotational modes
//! for ortho/para hydrogen and deuterium — is ported as
//! [`add_cold_hydrogen`] (self-consistency tested; **not** validated against a
//! reference LEAPR cold tape). Its self-contained numerical helpers are ported
//! and unit-tested here:
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

use crate::common::phys::{EV_ERG, HBAR_ERG_S};
use crate::leapr::discrete::{bfill, exts, sint};
use crate::leapr::input::{ColdOption, LeaprInput};
use crate::leapr::SabMatrix;

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

/// Convolve the solid/diffusive `S(alpha, beta)` with discrete Young-Koppel
/// rotational modes for ortho/para hydrogen or deuterium (`coldh`,
/// leapr.f90:1936-2183).
///
/// This is the cold-moderator orchestrator. It reads the incoherent-Gaussian
/// negative-beta law `ssm` (from the continuous/translation/discrete pipeline),
/// convolves it with the molecular rotational transitions (`j -> j'`) using the
/// Young-Koppel formulas, and writes the resulting **asymmetric** law back:
/// negative-beta into `ssm`, positive-beta into `ssp`. The result is *not*
/// symmetric in beta (spin correlations break the symmetry), which is why LEAPR
/// sets `LASYM`/`isym` when a cold option is active.
///
/// # Arguments
/// * `ssm` — negative-beta asymmetric law `Ss(alpha,-beta)`; read as the input
///   solid law, overwritten in place with the cold result.
/// * `ssp` — positive-beta asymmetric law `Ss(alpha,+beta)`; filled here (pass a
///   zeroed matrix of the same `nbeta x nalpha` shape).
/// * `input` — the LEAPR job (alpha/beta grids, `lat`, `arat`, `twt`, `tbeta`).
/// * `cold` — which rotational law (ortho/para H/D). [`ColdOption::None`] is a
///   no-op returning `0.0`.
/// * `ska`, `dka` — the static structure factor `S(kappa)` table and its kappa
///   increment \[1/angstrom\] (card 17/18).
/// * `tempf` — the effective (SCT) temperature `T_eff` \[K\] for this temperature
///   (LEAPR `tempf(itemp)`).
///
/// # Returns
/// The trapezoidal normalization check `sum0` of the **last** alpha column
/// (LEAPR prints this per alpha; the last one is returned for a smoke test). It
/// should be `O(1)` for a well-normalized law.
///
/// # Validation status
/// **Untrusted AI draft, self-consistency tested only.** Ported line-for-line
/// from `coldh`, reusing the already-ported [`bt`]/[`sumh`]/[`terpk`] helpers and
/// the discrete-oscillator [`bfill`]/[`exts`]/[`sint`]. It has **not** been
/// validated against a reference LEAPR cold-H2/D2 MF=7 tape — only checked for
/// finiteness, correct array population, and a plausible normalization (see the
/// unit test). The internal `ifree`/`nokap` switches are fixed off, matching the
/// NJOY defaults (`ifree=0`, `nokap=0`).
pub fn add_cold_hydrogen(
    ssm: &mut SabMatrix,
    ssp: &mut SabMatrix,
    input: &LeaprInput,
    cold: ColdOption,
    ska: &[f64],
    dka: f64,
    tempf: f64,
) -> f64 {
    // Physical constants from leapr.f90:1962-1976 (HUMAN RE-VERIFY the literals).
    const PMASS: f64 = 1.6726231e-24; // proton mass [g]
    const DMASS: f64 = 3.343586e-24; // deuteron mass [g]
    const DEH: f64 = 0.0147; // H2 rotational constant [eV]
    const DED: f64 = 0.0074; // D2 rotational constant [eV]
    const SAMPCH: f64 = 0.356; // H coherent scattering amplitude
    const SAMPCD: f64 = 0.668; // D coherent scattering amplitude
    const SAMPIH: f64 = 2.526; // H incoherent scattering amplitude
    const SAMPID: f64 = 0.403; // D incoherent scattering amplitude
    const THERM: f64 = 0.0253; // thermal reference [eV]
    const ANGST: f64 = 1.0e-8; // 1 angstrom [cm]
    const JTERM: i32 = 3; // number of j terms

    let law = match cold {
        ColdOption::None => return 0.0,
        ColdOption::OrthoHydrogen => 2,
        ColdOption::ParaHydrogen => 3,
        ColdOption::OrthoDeuterium => 4,
        ColdOption::ParaDeuterium => 5,
    };

    let tev = input.tev();
    let tempr = input.temperature_k.abs();
    let nbeta = input.beta.len();
    let nalpha = input.alpha.len();

    let de = if law > 3 { DED } else { DEH };
    let x = de / tev;
    let (amassm, sampc, bp, sampi) = if law > 3 {
        (
            6.69e-24_f64,
            SAMPCD,
            HBAR_ERG_S / 2.0 * (2.0 / DED / EV_ERG / DMASS).sqrt() / ANGST,
            SAMPID,
        )
    } else {
        (
            3.3464e-24_f64,
            SAMPCH,
            HBAR_ERG_S / 2.0 * (2.0 / DEH / EV_ERG / PMASS).sqrt() / ANGST,
            SAMPIH,
        )
    };
    let wt = input.continuous.twt + input.continuous.tbeta;
    // tbart is set once (not accumulated across alpha, unlike `discre`).
    let tbart = tempf / tempr;

    // beta grid, scaled to eV when lat=1 (leapr.f90:2047-2052).
    let mut exb = vec![0.0_f64; nbeta];
    let mut betan = vec![0.0_f64; nbeta];
    for i in 0..nbeta {
        let mut be = input.beta[i];
        if input.lat {
            be = be * THERM / tev;
        }
        exb[i] = (-be / 2.0).exp();
        betan[i] = be;
    }
    let (bex, rdbex, nbx) = bfill(&betan);

    let mut last_sum0 = 0.0_f64;

    for nal in 0..nalpha {
        let al = input.alpha[nal] * (if input.lat { THERM / tev } else { 1.0 }) / input.arat;
        let waven = ANGST * (amassm * tev * EV_ERG * al).sqrt() / HBAR_ERG_S;
        let y = bp * waven;
        let sk = terpk(ska, dka, waven); // nokap = 0

        // spin-correlation factors (2032-2043).
        let (mut swe, mut swo) = match law {
            2 => (
                sampi * sampi / 3.0,
                sk * sampc * sampc + 2.0 * sampi * sampi / 3.0,
            ),
            3 => (sk * sampc * sampc, sampi * sampi),
            4 => (
                sk * sampc * sampc + 5.0 * sampi * sampi / 8.0,
                3.0 * sampi * sampi / 8.0,
            ),
            5 => (
                3.0 * sampi * sampi / 4.0,
                sk * sampc * sampc + sampi * sampi / 4.0,
            ),
            _ => (0.0, 0.0),
        };
        let snorm = sampi * sampi + sampc * sampc;
        swe /= snorm;
        swo /= snorm;

        // snapshot the incoming ssm column, extend to the symmetric grid (2055).
        let col: Vec<f64> = (0..nbeta).map(|i| ssm.get(i, nal)).collect();
        let sex = exts(&col, &exb, &betan, nbx);

        // beta loop: positive beta -> ssp, negative beta -> ssm (2060-2134).
        let jjmax = 2 * nbeta - 1;
        for jj in 1..=jjmax {
            let k = if jj < nbeta {
                nbeta - jj + 1
            } else {
                jj - nbeta + 1
            }; // 1-based
            let mut be = betan[k - 1];
            if jj < nbeta {
                be = -be;
            }
            let mut sn = 0.0_f64;

            // oscillator (rotational j) loop; parity set by the ortho/para law.
            let ipo: i32 = if law == 2 || law == 5 { 2 } else { 1 };
            let jt1: i32 = 2 * JTERM + if ipo == 2 { 1 } else { 0 };
            let mut l = ipo;
            while l <= jt1 {
                let j = l - 1;
                let pj = bt(j, x);

                // even j-prime (2082-2100)
                let mut snlg = 0.0_f64;
                let mut lp = 1;
                while lp <= 9 {
                    let jp = lp - 1;
                    let betap = ((-j * (j + 1) + jp * (jp + 1)) as f64) * x / 2.0;
                    let tmp = (2.0 * (jp as f64) + 1.0) * pj * swe * 4.0 * sumh(j, jp, y);
                    let bn = be + betap;
                    let add = sint(bn, &bex, &rdbex, &sex, nbx, al, wt, tbart, &betan);
                    snlg += tmp * add;
                    lp += 2;
                }

                // odd j-prime (2103-2122)
                let mut snlk = 0.0_f64;
                let mut lp = 2;
                while lp <= 10 {
                    let jp = lp - 1;
                    let betap = ((-j * (j + 1) + jp * (jp + 1)) as f64) * x / 2.0;
                    let tmp = (2.0 * (jp as f64) + 1.0) * pj * swo * 4.0 * sumh(j, jp, y);
                    let bn = be + betap;
                    let add = sint(bn, &bex, &rdbex, &sex, nbx, al, wt, tbart, &betan);
                    snlk += tmp * add;
                    lp += 2;
                }

                sn += snlg + snlk;
                l += 2;
            }

            if jj <= nbeta {
                ssm.set(k - 1, nal, sn);
            }
            if jj >= nbeta {
                ssp.set(k - 1, nal, sn);
            }
        }

        // trapezoidal normalization check over beta (2152-2173).
        let mut sum0 = 0.0_f64;
        let mut bel = 0.0_f64;
        let mut ff1l = 0.0_f64;
        let mut ff2l = 0.0_f64;
        for nbe in 0..nbeta {
            let be = betan[nbe];
            let ff2 = ssm.get(nbe, nal);
            let ff1 = ssp.get(nbe, nal);
            if nbe != 0 {
                sum0 += (be - bel) * (ff1l + ff2l + ff1 + ff2) / 2.0;
            }
            bel = be;
            ff1l = ff1;
            ff2l = ff2;
        }
        last_sum0 = sum0;
    }

    last_sum0
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
        assert!(
            (cn(0, 0, 0) - 1.0).abs() < 1e-12,
            "cn(0,0,0) = {}",
            cn(0, 0, 0)
        );
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

    use crate::leapr::input::{ColdOption, ContinuousDist, LeaprInput};
    use crate::leapr::SabMatrix;

    /// Methodology: drive [`add_cold_hydrogen`] for para-hydrogen on a small
    /// grid (3 alpha x 4 beta) with a flat solid law and a unit `S(kappa)`, then
    /// check that (a) it runs without panicking, (b) both `ssm` and `ssp` are
    /// fully populated with finite values, (c) the convolution actually changed
    /// the input `ssm` (it is no longer the flat seed), and (d) the returned
    /// normalization is finite. There is **no** reference oracle here, so this is
    /// a self-consistency smoke test only, not a validation against NJOY.
    /// Result (2026-07-15): runs; all entries finite; ssm changed; sum0 finite.
    #[test]
    fn add_cold_hydrogen_runs_and_populates_ssp() {
        let input = LeaprInput {
            alpha: vec![0.2, 1.0, 3.0],
            beta: vec![0.0, 0.5, 1.5, 3.0],
            lat: false,
            arat: 1.0,
            nphon: 100,
            temperature_k: 20.0, // cold moderator
            continuous: ContinuousDist {
                delta_ev: 1e-3,
                rho: vec![0.0, 1.0, 2.0],
                twt: 0.0,
                c: 0.0,
                tbeta: 1.0,
            },
            oscillators: vec![],
            constants: crate::leapr::vintage::PhysicalConstants::default(),
        };
        let nbeta = input.beta.len();
        let nalpha = input.alpha.len();
        let mut ssm = SabMatrix::zeros(nbeta, nalpha);
        // flat non-trivial seed solid law
        for ib in 0..nbeta {
            for ia in 0..nalpha {
                ssm.set(ib, ia, 0.1);
            }
        }
        let seed = ssm.clone();
        let mut ssp = SabMatrix::zeros(nbeta, nalpha);
        let ska = vec![1.0, 1.0, 1.0, 1.0, 1.0];
        let dka = 0.5;
        let sum0 = add_cold_hydrogen(
            &mut ssm,
            &mut ssp,
            &input,
            ColdOption::ParaHydrogen,
            &ska,
            dka,
            30.0,
        );
        // every entry finite
        for ib in 0..nbeta {
            for ia in 0..nalpha {
                assert!(ssm.get(ib, ia).is_finite(), "ssm[{ib},{ia}] finite");
                assert!(ssp.get(ib, ia).is_finite(), "ssp[{ib},{ia}] finite");
                assert!(ssm.get(ib, ia) >= 0.0, "ssm[{ib},{ia}] >= 0");
                assert!(ssp.get(ib, ia) >= 0.0, "ssp[{ib},{ia}] >= 0");
            }
        }
        assert!(sum0.is_finite(), "normalization check finite, got {sum0}");
        // the convolution changed the law (not still the flat seed everywhere)
        assert!(ssm != seed, "cold convolution modified ssm");
        assert!(
            (0..nalpha).any(|ia| (0..nbeta).any(|ib| ssp.get(ib, ia) > 0.0)),
            "ssp gained positive-beta scattering"
        );
    }

    /// [`ColdOption::None`] is an explicit no-op returning `0.0`.
    #[test]
    fn add_cold_hydrogen_none_is_noop() {
        let input = LeaprInput {
            alpha: vec![1.0],
            beta: vec![0.0],
            lat: false,
            arat: 1.0,
            nphon: 100,
            temperature_k: 296.0,
            continuous: ContinuousDist {
                delta_ev: 1e-3,
                rho: vec![0.0, 1.0],
                twt: 0.0,
                c: 0.0,
                tbeta: 1.0,
            },
            oscillators: vec![],
            constants: crate::leapr::vintage::PhysicalConstants::default(),
        };
        let mut ssm = SabMatrix::zeros(1, 1);
        let mut ssp = SabMatrix::zeros(1, 1);
        let r = add_cold_hydrogen(
            &mut ssm,
            &mut ssp,
            &input,
            ColdOption::None,
            &[1.0],
            0.5,
            300.0,
        );
        assert_eq!(r, 0.0);
    }
}
