//! # V&V: heat structures coupled to HEM pipes (gh:#1077, RELAP-7 §4.1, §6.2–6.4)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. **Slab transient:** a 2 cm slab at 400 K with both faces held at
//!    300 K; at Fourier number 0.1 the centre temperature matches the Fourier
//!    series within 0.5 % of the 100 K step (40 cells, Δt = 0.01 s).
//! 2. **Fuel rod at steady state:** UO2-like fuel (R = 4.1 mm, k = 3), a
//!    helium gap (0.08 mm, k = 0.3) and cladding (0.57 mm, k = 16) with
//!    q''' = 3e8 W/m³ and the cladding surface at 600 K: the centre-cell and
//!    cladding-inner temperatures match the series-resistance hand solution
//!    within 0.5 % of their rise.
//! 3. **Coupled energy conservation:** rods with a source in a closed water
//!    pipe: `Δ(E_fluid + E_structure) = P t` to < 1e-9 relative of `P t`.
//! 4. **Heated channel:** air through a pipe heated by a slab wall with a
//!    source `P`; at steady state the outlet energy flow exceeds the inlet by
//!    `P` within 1 %.
//! 5. **Wall heat exchanger:** counter-flow air streams through a slab wall;
//!    at steady state hot loses what cold gains within 1 %, and the
//!    effectiveness matches ε-NTU (counter-flow, `UA` from the two films and
//!    the wall) within 5 %.

use tampines::components::heat_structure::*;
use tampines_steam_tables::hem::*;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::power::watt;
use uom::si::pressure::{megapascal, pascal};
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::velocity::meter_per_second;
use uom::si::volumetric_heat_capacity::joule_per_cubic_meter_kelvin;

fn zone(th: f64, n: usize, k: f64, rc: f64) -> ConductionZone {
    ConductionZone {
        thickness: Length::new::<meter>(th),
        n_cells: n,
        conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(k),
        volumetric_heat_capacity: VolumetricHeatCapacity::new::<joule_per_cubic_meter_kelvin>(rc),
    }
}
fn kel(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}
fn sec(t: f64) -> Time {
    Time::new::<second>(t)
}

/// **Slab transient against the Fourier series.** k = 15 W/(m K),
/// ρc = 4e6 J/(m³ K) (α = 3.75e-6 m²/s), L = 0.02 m, 40 cells, Δt = 0.01 s
/// to t = 0.1 L²/α = 10.667 s. **Result (2026-11-01):** centre 347.4696 K
/// against the series 347.4487 K (+0.021 K, inside the 0.5 K bar); at
/// Δt = 0.005 s +0.010 K; the time error against a Δt = 0.000625 s reference
/// halves with Δt (ratio 2.141, asserted in [1.8, 2.2]: backward Euler, first
/// order). **Instrument corrected during the work:** the first runs rounded
/// the step count, so they ended up to 5 ms apart; at the centre's ~3 K/s
/// that swamped the time error (it read +0.006 K and +0.017 K, ratio 0.36,
/// and was first misread as spatial and temporal errors cancelling). Each
/// run now ends at exactly `t_end`.
#[test]
fn a_slab_cools_as_the_fourier_series_says() {
    let (l, k, rc) = (0.02, 15.0, 4.0e6);
    let alpha = k / rc;
    let t_end = 0.1 * l * l / alpha;
    let run = |dt: f64, n: usize| {
        let mut s = HeatStructure::new(
            StructureShape::Slab { width: Length::new::<meter>(1.0) },
            &[zone(l, n, k, rc)],
            1,
            Length::new::<meter>(1.0),
            kel(400.0),
        )
        .unwrap();
        // dt adjusted so every run ends at exactly t_end (rounding the step
        // count instead made the runs end up to 5 ms apart, which at the
        // centre's ~3 K/s cooling swamped the time error: a first attempt read
        // a ratio of 0.36).
        let steps = (t_end / dt).round() as usize;
        let dt = t_end / steps as f64;
        for _ in 0..steps {
            s.step(sec(dt), &StructureBoundary::Temperature(kel(300.0)), &StructureBoundary::Temperature(kel(300.0))).unwrap();
        }
        // The centre lies on a face for even n: average the two cells.
        0.5 * (s.temperature(0, n / 2 - 1).unwrap().value + s.temperature(0, n / 2).unwrap().value)
    };
    let fo = alpha * t_end / (l * l);
    let exact: f64 = 300.0
        + 100.0
            * (0..200)
                .map(|m| {
                    let nn = (2 * m + 1) as f64;
                    let sign = if m % 2 == 0 { 1.0 } else { -1.0 };
                    4.0 / (nn * std::f64::consts::PI) * sign * (-(nn * std::f64::consts::PI).powi(2) * fo).exp()
                })
                .sum::<f64>();
    let (a, b, r) = (run(0.01, 40), run(0.005, 40), run(0.000625, 40));
    let ratio = (a - r) / (b - r);
    println!("slab: exact centre {exact:.4} K; dt 0.01 s {a:.4} K ({:+.4} K), dt 0.005 s {b:.4} K ({:+.4} K); time-error ratio {ratio:.3}",
        a - exact, b - exact);
    assert!((a - exact).abs() < 0.5);
    assert!((1.8..2.2).contains(&ratio), "backward Euler: first order in time");
}

