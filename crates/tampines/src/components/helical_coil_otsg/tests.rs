//! Tests of the helical-coil OTSG march. The V&V ones print their numbers
//! (`-- --nocapture`) into the record
//! `crates/tampines/verification_and_validation/helical_coil_otsg_gh1157.md`;
//! the prediction each one is set against is there (§1), written before the
//! first run.

use super::correlations as corr;
use super::*;
use tuas_boussinesq_solver::heat_transfer_correlations::nusselt_number_correlations::pipe_correlations::gnielinski_correlation_liquids_fully_developed;

fn k(t: ThermodynamicTemperature) -> f64 {
    t.get::<kelvin>()
}

fn illustrative(options: OtsgOptions) -> HelicalCoilOtsg {
    HelicalCoilOtsg::new(
        HelicalCoilGeometry::illustrative(),
        OperatingPoint::illustrative(),
        options,
    )
    .expect("the illustrative case is valid")
}

/// A single-phase (all-liquid) case: the hot inlet (450 K) is below
/// `T_sat(4.5 MPa) = 530.5 K`, so nothing boils. Tube length held at
/// `length_m` whatever the coil diameter.
fn liquid_case(curvature_ratio: f64, length_m: f64, options: OtsgOptions) -> HelicalCoilOtsg {
    liquid_case_tubes(curvature_ratio, length_m, 90, options)
}

/// [`liquid_case`] with `tubes` parallel tubes (fewer tubes, more flow per
/// tube).
fn liquid_case_tubes(curvature_ratio: f64, length_m: f64, tubes: u32, options: OtsgOptions) -> HelicalCoilOtsg {
    let d_i = 0.014;
    let pitch = 0.03;
    // d_i / D_c = r with D_c = D_w (1 + (h/(π D_w))²) ≈ D_w for D_w ≫ h.
    let d_c = d_i / curvature_ratio;
    // Solve D_w (1 + (h/(π D_w))²) = D_c by fixed point.
    let mut d_w = d_c;
    for _ in 0..50 {
        let q = pitch / (std::f64::consts::PI * d_w);
        d_w = d_c / (1.0 + q * q);
    }
    let mut g = HelicalCoilGeometry {
        coil_diameter: Length::new::<meter>(d_w),
        pitch: Length::new::<meter>(pitch),
        turns: 1.0,
        parallel_tubes: tubes,
        ..HelicalCoilGeometry::illustrative()
    };
    g.turns = length_m / g.turn_length().get::<meter>();
    let op = OperatingPoint {
        water_inlet_temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
        hot_inlet_temperature: ThermodynamicTemperature::new::<kelvin>(450.0),
        ..OperatingPoint::illustrative()
    };
    HelicalCoilOtsg::new(g, op, options).expect("valid liquid case")
}

/// The constructor's ordinary path carries every physics term (the
/// workspace's "correct physics is the default" rule).
#[test]
fn correct_physics_is_the_default() {
    let o = OtsgOptions::default();
    assert!(o.curvature && o.nucleate_boiling && o.subcooled_boiling && o.dryout);
    assert!(o.gravity && o.acceleration && o.wall_resistance);
    assert_eq!(o.nodes, 200);
    let a = o
        .without_curvature()
        .without_nucleate_boiling()
        .without_subcooled_boiling()
        .without_dryout()
        .without_gravity()
        .without_acceleration()
        .without_wall_resistance()
        .with_nodes(10);
    assert!(!(a.curvature || a.nucleate_boiling || a.subcooled_boiling || a.dryout));
    assert!(!(a.gravity || a.acceleration || a.wall_resistance));
    assert_eq!(a.nodes, 10);
}

