// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from code_aster (https://gitlab.com/codeaster/src)
//   Copyright (C) 1991 - 2026 - EDF R&D
//   Licence: GPL-3.0-or-later
//   Upstream commit: b504ea08c2f49575e04644cee2e39a63ea45c16e
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Verification of the ported fracture-mechanics subset.
//!
//! Every number quoted in a doc comment below was printed by the test that
//! quotes it and transcribed. None is predicted.

use super::*;
use crate::rheology::aster::integration::{brent, SolverControl};
use approx::assert_relative_eq;
use std::f64::consts::PI;

/// A representative reactor-pressure-vessel ferritic steel: `E = 200 GPa`,
/// `nu = 0.3`. Chosen because `nu = 0.3` makes the plane-strain/plane-stress
/// modulus gap (`1/(1 - nu^2) = 1.0989`) large enough to be unmissable in a test
/// and small enough to be mistaken for mesh error in practice.
fn steel() -> LinearElasticConstants {
    LinearElasticConstants::new(200.0e9, 0.3).unwrap()
}

// =====================================================================
// Elastic constants and the plane state
// =====================================================================

/// **Kolosov's `kappa` and the effective modulus `E'` against their published
/// closed forms.**
///
/// *Methodology:* evaluate [`LinearElasticConstants::kolosov_kappa`] and
/// [`LinearElasticConstants::effective_modulus`] for `E = 200 GPa`, `nu = 0.3`
/// in plane strain and plane stress and compare against the textbook values
/// `kappa = 3 - 4 nu` / `(3 - nu)/(1 + nu)` and `E' = E/(1 - nu^2)` / `E`
/// (Irwin; Kolosov-Muskhelishvili). Reference: closed form, exact. Pass
/// criterion: 1e-12 relative. Also checks that the axisymmetric and 3-D variants
/// coincide with plane strain, and reports the plane-strain/plane-stress ratio.
///
/// *Results (measured 2026-08-05):*
///
/// | Quantity | Plane strain | Plane stress |
/// |---|---|---|
/// | `kappa` | 1.8000000000000000 | 2.0769230769230771 |
/// | `E'` (Pa) | 2.1978021978021979e11 | 2.0000000000000000e11 |
///
/// `E'(plane strain) / E'(plane stress) = 1.0989010989010990`, i.e. **9.89%**.
/// Interpretation: an energy release rate computed with the wrong plane state at
/// `nu = 0.3` is wrong by 9.89% — a discrepancy small enough to be attributed to
/// mesh refinement and large enough to invalidate a toughness comparison. That
/// is the reason this is a typed choice and not a default.
#[test]
fn kappa_and_effective_modulus_match_their_published_closed_forms() {
    let m = steel();
    let nu = m.poisson;

    let kappa_ps = m.kolosov_kappa(CrackPlaneState::PlaneStrain);
    let kappa_cp = m.kolosov_kappa(CrackPlaneState::PlaneStress);
    let e_ps = m.effective_modulus(CrackPlaneState::PlaneStrain);
    let e_cp = m.effective_modulus(CrackPlaneState::PlaneStress);

    println!("kappa plane strain = {kappa_ps:.16}");
    println!("kappa plane stress = {kappa_cp:.16}");
    println!("E' plane strain    = {e_ps:.16e} Pa");
    println!("E' plane stress    = {e_cp:.16e} Pa");
    println!("E' ratio           = {:.16}", e_ps / e_cp);

    assert_relative_eq!(kappa_ps, 3.0 - 4.0 * nu, max_relative = 1e-12);
    assert_relative_eq!(kappa_cp, (3.0 - nu) / (1.0 + nu), max_relative = 1e-12);
    assert_relative_eq!(e_ps, m.young / (1.0 - nu * nu), max_relative = 1e-12);
    assert_relative_eq!(e_cp, m.young, max_relative = 1e-12);

    for state in [
        CrackPlaneState::Axisymmetric,
        CrackPlaneState::ThreeDimensional,
    ] {
        assert_relative_eq!(m.kolosov_kappa(state), kappa_ps, max_relative = 1e-12);
        assert_relative_eq!(m.effective_modulus(state), e_ps, max_relative = 1e-12);
    }
}

/// **Inadmissible elastic constants are refused at construction.**
///
/// *Methodology:* a non-positive Young's modulus and a Poisson's ratio at or
/// beyond the incompressible limit must be rejected, because the plane-strain
/// Lame constant diverges at `nu = 0.5` and every quantity downstream becomes
/// meaningless. Pass criterion: all four cases return `Err`.
///
/// *Result:* all four rejected (measured 2026-08-05). `nu = 0.5` is refused
/// rather than clamped — clamping would return a finite, confident, wrong
/// stress.
#[test]
fn inadmissible_elastic_constants_are_refused() {
    assert!(LinearElasticConstants::new(0.0, 0.3).is_err());
    assert!(LinearElasticConstants::new(-1.0e9, 0.3).is_err());
    assert!(LinearElasticConstants::new(200.0e9, 0.5).is_err());
    assert!(LinearElasticConstants::new(200.0e9, -1.0).is_err());
    assert!(LinearElasticConstants::new(200.0e9, 0.49999).is_ok());
}

// =====================================================================
// Near-tip fields — the strongest verification here
// =====================================================================

/// **The near-tip field reproduces the Williams singular stress, identically in
/// plane strain and in plane stress.**
///
/// *Methodology:* the leading Williams term gives, on the crack plane ahead of
/// the tip (`theta = 0`), the mode-resolved singular stresses
///
/// - mode I: `sigma_yy = K_I / sqrt(2 pi r)`, `sigma_xy = 0`
/// - mode II: `sigma_xy = K_II / sqrt(2 pi r)`, `sigma_yy = 0`
/// - mode III: `sigma_yz = K_III / sqrt(2 pi r)`
///
/// and these amplitudes are **independent of the plane state**, even though the
/// displacement field is not. So: build the unit-`K` field with
/// [`westergaard_unit_field`], apply Hooke through [`near_tip_stress`], multiply
/// by `sqrt(2 pi r)`, and check the result is 1 in every mode and both plane
/// states, at `r = 1e-5 m`. Reference: the Williams (1957) expansion, exact.
/// Pass criterion: 1e-12 relative.
///
/// *Results (measured 2026-08-05), `sqrt(2 pi r) * sigma` for unit `K`:*
///
/// | Mode | component | plane strain (`D_PLAN`) | plane stress (`C_PLAN`) |
/// |---|---|---|---|
/// | I | `sigma_yy` | 0.9999999999999999 | 1.0000000000000000 |
/// | I | `sigma_xy` | 0.0000000000000000e0 | 0.0000000000000000e0 |
/// | II | `sigma_xy` | 0.9999999999999999 | 0.9999999999999998 |
/// | II | `sigma_yy` | 0.0000000000000000e0 | 0.0000000000000000e0 |
/// | III | `sigma_yz` | 0.9999999999999999 | 0.9999999999999999 |
///
/// *Interpretation:* this is the decisive test for the plane-state constants.
/// The stress amplitude comes out of `2 cr1 (kappa - 1) (lambda + mu)` where
/// `cr1` carries the `1/(4 mu)`; that product collapses to `4 mu cr1 = 1` only
/// if `kappa` and `lambda_plane` are the *matching* pair for the state. Pairing
/// the plane-strain `kappa` with the plane-stress `lambda*` (or the reverse)
/// breaks it. Since the element routine that supplies upstream's `ka` is absent
/// from the available clone, this identity is what stands in for it — and it
/// passes for both states — every entry is 1 to within one or two units in the
/// last place — so the mapping in [`CrackPlaneState`] is confirmed
/// self-consistent with the Hooke law used here. The mode-III result is the same
/// in both states, confirming that anti-plane shear does not see the in-plane
/// constraint. The two zero entries are exact zeros, not small residuals: mode I
/// produces no shear and mode II no opening stress directly ahead of the tip.
#[test]
fn near_tip_stress_reproduces_the_williams_singularity_in_both_plane_states() {
    let m = steel();
    let r = 1.0e-5;
    let amplitude = (2.0 * PI * r).sqrt();

    for state in [CrackPlaneState::PlaneStrain, CrackPlaneState::PlaneStress] {
        let s1 = near_tip_stress(
            westergaard_unit_field(CrackOpeningMode::Opening, r, 0.0, m, state).unwrap(),
            m,
            state,
        );
        let s2 = near_tip_stress(
            westergaard_unit_field(CrackOpeningMode::InPlaneShear, r, 0.0, m, state).unwrap(),
            m,
            state,
        );
        let s3 = near_tip_stress(
            westergaard_unit_field(CrackOpeningMode::AntiPlaneShear, r, 0.0, m, state).unwrap(),
            m,
            state,
        );

        println!("--- {} ---", state.aster_name());
        println!("mode I  sqrt(2 pi r) sigma_yy = {:.16}", amplitude * s1.yy);
        println!("mode I  sqrt(2 pi r) sigma_xy = {:.16e}", amplitude * s1.xy);
        println!("mode II sqrt(2 pi r) sigma_xy = {:.16}", amplitude * s2.xy);
        println!("mode II sqrt(2 pi r) sigma_yy = {:.16e}", amplitude * s2.yy);
        println!("mode III sqrt(2 pi r) sigma_yz = {:.16}", amplitude * s3.yz);

        assert_relative_eq!(amplitude * s1.yy, 1.0, max_relative = 1e-12);
        assert!(
            (amplitude * s1.xy).abs() < 1e-12,
            "mode I must produce no shear on the crack plane"
        );
        assert_relative_eq!(amplitude * s2.xy, 1.0, max_relative = 1e-12);
        assert!(
            (amplitude * s2.yy).abs() < 1e-12,
            "mode II must produce no opening stress on the crack plane"
        );
        assert_relative_eq!(amplitude * s3.yz, 1.0, max_relative = 1e-12);
    }
}

