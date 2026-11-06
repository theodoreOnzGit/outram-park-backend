//! # V&V: `tampines`'s wet well against `sembawang`'s Mark I torus (gh:#1075)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! Moved here from `crates/tampines/tests/wet_well.rs` on 2026-11-01, at the
//! maintainer's direction: `sembawang` (severe accidents, orchestration)
//! depends on `tampines` (thermal hydraulics), never the reverse, so the
//! comparison against `sembawang`'s originals lives on this side of the edge.
//! The predictions were written before the first run (in the original file,
//! item 3); the results are on each test.
//!
//! Geometry: `sembawang`'s Fukushima Daiichi Unit 1 torus (`bwr_mark1::params`):
//! pool surface 750 m², pool 1750 m³ at 995 kg/m³, gas space 2620 m³.

use sembawang::bwr_mark1::containment::{Containment, ContainmentSnapshot};
use sembawang::bwr_mark1::params::Params;
use sembawang::bwr_mark1::{gas, pool_surface};
use tampines::components::wet_well::*;
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::v_tp_1;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;

const AREA: f64 = 750.0;
const GAS_M3: f64 = 2620.0;
const POOL_KG: f64 = 1750.0 * 995.0;
const P0: f64 = 1.05e5;
const T0: f64 = 303.15;
const SUBMERGENCE_PA: f64 = 11_800.0;

fn kgs(x: f64) -> MassRate {
    MassRate::new::<kilogram_per_second>(x)
}
fn kelv(x: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(x)
}
fn pa(x: f64) -> Pressure {
    Pressure::new::<pascal>(x)
}
fn sec(x: f64) -> Time {
    Time::new::<second>(x)
}
fn jkg(x: f64) -> AvailableEnergy {
    AvailableEnergy::new::<joule_per_kilogram>(x)
}

fn rho_if97(t: f64, p: f64) -> f64 {
    1.0 / v_tp_1(kelv(t), pa(p)).get::<cubic_meter_per_kilogram>()
}

fn breaker(direction: VacuumBreakerDirection) -> VacuumBreaker {
    VacuumBreaker {
        direction,
        opening_pressure_difference: pa(3400.0),
        flow_area: Area::new::<square_meter>(0.5),
        discharge_coefficient: 0.6,
    }
}

fn torus_at(t_pool: f64, t_gas: f64, rh: f64, direction: VacuumBreakerDirection) -> WetWell {
    torus_at_p(t_pool, t_gas, rh, P0, direction)
}

fn torus_at_p(t_pool: f64, t_gas: f64, rh: f64, p0: f64, direction: VacuumBreakerDirection) -> WetWell {
    let level = POOL_KG / (rho_if97(t_pool, p0) * AREA);
    let submergence = SUBMERGENCE_PA / (995.0 * 9.81);
    WetWell::new(
        WetWellGeometry {
            cross_section: Area::new::<square_meter>(AREA),
            total_height: Length::new::<meter>(level + GAS_M3 / AREA),
            injection_elevation: Length::new::<meter>(level - submergence),
            injection_opening: Length::new::<meter>((4.0 * 20.0 / (96.0 * std::f64::consts::PI)).sqrt()),
        },
        breaker(direction),
        WetWellInitialState {
            pool_level: Length::new::<meter>(level),
            gas_pressure: pa(p0),
            pool_temperature: kelv(t_pool),
            gas_temperature: kelv(t_gas),
            relative_humidity: rh,
        },
    )
    .unwrap()
}

fn torus() -> WetWell {
    torus_at(T0, T0, 1.0, VacuumBreakerDirection::OutOfGasSpace)
}

fn quiet(external_pa: f64) -> WetWellInputs {
    WetWellInputs {
        injection: Injection::none(),
        external: ExternalVolume { pressure: pa(external_pa), temperature: kelv(T0), steam_mole_fraction: 0.0 },
        heat_exchanger: ImmersedHeatExchanger::off(),
        liquid_outflow: kgs(0.0),
    }
}

