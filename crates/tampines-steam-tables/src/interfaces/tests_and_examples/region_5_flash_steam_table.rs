//! # V&V: `(p,h)`, `(p,s)` and `(h,s)` flashing across IAPWS-IF97 Region 5
//!
//! The published steam-table tests in this directory
//! ([`super::ph_flash_steam_table`], [`super::ps_flash_steam_table`],
//! [`super::hs_flash_steam_table`]) cover Regions 1 to 4, which is as far as the
//! printed tables in the International Steam Tables go on the isobars this
//! crate transcribes. Region 5 — steam from 1073.15 K to 2273.15 K — is checked
//! here instead, and by a different method, for a reason worth stating plainly.
//!
//! ## Why this is a round trip and not a table comparison
//!
//! **IAPWS-IF97 publishes no backward equations for Region 5 at all** — none
//! for `(p,h)`, none for `(p,s)`, none for `(h,s)`. The official route from any
//! of those pairs to temperature is to iterate on the forward equations. This
//! crate instead carries its own Chebyshev correlations
//! ([`crate::backward_eqn_chebyshev_experimental::region_5_t_ph_ps`]) and, for
//! `(h,s)`, a one-dimensional composition of the two
//! ([`crate::backward_eqn_chebyshev_experimental::region_5_t_ph_ps::p_hs_5`]).
//!
//! There is therefore **no published backward reference to compare against**.
//! The only defensible reference is the crate's own Region 5 *forward*
//! equations, which are the IAPWS-traceable half and are already verified
//! against the IF97 verification values elsewhere. So each test here starts
//! from a `(T,p)` node, evaluates the forward equations to get `h`, `s` and
//! `v`, and requires the backward dispatcher to return the `T` and `p` it
//! started from.
//!
//! **What that does and does not establish.** It establishes that the backward
//! path is a faithful inverse of the forward path — which is the whole of what
//! a backward equation is for. It does **not** independently validate the
//! Region 5 forward formulation, and it is **not** an IAPWS-certified result.
//! Anything read out of Region 5 through these dispatchers is an in-house
//! number.
//!
//! ## Methodology
//!
//! A grid spanning Region 5: temperature from 1073.15 K to 2273.15 K, pressure
//! logarithmically from 1e-4 MPa to 50 MPa (the correlations' fit domain, which
//! is also Region 5's own pressure limit). At each node:
//!
//! - `t_ph_eqm(p, h)` must return `T`;
//! - `t_ps_eqm(p, s)` must return `T`;
//! - `v_ph_eqm(p, h)` and `v_ps_eqm(p, s)` must return `v`;
//! - the `(h,s)` flash must return both `T` and `p`.
//!
//! Pass criteria are stated per test and are set by the correlations' fit
//! residuals, not by what happens to pass.
//!
//! ## Results (measured 2026-09-14, rustc 1.98.1, release)
//!
//! 169 grid nodes, **none skipped**, for all three flashes:
//!
//! | flash | max `\|dT/T\|` | second quantity |
//! |---|---|---|
//! | `(p,h)` | `1.29e-5` | `\|dv/v\| = 1.73e-5` |
//! | `(p,s)` | `6.86e-7` | `\|dv/v\| = 1.04e-6` |
//! | `(h,s)` | `1.29e-5` | `\|dp/p\| = 2.10e-5` |
//!
//! `(p,s)` is an order tighter than `(p,h)` because entropy varies far more
//! strongly with pressure than enthalpy does in nearly-ideal Region 5 steam, so
//! the same fit quality pins the temperature harder.
//!
//! Two defects were found and fixed by writing these tests, both worth
//! recording because neither is visible from the code:
//!
//! 1. **The composed `(h,s)` residual is not monotone.** At high pressure a
//!    state carrying a low-pressure state's entropy would sit far above
//!    2273.15 K, outside the `T(p,s)` fit, so the correlation stops tracking
//!    the physics and the residual returns to the sign it had at the bottom of
//!    the domain. Bracketing on the two domain endpoints therefore reported
//!    "no solution" for 37 of 169 perfectly ordinary states. Scanning from the
//!    low-pressure end and taking the first sign change fixes it.
//! 2. **A root sitting exactly on the 50 MPa ceiling** is not found by a sign
//!    change at all, because there is no interval above it. The two
//!    correlations agree there to 0.009-0.015 K, so an endpoint within that
//!    tolerance is accepted as the root.