/// **Fuel rod at steady state against the hand solution.** Cylinder from
/// r = 0: fuel 4.1 mm (20 cells, k = 3), gap 0.08 mm (2 cells, k = 0.3),
/// cladding 0.57 mm (5 cells, k = 16); q''' = 3e8 W/m³ in the fuel; cladding
/// surface 600 K; backward Euler with Δt = 1e4 s to steady state.
/// Hand: `q' = q''' π R_f²`; cladding inner `600 + q' ln(r_co/r_ci)/(2π k_c)`;
/// fuel surface adds `q' ln(r_ci/R_f)/(2π k_g)`; at radius `r` in the fuel
/// `T = T_fs + q'''(R_f² − r²)/(4 k_f)`. **Result (2026-11-01):** centre
/// cell 1201.485 K against 1202.291 K (−0.134 % of the rise); first cladding
/// cell 617.997 K against 617.997 K (0.000 %); source 15 843.1 W = q' to 1e-12.
/// Held.
#[test]
fn a_fuel_rod_reaches_the_hand_steady_state() {
    let (rf, g, c) = (4.1e-3, 0.08e-3, 0.57e-3);
    let (kf, kg, kc, q3) = (3.0, 0.3, 16.0, 3.0e8);
    let mut s = HeatStructure::new(
        StructureShape::Cylinder { inner_radius: Length::new::<meter>(0.0), multiplicity: 1.0 },
        &[zone(rf, 20, kf, 3.0e6), zone(g, 2, kg, 1.0e3), zone(c, 5, kc, 2.0e6)],
        1,
        Length::new::<meter>(1.0),
        kel(600.0),
    )
    .unwrap();
    s.set_source(0, 0..20, VolumetricPowerDensity::new::<uom::si::volumetric_power_density::watt_per_cubic_meter>(q3)).unwrap();
    assert!(s.step(sec(1.0), &StructureBoundary::Temperature(kel(600.0)), &StructureBoundary::Temperature(kel(600.0))).is_err(), "a solid rod's axis is adiabatic");
    for _ in 0..50 {
        s.step(sec(1.0e4), &StructureBoundary::Adiabatic, &StructureBoundary::Temperature(kel(600.0))).unwrap();
    }
    let pi = std::f64::consts::PI;
    let qp = q3 * pi * rf * rf;
    let (rci, rco) = (rf + g, rf + g + c);
    let t_ci = 600.0 + qp * (rco / rci).ln() / (2.0 * pi * kc);
    let t_fs = t_ci + qp * (rci / rf).ln() / (2.0 * pi * kg);
    // Centre cell 0 spans [0, rf/20]; its area-weighted centre r² = (rf/20)²/2.
    let r0sq = 0.5 * (rf / 20.0).powi(2);
    let t_c_hand = t_fs + q3 * (rf * rf - r0sq) / (4.0 * kf);
    let t_c = s.temperature(0, 0).unwrap().value;
    // First cladding cell: compare the cladding inner face via the last gap cell? Use the
    // first cladding cell centre: r² = (a² + b²)/2.
    let (a, b) = (rci, rci + c / 5.0);
    let r1 = (0.5 * (a * a + b * b)).sqrt();
    let t_cl_hand = 600.0 + qp * (rco / r1).ln() / (2.0 * pi * kc);
    let t_cl = s.temperature(0, 22).unwrap().value;
    println!("rod: centre {t_c:.3} K vs hand {t_c_hand:.3} K ({:+.3} % of rise); first cladding cell {t_cl:.3} vs {t_cl_hand:.3} ({:+.3} % of rise); source {:.1} W",
        100.0 * (t_c - t_c_hand) / (t_c_hand - 600.0), 100.0 * (t_cl - t_cl_hand) / (t_cl_hand - 600.0), s.source_power());
    assert!(((t_c - t_c_hand) / (t_c_hand - 600.0)).abs() < 0.005);
    assert!(((t_cl - t_cl_hand) / (t_cl_hand - 600.0)).abs() < 0.005);
    assert!((s.source_power() / qp - 1.0).abs() < 1e-12);
}

