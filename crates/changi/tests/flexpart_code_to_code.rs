// SPDX-License-Identifier: GPL-3.0
//
// Code-to-code verification of `changi::flexpart` against upstream FLEXPART
// v10.4 (commit 3d7eebf, GPL-3.0-or-later).
//
// Uses only changi + approx, and reads its fixtures through `include_str!`, so
// there is no filesystem access at run time and no target gate is needed.

//! # V&V: FLEXPART surface-layer and deposition kernels, code-to-code
//!
//! ## Methodology
//!
//! Every reference value here is produced by **compiling and running the
//! upstream FLEXPART Fortran itself**. `dev/flexpart_reference.f90` links the
//! upstream routines *verbatim* — `par_mod.f90`, `ew.f90`,
//! `dynamic_viscosity.f90`, `psim.f90`, `psih.f90`, `scalev.f90`, `raerod.f90`,
//! `obukhov.f90`, `erf.f90`, `part0.f90`, none of them edited — and sweeps each
//! over a grid chosen to straddle its branches and guards. The only non-upstream
//! Fortran is a shim supplying the single integer parameter `obukhov.f90` takes
//! from `class_gribfile`, which would otherwise drag in ecCodes; being a
//! compile-time parameter, it changes nothing about the routine under test.
//!
//! ## The precision question, which governs everything here
//!
//! **FLEXPART's makefile passes no `-fdefault-real-8`.** Its default `real` is
//! therefore `real(4)` — single precision. `par_mod.f90` writes
//! `pi = 3.14159265` and stores `3.14159274…`. A `f64` port cannot agree with a
//! stock FLEXPART build to better than about `1e-7`, however perfect the
//! translation.
//!
//! Comparing against only the shipped build would therefore conflate two
//! completely different things: a translation error, and upstream's storage
//! format. So `dev/build_reference.sh` builds the **same driver twice** —
//! as-shipped `real(4)`, and `-fdefault-real-8` — and this test checks both:
//!
//! - **against `real8`** — agreement to near machine precision proves the
//!   *translation* is right, because the only remaining difference is the order
//!   of floating-point operations;
//! - **against `real4`** — the residual is then a *measurement of FLEXPART's own
//!   precision*, reported rather than assumed.
//!
//! ## Results (2026-09-15, upstream `3d7eebf`, 1 756 cases per fixture)
//!
//! Measured by this test; re-print with `--nocapture`. Full table and analysis
//! in `docs/flexpart-code-to-code.md`.
//!
//! ## What this does NOT establish
//!
//! **Verification, not validation.** It shows the Rust computes what the Fortran
//! computes. It says nothing about whether FLEXPART's parameterisations
//! reproduce measured atmospheric dispersion — that needs field data and is not
//! claimed. Per the crate scope limit, nothing here supports emergency response
//! or dose assessment for real populations.

use changi::prelude::*;

/// Upstream reference, built as FLEXPART actually ships (`real(4)`).
const FIXTURE_REAL4: &str = include_str!("data/flexpart_reference_real4.csv");
/// Upstream reference, same algorithm at `-fdefault-real-8`.
const FIXTURE_REAL8: &str = include_str!("data/flexpart_reference_real8.csv");

/// The `akm`/`bkm` hybrid coefficients the Fortran driver uses for the ECMWF
/// branch of `obukhov`. Mirrored here so the Rust call reproduces it exactly.
const AKM: [f64; 2] = [0.0, 20.0];
const BKM: [f64; 2] = [1.0, 0.997];

struct Case {
    function: String,
    args: Vec<f64>,
    expected: f64,
}

fn parse(fixture: &str) -> Vec<Case> {
    fixture
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty() && !l.starts_with("function,"))
        .map(|l| {
            let mut f = l.split(',');
            let function = f.next().expect("function column").trim().to_string();
            let arg_field = f.next().expect("args column");
            let expected = f.next().expect("expected column");
            let args = arg_field
                .split(';')
                .map(|a| a.trim().parse::<f64>().expect("f64 arg"))
                .collect();
            Case {
                function,
                args,
                expected: expected.trim().parse::<f64>().expect("f64 expected"),
            }
        })
        .collect()
}

