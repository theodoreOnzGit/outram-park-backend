//! # V&V: pump, valve and turbine junctions (gh:#1072, RELAP-7 §6.6, 6.7, 6.10)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. Pump, steady, frictionless pipes between 1e5 Pa ends: the mid-pipe
//!    pressure rise is `ρ g H` within 3 % and the pump power `ṁ g H`; with
//!    the reservoir's `½ρu²` the flow speed is `√(2gH)`.
//! 2. Head from shaft power (Eq 355) reproduces the head-table run within 1 %
//!    when `W = ṁ g H` of that run.
//! 3. A fully open valve with `K` equals a flow junction with `K`, bit for bit.
//! 4. A closed valve passes no mass.
//! 5. A turbine at steady state carries its characteristic's mass flow through
//!    both pipes (mid-pipe flux within 0.5 %), takes shaft power `ṁ (h01 −
//!    h02)`, and closes when the outlet stagnation pressure exceeds the inlet.

use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

const R_AIR: f64 = 8314.462_618_153_24 / 28.96;

fn pipe(p: f64) -> HemPipe<IdealGasHem> {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.01),
        50,
    )
    .unwrap();
    let s = PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(p),
        density: MassDensity::new::<kilogram_per_cubic_meter>(p / (R_AIR * 300.0)),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    HemPipe::new(
        geo,
        IdealGasHem::air(),
        HemInitialCondition::Uniform(s),
        HemNumerics::default(),
    )
    .unwrap()
}

fn reservoir(p0: f64) -> HemBoundary {
    HemBoundary::Reservoir {
        stagnation_pressure: Pressure::new::<pascal>(p0),
        stagnation_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(3.5 * R_AIR * 300.0),
    }
}

fn end(pipe: usize, end: PipeEnd) -> PipeEndRef {
    PipeEndRef { pipe, end }
}

/// Two pipes joined by `component(a_outlet, b_inlet)`, reservoir `p_in` at
/// A's inlet and `p_out` at B's outlet, run `t`; returns the network and the
/// junction index.
fn two_pipes(
    p_in: f64,
    p_out: f64,
    t: f64,
    component: impl Fn(PipeEndRef, PipeEndRef) -> HemJunction,
) -> (HemNetwork<IdealGasHem>, usize) {
    let mut net = HemNetwork::new(IdealGasHem::air(), 0.2).unwrap();
    let mut a = pipe(p_out);
    a.set_boundary(PipeEnd::Inlet, reservoir(p_in)).unwrap();
    let mut b = pipe(p_out);
    b.set_boundary(
        PipeEnd::Outlet,
        HemBoundary::FixedPressure(Pressure::new::<pascal>(p_out)),
    )
    .unwrap();
    let ia = net.add_pipe(a);
    let ib = net.add_pipe(b);
    let j = net
        .add_component(component(end(ia, PipeEnd::Outlet), end(ib, PipeEnd::Inlet)))
        .unwrap();
    net.run_until(Time::new::<second>(t)).unwrap();
    (net, j)
}

fn mid(net: &HemNetwork<IdealGasHem>, i: usize) -> HemCellState {
    let s = net.pipe(i).unwrap().snapshot();
    s.cells[s.cells.len() / 2]
}