fn water_pipe(n: usize) -> HemPipe<Iapws97Hem> {
    let geo = HemPipeGeometry::new(Length::new::<meter>(1.0), Area::new::<square_meter>(1.0e-3), n).unwrap();
    let s = PrimitiveState::PressureTemperature {
        pressure: Pressure::new::<megapascal>(7.0),
        temperature: kel(500.0),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    HemPipe::new(geo, Iapws97Hem, HemInitialCondition::Uniform(s), HemNumerics::default()).unwrap()
}

/// **Coupled energy conservation (CoreChannel).** 10 rods (fuel 4.1 mm /
/// gap / cladding as above, 6 + 1 + 2 cells, initial 500 K) in a closed 1 m
/// water pipe of 1e-3 m², 10 cells, at 7 MPa and 500 K, h = 3e4 W/(m² K);
/// 20 kW of fuel power; 300 coupled steps. **Result (2026-11-01):**
/// t = 9.53 ms, ΔE = 190.6910 J = P t to 4.8e-11. Held.
#[test]
fn a_core_channel_conserves_energy() {
    let mut net = HemNetwork::new(Iapws97Hem, 0.2).unwrap();
    let ip = net.add_pipe(water_pipe(10));
    let mut rod = HeatStructure::new(
        StructureShape::Cylinder { inner_radius: Length::new::<meter>(0.0), multiplicity: 10.0 },
        &[zone(4.1e-3, 6, 3.0, 3.0e6), zone(0.08e-3, 1, 0.3, 1.0e3), zone(0.57e-3, 2, 16.0, 2.0e6)],
        10,
        Length::new::<meter>(0.1),
        kel(500.0),
    )
    .unwrap();
    for j in 0..10 {
        rod.set_power(j, 0..6, Power::new::<watt>(2.0e3)).unwrap();
    }
    let p = rod.source_power();
    let mut coupler = ThermalCoupler::new();
    coupler
        .add(&net, HeatStructureLink {
            structure: rod,
            inner: SideCondition::Boundary(StructureBoundary::Adiabatic),
            outer: SideCondition::Pipe { pipe: ip, h: HeatTransfer::new::<watt_per_square_meter_kelvin>(3.0e4) },
        })
        .unwrap();
    let e0 = net.total_energy().value + coupler.stored_energy();
    let mut t = 0.0;
    for _ in 0..300 {
        t += coupler.step(&mut net).unwrap().value;
    }
    let de = net.total_energy().value + coupler.stored_energy() - e0;
    println!("core channel: P {p:.1} W, t {t:.4e} s, dE {de:.6e} J vs P t {:.6e} J ({:e}); heat to fluid {:.4e} J; clad surface {:.3} K",
        p * t, de / (p * t) - 1.0, coupler.heat_to_fluid().value, coupler.link(0).unwrap().structure.temperature(5, 8).unwrap().value);
    assert!((p / 2.0e4 - 1.0).abs() < 1e-12);
    assert!((de / (p * t) - 1.0).abs() < 1e-9);
    // A pipe of the wrong length is refused.
    let mut bad = ThermalCoupler::new();
    let s = HeatStructure::new(StructureShape::Slab { width: Length::new::<meter>(0.1) }, &[zone(0.01, 2, 1.0, 1.0e6)], 4, Length::new::<meter>(0.25), kel(300.0)).unwrap();
    assert!(bad.add(&net, HeatStructureLink { structure: s, inner: SideCondition::Pipe { pipe: ip, h: HeatTransfer::new::<watt_per_square_meter_kelvin>(1.0) }, outer: SideCondition::Boundary(StructureBoundary::Adiabatic) }).is_err());
}

fn air_pipe(t: f64, n: usize) -> HemPipe<IdealGasHem> {
    let r = IdealGasHem::air().specific_gas_constant.value;
    let geo = HemPipeGeometry::new(Length::new::<meter>(1.0), Area::new::<square_meter>(0.01), n).unwrap();
    let s = PrimitiveState::PressureDensity {
        pressure: Pressure::new::<pascal>(1.0e5),
        density: MassDensity::new::<kilogram_per_cubic_meter>(1.0e5 / (r * t)),
        velocity: Velocity::new::<meter_per_second>(0.0),
    };
    HemPipe::new(geo, IdealGasHem::air(), HemInitialCondition::Uniform(s), HemNumerics::default()).unwrap()
}

fn reservoir(p0: f64, t0: f64) -> HemBoundary {
    let r = IdealGasHem::air().specific_gas_constant.value;
    HemBoundary::Reservoir {
        stagnation_pressure: Pressure::new::<pascal>(p0),
        stagnation_enthalpy: AvailableEnergy::new::<joule_per_kilogram>(3.5 * r * t0),
    }
}

/// Mean energy flows through both ends of a pipe over the last `window` s
/// of a coupled run to `t_end`: (inlet inflow, outlet outflow), W.
fn run_and_measure(net: &mut HemNetwork<IdealGasHem>, coupler: &mut ThermalCoupler, t_end: f64, window: f64, pipes: &[usize]) -> Vec<(f64, f64, f64, f64)> {
    while net.time().value < t_end - window {
        coupler.step(net).unwrap();
    }
    let snap = |net: &HemNetwork<IdealGasHem>| -> Vec<(f64, f64, f64, f64)> {
        pipes.iter().map(|&i| {
            let p = net.pipe(i).unwrap();
            (-p.cumulative_energy_outflow(PipeEnd::Inlet).value, p.cumulative_energy_outflow(PipeEnd::Outlet).value,
             -p.cumulative_mass_outflow(PipeEnd::Inlet).value, p.cumulative_mass_outflow(PipeEnd::Outlet).value)
        }).collect()
    };
    let (s0, t0) = (snap(net), net.time().value);
    while net.time().value < t_end {
        coupler.step(net).unwrap();
    }
    let (s1, t1) = (snap(net), net.time().value);
    s0.iter().zip(&s1).map(|(a, b)| ((b.0 - a.0) / (t1 - t0), (b.1 - a.1) / (t1 - t0), (b.2 - a.2) / (t1 - t0), (b.3 - a.3) / (t1 - t0))).collect()
}

/// **Heated channel (PipeWithHeatStructure).** Air, 1 m × 0.01 m², 20 cells,
/// reservoir 1.01e5 Pa / 300 K to 1e5 Pa. Wall: a slab 2 mm thick, width
/// 0.3545 m (the pipe's perimeter), k = 15, **ρc = 4e3 J/(m³ K)**, a
/// deliberately small heat capacity so the wall settles in ~0.1 s (a
/// verification choice, not a material), 2 kW of source, outer side
/// adiabatic, inner on the pipe with h = 100 W/(m² K). Steady-state energy
/// flows over the last 0.2 s of 1 s. **Result (2026-11-01):** ṁ 0.4760 kg/s;
/// energy gain 1999.95 W against 2000 W (−0.003 %). Held. (A 0.5 s trial read
/// −1.01 %, not yet steady; the protocol was 1 s from the start.)
#[test]
fn a_heated_channel_carries_away_the_wall_power() {
    let mut net = HemNetwork::new(IdealGasHem::air(), 0.2).unwrap();
    let mut p = air_pipe(300.0, 20);
    p.set_boundary(PipeEnd::Inlet, reservoir(1.01e5, 300.0)).unwrap();
    p.set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(Pressure::new::<pascal>(1.0e5))).unwrap();
    let ip = net.add_pipe(p);
    let mut wall = HeatStructure::new(StructureShape::Slab { width: Length::new::<meter>(0.3545) }, &[zone(2.0e-3, 4, 15.0, 4.0e3)], 20, Length::new::<meter>(0.05), kel(300.0)).unwrap();
    for j in 0..20 {
        wall.set_power(j, 0..4, Power::new::<watt>(100.0)).unwrap();
    }
    let mut coupler = ThermalCoupler::new();
    coupler.add(&net, HeatStructureLink {
        structure: wall,
        inner: SideCondition::Pipe { pipe: ip, h: HeatTransfer::new::<watt_per_square_meter_kelvin>(100.0) },
        outer: SideCondition::Boundary(StructureBoundary::Adiabatic),
    }).unwrap();
    let t_end: f64 = std::env::var("HS_T_END").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let f = run_and_measure(&mut net, &mut coupler, t_end, 0.2, &[ip])[0];
    let gain = f.1 - f.0;
    println!("heated channel to {t_end} s: mdot in {:.4} out {:.4} kg/s; energy gain {gain:.2} W vs 2000 W ({:+.3} %)", f.2, f.3, 100.0 * (gain / 2000.0 - 1.0));
    assert!((gain / 2000.0 - 1.0).abs() < 0.01);
}

