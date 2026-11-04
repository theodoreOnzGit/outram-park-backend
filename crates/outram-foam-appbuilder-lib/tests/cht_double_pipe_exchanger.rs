//! V&V V3 of gh:#1156: a counterflow double-pipe exchanger
//! (`cases::double_pipe`, three regions by `ChtMultiRegionFoam`) against
//! ε-NTU at the solved UA, and its energy balance.
//!
//! **Methodology** (written before the runs, with predictions, in
//! `verification_and_validation/cht_double_pipe_gh1156.md`, V3): meshes from
//! `block_primitives::DoublePipe` (r = 5 / 6 / 10 mm, L = 200 mm, conformal);
//! `DoublePipeExchanger::demo()` (water-like fluids, steel-like tube, equal
//! flow rates, Re = 30 in the tube, 350 K / 300 K); flows marched to the
//! `laminar_pipe` steady criterion (pseudo-transient PISO, Co 4, viscous
//! number ≤ 0.5), frozen, then the three steady energy equations iterated to
//! a pass change of 1e-9 K. ε_CFD from the hot stream's heat; ε-NTU from
//! `outram_park_fork_dwsim_libs::heat_exchanger::ntu_effectiveness::evaluate`
//! (counter-current) at the local UA; energy balance hot / cold / both
//! interfaces.
//!
//! **Results (2026-11-03):** ε_CFD / ε-NTU(UA_local) − 1 = −2.01 / −1.23 /
//! −0.76 % at nz = 30 / 50 / 80 (ε_CFD 0.55389 / 0.55656 / 0.55818; NTU ≈
//! 1.29). The gap falls as 1/nz (the first-order axial discretisation), to
//! < 0.1 % in the limit. Energy closes to < 1e-9 between the hot and cold
//! streams and both interfaces; the tube flow is developed (u_c/2U 0.980 →
//! 0.995). Interpretation: the coupled solve behaves as an ideal counterflow
//! exchanger up to its discretisation error. The coarse test grid (nz = 30)
//! is gated at 5 % as a regression pin.
use std::sync::Arc;

use outram_foam_appbuilder_lib::cases::laminar_pipe::{centreline_velocity, PipeFlow};
use outram_foam_appbuilder_lib::cases::double_pipe::{freeze_flows_for_steady_energy, pressure_drop, profiles, pseudo_time_step, report, step_flows, DoublePipeExchanger, ExchangerProfiles, ExchangerReport};
use outram_foam_mesh::block_primitives::DoublePipe;
use outram_park_fork_dwsim_libs::heat_exchanger::lmtd::FlowArrangement;
use outram_park_fork_dwsim_libs::heat_exchanger::ntu_effectiveness::evaluate;
use uom::si::f64::{Length, TemperatureInterval, ThermalConductance, Velocity};
use uom::si::length::meter;
use uom::si::ratio::ratio;
use uom::si::temperature_interval::kelvin;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::velocity::meter_per_second;
use uom::si::volume_rate::cubic_meter_per_second;

fn m(v: f64) -> Length {
    Length::new::<meter>(v)
}

struct Outcome {
    rep: ExchangerReport,
    prof: ExchangerProfiles,
    dp: [f64; 2],
    uc: f64,
    eps_ntu: f64,
    cells: [usize; 3],
    flow_steps: usize,
    energy_passes: usize,
    secs: f64,
}

fn run(n: usize, nr: [usize; 3], nz: usize) -> Outcome {
    run_case(DoublePipeExchanger::demo(), n, nr, nz)
}