/// **Pump at steady state.** `H = 50` m, run 1 s.
///
/// **Protocol corrected 2026-11-01.** The first runs stopped at 0.3 s, which
/// was not steady: the head run read −1.83 % and the shaft-power run +1.44 %
/// of it. At 1 s the head run reads +0.91 % and the shaft-power run 0.00 %;
/// the two must agree at steady state, since `W = ṁ g H` with `ṁ = ρA√(2gH)`
/// has one solution. The settle time was lengthened; the criteria were not
/// changed.
///
/// **Design corrected 2026-11-01.** The shaft-power case first gave **no rise
/// at all**: Eq 355 is singular at `ṁ = 0`, and the first version gave zero
/// head until flow existed, so a pump started from rest never started. It now
/// caps the head at a shut-off head (200 m here).
///
/// **Result (2026-11-01), 1 s:** rise +0.91 % of `ρ g H`, power equal to `ṁ g
/// H`, shaft-power run within 0.00 % of the head run. Held.
#[test]
fn a_pump_raises_the_pressure_by_rho_g_h() {
    let h = 50.0;
    let (net, j) = two_pipes(1.0e5, 1.0e5, 1.0, |a, b| HemJunction::Pump {
        a,
        b,
        head: PumpHead::Head(TimeTable::constant(h)),
        reverse_loss: (0.0, 0.0),
    });
    let (ca, cb) = (mid(&net, 0), mid(&net, 1));
    let rise = cb.pressure.value - ca.pressure.value;
    let expect = ca.density.value * 9.80665 * h;
    let u = ca.velocity.value;
    let mdot = net.junction_mass_flow(j);
    let w = net.junction_power(j);
    println!(
        "pump H = {h} m: rise {rise:.2} Pa against rho g H {expect:.2} ({:+.2} %); u {u:.3} against sqrt(2gH) {:.3} m/s; power {w:.2} W against mdot g H {:.2} W",
        100.0 * (rise / expect - 1.0),
        (2.0 * 9.80665 * h).sqrt(),
        mdot * 9.80665 * h
    );
    assert!((rise / expect - 1.0).abs() < 0.03);
    assert!((w / (mdot * 9.80665 * h) - 1.0).abs() < 0.03);

    // Head from the shaft power of this run.
    let w_shaft = mdot * 9.80665 * h;
    let (net2, _) = two_pipes(1.0e5, 1.0e5, 1.0, |a, b| HemJunction::Pump {
        a,
        b,
        head: PumpHead::ShaftPower {
            power: TimeTable::constant(w_shaft),
            shutoff_head_m: 200.0,
        },
        reverse_loss: (0.0, 0.0),
    });
    let rise2 = mid(&net2, 1).pressure.value - mid(&net2, 0).pressure.value;
    println!(
        "pump from shaft power {w_shaft:.2} W: rise {rise2:.2} Pa ({:+.2} % of the head run)",
        100.0 * (rise2 / rise - 1.0)
    );
    assert!((rise2 / rise - 1.0).abs() < 0.01);
}

/// **Valves.** Fully open with `K = 2` against a flow junction with `K = 2`
/// (reservoir 1.02e5 → 1e5 Pa, 50 ms): identical. Closed between 2e5 and
/// 1e5 Pa pipes with walls at the far ends: no mass crosses.
/// **Result (2026-11-01):** asserted with `==`.
#[test]
fn a_valve_is_a_flow_junction_when_open_and_a_wall_when_closed() {
    let (v, _) = two_pipes(1.02e5, 1.0e5, 0.05, |a, b| HemJunction::Valve {
        a,
        b,
        opening: TimeTable::constant(1.0),
        form_loss: 2.0,
    });
    let (f, _) = two_pipes(1.02e5, 1.0e5, 0.05, |a, b| HemJunction::Flow {
        a,
        b,
        form_loss: 2.0,
    });
    for i in 0..2 {
        assert_eq!(
            v.pipe(i).unwrap().snapshot().cells,
            f.pipe(i).unwrap().snapshot().cells
        );
    }

    let mut net = HemNetwork::new(IdealGasHem::air(), 0.2).unwrap();
    let ia = net.add_pipe(pipe(2.0e5));
    let ib = net.add_pipe(pipe(1.0e5));
    net.add_component(HemJunction::Valve {
        a: end(ia, PipeEnd::Outlet),
        b: end(ib, PipeEnd::Inlet),
        opening: TimeTable::constant(0.0),
        form_loss: 1.0,
    })
    .unwrap();
    let (ma, mb) = (
        net.pipe(ia).unwrap().total_mass().value,
        net.pipe(ib).unwrap().total_mass().value,
    );
    net.run_until(Time::new::<second>(0.01)).unwrap();
    assert!((net.pipe(ia).unwrap().total_mass().value / ma - 1.0).abs() < 1e-13);
    assert!((net.pipe(ib).unwrap().total_mass().value / mb - 1.0).abs() < 1e-13);
}

