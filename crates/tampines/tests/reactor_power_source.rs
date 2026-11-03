//! # V&V: reactor power source (gh:#1079, RELAP-7 §6.16)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. The cell powers of every channel sum to the total to round-off (< 1e-14
//!    relative), for uniform, chopped-cosine and tabulated shapes; the
//!    chopped cosine with `H_e = H` and many cells has peak/mean `π/2`.
//! 2. A decay-heat transient deposited in a closed HEM pipe raises its total
//!    energy by exactly `Σ P(t_k)·dt_k` (to < 1e-9), and within 1 % of the
//!    exact integral of the hand Way–Wigner curve.

use petir::mathf::RealMath;
use tampines::components::reactor_power::*;
use tampines_steam_tables::hem::{
    HemInitialCondition, HemNetwork, HemNumerics, HemPipe, HemPipeGeometry, IdealGasHem,
    PrimitiveState, TimeTable,
};
use uom::si::area::square_meter;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::power::watt;
use uom::si::pressure::pascal;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;

fn t(s: f64) -> Time {
    Time::new::<second>(s)
}

/// **The distribution sums to the total.** Three channels with radial
/// weights 1 : 2 : 0.5 and uniform, chopped-cosine (ratio 1.2) and
/// tabulated shapes over 20 cells, total 3 MW. **Result (2026-11-01):** each
/// sum within 1e-14; the bare cosine over 2000 cells has peak/mean
/// 1.570795681 against π/2 = 1.570796327 (−4.1e-7, the cell-average
/// discretisation).
#[test]
fn the_distributed_power_sums_to_the_total() {
    let table: Vec<f64> = (0..20).map(|i| 1.0 + i as f64).collect();
    let src = ReactorPowerSource::new(
        vec![
            CoreChannelShare { radial_weight: 1.0, axial_shape: AxialPowerShape::Uniform },
            CoreChannelShare {
                radial_weight: 2.0,
                axial_shape: AxialPowerShape::ChoppedCosine { extrapolated_length_ratio: 1.2 },
            },
            CoreChannelShare { radial_weight: 0.5, axial_shape: AxialPowerShape::Table(table) },
        ],
        TotalPower::External(Power::new::<watt>(3.0e6)),
    )
    .unwrap();
    assert_eq!(src.n_channels(), 3);
    let mut sum = 0.0;
    for c in 0..3 {
        let cells = src.cell_powers(c, 20, t(0.0)).unwrap();
        let s: f64 = cells.iter().map(|p| p.get::<watt>()).sum();
        let ch = src.channel_powers(t(0.0))[c].get::<watt>();
        assert!((s / ch - 1.0).abs() < 1e-14, "channel {c}: {s} vs {ch}");
        sum += s;
    }
    assert!((sum / 3.0e6 - 1.0).abs() < 1e-14, "{sum}");
    assert!((src.channel_powers(t(0.0))[1].get::<watt>() / (3.0e6 * 2.0 / 3.5) - 1.0).abs() < 1e-14);

    let n = 2000;
    let f = AxialPowerShape::ChoppedCosine { extrapolated_length_ratio: 1.0 }.cell_fractions(n).unwrap();
    let peak = f.iter().cloned().fold(0.0, f64::max) * n as f64;
    println!("bare cosine peak/mean over {n} cells: {peak:.9} (pi/2 = {:.9})", core::f64::consts::FRAC_PI_2);
    assert!((peak / core::f64::consts::FRAC_PI_2 - 1.0).abs() < 1e-6);

    // Refusals.
    assert!(AxialPowerShape::ChoppedCosine { extrapolated_length_ratio: 0.9 }.cell_fractions(4).is_err());
    assert!(AxialPowerShape::Table(vec![1.0, -1.0]).cell_fractions(2).is_err());
    assert!(AxialPowerShape::Table(vec![0.0, 0.0]).cell_fractions(2).is_err());
    assert!(AxialPowerShape::Uniform.cell_fractions(0).is_err());
    assert!(ReactorPowerSource::new(vec![], TotalPower::External(Power::new::<watt>(1.0))).is_err());
    assert!(src.cell_powers(7, 20, t(0.0)).is_err());
}

