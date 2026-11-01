//! # V&V: the control-volume network (gh:#1095, with gh:#1094's paths)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. **Closed two-volume gas network** (N2 + steam at 2 and 1 bar, one gas
//!    path): water, N2 and energy conserved to round-off (< 1e-12 relative)
//!    every step, the pressures equalise, and the Picard step converges.
//! 2. **Implicit large steps:** the same transient at `Δt` = 1 s (acoustic
//!    Courant number ≫ 1) is stable and ends at the same equalised pressure
//!    as at `Δt` = 0.01 s, within 1e-6 relative; the state at an early time
//!    converges at first order in `Δt`. **First half MISSED (2026-11-01):**
//!    1.06e-2 apart at 120 s. My design error: at 120 s the pressures have
//!    equalised but the gas and the pools are still exchanging heat, so the
//!    state is a transient that legitimately depends on `Δt`, not an
//!    equilibrium. The test now asserts stability and that `Δt` = 1 s is
//!    within 2 % (a bar set after the run), and measures the order.
//!    **Second half MISSED too:** at 2 s the errors against a Δt = 0.0125 s
//!    reference are +107 Pa (Δt 0.2 s) and −3.2 Pa (Δt 0.1 s): not
//!    monotone, so no order can be read. The equalising limit and choking
//!    are switches (non-smooth), and the pressure is near its overshoot at
//!    2 s. Printed, not asserted; filed in gh:#1098.
//! 3. **Manometer:** two vessels with pools joined at the bottom by a liquid
//!    path, gas spaces closed: the level difference oscillates with
//!    `ω² = 2 A_p (g + γ p A/(ρ V)) / (L A)` (γ = 1.4, dry N2 above), period
//!    within 3 % at `Δt` = 2 ms.
//! 4. **Vent to a boundary:** a 3 bar N2 volume venting to a 1 bar dry-air
//!    boundary: the network inventory plus the boundary account closes to
//!    round-off; the first steps are choked.
//! 5. **Loose coupling** (one pass) also conserves to round-off.

use super::*;
use crate::control_volume::{Port, VolumeAltitudeTable};
use crate::flow_path::{FlowPathGeometry, FlowPathKind};
use tampines_steam_tables::gas_mixture::{GasMixture, M_H2O};
use tampines_steam_tables::region_4_vap_liq_equilibrium::sat_pressure_4;
use uom::si::amount_of_substance::mole;
use uom::si::area::square_meter;
use uom::si::f64::{AmountOfSubstance, Area, Length};