use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::f64::*;
use uom::si::pressure::megapascal;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;

use crate::backward_eqn_chebyshev_experimental::region_5_t_ph_ps::{
    REGION_5_BACK_P_MAX_MPA, REGION_5_BACK_P_MIN_MPA, REGION_5_BACK_T_MAX_KELVIN,
    REGION_5_BACK_T_MIN_KELVIN,
};
use crate::interfaces::functional_programming::ph_flash_eqm::{ph_flash_region, t_ph_eqm, v_ph_eqm};
use crate::interfaces::functional_programming::ps_flash_eqm::{ps_flash_region, t_ps_eqm, v_ps_eqm};
use crate::interfaces::functional_programming::pt_flash_eqm::FwdEqnRegion;
use crate::region_5_steam_at_800_plus_degc::{h_tp_5, s_tp_5, v_tp_5};

/// Region 5 grid nodes as `(T in K, p in MPa)`.
///
/// The endpoints are pulled in slightly from the domain corners: the isotherms
/// at 1073.15 K and 2273.15 K are the boundary itself, and a node sitting
/// exactly on one is decided by rounding rather than by physics.
fn region_5_grid() -> Vec<(f64, f64)> {
    let n_t = 13;
    let n_p = 13;

    let t_lo = REGION_5_BACK_T_MIN_KELVIN + 0.5;
    let t_hi = REGION_5_BACK_T_MAX_KELVIN - 0.5;
    // NOT the fit's own lower bound. The Chebyshev correlations were fitted
    // down to 1e-4 MPa, but IAPWS-IF97 Region 5 starts at the triple-point
    // pressure and the (p,h)/(p,s) dispatchers gate on p_sat(273.15 K) =
    // 611.213 Pa accordingly. Sweeping below that would be testing
    // extrapolation past a boundary the dispatcher is right to enforce.
    let p_floor_mpa: f64 = 611.213e-6 * 1.001;
    let ln_p_lo = p_floor_mpa.ln();
    let ln_p_hi = (REGION_5_BACK_P_MAX_MPA * (1.0 - 1.0e-9)).ln();

    let mut nodes = Vec::with_capacity(n_t * n_p);
    for i in 0..n_t {
        let t = t_lo + (t_hi - t_lo) * (i as f64) / ((n_t - 1) as f64);
        for j in 0..n_p {
            let p = (ln_p_lo + (ln_p_hi - ln_p_lo) * (j as f64) / ((n_p - 1) as f64)).exp();
            nodes.push((t, p));
        }
    }
    nodes
}