/// Drive the Rust port for one fixture row.
///
/// Returns `None` for a row this test does not evaluate, so an unknown function
/// name is a visible gap rather than a silent pass.
fn evaluate(name: &str, a: &[f64]) -> Option<f64> {
    let v = match name {
        "ew" => ew_kelvin(a[0]),
        "viscosity" => viscosity_kelvin(a[0]),
        "psim" => psim(a[0], a[1]),
        "psih" => psih(a[0], a[1]),
        "scalev" => scalev(a[0], a[1], a[2], a[3]),
        "obukhov.ncep" => obukhov(
            a[0],
            a[1],
            a[2],
            a[3],
            a[4],
            a[5],
            a[6],
            MetDataFormat::Ncep,
        ),
        "obukhov.ecmwf" => obukhov(
            a[0],
            a[1],
            a[2],
            a[3],
            a[4],
            a[5],
            a[6],
            MetDataFormat::Ecmwf { akm: AKM, bkm: BKM },
        ),
        "raerod" => raerod(a[0], a[1], a[2]),
        "part0.fract" | "part0.schmi" | "part0.vsh" => {
            let bins = part0(a[0], a[1], a[2]);
            let idx = a[3] as usize - 1; // Fortran bins are 1-based
            match name {
                "part0.fract" => bins.mass_fraction[idx],
                "part0.schmi" => bins.schmidt_factor[idx],
                _ => bins.settling_velocity[idx],
            }
        }
        "part0.cun_scalar" => part0(a[0], a[1], a[2]).upstream_scalar_cunningham(),
        _ => return None,
    };
    Some(v)
}

fn deviation(got: f64, want: f64) -> f64 {
    if want == 0.0 {
        got.abs()
    } else {
        (got - want).abs() / want.abs()
    }
}

/// Check one function group against one fixture, returning the observed maximum
/// relative deviation so the test can print a real measured number (workspace
/// V&V rule: record results, not just methodology).
fn check(fixture: &str, label: &str, name: &str, tol: f64, abs_floor: f64) -> f64 {
    let all = parse(fixture);
    let group: Vec<&Case> = all.iter().filter(|c| c.function == name).collect();
    assert!(
        !group.is_empty(),
        "no `{name}` rows in the {label} fixture — regenerate with dev/build_reference.sh"
    );

    let mut worst = 0.0_f64;
    let mut worst_args: Vec<f64> = Vec::new();
    for c in &group {
        let got = evaluate(&c.function, &c.args)
            .unwrap_or_else(|| panic!("no evaluator wired for `{}`", c.function));
        let dev = deviation(got, c.expected);
        // Mixed criterion: a case passes on EITHER the relative bound or the
        // absolute floor. The floor matters where the routine's intermediate
        // terms are O(1) but cancel to near zero — there the reference's own
        // arithmetic has no significant digits left, so a relative test would be
        // testing its rounding, not our translation. See the group docs.
        let abs_dev = (got - c.expected).abs();
        assert!(
            dev <= tol || abs_dev <= abs_floor,
            "{label}/{name}{:?}: port {got:e} vs FLEXPART {:e} \
             (rel dev {dev:e} > tol {tol:e}, abs dev {abs_dev:e} > floor {abs_floor:e})",
            c.args,
            c.expected
        );
        if dev > worst && abs_dev > abs_floor {
            worst = dev;
            worst_args = c.args.clone();
        }
    }
    println!(
        "{label:>5} {name:<18} {} cases, max_rel_dev = {worst:e} (tol {tol:e}) at {worst_args:?}",
        group.len()
    );
    worst
}

/// Assert a group against BOTH references at once.
///
/// `tol8` is the translation bound; `tol4` is the wider bound that FLEXPART's
/// single precision imposes.
fn check_both(name: &str, tol8: f64, tol4: f64) {
    check_both_with_floor(name, tol8, tol4, 0.0, 0.0);
}