/// **3a. The ported primitives are bit for bit sembawang's.**
/// `exchange` over a grid of pool and gas states (no hydrogen), from
/// evaporation into dry gas to condensation from nearly pure steam;
/// `bubble_efficiency`, `t_sat`, `steam_h`, and N2's `c_v` and `u` with
/// and without the fit.
///
/// **Result (2026-11-01, held):** 300 surface states, 5 bubble
/// efficiencies and 10 N2 `c_v`/`u` pairs, all bit-identical.
#[test]
fn the_ported_primitives_are_bit_for_bit_sembawangs() {
    let mut n_cases = 0;
    for &t_pool in &[290.0_f64, 303.15, 330.0, 360.0, 385.0] {
        for &t_gas in &[300.0, 330.0, 380.0, 420.0] {
            for &x_steam in &[0.0, 0.05, 0.4, 0.9, 0.999] {
                for &(p, dt) in &[(1.05e5, 1.0), (3.0e5, 0.5), (6.0e5, 2.0)] {
                    let n = p * 2620.0 / (R_GAS * t_gas);
                    let (ns, nn) = (x_steam * n, (1.0 - x_steam) * n);
                    let rho = rho_if97(t_pool.min(370.0), p);
                    let a = exchange(
                        &PoolState { t_k: t_pool, kg: 1.7e6, rho, cp: 4186.0 },
                        &Atmosphere { t_k: t_gas, p_pa: p, n_steam: ns, n_n2: nn },
                        &SurfaceGeometry { area_m2: 750.0, pool_depth_m: 2.3, gas_depth_m: 3.5 },
                        dt,
                    );
                    let b = pool_surface::exchange(
                        &pool_surface::Pool { t_k: t_pool, kg: 1.7e6, rho, cp: 4186.0 },
                        &pool_surface::Atmosphere { t_k: t_gas, p_pa: p, n_steam: ns, n_n2: nn, n_h2: 0.0 },
                        &pool_surface::Surface { area_m2: 750.0, pool_depth_m: 2.3, gas_depth_m: 3.5 },
                        dt,
                    );
                    let ab = [a.evaporation_kg_s, a.q_pool_w, a.q_atm_w, a.t_surface_k].map(f64::to_bits);
                    let bb = [b.evaporation_kg_s, b.q_pool_w, b.q_atm_w, b.t_surface_k].map(f64::to_bits);
                    assert_eq!(ab, bb, "{t_pool} {t_gas} {x_steam} {p}: {a:?} vs {b:?}");
                    n_cases += 1;
                }
            }
        }
    }
    for &(d, h, p, t) in &[(1.2, 0.5, 2.0e5, 380.0), (0.26, 0.515, 1.2e5, 360.0), (1.0, 0.5, 1.1e5, 375.0), (1.0, 0.5, 1.1e5, 373.9), (0.0, 0.5, 1e5, 300.0)] {
        assert_eq!(bubble_efficiency(d, h, p, t).to_bits(), pool_surface::bubble_efficiency(d, h, p, t).to_bits());
    }
    // `pool_surface::t_sat` and `steam_h` are crate-private in
    // sembawang; `t_sat` is reached through `ε_T` in the partial cases
    // above (and in every `exchange` case), `steam_h` through
    // `exchange`'s latent heat.
    for &t in &[250.0, 303.15, 700.0, 4000.0, 6000.0] {
        for tv in [true, false] {
            assert_eq!(cv_n2(t, tv).to_bits(), gas::cv_n2(t, tv).to_bits(), "{t} {tv}");
            assert_eq!(u_n2(t, tv).to_bits(), gas::u_n2(t, tv).to_bits(), "{t} {tv}");
        }
    }
    println!("bit-identical: {n_cases} surface states, 5 bubble efficiencies, 10 N2 c_v/u pairs");
}