fn run_case(case: DoublePipeExchanger, n: usize, nr: [usize; 3], nz: usize) -> Outcome {
    let t0 = std::time::Instant::now();
    let dp = DoublePipe { r_tube_inner: m(0.005), r_tube_outer: m(0.006), r_shell: m(0.01), length: case.length, origin: [m(0.0); 3], n, nr, nz };
    let [a, b, c] = dp.meshes().unwrap();
    let (fa, fb, fc) = (Arc::new(a.to_fv_mesh().unwrap()), Arc::new(b.to_fv_mesh().unwrap()), Arc::new(c.to_fv_mesh().unwrap()));
    let q = case.flow_rate.get::<cubic_meter_per_second>();
    let dz = m(case.length.get::<meter>() / nz as f64);
    let area = |mesh: &outram_foam_basic_lib::mesh::FvMesh| {
        let p = mesh.patches.iter().find(|p| p.name == "inlet").unwrap();
        (0..p.size).map(|i| mesh.face_areas[p.start + i]).sum::<f64>()
    };
    let dt_in = pseudo_time_step(&fa, Velocity::new::<meter_per_second>(q / area(&fa)), case.nu, dz, 4.0, 0.5);
    let q_out = case.annulus_flow().get::<cubic_meter_per_second>();
    let dt_out = pseudo_time_step(&fc, Velocity::new::<meter_per_second>(q_out / area(&fc)), case.nu, dz, 4.0, 0.5);
    let mut cht = case.build(fa, fb, fc, dt_in, dt_out).unwrap();
    // The conformal interfaces really are conformal.
    for i in &cht.interfaces {
        assert!(i.max_pairing_gap() < 1e-12, "pairing gap {}", i.max_pairing_gap());
    }
    let mut flow_steps = 0;
    for _ in 0..2000 {
        flow_steps += 1;
        if step_flows(&mut cht, case.length).unwrap() < 1e-3 {
            break;
        }
    }
    // The tube flow must be developed Hagen-Poiseuille downstream: u_c / 2U
    // at 0.75 L (the stage 1 instrument).
    let f0 = &cht.fluids[0];
    let flow = PipeFlow { radius: m(0.005), length: case.length, bulk_velocity: Velocity::new::<meter_per_second>(q / area(&f0.mesh)), nu: case.nu };
    let uc = centreline_velocity(f0, &flow, (0.0, 0.0), 0.75 * case.length.get::<meter>(), 0.5 * dz.get::<meter>()).get::<meter_per_second>() / (2.0 * q / area(&f0.mesh));
    freeze_flows_for_steady_energy(&mut cht);
    cht.energy_outer_correctors = 5000;
    cht.energy_outer_tolerance = TemperatureInterval::new::<kelvin>(1e-9);
    let st = cht.step().unwrap();
    assert!(st.last_pass_change <= 1e-9, "energy not converged: {st:?}");
    let rep = report(&cht, &case);
    let prof = profiles(&cht);
    let rho = case.fluid.density.get::<uom::si::mass_density::kilogram_per_cubic_meter>();
    let dps = [pressure_drop(&cht.fluids[0], rho), pressure_drop(&cht.fluids[1], rho)];
    let e = evaluate(
        FlowArrangement::CounterCurrent,
        ThermalConductance::new::<watt_per_kelvin>(rep.ua_local),
        ThermalConductance::new::<watt_per_kelvin>(rep.c_hot),
        ThermalConductance::new::<watt_per_kelvin>(rep.c_cold),
        TemperatureInterval::new::<kelvin>(50.0),
    );
    Outcome { rep, prof, dp: dps, uc, eps_ntu: e.effectiveness.get::<ratio>(), cells: dp.n_cells(), flow_steps, energy_passes: st.passes, secs: t0.elapsed().as_secs_f64() }
}

fn print(o: &Outcome) {
    let r = &o.rep;
    let e_lmtd = evaluate(
        FlowArrangement::CounterCurrent,
        ThermalConductance::new::<watt_per_kelvin>(r.ua_lmtd),
        ThermalConductance::new::<watt_per_kelvin>(r.c_hot),
        ThermalConductance::new::<watt_per_kelvin>(r.c_cold),
        TemperatureInterval::new::<kelvin>(50.0),
    )
    .effectiveness
    .get::<ratio>();
    println!(
        "cells {:?} flow steps {} (tube u_c/2U at 0.75 L {:.4}) energy passes {} ({:.1} s)\n  T_hot,out {:.4} K  T_cold,out {:.4} K  C_hot {:.6} C_cold {:.6} W/K\n  Q_hot {:.6} Q_cold {:.6} Q_wall,in {:.6} Q_wall,out {:.6} W\n  imbalance hot-cold {:.2e}, hot-wall_in {:.2e}, wall_in-wall_out {:.2e} (relative)\n  eps_CFD {:.5}  UA_local {:.5} W/K (NTU {:.4})  eps-NTU(UA_local) {:.5}  eps_CFD/eps-NTU - 1 = {:+.4}\n  UA_LMTD {:.5} W/K  eps-NTU(UA_LMTD) {:.6} (near-identity)",
        o.cells,
        o.flow_steps,
        o.uc,
        o.energy_passes,
        o.secs,
        r.t_hot_out,
        r.t_cold_out,
        r.c_hot,
        r.c_cold,
        r.q_hot,
        r.q_cold,
        r.q_wall_in,
        r.q_wall_out,
        (r.q_hot - r.q_cold) / r.q_hot,
        (r.q_hot - r.q_wall_in) / r.q_hot,
        (r.q_wall_in - r.q_wall_out) / r.q_hot,
        r.effectiveness,
        r.ua_local,
        r.ua_local / r.c_hot.min(r.c_cold),
        o.eps_ntu,
        r.effectiveness / o.eps_ntu - 1.0,
        r.ua_lmtd,
        e_lmtd
    );
}