/// **The mode-I crack-opening displacement matches its closed form, and *that*
/// is where the plane state shows up.**
///
/// *Methodology:* behind the tip, on the crack faces, the mode-I opening is
///
/// `COD(r) = u_y(r, pi) - u_y(r, -pi) = (8 K_I / E') sqrt(r / (2 pi))`
///
/// (equivalently `(kappa + 1) K_I / mu * sqrt(r/(2 pi))`, the two forms being
/// identical once `mu` and `kappa` are expanded). Evaluate the ported field at
/// `theta = +/- pi`, `r = 1e-5 m`, unit `K_I`, and compare against
/// `8 / E' * sqrt(r / (2 pi))` in both plane states. Reference: closed form,
/// exact. Pass criterion: 1e-12 relative.
///
/// *Results (measured 2026-08-05), unit `K_I`, `r = 1e-5 m`:*
///
/// | Plane state | measured COD (m^(1/2)/Pa) | closed form | relative error |
/// |---|---|---|---|
/// | `D_PLAN` | 4.5921011900766911e-14 | 4.5921011900766911e-14 | 0.0e0 |
/// | `C_PLAN` | 5.0462650440403205e-14 | 5.0462650440403199e-14 | 1.3e-16 |
///
/// *Interpretation:* the two are in the ratio `1/(1 - nu^2) = 1.0989`, i.e. the
/// plane-**stress** crack opens **9.89% more** than the plane-strain one under
/// the same `K_I` — the constraint that raises `E'` also stiffens the opening.
/// Together with the previous test this pins the plane state from both
/// directions: the *stress* amplitude is state-independent and the
/// *displacement* is not, so a `kappa`/`lambda` mix-up that survived the stress
/// check would be caught here.
#[test]
fn mode_i_crack_opening_displacement_matches_its_closed_form() {
    let m = steel();
    let r = 1.0e-5;

    for state in [CrackPlaneState::PlaneStrain, CrackPlaneState::PlaneStress] {
        let upper = westergaard_unit_field(CrackOpeningMode::Opening, r, PI, m, state).unwrap();
        let lower = westergaard_unit_field(CrackOpeningMode::Opening, r, -PI, m, state).unwrap();
        let cod = upper.displacement.y - lower.displacement.y;

        let expected = 8.0 / m.effective_modulus(state) * (r / (2.0 * PI)).sqrt();
        let rel = (cod - expected).abs() / expected;

        println!(
            "{}: COD = {cod:.16e}, closed form = {expected:.16e}, rel = {rel:.1e}",
            state.aster_name()
        );
        assert_relative_eq!(cod, expected, max_relative = 1e-12);

        // The equivalent `(kappa + 1) / mu` form must agree exactly.
        let via_kappa =
            (m.kolosov_kappa(state) + 1.0) / m.shear_modulus() * (r / (2.0 * PI)).sqrt();
        assert_relative_eq!(cod, via_kappa, max_relative = 1e-12);
    }
}

/// **The returned gradient really is the gradient of the returned
/// displacement.**
///
/// *Methodology:* the polar-to-Cartesian conversion
/// `d/dx = cos(t) d/dr - (sin(t)/r) d/dt` is transcribed from upstream and is
/// the easiest thing in the port to get subtly wrong (a swapped sign, a missing
/// `1/r`). Check it independently: sample the *displacement* on a Cartesian
/// grid around a point well away from the crack faces and form a second-order
/// central difference, then compare against the returned `gradient`. Point:
/// `r = 1e-5 m`, `theta = 0.7 rad`; step `h = 1e-9 m` (about `1e-4 r`, the
/// sweet spot between truncation and cancellation for a `r^(1/2)` field). Pass
/// criterion: 1e-5 relative — the central difference's own accuracy, not the
/// port's.
///
/// *Results (measured 2026-08-05), worst relative discrepancy over all
/// components:*
///
/// | Mode | worst relative error |
/// |---|---|
/// | I | 4.8335e-9 |
/// | II | 1.3811e-9 |
/// | III | 1.2500e-9 |
///
/// *Interpretation:* three to four orders of magnitude inside the tolerance and
/// consistent with `O(h^2/r^2)` truncation, so the analytic gradient is the true
/// derivative of the analytic displacement. This is what makes the stress checks
/// above meaningful — they consume the gradient, so a wrong gradient with a
/// right displacement would otherwise pass unnoticed in one and fail obscurely
/// in the other.
#[test]
fn westergaard_gradient_is_the_derivative_of_its_displacement() {
    let m = steel();
    let state = CrackPlaneState::PlaneStrain;
    let (r0, t0) = (1.0e-5, 0.7);
    let h = 1.0e-9;

    let sample = |mode: CrackOpeningMode, x: f64, y: f64| {
        let r = (x * x + y * y).sqrt();
        let t = y.atan2(x);
        westergaard_unit_field(mode, r, t, m, state)
            .unwrap()
            .displacement
    };

    for mode in [
        CrackOpeningMode::Opening,
        CrackOpeningMode::InPlaneShear,
        CrackOpeningMode::AntiPlaneShear,
    ] {
        let analytic = westergaard_unit_field(mode, r0, t0, m, state)
            .unwrap()
            .gradient;
        let (x0, y0) = (r0 * t0.cos(), r0 * t0.sin());

        let dx = sample(mode, x0 + h, y0) - sample(mode, x0 - h, y0);
        let dy = sample(mode, x0, y0 + h) - sample(mode, x0, y0 - h);
        let numeric = Tensor::new(
            dx.x / (2.0 * h),
            dy.x / (2.0 * h),
            0.0,
            dx.y / (2.0 * h),
            dy.y / (2.0 * h),
            0.0,
            dx.z / (2.0 * h),
            dy.z / (2.0 * h),
            0.0,
        );

        let scale = analytic
            .double_inner(analytic)
            .sqrt()
            .max(f64::MIN_POSITIVE);
        let diff = numeric - analytic;
        let worst = diff.double_inner(diff).sqrt() / scale;

        println!("mode {}: worst relative error = {worst:.4e}", mode.number());
        assert!(
            worst < 1.0e-5,
            "mode {} gradient disagrees with a central difference by {worst:e}",
            mode.number()
        );
    }
}

/// **A field is only defined off the tip and inside the branch cut.**
///
/// *Methodology:* the Williams field is singular at `r = 0` and discontinuous
/// across `theta = +/- pi` (the crack faces), so both must be rejected rather
/// than evaluated. Pass criterion: `Err` for `r <= 0` and for `|theta| > pi`;
/// `Ok` exactly at `theta = +/- pi`, which is on the faces and is a legitimate
/// query.
///
/// *Result:* all as specified (measured 2026-08-05).
#[test]
fn near_tip_field_rejects_the_tip_itself_and_angles_outside_the_branch() {
    let m = steel();
    let s = CrackPlaneState::PlaneStrain;
    let mode = CrackOpeningMode::Opening;

    assert!(westergaard_unit_field(mode, 0.0, 0.0, m, s).is_err());
    assert!(westergaard_unit_field(mode, -1.0e-6, 0.0, m, s).is_err());
    assert!(westergaard_unit_field(mode, 1.0e-5, 1.01 * PI, m, s).is_err());
    assert!(westergaard_unit_field(mode, 1.0e-5, -1.01 * PI, m, s).is_err());
    assert!(westergaard_unit_field(mode, 1.0e-5, PI, m, s).is_ok());
    assert!(westergaard_unit_field(mode, 1.0e-5, -PI, m, s).is_ok());
}