/// Way–Wigner decay power after shutdown, `P/P0 = 0.0622 [t^-0.2 − (t+T)^-0.2]`
/// (Glasstone & Sesonske, *Nuclear Reactor Engineering*), `T` the operating
/// time. A hand curve, used only as a smooth transient.
fn way_wigner(p0: f64, t_after: f64, t_op: f64) -> f64 {
    p0 * 0.0622 * (t_after.r_powf(-0.2) - (t_after + t_op).r_powf(-0.2))
}

/// **A decay-heat transient heats a channel by the energy balance.** Air at
/// 1e5 Pa in a closed 1 m, 0.01 m² pipe of 20 cells (walls at both ends),
/// one channel with a chopped cosine, total power from a table sampled every
/// 0.1 ms of the Way–Wigner curve for `P0 = 10 kW`, operating time 1 day,
/// from 1 s to 1.005 s after shutdown (5 ms). The power source updates the
/// pipe before each network step. **Result (2026-11-01):** P(0) = 558.0 W;
/// energy gained 2.792025 J = the step sum to 1e-9; +0.001 % from the exact
/// integral 2.792007 J (the 0.1 ms table and start-of-step sampling).
#[test]
fn a_decay_heat_transient_heats_a_channel_by_the_energy_balance() {
    let geo = HemPipeGeometry::new(Length::new::<meter>(1.0), Area::new::<square_meter>(0.01), 20).unwrap();
    let s = PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(1.0e5),
        density: MassDensity::new::<kilogram_per_cubic_meter>(1.16),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    let pipe = HemPipe::new(geo, IdealGasHem::air(), HemInitialCondition::Uniform(s), HemNumerics::default()).unwrap();
    let (p0, t_op, t0) = (1.0e4, 86_400.0, 1.0);
    let pts: Vec<(f64, f64)> = (0..=60).map(|k| {
        let tt = k as f64 * 1.0e-4;
        (tt, way_wigner(p0, t0 + tt, t_op))
    }).collect();
    let table = TimeTable::new(&pts).unwrap();
    let src = ReactorPowerSource::new(
        vec![CoreChannelShare {
            radial_weight: 1.0,
            axial_shape: AxialPowerShape::ChoppedCosine { extrapolated_length_ratio: 1.2 },
        }],
        TotalPower::Table(table),
    )
    .unwrap();
    let mut net = HemNetwork::new(IdealGasHem::air(), 0.2).unwrap();
    let ip = net.add_pipe(pipe);
    let e0 = net.total_energy().value;
    let mut expected = 0.0;
    while net.time().value < 5.0e-3 {
        let now = net.time();
        src.apply_to_hem_pipe(0, now, net.pipe_mut(ip).unwrap()).unwrap();
        let q: f64 = net.pipe(ip).unwrap().cell_heat_input().iter().map(|p| p.get::<watt>()).sum();
        assert!((q / src.total_power(now).get::<watt>() - 1.0).abs() < 1e-13);
        let dt = net.step().unwrap().value;
        expected += src.total_power(now).get::<watt>() * dt;
    }
    let t_end = net.time().value;
    let gained = net.total_energy().value - e0;
    // Exact integral of the Way–Wigner curve over [t0, t0 + t_end].
    let anti = |x: f64| p0 * 0.0622 * (x.r_powf(0.8) - (x + t_op).r_powf(0.8)) / 0.8;
    let exact = anti(t0 + t_end) - anti(t0);
    println!(
        "decay heat: P(0) = {:.3} W, gained {gained:.6e} J, step sum {expected:.6e} J, exact {exact:.6e} J ({:+.3} %)",
        way_wigner(p0, t0, t_op),
        100.0 * (gained / exact - 1.0)
    );
    assert!((gained / expected - 1.0).abs() < 1e-9);
    assert!((gained / exact - 1.0).abs() < 0.01);
    // Clearing restores the source-free pipe.
    net.pipe_mut(ip).unwrap().set_cell_heat_input(&[]).unwrap();
    assert!(net.pipe(ip).unwrap().cell_heat_input().is_empty());
    assert!(net.pipe_mut(ip).unwrap().set_cell_heat_input(&[Power::new::<watt>(1.0)]).is_err());
}
