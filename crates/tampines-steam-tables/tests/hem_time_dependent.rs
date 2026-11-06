//! # V&V: time-dependent volumes and junctions in a `HemNetwork` (gh:#1076)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. A time-dependent volume with constant tables is the static
//!    `Reservoir` boundary, bit for bit.
//! 2. A time-dependent junction puts exactly `Σ ṁ(t_k)·dt_k` into a pipe
//!    closed at its other end (the table is sampled at the start of each
//!    step), to round-off; against the exact integral of the table the
//!    difference is the left-Riemann sampling error, below 1 %.
//! 3. The table interpolates linearly and holds its end values.

use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

fn air_pipe(p: f64) -> HemPipe<IdealGasHem> {
    let r = IdealGasHem::air().specific_gas_constant.value;
    let geo = HemPipeGeometry::new(
        Length::new::<meter>(1.0),
        Area::new::<square_meter>(0.01),
        40,
    )
    .unwrap();
    let s = PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(p),
        density: MassDensity::new::<kilogram_per_cubic_meter>(p / (r * 300.0)),
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

/// **Constant TDV equals the static reservoir.** Air, 1 m pipe, reservoir
/// (1.05e5 Pa, 300 K) at the inlet, 1e5 Pa at the outlet, 20 ms through a
/// one-pipe network both ways. **Result (2026-11-01):** identical, asserted
/// with `==` on every cell.
#[test]
fn a_constant_time_dependent_volume_is_the_static_reservoir() {
    let air = IdealGasHem::air();
    let r = air.specific_gas_constant.value;
    let h0 = 3.5 * r * 300.0;
    let outlet = |p: &mut HemPipe<IdealGasHem>| {
        p.set_boundary(
            PipeEnd::Outlet,
            HemBoundary::FixedPressure(Pressure::new::<pascal>(1.0e5)),
        )
        .unwrap()
    };

    let mut a = air_pipe(1.0e5);
    outlet(&mut a);
    a.set_boundary(
        PipeEnd::Inlet,
        HemBoundary::Reservoir {
            stagnation_pressure: Pressure::new::<pascal>(1.05e5),
            stagnation_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(h0),
        },
    )
    .unwrap();
    let mut net_a = HemNetwork::new(air, 0.2).unwrap();
    net_a.add_pipe(a);
    net_a.run_until(Time::new::<second>(0.02)).unwrap();

    let mut b = air_pipe(1.0e5);
    outlet(&mut b);
    let mut net_b = HemNetwork::new(air, 0.2).unwrap();
    let ib = net_b.add_pipe(b);
    net_b
        .add_time_dependent(
            PipeEndRef {
                pipe: ib,
                end: PipeEnd::Inlet,
            },
            TimeDependentBoundary::Volume {
                pressure: TimeTable::constant(1.05e5),
                thermo: TdvThermo::Enthalpy(TimeTable::constant(h0)),
            },
        )
        .unwrap();
    net_b.run_until(Time::new::<second>(0.02)).unwrap();
    assert_eq!(
        net_a.pipe(0).unwrap().snapshot().cells,
        net_b.pipe(ib).unwrap().snapshot().cells
    );

    // The temperature form closes the same enthalpy through the EOS.
    let mut c = air_pipe(1.0e5);
    outlet(&mut c);
    let mut net_c = HemNetwork::new(air, 0.2).unwrap();
    let ic = net_c.add_pipe(c);
    net_c
        .add_time_dependent(
            PipeEndRef {
                pipe: ic,
                end: PipeEnd::Inlet,
            },
            TimeDependentBoundary::Volume {
                pressure: TimeTable::constant(1.05e5),
                thermo: TdvThermo::Temperature(TimeTable::constant(300.0)),
            },
        )
        .unwrap();
    net_c.run_until(Time::new::<second>(0.02)).unwrap();
    let pc = net_c.pipe(ic).unwrap().snapshot().cells[20].pressure.value;
    let pb = net_b.pipe(ib).unwrap().snapshot().cells[20].pressure.value;
    assert!((pc / pb - 1.0).abs() < 1e-12, "{pc} vs {pb}");
}

/// **A TDJ puts in exactly the mass its table says.** Air pipe, walls at the
/// outlet, a TDJ at the inlet ramping 0 → 0.2 kg/s over 2 ms then holding,
/// at 300 K, run 5 ms. Expected mass gain `Σ ṁ(t_k) dt_k` over the steps
/// taken; the exact integral is `0.2·(5 − 1)·1e-3 = 8e-4` kg.
/// **Result (2026-11-01):** printed and asserted.
#[test]
fn a_time_dependent_junction_puts_in_its_tabulated_mass() {
    let air = IdealGasHem::air();
    let mut net = HemNetwork::new(air, 0.2).unwrap();
    let ip = net.add_pipe(air_pipe(1.0e5));
    let table = TimeTable::new(&[(0.0, 0.0), (2.0e-3, 0.2)]).unwrap();
    net.add_time_dependent(
        PipeEndRef {
            pipe: ip,
            end: PipeEnd::Inlet,
        },
        TimeDependentBoundary::Junction {
            mass_flow: table.clone(),
            temperature: TimeTable::constant(300.0),
        },
    )
    .unwrap();
    let m0 = net.total_mass().value;
    let mut expected = 0.0;
    while net.time().value < 5.0e-3 {
        let t = net.time().value;
        let dt = net.step().unwrap().value;
        expected += table.at(t) * dt;
    }
    let gained = net.total_mass().value - m0;
    let t_end = net.time().value;
    let exact = 0.2 * (t_end - 1.0e-3);
    println!("TDJ: gained {gained:.9e} kg, step sum {expected:.9e} kg, exact integral {exact:.9e} kg ({:+.3} %)", 100.0 * (gained / exact - 1.0));
    assert!((gained / expected - 1.0).abs() < 1e-12);
    assert!((gained / exact - 1.0).abs() < 0.01);
}

/// The table: interpolation, clamping, refusals.
#[test]
fn time_tables_interpolate_and_hold_their_ends() {
    let t = TimeTable::new(&[(1.0, 10.0), (3.0, 30.0)]).unwrap();
    assert_eq!(t.at(0.0), 10.0);
    assert_eq!(t.at(2.0), 20.0);
    assert_eq!(t.at(5.0), 30.0);
    assert_eq!(TimeTable::constant(7.0).at(123.0), 7.0);
    assert!(TimeTable::new(&[]).is_err());
    assert!(TimeTable::new(&[(1.0, 0.0), (1.0, 1.0)]).is_err());
    assert!(TimeTable::new(&[(0.0, f64::NAN)]).is_err());
    let air = IdealGasHem::air();
    let mut net = HemNetwork::new(air, 0.2).unwrap();
    let ip = net.add_pipe(air_pipe(1.0e5));
    let end = PipeEndRef {
        pipe: ip,
        end: PipeEnd::Inlet,
    };
    let tdb = TimeDependentBoundary::Junction {
        mass_flow: TimeTable::constant(0.0),
        temperature: TimeTable::constant(300.0),
    };
    net.add_time_dependent(end, tdb.clone()).unwrap();
    assert!(
        net.add_time_dependent(end, tdb).is_err(),
        "an end is driven once"
    );
}