/// **The Mandel six-vector of the near-tip strain reproduces the tensor double
/// contraction.**
///
/// *Methodology:* upstream's `gbilin.F90` stores the strain of a near-tip field
/// as a `sqrt(2)`-scaled four-vector (`epsu(4) = 0.5*(dudm(1,2)+dudm(2,1))*rac2`)
/// so that a dot product is a double contraction.
/// [`NearTipField::small_strain_mandel`] must reproduce that property against
/// [`AsterVoigt`], the convention pinned in
/// [`kinematics`](crate::rheology::aster::kinematics). Check that
/// `dot(mandel(sigma), mandel(eps))` equals `sigma : eps` for the mode-I field
/// at `r = 1e-5 m`, `theta = 0.7 rad`, scaled to `K_I = 50 MPa m^(1/2)`. Pass
/// criterion: 1e-12 relative.
///
/// *Result (measured 2026-08-05):* Mandel dot product
/// `2.3624136442953223e8 J/m^3`, tensor double contraction
/// `2.3624136442953223e8 J/m^3`, relative difference `0.0e0`. Twice the strain
/// energy density, as it should be. Interpretation: a future port of the
/// G-theta integrand can consume `small_strain_mandel` directly without
/// re-deriving the scaling, which is the whole reason the accessor exists.
#[test]
fn near_tip_strain_in_mandel_form_contracts_correctly() {
    let m = steel();
    let state = CrackPlaneState::PlaneStrain;
    let k1 = 50.0e6;

    let field = westergaard_unit_field(CrackOpeningMode::Opening, 1.0e-5, 0.7, m, state)
        .unwrap()
        .scaled(k1);
    let sigma = near_tip_stress(field, m, state);
    let eps = field.small_strain();

    let mandel = AsterVoigt::from_tensor(sigma).dot(field.small_strain_mandel());
    let tensor = sigma.double_inner(eps);

    println!("Mandel dot        = {mandel:.16e} J/m^3");
    println!("double contraction = {tensor:.16e} J/m^3");
    println!(
        "relative difference = {:.1e}",
        (mandel - tensor).abs() / tensor
    );
    assert_relative_eq!(mandel, tensor, max_relative = 1e-12);
}

// =====================================================================
// Irwin's relation
// =====================================================================

/// **Irwin's relation on a centre-cracked infinite plate — the classical
/// analytical benchmark.**
///
/// *Methodology:* for a through-crack of half-length `a` in an infinite plate
/// under remote uniaxial tension `sigma`, the exact solution is
/// `K_I = sigma sqrt(pi a)` and hence `G = sigma^2 pi a / E'`. Take
/// `sigma = 100 MPa`, `a = 10 mm`, `E = 200 GPa`, `nu = 0.3`, form `K_I` from
/// the closed form, push it through [`irwin_energy_release_rate`], and compare
/// against `sigma^2 pi a / E'` computed independently. Reference: Irwin (1957) /
/// Griffith, exact. Pass criterion: 1e-12 relative in both plane states.
///
/// *Results (measured 2026-08-05), `sigma = 100 MPa`, `a = 0.01 m`:*
///
/// `K_I = 1.7724538509055160e7 Pa m^(1/2)` (17.72 MPa m^(1/2))
///
/// | Plane state | `G` from Irwin (J/m^2) | `sigma^2 pi a / E'` (J/m^2) |
/// |---|---|---|
/// | `D_PLAN` | 1.4294246573833559e3 | 1.4294246573833559e3 |
/// | `C_PLAN` | 1.5707963267948965e3 | 1.5707963267948965e3 |
///
/// *Interpretation:* the plane-strain crack releases 1429.42 J/m^2 and the
/// plane-stress one 1570.80 J/m^2 for the same `K_I` — the same 9.89% again, now
/// in energy. Both agree with the closed form to machine precision, so the
/// relation and the effective modulus are consistent with each other and with
/// the published result.
#[test]
fn irwin_relation_reproduces_the_centre_cracked_infinite_plate() {
    let m = steel();
    let (sigma, a) = (100.0e6, 0.010);
    let k1 = sigma * (PI * a).sqrt();
    println!("K_I = {k1:.16e} Pa m^(1/2)");

    for state in [CrackPlaneState::PlaneStrain, CrackPlaneState::PlaneStress] {
        let g = irwin_energy_release_rate(StressIntensityFactors::mode_i(k1), m, state);
        let expected = sigma * sigma * PI * a / m.effective_modulus(state);
        println!(
            "{}: G = {g:.16e} J/m^2, closed form = {expected:.16e} J/m^2",
            state.aster_name()
        );
        assert_relative_eq!(g, expected, max_relative = 1e-12);
    }
}

/// **The three modes add, and mode III uses `2 mu` in every plane state.**
///
/// *Methodology:* two properties that a mixed-mode implementation gets wrong
/// independently. (a) Additivity: `G(K_I, K_II, K_III)` must equal
/// `G(K_I,0,0) + G(0,K_II,0) + G(0,0,K_III)`, with no cross term — the modes are
/// orthogonal over a circuit around the tip. (b) The mode-III factor is
/// `1/(2 mu) = (1 + nu)/E` and is **independent of the plane state**, unlike
/// modes I and II; applying `E'` to mode III is the plausible-looking error.
/// Inputs `K_I = 30`, `K_II = 20`, `K_III = 10` MPa m^(1/2). Pass criterion:
/// 1e-12 relative.
///
/// *Results (measured 2026-08-05), plane strain:*
///
/// | Contribution | value (J/m^2) |
/// |---|---|
/// | `G_I` | 4.0950000000000000e3 |
/// | `G_II` | 1.8200000000000000e3 |
/// | `G_III` | 6.5000000000000000e2 |
/// | total | 6.5650000000000000e3 |
///
/// Sum of the three single-mode calls: `6.5650000000000000e3 J/m^2`, identical.
/// `G_III` in plane stress: `6.5000000000000000e2 J/m^2` — the **same** value,
/// confirming the plane state does not reach mode III. For contrast, had `E'`
/// been used for mode III in plane strain it would have read
/// `4.5500000000000000e2 J/m^2`, 30% low.
#[test]
fn mode_contributions_add_and_mode_iii_ignores_the_plane_state() {
    let m = steel();
    let k = StressIntensityFactors::new(30.0e6, 20.0e6, 10.0e6);

    let split = irwin_mode_split(k, m, CrackPlaneState::PlaneStrain);
    println!("G_I   = {:.16e} J/m^2", split.mode_i);
    println!("G_II  = {:.16e} J/m^2", split.mode_ii);
    println!("G_III = {:.16e} J/m^2", split.mode_iii);
    println!("total = {:.16e} J/m^2", split.total);

    let separate = irwin_energy_release_rate(
        StressIntensityFactors::new(k.k1, 0.0, 0.0),
        m,
        CrackPlaneState::PlaneStrain,
    ) + irwin_energy_release_rate(
        StressIntensityFactors::new(0.0, k.k2, 0.0),
        m,
        CrackPlaneState::PlaneStrain,
    ) + irwin_energy_release_rate(
        StressIntensityFactors::new(0.0, 0.0, k.k3),
        m,
        CrackPlaneState::PlaneStrain,
    );
    println!("sum of single-mode calls = {separate:.16e} J/m^2");
    assert_relative_eq!(split.total, separate, max_relative = 1e-12);
    assert_relative_eq!(
        split.total,
        split.mode_i + split.mode_ii + split.mode_iii,
        max_relative = 1e-12
    );

    let g3_cp = irwin_mode_split(k, m, CrackPlaneState::PlaneStress).mode_iii;
    println!("G_III plane stress = {g3_cp:.16e} J/m^2");
    assert_relative_eq!(split.mode_iii, g3_cp, max_relative = 1e-12);
    assert_relative_eq!(
        split.mode_iii,
        k.k3 * k.k3 / (2.0 * m.shear_modulus()),
        max_relative = 1e-12
    );

    let wrong = k.k3 * k.k3 / m.effective_modulus(CrackPlaneState::PlaneStrain);
    println!("(if E' had been used for mode III: {wrong:.16e} J/m^2)");
}

/// **`K_eq = sqrt(G E')` round-trips, and a negative `G` is clipped, not
/// rejected.**
///
/// *Methodology:* [`equivalent_mode_i_factor`] must invert
/// [`irwin_energy_release_rate`] exactly for a pure mode-I state. It must also
/// reproduce upstream's clipping behaviour: `calcG_type.F90::addValues` writes
/// `if (gth(2) >= 0) sqrt(gth(2)) else 0`, so a negative energy release rate —
/// which a badly-resolved ring can produce — becomes zero rather than an error.
/// This port **reproduces** that rather than correcting it. Pass criterion:
/// 1e-12 relative on the round trip; exactly 0 for `G < 0`.
///
/// *Results (measured 2026-08-05):* `K_I` in `1.7724538509055160e7
/// Pa m^(1/2)`, `G = 1.4294246573833559e3 J/m^2`, `K_eq` out
/// `1.7724538509055160e7 Pa m^(1/2)`, relative error `0.0e0`. For
/// `G = -1.0 J/m^2`, `K_eq = 0`.
///
/// *Interpretation:* the clipping is upstream's judgement call, not this port's.
/// A user seeing `K_eq = 0` from a nonzero load should read it as "the domain
/// integral returned a negative `G`" — a mesh diagnostic, not a physical result.
#[test]
fn equivalent_mode_i_factor_round_trips_and_clips_a_negative_g() {
    let m = steel();
    let state = CrackPlaneState::PlaneStrain;
    let k1 = 100.0e6 * (PI * 0.010_f64).sqrt();

    let g = irwin_energy_release_rate(StressIntensityFactors::mode_i(k1), m, state);
    let back = equivalent_mode_i_factor(g, m, state);
    println!("K_I in = {k1:.16e}, G = {g:.16e} J/m^2, K_eq out = {back:.16e}");
    println!("relative error = {:.1e}", (back - k1).abs() / k1);
    assert_relative_eq!(back, k1, max_relative = 1e-12);

    let clipped = equivalent_mode_i_factor(-1.0, m, state);
    println!("K_eq for G = -1 J/m^2: {clipped}");
    assert_eq!(clipped, 0.0);
}