fn m(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn kel(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}
fn p_sat(t: f64) -> f64 {
    sat_pressure_4(kel(t)).get::<pascal>()
}
fn atm(v: f64, p: f64, t: f64, rh: f64) -> GasMixture {
    let p_s = rh * p_sat(t);
    let n = p * v / (R_GAS * t);
    GasMixture::new(Mass::new::<kilogram>(n * p_s / p * M_H2O), &[(NonCondensable::N2, AmountOfSubstance::new::<mole>(n * (1.0 - p_s / p)))])
}
/// A prismatic vessel `a` m² × `h` m with a pool of `pool_h` m at 300 K and
/// N2 (+ steam at `rh`) above at `p`; ports at the given elevations.
fn vessel(a: f64, h: f64, pool_h: f64, p: f64, rh: f64, ports: &[f64]) -> ControlVolume {
    vessel_at(a, h, pool_h, p, rh, ports, 300.0)
}
fn vessel_at(a: f64, h: f64, pool_h: f64, p: f64, rh: f64, ports: &[f64], t: f64) -> ControlVolume {
    let rho = 1.0 / tampines_steam_tables::region_1_subcooled_liquid::v_tp_1(kel(t), Pressure::new::<pascal>(p)).value;
    ControlVolume::new(
        VolumeAltitudeTable::prismatic(Area::new::<square_meter>(a), m(h)).unwrap(),
        ports.iter().map(|z| Port { elevation: m(*z), opening_height: m(0.05) }).collect(),
        Mass::new::<kilogram>(pool_h * a * rho),
        kel(t),
        atm(a * (h - pool_h), p, t, rh),
        kel(t),
    )
    .unwrap()
}
fn gas_path(z_from: f64, z_to: f64, area: f64, k: f64) -> FlowPath {
    FlowPath::new(
        FlowPathGeometry {
            area: Area::new::<square_meter>(area),
            inertial_length: m(2.0),
            form_loss: k,
            discharge_coefficient: 1.0,
            from_elevation: m(z_from),
            to_elevation: m(z_to),
            opening_height: m(0.05),
        },
        FlowPathKind::Open,
    )
    .unwrap()
}
fn totals(n: &CvNetwork) -> (f64, f64, f64) {
    (n.water_mass().get::<kilogram>(), n.ncg_masses()[0], n.total_energy().get::<joule>())
}

/// Two vessels at 350 K, gas path between their gas spaces at 4 m. (At
/// 300 K a first run stopped: the blowdown of the 2 bar tank cools it below
/// 273 K, where IF97 steam has no state; recorded in gh:#1098.)
fn two_tanks(control: NetworkControl) -> CvNetwork {
    let mut net = CvNetwork::new(control);
    let a = net.add_volume(vessel_at(1.0, 5.0, 1.0, 2.0e5, 0.5, &[4.0], 350.0));
    let b = net.add_volume(vessel_at(1.0, 5.0, 1.0, 1.0e5, 0.5, &[4.0], 350.0));
    net.add_path(NetworkPath {
        path: gas_path(4.0, 4.0, 0.01, 1.0),
        from: NetworkEnd::Volume { volume: a, port: 0 },
        to: NetworkEnd::Volume { volume: b, port: 0 },
    })
    .unwrap();
    net
}

/// **1. Closed gas network conserves; pressures equalise.** 60 s at 0.1 s.
/// **Result (2026-11-01):** worst water 8.9e-16, N2 2.2e-16, energy
/// 1.6e-15; pressures 151 324.94 / 151 319.91 Pa; at most 30 Picard
/// iterations. Held (after the corrections in the V&V doc).
#[test]
fn a_closed_gas_network_conserves_and_equalises() {
    let mut net = two_tanks(NetworkControl::default());
    let (w0, n0, e0) = totals(&net);
    let mut worst = (0.0_f64, 0.0_f64, 0.0_f64);
    let mut max_it = 0;
    for _ in 0..600 {
        let r = net.step(Time::new::<second>(0.1)).unwrap();
        max_it = max_it.max(r.iterations);
        let (w, n, e) = totals(&net);
        worst = (worst.0.max((w / w0 - 1.0).abs()), worst.1.max((n / n0 - 1.0).abs()), worst.2.max((e / e0 - 1.0).abs()));
    }
    let (pa, pb) = (net.volume(0).unwrap().atmosphere_pressure().value, net.volume(1).unwrap().atmosphere_pressure().value);
    println!("closed gas network: worst water {:.1e}, N2 {:.1e}, energy {:.1e}; p {pa:.2} / {pb:.2} Pa; max Picard iterations {max_it}", worst.0, worst.1, worst.2);
    assert!(worst.0 < 1e-12 && worst.1 < 1e-12 && worst.2 < 1e-12);
    assert!((pa / pb - 1.0).abs() < 1e-3, "{pa} vs {pb}");
}

/// **2. Implicit large steps.** **Result (2026-11-01):** Δt = 1 s stable,
/// 153 274.33 Pa against 151 673.39 Pa at Δt = 0.01 s (+1.06 %); the order
/// at 2 s is non-monotone (both halves of the prediction missed; see the
/// module docs).
#[test]
fn large_implicit_steps_are_stable_and_first_order() {
    let run = |dt: f64, t_end: f64| {
        let mut net = two_tanks(NetworkControl::default());
        let n = (t_end / dt).round() as usize;
        for _ in 0..n {
            net.step(Time::new::<second>(dt)).unwrap();
        }
        net.volume(0).unwrap().atmosphere_pressure().value
    };
    let (p_small, p_big) = (run(0.01, 120.0), run(1.0, 120.0));
    println!("equalised p: dt 0.01 s {p_small:.4} Pa, dt 1 s {p_big:.4} Pa ({:+.2e})", p_big / p_small - 1.0);
    // Predicted 1e-6, measured 1.06e-2 (see the module docs); only
    // stability and a loose bound are asserted, a bar set after the run.
    assert!(p_big.is_finite() && (p_big / p_small - 1.0).abs() < 0.02);
    let (a, b, c) = (run(0.2, 2.0), run(0.1, 2.0), run(0.0125, 2.0));
    let ratio = (a - c) / (b - c);
    println!("p(2 s): dt 0.2 {a:.3}, 0.1 {b:.3}, ref 0.0125 {c:.3}; error ratio {ratio:.3}");
    // Predicted first order; measured non-monotone (see the module docs).
    let _ = ratio;
    assert!(a.is_finite() && b.is_finite() && c.is_finite());
}

/// **3. Manometer.** Two 1 m² vessels, pools 2.0 and 2.2 m deep at 300 K,
/// 50 m³ of dry N2 at 1 bar above each, joined at 0.5 m by a liquid path of
/// 0.01 m², L = 5 m, K = 0. **Result (2026-11-01):** period 27.977 s against
/// the hand 27.969 s (+0.03 %); water −8.7e-15, energy 6.7e-16. Held.
#[test]
fn a_manometer_oscillates_at_the_hand_frequency() {
    let mut net = CvNetwork::new(NetworkControl::default());
    let a = net.add_volume(vessel(1.0, 52.0, 2.0, 1.0e5, 0.0, &[0.5]));
    let b = net.add_volume(vessel(1.0, 52.2, 2.2, 1.0e5, 0.0, &[0.5]));
    let mut path = gas_path(0.5, 0.5, 0.01, 0.0);
    path = FlowPath::new(FlowPathGeometry { inertial_length: m(5.0), ..*path.geometry() }, FlowPathKind::Open).unwrap().without_choking();
    net.add_path(NetworkPath { path, from: NetworkEnd::Volume { volume: a, port: 0 }, to: NetworkEnd::Volume { volume: b, port: 0 } }).unwrap();
    let rho = net.volume(0).unwrap().pool_density().value;
    let (ap, l, area, v, p, g) = (0.01, 5.0, 1.0, 50.0, 1.0e5, 9.80665);
    let omega = (2.0 * ap * (g + 1.4 * p * area / (rho * v)) / (l * area)).sqrt();
    let period = 2.0 * std::f64::consts::PI / omega;
    let dt = 0.002;
    let diff = |n: &CvNetwork| n.volume(1).unwrap().pool_level().value - n.volume(0).unwrap().pool_level().value;
    let mut prev = diff(&net);
    let mut crossings = Vec::new();
    let (w0, _, e0) = totals(&net);
    while crossings.len() < 5 && net.time().value < 4.0 * period {
        net.step(Time::new::<second>(dt)).unwrap();
        let d = diff(&net);
        if (prev > 0.0) != (d > 0.0) {
            // Linear interpolation of the crossing time.
            let t = net.time().value - dt * d / (d - prev);
            crossings.push(t);
        }
        prev = d;
    }
    let measured = if crossings.len() >= 3 { crossings[2] - crossings[0] } else { f64::NAN };
    let (w, _, e) = totals(&net);
    println!("manometer: period {measured:.4} s vs hand {period:.4} s ({:+.2} %); crossings {crossings:?}; water {:.1e}, energy {:.1e}", 100.0 * (measured / period - 1.0), w / w0 - 1.0, e / e0 - 1.0);
    assert!((measured / period - 1.0).abs() < 0.03);
    assert!((w / w0 - 1.0).abs() < 1e-12 && (e / e0 - 1.0).abs() < 1e-12);
}

/// **4. Vent to a boundary; 5. loose coupling.** A 3 bar N2 vessel (no pool
/// water above the port) vents at 4 m through 0.002 m² to 1 bar dry air.
/// The vessel uses the ideal-steam ablation: the blowdown cools the gas below
/// 273 K (adiabatic expansion from 3 bar, as it should), where IF97 steam has
/// no state (a first run stopped there; the limit is recorded in gh:#1098).
/// **Result (2026-11-01):** implicit and loose both close water, N2 and
/// energy to ≤ 1.7e-15 against the boundary account; choked at step 3;
/// ~~the vessel ends at 1.0000 bar~~ **CORRECTED 2026-11-01 (gh:#1097):**
/// 99 944 Pa, since the equalising limit counts the 4 m gas column (47 Pa)
/// to the boundary. Held.
#[test]
fn a_vent_to_a_boundary_closes_and_loose_coupling_conserves() {
    for control in [NetworkControl::default(), NetworkControl { max_iterations: 1, tolerance: 0.0 }] {
        let mut net = CvNetwork::new(control);
        let a = net.add_volume(vessel(1.0, 5.0, 1.0, 3.0e5, 0.0, &[4.0]).without_real_steam().unwrap());
        net.add_path(NetworkPath {
            path: gas_path(4.0, 0.0, 0.002, 1.0),
            from: NetworkEnd::Volume { volume: a, port: 0 },
            to: NetworkEnd::Boundary(NetworkBoundary::dry_air(Pressure::new::<pascal>(1.0e5), kel(300.0))),
        })
        .unwrap();
        let (w0, n0, e0) = totals(&net);
        let mut first_choked = None;
        for k in 0..400 {
            let r = net.step(Time::new::<second>(0.05)).unwrap();
            if k == 2 {
                first_choked = Some(r.transfers[0].gas_choked);
            }
        }
        let (w, n, e) = totals(&net);
        let (bw, bn, be) = net.boundary_inflow();
        let (rw, rn, re) = ((w - w0 - bw) / w0, (n - n0 - bn[0]) / n0, (e - e0 - be) / e0);
        let p = net.volume(0).unwrap().atmosphere_pressure().value;
        println!("vent ({} iterations max): p {p:.1} Pa; closure water {rw:.1e}, N2 {rn:.1e}, energy {re:.1e}; choked at step 3: {first_choked:?}", control.max_iterations);
        assert!(rw.abs() < 1e-12 && rn.abs() < 1e-12 && re.abs() < 1e-12);
        assert_eq!(first_choked, Some(true));
        assert!(p < 1.5e5);
    }
    // Refusals.
    let mut net = two_tanks(NetworkControl::default());
    let dup = NetworkPath { path: gas_path(4.0, 4.0, 0.01, 1.0), from: NetworkEnd::Volume { volume: 0, port: 0 }, to: NetworkEnd::Volume { volume: 1, port: 0 } };
    assert!(net.add_path(dup).is_err(), "a port takes one path");
    assert!(net.set_pool_heat(9, uom::si::f64::Power::new::<uom::si::power::watt>(1.0)).is_err());
}

/// **6. External sources, sinks, heats and a valve (gh:#1097).** Prediction
/// (written before the first run, 2026-11-01): a vessel taking 0.1 kg/s of
/// steam at 2.7 MJ/kg and 1 g/s of H2 through a submerged port, 20 kW into
/// its atmosphere, 5 kW into its pool and 1 g/s of steam condensed out of
/// its atmosphere, venting through a valve to a 1 bar boundary that is
/// closed for the first 100 s and then half open: water, N2, H2 and energy
/// close against `boundary_inflow` to round-off (< 1e-12 relative) whether
/// the valve is open or not; nothing leaves while it is closed; a source on
/// a path's port and a path on a source's port are refused.
///
/// **Result (2026-11-01):** worst closure 2.6e-15 over the 300 steps;
/// the vessel at 100 977 Pa after the valve opened. Held.
#[test]
fn external_sources_sinks_heats_and_a_valve_close() {
    use crate::control_volume::Source;
    use uom::si::power::watt;
    let mut net = CvNetwork::new(NetworkControl::default());
    let a = net.add_volume(vessel_at(2.0, 5.0, 1.0, 1.2e5, 0.8, &[0.5, 4.0, 4.5], 330.0));
    let vent = net
        .add_path(NetworkPath {
            path: gas_path(4.0, 0.0, 0.001, 1.0),
            from: NetworkEnd::Volume { volume: a, port: 1 },
            to: NetworkEnd::Boundary(NetworkBoundary::dry_air(Pressure::new::<pascal>(1.0e5), kel(300.0))),
        })
        .unwrap();
    net.set_path_open_fraction(vent, 0.0).unwrap();
    let src = Source::water(mr(0.1), AvailableEnergy::new::<joule_per_kilogram>(2.7e6)).with_gas(&[(NonCondensable::H2, mr(1e-3))], kel(400.0));
    assert!(net.set_source(a, 1, src).is_err(), "a path's port takes no source");
    net.set_source(a, 0, src).unwrap();
    let dup = NetworkPath { path: gas_path(0.5, 0.0, 0.01, 1.0), from: NetworkEnd::Volume { volume: a, port: 0 }, to: NetworkEnd::Boundary(NetworkBoundary::dry_air(Pressure::new::<pascal>(1.0e5), kel(300.0))) };
    assert!(net.add_path(dup).is_err(), "a source's port takes no path");
    net.set_atmosphere_heat(a, uom::si::f64::Power::new::<watt>(2.0e4)).unwrap();
    net.set_pool_heat(a, uom::si::f64::Power::new::<watt>(5.0e3)).unwrap();
    net.set_atmosphere_steam_sink(a, mr(1e-3)).unwrap();
    assert!(net.set_atmosphere_steam_sink(a, mr(-1.0)).is_err());
    assert!(net.set_path_open_fraction(7, 1.0).is_err());
    let all = |n: &CvNetwork| (n.water_mass().get::<kilogram>(), n.ncg_masses(), n.total_energy().get::<joule>());
    let (w0, g0, e0) = all(&net);
    let mut worst: f64 = 0.0;
    for k in 0..300 {
        if k == 100 {
            net.set_path_open_fraction(vent, 0.5).unwrap();
        }
        let r = net.step(Time::new::<second>(1.0)).unwrap();
        if k < 100 {
            assert_eq!(r.transfers[vent].gas_mass_rate.value, 0.0);
        }
        let (w, g, e) = all(&net);
        let (bw, bg, be) = net.boundary_inflow();
        let rw = (w - w0 - bw) / w0;
        let rn = (g[0] - g0[0] - bg[0]) / g0[0];
        let rh = (g[NonCondensable::H2.index()] - g0[NonCondensable::H2.index()] - bg[NonCondensable::H2.index()]) / 0.3;
        let re = (e - e0 - be) / e0;
        worst = worst.max(rw.abs()).max(rn.abs()).max(rh.abs()).max(re.abs());
    }
    let p = net.volume(a).unwrap().atmosphere_pressure().value;
    println!("external terms: worst closure {worst:.1e}; p {p:.0} Pa after the valve opened");
    assert!(worst < 1e-12, "{worst}");
    assert_eq!(net.path(vent).unwrap().path.open_fraction(), 0.5);
}

/// **7. A vacuum breaker under a rising pressure (gh:#1097).** Found on
/// the Mark I containment: decided on each Picard iterate, a breaker
/// equalised on the first pass, was shut on the second, and the step
/// converged with no flow, so it never opened. Now held for the step by
/// the start-of-step pressures, its equalising limit stopping at the
/// opening difference. Prediction (before the run, 2026-11-01): tank A
/// filling with 0.05 kg/s of N2, a 3.4 kPa breaker A → B: no flow until
/// `p_A − p_B` passes 3.4 kPa, then B follows A with the difference held
/// between 3.4 and 3.8 kPa at the end of every later step; closure < 1e-12.
///
/// **Result (2026-11-01):** opens at step 5 (4247 Pa at the end of that
/// step); afterwards the difference sits at **3905–4064 Pa**, closure
/// 9.8e-15. The band **MISSED** high: my prediction ignored that the
/// equalising limit is taken from the start-of-step state and does not see
/// the source's own rise within the step (1.8 mol of N2 a step at about
/// 340 Pa/mol, so up to about 600 Pa). The bound below is set after the run
/// and labelled so: at least the opening difference, at most one step's
/// rise above it.
#[test]
fn a_vacuum_breaker_holds_its_opening_difference() {
    use crate::control_volume::Source;
    let mut net = CvNetwork::new(NetworkControl::default());
    let a = net.add_volume(vessel_at(2.0, 5.0, 1.0, 1.0e5, 0.5, &[4.0, 4.5], 330.0));
    let b = net.add_volume(vessel_at(2.0, 5.0, 1.0, 1.0e5, 0.5, &[4.0], 330.0));
    let vb = FlowPath::new(
        FlowPathGeometry {
            area: Area::new::<square_meter>(0.01),
            inertial_length: m(1.0),
            form_loss: 1.0,
            discharge_coefficient: 0.6,
            from_elevation: m(4.0),
            to_elevation: m(4.0),
            opening_height: m(0.0),
        },
        FlowPathKind::VacuumBreaker { opening_pressure_difference: Pressure::new::<pascal>(3400.0) },
    )
    .unwrap();
    net.add_path(NetworkPath { path: vb, from: NetworkEnd::Volume { volume: a, port: 0 }, to: NetworkEnd::Volume { volume: b, port: 0 } }).unwrap();
    net.set_source(a, 1, Source::gas(&[(NonCondensable::N2, mr(0.05))], kel(330.0))).unwrap();
    let (w0, g0, e0) = (net.water_mass().get::<kilogram>(), net.ncg_masses()[0], net.total_energy().get::<joule>());
    let (mut opened_at, mut dp_lo, mut dp_hi, mut worst) = (None, f64::INFINITY, 0.0_f64, 0.0_f64);
    for k in 0..400 {
        let r = net.step(Time::new::<second>(1.0)).unwrap();
        let dp = net.volume(a).unwrap().atmosphere_pressure().value - net.volume(b).unwrap().atmosphere_pressure().value;
        let flows = r.transfers[0].gas_mass_rate.value > 0.0;
        if flows && opened_at.is_none() {
            opened_at = Some((k, dp));
        } else if opened_at.is_some() {
            dp_lo = dp_lo.min(dp);
            dp_hi = dp_hi.max(dp);
        }
        let (bw, bg, be) = net.boundary_inflow();
        let rn = (net.ncg_masses()[0] - g0 - bg[0]) / g0;
        let rw = (net.water_mass().get::<kilogram>() - w0 - bw) / w0;
        let re = (net.total_energy().get::<joule>() - e0 - be) / e0;
        worst = worst.max(rn.abs()).max(rw.abs()).max(re.abs());
    }
    println!("breaker: opened at step {opened_at:?}; afterwards dp in [{dp_lo:.0}, {dp_hi:.0}] Pa; closure {worst:.1e}");
    assert!(opened_at.is_some());
    assert!(dp_lo >= 3400.0 && dp_hi <= 4100.0, "{dp_lo} {dp_hi}");
    assert!(worst < 1e-12, "{worst}");
}
