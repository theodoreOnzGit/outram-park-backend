// SPDX-License-Identifier: GPL-3.0-only
//! Regression test: the default plume-shine integrator (petir's GSL QAGS)
//! against the SciPy QUADPACK reference, over the code-to-code fixture's
//! plume-shine cases.
//!
//! # Methodology
//!
//! **Cases.** The five plume-shine geometries of the pyDOSEIA fixture
//! (`tests/data/pydoseia_reference.csv`, groups `plume_shine_single`,
//! `plume_shine_long_term`, `plume_shine_met`), release height 30 m:
//! single plume at `(X1, Y, Z)` = (200, 0, 0) and (900, 25, 1.5) m; sector
//! averaged at `(X1, Z)` = (200, 0), (900, 1.5) and (400, 0) m. For each, every
//! non-zero gamma line of the fixture's nuclides (SYN-1 with its substring
//! match SYN-1D, SYN-3, SYN-6; synthetic tables in `tests/data/`; four lines
//! in all) and all six stability classes: **120 triple integrals**, each the kernel over
//! upstream's limits at upstream's tolerance (`epsabs = epsrel` = 1.49e-2
//! single plume, 1.49e-3 sector averaged; 50 sub-intervals per level).
//!
//! **Compared.** `plume_shine::petir_tplquad` (the default) against
//! `pydoseia::quadpack::tplquad` (the reference), value by value.
//!
//! **Pass criterion, derived from the requested tolerances, not from the
//! observed difference.** Each level asks for `|error| <= max(epsabs, epsrel
//! |value|)`. The kernel is positive, so an inner error integrated over the
//! next level's range `L` adds at most `epsabs L + epsrel |value|` there.
//! Summing the three levels, each integrator's nominal error is at most
//!
//! ```text
//! E = epsabs (Ly Lz + Lz + 1) + 3 epsrel |I|
//! ```
//!
//! with `Ly`, `Lz` the lengths of the `y` and `z` ranges. Two integrators each
//! within `E` of the truth differ by at most `2E`, and that is the bound
//! asserted. It rests on QUADPACK's error *estimates*, which are heuristics
//! (usually pessimistic) rather than guarantees.
//!
//! **What the criterion is worth, stated before the result.** pyDOSEIA's
//! `epsabs` is absolute and the integrals here are of order 1e-2 or smaller,
//! so `epsabs (Ly Lz)` exceeds `|I|` by orders of magnitude: `2E / |I|` is
//! printed per case. The derived bound is therefore **not informative**; it
//! can catch a NaN, a sign error or a gross failure, not a subtle one. That
//! is a property of upstream's tolerance, reported rather than tightened, and
//! it is why the stronger evidence is the second test below, which demands
//! bit-identity.
//!
//! # Results (2026-09-28)
//!
//! 120 integrals: petir QAGS and the SciPy QUADPACK port are **bit-identical in
//! all 120** (max relative difference 0), inside `2E` by construction. The
//! smallest `2E / |I|` over the cases is **716**: as predicted, the derived
//! bound is uninformative at pyDOSEIA's tolerances. The adaptive paths are
//! exercised: 265 812 one-dimensional QAGS calls against the 55 560 a run
//! with no subdivision anywhere would make, and **none** reports a non-`Ok`
//! status. The bit-exact check that can fail is
//! `petir_plume_shine_also_reproduces_the_fixture` in
//! `tests/pydoseia_code_to_code.rs`, which holds the petir path to the
//! fixture's own (zero) tolerances.

#![cfg(not(target_os = "android"))]

use buangkok::pydoseia::dispersion::StabilityClass;
use buangkok::pydoseia::plume_shine::{
    integration_limits_sector_averaged, integration_limits_single_plume, kernel_sector_averaged,
    kernel_single_plume, petir_tplquad, AirPhotonCoefficients, AttenuationTable, GammaLineTable,
    SECTOR_AVERAGED_EPS, SINGLE_PLUME_EPS,
};
use buangkok::pydoseia::quadpack::{tplquad, SCIPY_QUAD_LIMIT};
use uom::si::f64::Length;
use uom::si::length::meter;

const GAMMA: &str = include_str!("data/pydoseia_synthetic_gamma_lines.csv");
const ATT: &str = include_str!("data/pydoseia_synthetic_attenuation.csv");
const RELEASE_HEIGHT_M: f64 = 30.0;
const NUCLIDES: [&str; 3] = ["SYN-1", "SYN-3", "SYN-6"];