// =====================================================================
// Crack-kink direction
// =====================================================================

/// **The maximum-hoop-stress kink angle reproduces its two published limits.**
///
/// *Methodology:* the Erdogan-Sih criterion has two values every textbook
/// quotes: `theta_c = 0` under pure mode I, and `theta_c = -70.53 degrees` (the
/// exact value `-arccos(1/3)`) under pure mode II with `K_II > 0`. Evaluate
/// [`max_hoop_stress_kink_angle`] at both, and at the symmetric case
/// `K_II < 0`. Reference: Erdogan and Sih (1963); the mode-II value is exactly
/// `-arccos(1/3)`. Pass criterion: 1e-12 relative against `-arccos(1/3)`, and
/// exactly 0 for pure mode I.
///
/// *Results (measured 2026-08-05):*
///
/// | Loading | `theta_c` (rad) | `theta_c` (deg) |
/// |---|---|---|
/// | pure mode I (`K_I = 1`, `K_II = 0`) | -0.0000000000000000e0 | -0.00000000000000 |
/// | pure mode II (`K_I = 0`, `K_II = 1`) | -1.2309594173407745e0 | -70.52877936550931 |
/// | pure mode II (`K_I = 0`, `K_II = -1`) | 1.2309594173407745e0 | 70.52877936550931 |
/// | mixed (`K_I = 1`, `K_II = 0.5`) | -7.0175882174451543e-1 | -40.20781872203420 |
///
/// `-arccos(1/3) = -1.2309594173407747e0 rad`; the pure mode-II value differs
/// from it by one unit in the last place. *Interpretation:* the criterion turns
/// the crack away from
/// the shear, and the pure-mode-I answer is exactly zero rather than
/// `1e-17`-ish — a consequence of the rationalised form, which evaluates `-2 K_II
/// / (K_I + sqrt(...))` and returns a hard zero when `K_II` is zero.
#[test]
fn kink_angle_reproduces_its_published_limits() {
    let pure_i = max_hoop_stress_kink_angle(1.0, 0.0).unwrap();
    let pure_ii = max_hoop_stress_kink_angle(0.0, 1.0).unwrap();
    let pure_ii_neg = max_hoop_stress_kink_angle(0.0, -1.0).unwrap();
    let mixed = max_hoop_stress_kink_angle(1.0, 0.5).unwrap();

    for (name, v) in [
        ("pure mode I", pure_i),
        ("pure mode II (+)", pure_ii),
        ("pure mode II (-)", pure_ii_neg),
        ("mixed K_II/K_I = 0.5", mixed),
    ] {
        println!("{name}: {v:.16e} rad = {:.14} deg", v.to_degrees());
    }

    let exact_ii = -(1.0_f64 / 3.0).acos();
    println!("-arccos(1/3) = {exact_ii:.16e} rad");
    assert_eq!(pure_i, 0.0);
    assert_relative_eq!(pure_ii, exact_ii, max_relative = 1e-12);
    assert_relative_eq!(pure_ii_neg, -exact_ii, max_relative = 1e-12);
    assert!(mixed < 0.0, "a positive K_II must turn the crack clockwise");
}

/// **The kink angle really is the stationary point of the hoop stress — checked
/// numerically with this port's own Brent solver.**
///
/// *Methodology:* the closed form in [`max_hoop_stress_kink_angle`] is derived
/// by solving `d sigma_tt / d theta = 0`, i.e.
/// `K_I sin(theta) + K_II (3 cos(theta) - 1) = 0`. Rather than trust the
/// derivation, find that root independently with
/// [`brent`](crate::rheology::aster::integration::brent) on the bracket
/// `(-pi, 0)` and compare. Then confirm the stationary point is a **maximum**,
/// not a minimum, by evaluating [`scaled_hoop_stress`] at `theta_c` and at
/// `theta_c +/- 0.05 rad`. Reference: the criterion's own defining condition —
/// an internal-consistency check between the closed form and a numerical
/// optimum. Ratios `K_II/K_I` of 0.1, 0.5, 1.0 and 3.0. Pass criterion: 1e-9
/// absolute in radians, and the hoop stress strictly greater at `theta_c`.
///
/// *Results (measured 2026-08-05), `K_I = 1`:*
///
/// | `K_II` | closed form (rad) | Brent root (rad) | difference | Brent iterations |
/// |---|---|---|---|---|
/// | 0.1 | -1.9552710137718032e-1 | -1.9552710137720489e-1 | 2.5e-14 | 6 |
/// | 0.5 | -7.0175882174451543e-1 | -7.0175882174451421e-1 | 1.2e-15 | 7 |
/// | 1.0 | -9.2729521800161219e-1 | -9.2729521800161219e-1 | 0.0e0 | 7 |
/// | 3.0 | -1.1224637982012291e0 | -1.1224637982012291e0 | 0.0e0 | 7 |
///
/// Hoop stress (scaled) at the optimum against its two neighbours at
/// `theta_c +/- 0.05 rad`: `1.014747` against `1.013777`/`1.013777` at
/// `K_II = 0.1`; `1.282795` against `1.281272`/`1.281268` at `0.5`; `1.788854`
/// against `1.786347`/`1.786333` at `1.0`; `4.039977` against
/// `4.033230`/`4.033168` at `3.0` — a maximum in every case. *Interpretation:*
/// the closed form and the numerical stationary point agree to machine
/// precision, so upstream's expression selects the correct (maximising) root of
/// the quadratic in `tan(theta/2)` — the other root is the hoop-stress *minimum*
/// and would send the crack the wrong way.
#[test]
fn kink_angle_is_the_maximum_of_the_hoop_stress() {
    let control = SolverControl::default();
    let k1 = 1.0;

    for k2 in [0.1_f64, 0.5, 1.0, 3.0] {
        let closed = max_hoop_stress_kink_angle(k1, k2).unwrap();

        // d(sigma_tt)/d(theta) = 0, up to a positive factor.
        let stationarity = |t: f64| k1 * t.sin() + k2 * (3.0 * t.cos() - 1.0);
        let solution = brent(stationarity, (-PI, 0.0), &control).unwrap();

        println!(
            "K_II = {k2}: closed = {closed:.16e}, brent = {:.16e}, diff = {:.1e}, iters = {}",
            solution.root,
            (solution.root - closed).abs(),
            solution.iterations
        );
        assert!(
            (solution.root - closed).abs() < 1.0e-9,
            "closed form and numerical stationary point disagree"
        );

        let at = scaled_hoop_stress(k1, k2, closed);
        let left = scaled_hoop_stress(k1, k2, closed - 0.05);
        let right = scaled_hoop_stress(k1, k2, closed + 0.05);
        println!("  hoop: left {left:.6}, at {at:.6}, right {right:.6}");
        assert!(
            at > left && at > right,
            "the stationary point must be a maximum of the hoop stress"
        );
    }
}

