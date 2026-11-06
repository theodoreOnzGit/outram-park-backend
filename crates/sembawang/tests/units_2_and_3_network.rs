//! # V&V: Fukushima Daiichi Units 2 and 3 on the containment network (gh:#1101)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! `NetworkConfig::unit2()` / `unit3()` with `Params::unit2()` / `unit3()`.
//! Every value's mark and source is in those constructors' docs and in
//! `verification_and_validation/bwr_mark1/units23_open_data.md`; the record
//! of this test is in `verification_and_validation/bwr_mark1/containment_network.md`
//! ("Units 2 and 3 by configuration").
//!
//! **No comparison with the accident is made or implied.** ~~This crate has no
//! RCIC or HPCI model and no Unit 2 or 3 sequence (`Mode::Historical` is
//! Unit 1's), so these tests~~ **CORRECTED 2026-11-02 (gh:#1109):** the crate
//! now has both (`turbine_pump`, `units23`, `Plant::for_unit`; the
//! comparison is `tests/units_2_and_3_sequence.rs`); these tests still
//! check only that each configuration builds the
//! geometry its sourced values imply and that a short run conserves what it
//! should.
//!
//! ## Methodology
//!
//! - **G (geometry).** Build each network and read the ASSEMBLED volumes
//!   (their V(z) tables, ports and paths), not the constants: the torus's
//!   gas and pool volumes against the sourced 3160 and 2980 m³, its tube
//!   radius against the sourced 4.45 m, the pool's area at the level against
//!   `Params::ww_pool_surface_m2`, the downcomers' total area and diameter,
//!   the drywell's and the building's tops.
//! - **A' (a short run).** Case A of `containment_network_vs_containment.rs`
//!   (10 kg/s of relief steam at 2.7 MJ/kg into the torus, 1 s steps, the
//!   same snapshot flags), for 300 s, on Units 1, 2 and 3, every mechanism
//!   on. Instrument: the network's own balance, `water + each gas + energy
//!   = start + CvNetwork::boundary_inflow()` (the pattern of
//!   `reaction_in_a_control_volume.rs`' R6: the structures' heat and
//!   condensation cross the network's boundary as external heats and
//!   sinks, so the balance is on the network, not on the network plus the
//!   lumped walls); and the pool's heating against the energy that entered.
//!
//! ## Predictions, written 2026-11-01 before the first run (hand arithmetic)
//!
//! **G.** For Units 2 and 3 (one configuration):
//!
//! 1. The torus's gas space 3160 m³ and its pool `2980 × 995` kg, so about
//!    2978 m³ at IF97's 995.6 kg/m³, to 1e-4 relative.
//! 2. Its tube radius, fitted to those volumes on R = 16.75 m, 4.309 m
//!    (±0.002); the free volume 0.9375 (±0.002) of the sourced geometric
//!    torus (r = 4.45 m), as the V&V data file's 0.938.
//! 3. The initial level 4.208 m (±0.01) above the torus bottom, its area
//!    907 m² (`Params::ww_pool_surface_m2`, within 0.5 %).
//! 4. The downcomers' exits 1.209 m below the level (11 800 Pa at
//!    995 kg/m³), each 0.606 m across (27.71 m² over 96); their total area
//!    1.039 of the main vents' 26.66 m² (8 × 2.06 m); 14 pool layers.
//! 5. The drywell's top 26.94 m (±0.05) and the building's 58.75 m (±0.01),
//!    the building above the drywell.
//!
//! **A'.** After 300 s:
//!
//! 6. The network balances water and each gas to 1e-9 of what entered
//!    (3000 kg; each gas's initial mass) and energy to 1e-9 of the energy
//!    that entered, on all three units.
//! 7. The pool's mean temperature rises 0.62 K (±5 %) on Units 2 and 3 and
//!    1.06 K (±5 %) on Unit 1: about 7.7 GJ into 2.97 Mt and 1.74 Mt of
//!    water; the ratio 0.587 (±0.01).
//! 8. No downcomer or vacuum-breaker flow (below 1e-9 kg/s) on any unit; at
//!    most 30 Picard iterations a step.
//! 9. Unit 3 bit-identical to Unit 2 (the disk is not reached, and the
//!    network does not read the vessel's height, their only difference).
//!
//! ## Results (2026-11-01, release, Rust 1.99.1)
//!
//! **G, Units 2 and 3 (identical):** gas 3160.000 m³; pool 2 965 100 kg at
//! 995.654 kg/m³ (2978.044 m³); tube radius 4.3087 m; free/geometric
//! 0.9375; level 4.2083 m; area 906.26 m² (the constant set to 906.3 after
//! the run; the hand value 907 was within 0.08 %); exits 1.2089 m below
//! the level; downcomers 0.6063 m × 96 = 27.713 m², 1.0394 of the main
//! vents' 26.663 m²; 14 layers; drywell top 26.939 m; building top
//! 58.745 m. Predictions 1–5 held. One instrument fix after the first run:
//! the exit was asserted to 1e-9 m against the volume's reported level,
//! which differs from the constructor's by 1.2e-8 m (see the test).
//!
//! **A', 300 s:**
//!
//! | unit | ΔT_pool, K | p_ww, Pa | water residual, kg | energy residual / inflow | gas residual | iterations |
//! |---|---|---|---|---|---|---|
//! | 1 | +1.0590 | 105 269.3 | −1.16e-9 | −3.4e-14 | 0 | ≤ 5 |
//! | 2 | +0.6224 | 105 198.9 | +2.33e-9 | −5.3e-14 | 0 | ≤ 5 |
//! | 3 | = Unit 2, bit for bit | | | | | |
//!
//! Predictions 6–9 held: the ratio of the heatings is 0.5877; no downcomer
//! or breaker flow; Unit 3 bit-identical to Unit 2. **What this does not
//! test:** the gases' balance is trivial here (nothing carries gas in or
//! out), and the vent, the rupture disk, the flange, the fission products
//! and the burns are not reached in 300 s of relief steam. The water
//! residuals are round-off on 3e6 kg (one ulp there is 4.7e-10 kg).

