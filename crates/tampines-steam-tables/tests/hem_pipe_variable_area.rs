//! # V&V: variable flow area in `HemPipe` (gh:#1058, RELAP-7 §2.1.1)
//!
//! Quasi-1-D finite volume: `V_i dW/dt = −(F A)_e + (F A)_w + [0, p_i(A_e −
//! A_w), 0]`. Predictions written before the first run (2026-11-01), in
//! `verification_and_validation/hem_pipe_variable_area.md`:
//!
//! 1. Uniform area byte-identical to the constant-area solver (checked by
//!    hashing a Sod run and an IF97 choked-break run on the old and new code:
//!    identical, `c79c9553e38e985d`; not a test here, since the old code is
//!    gone).
//! 2. A fluid at rest at uniform pressure in a nozzle stays at rest.
//! 3. Subsonic converging–diverging nozzle: pressure within 1 % of the
//!    isentropic area–Mach solution, mass flux uniform to < 1 %.
//! 4. Shocked nozzle: shock within 3 cells of the analytic position,
//!    post-shock pressure within 3 %.
//! 5. Mass and total energy conserved to round-off between walls.
//!
//! Results are recorded on each test and in the V&V doc.

use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

const GAMMA: f64 = 1.4;
const N: usize = 200;

fn nozzle() -> AreaProfile {
    AreaProfile::ConvergingDiverging {
        inlet_m2: 1.5,
        throat_m2: 1.0,
        outlet_m2: 1.5,
        throat_fraction: 0.5,
    }
}

fn gas(p: f64, rho: f64) -> PrimitiveState {
    PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(p),
        density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
        velocity: Velocity::new::<meter_per_second>(0.0),
    }
}

/// `A/A*` for Mach `m` (isentropic, γ = 1.4).
fn area_ratio(m: f64) -> f64 {
    let k = (GAMMA + 1.0) / (2.0 * (GAMMA - 1.0));
    (1.0 / m) * ((2.0 / (GAMMA + 1.0)) * (1.0 + 0.5 * (GAMMA - 1.0) * m * m)).powf(k)
}

/// Mach for `A/A*` on the subsonic (`supersonic = false`) or supersonic branch,
/// by bisection.
fn mach_from_area(ratio: f64, supersonic: bool) -> f64 {
    let (mut lo, mut hi) = if supersonic { (1.0, 10.0) } else { (1e-6, 1.0) };
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        let too_big_area = area_ratio(mid) > ratio;
        // Subsonic: A/A* falls with M; supersonic: it rises.
        if too_big_area != supersonic {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

fn p_over_p0(m: f64) -> f64 {
    (1.0 + 0.5 * (GAMMA - 1.0) * m * m).powf(-GAMMA / (GAMMA - 1.0))
}

fn mach_from_p(p_ratio: f64) -> f64 {
    ((2.0 / (GAMMA - 1.0)) * (p_ratio.powf(-(GAMMA - 1.0) / GAMMA) - 1.0)).sqrt()
}

/// Run the nozzle from rest to `t_end` with a reservoir inlet `(p0, T0 = 300
/// K)` and a fixed exit pressure; return cell centres, pressures and mass
/// fluxes `ρuA`.
fn run_nozzle(p_exit: f64, t_end: f64) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
    let p0 = 1.0e5;
    let air = IdealGasHem::air();
    let r = air.specific_gas_constant.value;
    let h0 = GAMMA / (GAMMA - 1.0) * r * 300.0;
    let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), N).unwrap();
    let rho_init = p_exit / (r * 300.0);
    let mut pipe = HemPipe::new(
        geo,
        air,
        HemInitialCondition::Uniform(gas(p_exit, rho_init)),
        HemNumerics::default(),
    )
    .unwrap();
    pipe.set_boundary(
        PipeEnd::Inlet,
        HemBoundary::Reservoir {
            stagnation_pressure: Pressure::new::<pascal>(p0),
            stagnation_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(h0),
        },
    )
    .unwrap();
    pipe.set_boundary(
        PipeEnd::Outlet,
        HemBoundary::FixedPressure(Pressure::new::<pascal>(p_exit)),
    )
    .unwrap();
    pipe.run_until(Time::new::<second>(t_end)).unwrap();
    let snap = pipe.snapshot();
    let x: Vec<f64> = snap.cell_centres.iter().map(|c| c.get::<meter>()).collect();
    let p: Vec<f64> = snap
        .cells
        .iter()
        .map(|c| c.pressure.get::<pascal>())
        .collect();
    let g: Vec<f64> = snap
        .cells
        .iter()
        .zip(&snap.cell_centres)
        .map(|(c, xc)| {
            c.density.value * c.velocity.value * geo.flow_area_at(*xc).get::<square_meter>()
        })
        .collect();
    (x, p, g)
}