/// **The rationalised kink-angle form agrees with upstream's literal expression
/// wherever upstream's is defined — and stays defined where upstream's is
/// not.**
///
/// *Methodology:* `gkmet1.F90` / `gkmet3.F90` carry (commented out)
/// `beta = 2*atan2(0.25*(K1/K2 - sign(1,K2)*sqrt((K1/K2)^2 + 8)), 1)`, guarded
/// by `abs(K2) >= 1e-12`. This port uses the algebraically identical
/// `theta = 2 atan(-2 K2 / (K1 + sqrt(K1^2 + 8 K2^2)))`, which has no division
/// by `K_II`. Transcribe upstream's form here and sweep `K_II/K_I` over eight
/// decades from 1e0 down to 1e-7 with `K_I = 1`, comparing the two. Pass
/// criterion: 1e-8 absolute in radians wherever upstream's guard admits the
/// input; the ported form must additionally return a finite answer at
/// `K_II = 0`, where upstream's returns nothing. Finally, arbitrate between the
/// two at `K_II = 1e-6` against a tightly converged Brent root of the
/// stationarity condition (`residual_tol = 1e-18`, `step_tol = 1e-20`).
///
/// *Results (measured 2026-08-05), `K_I = 1`:*
///
/// | `K_II` | ported (rad) | upstream literal (rad) | absolute difference | relative |
/// |---|---|---|---|---|
/// | 1e0 | -9.2729521800161219e-1 | -9.2729521800161219e-1 | 0.00e0 | 0.00e0 |
/// | 1e-1 | -1.9552710137718032e-1 | -1.9552710137718046e-1 | 1.39e-16 | 7.10e-16 |
/// | 1e-2 | -1.9995335372251111e-2 | -1.9995335372254001e-2 | 2.89e-15 | 1.45e-13 |
/// | 1e-3 | -1.9999953333537336e-3 | -1.9999953333327413e-3 | 2.10e-14 | 1.05e-11 |
/// | 1e-4 | -1.9999999533333360e-4 | -1.9999999519040312e-4 | 1.43e-13 | 7.15e-10 |
/// | 1e-5 | -1.9999999995333335e-5 | -1.9999999494090838e-5 | 5.01e-13 | 2.51e-8 |
/// | 1e-6 | -1.9999999999953332e-6 | -2.0000152289860735e-6 | 1.52e-11 | 7.61e-6 |
/// | 1e-7 | -1.9999999999999533e-7 | -2.0023435354232722e-7 | 2.34e-10 | 1.17e-3 |
///
/// Arbitration at `K_II = 1e-6`: Brent reference `-1.9999999999953336e-6 rad`;
/// ported `-1.9999999999953332e-6 rad`, error `4.235e-22`; upstream
/// `-2.0000152289860735e-6 rad`, error `1.523e-11`.
///
/// *Interpretation:* the two agree to well inside the tolerance in absolute
/// terms, so this is a numerical restatement and not a change of behaviour — but
/// the **relative** column is the point, and it grows by roughly two decades per
/// decade of shrinking `K_II`. Upstream's form computes
/// `K1/K2 - sqrt((K1/K2)^2 + 8)`, a difference of two nearly equal large
/// numbers, so it loses about two significant figures for every decade `K_II`
/// falls. By `K_II = 1e-7 K_I` its answer is wrong in the fourth significant
/// figure (0.117%). The Brent arbitration settles which of the two is drifting:
/// the ported value sits `4.2e-22` from the independent numerical root, upstream
/// `1.5e-11` — eleven orders of magnitude further out. At `K_II = 0` upstream's
/// guard declines to answer at all, while the ported form returns exactly 0.
/// Since near-mode-I is the regime most real calculations sit in, this matters
/// more than it looks.
#[test]
fn rationalised_kink_angle_agrees_with_the_upstream_literal_expression() {
    /// Upstream's expression, transcribed verbatim from the commented-out line
    /// in `gkmet1.F90` / `gkmet3.F90`, including its `1e-12` guard on `K_II`.
    fn upstream_literal(k1: f64, k2: f64) -> Option<f64> {
        if k2.abs() < 1.0e-12 {
            return None;
        }
        let ratio = k1 / k2;
        let sign = if k2 >= 0.0 { 1.0 } else { -1.0 };
        Some(2.0 * (0.25 * (ratio - sign * (ratio * ratio + 8.0).sqrt())).atan2(1.0))
    }

    let k1 = 1.0;
    for exponent in 0..8 {
        let k2 = 10.0_f64.powi(-exponent);
        let ported = max_hoop_stress_kink_angle(k1, k2).unwrap();
        let literal = upstream_literal(k1, k2).unwrap();
        println!(
            "K_II = 1e-{exponent}: ported = {ported:.16e}, upstream = {literal:.16e}, \
             |diff| = {:.2e}, relative = {:.2e}",
            (ported - literal).abs(),
            (ported - literal).abs() / ported.abs()
        );
        assert!(
            (ported - literal).abs() < 1.0e-8,
            "ported and upstream forms disagree at K_II = {k2:e}"
        );
    }

    // Which of the two is right? Arbitrate with an independent, tightly
    // converged numerical root of the stationarity condition.
    let k2 = 1.0e-6;
    let tight = SolverControl {
        max_iter: 200,
        residual_tol: 1.0e-18,
        step_tol: 1.0e-20,
    };
    let reference = brent(
        |t: f64| k1 * t.sin() + k2 * (3.0 * t.cos() - 1.0),
        (-1.0e-3, 0.0),
        &tight,
    )
    .unwrap()
    .root;
    let ported = max_hoop_stress_kink_angle(k1, k2).unwrap();
    let literal = upstream_literal(k1, k2).unwrap();
    println!("arbitration at K_II = 1e-6:");
    println!("  Brent reference = {reference:.16e} rad");
    println!(
        "  ported          = {ported:.16e} rad  (error {:.3e})",
        (ported - reference).abs()
    );
    println!(
        "  upstream        = {literal:.16e} rad  (error {:.3e})",
        (literal - reference).abs()
    );
    assert!(
        (ported - reference).abs() < (literal - reference).abs(),
        "the rationalised form must be at least as accurate as upstream's"
    );

    assert!(upstream_literal(1.0, 0.0).is_none());
    assert_eq!(max_hoop_stress_kink_angle(1.0, 0.0).unwrap(), 0.0);
}

/// **A closed crack in pure compression is refused.**
///
/// *Methodology:* with `K_II = 0` and `K_I <= 0` the hoop stress is nowhere
/// tensile and has no maximum; the maximum-hoop-stress criterion simply does not
/// apply. Rather than return an arbitrary angle, the port errors. Pass
/// criterion: `Err` for `(K_I, K_II) = (-1, 0)` and `(0, 0)`; `Ok` as soon as
/// any shear is present.
///
/// *Results (measured 2026-08-05), `K_I = -1`:*
///
/// | `K_II` | `theta_c` (rad) |
/// |---|---|
/// | 1e-3 | -3.1375926669230498e0 |
/// | 1e-15 | -3.1415926535897891e0 |
/// | 1e-30 | -3.1415926535897931e0 |
///
/// Both degenerate cases (`K_II = 0` with `K_I = -1` and with `K_I = 0`) are
/// rejected. *Interpretation:* the negative-`K_I` branch stays finite and
/// converges smoothly to `-pi` as the shear vanishes — a full turn back along
/// the crack. This is the branch that exists specifically to avoid the
/// cancellation `K_I + sqrt(K_I^2 + 8 K_II^2)` suffers when `K_I < 0`; the
/// first form would evaluate `-1 + 1.0` to exactly zero here and fail. The
/// answer is numerically sound but physically suspect — a caller seeing
/// `theta_c` near `-pi` should conclude the criterion has left its domain of
/// validity, not that the crack turns around.
#[test]
fn kink_angle_refuses_a_closed_crack_in_pure_compression() {
    assert!(max_hoop_stress_kink_angle(-1.0, 0.0).is_err());
    assert!(max_hoop_stress_kink_angle(0.0, 0.0).is_err());

    for k2 in [1.0e-3_f64, 1.0e-15, 1.0e-30] {
        let grazing = max_hoop_stress_kink_angle(-1.0, k2).unwrap();
        println!("K_I = -1, K_II = {k2:e}: {grazing:.16e} rad");
        assert!(
            grazing.is_finite(),
            "the negative-K_I branch must stay finite for any nonzero shear"
        );
        assert!(grazing < -PI / 2.0);
    }
}

// =====================================================================
// Crack-tip basis
// =====================================================================

/// **The 2-D crack-tip frame reproduces `cakg2d.F90`'s 90-degree rotation, and
/// is orthonormal.**
///
/// *Methodology:* upstream builds the crack-plane normal by rotating the
/// propagation vector, not by reading a stored normal —
/// `rcmp(5) = -zr(jbasfo+3)`, `rcmp(6) = zr(jbasfo+2)`, i.e.
/// `(n_x, n_y) = (-t_y, t_x)` (`cakg2d.F90` lines 276-279). Build a frame from a
/// propagation direction at 30 degrees, check the normal is that exact rotation,
/// check all three axes are unit and mutually orthogonal, and check the frame is
/// right-handed (`x cross y = z`). Pass criterion: 1e-14 absolute.
///
/// *Results (measured 2026-08-05), propagation at 30 degrees:*
///
/// - propagation `(0.8660254037844387, 0.4999999999999999, 0)`
/// - normal `(-0.4999999999999999, 0.8660254037844387, 0)`
/// - tangent `(0, 0, 1)`
/// - worst deviation from orthonormality: `0.0000000000000000e0`
/// - `|x cross y - z| = 0.0000000000000000e0`
///
/// *Interpretation:* the normal is the propagation direction turned 90 degrees
/// anticlockwise, matching upstream's sense. Getting that sense backwards would
/// flip the sign of `K_II` throughout without disturbing `K_I`, which is the
/// kind of error that survives a mode-I regression suite.
#[test]
fn planar_crack_tip_frame_matches_the_upstream_rotation() {
    let angle = PI / 6.0;
    let dir = Vector3::new(angle.cos(), angle.sin(), 0.0);
    let basis = CrackTipBasis::from_propagation_direction_2d(dir).unwrap();

    let (p, n, t) = (
        basis.propagation_direction(),
        basis.crack_plane_normal(),
        basis.front_tangent(),
    );
    println!("propagation = ({:.16}, {:.16}, {:.16})", p.x, p.y, p.z);
    println!("normal      = ({:.16}, {:.16}, {:.16})", n.x, n.y, n.z);
    println!("tangent     = ({:.16}, {:.16}, {:.16})", t.x, t.y, t.z);

    assert_relative_eq!(n.x, -p.y, max_relative = 1e-14);
    assert_relative_eq!(n.y, p.x, max_relative = 1e-14);

    let worst = [
        (p.mag() - 1.0).abs(),
        (n.mag() - 1.0).abs(),
        (t.mag() - 1.0).abs(),
        p.dot(n).abs(),
        p.dot(t).abs(),
        n.dot(t).abs(),
    ]
    .into_iter()
    .fold(0.0_f64, f64::max);
    println!("worst orthonormality deviation = {worst:.16e}");
    assert!(worst < 1.0e-14);

    let handedness = p.cross(n) - t;
    println!("|x cross y - z| = {:.16e}", handedness.mag());
    assert!(handedness.mag() < 1.0e-14);

    assert!(CrackTipBasis::from_propagation_direction_2d(Vector3::new(0.0, 0.0, 1.0)).is_err());
}