/// As [`check_both`], with explicit absolute floors for groups whose upstream
/// arithmetic cancels.
fn check_both_with_floor(name: &str, tol8: f64, tol4: f64, floor8: f64, floor4: f64) {
    check(FIXTURE_REAL8, "real8", name, tol8, floor8);
    check(FIXTURE_REAL4, "real4", name, tol4, floor4);
}

// ── thermodynamics ───────────────────────────────────────────────────────────

/// Saturation vapour pressure (Goff–Gratch) over 173–373 K.
#[test]
fn ew_matches_flexpart() {
    check_both("ew", 1e-13, 1e-5);
}

/// Dynamic viscosity of air (Sutherland) over 200–350 K.
#[test]
fn viscosity_matches_flexpart() {
    check_both("viscosity", 1e-14, 1e-6);
}

// ── Monin–Obukhov similarity ─────────────────────────────────────────────────

/// Momentum stability function across both stability branches.
///
/// # Why this group carries an absolute floor
///
/// Near neutral (`|zeta| -> 0`) `psi_m` is a difference of O(1) terms —
/// `ln(a1 a2)`, `2 atan(x)` and `pi/2` — that cancels to near zero. At
/// `z = 0.01 m`, `L = -10000 m` the terms are O(1.57) and the result is
/// `3.75e-6`: a cancellation ratio of **4.19e5**. In `real(4)` that puts the
/// noise floor at `eps_f32 x 4.19e5 = 5.0e-2` relative, and FLEXPART's answer
/// there is exactly `2^-18` — fully quantised, with no significant digits left.
/// Asserting a relative bound against that would be testing gfortran's rounding,
/// not this port.
///
/// So the `real4` check passes on either a `1e-5` relative bound or a `1e-6`
/// absolute one. Since the intermediate terms are O(1), an absolute `1e-6` is
/// still a *relative* `1e-6` on anything of normal magnitude — it does not
/// weaken the test where the arithmetic is well conditioned. The `real8` check
/// keeps its tight bound and passes, which is the statement that matters.
#[test]
fn psim_matches_flexpart() {
    check_both_with_floor("psim", 1e-13, 1e-5, 1e-15, 1e-6);
}

/// Heat stability function across both branches, including the near-zero-`L`
/// nudge and the far-field cutoff.
///
/// Carries an absolute floor for the same reason as [`psim_matches_flexpart`],
/// but a larger one, because `psi_h` cancels against a *bigger* constant.
///
/// On the stable branch the terms are
/// `-(1 + 0.667 zeta)^{3/2}`, `-b(zeta - c/d)exp(-d zeta)`, `-bc/d` and `+1`,
/// and the third is the fixed constant `b c / d = 0.667 x 5 / 0.35 = 9.5286`.
/// At `z = 0.01 m`, `L = 1 m` those sum to `-4.996e-2` from a largest term of
/// `9.5286` — a cancellation ratio of **191**, putting the `real(4)` absolute
/// noise floor at `eps_f32 x 9.5286 = 1.14e-6`. The measured disagreement there
/// is `1.03e-6`, i.e. inside the floor.
///
/// The floor is set to `5e-6`, a small safety factor over that bound to cover
/// the several rounding steps involved. It is derived from the term scale, not
/// chosen to make the test pass: on the unstable branch, where the terms are
/// O(1), the same floor is 5e-6 absolute on an O(1) quantity and so still
/// catches any real error, which a wrong constant would make O(0.1).
#[test]
fn psih_matches_flexpart() {
    check_both_with_floor("psih", 1e-13, 1e-5, 1e-15, 5e-6);
}

/// Friction velocity, including the zero-stress case.
#[test]
fn scalev_matches_flexpart() {
    check_both("scalev", 1e-13, 1e-5);
}

/// Obukhov length, NCEP path — caller-supplied level pressure.
///
/// Covers the `ustar <= 0` guard, the zero-heat-flux `9999` sentinel and the
/// `±9999` clamp.
#[test]
fn obukhov_ncep_matches_flexpart() {
    check_both("obukhov.ncep", 1e-13, 1e-5);
}