/// **Counter-flow wall heat exchanger.** Two air pipes as above, 20 cells:
/// hot from a 400 K reservoir at its inlet, cold from a 300 K reservoir at
/// its outlet end (so the streams run opposite), each driven by 100 Pa to
/// 1e5 Pa. Wall: slab 1 mm, width 0.3545 m, k = 15, ρc = 4e3 J/(m³ K) (as
/// above), h = 200 W/(m² K) on both sides. Hand: `UA = L/(1/(hW) + δ/(kW) +
/// 1/(hW))`; with the measured capacity rates, counter-flow ε-NTU,
/// `ε = (1 − e^{−NTU(1−Cr)})/(1 − Cr e^{−NTU(1−Cr)})`. Last 0.2 s of 1.5 s.
/// **Result (2026-11-01):** ṁ hot 0.1391, cold 0.1435 kg/s; q hot = q cold =
/// 2807.98 W (0.000 %); NTU 0.2520, Cr 0.9694; ε 0.20093 against 0.20189
/// (−0.48 %). Held.
#[test]
fn a_wall_heat_exchanger_matches_epsilon_ntu() {
    let mut net = HemNetwork::new(IdealGasHem::air(), 0.2).unwrap();
    let mut hot = air_pipe(400.0, 20);
    hot.set_boundary(PipeEnd::Inlet, reservoir(1.001e5, 400.0)).unwrap();
    hot.set_boundary(PipeEnd::Outlet, HemBoundary::FixedPressure(Pressure::new::<pascal>(1.0e5))).unwrap();
    let mut cold = air_pipe(300.0, 20);
    cold.set_boundary(PipeEnd::Outlet, reservoir(1.001e5, 300.0)).unwrap();
    cold.set_boundary(PipeEnd::Inlet, HemBoundary::FixedPressure(Pressure::new::<pascal>(1.0e5))).unwrap();
    let ih = net.add_pipe(hot);
    let ic = net.add_pipe(cold);
    let (w, d, k, h) = (0.3545, 1.0e-3, 15.0, 200.0);
    let wall = HeatStructure::new(StructureShape::Slab { width: Length::new::<meter>(w) }, &[zone(d, 4, k, 4.0e3)], 20, Length::new::<meter>(0.05), kel(350.0)).unwrap();
    let mut coupler = ThermalCoupler::new();
    let hh = HeatTransfer::new::<watt_per_square_meter_kelvin>(h);
    coupler.add(&net, HeatStructureLink { structure: wall, inner: SideCondition::Pipe { pipe: ih, h: hh }, outer: SideCondition::Pipe { pipe: ic, h: hh } }).unwrap();
    let t_end: f64 = std::env::var("HS_T_END").ok().and_then(|v| v.parse().ok()).unwrap_or(1.5);
    let f = run_and_measure(&mut net, &mut coupler, t_end, 0.2, &[ih, ic]);
    let (fh, fc) = (f[0], f[1]);
    // Hot flows inlet → outlet; cold flows outlet → inlet (its "inflow" at
    // the inlet end is negative).
    let q_hot = fh.0 - fh.1;
    let q_cold = (-fc.0) - (-fc.1);
    let cp = 3.5 * IdealGasHem::air().specific_gas_constant.value;
    let (c_h, c_c) = (fh.2 * cp, -fc.3 * cp);
    let (c_min, c_max) = (c_h.min(c_c), c_h.max(c_c));
    let ua = 1.0 / (2.0 / (h * w) + d / (k * w));
    let (ntu, cr) = (ua / c_min, c_min / c_max);
    let x = (-ntu * (1.0 - cr)).exp();
    let eps_hand = (1.0 - x) / (1.0 - cr * x);
    let eps = q_hot / (c_min * 100.0);
    println!("heat exchanger to {t_end} s: mdot hot {:.4} cold {:.4} kg/s; q hot {q_hot:.2} W, q cold {q_cold:.2} W ({:+.3} %); NTU {ntu:.4}, Cr {cr:.4}; eps {eps:.5} vs hand {eps_hand:.5} ({:+.2} %)",
        fh.2, -fc.3, 100.0 * (q_cold / q_hot - 1.0), 100.0 * (eps / eps_hand - 1.0));
    assert!((q_cold / q_hot - 1.0).abs() < 0.01);
    assert!((eps / eps_hand - 1.0).abs() < 0.05);
}
