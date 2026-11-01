//! # V&V: wall friction and gravity in `HemPipe` (gh:#1061)
//!
//! Predictions written before the first run (2026-11-01), with results in
//! `verification_and_validation/hem_pipe_friction_gravity.md`:
//!
//! - closures without a viscosity and horizontal pipes are byte-identical to
//!   the source-free solver (checked by hashing Sod, and an IF97 break with
//!   friction ablated, on the old and new code: identical,
//!   `06b78ecca14dfc88` / `3627eddfbfad3e55`; not a test here);
//! - a uniform flow decelerates exactly by the closure;
//! - a hydrostatic column stays near rest.

use tampines_steam_tables::hem::wall_friction::darcy_friction_factor;
use tampines_steam_tables::hem::*;
use uom::si::angle::degree;
use uom::si::area::square_meter;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::pressure::{megapascal, pascal};
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

/// **Uniform flow decelerates by the friction closure.** IF97 liquid at
/// 1 MPa, 300 K, `u = 3` m/s, in every cell of a 1 m, 10-cell pipe of area
/// 0.01 m² (`D_h = 0.1128` m), roughness 1e-4 m, transmissive ends, so the
/// flux divergence is zero and only the friction source acts. One forward-
/// Euler step of 1e-6 s: the momentum change in a middle cell must equal
/// `−dt·ρ f u|u|/(2D_h)` with `f` from the four-region factor at that cell's
/// IF97 viscosity. Prediction: equal to 1e-9 relative. Ablated, the change is
/// zero. **Result (2026-11-01):** asserted.
#[test]
fn a_uniform_flow_decelerates_exactly_by_the_friction_closure() {
    let state = PrimitiveState::PressureTemperature {
        pressure: Pressure::new::<megapascal>(1.0),
        temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
        velocity: Velocity::new::<meter_per_second>(3.0),
    };
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.01),
        10,
    )
    .unwrap()
    .with_roughness(Length::new::<meter>(1.0e-4))
    .unwrap();
    let numerics = HemNumerics {
        time_integrator: TimeIntegrator::ForwardEuler,
        ..HemNumerics::default()
    };
    let build = || {
        let mut p = HemPipe::new(
            geo,
            Iapws97Hem,
            HemInitialCondition::Uniform(state),
            numerics,
        )
        .unwrap();
        p.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive)
            .unwrap();
        p.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive)
            .unwrap();
        p
    };
    let mut pipe = build();
    assert!(pipe.has_wall_friction());
    let c0 = pipe.cell_state(5).unwrap();
    let (rho, u) = (c0.density.value, c0.velocity.value);
    let mu = c0
        .dynamic_viscosity
        .expect("IF97 supplies a viscosity")
        .get::<pascal_second>();
    let d_h = geo
        .hydraulic_diameter_at(Length::new::<meter>(0.55))
        .get::<meter>();
    let f = darcy_friction_factor(rho * u * d_h / mu, 1.0e-4 / d_h, 1.0);
    let dt = 1.0e-6;
    pipe.step_by(Time::new::<second>(dt)).unwrap();
    let c1 = pipe.cell_state(5).unwrap();
    let measured = c1.density.value * c1.velocity.value - rho * u;
    let expect = -dt * rho * f * u * u / (2.0 * d_h);
    println!(
        "uniform flow: Re {:.0}, f {f:.5}, d(rho u) {measured:.6e} against {expect:.6e}",
        rho * u * d_h / mu
    );
    assert!(
        (measured / expect - 1.0).abs() < 1e-9,
        "{measured} vs {expect}"
    );

    let mut ablated = build().without_wall_friction();
    ablated.step_by(Time::new::<second>(dt)).unwrap();
    let c2 = ablated.cell_state(5).unwrap();
    assert_eq!(c2.density.value * c2.velocity.value, rho * u);
}

/// **A vertical column initialised hydrostatic stays near rest.** IF97 liquid
/// at 300 K, a 10 m column inclined 90° (`+x` upward), 50 cells, walls at
/// both ends, pressure `p(z) = 2 MPa − ρ₀ g z` with `ρ₀` the 2 MPa density;
/// run 20 ms (about three acoustic transits). The source-plus-KNP discrete
/// balance is not exactly well-balanced, so a small residual motion is
/// expected. Prediction: |u| < 1e-2 m/s and the mid-column `dp/dz` within 1 %
/// of `−ρ g`. **Result (2026-11-01):** printed and asserted.
#[test]
fn a_hydrostatic_column_stays_near_rest() {
    let n = 50;
    let l = 10.0;
    let rho0 = Iapws97Hem
        .state_p_t(
            Pressure::new::<megapascal>(2.0),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap()
        .density
        .value;
    let g = 9.80665;
    let cells: Vec<PrimitiveState> = (0..n)
        .map(|i| {
            let z = (i as f64 + 0.5) * l / n as f64;
            PrimitiveState::PressureTemperature {
                pressure: Pressure::new::<pascal>(2.0e6 - rho0 * g * z),
                temperature: ThermodynamicTemperature::new::<kelvin>(300.0),
                velocity: Velocity::new::<meter_per_second>(0.0),
            }
        })
        .collect();
    let geo = HemPipeGeometry::new(Length::new::<meter>(l), Area::new::<square_meter>(0.01), n)
        .unwrap()
        .with_incline(Angle::new::<degree>(90.0));
    let mut pipe = HemPipe::new(
        geo,
        Iapws97Hem,
        HemInitialCondition::PerCell(cells),
        HemNumerics::default(),
    )
    .unwrap();
    pipe.run_until(Time::new::<second>(0.02)).unwrap();
    let s = pipe.snapshot();
    let umax = s
        .cells
        .iter()
        .map(|c| c.velocity.value.abs())
        .fold(0.0, f64::max);
    let (a, b) = (s.cells[20].pressure.value, s.cells[30].pressure.value);
    let dz = 10.0 * l / n as f64;
    let rho_mid = s.cells[25].density.value;
    let grad = (b - a) / dz;
    let rel = grad / (-rho_mid * g) - 1.0;
    println!("hydrostatic column at 20 ms: max |u| {umax:.3e} m/s, dp/dz {grad:.2} Pa/m against -rho g {:.2} ({:+.4} %)", -rho_mid * g, 100.0 * rel);
    assert!(umax < 1e-2, "{umax}");
    assert!(rel.abs() < 0.01, "{rel}");
    assert!((pipe.geometry().incline().get::<degree>() - 90.0).abs() < 1e-9);
}

/// Invalid geometry inputs are refused.
#[test]
fn invalid_roughness_and_shape_factor_are_refused() {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.01),
        10,
    )
    .unwrap();
    assert!(geo.with_roughness(Length::new::<meter>(-1.0)).is_err());
    assert!(geo.with_shape_factor(0.0).is_err());
    let g2 = geo.with_shape_factor(1.5).unwrap();
    assert_eq!(g2.roughness().get::<meter>(), 0.0);
}