/// **Geometry helpers.** The converging–diverging profile hits its three
/// areas; a circular pipe's hydraulic diameter is `√(4A/π)`; the wall area per
/// length of a uniform pipe is `2πR`, and of a linear (conical) one
/// `2πR√(1 + R'²)`. Invalid profiles are refused.
#[test]
fn area_profiles_and_their_geometry() {
    let l = Length::new::<meter>(1.0);
    let geo = HemPipeGeometry::with_area_profile(l, nozzle(), N).unwrap();
    let a = |x: f64| {
        geo.flow_area_at(Length::new::<meter>(x))
            .get::<square_meter>()
    };
    assert_eq!(a(0.0), 1.5);
    assert_eq!(a(0.5), 1.0);
    assert_eq!(a(1.0), 1.5);
    assert!(a(0.25) < 1.5 && a(0.25) > 1.0);
    assert!(!geo.is_uniform());
    assert_eq!(geo.flow_area().get::<square_meter>(), 1.5);
    assert_eq!(geo.area_profile(), nozzle());
    let pi = std::f64::consts::PI;
    let dh = geo
        .hydraulic_diameter_at(Length::new::<meter>(0.5))
        .get::<meter>();
    assert!((dh - (4.0 / pi).sqrt()).abs() < 1e-12);

    let uni = HemPipeGeometry::new(l, Area::new::<square_meter>(pi), 10).unwrap();
    assert!(uni.is_uniform());
    let wall = uni
        .wall_area_per_length_at(Length::new::<meter>(0.3))
        .get::<meter>();
    assert!((wall - 2.0 * pi).abs() < 1e-12, "{wall}");
    // A cone, radius 1 → 2 over 1 m: R' = 1, factor √2 (linear in R, so the
    // area profile is not linear; use a Linear area and check the slant factor
    // against the central difference it is defined by).
    let cone = HemPipeGeometry::with_area_profile(
        l,
        AreaProfile::Linear {
            inlet_m2: pi,
            outlet_m2: 4.0 * pi,
        },
        1000,
    )
    .unwrap();
    let x = 0.5;
    let r = |x: f64| ((pi + 3.0 * pi * x) / pi).sqrt();
    let rp = (r(x + 5e-4) - r(x - 5e-4)) / 1e-3;
    let expect = 2.0 * pi * r(x) * (1.0 + rp * rp).sqrt();
    let got = cone
        .wall_area_per_length_at(Length::new::<meter>(x))
        .get::<meter>();
    assert!((got / expect - 1.0).abs() < 1e-9, "{got} vs {expect}");

    let bad = |p| HemPipeGeometry::with_area_profile(l, p, 10).is_err();
    assert!(bad(AreaProfile::Uniform(0.0)));
    assert!(bad(AreaProfile::Linear {
        inlet_m2: 1.0,
        outlet_m2: -1.0
    }));
    assert!(bad(AreaProfile::ConvergingDiverging {
        inlet_m2: 1.5,
        throat_m2: 1.0,
        outlet_m2: 1.5,
        throat_fraction: 1.0
    }));
    assert!(HemPipeGeometry::with_area_profile(Length::new::<meter>(0.0), nozzle(), 10).is_err());
    assert!(HemPipeGeometry::with_area_profile(l, nozzle(), 0).is_err());
}