/// Derived geometry of the illustrative coil, and the refusals.
#[test]
fn geometry_is_derived_and_validated() {
    let g = HelicalCoilGeometry::illustrative();
    let turn = (std::f64::consts::PI * 0.1f64).hypot(0.03);
    assert!((g.turn_length().get::<meter>() - turn).abs() < 1e-12);
    assert!((g.tube_length().get::<meter>() - 108.0 * turn).abs() < 1e-9);
    assert!((g.tube_length().get::<meter>() - 34.08).abs() < 0.01, "the published 34 m");
    assert!((g.coil_height().get::<meter>() - 3.24).abs() < 1e-12);
    assert!((g.elevation_sine() - 0.03 / turn).abs() < 1e-15);
    let d_c = 0.1 * (1.0 + (0.03 / (std::f64::consts::PI * 0.1)).powi(2));
    assert!((g.curvature_ratio().get::<ratio>() - 0.014 / d_c).abs() < 1e-15);
    g.validate().expect("valid");
    // Generic, not `&dyn Fn` (workspace rule: no trait objects).
    fn bad<F: Fn(&mut HelicalCoilGeometry)>(g: HelicalCoilGeometry, f: F) -> bool {
        let mut b = g;
        f(&mut b);
        b.validate().is_err()
    }
    assert!(bad(g, |b| b.tube_outer_diameter = Length::new::<meter>(0.01)));
    assert!(bad(g, |b| b.pitch = Length::new::<meter>(0.01)));
    assert!(bad(g, |b| b.coil_diameter = Length::new::<meter>(0.015)));
    assert!(bad(g, |b| b.turns = 0.0));
    assert!(bad(g, |b| b.parallel_tubes = 0));
    let op = OperatingPoint::illustrative();
    op.validate().expect("valid");
    let mut o2 = op;
    o2.water_inlet_temperature = ThermodynamicTemperature::new::<kelvin>(600.0);
    assert!(o2.validate().is_err(), "water must enter subcooled");
    let mut o3 = op;
    o3.hot_inlet_temperature = ThermodynamicTemperature::new::<kelvin>(1200.0);
    assert!(o3.validate().is_err(), "above IF97 region 2");
    assert!(HelicalCoilOtsg::new(g, op, OtsgOptions::default().with_nodes(1)).is_err());
    for r in [
        WaterRegime::SubcooledLiquid,
        WaterRegime::SubcooledBoiling,
        WaterRegime::SaturatedBoiling,
        WaterRegime::PostDryout,
        WaterRegime::SuperheatedVapour,
    ] {
        assert_eq!(WaterRegime::from_code(r.code()), Some(r));
        assert!(!r.label().is_empty());
    }
    assert_eq!(WaterRegime::from_code(9), None);
}

/// The region query sees the helix the march integrates along: the
/// centreline is bore, `d_i/2 < d ≤ d_o/2` is wall, beyond is shell, the
/// helix axis and the space above the last turn are shell, and the distance
/// to the centreline at points on it is zero.
#[test]
fn the_region_query_sees_the_assembled_helix() {
    let g = HelicalCoilGeometry::illustrative();
    let n = g.turns;
    for phi in [0.0, 1.0, 17.3, 2.0 * std::f64::consts::PI * n] {
        let c = g.centreline_point(phi);
        assert!(g.distance_to_centreline(c) < 1e-9, "phi {phi}");
        assert_eq!(g.region_at(c), CoilRegion::Bore);
    }
    // Radially out from the centreline at phi = 30 (so not at an end).
    let phi = 30.0;
    let c = g.centreline_point(phi);
    let (s, co) = (phi.sin(), phi.cos());
    let at = |dr: f64| [c[0] + dr * co, c[1] + dr * s, c[2]];
    assert_eq!(g.region_at(at(0.0069)), CoilRegion::Bore);
    assert_eq!(g.region_at(at(0.0071)), CoilRegion::Wall);
    assert_eq!(g.region_at(at(0.0094)), CoilRegion::Wall);
    assert_eq!(g.region_at(at(0.0096)), CoilRegion::Shell);
    assert!((g.distance_to_centreline(at(0.004)) - 0.004).abs() < 1e-6);
    assert_eq!(g.region_at([0.0, 0.0, 1.0]), CoilRegion::Shell, "the helix axis");
    let top = g.coil_height().get::<meter>();
    assert_eq!(g.region_at([0.05, 0.0, top + 0.05]), CoilRegion::Shell, "above the coil");
    // Midway between two turns on the outer side: shell (pitch 30 mm > d_o).
    assert_eq!(g.region_at([0.05, 0.0, 0.3 + 0.015]), CoilRegion::Shell);
}