#[derive(Clone, Copy)]
enum Geometry {
    Single { x1: f64, y: f64, z: f64 },
    Sector { x1: f64, z: f64 },
}

const GEOMETRIES: [Geometry; 5] = [
    Geometry::Single {
        x1: 200.0,
        y: 0.0,
        z: 0.0,
    },
    Geometry::Single {
        x1: 900.0,
        y: 25.0,
        z: 1.5,
    },
    Geometry::Sector { x1: 200.0, z: 0.0 },
    Geometry::Sector { x1: 900.0, z: 1.5 },
    Geometry::Sector { x1: 400.0, z: 0.0 },
];

/// One triple integral: its label, limits, tolerance and kernel inputs.
struct Case {
    label: String,
    g: Geometry,
    s: StabilityClass,
    c: AirPhotonCoefficients,
    limits: [f64; 6],
    eps: f64,
}

impl Case {
    fn kernel(&self, x: f64, y: f64, z: f64) -> f64 {
        match self.g {
            Geometry::Single { x1, y: ry, z: rz } => {
                kernel_single_plume(self.s, self.c, x1, ry, rz, RELEASE_HEIGHT_M, x, y, z)
            }
            Geometry::Sector { x1, z: rz } => {
                kernel_sector_averaged(self.s, self.c, x1, rz, RELEASE_HEIGHT_M, x, y, z)
            }
        }
    }

    /// The derived nominal bound `2E` on the difference of two integrators.
    fn bound(&self, i: f64) -> f64 {
        let [z_lo, z_hi, y_lo, y_hi, _, _] = self.limits;
        let (ly, lz) = (y_hi - y_lo, z_hi - z_lo);
        2.0 * (self.eps * (ly * lz + lz + 1.0) + 3.0 * self.eps * i.abs())
    }
}

fn cases() -> Vec<Case> {
    let gamma = GammaLineTable::from_csv(GAMMA).unwrap();
    let att = AttenuationTable::from_csv(ATT).unwrap();
    let h = Length::new::<meter>(RELEASE_HEIGHT_M);
    let mut out = Vec::new();
    for g in GEOMETRIES {
        for n in NUCLIDES {
            for line in gamma.plume_shine_lines(n) {
                if line.energy_mev == 0.0 {
                    continue; // not integrated (see `line_integrals`)
                }
                let c = att.air_coefficients(line.energy_mev);
                for s in StabilityClass::ALL {
                    let (x1, limits, eps, tag) = match g {
                        Geometry::Single { x1, y, z } => (
                            x1,
                            integration_limits_single_plume(
                                s,
                                Length::new::<meter>(x1),
                                h,
                                c.mfp_m,
                            ),
                            SINGLE_PLUME_EPS,
                            format!("single ({x1}, {y}, {z})"),
                        ),
                        Geometry::Sector { x1, z } => (
                            x1,
                            integration_limits_sector_averaged(
                                s,
                                Length::new::<meter>(x1),
                                h,
                                c.mfp_m,
                            ),
                            SECTOR_AVERAGED_EPS,
                            format!("sector ({x1}, {z})"),
                        ),
                    };
                    let _ = x1;
                    out.push(Case {
                        label: format!("{tag} {n} {} MeV class {}", line.energy_mev, s.index()),
                        g,
                        s,
                        c,
                        limits,
                        eps,
                    });
                }
            }
        }
    }
    out
}

/// Petir QAGS against the SciPy QUADPACK reference, within the derived bound;
/// see the module docs for the methodology and the results.
#[test]
fn petir_qags_matches_the_quadpack_reference_within_the_requested_tolerance() {
    let cases = cases();
    let (mut worst_rel, mut min_bound_rel) = (0.0_f64, f64::INFINITY);
    let (mut identical, mut calls, mut uncertified) = (0usize, 0usize, 0usize);
    let mut failures = Vec::new();
    for case in &cases {
        let r = tplquad(
            |x, y, z| case.kernel(x, y, z),
            case.limits,
            case.eps,
            case.eps,
        );
        let p = petir_tplquad(
            |x, y, z| case.kernel(x, y, z),
            case.limits,
            case.eps,
            case.eps,
        );
        calls += p.calls;
        uncertified += p.uncertified_calls;
        let diff = (p.value - r).abs();
        let bound = case.bound(r);
        if p.value.to_bits() == r.to_bits() {
            identical += 1;
        }
        worst_rel = worst_rel.max(diff / r.abs());
        min_bound_rel = min_bound_rel.min(bound / r.abs());
        if !(diff <= bound) {
            failures.push(format!(
                "{}: petir {:e} vs reference {r:e}, |diff| {diff:e} > 2E {bound:e}",
                case.label, p.value
            ));
        }
    }
    println!(
        "{} integrals: {identical} bit-identical, max rel diff {worst_rel:e}; \
         smallest 2E/|I| {min_bound_rel:e}; petir QAGS calls {calls} \
         (no subdivision anywhere would be {}), uncertified {uncertified}",
        cases.len(),
        cases.len() * (1 + 21 + 21 * 21)
    );
    assert_eq!(cases.len(), 120);
    assert!(failures.is_empty(), "{failures:#?}");
}

