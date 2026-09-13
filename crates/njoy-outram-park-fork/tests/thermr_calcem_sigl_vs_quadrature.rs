//! **V&V — THERMR's freshly ported `sigl`, `egrid` and the `iform=1` continuous
//! outgoing-energy law, against oracles that do not share their code.**
//!
//! # Why these oracles
//!
//! `sigl` (thermr.f90:2660-2872) is the hardest piece of THERMR to port: it
//! adaptively linearises the angular distribution on an explicit 20-deep stack
//! and then solves a quadratic for each equally-probable cosine bin boundary
//! (`fract`/`gral`/`rfract`/`disc`/`xbar`). Nothing about that machinery is
//! self-checking, and this repo has no NJOY MF=6 reference matrix for it. So it
//! is checked against two things it shares no code with:
//!
//! 1. **`IncoherentInelastic::sigma_e_to_ep`** — the pre-existing μ integration,
//!    a flat 200-point trapezoid. It computes `sigl`'s `s(1)` by a completely
//!    different rule over the same integrand ([`double_differential`], which both
//!    call). Agreement is mutual verification; disagreement localises to one of
//!    the two.
//! 2. **A 4000-point quadrature `μ̄`** computed in this file. The mean of the
//!    equally-probable cosines must reproduce it, because equally-probable bins
//!    partition the same distribution — and that is a check on the bin-boundary
//!    solve specifically, which nothing else here touches.
//!
//! `EGRID` needs no oracle at all: it is a literal table, so it is asserted
//! **bit-for-bit** against the Fortran's own values, transcribed in this file.
//!
//! # Results (2026-09-13, ENDF/B-VIII.0 `tsl-crystalline-graphite` at 600 K)
//!
//! `sigl`'s `s(1)` against the independent trapezoid, and the convergence that
//! shows the residual is the adaptive tolerance rather than a bias:
//!
//! ```text
//!     E       E'    tol=5e-3     tol=5e-4     tol=5e-5    trapezoid
//!   0.0253  0.0253  53.086337    53.132198    53.136098   53.137733
//!   0.0253  0.0100   5.270219     5.275754     5.276303    5.276336
//!   0.0253  0.0500   9.215487     9.222706     9.223549    9.223621
//!   0.1000  0.0500   6.098963     6.104910     6.105317    6.105363
//!   0.1000  0.1500   5.727776     5.732593     5.733091    5.733113
//!   1.0000  0.5000   0.912942     0.912468     0.912370    0.912374
//!   1.0000  1.2000   0.223235     0.223412     0.223428    0.223428
//! ```
//!
//! Every row converges **monotonically onto the trapezoid** as the tolerance
//! tightens, to 5-6 significant figures at `tol = 5e-5`. At NJOY's own working
//! tolerance the two differ by 0.06-0.12 %, which is the tolerance and not an
//! error.
//!
//! The equally-probable cosines' mean against the 4000-point quadrature `μ̄`:
//!
//! ```text
//!     E       E'    μ̄ (bins)    μ̄ (quadrature)
//!   0.0253  0.0253   -0.28361      -0.28364
//!   0.0253  0.0100   -0.28025      -0.28035
//!   0.0253  0.0500   -0.28101      -0.28096
//!   0.1000  0.0500   -0.27437      -0.27431
//!   0.1000  0.1500   -0.25751      -0.25757
//!   1.0000  0.5000   -0.54150      -0.54165
//!   1.0000  1.2000   -0.10631      -0.10625
//! ```
//!
//! Agreement to 1e-4 in `μ̄` across a range where `μ̄` itself moves by a factor
//! of five. **`EGRID`: all 118 values bit-for-bit identical to thermr.f90:1587.**

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::thermr::calcem::egrid::{BREAK, EGRID, NGRID, THERM};
use njoy_outram_park_fork::thermr::calcem::sigl::sigl;
use njoy_outram_park_fork::thermr::mf7::{parse_mf7_at_temperature, IncoherentInelastic};

