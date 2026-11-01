//! # V&V: HEM pipe networks with flow junctions and volume branches (gh:#1071)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. A closed network conserves mass and total energy to < 1e-12 relative.
//! 2. Sod across a `K = 0` equal-area flow junction differs from one
//!    continuous pipe by < 3 % of the pressure jump at worst, < 0.5 % in L1.
//! 3. Across a `K = 2` junction at steady low-Mach flow, the pressure step is
//!    `K·½ρu²` within 10 %.
//! 4. A symmetric T through a volume branch splits the flow equally (to
//!    round-off) and balances mass within 0.5 % at steady state.
//!
//! Results on each test and in
//! `verification_and_validation/hem_network_junctions.md`.

use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volume::cubic_meter;

fn gas(p: f64, rho: f64) -> PrimitiveState {
    PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(p),
        density: MassDensity::new::<kilogram_per_cubic_meter>(rho),
        velocity: Velocity::new::<meter_per_second>(0.0),
    }
}

fn pipe(len: f64, n: usize, ic: HemInitialCondition) -> HemPipe<IdealGasHem> {
    let geo =
        HemPipeGeometry::new(Length::new::<meter>(len), Area::new::<square_meter>(1.0), n).unwrap();
    HemPipe::new(geo, IdealGasHem::air(), ic, HemNumerics::default()).unwrap()
}

fn end(pipe: usize, end: PipeEnd) -> PipeEndRef {
    PipeEndRef { pipe, end }
}

/// **Closed network.** Pipe 0 (1 m, 1e5 Pa) → flow junction (K = 0.5) → pipe
/// 1 (1 m, 5e4 Pa) → volume (0.05 m³, 2e4 Pa) ← pipe 2 (1 m, 3e4 Pa); walls at
/// the three free ends; 5 ms. **Result (2026-11-01):** printed and asserted.
#[test]
fn a_closed_network_conserves_mass_and_energy() {
    let air = IdealGasHem::air();
    let mut net = HemNetwork::new(air, 0.2).unwrap();
    let p0 = net.add_pipe(pipe(1.0, 50, HemInitialCondition::Uniform(gas(1.0e5, 1.2))));
    let p1 = net.add_pipe(pipe(1.0, 50, HemInitialCondition::Uniform(gas(5.0e4, 0.6))));
    let p2 = net.add_pipe(pipe(1.0, 50, HemInitialCondition::Uniform(gas(3.0e4, 0.4))));
    net.add_flow_junction(end(p0, PipeEnd::Outlet), end(p1, PipeEnd::Inlet), 0.5)
        .unwrap();
    let vol_state = air
        .state_p_rho(
            Pressure::new::<pascal>(2.0e4),
            MassDensity::new::<kilogram_per_cubic_meter>(0.25),
        )
        .unwrap();
    let v = net
        .add_volume_branch(
            Volume::new::<cubic_meter>(0.05),
            vol_state,
            vec![
                (end(p1, PipeEnd::Outlet), 0.0),
                (end(p2, PipeEnd::Outlet), 1.0),
            ],
        )
        .unwrap();
    let (m0, e0) = (net.total_mass().value, net.total_energy().value);
    let steps = net.run_until(Time::new::<second>(5.0e-3)).unwrap();
    let dm = net.total_mass().value / m0 - 1.0;
    let de = net.total_energy().value / e0 - 1.0;
    let pv = net.volume_state(v).unwrap().pressure.get::<pascal>();
    println!("closed network: {steps} steps, mass change {dm:e}, energy change {de:e}, volume p {pv:.0} Pa");
    assert!(dm.abs() < 1e-12 && de.abs() < 1e-12);
    assert!(
        pv > 2.0e4,
        "the volume fills from the higher-pressure pipes"
    );
}