/// **At rest in a nozzle.** Uniform `p = 1e5` Pa, `ρ = 1.16` kg/m³, `u = 0`,
/// walls at both ends, 200 steps. The face pressure fluxes and the wall
/// force cancel except for round-off in `p·r_e − p·r_w` against `p·(r_e −
/// r_w)`. Prediction: stays at rest. **Result (2026-11-01):** the largest
/// speed is printed and must stay below 1e-9 m/s.
#[test]
fn a_fluid_at_rest_in_a_nozzle_stays_at_rest() {
    let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), N).unwrap();
    let mut pipe = HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Uniform(gas(1.0e5, 1.16)),
        HemNumerics::default(),
    )
    .unwrap();
    pipe.set_boundary(PipeEnd::Inlet, HemBoundary::Wall)
        .unwrap();
    pipe.set_boundary(PipeEnd::Outlet, HemBoundary::Wall)
        .unwrap();
    for _ in 0..200 {
        pipe.step().unwrap();
    }
    let umax = pipe
        .snapshot()
        .cells
        .iter()
        .map(|c| c.velocity.value.abs())
        .fold(0.0, f64::max);
    println!("at rest in a nozzle: max |u| after 200 steps = {umax:e} m/s");
    assert!(umax < 1e-9);
}

/// **Subsonic converging–diverging nozzle against the isentropic solution.**
/// Ideal-gas air, `A = 1.5 → 1.0 → 1.5` m² over 1 m (throat at 0.5 m), 200
/// cells, reservoir `p0 = 1e5` Pa, `T0 = 300` K, exit `p = 0.95e5` Pa, run
/// from rest for 200 ms. Analytic: exit Mach from `p_e/p0`, then `A*` from
/// `A_e/A*`, then the subsonic Mach at every cell centre from `A(x)/A*`.
/// Measures: worst relative pressure error, and the spread of `ρuA` over the
/// cells. Prediction: < 1 % and < 1 %.
///
/// **Protocol corrected 2026-11-01.** The first run stopped at 50 ms, assumed
/// steady after about 17 acoustic transits; it was not (mass flux 147 kg/s,
/// rising), and it missed at 1.56 % / 1.18 %. The flow is steady by 150 ms:
/// the worst pressure error is 0.3123 % at 150 ms and 0.3124 % at 300 ms. The
/// settle time was lengthened; the 1 % criterion was not touched.
///
/// **Result (2026-11-01), 200 ms:** worst pressure error 0.31 % (at the exit
/// cell), mass-flux spread 0.74 %: both held. Grid study (200 / 400 / 800
/// cells): worst error 0.31 / 0.18 / 0.18 %, at the throat on the finer grids.
/// **Recorded, not a defect of the area terms:** the steady mass flux is
/// 158.6 kg/s against the isentropic 157.24 (+0.9 %), and it does not fall
/// with the grid. The cause is the `Reservoir` inlet, which uses the
/// incompressible total pressure `p = p0 − ½ρu²` (upstream `totalPressure`
/// without `psi`/`gamma`). At the inlet Mach of about 0.3 that raises the
/// effective stagnation pressure by about 0.13 %, and with only a 5 % pressure
/// drop across the nozzle the mass flux, roughly ∝ √Δp, moves by about 1.3 %.
#[test]
fn subsonic_nozzle_follows_the_isentropic_area_mach_relation() {
    let (x, p, g) = run_nozzle(0.95e5, 0.2);
    let m_e = mach_from_p(0.95);
    let a_star = 1.5 / area_ratio(m_e);
    let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), N).unwrap();
    let mut worst = 0.0_f64;
    for (xi, pi) in x.iter().zip(&p) {
        let a = geo
            .flow_area_at(Length::new::<meter>(*xi))
            .get::<square_meter>();
        let m = mach_from_area(a / a_star, false);
        worst = worst.max((pi / (1.0e5 * p_over_p0(m)) - 1.0).abs());
    }
    let (gmin, gmax) = g
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(*v), hi.max(*v))
        });
    let spread = (gmax - gmin) / gmax;
    println!("subsonic nozzle: worst pressure error {:.4} %, mass-flux spread {:.4} %, mass flux {gmax:.3} kg/s", 100.0 * worst, 100.0 * spread);
    assert!(worst < 0.01, "pressure {worst}");
    assert!(spread < 0.01, "mass flux spread {spread}");
}