#[test]
fn double_pipe_effectiveness_against_ntu_coarse() {
    let o = run(3, [2, 2, 3], 30);
    print(&o);
    let r = &o.rep;
    assert!((r.c_hot / r.c_cold - 1.0).abs() < 1e-9, "C_r must be 1");
    for x in [r.q_cold, r.q_wall_in, r.q_wall_out] {
        assert!((x / r.q_hot - 1.0).abs() < 1e-6, "energy does not close: {} vs {}", x, r.q_hot);
    }
    assert!((o.uc - 1.0).abs() < 0.05, "tube flow not developed: u_c/2U {}", o.uc);
    assert!(r.effectiveness > 0.05 && r.effectiveness < 0.95);
    assert!(r.t_hot_out < 350.0 && r.t_cold_out > 300.0);
    // The prediction was |gap| < 1 % at the V&V grids; the coarse test grid
    // is not one of them, so it is gated loosely and recorded.
    assert!((r.effectiveness / o.eps_ntu - 1.0).abs() < 0.05, "{} vs {}", r.effectiveness, o.eps_ntu);
}

/// The V&V grids (`--ignored`).
#[test]
#[ignore]
fn measure_gh1156_double_pipe() {
    for (n, nr, nz) in [(4, [3, 2, 4], 50), (6, [4, 2, 5], 80)] {
        let o = run(n, nr, nz);
        print(&o);
    }
}

/// gh:#1178's instruments on the coarse grid, with unequal flow rates
/// (annulus 1.5× the tube, so C_r = 2/3): the per-layer interface heats sum
/// to the interface totals, energy closes, the local Nusselt numbers are
/// finite and positive, and the tube's pressure drop lies between fully
/// developed Hagen–Poiseuille (64/Re) and that plus Shah's entrance
/// increment K(∞) = 1.25 with a 10 % allowance for the coarse grid.
///
/// **Results (2026-11-03, this grid):** printed by the test; the design-grid
/// numbers are in `crates/outram-park-digital-twin-engine/verification_and_validation/hf_lf_double_pipe_gh1178.md`.
#[test]
fn double_pipe_profiles_and_pressure_drop_unequal_flows() {
    let mut case = DoublePipeExchanger::demo();
    case.annulus_flow_rate = Some(case.flow_rate * 1.5);
    let o = run_case(case, 3, [2, 2, 3], 30);
    print(&o);
    let (r, p) = (&o.rep, &o.prof);
    assert!((r.c_cold / r.c_hot - 1.5).abs() < 1e-6, "C_cold/C_hot {}", r.c_cold / r.c_hot);
    assert!((r.q_cold / r.q_hot - 1.0).abs() < 1e-6);
    let (qi, qo): (f64, f64) = (p.q_inner.iter().sum(), p.q_outer.iter().sum());
    assert!((qi / r.q_wall_in - 1.0).abs() < 1e-9, "{qi} vs {}", r.q_wall_in);
    assert!((qo / r.q_wall_out - 1.0).abs() < 1e-9, "{qo} vs {}", r.q_wall_out);
    assert_eq!(p.z.len(), 30);
    for (a, b) in p.nu_hot().iter().zip(p.nu_cold()) {
        assert!(a.is_finite() && *a > 1.0 && b.is_finite() && b > 1.0, "{a} {b}");
    }
    let (nh, nc) = (p.mean_nu_hot(), p.mean_nu_cold());
    println!("  mean Nu hot {nh:.4} cold {nc:.4}; D_h {:.5} / {:.5} m; dp {:.4e} / {:.4e} Pa", p.dh_hot, p.dh_cold, o.dp[0], o.dp[1]);
    assert!(nh > 3.0 && nh < 15.0 && nc > 3.0 && nc < 20.0);
    // The hydraulic diameters from the mesh. Every wall is a regular
    // 4n-gon inscribed in its circle, for which 4A/P is the circle's value
    // times cos(pi/4n): 2 r1 and 2 (r3 - r2), both times cos(pi/12) here.
    let c = (std::f64::consts::PI / 12.0).cos();
    assert!((p.dh_hot / (0.010 * c) - 1.0).abs() < 1e-9 && (p.dh_cold / (0.008 * c) - 1.0).abs() < 1e-9, "{} {}", p.dh_hot, p.dh_cold);
    // Tube: Re = 30, L/D = 20, rho U^2/2 with U = Q / A_circle.
    let u = case.flow_rate.get::<cubic_meter_per_second>() / (std::f64::consts::PI * 0.005f64.powi(2));
    let dyn_p = 0.5 * 1000.0 * u * u;
    let hp = 64.0 / 30.0 * 20.0 * dyn_p;
    assert!(o.dp[0] > 0.95 * hp && o.dp[0] < 1.10 * (hp + 1.25 * dyn_p), "{} vs HP {}", o.dp[0], hp);
    assert!(o.dp[1] > 0.0);
}