/// **Transparency of a K = 0 equal-area junction.** Sod (1e5 | 1e4 Pa,
/// 1 | 0.125 kg/m³) on one 2 m, 200-cell pipe against two 1 m, 100-cell pipes
/// joined at the diaphragm-side... the diaphragm is at 0.8 m so the waves
/// cross the junction at 1 m; transmissive ends; run to 2 ms. Both use the
/// network's step (one pipe as a one-pipe network) so the time steps are the
/// same rule. **Result (2026-11-01):** printed and asserted at the prediction.
#[test]
fn a_lossless_junction_is_nearly_transparent_to_sod() {
    let air = IdealGasHem::air();
    let riemann = |len: f64, diaphragm: f64, n: usize| {
        let ic = HemInitialCondition::Riemann {
            diaphragm: Length::new::<meter>(diaphragm),
            left: gas(1.0e5, 1.0),
            right: gas(1.0e4, 0.125),
        };
        let mut p = pipe(len, n, ic);
        p.set_boundary(PipeEnd::Inlet, HemBoundary::Transmissive)
            .unwrap();
        p.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive)
            .unwrap();
        p
    };
    let mut one = HemNetwork::new(air, 0.2).unwrap();
    one.add_pipe(riemann(2.0, 0.8, 200));
    one.run_until(Time::new::<second>(2.0e-3)).unwrap();

    let mut two = HemNetwork::new(air, 0.2).unwrap();
    let a = two.add_pipe(riemann(1.0, 0.8, 100));
    let mut b = pipe(1.0, 100, HemInitialCondition::Uniform(gas(1.0e4, 0.125)));
    b.set_boundary(PipeEnd::Outlet, HemBoundary::Transmissive)
        .unwrap();
    let b = two.add_pipe(b);
    two.add_flow_junction(end(a, PipeEnd::Outlet), end(b, PipeEnd::Inlet), 0.0)
        .unwrap();
    two.run_until(Time::new::<second>(2.0e-3)).unwrap();

    let p_one: Vec<f64> = one
        .pipe(0)
        .unwrap()
        .snapshot()
        .cells
        .iter()
        .map(|c| c.pressure.value)
        .collect();
    let p_two: Vec<f64> = [a, b]
        .iter()
        .flat_map(|i| {
            two.pipe(*i)
                .unwrap()
                .snapshot()
                .cells
                .into_iter()
                .map(|c| c.pressure.value)
        })
        .collect();
    let jump = 1.0e5 - 1.0e4;
    let worst = p_one
        .iter()
        .zip(&p_two)
        .map(|(x, y)| (x - y).abs())
        .fold(0.0, f64::max)
        / jump;
    let l1 = p_one
        .iter()
        .zip(&p_two)
        .map(|(x, y)| (x - y).abs())
        .sum::<f64>()
        / (p_one.len() as f64 * jump);
    println!("Sod across a K = 0 junction: worst |dp| {:.3} % of the jump, L1 {:.4} %; times {:.6e} / {:.6e} s", 100.0 * worst, 100.0 * l1, one.time().value, two.time().value);
    assert!(worst < 0.03 && l1 < 0.005);
}

/// **Steady form loss.** Two 1 m, 50-cell pipes joined with K = 2; reservoir
/// `p0 = 1.02e5` Pa, `T0 = 300` K at the inlet, 1e5 Pa at the outlet; run
/// 200 ms to steady. The pressure step across the junction is the difference
/// of the two pipes' mid-length pressures (a frictionless constant-area pipe
/// is uniform in pressure at steady state), against `K·½ρu²` from the upstream
/// pipe's mid cell.
///
/// **Instrument corrected 2026-11-01.** The first measure extrapolated the two
/// end cells to the junction. Those cells hold a thin transition layer at the
/// first-order junction face (A's last cell 101 037 Pa against 101 293 next to
/// it; B's first 100 409 against 99 986), so it read 288 Pa; the pipes differ
/// by 1313 Pa.
///
/// **Loss formulation corrected 2026-11-01.** The first version added the loss
/// to the upstream momentum flux after the KNP flux and carried only part of
/// it: the flow ran at 32.0 m/s against the 33.9 m/s that `p0 − p_e = (1 +
/// K)·½ρu²` gives. The loss now enters RELAP5's way, the downstream pressure
/// raised by `Δp` in the junction flux (module doc).
///
/// **Result (2026-11-01):** printed and asserted at the prediction.
#[test]
fn a_form_loss_drops_k_half_rho_u_squared_at_steady_state() {
    let air = IdealGasHem::air();
    let r = air.specific_gas_constant.value;
    let mut net = HemNetwork::new(air, 0.2).unwrap();
    let rho = 1.0e5 / (r * 300.0);
    let mut a = pipe(1.0, 50, HemInitialCondition::Uniform(gas(1.0e5, rho)));
    a.set_boundary(
        PipeEnd::Inlet,
        HemBoundary::Reservoir {
            stagnation_pressure: Pressure::new::<pascal>(1.02e5),
            stagnation_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(3.5 * r * 300.0),
        },
    )
    .unwrap();
    let mut b = pipe(1.0, 50, HemInitialCondition::Uniform(gas(1.0e5, rho)));
    b.set_boundary(
        PipeEnd::Outlet,
        HemBoundary::FixedPressure(Pressure::new::<pascal>(1.0e5)),
    )
    .unwrap();
    let ia = net.add_pipe(a);
    let ib = net.add_pipe(b);
    net.add_flow_junction(end(ia, PipeEnd::Outlet), end(ib, PipeEnd::Inlet), 2.0)
        .unwrap();
    net.run_until(Time::new::<second>(0.2)).unwrap();
    let sa = net.pipe(ia).unwrap().snapshot();
    let sb = net.pipe(ib).unwrap().snapshot();
    let n = sa.cells.len();
    let pa = sa.cells[n / 2].pressure.value;
    let pb = sb.cells[n / 2].pressure.value;
    let (rho_up, u_up) = (sa.cells[n / 2].density.value, sa.cells[n / 2].velocity.value);
    let expect = 2.0 * 0.5 * rho_up * u_up * u_up;
    let rel = (pa - pb) / expect - 1.0;
    println!(
        "form loss K = 2: step {:.2} Pa against K·½ρu² {:.2} Pa ({:+.2} %), u {:.2} m/s",
        pa - pb,
        expect,
        100.0 * rel,
        u_up
    );
    assert!(rel.abs() < 0.10);
}