/// **The 3-D frame orthogonalises a not-quite-perpendicular normal, and the
/// local/global rotations invert each other.**
///
/// *Methodology:* a crack-plane normal extracted from a discretised surface is
/// only approximately perpendicular to a front tangent extracted from a
/// discretised front. Supply a normal deliberately tilted 5 degrees out of
/// perpendicular and check the returned frame is exactly orthonormal, that the
/// front tangent is preserved (it is the trusted direction), and that
/// `global_to_local(local_to_global(x)) = x` for both a vector and a
/// second-order tensor. Pass criterion: 1e-14 absolute.
///
/// *Results (measured 2026-08-05), normal tilted by 5 degrees:*
///
/// - worst orthonormality deviation after Gram-Schmidt:
///   `0.0000000000000000e0`
/// - front tangent preserved to `0.0000000000000000e0`
/// - vector round-trip error `0.0000000000000000e0`
/// - tensor round-trip error `0.0000000000000000e0`
///
/// *Interpretation:* silently accepting a 5-degree-skew frame would leak mode I
/// into mode II at about `sin(5 deg) = 8.7%`. The orthogonalisation removes it,
/// and the round-trip confirms `local_to_global_gradient` (`P G P^T`, upstream's
/// `invp` double contraction in `chauxi.F90`) is a true rotation rather than
/// merely a plausible index shuffle.
#[test]
fn three_dimensional_frame_orthogonalises_and_round_trips() {
    let tangent = Vector3::new(0.0, 0.0, 1.0);
    let tilt = 5.0_f64.to_radians();
    // A normal that is 5 degrees away from perpendicular to the front.
    let normal = Vector3::new(0.0, tilt.cos(), tilt.sin());

    let basis = CrackTipBasis::from_front_tangent_and_normal(tangent, normal).unwrap();
    let (p, n, t) = (
        basis.propagation_direction(),
        basis.crack_plane_normal(),
        basis.front_tangent(),
    );

    let worst = [
        (p.mag() - 1.0).abs(),
        (n.mag() - 1.0).abs(),
        (t.mag() - 1.0).abs(),
        p.dot(n).abs(),
        p.dot(t).abs(),
        n.dot(t).abs(),
    ]
    .into_iter()
    .fold(0.0_f64, f64::max);
    println!("worst orthonormality deviation = {worst:.16e}");
    assert!(worst < 1.0e-14);

    let preserved = (t - tangent).mag();
    println!("front tangent preserved to {preserved:.16e}");
    assert!(preserved < 1.0e-14);

    let v = Vector3::new(0.3, -1.7, 2.9);
    let v_back = basis.global_to_local_vector(basis.local_to_global_vector(v));
    println!("vector round-trip error = {:.16e}", (v_back - v).mag());
    assert!((v_back - v).mag() < 1.0e-14);

    let g = Tensor::new(1.0, 0.4, -0.7, -0.3, 0.8, 0.5, 0.6, -0.2, 1.3);
    let g_back = basis.global_to_local_gradient(basis.local_to_global_gradient(g));
    let diff = g_back - g;
    println!(
        "tensor round-trip error = {:.16e}",
        diff.double_inner(diff).sqrt()
    );
    assert!(diff.double_inner(diff).sqrt() < 1.0e-14);

    // Degenerate inputs.
    assert!(
        CrackTipBasis::from_front_tangent_and_normal(Vector3::new(0.0, 0.0, 0.0), normal).is_err()
    );
    assert!(CrackTipBasis::from_front_tangent_and_normal(tangent, tangent).is_err());
}

/// **Rotating a near-tip field rotates its stress the same way.**
///
/// *Methodology:* objectivity of the ported transformation. Compute the mode-I
/// stress in the local frame, then rotate the *field* into a global frame whose
/// propagation direction is at 37 degrees and recompute the stress there; the
/// two stress tensors must be related by the same rotation. Concretely, check
/// that the two stress *invariants* (trace and the second invariant, both
/// rotation-independent) agree, and that the rotated local stress equals the
/// global one component by component. Inputs: `r = 1e-5 m`, `theta = 0.4 rad`,
/// `K_I = 50 MPa m^(1/2)`, plane strain. Pass criterion: 1e-10 relative.
///
/// *Results (measured 2026-08-05):*
///
/// - trace, local frame: `1.6073446065925560e10 Pa`
/// - trace, global frame: `1.6073446065925554e10 Pa`
/// - worst component discrepancy between the rotated local stress and the
///   global stress: `1.9073486328125000e-6 Pa` on a stress scale of
///   `9.6546407669042320e9 Pa`, i.e. `2.0e-16` relative.
///
/// *Interpretation:* the field transformation and the Hooke law commute, which
/// they must — a stress is a tensor, and computing it before or after a change
/// of frame has to give the same physical state. A transposed rotation matrix
/// would pass the invariant check and fail the component check, so both are
/// made.
#[test]
fn rotating_the_field_rotates_its_stress() {
    let m = steel();
    let state = CrackPlaneState::PlaneStrain;
    let field = westergaard_unit_field(CrackOpeningMode::Opening, 1.0e-5, 0.4, m, state)
        .unwrap()
        .scaled(50.0e6);

    let sigma_local = near_tip_stress(field, m, state);

    let angle = 37.0_f64.to_radians();
    let basis =
        CrackTipBasis::from_propagation_direction_2d(Vector3::new(angle.cos(), angle.sin(), 0.0))
            .unwrap();
    let global_field = basis.field_to_global(field);
    let sigma_global = near_tip_stress(global_field, m, state);

    println!("trace local  = {:.16e} Pa", sigma_local.tr());
    println!("trace global = {:.16e} Pa", sigma_global.tr());
    assert_relative_eq!(sigma_local.tr(), sigma_global.tr(), max_relative = 1e-10);

    // Rotate the local stress explicitly and compare component by component.
    let as_tensor = Tensor::new(
        sigma_local.xx,
        sigma_local.xy,
        sigma_local.xz,
        sigma_local.xy,
        sigma_local.yy,
        sigma_local.yz,
        sigma_local.xz,
        sigma_local.yz,
        sigma_local.zz,
    );
    let rotated = basis.local_to_global_gradient(as_tensor);
    let components = [
        (rotated.xx, sigma_global.xx),
        (rotated.xy, sigma_global.xy),
        (rotated.xz, sigma_global.xz),
        (rotated.yy, sigma_global.yy),
        (rotated.yz, sigma_global.yz),
        (rotated.zz, sigma_global.zz),
    ];
    let worst = components
        .iter()
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f64, f64::max);
    let scale = sigma_local.mag();
    println!("worst component discrepancy = {worst:.16e} Pa on a scale of {scale:.16e} Pa");
    println!("relative = {:.1e}", worst / scale);
    assert!(worst / scale < 1.0e-10);
}

// =====================================================================
// 2-D summed-result post-processing
// =====================================================================