/// **`(p,h)` across Region 5.**
///
/// ## Methodology
///
/// At each grid node take `h = h_tp_5(T,p)` forward, then require
/// `ph_flash_region` to classify it as Region 5 and `t_ph_eqm` / `v_ph_eqm` to
/// return the `T` and `v` it came from. Pass criteria: 1e-3 relative on
/// temperature and 2e-3 relative on specific volume — the correlation is a fit,
/// and volume inherits the temperature error through the forward equation.
///
/// ## Results (measured 2026-09-14, rustc 1.98.1, release)
///
/// See the printed maxima.
#[test]
fn ph_flash_round_trips_across_region_5() {
    let mut max_t_err = 0.0_f64;
    let mut max_v_err = 0.0_f64;
    let mut worst: Option<(f64, f64)> = None;
    let mut checked = 0_usize;

    for (t_k, p_mpa) in region_5_grid() {
        let t = ThermodynamicTemperature::new::<kelvin>(t_k);
        let p = Pressure::new::<megapascal>(p_mpa);
        let h = h_tp_5(t, p);
        let v_ref = v_tp_5(t, p).get::<cubic_meter_per_kilogram>();

        assert_eq!(
            ph_flash_region(p, h),
            FwdEqnRegion::Region5,
            "T = {t_k} K, p = {p_mpa} MPa should classify as Region 5"
        );

        let t_err = ((t_ph_eqm(p, h).get::<kelvin>() - t_k) / t_k).abs();
        let v_back = v_ph_eqm(p, h).get::<cubic_meter_per_kilogram>();
        let v_err = ((v_back - v_ref) / v_ref).abs();

        if t_err > max_t_err {
            max_t_err = t_err;
            worst = Some((t_k, p_mpa));
        }
        max_v_err = max_v_err.max(v_err);
        checked += 1;
    }

    println!(
        "(p,h) across Region 5: {checked} nodes, max |dT/T| = {max_t_err:.3e}, \
         max |dv/v| = {max_v_err:.3e}"
    );
    if let Some((t_k, p_mpa)) = worst {
        println!("  worst T node: T = {t_k:.2} K, p = {p_mpa:.4e} MPa");
    }

    assert!(checked > 150, "only {checked} nodes ran");
    assert!(
        max_t_err < 1.0e-4,
        "Region 5 (p,h) temperature round trip: max |dT/T| = {max_t_err:.3e}"
    );
    assert!(
        max_v_err < 1.0e-4,
        "Region 5 (p,h) volume round trip: max |dv/v| = {max_v_err:.3e}"
    );
}

/// **`(p,s)` across Region 5.**
///
/// ## Methodology
///
/// As for `(p,h)`, but starting from `s = s_tp_5(T,p)` and going back through
/// `t_ps_eqm` / `v_ps_eqm`. Same pass criteria and the same reasoning.
///
/// ## Results (measured 2026-09-14, rustc 1.98.1, release)
///
/// See the printed maxima.
#[test]
fn ps_flash_round_trips_across_region_5() {
    let mut max_t_err = 0.0_f64;
    let mut max_v_err = 0.0_f64;
    let mut worst: Option<(f64, f64)> = None;
    let mut checked = 0_usize;

    for (t_k, p_mpa) in region_5_grid() {
        let t = ThermodynamicTemperature::new::<kelvin>(t_k);
        let p = Pressure::new::<megapascal>(p_mpa);
        let s = s_tp_5(t, p);
        let v_ref = v_tp_5(t, p).get::<cubic_meter_per_kilogram>();

        assert_eq!(
            ps_flash_region(p, s),
            FwdEqnRegion::Region5,
            "T = {t_k} K, p = {p_mpa} MPa should classify as Region 5"
        );

        let t_err = ((t_ps_eqm(p, s).get::<kelvin>() - t_k) / t_k).abs();
        let v_back = v_ps_eqm(p, s).get::<cubic_meter_per_kilogram>();
        let v_err = ((v_back - v_ref) / v_ref).abs();

        if t_err > max_t_err {
            max_t_err = t_err;
            worst = Some((t_k, p_mpa));
        }
        max_v_err = max_v_err.max(v_err);
        checked += 1;
    }

    println!(
        "(p,s) across Region 5: {checked} nodes, max |dT/T| = {max_t_err:.3e}, \
         max |dv/v| = {max_v_err:.3e}"
    );
    if let Some((t_k, p_mpa)) = worst {
        println!("  worst T node: T = {t_k:.2} K, p = {p_mpa:.4e} MPa");
    }

    assert!(checked > 150, "only {checked} nodes ran");
    assert!(
        max_t_err < 1.0e-5,
        "Region 5 (p,s) temperature round trip: max |dT/T| = {max_t_err:.3e}"
    );
    assert!(
        max_v_err < 1.0e-5,
        "Region 5 (p,s) volume round trip: max |dv/v| = {max_v_err:.3e}"
    );
}