/// **P1 and P4.** The illustrative case: converged, energy closed, the
/// regimes in their physical order, no temperature cross, pressure falling;
/// and its numbers against the published HTR-10 design point (a comparison,
/// not validation: the geometry and `h_o` are invented).
#[test]
fn illustrative_case_solves_closes_and_orders_its_regimes() {
    let sol = illustrative(OtsgOptions::default()).solve().expect("solves");
    assert!(sol.converged);
    let out = sol.outlet();
    let c = sol.energy_closure.get::<ratio>();
    let x_dry = sol
        .points
        .iter()
        .find(|p| p.regime == WaterRegime::PostDryout)
        .map(|p| p.equilibrium_quality.get::<ratio>());
    let peak_wall = sol.points.iter().map(|p| k(p.inner_wall_temperature)).fold(0.0, f64::max);
    println!("P4 illustrative: duty water {:.4} MW, duty hot {:.4} MW, closure {:.3e}", sol.duty_water.get::<watt>() / 1e6, sol.duty_hot.get::<watt>() / 1e6, c);
    println!("P4 steam out {:.2} C at {:.4} MPa (x_eq {:.4}); helium out {:.2} C; marches {}", k(out.water_temperature) - 273.15, out.pressure.get::<pascal>() / 1e6, out.equilibrium_quality.get::<ratio>(), k(sol.hot_outlet_temperature) - 273.15, sol.shooting_iterations);
    println!("P4 dp total {:.2} kPa = friction {:.2} + gravity {:.2} + acceleration {:.2}", sol.pressure_drop().get::<pascal>() / 1e3, sol.pressure_drop_friction.get::<pascal>() / 1e3, sol.pressure_drop_gravity.get::<pascal>() / 1e3, sol.pressure_drop_acceleration.get::<pascal>() / 1e3);
    println!("P4 dryout at s = {:?} m, x_eq = {:?}; peak inner wall {:.1} C", sol.dryout_length.map(|l| l.get::<meter>()), x_dry, peak_wall - 273.15);
    let mut first: Vec<(WaterRegime, f64)> = Vec::new();
    for p in &sol.points {
        if first.last().map(|f| f.0) != Some(p.regime) {
            first.push((p.regime, p.length.get::<meter>()));
        }
    }
    println!("P4 regime sequence (first s, m): {first:?}");
    assert!(c.abs() <= 1e-8, "P1: closure {c}");
    // Regimes never go backwards along the tube.
    for w in sol.points.windows(2) {
        assert!(w[1].regime.code() >= w[0].regime.code(), "{:?} then {:?}", w[0].regime, w[1].regime);
        assert!(w[1].pressure <= w[0].pressure);
        assert!(w[1].hot_temperature >= w[0].hot_temperature);
    }
    for p in &sol.points {
        assert!(k(p.water_temperature) <= k(p.hot_temperature) + 1e-9);
        assert!(k(p.inner_wall_temperature) <= k(p.outer_wall_temperature) + 1e-9);
    }
    assert_eq!(sol.points.first().map(|p| p.regime), Some(WaterRegime::SubcooledLiquid));
    assert_eq!(out.regime, WaterRegime::SuperheatedVapour);
    assert!((k(sol.hot_inlet_returned) - 973.15).abs() < SHOOTING_TOLERANCE_K);
}

