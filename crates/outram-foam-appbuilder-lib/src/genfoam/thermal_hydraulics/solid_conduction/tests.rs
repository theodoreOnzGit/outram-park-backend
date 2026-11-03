// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// NEW WORK (gh:#1149). This file is part of OUTRAM PARK, distributed under
// GPL-3.0 like the rest of this crate's GeN-Foam module.

//! Verification of the R-Z solid conduction field against closed-form
//! solutions (gh:#1149 cases (a)-(e)), plus unit checks of the pieces.
//!
//! Every V&V test states its methodology, the **prediction written before
//! the first run** (2026-11-03), and the measured result. Measured numbers
//! are from `cargo test --release` on 2026-11-03 at the commit that adds this
//! file.

use petir::mathf::RealMath;
use std::f64::consts::PI;

use uom::si::angle::radian;
use uom::si::f64::{
    Angle, HeatTransfer, Length, Ratio, TemperatureCoefficient, ThermalConductivity,
    ThermodynamicTemperature, Time, VolumetricPowerDensity,
};
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::power::watt;
use uom::si::ratio::ratio;
use uom::si::temperature_coefficient::per_kelvin;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;
use uom::si::volumetric_power_density::watt_per_cubic_meter;

use super::*;
use crate::genfoam::thermal_hydraulics::boundary_conditions::blackbody_radiation::STEFAN_BOLTZMANN_SIGMA;
use crate::genfoam::thermal_hydraulics::structure::units::VolumetricHeatCapacity;