/// **Symmetric T through a volume branch.** Reservoir (1.05e5 Pa, 300 K) →
/// pipe A → volume (0.01 m³) → pipes B and C, identical, each to 1e5 Pa; run
/// 2 s.
///
/// **Protocol corrected 2026-11-01.** The first run stopped at 200 ms, which
/// is not steady: the inviscid pipes and the volume slosh (A's flow at 0.05 /
/// 0.2 / 0.5 / 1 / 2 s: 13.37, 11.71, 10.54, 9.57, 9.64 kg/s), decaying only
/// through the partly reflecting ends; at 1 s the balance was still +2.3 %.
/// The settle time was lengthened; the 0.5 % criterion was not changed.
///
/// ~~**Result (2026-11-01), 2 s:** B and C identical to the last bit (4.811879
/// kg/s each); `(B + C)/A − 1 = −0.15 %`. Held.~~ **CORRECTED 2026-11-01
/// (gh:#1073)**: those flows were ten times too low. The volume's ports were a
/// KNP flux against the volume at rest, which at steady flow charges an
/// artificial port loss `Δp ≈ ρcu`; the 5 kPa drive went mostly into it. The
/// ports are now characteristic boundaries (reservoir outflow, sink inflow).
/// **Re-measured (2026-11-01), 2 s:** A 97.1574, B = C = 48.578690 kg/s
/// (bit-identical), `(B + C)/A − 1 = +0.0000 %`. Held. A hand estimate,
/// written **after** this run and so not a prediction: incompressible, no
/// friction, A's kinetic energy lost in the volume (Borda), B and C leaving
/// it as reservoir outflow at `u_A/2`, so `p0 − p_exit = ½ρu_A²(1 + ¼)`,
/// `u_A = 83.0` m/s, `ṁ_A = 96.4` kg/s; the run is +0.8 % (compressibility
/// at Mach 0.24), asserted within 3 %.
#[test]
fn a_symmetric_tee_splits_the_flow_equally_and_balances_mass() {
    let air = IdealGasHem::air();
    let r = air.specific_gas_constant.value;
    let rho = 1.0e5 / (r * 300.0);
    let mut net = HemNetwork::new(air, 0.2).unwrap();
    let mut a = pipe(1.0, 50, HemInitialCondition::Uniform(gas(1.0e5, rho)));
    a.set_boundary(
        PipeEnd::Inlet,
        HemBoundary::Reservoir {
            stagnation_pressure: Pressure::new::<pascal>(1.05e5),
            stagnation_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(3.5 * r * 300.0),
        },
    )
    .unwrap();
    let branch = || {
        let mut p = pipe(1.0, 50, HemInitialCondition::Uniform(gas(1.0e5, rho)));
        p.set_boundary(
            PipeEnd::Outlet,
            HemBoundary::FixedPressure(Pressure::new::<pascal>(1.0e5)),
        )
        .unwrap();
        p
    };
    let ia = net.add_pipe(a);
    let ib = net.add_pipe(branch());
    let ic = net.add_pipe(branch());
    let vs = air
        .state_p_rho(
            Pressure::new::<pascal>(1.0e5),
            MassDensity::new::<kilogram_per_cubic_meter>(rho),
        )
        .unwrap();
    net.add_volume_branch(
        Volume::new::<cubic_meter>(0.01),
        vs,
        vec![
            (end(ia, PipeEnd::Outlet), 0.0),
            (end(ib, PipeEnd::Inlet), 0.0),
            (end(ic, PipeEnd::Inlet), 0.0),
        ],
    )
    .unwrap();
    net.run_until(Time::new::<second>(2.0)).unwrap();
    let mdot = |i: usize| {
        let s = net.pipe(i).unwrap().snapshot();
        let c = &s.cells[s.cells.len() / 2];
        c.density.value * c.velocity.value
    };
    let (ma, mb, mc) = (mdot(ia), mdot(ib), mdot(ic));
    let balance = (mb + mc) / ma - 1.0;
    println!(
        "symmetric tee: A {ma:.4}, B {mb:.6}, C {mc:.6} kg/s; (B + C)/A − 1 = {:+.4} %",
        100.0 * balance
    );
    assert_eq!(mb, mc, "identical branches must carry identical flow");
    assert!(balance.abs() < 0.005);
    let u_hand = (5.0e3 / (0.625 * rho)).sqrt();
    let m_hand = rho * u_hand;
    println!("tee hand estimate: u_A {u_hand:.2} m/s, mdot_A {m_hand:.3} kg/s; run {:+.2} %", 100.0 * (ma / m_hand - 1.0));
    assert!((ma / m_hand - 1.0).abs() < 0.03);
}