/// **P1 across the ablations**: each converges and closes, and each moves
/// the answer the way its physics says.
#[test]
fn ablations_close_and_move_the_answer_the_right_way() {
    let base = illustrative(OtsgOptions::default()).solve().expect("base");
    let run = |o: OtsgOptions| illustrative(o).solve().expect("ablation solves");
    let cases = [
        ("without_curvature", run(OtsgOptions::default().without_curvature())),
        ("without_nucleate_boiling", run(OtsgOptions::default().without_nucleate_boiling())),
        ("without_subcooled_boiling", run(OtsgOptions::default().without_subcooled_boiling())),
        ("without_dryout", run(OtsgOptions::default().without_dryout())),
        ("without_gravity", run(OtsgOptions::default().without_gravity())),
        ("without_acceleration", run(OtsgOptions::default().without_acceleration())),
        ("without_wall_resistance", run(OtsgOptions::default().without_wall_resistance())),
    ];
    let peak = |s: &OtsgSolution| s.points.iter().map(|p| k(p.inner_wall_temperature)).fold(0.0, f64::max);
    println!("ablation | duty MW | steam out C | dp kPa | peak wall C | dryout s m | closure");
    for (name, s) in std::iter::once(("default", base.clone())).chain(cases.iter().cloned()) {
        println!("{name} | {:.4} | {:.2} | {:.2} | {:.1} | {:?} | {:.2e}", s.duty_water.get::<watt>() / 1e6, k(s.outlet().water_temperature) - 273.15, s.pressure_drop().get::<pascal>() / 1e3, peak(&s) - 273.15, s.dryout_length.map(|l| (l.get::<meter>() * 100.0).round() / 100.0), s.energy_closure.get::<ratio>());
        assert!(s.converged && s.energy_closure.get::<ratio>().abs() <= 1e-8, "{name}");
    }
    let get = |n: &str| &cases.iter().find(|c| c.0 == n).expect("case").1;
    assert_eq!(get("without_gravity").pressure_drop_gravity.get::<pascal>(), 0.0);
    assert_eq!(get("without_acceleration").pressure_drop_acceleration.get::<pascal>(), 0.0);
    assert!(get("without_dryout").dryout_length.is_none());
    assert!(base.dryout_length.is_some(), "the default case dries out");
    // Curvature raises friction (same flow, steeper friction law).
    assert!(get("without_curvature").pressure_drop_friction < base.pressure_drop_friction);
    // A perfectly conducting wall can only lower the tube's outer wall
    // temperature and raise the duty.
    assert!(get("without_wall_resistance").duty_water >= base.duty_water);
}

/// **P2a–P2c.** The coil correlations at `d/D_c = 0` against the straight
/// tube: friction exactly; turbulent Nu against TUAS's straight-tube
/// Gnielinski (Blasius `f`, `Pr_w = Pr`); laminar Nu against 3.66.
#[test]
fn single_phase_straight_tube_limit_correlations() {
    for re in [1.0e4f64, 5.0e4, 1.0e5] {
        let pr = 3.0;
        let f = 0.3164 * re.powf(-0.25);
        let coil = corr::nusselt_turbulent_gnielinski(re, pr, 0.0);
        let straight = gnielinski_correlation_liquids_fully_developed(re, pr, pr, f);
        let ratio_measured = coil / straight;
        let ratio_predicted = re / (re - 1000.0);
        println!("P2b Re {re:.0}: coil limit {coil:.4}, TUAS straight {straight:.4}, ratio {ratio_measured:.6} (predicted {ratio_predicted:.6})");
        assert!((ratio_measured / ratio_predicted - 1.0).abs() < 1e-5);
    }
    let (re, pr) = (1000.0f64, 5.0f64);
    let lam = corr::nusselt_laminar_gnielinski(re, pr, 0.0);
    println!("P2c laminar coil limit at Re 1000, Pr 5: {lam:.4} against 3.66 ({:+.1} %)", (lam / 3.66 - 1.0) * 100.0);
    assert!((lam - (3.66 + 0.08 * re.sqrt() * pr.cbrt())).abs() < 1e-12);
    assert_eq!(corr::darcy_friction_coil(1500.0, 0.0), 64.0 / 1500.0);
    // The friction jump at Re_crit, quoted in the correlation's doc.
    let r = 0.1;
    let rc = corr::critical_reynolds_schmidt(r);
    let jump = corr::darcy_friction_turbulent_coil(rc, r) / corr::darcy_friction_laminar_mishra_gupta(rc, r) - 1.0;
    println!("friction jump at Re_crit = {rc:.0} (d/D_c = 0.1): {:+.2} %", jump * 100.0);
    assert!(jump.abs() < 0.15);
}