/// Obukhov length, ECMWF path — level pressure rebuilt from hybrid coefficients.
#[test]
fn obukhov_ecmwf_matches_flexpart() {
    check_both("obukhov.ecmwf", 1e-13, 1e-5);
}

/// Aerodynamic resistance over roughness lengths spanning 1e-4 to 1 m.
#[test]
fn raerod_matches_flexpart() {
    check_both("raerod", 1e-13, 1e-5);
}

// ── aerosol size distribution ────────────────────────────────────────────────

/// Per-bin mass fractions from the lognormal distribution.
///
/// Tolerance is looser than the other groups **by design, not by accident**:
/// this is the one quantity where the port deliberately differs from upstream.
/// `part0` needs `erf`, and the port calls `petir`'s GSL-derived implementation
/// instead of porting FLEXPART's own Numerical-Recipes-style `erf.f90`, per the
/// workspace "reuse before porting" rule. The two `erf` implementations differ,
/// so the bound here measures that substitution rather than a translation error.
/// The size of that difference is reported in `docs/flexpart-code-to-code.md`.
#[test]
fn part0_mass_fraction_matches_flexpart() {
    check_both("part0.fract", 1e-6, 1e-4);
}

/// Per-bin Schmidt-number factor `Sc^(-2/3)`. Independent of `erf`, so it holds
/// the tight translation bound.
#[test]
fn part0_schmidt_matches_flexpart() {
    check_both("part0.schmi", 1e-13, 1e-5);
}

/// Per-bin gravitational settling velocity. Independent of `erf`.
#[test]
fn part0_settling_matches_flexpart() {
    check_both("part0.vsh", 1e-13, 1e-5);
}

/// Upstream's scalar `cun` output — the Cunningham factor of the **largest**
/// bin only, which is what `part0.f90` actually returns.
///
/// Pinning this is what makes the quirk documented on
/// [`AerosolBins::cunningham`] falsifiable rather than a claim.
#[test]
fn part0_scalar_cunningham_matches_flexpart() {
    check_both("part0.cun_scalar", 1e-13, 1e-5);
}

// ── radioactive decay ────────────────────────────────────────────────────────

/// FLEXPART's half-life → decay-constant conversion and its exponential.
///
/// There is no Fortran fixture for this: upstream computes it inline in
/// `readreleases.f90:317` and `timemanager.f90:275` rather than in a callable
/// routine, so there is nothing to link against. It is checked here against the
/// upstream *expressions* instead, which is stated plainly rather than dressed
/// up as code-to-code.
#[test]
#[allow(clippy::approx_constant)] // 0.693147 is upstream's truncated ln 2, on purpose
fn decay_reproduces_upstream_expressions() {
    // readreleases.f90:317 — decay(i) = 0.693147/halflife
    let t_half = 2.4e5_f64; // Cs-137-ish, seconds
    let lambda = decay_constant(t_half);
    assert!((lambda - 0.693_147 / t_half).abs() < 1e-18);

    // Upstream's truncated ln2 differs from the exact value by ~2.6e-7.
    let exact = decay_constant_exact(t_half);
    let rel = (lambda - exact).abs() / exact;
    assert!(
        (2.0e-7..4.0e-7).contains(&rel),
        "upstream's truncated ln2 should differ from exact by ~2.6e-7, got {rel:e}"
    );

    // timemanager.f90:275 — exp(-1.*outstep*decay(ks))
    let dt = 3600.0;
    assert!((surviving_fraction(lambda, dt) - (-dt * lambda).exp()).abs() < 1e-18);

    // Stable species: decay(ks) > 0 guard means no decay at all.
    assert_eq!(surviving_fraction(0.0, 1e9), 1.0);
    assert_eq!(decayed(5.0, 0.0, 1e9), 5.0);

    // Half a half-life's worth of decay halves the amount.
    let half = decayed(1.0, decay_constant_exact(t_half), t_half);
    assert!(
        (half - 0.5).abs() < 1e-12,
        "one half-life should halve, got {half}"
    );
}