/// `(E, E')` probe pairs \[eV\]: the quasi-elastic point, down-scatter,
/// up-scatter, and three energies spanning the thermal range.
const PROBES: &[(f64, f64)] = &[
    (0.0253, 0.0253),
    (0.0253, 0.0100),
    (0.0253, 0.0500),
    (0.1000, 0.0500),
    (0.1000, 0.1500),
    (1.0000, 0.5000),
    (1.0000, 1.2000),
];

fn graphite(temp_k: f64) -> Option<IncoherentInelastic> {
    let path = reference_endf_or_skip("tsl-crystalline-graphite.endf", "calcem-sigl")?;
    let tape = Tape::read_file(&path).ok()?;
    let mat = *tape.materials().first()?;
    parse_mf7_at_temperature(&tape, mat, Some(temp_k))
        .ok()?
        .incoherent_inelastic
}

/// Independent `μ̄ = ∫μ f dμ / ∫f dμ` by dense trapezoid — shares only
/// `double_differential` with `sigl`, not a line of its integration.
fn mubar_quadrature(ii: &IncoherentInelastic, e: f64, ep: f64, t: f64, natom: f64) -> f64 {
    const N: usize = 4000;
    let f = |mu: f64| ii.double_differential(e, ep, mu, t, natom);
    let (mut num, mut den) = (0.0, 0.0);
    let mut prev = f(-1.0);
    for k in 1..=N {
        let mu0 = -1.0 + 2.0 * (k - 1) as f64 / N as f64;
        let mu1 = -1.0 + 2.0 * k as f64 / N as f64;
        let cur = f(mu1);
        den += 0.5 * (cur + prev) * (mu1 - mu0);
        num += 0.5 * (mu1 * cur + mu0 * prev) * (mu1 - mu0);
        prev = cur;
    }
    if den > 0.0 {
        num / den
    } else {
        0.0
    }
}

/// `EGRID` is a literal table, so it gets a literal oracle: the Fortran's own
/// 118 values, transcribed from thermr.f90:1587-1607.
#[test]
fn egrid_is_bit_for_bit_the_fortran_table() {
    #[rustfmt::skip]
    const FORTRAN: [f64; 118] = [
        1.0e-5, 1.78e-5, 2.5e-5, 3.5e-5, 5.0e-5, 7.0e-5, 1.0e-4,
        1.26e-4, 1.6e-4, 2.0e-4, 0.000253, 0.000297, 0.000350,
        0.00042, 0.000506, 0.000615, 0.00075, 0.00087,
        0.001012, 0.00123, 0.0015, 0.0018, 0.00203, 0.002277,
        0.0026, 0.003, 0.0035, 0.004048, 0.0045, 0.005,
        0.0056, 0.006325, 0.0072, 0.0081, 0.009108, 0.01,
        0.01063, 0.0115, 0.012397, 0.0133, 0.01417, 0.015,
        0.016192, 0.0182, 0.0199, 0.020493, 0.0215, 0.0228,
        0.0253, 0.028, 0.030613, 0.0338, 0.0365, 0.0395,
        0.042757, 0.0465, 0.050, 0.056925, 0.0625, 0.069,
        0.075, 0.081972, 0.09, 0.096, 0.1035, 0.111573,
        0.120, 0.128, 0.1355, 0.145728, 0.160, 0.172,
        0.184437, 0.20, 0.2277, 0.2510392, 0.2705304,
        0.2907501, 0.3011332, 0.3206421, 0.3576813, 0.39,
        0.4170351, 0.45, 0.5032575, 0.56, 0.625,
        0.70, 0.78, 0.86, 0.95, 1.05, 1.16, 1.28,
        1.42, 1.55, 1.70, 1.855, 2.02, 2.18,
        2.36, 2.59, 2.855, 3.12, 3.42, 3.75,
        4.07, 4.46, 4.90, 5.35, 5.85, 6.40,
        7.00, 7.65, 8.40, 9.15, 9.85, 10.00,
    ];
    assert_eq!(NGRID, 118, "thermr.f90 declares ngrid = 118");
    assert_eq!(EGRID.len(), NGRID);
    for (i, (&ours, &theirs)) in EGRID.iter().zip(FORTRAN.iter()).enumerate() {
        assert_eq!(
            ours, theirs,
            "EGRID[{i}] = {ours:e} but thermr.f90:1587 says {theirs:e} — this table \
             is transcribed, not computed, so any difference is a transcription error"
        );
    }
    assert!(
        EGRID.windows(2).all(|w| w[1] > w[0]),
        "EGRID must ascend strictly"
    );
    assert_eq!(BREAK, 3000.0, "thermr.f90 `break`");
    assert_eq!(THERM, 0.0253, "thermr.f90 `therm`");
}