use sembawang::bwr_mark1::containment::ContainmentSnapshot;
use sembawang::bwr_mark1::containment_network::{ContainmentNetwork, NetworkConfig, NetworkMechanisms, NetworkResult};
use sembawang::bwr_mark1::params::Params;
use uom::si::area::square_meter;
use uom::si::energy::joule;
use uom::si::f64::{Length, Volume};
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::volume::cubic_meter;

/// The volumes' indices in the order `with_config` adds them, and the paths'.
const DW: usize = 0;
const WW: usize = 1;
const RB: usize = 2;
const DOWNCOMER: usize = 0;

fn units() -> [(&'static str, Params, NetworkConfig); 3] {
    [("unit1", Params::default(), NetworkConfig::unit1()), ("unit2", Params::unit2(), NetworkConfig::unit2()), ("unit3", Params::unit3(), NetworkConfig::unit3())]
}

/// **G (predictions 1–5).** Results (2026-11-01) in the module docs: all
/// held.
#[test]
fn units_2_and_3_build_the_geometry_their_sources_imply() {
    for (name, prm, cfg) in units().into_iter().skip(1) {
        let n = ContainmentNetwork::with_config(&prm, cfg, NetworkMechanisms::default()).unwrap();
        let net = n.network();
        let ww = net.volume(WW).unwrap();
        let table = ww.table();
        let v_total = table.total_volume().get::<cubic_meter>();
        let pool_kg = ww.pool_mass().get::<kilogram>();
        let rho = ww.pool_density().get::<kilogram_per_cubic_meter>();
        let v_gas = ww.atmosphere_volume().get::<cubic_meter>();
        // 1.
        assert_eq!(pool_kg, 2980.0 * 995.0, "{name} G1 pool");
        assert!((v_gas / 3160.0 - 1.0).abs() < 1e-4, "{name} G1 gas {v_gas}");
        assert!((v_total - v_gas - pool_kg / rho).abs() < 1e-4 * v_total, "{name} G1 total");
        // 2. The table's top is the tube's diameter.
        let top = table.height_of_volume(Volume::new::<cubic_meter>(v_total)).get::<meter>();
        let r = 0.5 * top;
        let pi = std::f64::consts::PI;
        let free_over_geometric = v_total / (2.0 * pi * pi * 16.75 * 4.45 * 4.45);
        // 3.
        let level = ww.pool_level().get::<meter>();
        let area = table.area_at(Length::new::<meter>(level)).get::<square_meter>();
        // 4.
        let (exit, quencher) = n.port_elevations();
        let d_dc = ww.ports()[0].opening_height.get::<meter>();
        let dc_area = net.path(DOWNCOMER).unwrap().path.geometry().area.get::<square_meter>();
        let vents = 8.0 * 0.25 * pi * 2.06 * 2.06;
        // 5.
        let dw_top = net.volume(DW).unwrap().ports()[2].elevation.get::<meter>() + 0.2;
        let rb_top = net.volume(RB).unwrap().ports()[1].elevation.get::<meter>() + 1.0;
        println!(
            "{name} G: gas {v_gas:.3} m3, pool {pool_kg:.0} kg at {rho:.3} kg/m3 ({:.3} m3), r {r:.4} m, free/geometric {free_over_geometric:.4}, level {level:.4} m, area {area:.2} m2 (params {:.1}), exit {exit:.4} m (level - {:.4}), quenchers {quencher:.2} m, downcomer {d_dc:.4} m x 96 = {dc_area:.3} m2 ({:.4} of the vents' {vents:.3} m2), {} layers, drywell top {dw_top:.3} m, building top {rb_top:.3} m",
            pool_kg / rho,
            prm.ww_pool_surface_m2,
            level - exit,
            dc_area / vents,
            n.pool_layer_count()
        );
        assert!((r - 4.309).abs() < 0.002, "{name} G2 r {r}");
        assert!((free_over_geometric - 0.9375).abs() < 0.002, "{name} G2 {free_over_geometric}");
        assert!((level - 4.208).abs() < 0.01, "{name} G3 level {level}");
        assert!((area / prm.ww_pool_surface_m2 - 1.0).abs() < 0.005, "{name} G3 area {area}");
        println!("{name} G4: level - exit - submergence = {:.3e} m", level - exit - 11_800.0 / (995.0 * 9.81));
        // The constructor places the exits below its own level (the pool's
        // volume at IF97's density at the initial containment pressure);
        // the volume reports its level from its own state, 1.2e-8 m apart.
        // The first run asserted 1e-9 m against the reported level, an
        // instrument wrong for that reason, not a geometry miss; the
        // prediction was stated to the millimetre.
        assert!((level - exit - 11_800.0 / (995.0 * 9.81)).abs() < 1e-6, "{name} G4 exit");
        assert!((d_dc - 0.606).abs() < 0.001 && dc_area == prm.downcomer_area_m2, "{name} G4 {d_dc} {dc_area}");
        assert!((dc_area / vents - 1.039).abs() < 0.001, "{name} G4 vents");
        assert_eq!(n.pool_layer_count(), 14, "{name} G4 layers");
        assert!((dw_top - 26.94).abs() < 0.05, "{name} G5 drywell {dw_top}");
        assert!((rb_top - 58.75).abs() < 0.01 && rb_top > dw_top, "{name} G5 building {rb_top}");
    }
}

fn relief(prm: Params) -> ContainmentSnapshot {
    ContainmentSnapshot {
        dt_s: 1.0,
        srv_steam_kg_s: 10.0,
        srv_h: 2.7e6,
        gas_cv_t: true,
        ww_atmosphere: true,
        dw_wall_hs: true,
        two_sided_equalising: true,
        outflow_enthalpy: true,
        prm,
        ..Default::default()
    }
}

/// One unit through 300 s of case A: the last result, the start's pool
/// temperature, the water, gas and energy residuals of the network's
/// balance, and the most Picard iterations in a step.
fn run_a(prm: Params, cfg: NetworkConfig) -> (NetworkResult, f64, f64, [f64; 6], f64, usize) {
    let mut n = ContainmentNetwork::with_config(&prm, cfg, NetworkMechanisms::default()).unwrap();
    let t0 = n.network().volume(WW).unwrap().pool_temperature().get::<uom::si::thermodynamic_temperature::kelvin>();
    let (w0, g0, e0) = (n.network().water_mass().get::<kilogram>(), n.network().ncg_masses(), n.network().total_energy().get::<joule>());
    let snap = relief(prm);
    let mut most = 0;
    let mut r = NetworkResult::default();
    for _ in 0..300 {
        r = n.step(&snap).unwrap();
        most = most.max(r.iterations);
    }
    let net = n.network();
    let (bw, bg, be) = net.boundary_inflow();
    let dw = net.water_mass().get::<kilogram>() - w0 - bw;
    let de = net.total_energy().get::<joule>() - e0 - be;
    let g1 = net.ncg_masses();
    let mut dg = [0.0; 6];
    for k in 0..6 {
        dg[k] = (g1[k] - g0[k] - bg[k]) / g0[k].max(1.0);
    }
    (r, t0, dw, dg, de, most)
}

/// **A' (predictions 6–9).** Results (2026-11-01) in the module docs: all
/// held; energy closes to 5e-14 of the inflow.
#[test]
fn a_short_relief_run_conserves_on_units_1_2_and_3() {
    let mut dt = Vec::new();
    let mut results = Vec::new();
    for (name, prm, cfg) in units() {
        let (r, t0, dw, dg, de, most) = run_a(prm, cfg);
        let water_in = 10.0 * 300.0;
        let energy_in = water_in * 2.7e6;
        let worst_g = dg.iter().fold(0.0_f64, |a, b| a.max(b.abs()));
        println!(
            "{name} A': p_dw {:.1} Pa, p_ww {:.1} Pa, T_pool {:.4} K (+{:.4} K), level {:.4} m, downcomer {:.2e} kg/s, VB {:.2e} kg/s, iterations <= {most}; residuals water {dw:.3e} kg, gas (relative) {worst_g:.3e}, energy {de:.3e} J ({:.3e} of the inflow)",
            r.p_dw_pa,
            r.p_ww_pa,
            r.t_pool_k,
            r.t_pool_k - t0,
            r.pool_level_m,
            r.downcomer_kg_s,
            r.vacuum_breaker_kg_s,
            de / energy_in
        );
        // 6.
        assert!(dw.abs() < 1e-9 * water_in, "{name} A'6 water {dw}");
        assert!(worst_g < 1e-9, "{name} A'6 gas {dg:?}");
        assert!(de.abs() < 1e-9 * energy_in, "{name} A'6 energy {de}");
        // 8.
        assert!(r.downcomer_kg_s.abs() < 1e-9 && r.vacuum_breaker_kg_s.abs() < 1e-9, "{name} A'8 {r:?}");
        assert!(most <= 30, "{name} A'8 iterations {most}");
        dt.push(r.t_pool_k - t0);
        results.push(r);
    }
    // 7.
    assert!((dt[0] / 1.06 - 1.0).abs() < 0.05, "A'7 unit 1 {}", dt[0]);
    assert!((dt[1] / 0.62 - 1.0).abs() < 0.05, "A'7 unit 2 {}", dt[1]);
    assert!((dt[1] / dt[0] - 0.587).abs() < 0.01, "A'7 ratio {}", dt[1] / dt[0]);
    // 9.
    assert_eq!(results[1], results[2], "A'9 unit 3 against unit 2");
}