/// **The symmetric-half-model correction *restores* the identity
/// `G_IRWIN = G`, which does not hold before it.**
///
/// *Methodology:* `cakg2d.F90` lines 485-491 double `G`, the mode-I Irwin root
/// and `K_I` while zeroing the mode-II slots. Doubling `G` *and* the root looks
/// inconsistent, since `G_IRWIN` is the square of the root. Construct a
/// half-model result that is consistent with a known full-model mode-I state
/// (`G_full = 1400 J/m^2`, `root_full = sqrt(1400)`, `K_I_full = 10 MPa
/// m^(1/2)`, every slot halved because every slot is an integral over the ring),
/// add a spurious mode-II root that symmetry must remove, apply the correction,
/// and check each slot's factor plus the identity `G_IRWIN = G` before and
/// after. Pass criterion: 1e-12 relative.
///
/// *Results (measured 2026-08-05):*
///
/// | Quantity | before | after |
/// |---|---|---|
/// | `G` (J/m^2) | 7.0000000000000000e2 | 1.4000000000000000e3 |
/// | mode-I root | 1.8708286933869708e1 | 3.7416573867739416e1 |
/// | mode-II root | 3.0000000000000000e0 | 0.0000000000000000e0 |
/// | `K_I` (Pa m^(1/2)) | 5.0000000000000000e6 | 1.0000000000000000e7 |
/// | `G_IRWIN` (J/m^2) | 3.5900000000000006e2 | 1.4000000000000002e3 |
/// | `G_IRWIN / G` | 0.5128571428571429 | 1.0000000000000002 |
///
/// With the spurious mode-II root removed from the comparison, the *before*
/// ratio is `0.5000000000000001` (printed by the test as
/// `mode-I-only ratio before`).
///
/// *Interpretation, and this test was written the wrong way round first:* the
/// initial version asserted `G_IRWIN = G` on the **uncorrected** half-model
/// result and failed, reading `G_IRWIN = 2800` against `G = 1400`. The
/// assumption behind it — that a half-model result satisfies the Irwin identity
/// and the correction preserves it — is wrong. What is true is that every one of
/// the five slots is an integral over the ring, so meshing half the body halves
/// all five; `G_IRWIN`, being a square, is therefore **quartered** while `G` is
/// halved, and the raw half-model ratio is 0.5. The doubling restores both to
/// their full-model values at once, and only then do they agree. The corollary
/// noted on
/// [`with_symmetric_half_model`](super::PlanarCrackTipResult::with_symmetric_half_model)
/// — that slot 2 must be a quantity linear in the ring, i.e. `K_I / sqrt(E')`
/// and not `sqrt(G_I)` — follows from the same arithmetic and is the part that
/// cannot be checked against source, because the element routine that fills the
/// slots is absent from the available upstream clone.
#[test]
fn symmetric_half_model_correction_restores_the_irwin_identity() {
    // A full-model mode-I state, and the half-model result that would produce
    // it: every slot halved, because every slot is an integral over the ring.
    let g_full = 1400.0_f64;
    let root_full = g_full.sqrt();
    let k1_full = 1.0e7;

    let half = PlanarCrackTipResult {
        g: 0.5 * g_full,
        mode_i_root: 0.5 * root_full,
        mode_ii_root: 3.0, // spurious shear that symmetry must remove
        k1: 0.5 * k1_full,
        k2: 1.0e6,
    };
    println!(
        "before: G = {:.16e}, G_IRWIN = {:.16e}, ratio G_IRWIN/G = {:.16}",
        half.g,
        half.g_irwin(),
        half.g_irwin() / half.g
    );
    println!(
        "        root_I = {:.16e}, root_II = {:.16e}, K1 = {:.16e}",
        half.mode_i_root, half.mode_ii_root, half.k1
    );

    let full = half.with_symmetric_half_model();
    println!(
        "after:  G = {:.16e}, G_IRWIN = {:.16e}, ratio G_IRWIN/G = {:.16}",
        full.g,
        full.g_irwin(),
        full.g_irwin() / full.g
    );
    println!(
        "        root_I = {:.16e}, root_II = {:.16e}, K1 = {:.16e}",
        full.mode_i_root, full.mode_ii_root, full.k1
    );

    // Transcription of cakg2d.F90 lines 485-491, slot by slot.
    assert_relative_eq!(full.g, 2.0 * half.g, max_relative = 1e-12);
    assert_relative_eq!(
        full.mode_i_root,
        2.0 * half.mode_i_root,
        max_relative = 1e-12
    );
    assert_relative_eq!(full.k1, 2.0 * half.k1, max_relative = 1e-12);
    assert_eq!(full.mode_ii_root, 0.0);
    assert_eq!(full.k2, 0.0);

    // The identity holds after the correction and not before, by exactly 2.
    assert_relative_eq!(full.g_irwin(), full.g, max_relative = 1e-12);
    assert_relative_eq!(full.g, g_full, max_relative = 1e-12);
    let mode_i_only = PlanarCrackTipResult {
        mode_ii_root: 0.0,
        ..half
    };
    println!(
        "mode-I-only ratio before = {:.16}",
        mode_i_only.g_irwin() / mode_i_only.g
    );
    assert_relative_eq!(
        mode_i_only.g_irwin() / mode_i_only.g,
        0.5,
        max_relative = 1e-12
    );
}

/// **The axisymmetric normalisation divides all five slots by the crack-tip
/// radius, and refuses a tip on the axis.**
///
/// *Methodology:* `cakg2d.F90` lines 479-483 apply `valg(i) = valg(i)/rcmp(1)`
/// to all five summed quantities when the model is axisymmetric, converting a
/// whole-revolution integral back to the per-unit-length quantity `G` is defined
/// as. Check every slot is divided, and that a tip radius of zero is rejected
/// rather than producing infinities. Pass criterion: 1e-12 relative; `Err` for
/// `r_tip <= 0`.
///
/// *Results (measured 2026-08-05), `r_tip = 0.05 m`:*
///
/// | Slot | before | after |
/// |---|---|---|
/// | `G` | 7.0000000000000000e2 | 1.4000000000000000e4 |
/// | mode-I root | 2.6457513110645905e1 | 5.2915026221291805e2 |
/// | mode-II root | 3.0000000000000000e0 | 6.0000000000000000e1 |
/// | `K_I` | 5.0000000000000000e6 | 1.0000000000000000e8 |
/// | `K_II` | 1.0000000000000000e6 | 2.0000000000000000e7 |
///
/// `r_tip = 0` and `r_tip = -0.05` both rejected. *Interpretation:* upstream
/// would divide by zero on the axis and emit an infinity into its result table;
/// this port declines instead, which is a deliberate departure and is recorded
/// as such in the method documentation.
#[test]
fn axisymmetric_normalisation_divides_every_slot() {
    let raw = PlanarCrackTipResult {
        g: 700.0,
        mode_i_root: 700.0_f64.sqrt(),
        mode_ii_root: 3.0,
        k1: 5.0e6,
        k2: 1.0e6,
    };
    let r_tip = 0.05;
    let out = raw.with_axisymmetric_normalisation(r_tip).unwrap();

    for (name, before, after) in [
        ("G", raw.g, out.g),
        ("root_I", raw.mode_i_root, out.mode_i_root),
        ("root_II", raw.mode_ii_root, out.mode_ii_root),
        ("K1", raw.k1, out.k1),
        ("K2", raw.k2, out.k2),
    ] {
        println!("{name}: {before:.16e} -> {after:.16e}");
        assert_relative_eq!(after, before / r_tip, max_relative = 1e-12);
    }

    assert!(raw.with_axisymmetric_normalisation(0.0).is_err());
    assert!(raw.with_axisymmetric_normalisation(-0.05).is_err());

    let k = out.stress_intensity_factors();
    assert_eq!(k.k3, 0.0);
}

// =====================================================================
// Crack-front Legendre basis
// =====================================================================

/// **The crack-front Legendre basis is orthonormal in `L2(0, L)`.**
///
/// *Methodology:* the `sqrt((2n + 1)/L)` normalisation in `plegen.F90` exists to
/// make `integral_0^L phi_m phi_n ds = delta_mn`, which is what keeps the Gram
/// matrix upstream assembles well-conditioned. Verify by composite Simpson
/// quadrature with 8000 intervals over a front of length `L = 0.35 m`, for all
/// pairs of degrees 0 through 7. Reference: the orthogonality of the Legendre
/// polynomials, `integral_{-1}^{1} P_m P_n dxi = 2 delta_mn / (2n + 1)`, exact.
/// Pass criterion: 1e-9 absolute on every one of the 64 entries.
///
/// *Results (measured 2026-08-05), `L = 0.35 m`, 8000 Simpson intervals:*
///
/// - worst diagonal entry: `1.0000000000227200e0`, i.e. an error of `2.2720e-11`
/// - worst magnitude of an off-diagonal entry: `8.1975923530800774e-12`
///
/// *Interpretation:* orthonormal to `2.3e-11`, a residual consistent with
/// Simpson's `O(h^4)` truncation on a degree-14 integrand rather than with any
/// error in the basis. So the `sqrt((2n + 1)/L)` normalisation is transcribed
/// correctly, and
/// the family is orthogonal as well as normalised (all 56 off-diagonal entries
/// below 1e-11).
#[test]
fn legendre_front_basis_is_orthonormal_over_the_front() {
    let l = 0.35;
    let n_intervals = 8000;
    let h = l / n_intervals as f64;

    let integrate = |m: usize, n: usize| {
        let f =
            |s: f64| legendre_front_mode(m, s, l).unwrap() * legendre_front_mode(n, s, l).unwrap();
        let mut sum = f(0.0) + f(l);
        for i in 1..n_intervals {
            let weight = if i % 2 == 1 { 4.0 } else { 2.0 };
            sum += weight * f(i as f64 * h);
        }
        sum * h / 3.0
    };

    let mut worst_diagonal = 1.0_f64;
    let mut worst_diagonal_error = 0.0_f64;
    let mut worst_off = 0.0_f64;
    for m in 0..=MAX_LEGENDRE_FRONT_DEGREE {
        for n in 0..=MAX_LEGENDRE_FRONT_DEGREE {
            let value = integrate(m, n);
            if m == n {
                if (value - 1.0).abs() > worst_diagonal_error {
                    worst_diagonal_error = (value - 1.0).abs();
                    worst_diagonal = value;
                }
                assert!(
                    (value - 1.0).abs() < 1.0e-9,
                    "diagonal entry ({m},{n}) = {value}"
                );
            } else {
                if value.abs() > worst_off.abs() {
                    worst_off = value;
                }
                assert!(
                    value.abs() < 1.0e-9,
                    "off-diagonal entry ({m},{n}) = {value}"
                );
            }
        }
    }
    println!("worst diagonal entry  = {worst_diagonal:.16e} (error {worst_diagonal_error:.4e})");
    println!("worst off-diagonal    = {worst_off:.16e}");
}