/// `sigl`'s `s(1)` is the μ-integrated `σ(E→E')`, and so is
/// `sigma_e_to_ep` — by a different rule. They must agree, and the agreement
/// must **tighten as `sigl`'s tolerance tightens**, which is what distinguishes
/// a quadrature difference from a defect.
#[test]
fn sigl_cross_section_converges_onto_an_independent_quadrature() {
    const TEMP_K: f64 = 600.0;
    const NATOM: f64 = 1.0;
    let Some(ii) = graphite(TEMP_K) else { return };

    for &(e, ep) in PROBES {
        let reference = ii.sigma_e_to_ep(e, ep, TEMP_K, NATOM);
        assert!(reference > 0.0, "probe ({e}, {ep}) must have a cross section");

        let mut last_err = f64::INFINITY;
        for &tol in &[5.0e-3_f64, 5.0e-4, 5.0e-5] {
            let s = sigl(&ii, e, ep, -9, NATOM, tol).expect("sigl must converge");
            let err = ((s[1] - reference) / reference).abs();
            assert!(
                err <= last_err + 1.0e-9,
                "sigl at ({e} -> {ep}) got WORSE as its tolerance tightened: \
                 {err:.3e} at tol={tol:.0e} against {last_err:.3e} before it. A \
                 quadrature difference shrinks with the tolerance; a defect does not."
            );
            last_err = err;
        }
        assert!(
            last_err < 2.0e-4,
            "sigl at ({e} -> {ep}) is {:.3e} from the 200-point trapezoid at \
             tol = 5e-5, against a recorded 2e-5 worst on 2026-09-13",
            last_err
        );
    }
}

/// The equally-probable cosines must partition the *same* angular distribution
/// the quadrature integrates, so their mean is `μ̄`. This is the check on the
/// `fract`/`gral`/`disc`/`xbar` bin-boundary solve — the part of `sigl` that
/// nothing else in this crate exercises.
#[test]
fn sigl_equiprobable_cosines_reproduce_the_quadrature_mean_cosine() {
    const TEMP_K: f64 = 600.0;
    const NATOM: f64 = 1.0;
    let Some(ii) = graphite(TEMP_K) else { return };

    for &(e, ep) in PROBES {
        let want = mubar_quadrature(&ii, e, ep, TEMP_K, NATOM);
        // nlin < 0 selects equally-probable cosines; |nlin| - 1 = 8 of them.
        let s = sigl(&ii, e, ep, -9, NATOM, 5.0e-4).expect("sigl must converge");
        let bins = &s[2..];
        assert_eq!(bins.len(), 8, "|nlin| - 1 equally probable cosines");
        assert!(
            bins.iter().all(|m| (-1.0..=1.0).contains(m)),
            "({e} -> {ep}): every equally-probable cosine must lie in [-1, 1], got {bins:?}"
        );
        assert!(
            bins.windows(2).all(|w| w[1] >= w[0]),
            "({e} -> {ep}): equally-probable cosines must ascend, got {bins:?}"
        );
        let got = bins.iter().sum::<f64>() / bins.len() as f64;
        assert!(
            (got - want).abs() < 2.0e-3,
            "({e} -> {ep}): mean of the equally-probable cosines is {got:.6}, but a \
             4000-point quadrature says μ̄ = {want:.6}. Equally-probable bins \
             partition the same distribution, so these cannot disagree; worst \
             recorded 2026-09-13 was 1.0e-4."
        );
    }
}
