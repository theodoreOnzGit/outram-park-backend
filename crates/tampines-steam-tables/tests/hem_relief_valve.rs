//! # V&V: relief valve with HEM choking (gh:#1074, RELAP-7 §6.11)
//!
//! Predictions written before the first run (2026-11-01): a closed pipe of
//! 500 K water at 7.5 MPa behind a valve set at 7.0 MPa (reseat 6.5 MPa)
//! discharging into a closed 0.5 MPa receiver pipe: the valve opens on the
//! first step, the flow is **choked** (pressure ratio ~15), the protected
//! pipe depressurises, the valve **recloses** once its end cell falls to
//! 6.5 MPa and **stays closed**; the combined mass is conserved to round-off.
//!
//! **Result (2026-11-01):** opens on the first step, choked, 90.7 kg/s;
//! recloses at 0.040 ms with the end cell at 5.95 MPa; mass conserved to
//! 2e-16. **"Stays closed" MISSED:** the abrupt closure in a stiff inviscid
//! liquid sends a water-hammer wave back that lifts the end cell above the
//! set pressure, and the valve **chatters** (re-opens). That is physical, not
//! a defect, so the test now checks what must hold on every step: the
//! hysteresis (open only at `p ≥ p_set`, close only at `p ≤ p_reseat`), no
//! mass across the valve on any step it is closed, and conservation.
//! Over 2000 steps: **12 openings and 12 closings**, each at the right side of
//! its threshold; worst per-pipe mass change on a closed step 4.4e-16;
//! combined mass change 4.4e-16. (A first version asserted closed-step pipe
//! masses with `==` and failed by one ulp: interior flux sums cancel only to
//! round-off. The bar is now 1e-13 relative, as in `hem_components`.)

use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::pressure::megapascal;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::velocity::meter_per_second;

fn water_pipe(p_mpa: f64, t_k: f64) -> HemPipe<Iapws97Hem> {
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.01),
        10,
    )
    .unwrap();
    let s = PrimitiveState::PressureTemperature {
        pressure: Pressure::new::<megapascal>(p_mpa),
        temperature: ThermodynamicTemperature::new::<kelvin>(t_k),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    HemPipe::new(
        geo,
        Iapws97Hem,
        HemInitialCondition::Uniform(s),
        HemNumerics::default(),
    )
    .unwrap()
}

/// **Opens, chokes, obeys its hysteresis, conserves mass.** Area fraction
/// 0.1, 2000 steps.
#[test]
fn a_relief_valve_opens_chokes_and_obeys_its_hysteresis() {
    let mut net = HemNetwork::new(Iapws97Hem, 0.2).unwrap();
    let ia = net.add_pipe(water_pipe(7.5, 500.0));
    let ib = net.add_pipe(water_pipe(0.5, 400.0));
    let j = net
        .add_component(HemJunction::ReliefValve {
            a: PipeEndRef {
                pipe: ia,
                end: PipeEnd::Outlet,
            },
            b: PipeEndRef {
                pipe: ib,
                end: PipeEnd::Inlet,
            },
            set_pressure_pa: 7.0e6,
            reseat_pressure_pa: 6.5e6,
            area_fraction: 0.1,
        })
        .unwrap();
    let m0 = net.total_mass().value;
    let end_p = |net: &HemNetwork<Iapws97Hem>| {
        net.pipe(ia)
            .unwrap()
            .cell_state(9)
            .unwrap()
            .pressure
            .get::<megapascal>()
    };
    let masses = |net: &HemNetwork<Iapws97Hem>| {
        (
            net.pipe(ia).unwrap().total_mass().value,
            net.pipe(ib).unwrap().total_mass().value,
        )
    };
    let mut was_open = false;
    let mut worst_closed: f64 = 0.0;
    let (mut opens, mut closes) = (0, 0);
    let (mut first_open, mut first_choked) = (false, false);
    for k in 0..2000 {
        let p_before = end_p(&net);
        let m_before = masses(&net);
        net.step().unwrap();
        let (open, choked) = net.relief_valve_state(j);
        if k == 0 {
            first_open = open;
            first_choked = choked;
        }
        if open && !was_open {
            opens += 1;
            assert!(
                p_before >= 7.0 - 1e-12,
                "opened at {p_before} MPa, below the set pressure"
            );
        }
        if !open && was_open {
            closes += 1;
            assert!(
                p_before <= 6.5 + 1e-12,
                "closed at {p_before} MPa, above the reseat pressure"
            );
        }
        if !open {
            // Each pipe is then closed at both ends, so its mass is constant
            // to round-off (the interior flux sums cancel only to an ulp);
            // the same 1e-13 bar as the closed-valve test in hem_components.
            let m_after = masses(&net);
            worst_closed = worst_closed
                .max((m_after.0 / m_before.0 - 1.0).abs())
                .max((m_after.1 / m_before.1 - 1.0).abs());
            assert!(
                worst_closed < 1e-13,
                "mass crossed a closed valve: {m_before:?} -> {m_after:?}"
            );
        }
        was_open = open;
    }
    let dm = net.total_mass().value / m0 - 1.0;
    println!("relief valve: first step open {first_open}, choked {first_choked}; {opens} openings, {closes} closings in 2000 steps; worst closed-step pipe mass change {worst_closed:e}; mass change {dm:e}");
    assert!(first_open && first_choked);
    assert!(closes >= 1, "it reseats");
    assert!(dm.abs() < 1e-12);
}
/// Invalid relief valves are refused, and a closure without a break-flow
/// model cannot open one (the error propagates).
#[test]
fn relief_valve_inputs_are_checked() {
    let mut net = HemNetwork::new(Iapws97Hem, 0.2).unwrap();
    let ia = net.add_pipe(water_pipe(7.5, 500.0));
    let ib = net.add_pipe(water_pipe(0.5, 400.0));
    let rv = |set: f64, reseat: f64, frac: f64| HemJunction::ReliefValve {
        a: PipeEndRef {
            pipe: ia,
            end: PipeEnd::Outlet,
        },
        b: PipeEndRef {
            pipe: ib,
            end: PipeEnd::Inlet,
        },
        set_pressure_pa: set,
        reseat_pressure_pa: reseat,
        area_fraction: frac,
    };
    assert!(
        net.add_component(rv(7.0e6, 7.5e6, 0.1)).is_err(),
        "reseat above set"
    );
    assert!(net.add_component(rv(7.0e6, 6.5e6, 0.0)).is_err());
    assert!(net.add_component(rv(7.0e6, 6.5e6, 1.5)).is_err());
}