/// Invalid networks are refused.
#[test]
fn invalid_networks_are_refused() {
    let air = IdealGasHem::air();
    assert!(HemNetwork::new(air, 0.0).is_err());
    assert!(HemNetwork::new(air, 0.6).is_err());
    let mut net = HemNetwork::new(air, 0.2).unwrap();
    let a = net.add_pipe(pipe(1.0, 10, HemInitialCondition::Uniform(gas(1.0e5, 1.2))));
    let b = net.add_pipe(pipe(1.0, 10, HemInitialCondition::Uniform(gas(1.0e5, 1.2))));
    assert!(net
        .add_flow_junction(end(a, PipeEnd::Outlet), end(a, PipeEnd::Outlet), 0.0)
        .is_err());
    assert!(net
        .add_flow_junction(end(a, PipeEnd::Outlet), end(9, PipeEnd::Inlet), 0.0)
        .is_err());
    assert!(net
        .add_flow_junction(end(a, PipeEnd::Outlet), end(b, PipeEnd::Inlet), -1.0)
        .is_err());
    net.add_flow_junction(end(a, PipeEnd::Outlet), end(b, PipeEnd::Inlet), 0.0)
        .unwrap();
    assert!(
        net.add_flow_junction(end(a, PipeEnd::Outlet), end(b, PipeEnd::Outlet), 0.0)
            .is_err(),
        "end already joined"
    );
    let vs = air
        .state_p_rho(
            Pressure::new::<pascal>(1.0e5),
            MassDensity::new::<kilogram_per_cubic_meter>(1.2),
        )
        .unwrap();
    assert!(net
        .add_volume_branch(
            Volume::new::<cubic_meter>(0.0),
            vs,
            vec![(end(b, PipeEnd::Outlet), 0.0)]
        )
        .is_err());
    assert!(net
        .add_volume_branch(Volume::new::<cubic_meter>(1.0), vs, vec![])
        .is_err());
    assert!(net
        .add_volume_branch(
            Volume::new::<cubic_meter>(1.0),
            vs,
            vec![
                (end(b, PipeEnd::Outlet), 0.0),
                (end(b, PipeEnd::Outlet), 0.0)
            ]
        )
        .is_err());
    assert!(
        net.volume_state(0).is_err(),
        "junction 0 is a flow junction"
    );
    assert!(matches!(net.junction(0), Some(HemJunction::Flow { .. })));
    let mut p = pipe(1.0, 10, HemInitialCondition::Uniform(gas(1.0e5, 1.2)));
    assert!(p.step_by(Time::new::<second>(-1.0)).is_err());
    assert!(p
        .set_boundary(
            PipeEnd::Inlet,
            HemBoundary::Flux {
                mass_flux: MassFlux::new::<uom::si::mass_flux::kilogram_per_square_meter_second>(
                    f64::NAN
                ),
                momentum_flux: Pressure::new::<pascal>(1.0),
                energy_flux: HeatFluxDensity::new::<
                    uom::si::heat_flux_density::watt_per_square_meter,
                >(1.0),
            }
        )
        .is_err());
}