/// **3b. The whole torus against sembawang's `Containment`**, stepped
/// with relief steam only (10 kg/s at 2.7e6 J/kg, 600 s at 1 s), its
/// wetwell atmosphere on and the mechanisms that touch nothing here off
/// (no pool layers, no shell, no scrubbing); the wet well here built to
/// the same pool mass and gas volume, the port at the downcomers' depth.
///
/// **Result (2026-11-01, held):** pool 305.261372 K against sembawang's
/// 305.261414 K (−4.2e-5 K; +5.4e-6 K without the pressure work, which
/// sembawang does not have); gas pressure +349.3 Pa (105693.96 against
/// 105344.68), inside the predicted 150–400 Pa from the 9.6 mm rise of
/// the pool compressing the gas. Both at rest read 105000.000 Pa.
///
/// **Re-pinned 2026-11-02 (gh:#1118, maintainer decision).** The wet well's
/// default rain-out became the control volume's equilibrium solve, and
/// `sembawang`'s four passes became its ablation
/// `without_equilibrium_rain_out`. This test is a port check, so the record
/// above stays on the ablation (`w`, `wa`), which reproduces it bit for bit
/// (tampines' `four_pass_ablation_is_the_old_wet_well_bit_for_bit` pins this
/// trajectory's hash on the pre-change code). The new default (`wd`) is run
/// beside it, against the same criteria.
///
/// **Prediction E3 (written before the change):** the pool temperature
/// moves < 1e-4 K and the gas pressure 0 to +30 Pa from the ablation's
/// numbers. Both criteria still hold.
///
/// **Result (2026-11-02, release).** The ablation is unchanged: 305.261372 K,
/// +349.28 Pa. The new default ends at 305.261372 K (−6.1e-8 K against the
/// ablation, −4.21e-5 K against `sembawang`) and 105693.95 Pa (−0.01 Pa
/// against the ablation, +349.27 Pa against `sembawang`), with 80.187 kg of
/// steam in both. The pool temperature held. **The pressure band missed**
/// (predicted 0 to +30 Pa, measured −0.01 Pa): in this relief case the
/// surface warms the gas from 303 K to 304.2 K, so it is mostly
/// subsaturated and the rain-out barely acts. Both criteria hold.
#[test]
fn the_torus_tracks_sembawangs_containment() {
    let prm = Params::default();
    let mut c = Containment::new(&prm);
    let snap = ContainmentSnapshot {
        dt_s: 1.0,
        srv_steam_kg_s: 10.0,
        srv_h: 2.7e6,
        gas_cv_t: true,
        ww_atmosphere: true,
        two_sided_equalising: true,
        outflow_enthalpy: true,
        prm,
        ..Default::default()
    };
    let mut w = torus().without_equilibrium_rain_out();
    let mut wa = torus().without_equilibrium_rain_out().without_pressure_work();
    let mut wd = torus();
    let inputs = WetWellInputs { injection: Injection::water(kgs(10.0), jkg(2.7e6)), ..quiet(P0) };
    let mut r = c.step(&ContainmentSnapshot { srv_steam_kg_s: 0.0, ..snap.clone() });
    // The first step above carried no steam: compare its gas pressure
    // with ours at rest.
    let p_rest = w.gas_pressure().get::<pascal>();
    println!("at rest: sembawang p_ww {:.3} Pa, here {p_rest:.3} Pa", r.p_ww_pa);
    let mut worst_t: f64 = 0.0;
    let mut worst_td: f64 = 0.0;
    for _ in 0..600 {
        r = c.step(&snap);
        w.step(sec(1.0), &inputs).unwrap();
        wa.step(sec(1.0), &inputs).unwrap();
        wd.step(sec(1.0), &inputs).unwrap();
        worst_t = worst_t.max((w.pool_temperature().get::<kelvin>() - r.t_pool_k).abs());
        worst_td = worst_td.max((wd.pool_temperature().get::<kelvin>() - r.t_pool_k).abs());
    }
    let dp = w.gas_pressure().get::<pascal>() - r.p_ww_pa;
    let dt = w.pool_temperature().get::<kelvin>() - r.t_pool_k;
    let dt_a = wa.pool_temperature().get::<kelvin>() - r.t_pool_k;
    println!(
        "vs sembawang after 600 s: T_pool {:.6} vs {:.6} K ({dt:+.3e}; without pressure work {dt_a:+.3e}; worst {worst_t:.3e}), p {:.2} vs {:.2} Pa ({dp:+.2}), level +{:.5} m, T_gas {:.4} K, p_dw {:.1}",
        w.pool_temperature().get::<kelvin>(),
        r.t_pool_k,
        w.gas_pressure().get::<pascal>(),
        r.p_ww_pa,
        w.pool_level().get::<meter>() - POOL_KG / (rho_if97(T0, P0) * AREA),
        w.gas_temperature().get::<kelvin>(),
        r.p_dw_pa,
    );
    assert!(worst_t < 2e-3, "{worst_t}");
    assert!(dp > 150.0 && dp < 400.0, "{dp}");
    let dp_d = wd.gas_pressure().get::<pascal>() - r.p_ww_pa;
    let dt_d = wd.pool_temperature().get::<kelvin>() - r.t_pool_k;
    println!(
        "new default (equilibrium rain-out): T_pool {:.6} K ({dt_d:+.3e} vs sembawang; {:+.3e} vs the ablation; worst {worst_td:.3e}), p {:.2} Pa ({dp_d:+.2} vs sembawang; {:+.2} vs the ablation), T_gas {:.4} K, steam {:.3} kg vs {:.3}",
        wd.pool_temperature().get::<kelvin>(),
        wd.pool_temperature().get::<kelvin>() - w.pool_temperature().get::<kelvin>(),
        wd.gas_pressure().get::<pascal>(),
        wd.gas_pressure().get::<pascal>() - w.gas_pressure().get::<pascal>(),
        wd.gas_temperature().get::<kelvin>(),
        wd.steam_mass().get::<uom::si::mass::kilogram>(),
        w.steam_mass().get::<uom::si::mass::kilogram>(),
    );
    assert!(worst_td < 2e-3, "{worst_td}");
    assert!(dp_d > 150.0 && dp_d < 400.0, "{dp_d}");
}