/// **Normal shock in the diverging part.** Same nozzle, exit `p/p0 = 0.75`
/// (between shock-at-exit 0.612 and fully subsonic 0.88, so a shock stands
/// inside). Analytic: bisect on the shock area `A_s` so that the subsonic
/// branch behind it, with `A*₂ = A_t·p01/p02`, reaches the exit pressure; then
/// `x_s` from the profile. Measured shock: the cell face with the largest
/// pressure rise in the diverging half. Post-shock check: the pressure 5
/// cells downstream of the analytic shock against the analytic subsonic value
/// there. Run 100 ms. Predictions: shock within 3 cells, post-shock within
/// 3 %. **Result (2026-11-01):** printed and asserted at the prediction.
#[test]
fn shocked_nozzle_places_the_shock_where_the_analysis_does() {
    let p_exit_ratio = 0.75;
    let (at, ae) = (1.0, 1.5);
    let p02_over_p01 = |m1: f64| {
        let a =
            ((GAMMA + 1.0) * m1 * m1 / ((GAMMA - 1.0) * m1 * m1 + 2.0)).powf(GAMMA / (GAMMA - 1.0));
        let b = ((GAMMA + 1.0) / (2.0 * GAMMA * m1 * m1 - (GAMMA - 1.0))).powf(1.0 / (GAMMA - 1.0));
        a * b
    };
    let exit_p = |a_s: f64| {
        let m1 = mach_from_area(a_s / at, true);
        let r0 = p02_over_p01(m1);
        let a_star2 = at / r0;
        r0 * p_over_p0(mach_from_area(ae / a_star2, false))
    };
    // exit_p falls as the shock moves downstream (larger A_s).
    let (mut lo, mut hi) = (at * 1.000_001, ae);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if exit_p(mid) > p_exit_ratio {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let a_s = 0.5 * (lo + hi);
    let x_s = 0.5 + 0.5 * ((a_s - at) / (ae - at)).sqrt();

    let (x, p, _) = run_nozzle(p_exit_ratio * 1.0e5, 0.1);
    let (k, _) = (N / 2..N - 1)
        .map(|i| (i, p[i + 1] - p[i]))
        .fold((0, f64::NEG_INFINITY), |b, c| if c.1 > b.1 { c } else { b });
    let x_num = 0.5 * (x[k] + x[k + 1]);
    let dx = 1.0 / N as f64;

    let m1 = mach_from_area(a_s / at, true);
    let r0 = p02_over_p01(m1);
    let a_star2 = at / r0;
    let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), N).unwrap();
    let j = x.iter().position(|xi| *xi > x_s + 5.0 * dx).unwrap();
    let a_j = geo
        .flow_area_at(Length::new::<meter>(x[j]))
        .get::<square_meter>();
    let p_post = 1.0e5 * r0 * p_over_p0(mach_from_area(a_j / a_star2, false));
    let e_post = p[j] / p_post - 1.0;
    println!(
        "shocked nozzle: analytic shock {x_s:.4} m (M1 {m1:.3}), numerical {x_num:.4} m ({:+.2} cells); post-shock p {:.0} vs {p_post:.0} Pa ({:+.2} %)",
        (x_num - x_s) / dx,
        p[j],
        100.0 * e_post
    );
    assert!(
        (x_num - x_s).abs() <= 3.0 * dx,
        "shock at {x_num}, analytic {x_s}"
    );
    assert!(e_post.abs() < 0.03, "post-shock {e_post}");
}

/// **Conservation between walls in a varying-area pipe.** A Riemann problem
/// (1e5 | 1e4 Pa) in the nozzle with walls at both ends, run 2 ms. Mass and
/// total energy must not change beyond round-off: the wall force does no work
/// on the total-energy equation and adds no mass. Prediction: < 1e-12
/// relative. **Result (2026-11-01):** printed and asserted.
#[test]
fn mass_and_energy_are_conserved_between_walls_with_a_varying_area() {
    let geo = HemPipeGeometry::with_area_profile(Length::new::<meter>(1.0), nozzle(), N).unwrap();
    let mut pipe = HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(0.3),
            left: gas(1.0e5, 1.0),
            right: gas(1.0e4, 0.125),
        },
        HemNumerics::default(),
    )
    .unwrap();
    pipe.set_boundary(PipeEnd::Inlet, HemBoundary::Wall)
        .unwrap();
    pipe.set_boundary(PipeEnd::Outlet, HemBoundary::Wall)
        .unwrap();
    let (m0, e0) = (pipe.total_mass().value, pipe.total_energy().value);
    pipe.run_until(Time::new::<second>(2.0e-3)).unwrap();
    let dm = pipe.total_mass().value / m0 - 1.0;
    let de = pipe.total_energy().value / e0 - 1.0;
    println!("walls, varying area: mass change {dm:e}, energy change {de:e}");
    assert!(dm.abs() < 1e-12 && de.abs() < 1e-12);
}