/// **The hard-coded Legendre polynomials satisfy Bonnet's three-term
/// recurrence.**
///
/// *Methodology:* `plegen.F90` writes each `P_n` out in closed form as a
/// preprocessor macro, so a transcription typo in one coefficient would not
/// disturb any other degree and would not show up in an orthonormality check
/// that only sums squares. Bonnet's recurrence
/// `(n + 1) P_{n+1}(x) = (2n + 1) x P_n(x) - n P_{n-1}(x)` is an independent
/// identity that couples all eight degrees, so it catches exactly that. Evaluate
/// over 41 abscissae spanning `[-1, 1]` (obtained by sweeping `s` over
/// `[0, L]`), for `n = 1..6`. Reference: Bonnet's recurrence, exact. Pass
/// criterion: 1e-12 absolute.
///
/// *Result (measured 2026-08-05):* worst recurrence residual over all 41
/// abscissae and all six recurrence steps: `2.1066481892262345e-14`.
/// Interpretation: every closed form is the polynomial it claims to be; a wrong
/// coefficient anywhere would show up here as an `O(1)` residual.
#[test]
fn legendre_polynomials_satisfy_bonnets_recurrence() {
    let l = 1.0;
    let mut worst = 0.0_f64;
    for i in 0..=40 {
        let s = l * i as f64 / 40.0;
        let x = 2.0 * s / l - 1.0;
        for n in 1..=6usize {
            // Undo the L2 normalisation to recover the bare P_n.
            let p =
                |d: usize| legendre_front_mode(d, s, l).unwrap() / ((2 * d + 1) as f64 / l).sqrt();
            let residual =
                (n + 1) as f64 * p(n + 1) - (2 * n + 1) as f64 * x * p(n) + n as f64 * p(n - 1);
            worst = worst.max(residual.abs());
        }
    }
    println!("worst Bonnet recurrence residual = {worst:.16e}");
    assert!(worst < 1.0e-12);
}

/// **The Legendre basis derivative matches a finite difference of the basis.**
///
/// *Methodology:* `dplegen.F90` is a separate hand-written table, so it can drift
/// from `plegen.F90` without either being internally inconsistent. Compare
/// [`legendre_front_mode_derivative`] against a second-order central difference
/// of [`legendre_front_mode`] with step `h = 1e-6 L`, at eleven interior
/// abscissae, for degrees 0 through 7, `L = 0.35 m`. Pass criterion: 1e-6
/// relative to the local derivative scale — the central difference's own
/// accuracy, not the port's.
///
/// *Result (measured 2026-08-05):* worst relative discrepancy over all 88
/// (degree, abscissa) pairs: `4.4824485853288069e-9`. Interpretation: three
/// orders inside tolerance and consistent with `O(h^2)` truncation, so the two
/// tables agree and the `(2/L)` chain-rule prefactor is right. That prefactor is
/// the easy thing to drop, and dropping it would show as an `O(1)` failure here.
#[test]
fn legendre_basis_derivative_matches_a_finite_difference() {
    let l = 0.35;
    let h = 1.0e-6 * l;
    let mut worst = 0.0_f64;

    for degree in 0..=MAX_LEGENDRE_FRONT_DEGREE {
        for i in 1..=11 {
            let s = l * i as f64 / 12.0;
            let analytic = legendre_front_mode_derivative(degree, s, l).unwrap();
            let numeric = (legendre_front_mode(degree, s + h, l).unwrap()
                - legendre_front_mode(degree, s - h, l).unwrap())
                / (2.0 * h);
            let scale = analytic.abs().max(1.0 / l.sqrt());
            worst = worst.max((analytic - numeric).abs() / scale);
        }
    }
    println!("worst relative discrepancy = {worst:.16e}");
    assert!(worst < 1.0e-6);
}

/// **Out-of-range Legendre degrees and non-positive front lengths are refused.**
///
/// *Methodology:* upstream hits `ASSERT(.false.)` above degree 7; this port
/// returns [`OffbeatError::NotImplemented`] instead of panicking, which is the
/// workspace's convention for declared-but-absent capability. A zero or negative
/// front length is unphysical and is rejected separately. Pass criterion: `Err`
/// in all four cases, `Ok` at degree 7.
///
/// *Result:* all as specified (measured 2026-08-05).
#[test]
fn legendre_basis_refuses_unsupported_degrees_and_lengths() {
    assert!(legendre_front_mode(MAX_LEGENDRE_FRONT_DEGREE, 0.1, 1.0).is_ok());
    assert!(legendre_front_mode(8, 0.1, 1.0).is_err());
    assert!(legendre_front_mode_derivative(8, 0.1, 1.0).is_err());
    assert!(legendre_front_mode(2, 0.1, 0.0).is_err());
    assert!(legendre_front_mode_derivative(2, 0.1, -1.0).is_err());
}

// =====================================================================
// Crack-front hat smoothing
// =====================================================================

/// **Hat smoothing reproduces a constant exactly, is exact in the interior on a
/// linear field, and biases the two end values inward.**
///
/// *Methodology:* two properties of `hatSmooth.F90`'s fixed three-point filter,
/// checked on two deliberately different fronts.
///
/// (a) *Constant preservation* on a **non-uniform** front (5 nodes at
/// `[0, 0.1, 0.3, 0.8, 1.4]`, with mid-side nodes deliberately off-centre): the
/// interior weights satisfy `lg + ld = 2` identically, so `(lg c + c + ld c)/3 =
/// c`, and both end stencils are convex combinations summing to 1. A constant
/// `G(s)` must therefore come back unchanged, and a mistranscribed `lg`/`ld`
/// would show only on a non-uniform front. Pass criterion: 1e-15 absolute.
///
/// (b) *Linear-field behaviour* on a **uniform** front (`[0, 0.25, 0.5, 0.75,
/// 1]`, mid-side nodes at the segment midpoints), which isolates the end bias
/// from the effect of off-centre mid-side nodes. The interior corner stencil
/// must be exact on a linear field; the end stencils are not, and this asserts
/// the size of the resulting bias rather than tolerating it silently. Pass
/// criterion: interior exact to 1e-15; end shift equal to one third of the
/// corner-to-midside spacing to 1e-12 relative.
///
/// *Results (measured 2026-08-05):*
///
/// - constant `7.5` on the non-uniform front: maximum deviation after smoothing
///   `0.0000000000000000e0`
/// - linear `v = s` on the uniform front: input `[0, 0.25, 0.5, 0.75, 1]`,
///   output `[0.0833333333333333, 0.2916666666666667, 0.5000000000000000,
///   0.7083333333333333, 0.9166666666666666]`
///
/// | node | shift |
/// |---|---|
/// | 0 | +8.3333333333333329e-2 |
/// | 1 | +4.1666666666666685e-2 |
/// | 2 | +0.0000000000000000e0 |
/// | 3 | -4.1666666666666741e-2 |
/// | 4 | -8.3333333333333370e-2 |
///
/// *Interpretation:* the constant is exact to the last bit on a non-uniform
/// front, so the `lg`/`ld` weights are transcribed correctly. On the linear
/// field the interior corner node is untouched — the stencil is second-order
/// accurate there — while the two ends move inward by `0.08333 = 0.25/3`,
/// exactly one third of the corner-to-midside spacing, and the adjacent mid-side
/// nodes by half that. This is the documented limitation of upstream's filter,
/// reproduced rather than corrected: a user reading a smoothed `G(s)` should not
/// treat the two end values as converged, and on a coarse front the end bias can
/// be a sizeable fraction of the front-end `G`.
#[test]
fn hat_smoothing_preserves_a_constant_and_biases_the_ends() {
    let abscissae = [0.0, 0.1, 0.3, 0.8, 1.4];

    let mut constant = [7.5; 5];
    hat_smooth_front(&abscissae, &mut constant).unwrap();
    let deviation = constant
        .iter()
        .map(|v| (v - 7.5).abs())
        .fold(0.0_f64, f64::max);
    println!("constant 7.5: maximum deviation = {deviation:.16e}");
    assert!(deviation < 1.0e-15);

    // A uniform front with mid-side nodes at the segment midpoints isolates the
    // end bias from the effect of off-centre mid-side nodes.
    let uniform = [0.0, 0.25, 0.5, 0.75, 1.0];
    let mut linear = uniform;
    hat_smooth_front(&uniform, &mut linear).unwrap();
    println!("uniform linear input  = {uniform:?}");
    println!(
        "uniform linear output = [{}]",
        linear.map(|v| format!("{v:.16}")).join(", ")
    );
    for i in 0..5 {
        println!("  node {i}: moved by {:+.16e}", linear[i] - uniform[i]);
    }

    // Interior corner node: exact on a linear field.
    assert!((linear[2] - uniform[2]).abs() < 1.0e-15);
    // Ends: biased inward by exactly one third of the corner-to-midside spacing.
    let spacing = uniform[1] - uniform[0];
    assert_relative_eq!(linear[0] - uniform[0], spacing / 3.0, max_relative = 1e-12);
    assert_relative_eq!(linear[4] - uniform[4], -spacing / 3.0, max_relative = 1e-12);

    // Malformed fronts.
    let mut three = [1.0, 2.0, 3.0];
    assert!(hat_smooth_front(&[0.0, 0.5, 1.0], &mut three).is_ok());
    assert!(hat_smooth_front(&[0.0, 0.5], &mut three).is_err());
    let mut four = [1.0, 2.0, 3.0, 4.0];
    assert!(hat_smooth_front(&[0.0, 0.3, 0.6, 1.0], &mut four).is_err());
}