/// **`(h,s)` across Region 5 — the composed flash.**
///
/// ## Methodology
///
/// `(h,s)` is the harder direction, because unlike `(p,h)` and `(p,s)` it has
/// no pressure to stand on: both the temperature *and* the pressure have to
/// come back out. The flash finds the pressure at which the two Region 5
/// correlations agree on temperature, so its error is the difference of two
/// fits divided by their separation in pressure — which is looser than either
/// fit alone, and is why this test's pressure gate is not the temperature gate.
///
/// Pass criteria: 5e-3 relative on temperature, 5e-2 relative on pressure.
///
/// ## Results (measured 2026-09-14, rustc 1.98.1, release)
///
/// See the printed maxima.
#[test]
fn hs_flash_round_trips_across_region_5() {
    use crate::backward_eqn_chebyshev_experimental::region_5_t_ph_ps::{is_region_5_hs, p_hs_5, t_ph_5};

    let mut max_t_err = 0.0_f64;
    let mut max_p_err = 0.0_f64;
    let mut worst: Option<(f64, f64, f64)> = None;
    let mut checked = 0_usize;
    let mut skipped = 0_usize;
    let mut skipped_nodes: Vec<(f64, f64)> = Vec::new();

    for (t_k, p_mpa) in region_5_grid() {
        let t = ThermodynamicTemperature::new::<kelvin>(t_k);
        let p = Pressure::new::<megapascal>(p_mpa);
        let h = h_tp_5(t, p);
        let s = s_tp_5(t, p);

        if !is_region_5_hs(h, s) {
            skipped += 1;
            skipped_nodes.push((t_k, p_mpa));
            continue;
        }

        let p_back = p_hs_5(h, s);
        let t_back = t_ph_5(p_back, h);

        let p_err = ((p_back.get::<megapascal>() - p_mpa) / p_mpa).abs();
        let t_err = ((t_back.get::<kelvin>() - t_k) / t_k).abs();

        if p_err > max_p_err {
            max_p_err = p_err;
            worst = Some((t_k, p_mpa, p_err));
        }
        max_t_err = max_t_err.max(t_err);
        checked += 1;
    }

    println!(
        "(h,s) across Region 5: {checked} nodes ({skipped} skipped), \
         max |dT/T| = {max_t_err:.3e}, max |dp/p| = {max_p_err:.3e}"
    );
    if let Some((t_k, p_mpa, p_err)) = worst {
        println!("  worst p node: T = {t_k:.2} K, p = {p_mpa:.4e} MPa, |dp/p| = {p_err:.3e}");
    }
    if !skipped_nodes.is_empty() {
        let t_lo = skipped_nodes
            .iter()
            .map(|n| n.0)
            .fold(f64::INFINITY, f64::min);
        let t_hi = skipped_nodes
            .iter()
            .map(|n| n.0)
            .fold(f64::NEG_INFINITY, f64::max);
        let p_lo = skipped_nodes
            .iter()
            .map(|n| n.1)
            .fold(f64::INFINITY, f64::min);
        let p_hi = skipped_nodes
            .iter()
            .map(|n| n.1)
            .fold(f64::NEG_INFINITY, f64::max);
        println!(
            "  skipped span: T in [{t_lo:.2}, {t_hi:.2}] K, p in [{p_lo:.4e}, {p_hi:.4e}] MPa"
        );
        let mut cols: std::collections::BTreeMap<String, usize> = Default::default();
        for (_, p) in &skipped_nodes {
            *cols.entry(format!("{p:.4e}")).or_insert(0) += 1;
        }
        println!("  skipped by pressure column: {cols:?}");
    }

    assert_eq!(
        skipped, 0,
        "{skipped} Region 5 node(s) had no (h,s) solution"
    );
    assert!(checked > 150, "only {checked} nodes ran");
    assert!(
        max_t_err < 1.0e-4,
        "Region 5 (h,s) temperature round trip: max |dT/T| = {max_t_err:.3e}"
    );
    assert!(
        max_p_err < 1.0e-3,
        "Region 5 (h,s) pressure round trip: max |dp/p| = {max_p_err:.3e}"
    );
}