/// **P2d.** Whole march, all liquid: the coil against its own
/// `without_curvature` ablation as `d/D_c → 0`, at a fixed 20 m tube.
#[test]
fn single_phase_march_tends_to_the_straight_limit() {
    let straight = liquid_case(0.1, 20.0, OtsgOptions::default().without_curvature())
        .solve()
        .expect("straight");
    assert!(straight.points.iter().all(|p| p.regime == WaterRegime::SubcooledLiquid));
    let mut prev: Option<(f64, f64)> = None;
    for r in [1e-1, 1e-2, 1e-3, 1e-4, 1e-6] {
        let s = liquid_case(r, 20.0, OtsgOptions::default()).solve().expect("coil");
        // The straight run's geometry differs only in D_w, which without
        // curvature affects nothing but the helix angle (gravity).
        let s0 = liquid_case(r, 20.0, OtsgOptions::default().without_curvature()).solve().expect("s0");
        let df = (s.pressure_drop_friction / s0.pressure_drop_friction).get::<ratio>() - 1.0;
        let dq = (s.duty_water / s0.duty_water).get::<ratio>() - 1.0;
        let slope = prev.map(|(f0, _)| (df / f0).log10());
        println!("P2d d/D_c {r:.0e}: friction dp {:+.4e}, duty {:+.4e}, log10 ratio of successive friction gaps {slope:?}", df, dq);
        assert!(s.points.iter().all(|p| p.regime == WaterRegime::SubcooledLiquid));
        prev = Some((df, dq));
    }
    let (df, dq) = prev.expect("ran");
    assert!(df.abs() < 1e-2 && dq.abs() < 1e-2, "P2d at 1e-6: {df} {dq}");
    assert!(straight.converged);
}

/// Observed order from three runs at h, h/2, h/4.
fn order(a: f64, b: f64, c: f64) -> f64 {
    ((a - b) / (b - c)).abs().log2()
}