fn m(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn kel(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(x)
}
fn lam(x: f64) -> ConductivityModel {
    ConductivityModel::constant(ThermalConductivity::new::<watt_per_meter_kelvin>(x))
}
fn cap(x: f64) -> HeatCapacityModel {
    HeatCapacityModel::constant(VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(x))
}
fn qv(x: f64) -> VolumetricPowerDensity {
    VolumetricPowerDensity::new::<watt_per_cubic_meter>(x)
}
fn wedge() -> RzGeometry {
    RzGeometry::Axisymmetric {
        wedge_angle: Angle::new::<radian>(0.1),
    }
}
fn region(name: &str, k: ConductivityModel, c: f64, q: f64) -> SolidRegion {
    SolidRegion {
        name: name.into(),
        conductivity: k,
        heat_capacity: cap(c),
        heat_source: qv(q),
    }
}

/// A long cylinder `0 <= r <= radius` (one axial cell, adiabatic ends), one
/// region, `n` radial cells.
fn cylinder(n: usize, radius: f64, k: ConductivityModel, q: f64, outer: SolidBoundary) -> SolidConductionField {
    let mesh = RzMesh::build(
        wedge(),
        AxisEdges::uniform(m(0.0), m(radius), n).unwrap(),
        AxisEdges::uniform(m(0.0), m(1.0), 1).unwrap(),
        1,
        |_, _| 0,
    )
    .unwrap();
    let b = RzBoundaries {
        inner: SolidBoundary::Adiabatic,
        outer,
        bottom: SolidBoundary::Adiabatic,
        top: SolidBoundary::Adiabatic,
    };
    SolidConductionField::new(mesh, vec![region("rod", k, 1.0e6, q)], b, vec![], kel(300.0)).unwrap()
}

/// Largest |T_cell - exact(r_cell)| over the cells of `f`.
fn max_error(f: &SolidConductionField, exact: impl Fn(f64) -> f64) -> f64 {
    (0..f.mesh().n_cells())
        .map(|c| {
            let r = f.mesh().cell_centre(c).0.get::<meter>();
            (f.temperature(c).get::<kelvin>() - exact(r)).abs()
        })
        .fold(0.0, f64::max)
}

fn order(e_coarse: f64, e_fine: f64) -> f64 {
    (e_coarse / e_fine).r_log2()
}

// ── (a) uniform q''' in a long cylinder ──────────────────────────────────────

/// V&V (a) — steady radial conduction in a long cylinder with uniform `q'''`.
///
/// **Methodology.** Exact solution `T(r) = T_s + q'''(R^2 - r^2)/(4 lambda)`.
/// `R = 0.5 m`, `lambda = 10 W/(m K)`, `q''' = 1e5 W/m^3`, one axial cell with
/// adiabatic ends, axis adiabatic (zero-area face), wedge 0.1 rad. Three
/// surface conditions, each with its exact surface temperature:
/// fixed `T_s = 600 K`; convective `h = 500`, `T_inf = 400 K`
/// (`T_s = T_inf + q'''R/(2h) = 450 K`); radiative `eps = 0.8`,
/// `T_inf = 300 K` (`eps sigma (T_s^4 - T_inf^4) = q'''R/2`). Meshes of
/// `N = 20, 40, 80` cells.
///
/// **Prediction (written before the run).** The conservative cylindrical FV
/// flux between two cell centres reproduces the parabola's exact temperature
/// difference (`q/(4 lambda)(r_{i+1}^2 - r_i^2) = q r_f dr/(2 lambda)`), so
/// interior differences are exact. The only error is the half-cell surface
/// link, which assumes a linear profile: every cell is offset by the same
/// `q''' dr^2 / (16 lambda)`, for all three surface conditions (each carries
/// the exact flux `q'''R/2`). At `N = 20` (`dr = 2.5 cm`) that is
/// **0.390625 K**; the observed order is **exactly 2**; the radiative
/// surface temperature equals the exact root to the Picard tolerance.
///
/// **Result (2026-11-03).** See the assertion messages, recorded here:
/// Pass, as predicted, to the digit. All three surface
/// conditions: max error 0.390625 / 0.09765625 / 0.0244140625 K at
/// N = 20 / 40 / 80, equal to `q dr^2/(16 lambda)` within 1e-6 K; observed
/// order 2.000000 and 2.000000; surface temperatures 600, 450 and
/// 864.756292074 K, each equal to the exact value to 1e-9 K; energy
/// imbalance at most 7e-14. Picard passes: 2 (fixed, convective), 13
/// (radiative). The radiative case first **failed to converge** (500 passes)
/// with a secant linearisation of the radiation; it now uses the tangent
/// (Newton) form, see `SolidConductionField`'s `robin`.
#[test]
fn a_uniform_source_cylinder_matches_the_parabola() {
    let (radius, k, q) = (0.5, 10.0, 1.0e5);
    let ts_rad = {
        // eps sigma (Ts^4 - Ta^4) = q R / 2, solved in closed form.
        let flux = q * radius / 2.0;
        (flux / (0.8 * STEFAN_BOLTZMANN_SIGMA) + 300.0_f64.r_powi(4)).r_powf(0.25)
    };
    let cases = [
        ("fixed", SolidBoundary::FixedTemperature(kel(600.0)), 600.0),
        (
            "convective",
            SolidBoundary::Convective {
                coefficient: HeatTransfer::new::<watt_per_square_meter_kelvin>(500.0),
                ambient: kel(400.0),
            },
            400.0 + q * radius / (2.0 * 500.0),
        ),
        (
            "radiative",
            SolidBoundary::Radiative {
                emissivity: Ratio::new::<ratio>(0.8),
                ambient: kel(300.0),
            },
            ts_rad,
        ),
    ];
    for (name, bc, ts) in cases {
        let mut errs = Vec::new();
        for n in [20, 40, 80] {
            let mut f = cylinder(n, radius, lam(k), q, bc);
            let rep = f.solve_steady().unwrap();
            let exact = |r: f64| ts + q * (radius * radius - r * r) / (4.0 * k);
            let e = max_error(&f, exact);
            let dr = radius / n as f64;
            let predicted = q * dr * dr / (16.0 * k);
            let surf = f.surface_temperature(RzPatch::Outer, 0).get::<kelvin>();
            let bal = f.energy_balance().relative_imbalance();
            println!(
                "(a) {name} N={n}: max err {e:.6e} K, predicted {predicted:.6e} K, surface {surf:.9} vs {ts:.9} K, picard {}, imbalance {bal:.2e}",
                rep.iterations
            );
            assert!((e - predicted).abs() < 1e-6, "{name} N={n}: {e} vs predicted {predicted}");
            assert!((surf - ts).abs() < 1e-6, "{name}: surface {surf} vs {ts}");
            assert!(bal < 1e-9, "{name}: imbalance {bal}");
            errs.push(e);
        }
        let p1 = order(errs[0], errs[1]);
        let p2 = order(errs[1], errs[2]);
        println!("(a) {name}: observed order {p1:.6} {p2:.6}");
        assert!((p1 - 2.0).abs() < 1e-3 && (p2 - 2.0).abs() < 1e-3);
    }
}

/// Code-to-code — the same R-Z mesh through basic-lib's
/// `steady_conduction::SteadyConductionProblem` (the gh:#949 solver, a
/// second route over the same `fvm::laplacian`).
///
/// **Methodology.** Case (a), fixed and convective surfaces, `N = 40`,
/// constant `lambda`. The two solvers share the Laplacian and the Robin
/// weight but not the assembly code around them, nor the Picard loop or the
/// CG tolerance (1e-13 there, 1e-14 here).
///
/// **Prediction (before the run).** Not bit-identical (different linear
/// tolerances), agreement within 1e-9 relative.
///
/// **Result (2026-11-03).** Pass. 0 of 40 cells bit-identical; worst relative
/// difference 3.1e-15 (fixed) and 6.8e-15 (convective), i.e. the CG
/// tolerance difference, round-off level and below the 1e-8 allowance.
#[test]
fn code_to_code_against_basic_lib_steady_conduction() {
    use outram_foam_basic_lib::steady_conduction::{
        PatchCondition, SteadyConductionProblem, SteadyConductionSettings, ThermalBoundary,
    };
    for (name, ours, theirs) in [
        (
            "fixed",
            SolidBoundary::FixedTemperature(kel(600.0)),
            ThermalBoundary::FixedTemperature(kel(600.0)),
        ),
        (
            "convective",
            SolidBoundary::Convective {
                coefficient: HeatTransfer::new::<watt_per_square_meter_kelvin>(500.0),
                ambient: kel(400.0),
            },
            ThermalBoundary::Convective {
                coefficient: HeatTransfer::new::<watt_per_square_meter_kelvin>(500.0),
                ambient: kel(400.0),
            },
        ),
    ] {
        let mut f = cylinder(40, 0.5, lam(10.0), 1.0e5, ours);
        f.solve_steady().unwrap();
        let cm = f.mesh().conduction_mesh().clone();
        let n_patches = cm.mesh.patches.len();
        let mut conds = vec![PatchCondition::uniform(ThermalBoundary::Adiabatic); n_patches];
        conds[RzPatch::Outer.index()] = PatchCondition::uniform(theirs);
        let p = SteadyConductionProblem::new(cm, vec![qv(1.0e5)], conds).unwrap();
        let s = p
            .solve(
                |_, _| ThermalConductivity::new::<watt_per_meter_kelvin>(10.0),
                &SteadyConductionSettings::default(),
            )
            .unwrap();
        let mut worst = 0.0_f64;
        let mut identical = 0;
        for c in 0..f.mesh().n_cells() {
            let a = f.temperature(c).get::<kelvin>();
            let b = s.temperature(c).get::<kelvin>();
            if a.to_bits() == b.to_bits() {
                identical += 1;
            }
            worst = worst.max(((a - b) / b).abs());
        }
        println!("code-to-code {name}: {identical}/40 bit-identical, worst relative {worst:.3e}");
        assert!(worst < 1e-9, "{name}: {worst}");
    }
}

// ── (b) composite cylinder with a radiating gap ──────────────────────────────

/// The exact composite-cylinder solution of case (b): returns
/// `(T_gap_inner_surface, T_gap_outer_surface, T(r))`.
fn composite_exact(
    q: f64,
    r1: f64,
    r2: f64,
    k1: f64,
    k2: f64,
    ts: f64,
    gap: &GapConductance,
) -> (f64, f64, impl Fn(f64) -> f64) {
    let flux = q * r1 / 2.0; // W/m^2 across the gap (per unit gap area)
    let t_out = ts + q * r1 * r1 / (2.0 * k2) * (r2 / r1).r_ln(); // outer side of the gap
    // Inner side: h_gap(Ti, To) (Ti - To) = flux, monotone in Ti -> bisection.
    let (mut lo, mut hi) = (t_out, t_out + 1.0e4);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let g = gap
            .coefficient(kel(mid), kel(t_out))
            .get::<watt_per_square_meter_kelvin>()
            * (mid - t_out);
        if g > flux {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let t_in = 0.5 * (lo + hi);
    let profile = move |r: f64| {
        if r < r1 {
            t_in + q * (r1 * r1 - r * r) / (4.0 * k1)
        } else {
            ts + q * r1 * r1 / (2.0 * k2) * (r2 / r).r_ln()
        }
    };
    (t_in, t_out, profile)
}

/// V&V (b) — two-layer composite cylinder with a radiating gas gap.
///
/// **Methodology.** Heated core `r < R1 = 0.3 m` (`lambda1 = 20`,
/// `q''' = 2e5 W/m^3`), unheated shell `R1 < r < R2 = 0.5 m` (`lambda2 = 30`),
/// outer surface fixed at 500 K. Between them a 5 mm gap of gas with
/// `lambda_g = 0.3 W/(m K)`, emissivities 0.8 (core) and 0.7 (shell).
/// Closed form: series resistances — the shell's log profile, the gap's
/// `h_gas + h_rad(Ti, To)` (the scalar `Ti` found by bisection to round-off;
/// the radiative part is not linearised), and the core's parabola. Meshes of
/// `dr = 2, 1, 0.5 cm` (15+10, 30+20, 60+40 cells).
///
/// **Prediction (before the run).** The core carries case (a)'s uniform
/// offset `q dr^2/(16 lambda1)` relative to its gap surface (0.0625 K at 1 cm);
/// the source-free shell adds the midpoint error of the log profile, about
/// `dT_shell (dr/r)^2 / 12 ~ 0.01 K`; the gap surface temperatures are each
/// one half-cell link from a cell and so carry `O(dr^2)` errors too. So:
/// max error **below 0.1 K at dr = 1 cm**, observed order **2.0 +/- 0.1**,
/// and the two gap surface temperatures within 0.1 K of the exact ones.
///
/// **Result (2026-11-03).** Pass. Max error 0.294 / 0.0736 / 0.0184 K at
/// dr = 2 / 1 / 0.5 cm (prediction: below 0.1 K at 1 cm, met); observed order
/// 1.9999 and 2.0000. Gap surfaces 894.8271 / 653.2655 K at 1 cm against
/// the exact 894.8160 / 653.2477 K (0.011 and 0.018 K); 20 Picard passes;
/// energy imbalance at most 6e-14.
#[test]
fn b_composite_cylinder_with_radiating_gap() {
    let (q, r1, r2, k1, k2, ts) = (2.0e5, 0.3, 0.5, 20.0, 30.0, 500.0);
    let gap = GapConductance::new(
        Ratio::new::<ratio>(0.8),
        Ratio::new::<ratio>(0.7),
        lam(0.3),
        m(0.005),
    )
    .unwrap();
    let (t_in, t_out, exact) = composite_exact(q, r1, r2, k1, k2, ts, &gap);
    let mut errs = Vec::new();
    for (n1, n2) in [(15, 10), (30, 20), (60, 40)] {
        let mesh = RzMesh::build(
            wedge(),
            AxisEdges::segments(m(0.0), &[(m(r1), n1), (m(r2), n2)]).unwrap(),
            AxisEdges::uniform(m(0.0), m(1.0), 1).unwrap(),
            2,
            |r, _| usize::from(r.get::<meter>() > r1),
        )
        .unwrap();
        let mut f = SolidConductionField::new(
            mesh,
            vec![region("core", lam(k1), 1e6, q), region("shell", lam(k2), 1e6, 0.0)],
            RzBoundaries {
                inner: SolidBoundary::Adiabatic,
                outer: SolidBoundary::FixedTemperature(kel(ts)),
                bottom: SolidBoundary::Adiabatic,
                top: SolidBoundary::Adiabatic,
            },
            vec![RegionGap {
                region_a: 0,
                region_b: 1,
                gap: gap.clone(),
            }],
            kel(ts),
        )
        .unwrap();
        let rep = f.solve_steady().unwrap();
        let e = max_error(&f, &exact);
        let gf = f.gap_faces();
        assert_eq!(gf.len(), 1);
        let (si, so) = f.gap_surface_temperatures(gf[0]);
        let (si, so) = (si.get::<kelvin>(), so.get::<kelvin>());
        let bal = f.energy_balance().relative_imbalance();
        println!(
            "(b) cells {n1}+{n2}: max err {e:.4e} K; gap surfaces {si:.4} / {so:.4} K vs exact {t_in:.4} / {t_out:.4} K; picard {}; imbalance {bal:.1e}",
            rep.iterations
        );
        assert!((si - t_in).abs() < 0.1 && (so - t_out).abs() < 0.1);
        assert!(bal < 1e-9);
        errs.push(e);
    }
    let (p1, p2) = (order(errs[0], errs[1]), order(errs[1], errs[2]));
    println!("(b) observed order {p1:.4} {p2:.4}");
    assert!(errs[1] < 0.1, "max error at 1 cm {}", errs[1]);
    assert!((p1 - 2.0).abs() < 0.1 && (p2 - 2.0).abs() < 0.1);
}

// ── (c) lambda = lambda0 (1 + beta T): the Kirchhoff transform ───────────────

/// V&V (c) — temperature-dependent conductivity against the Kirchhoff
/// transform.
///
/// **Methodology.** `lambda(T) = lambda0 (1 + beta T)`, `lambda0 = 10 W/(m K)`,
/// `beta = 1e-3 /K` (a [`PropertyModel::Linear`] referenced to 0 K). With
/// `U = T + beta T^2 / 2` (so `lambda0 grad U = lambda grad T`) the problem is
/// linear in `U`: `U(r) = U_s + q'''(R^2 - r^2)/(4 lambda0)`, inverted as
/// `T = (sqrt(1 + 2 beta U) - 1)/beta`. Cylinder of case (a), `T_s = 500 K`,
/// `q''' = 1e5`, `N = 20, 40, 80`. As a control, the same problem with a
/// constant `lambda` evaluated at the mean of `T_s` and the exact centre
/// temperature, to show the nonlinearity is resolved rather than averaged.
///
/// **Prediction (before the run).** Picard converges in under 30 passes;
/// the error is `O(dr^2)` with observed order **2.0 +/- 0.05**; at `N = 20`
/// it is near case (a)'s offset scaled by `lambda0/lambda(T)`, about
/// **0.2-0.4 K**. The constant-`lambda` control misses the centre by **tens
/// of K**.
///
/// **Result (2026-11-03).** Pass on order and convergence; the size
/// prediction was pessimistic. Max error 0.1241 / 0.0310 / 0.00775 K at
/// N = 20 / 40 / 80 (predicted 0.2-0.4 K at N = 20: the error came out
/// smaller than the crude `lambda0/lambda(T)` scaling suggested); observed
/// order 2.00054 and 2.00016; 10 Picard passes each. Exact centre
/// temperature 870.83 K. Control with constant `lambda = 16.854` misses by
/// **10.18 K** (predicted "tens of K": it is ten).
#[test]
fn c_linear_conductivity_matches_the_kirchhoff_transform() {
    let (radius, k0, beta, q, ts) = (0.5, 10.0, 1.0e-3, 1.0e5, 500.0);
    let model = ConductivityModel::linear(
        ThermalConductivity::new::<watt_per_meter_kelvin>(k0),
        TemperatureCoefficient::new::<per_kelvin>(beta),
        kel(0.0),
    );
    let us = ts + 0.5 * beta * ts * ts;
    let exact = |r: f64| {
        let u = us + q * (radius * radius - r * r) / (4.0 * k0);
        ((1.0 + 2.0 * beta * u).sqrt() - 1.0) / beta
    };
    let mut errs = Vec::new();
    for n in [20, 40, 80] {
        let mut f = cylinder(n, radius, model.clone(), q, SolidBoundary::FixedTemperature(kel(ts)));
        let rep = f.solve_steady().unwrap();
        let e = max_error(&f, exact);
        println!("(c) N={n}: max err {e:.6e} K, picard {}", rep.iterations);
        assert!(rep.iterations < 30);
        errs.push(e);
    }
    let (p1, p2) = (order(errs[0], errs[1]), order(errs[1], errs[2]));
    println!("(c) observed order {p1:.5} {p2:.5}; centre exact {:.4} K", exact(0.0));
    assert!((p1 - 2.0).abs() < 0.05 && (p2 - 2.0).abs() < 0.05);
    assert!(errs[0] < 0.4);

    let k_mean = k0 * (1.0 + beta * 0.5 * (ts + exact(0.0)));
    let mut ctl = cylinder(80, radius, lam(k_mean), q, SolidBoundary::FixedTemperature(kel(ts)));
    ctl.solve_steady().unwrap();
    let miss = max_error(&ctl, exact);
    println!("(c) control: constant lambda {k_mean:.4} misses by {miss:.3} K");
    assert!(miss > 10.0);
}

// ── (d) transient slab against the Fourier series ────────────────────────────

/// Exact temperature of a slab `0 <= x <= L`, adiabatic at 0, stepped to
/// `T_s` at `L` from a uniform `T_i` (Carslaw & Jaeger's series, the
/// separation-of-variables solution; 400 terms).
fn slab_series(x: f64, t: f64, l: f64, alpha: f64, ti: f64, ts: f64) -> f64 {
    let mut s = 0.0;
    for n in 0..400 {
        let ln = (2 * n + 1) as f64 * PI / (2.0 * l);
        let sign = if n % 2 == 0 { 1.0 } else { -1.0 };
        s += sign * 4.0 / ((2 * n + 1) as f64 * PI) * (ln * x).r_cos() * (-alpha * ln * ln * t).r_exp();
    }
    ts + (ti - ts) * s
}

fn slab(n: usize, l: f64, k: f64, c: f64) -> SolidConductionField {
    let mesh = RzMesh::build(
        RzGeometry::Planar { depth: m(1.0) },
        AxisEdges::uniform(m(0.0), m(l), n).unwrap(),
        AxisEdges::uniform(m(0.0), m(1.0), 1).unwrap(),
        1,
        |_, _| 0,
    )
    .unwrap();
    SolidConductionField::new(
        mesh,
        vec![region("slab", lam(k), c, 0.0)],
        RzBoundaries {
            inner: SolidBoundary::Adiabatic,
            outer: SolidBoundary::FixedTemperature(kel(400.0)),
            bottom: SolidBoundary::Adiabatic,
            top: SolidBoundary::Adiabatic,
        },
        vec![],
        kel(300.0),
    )
    .unwrap()
}

/// V&V (d) — transient slab against its series solution.
///
/// **Methodology.** Planar mesh, half-thickness `L = 0.1 m`, symmetry
/// (adiabatic) at `x = 0`, the face `x = L` stepped from 300 K to 400 K at
/// `t = 0`. `lambda = 10`, `rho c_p = 2e6`, so `alpha = 5e-6 m^2/s` and
/// `L^2/alpha = 2000 s`. Compared at `t = 400 s` (`Fo = 0.2`) against the
/// 400-term series. `N = 40` cells, backward Euler at `dt = 4, 2, 1 s`.
/// The temporal order is measured by Richardson on the same mesh,
/// `(T_4 - T_2)/(T_2 - T_1) -> 2` for a first-order scheme, which removes the
/// spatial error from the measurement.
///
/// **Prediction (before the run).** Backward Euler is first order in time,
/// so the Richardson ratio is **2.0 +/- 0.1** (observed order 1.0); at
/// `dt = 1 s` the error against the series is dominated by time stepping and
/// is **below 0.1 K** (about `dt/t * dT * O(1)` near the surface).
///
/// **Result (2026-11-03).** Pass. Max error against the series 0.1131 / 0.0608 /
/// 0.0348 K at dt = 4 / 2 / 1 s (below 0.1 K at 1 s, as predicted);
/// Richardson ratio 2.00955, i.e. temporal order 1.00687; at most 2 Picard
/// passes per step (constant properties).
#[test]
fn d_transient_slab_matches_the_series() {
    let (l, k, c, t_end) = (0.1, 10.0, 2.0e6, 400.0);
    let alpha = k / c;
    let mut runs = Vec::new();
    for dt in [4.0, 2.0, 1.0] {
        let mut f = slab(40, l, k, c);
        let steps = (t_end / dt) as usize;
        for _ in 0..steps {
            let rep = f.step(Time::new::<second>(dt)).unwrap();
            assert!(rep.iterations <= 2);
        }
        assert!((f.time().get::<second>() - t_end).abs() < 1e-9);
        let e = max_error(&f, |x| slab_series(x, t_end, l, alpha, 300.0, 400.0));
        println!("(d) dt={dt}: max err vs series {e:.5e} K");
        runs.push((f, e));
    }
    let n = runs[0].0.mesh().n_cells();
    let diff = |a: usize, b: usize| {
        (0..n)
            .map(|c| (runs[a].0.temperature(c).get::<kelvin>() - runs[b].0.temperature(c).get::<kelvin>()).abs())
            .fold(0.0, f64::max)
    };
    let rich = diff(0, 1) / diff(1, 2);
    println!("(d) Richardson ratio {rich:.5} -> temporal order {:.5}", rich.r_log2());
    assert!((rich - 2.0).abs() < 0.1);
    assert!(runs[2].1 < 0.1);
}

// ── (e) 2-D manufactured solution: mesh-convergence order ────────────────────

/// V&V (e) — mesh-convergence order on a genuinely 2-D axisymmetric field.
///
/// **Methodology.** Manufactured solution on `0 <= r <= R = 0.5`,
/// `0 <= z <= H = 1`:
/// `T = T0 + A (1 - r^2/R^2) sin(pi z / H)`, `T0 = 500 K`, `A = 200 K`,
/// `lambda = 15`. Its source
/// `q''' = lambda A sin(pi z/H) [4/R^2 + (pi/H)^2 (1 - r^2/R^2)]`
/// is set per cell at the cell centre ([`SolidConductionField::set_cell_heat_source`]).
/// `T0` fixed on the outer, bottom and top surfaces; the axis adiabatic.
/// Meshes `N x 2N`, `N = 8, 16, 32, 64`; max-norm and volume-weighted L2 errors
/// at cell centres.
///
/// **Prediction (before the run).** Second order: observed order of both
/// norms **2.0 +/- 0.15** on every refinement.
///
/// **Result (2026-11-03).** Pass. Linf 0.7487 / 0.1914 / 0.04835 / 0.01215 K and
/// L2 0.4964 / 0.1243 / 0.03109 / 0.007774 K at N = 8 / 16 / 32 / 64;
/// observed orders Linf 1.968, 1.985, 1.993 and L2 1.997, 1.999, 2.000.
#[test]
fn e_manufactured_solution_converges_at_second_order() {
    let (rr, hh, t0, a, k) = (0.5, 1.0, 500.0, 200.0, 15.0);
    let exact = |r: f64, z: f64| t0 + a * (1.0 - r * r / (rr * rr)) * (PI * z / hh).r_sin();
    let source = |r: f64, z: f64| {
        k * a * (PI * z / hh).r_sin() * (4.0 / (rr * rr) + (PI / hh).r_powi(2) * (1.0 - r * r / (rr * rr)))
    };
    let mut linf = Vec::new();
    let mut l2 = Vec::new();
    for n in [8, 16, 32, 64] {
        let mesh = RzMesh::build(
            wedge(),
            AxisEdges::uniform(m(0.0), m(rr), n).unwrap(),
            AxisEdges::uniform(m(0.0), m(hh), 2 * n).unwrap(),
            1,
            |_, _| 0,
        )
        .unwrap();
        let mut f = SolidConductionField::new(
            mesh,
            vec![region("body", lam(k), 1e6, 0.0)],
            RzBoundaries {
                inner: SolidBoundary::Adiabatic,
                outer: SolidBoundary::FixedTemperature(kel(t0)),
                bottom: SolidBoundary::FixedTemperature(kel(t0)),
                top: SolidBoundary::FixedTemperature(kel(t0)),
            },
            vec![],
            kel(t0),
        )
        .unwrap();
        let src: Vec<_> = (0..f.mesh().n_cells())
            .map(|c| {
                let (r, z) = f.mesh().cell_centre(c);
                qv(source(r.get::<meter>(), z.get::<meter>()))
            })
            .collect();
        f.set_cell_heat_source(&src).unwrap();
        f.solve_steady().unwrap();
        let fv = f.mesh().mesh().clone();
        let (mut emax, mut s2, mut vol) = (0.0_f64, 0.0, 0.0);
        for c in 0..f.mesh().n_cells() {
            let (r, z) = f.mesh().cell_centre(c);
            let e = f.temperature(c).get::<kelvin>() - exact(r.get::<meter>(), z.get::<meter>());
            emax = emax.max(e.abs());
            s2 += e * e * fv.cell_volumes[c];
            vol += fv.cell_volumes[c];
        }
        let e2 = (s2 / vol).sqrt();
        println!("(e) N={n}x{}: Linf {emax:.5e} K, L2 {e2:.5e} K", 2 * n);
        linf.push(emax);
        l2.push(e2);
    }
    for i in 0..3 {
        let (pi, p2) = (order(linf[i], linf[i + 1]), order(l2[i], l2[i + 1]));
        println!("(e) order Linf {pi:.4}, L2 {p2:.4}");
        assert!((pi - 2.0).abs() < 0.15 && (p2 - 2.0).abs() < 0.15);
    }
}

// ── Energy conservation of the enthalpy-form storage ─────────────────────────

/// V&V — the enthalpy-form storage conserves energy for a
/// temperature-dependent `rho c_p`.
///
/// **Methodology.** An adiabatic cylinder (all surfaces adiabatic), 10x6
/// cells, two regions with different `lambda`, uniform `q''' = 1e6 W/m^3`,
/// `rho c_p = 2e6 (1 + 2e-3 (T - 300))` in region 0 and a tabulated
/// `rho c_p` in region 1, plus a non-uniform initial field. Marched 20 steps of
/// 5 s. Exact: thermal energy gained `= q''' V t`.
///
/// **Prediction (before the run).** Relative discrepancy **below 1e-9**
/// (limited by the 1e-8 K Picard tolerance, not by the scheme); a
/// non-conservative `C(T^n) dT/dt` form would miss by order `beta dT`, i.e.
/// percent.
///
/// **Result (2026-11-03).** Pass. Gained 4.800000000e5 J against
/// 4.800000000e5 J expected; relative discrepancy 1.2e-14.
#[test]
fn enthalpy_form_storage_conserves_energy() {
    let mesh = RzMesh::build(
        wedge(),
        AxisEdges::uniform(m(0.0), m(0.4), 10).unwrap(),
        AxisEdges::uniform(m(0.0), m(0.6), 6).unwrap(),
        2,
        |r, _| usize::from(r.get::<meter>() > 0.25),
    )
    .unwrap();
    let c0 = HeatCapacityModel::linear(
        VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(2.0e6),
        TemperatureCoefficient::new::<per_kelvin>(2.0e-3),
        kel(300.0),
    );
    let c1 = HeatCapacityModel::tabulated(&[
        (kel(250.0), VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(1.0e6)),
        (kel(320.0), VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(1.5e6)),
        (kel(400.0), VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(2.5e6)),
    ])
    .unwrap();
    let regions = vec![
        SolidRegion {
            name: "a".into(),
            conductivity: lam(5.0),
            heat_capacity: c0,
            heat_source: qv(1.0e6),
        },
        SolidRegion {
            name: "b".into(),
            conductivity: lam(40.0),
            heat_capacity: c1,
            heat_source: qv(1.0e6),
        },
    ];
    let mut f = SolidConductionField::new(
        mesh,
        regions,
        RzBoundaries::all(SolidBoundary::Adiabatic),
        vec![],
        kel(300.0),
    )
    .unwrap();
    let init: Vec<_> = (0..f.mesh().n_cells()).map(|c| kel(290.0 + 0.5 * c as f64)).collect();
    f.set_temperatures(&init).unwrap();
    let e0 = f.thermal_energy(kel(300.0));
    let vol: f64 = f.mesh().mesh().cell_volumes.iter().sum();
    for _ in 0..20 {
        f.step(Time::new::<second>(5.0)).unwrap();
    }
    let gained = (f.thermal_energy(kel(300.0)) - e0).get::<uom::si::energy::joule>();
    let expected = 1.0e6 * vol * 100.0;
    let rel = (gained / expected - 1.0).abs();
    println!("energy: gained {gained:.9e} J, expected {expected:.9e} J, relative {rel:.2e}");
    assert!(rel < 1e-9);
}

// ── Unit checks of the pieces ────────────────────────────────────────────────

/// The axisymmetric mesh has the exact wedge volumes and areas, and its
/// lookup and region map agree with the cell centres.
#[test]
fn rz_mesh_geometry_and_lookup() {
    let theta = 0.1;
    let map = RegionMap {
        default_region: 0,
        rects: vec![
            RzRect {
                r: [m(0.2), m(0.5)],
                z: [m(0.0), m(1.0)],
                region: 1,
            },
            RzRect {
                r: [m(0.0), m(0.5)],
                z: [m(0.8), m(1.0)],
                region: 2,
            },
        ],
    };
    let mesh = RzMesh::from_region_map(
        wedge(),
        AxisEdges::segments(m(0.0), &[(m(0.2), 4), (m(0.5), 3)]).unwrap(),
        AxisEdges::uniform(m(0.0), m(1.0), 5).unwrap(),
        3,
        &map,
    )
    .unwrap();
    let fv = mesh.mesh();
    let vol: f64 = fv.cell_volumes.iter().sum();
    assert!((vol - 0.5 * theta * 0.25).abs() < 1e-15);
    let outer = &fv.patches[RzPatch::Outer.index()];
    let a: f64 = (0..outer.size).map(|i| fv.face_areas[outer.start + i]).sum();
    assert!((a - theta * 0.5).abs() < 1e-15);
    let inner = &fv.patches[RzPatch::Inner.index()];
    assert!((0..inner.size).all(|i| fv.face_areas[inner.start + i] == 0.0));
    assert_eq!(fv.patches[4].kind, outram_foam_basic_lib::mesh::PatchKind::Wedge);
    for c in 0..mesh.n_cells() {
        let (r, z) = mesh.cell_centre(c);
        assert_eq!(mesh.cell_at(r, z), Some(c));
        assert_eq!(mesh.cell_region(c), map.region_at(r, z));
    }
    assert_eq!(mesh.cell_region(mesh.cell_at(m(0.1), m(0.9)).unwrap()), 2);
    assert_eq!(mesh.cell_region(mesh.cell_at(m(0.3), m(0.5)).unwrap()), 1);
    assert_eq!(mesh.cell_at(m(0.6), m(0.5)), None);
    assert!(RzMesh::build(wedge(), AxisEdges::uniform(m(0.0), m(1.0), 2).unwrap(), AxisEdges::uniform(m(0.0), m(1.0), 2).unwrap(), 1, |_, _| 3).is_err());
    assert!(AxisEdges::segments(m(0.0), &[(m(0.0), 2)]).is_err());
}

/// Property models: exact integrals, clamping, and the `fn` hook.
#[test]
fn property_models_values_and_integrals() {
    let lin = PropertyModel::Linear {
        value_at_reference: 10.0,
        beta: 1e-3,
        reference_temperature: 0.0,
    };
    // int_300^500 10(1 + 1e-3 T) dT = 2000 + 5e-3 (500^2 - 300^2) = 2800.
    assert!((lin.integral(300.0, 500.0) - 2800.0).abs() < 1e-9);
    let poly = PropertyModel::Polynomial(vec![10.0, 1e-2]);
    assert!((poly.integral(300.0, 500.0) - 2800.0).abs() < 1e-9);
    assert!((poly.value(300.0) - lin.value(300.0)).abs() < 1e-12);
    fn f(t: f64) -> f64 {
        10.0 * (1.0 + 1e-3 * (t - 0.0))
    }
    let func = PropertyModel::Function(f);
    assert_eq!(func.value(437.25).to_bits(), lin.value(437.25).to_bits());
    assert!((func.integral(300.0, 500.0) - 2800.0).abs() < 1e-9);
    let tab = PropertyModel::Tabulated(PropertyTable::new(&[(300.0, 1.0), (400.0, 3.0)]).unwrap());
    assert_eq!(tab.value(200.0), 1.0);
    assert_eq!(tab.value(500.0), 3.0);
    assert!((tab.value(350.0) - 2.0).abs() < 1e-15);
    // 100 K clamped at 1, the ramp (200), 100 K clamped at 3.
    assert!((tab.integral(200.0, 500.0) - 600.0).abs() < 1e-12);
    assert!((tab.integral(500.0, 200.0) + 600.0).abs() < 1e-12);
    assert!(PropertyTable::new(&[(1.0, 1.0), (1.0, 2.0)]).is_err());
    assert!(PropertyTable::new(&[(1.0, 1.0)]).is_err());
    assert!(PropertyModel::Constant(2.0).is_constant() && !lin.is_constant());
    assert!((lam(3.0).at(kel(900.0)).get::<watt_per_meter_kelvin>() - 3.0).abs() < 1e-15);
    assert!((cap(7.0).at(kel(900.0)).get::<joule_per_cubic_meter_kelvin>() - 7.0).abs() < 1e-15);
}

/// The gap coefficient against a hand evaluation.
///
/// `eps = 0.8, 0.7`, `T = 900, 800 K`, vacuum (`lambda_g = 0`) then helium-like
/// `0.3 W/(m K)` over 5 mm: `h_rad = sigma (900^2+800^2)(1700) / (1/0.8 +
/// 1/0.7 - 1) = 5.670374419e-8 * 1.45e6 * 1700 / 1.678571... = 83.2709 W/(m^2 K)`;
/// `h_gas = 60`.
#[test]
fn gap_coefficient_hand_value() {
    let vac = GapConductance::new(Ratio::new::<ratio>(0.8), Ratio::new::<ratio>(0.7), lam(0.0), m(0.005)).unwrap();
    let h = vac.coefficient(kel(900.0), kel(800.0)).get::<watt_per_square_meter_kelvin>();
    let hand = 5.670_374_419e-8 * 1.45e6 * 1700.0 / (1.0 / 0.8 + 1.0 / 0.7 - 1.0);
    assert!((h - hand).abs() < 1e-12 * hand);
    assert!((h - 83.2709).abs() < 1e-3, "{h}");
    let he = GapConductance::new(Ratio::new::<ratio>(0.8), Ratio::new::<ratio>(0.7), lam(0.3), m(0.005)).unwrap();
    let h2 = he.coefficient(kel(900.0), kel(800.0)).get::<watt_per_square_meter_kelvin>();
    assert!((h2 - hand - 60.0).abs() < 1e-9);
    let hr = he.radiative_coefficient(kel(900.0), kel(800.0)).get::<watt_per_square_meter_kelvin>();
    assert!((hr - hand).abs() < 1e-12 * hand);
    // Radiation is symmetric in the surfaces for a parallel-plate gap.
    let flux_ab = (vac.coefficient_si(900.0, 800.0)) * 100.0;
    let flux_ba = vac.swapped().coefficient_si(800.0, 900.0) * 100.0;
    assert!((flux_ab - flux_ba).abs() < 1e-12 * flux_ab);
    assert!(GapConductance::new(Ratio::new::<ratio>(1.2), Ratio::new::<ratio>(0.7), lam(0.0), m(0.005)).is_err());
    assert!(GapConductance::new(Ratio::new::<ratio>(0.8), Ratio::new::<ratio>(0.7), lam(0.0), m(0.0)).is_err());
}

/// Constant-property path: converges on the second Picard pass, and a
/// [`PropertyModel::Linear`] with `beta = 0` (and a `fn` hook with the same
/// arithmetic) reproduces it bit for bit — the Picard machinery adds no
/// round-off of its own when the properties do not vary.
#[test]
fn constant_property_path_is_two_passes_and_bit_stable() {
    let solve = |k: ConductivityModel| {
        let mut f = cylinder(30, 0.5, k, 1.0e5, SolidBoundary::FixedTemperature(kel(600.0)));
        let rep = f.solve_steady().unwrap();
        (rep, (0..30).map(|c| f.temperature(c).get::<kelvin>().to_bits()).collect::<Vec<_>>())
    };
    let (rep, base) = solve(lam(10.0));
    assert_eq!(rep.iterations, 2);
    let (_, lin) = solve(ConductivityModel(PropertyModel::Linear {
        value_at_reference: 10.0,
        beta: 0.0,
        reference_temperature: 0.0,
    }));
    assert_eq!(base, lin);
    fn ten(_t: f64) -> f64 {
        10.0
    }
    let (_, func) = solve(ConductivityModel(PropertyModel::Function(ten)));
    assert_eq!(base, func);
}

/// Input validation and the error paths.
#[test]
fn invalid_inputs_are_rejected() {
    let mk = || {
        RzMesh::build(wedge(), AxisEdges::uniform(m(0.0), m(1.0), 3).unwrap(), AxisEdges::uniform(m(0.0), m(1.0), 3).unwrap(), 2, |r, _| usize::from(r.get::<meter>() > 0.5)).unwrap()
    };
    let regs = || vec![region("a", lam(1.0), 1.0, 1.0), region("b", lam(1.0), 1.0, 0.0)];
    let adi = RzBoundaries::all(SolidBoundary::Adiabatic);
    // Wrong region count.
    assert!(SolidConductionField::new(mk(), vec![region("a", lam(1.0), 1.0, 0.0)], adi, vec![], kel(300.0)).is_err());
    // Heated, all adiabatic: no steady state.
    let mut f = SolidConductionField::new(mk(), regs(), adi, vec![], kel(300.0)).unwrap();
    assert!(f.solve_steady().is_err());
    assert!(f.step(Time::new::<second>(0.0)).is_err());
    assert!(f.set_cell_heat_source(&[qv(1.0)]).is_err());
    assert!(f.set_temperatures(&[kel(1.0)]).is_err());
    // Bad emissivity and h.
    let bad = RzBoundaries {
        outer: SolidBoundary::Radiative {
            emissivity: Ratio::new::<ratio>(0.0),
            ambient: kel(300.0),
        },
        ..adi
    };
    assert!(SolidConductionField::new(mk(), regs(), bad, vec![], kel(300.0)).is_err());
    let bad_h = RzBoundaries {
        outer: SolidBoundary::Convective {
            coefficient: HeatTransfer::new::<watt_per_square_meter_kelvin>(-1.0),
            ambient: kel(300.0),
        },
        ..adi
    };
    assert!(SolidConductionField::new(mk(), regs(), bad_h, vec![], kel(300.0)).is_err());
    // A gap from a region to itself.
    let g = GapConductance::new(Ratio::new::<ratio>(0.8), Ratio::new::<ratio>(0.8), lam(0.1), m(0.01)).unwrap();
    assert!(SolidConductionField::new(mk(), regs(), adi, vec![RegionGap { region_a: 1, region_b: 1, gap: g }], kel(300.0)).is_err());
    // A conductivity that goes negative is reported, not used.
    let neg = ConductivityModel(PropertyModel::Linear {
        value_at_reference: 1.0,
        beta: -1.0,
        reference_temperature: 0.0,
    });
    let mut f = SolidConductionField::new(
        mk(),
        vec![region("a", neg, 1.0, 0.0), region("b", lam(1.0), 1.0, 0.0)],
        RzBoundaries {
            outer: SolidBoundary::FixedTemperature(kel(300.0)),
            ..adi
        },
        vec![],
        kel(300.0),
    )
    .unwrap();
    assert!(matches!(f.solve_steady(), Err(SolidConductionError::BadProperty { .. })));
    let mut p = SolidConductionField::new(mk(), regs(), RzBoundaries { outer: SolidBoundary::FixedTemperature(kel(300.0)), ..adi }, vec![], kel(300.0))
        .unwrap()
        .with_settings(SolidConductionSettings {
            max_picard_iterations: 1,
            ..SolidConductionSettings::default()
        });
    assert!(matches!(p.solve_steady(), Err(SolidConductionError::PicardNotConverged { .. })));
    let _ = (p.regions().len(), p.time(), p.mesh().geometry(), p.mesh().r_edges().edge(0), p.mesh().z_edges().n_cells(), Power::new::<watt>(0.0));
    assert!(p.temperature_at(m(0.1), m(0.1)).is_some());
}

use uom::si::f64::Power;

/// Robustness — the secant gap iteration on a hard case: a **vacuum** gap
/// (radiation only) carrying 150 kW/m^2 with several hundred K across it,
/// from a cold (400 K) start, with the gap declared from the shell's side
/// (exercises the swapped orientation).
///
/// **Prediction (before the run).** The secant map's slope is roughly
/// `-3 dT_gap / (4 T_mean)`, about `-0.2` here, so it converges, in a few
/// tens of passes, to the closed form's bisection root (within 0.5 K at
/// `dr = 1 cm`).
///
/// **Result (2026-11-03).** Pass. 44 Picard passes; gap surfaces
/// 1621.949 / 1166.327 K against the exact 1621.916 / 1166.238 K (456 K
/// across the gap; differences 0.03 and 0.09 K, the `O(dr^2)` mesh error).
#[test]
fn vacuum_gap_with_large_temperature_drop_converges() {
    let (q, r1, r2, k1, k2, ts) = (1.0e6, 0.3, 0.5, 20.0, 30.0, 400.0);
    let gap = GapConductance::new(Ratio::new::<ratio>(0.8), Ratio::new::<ratio>(0.6), lam(0.0), m(0.01)).unwrap();
    let (t_in, t_out, _) = composite_exact(q, r1, r2, k1, k2, ts, &gap);
    let mesh = RzMesh::build(
        wedge(),
        AxisEdges::segments(m(0.0), &[(m(r1), 30), (m(r2), 20)]).unwrap(),
        AxisEdges::uniform(m(0.0), m(1.0), 1).unwrap(),
        2,
        |r, _| usize::from(r.get::<meter>() > r1),
    )
    .unwrap();
    let mut f = SolidConductionField::new(
        mesh,
        vec![region("core", lam(k1), 1e6, q), region("shell", lam(k2), 1e6, 0.0)],
        RzBoundaries {
            outer: SolidBoundary::FixedTemperature(kel(ts)),
            ..RzBoundaries::all(SolidBoundary::Adiabatic)
        },
        vec![RegionGap {
            region_a: 1,
            region_b: 0,
            gap: gap.swapped(),
        }],
        kel(ts),
    )
    .unwrap();
    let rep = f.solve_steady().unwrap();
    let (si, so) = f.gap_surface_temperatures(f.gap_faces()[0]);
    println!(
        "vacuum gap: picard {}, surfaces {:.4} / {:.4} K vs exact {t_in:.4} / {t_out:.4} K",
        rep.iterations,
        si.get::<kelvin>(),
        so.get::<kelvin>()
    );
    assert!((si.get::<kelvin>() - t_in).abs() < 0.5 && (so.get::<kelvin>() - t_out).abs() < 0.5);
}

/// **Held cells (the gas side as a prescribed temperature), gh:#1149 part B.**
///
/// *Methodology.* A planar slab `0 <= x <= 1 m`, 10 cells, `k = 2 W/(m K)`,
/// adiabatic at `x = 0`, held at 300 K at `x = 1`. Cell 0 (centre
/// `x = 0.05 m`) is held at 400 K with
/// `SolidConductionField::set_fixed_temperatures`; it also carries a heat
/// source, which must be ignored. The exact solution of the free cells is
/// the straight line from `(0.05, 400)` to `(1, 300)`, which the
/// finite-volume scheme reproduces exactly, and the heat the held cell gives
/// up is `k A 100 / 0.95`.
///
/// *Prediction (written before the run).* Free cells on the line to
/// round-off (< 1e-9 K); `supplied_by_fixed_cells` equals the exact flux and
/// the removed heat to 1e-9 relative; `generated` is zero.
///
/// *Result (2026-11-03).* Pass, as predicted (the error and balance are
/// printed by the test).
#[test]
fn held_cells_are_eliminated_and_their_duty_is_reported() {
    let base = slab(10, 1.0, 2.0, 1.0e6);
    let mut f = SolidConductionField::new(
        base.mesh().clone(),
        base.regions().to_vec(),
        RzBoundaries {
            inner: SolidBoundary::Adiabatic,
            outer: SolidBoundary::FixedTemperature(kel(300.0)),
            bottom: SolidBoundary::Adiabatic,
            top: SolidBoundary::Adiabatic,
        },
        vec![],
        kel(350.0),
    )
    .unwrap();
    let mut src = vec![qv(0.0); f.mesh().n_cells()];
    src[0] = qv(5.0e4);
    f.set_cell_heat_source(&src).unwrap();
    f.set_fixed_temperatures(&[(0, kel(400.0))]).unwrap();
    assert!(f.is_fixed(0) && !f.is_fixed(1));
    f.solve_steady().unwrap();
    let mut err: f64 = 0.0;
    for c in 0..f.mesh().n_cells() {
        let x = f.mesh().cell_centre(c).0.get::<meter>();
        let exact = 400.0 - 100.0 * (x - 0.05) / 0.95;
        err = err.max((f.temperature(c).get::<kelvin>() - exact).abs());
    }
    let bal = f.energy_balance();
    let fv = f.mesh().mesh();
    let area = fv.face_areas[fv.patches[RzPatch::Outer.index()].start];
    let exact_q = 2.0 * area * 100.0 / 0.95;
    let supplied = bal.supplied_by_fixed_cells.get::<watt>();
    println!(
        "held cell: max error {err:.3e} K, supplied {supplied:.9} W vs exact {exact_q:.9} W, imbalance {:.2e}",
        bal.relative_imbalance()
    );
    assert!(err < 1e-9);
    assert!((supplied / exact_q - 1.0).abs() < 1e-9);
    assert_eq!(bal.generated.get::<watt>(), 0.0);
    assert!(bal.relative_imbalance() < 1e-9);
    assert!(f.set_fixed_temperatures(&[(999, kel(400.0))]).is_err());
    assert!(f.set_fixed_temperatures(&[(0, kel(-1.0))]).is_err());
    f.set_fixed_temperatures(&[]).unwrap();
    assert!(!f.is_fixed(0));
}

/// V&V (gh:#1150) — the per-cell implicit fluid exchange
/// (`set_cell_exchange`) against the closed-form fin solution.
///
/// **Methodology.** Planar slab `0 <= x <= L = 0.1 m`, `lambda = 2 W/(m K)`,
/// uniform source `q = 1e5 W/m^3`, adiabatic at `x = 0`, held at 400 K at
/// `x = L`, every cell exchanging with a fluid at 300 K through
/// `h_v = 2000 W/(m^3 K)`. With `theta = T - T_f - q/h_v` the equation is
/// `theta'' = m^2 theta`, `m = sqrt(h_v/lambda)`, so
/// `T = T_f + q/h_v + (400 - T_f - q/h_v) cosh(m x)/cosh(m L)`. 100 cells.
/// Pass: max error < 0.02 K (second-order cell-centred error, estimated
/// `(m dx)^2/12 * 50 K ~ 4e-3 K`), energy balance closes to 1e-9 with the
/// exchange counted, and a fully adiabatic body with an exchange is solvable.
///
/// **Result (2026-11-03, release):** see the printed line; recorded in
/// `verification_and_validation/htr10_rz_flow_fpic/README.md`.
#[test]
fn cell_exchange_matches_the_fin_solution() {
    use super::VolumetricHeatTransferCoefficient as Hv;
    let mut f = slab(100, 0.1, 2.0, 1.0e6);
    let n = f.mesh().n_cells();
    f.set_cell_heat_source(&vec![qv(1.0e5); n]).unwrap();
    let hv = Hv { dimension: std::marker::PhantomData, units: std::marker::PhantomData, value: 2000.0 };
    f.set_cell_exchange(&vec![(hv, kel(300.0)); n]).unwrap();
    f.solve_steady().unwrap();
    let mm = (2000.0_f64 / 2.0).sqrt();
    let mut err: f64 = 0.0;
    for c in 0..n {
        let x = f.mesh().cell_centre(c).0.get::<meter>();
        let exact = 300.0 + 50.0 + 50.0 * (mm * x).r_cosh() / (mm * 0.1).r_cosh();
        err = err.max((f.temperature(c).get::<kelvin>() - exact).abs());
    }
    let bal = f.energy_balance();
    println!(
        "cell exchange: max error {err:.3e} K, to fluid {:.6} W, generated {:.6} W, imbalance {:.2e}",
        bal.removed_to_fluid.get::<watt>(),
        bal.generated.get::<watt>(),
        bal.relative_imbalance()
    );
    assert!(err < 0.02, "{err}");
    assert!(bal.relative_imbalance() < 1e-9);
    // Fully adiabatic body: the exchange alone carries the heat away.
    let g0 = slab(10, 0.1, 2.0, 1.0e6);
    let mut g = SolidConductionField::new(
        g0.mesh().clone(),
        g0.regions().to_vec(),
        RzBoundaries::all(SolidBoundary::Adiabatic),
        vec![],
        kel(300.0),
    )
    .unwrap();
    g.set_cell_heat_source(&vec![qv(1.0e5); 10]).unwrap();
    g.set_cell_exchange(&vec![(hv, kel(300.0)); 10]).unwrap();
    g.solve_steady().unwrap();
    for c in 0..10 {
        assert!((g.temperature(c).get::<kelvin>() - 350.0).abs() < 1e-8);
    }
    assert!(g.set_cell_exchange(&[(hv, kel(300.0))]).is_err());
    let bad = Hv { value: -1.0, ..hv };
    assert!(g.set_cell_exchange(&vec![(bad, kel(300.0)); 10]).is_err());
}

/// The per-cell capacity factor (gh:#1151 part B) scales storage exactly.
///
/// **Methodology.** An adiabatic cylinder (10 cells, constant
/// `rho c = 1e6 J/(m^3 K)`, uniform 1e5 W/m^3) with factor 0.25 on every
/// cell heats at `q / (0.25 rho c) = 0.4 K/s`; 10 steps of 1 s must give
/// +4 K in every cell, and `thermal_energy` must return `q V t`. A steady
/// solve must not see the factor. **Prediction:** exact to round-off.
/// **Result (2026-11-03):** as predicted (asserted to 1e-9 K and 1e-12
/// relative).
#[test]
fn capacity_factor_scales_storage_only() {
    let mut f = cylinder(10, 0.5, lam(10.0), 1.0e5, SolidBoundary::Adiabatic);
    f.set_cell_capacity_factor(&[0.25; 10]).unwrap();
    for _ in 0..10 {
        f.step(Time::new::<second>(1.0)).unwrap();
    }
    for c in 0..10 {
        assert!((f.temperature(c).get::<kelvin>() - 304.0).abs() < 1e-9);
    }
    let vol: f64 = f.mesh().mesh().cell_volumes.iter().sum();
    let e = f.thermal_energy(kel(300.0)).get::<uom::si::energy::joule>();
    assert!((e / (1.0e5 * vol * 10.0) - 1.0).abs() < 1e-12);
    assert!(f.set_cell_capacity_factor(&[0.25; 3]).is_err());
    assert!(f.set_cell_capacity_factor(&[0.0; 10]).is_err());
    assert!(f.set_cell_capacity_factor(&[f64::NAN; 10]).is_err());
}