/// The measurement that decided QAGS over plain QAG: petir's `qag` (GSL
/// `gsl_integration_qag`, 21-point rule, no extrapolation), nested the same
/// way, on the same 150 integrals at the same tolerances and limit.
///
/// `qag` returns no estimate when it fails, so a failed inner call is
/// counted and contributes `NaN`, which the comparison then flags.
///
/// # Results (2026-09-28)
///
/// Plain `qag` **meets the requested tolerance on every fixture case**: none
/// of its 265 812 calls fails, all 120 integrals are within `2E` of the
/// reference, 106 of 120 are bit-identical and the worst relative difference
/// is 4.3e-16 (summation order: `qag` keeps a running area, QAGS re-sums the
/// sub-intervals). So on these cases the measurement alone does not force
/// QAGS. It was chosen anyway, for reasons recorded in
/// `docs/pydoseia-code-to-code.md`: bit-identity with upstream's algorithm,
/// the kernel's `1/r^2` point singularity at a receptor inside the cloud,
/// and `qag` discarding its estimate on failure. The assertions pin this
/// measurement; if they fail, re-measure and revisit that decision.
#[test]
fn plain_qag_measurement_on_the_fixture_cases() {
    use std::cell::Cell;

    let cases = cases();
    let (mut worst_rel, mut identical, mut nan_cases) = (0.0_f64, 0usize, 0usize);
    let (mut within, mut failed_calls, mut calls) = (0usize, 0usize, 0usize);
    for case in &cases {
        let [z_lo, z_hi, y_lo, y_hi, x_lo, x_hi] = case.limits;
        let fails = Cell::new(0usize);
        let n = Cell::new(0usize);
        let eps = case.eps;
        let v = qag_level(
            |z| {
                qag_level(
                    |y| qag_level(|x| case.kernel(x, y, z), x_lo, x_hi, eps, &n, &fails),
                    y_lo,
                    y_hi,
                    eps,
                    &n,
                    &fails,
                )
            },
            z_lo,
            z_hi,
            eps,
            &n,
            &fails,
        );
        let r = tplquad(
            |x, y, z| case.kernel(x, y, z),
            case.limits,
            case.eps,
            case.eps,
        );
        failed_calls += fails.get();
        calls += n.get();
        if v.is_nan() {
            nan_cases += 1;
            continue;
        }
        if v.to_bits() == r.to_bits() {
            identical += 1;
        }
        let diff = (v - r).abs();
        worst_rel = worst_rel.max(diff / r.abs());
        if diff <= case.bound(r) {
            within += 1;
        }
    }
    println!(
        "plain qag: {} integrals, {nan_cases} lost to a failed call ({failed_calls} of \
         {calls} calls failed); of the rest {identical} bit-identical with the reference, \
         {within} within 2E, max rel diff {worst_rel:e}",
        cases.len()
    );
    assert_eq!(cases.len(), 120);
    assert_eq!(failed_calls, 0);
    assert_eq!(nan_cases, 0);
    assert_eq!(within, cases.len());
}

/// One level of the nested plain-`qag` integral, counting calls and failures.
fn qag_level<G: Fn(f64) -> f64>(
    g: G,
    a: f64,
    b: f64,
    eps: f64,
    n: &std::cell::Cell<usize>,
    fails: &std::cell::Cell<usize>,
) -> f64 {
    use petir::integration::{qag, QkRule};
    n.set(n.get() + 1);
    match qag(QkRule::Qk21, g, a, b, eps, eps, SCIPY_QUAD_LIMIT) {
        Ok(i) => i.value,
        Err(_) => {
            fails.set(fails.get() + 1);
            f64::NAN
        }
    }
}