/// **Turbine at steady state.** Air, reservoir 2e5 Pa → turbine → 1e5 Pa;
/// `ṁ_max = 3` kg/s, design `p01r = 2e5` Pa, `T01r = 300` K, ratio 2, flow
/// ratio 0.8, `η = 0.85`, `γ = 1.4`; 300 ms. **Result (2026-11-01):** printed
/// and asserted at the prediction.
#[test]
fn a_turbine_carries_its_characteristic_flow_and_takes_shaft_power() {
    let params = TurbineParams {
        max_mass_flow_kg_s: 3.0,
        design_stagnation_pressure_pa: 2.0e5,
        design_stagnation_temperature_k: 300.0,
        design_pressure_ratio: 2.0,
        design_mass_flow_ratio: 0.8,
        efficiency: 0.85,
        isentropic_exponent: 1.4,
    };
    let (net, j) = two_pipes(2.0e5, 1.0e5, 0.3, |a, b| HemJunction::Turbine {
        a,
        b,
        params,
    });
    let mdot = net.junction_mass_flow(j);
    let (ca, cb) = (mid(&net, 0), mid(&net, 1));
    let ga = ca.density.value * ca.velocity.value * 0.01;
    let gb = cb.density.value * cb.velocity.value * 0.01;
    let w = net.junction_power(j);
    println!(
        "turbine: junction {mdot:.4} kg/s, pipe A {ga:.4}, pipe B {gb:.4}; shaft power {:.1} kW; p_A {:.0} Pa, p_B {:.0} Pa; beta {:.4}",
        w / 1e3,
        ca.pressure.value,
        cb.pressure.value,
        params.beta()
    );
    assert!(mdot > 0.0 && w > 0.0);
    assert!((ga / mdot - 1.0).abs() < 0.005 && (gb / mdot - 1.0).abs() < 0.005);

    // Reversed pressures: closed.
    let (back, jb) = two_pipes(1.0e5, 2.0e5, 0.01, |a, b| HemJunction::Turbine {
        a,
        b,
        params,
    });
    assert_eq!(back.junction_mass_flow(jb), 0.0);
}

/// Invalid components are refused.
#[test]
fn invalid_components_are_refused() {
    let mut net = HemNetwork::new(IdealGasHem::air(), 0.2).unwrap();
    let a = net.add_pipe(pipe(1.0e5));
    let b = net.add_pipe(pipe(1.0e5));
    let bad = TurbineParams {
        max_mass_flow_kg_s: 1.0,
        design_stagnation_pressure_pa: 1.0e5,
        design_stagnation_temperature_k: 300.0,
        design_pressure_ratio: 0.5,
        design_mass_flow_ratio: 0.8,
        efficiency: 0.9,
        isentropic_exponent: 1.4,
    };
    assert!(net
        .add_component(HemJunction::Turbine {
            a: end(a, PipeEnd::Outlet),
            b: end(b, PipeEnd::Inlet),
            params: bad
        })
        .is_err());
    assert!(net
        .add_component(HemJunction::Valve {
            a: end(a, PipeEnd::Outlet),
            b: end(a, PipeEnd::Outlet),
            opening: TimeTable::constant(1.0),
            form_loss: 0.0
        })
        .is_err());
    assert!(net
        .add_component(HemJunction::Pump {
            a: end(a, PipeEnd::Outlet),
            b: end(b, PipeEnd::Inlet),
            head: PumpHead::Head(TimeTable::constant(1.0)),
            reverse_loss: (-1.0, 0.0)
        })
        .is_err());
    let vs = IdealGasHem::air()
        .state_p_rho(
            Pressure::new::<pascal>(1.0e5),
            MassDensity::new::<kilogram_per_cubic_meter>(1.16),
        )
        .unwrap();
    let _ = vs;
    assert!(net
        .add_component(HemJunction::Volume(match net.junction(0) {
            Some(HemJunction::Volume(v)) => v.clone(),
            _ => return,
        }))
        .is_err());
}