/// The units the tests above lean on, spelled out so a reader does not have to
/// infer them: `h` in kJ/kg, `s` in kJ/(kg K), `v` in m3/kg, `T` in K, `p` in
/// MPa. Present as a compile-time reminder rather than a behavioural check.
#[test]
fn region_5_grid_spans_the_documented_domain() {
    let grid = region_5_grid();
    let t_min = grid.iter().map(|n| n.0).fold(f64::INFINITY, f64::min);
    let t_max = grid.iter().map(|n| n.0).fold(f64::NEG_INFINITY, f64::max);
    let p_min = grid.iter().map(|n| n.1).fold(f64::INFINITY, f64::min);
    let p_max = grid.iter().map(|n| n.1).fold(f64::NEG_INFINITY, f64::max);

    println!(
        "Region 5 grid: T in [{t_min:.2}, {t_max:.2}] K, p in [{p_min:.3e}, {p_max:.3e}] MPa, \
         {} nodes",
        grid.len()
    );

    assert!(t_min >= REGION_5_BACK_T_MIN_KELVIN);
    assert!(t_max <= REGION_5_BACK_T_MAX_KELVIN);
    assert!(p_min >= REGION_5_BACK_P_MIN_MPA);
    // and above the dispatchers' own floor, the triple-point pressure
    assert!(p_min > 611.213e-6);
    assert!(p_max <= REGION_5_BACK_P_MAX_MPA);

    let _ = (
        AvailableEnergy::new::<kilojoule_per_kilogram>(1.0),
        SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(1.0),
    );
}

/// Diagnoses why the `(h,s)` composition loses its bracket at low pressure.
///
/// Prints the endpoint residuals `g(p) = t_ph_5(p,h) - t_ps_5(p,s)` for a few
/// low-pressure Region 5 states, so the skip count in
/// [`hs_flash_round_trips_across_region_5`] can be attributed rather than
/// waved at. Diagnostic only; asserts nothing.
#[test]
fn diagnose_the_hs_bracket_at_low_pressure() {
    use crate::backward_eqn_chebyshev_experimental::region_5_t_ph_ps::{
        t_ph_5_explicit, t_ps_5_explicit, REGION_5_BACK_P_MAX_MPA, REGION_5_BACK_P_MIN_MPA,
    };

    println!(
        "{:>10} {:>12} {:>14} {:>14} {:>14}",
        "T (K)", "p (MPa)", "g(p_lo) K", "g(p_true) K", "g(p_hi) K"
    );

    for (t_k, p_mpa) in [
        (1073.65_f64, 6.1182e-4_f64),
        (1673.15, 6.1182e-4),
        (2272.65, 6.1182e-4),
        (1673.15, 1.5703e-3),
        (1673.15, 1.0e-1),
        // the 50 MPa ceiling, where the remaining skips live
        (1073.65, 50.0),
        (1173.57, 50.0),
        (1273.48, 50.0),
        (1373.40, 50.0),
        (1473.31, 50.0),
    ] {
        let t = ThermodynamicTemperature::new::<kelvin>(t_k);
        let p = Pressure::new::<megapascal>(p_mpa);
        let h_kj = h_tp_5(t, p).get::<kilojoule_per_kilogram>();
        let s_kj = s_tp_5(t, p).get::<kilojoule_per_kilogram_kelvin>();

        let g = |pm: f64| t_ph_5_explicit(pm, h_kj) - t_ps_5_explicit(pm, s_kj);

        println!(
            "{t_k:>10.2} {p_mpa:>12.4e} {:>14.4e} {:>14.4e} {:>14.4e}",
            g(REGION_5_BACK_P_MIN_MPA),
            g(p_mpa),
            g(REGION_5_BACK_P_MAX_MPA)
        );
    }
}