/// **P3a and P3b.** Axial grid convergence: two single-phase liquid cases
/// (P3a, 90 tubes, whose Reynolds number crosses the coil `Re_crit`, where
/// the friction factor jumps; P3a′, 30 tubes, turbulent throughout) and the
/// full boiling case (P3b).
#[test]
fn axial_grid_convergence() {
    let liquid = |tubes: u32, tag: &str| {
        let mut v = Vec::new();
        for n in [10usize, 20, 40, 80, 160] {
            let model = liquid_case_tubes(0.05, 20.0, tubes, OtsgOptions::default().with_nodes(n));
            let s = model.solve().expect("liquid");
            v.push((n, k(s.outlet().water_temperature), s.pressure_drop().get::<pascal>()));
        }
        let re = |t: f64| {
            let p = Pressure::new::<pascal>(4.5e6);
            let mu = tampines_steam_tables::prelude::mu_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(t), p)
                .get::<pascal_second>();
            let model = liquid_case_tubes(0.05, 20.0, tubes, OtsgOptions::default());
            model.mass_flux() * 0.014 / mu
        };
        let re_crit = corr::critical_reynolds_schmidt(liquid_case_tubes(0.05, 20.0, tubes, OtsgOptions::default()).r());
        println!("{tag} ({tubes} tubes): Re from {:.0} to {:.0}, Re_crit {:.0}", re(300.0), re(v[4].1), re_crit);
        for r in &v {
            println!("{tag} n {} outlet {:.9} K dp {:.4} Pa", r.0, r.1, r.2);
        }
        let pt = order(v[2].1, v[3].1, v[4].1);
        let pp = order(v[2].2, v[3].2, v[4].2);
        println!("{tag} observed order (40/80/160): outlet T {pt:.3}, dp {pp:.3}");
        (pt, pp)
    };
    let (pt, pp) = liquid(90, "P3a");
    let (pt2, pp2) = liquid(30, "P3a'");
    let mut boil = Vec::new();
    for n in [25usize, 50, 100, 200, 400, 800] {
        let s = illustrative(OtsgOptions::default().with_nodes(n)).solve().expect("boiling");
        boil.push((
            n,
            k(s.outlet().water_temperature),
            s.pressure_drop().get::<pascal>(),
            s.duty_water.get::<watt>(),
            s.dryout_length.map_or(f64::NAN, |l| l.get::<meter>()),
            s.energy_closure.get::<ratio>(),
        ));
    }
    for b in &boil {
        println!("P3b n {} steam out {:.6} K dp {:.3} Pa duty {:.2} W dryout s {:.5} m closure {:.2e}", b.0, b.1, b.2, b.3, b.4, b.5);
    }
    let m = boil.len();
    let bt = order(boil[m - 3].1, boil[m - 2].1, boil[m - 1].1);
    let bp = order(boil[m - 3].2, boil[m - 2].2, boil[m - 1].2);
    let bq = order(boil[m - 3].3, boil[m - 2].3, boil[m - 1].3);
    println!("P3b observed order (200/400/800): steam out {bt:.3}, dp {bp:.3}, duty {bq:.3}");
    println!("P3b 400 vs 800: steam out {:.4} K, dp {:.3e} relative", (boil[m - 2].1 - boil[m - 1].1).abs(), ((boil[m - 2].2 - boil[m - 1].2) / boil[m - 1].2).abs());
    // P3a's outlet temperature and P3a′ in both: second order. P3a's dp is
    // recorded, not gated: its Re_crit crossing makes it first order.
    assert!((1.7..=2.3).contains(&pt), "P3a outlet T: {pt}");
    assert!((1.7..=2.3).contains(&pt2) && (1.7..=2.3).contains(&pp2), "P3a': {pt2} {pp2}");
    let _ = pp;
    assert!(bt >= 0.7 && bp >= 0.7, "P3b: {bt} {bp}");
}

/// The stepper runs one march per step and reaches the same answer as
/// `solve`; a solution exists only after the first step.
#[test]
fn shooting_steps_one_march_at_a_time() {
    let model = illustrative(OtsgOptions::default().with_nodes(60));
    let mut s = model.shooting();
    assert!(s.solution().is_none());
    assert!(!s.step().expect("first"));
    assert_eq!(s.iterations(), 1);
    let first = s.solution().expect("a march");
    assert!(!first.converged);
    while !s.step().expect("steps") {}
    assert!(s.converged());
    assert!(s.step().expect("idempotent once converged"));
    let a = s.solution().expect("solution");
    let b = model.solve().expect("solve");
    assert_eq!(a, b);
    println!("marches to converge (60 cells): {}", a.shooting_iterations);
    assert!(a.shooting_iterations < 40);
}

/// A flow the tube cannot pass at that pressure is refused, not clamped:
/// the illustrative flows through ONE tube instead of 90 (G ≈ 22 700
/// kg/(m²s)). Three tubes still pass (measured: 1.76 MPa of friction, all
/// liquid); one cannot.
#[test]
fn an_exhausted_pressure_is_an_error() {
    let mut g = HelicalCoilGeometry::illustrative();
    g.parallel_tubes = 1;
    let r = HelicalCoilOtsg::new(g, OperatingPoint::illustrative(), OtsgOptions::default().with_nodes(40))
        .expect("valid")
        .solve();
    match &r {
        Err(TampinesError::Unphysical(m)) => println!("refused: {m}"),
        other => panic!("expected Unphysical, got {other:?}"),
    }
}

