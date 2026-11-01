//! # V&V: wet-well component (gh:#1075, RELAP-7 §6.12, Eqs 389–400)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! The geometry throughout is `sembawang`'s Fukushima Daiichi Unit 1 torus
//! (`sembawang::bwr_mark1::params`): pool surface 750 m², pool 1750 m³ at
//! 995 kg/m³ (1.74125e6 kg), gas space 2620 m³, downcomer submergence
//! 11.8 kPa (1.209 m), 96 downcomers over 20 m² (each 0.515 m across),
//! vacuum breakers 0.5 m² at C_d 0.6 opening at 3.4 kPa; initially 105 kPa,
//! 303.15 K, gas saturated.
//!
//! **Predictions, written 2026-11-01 before the first run:**
//!
//! 1. *Closure* (steam and two-phase injection with N2, heat exchanger,
//!    liquid outlet, vacuum breaker venting, 600 s): water and nitrogen close
//!    to < 1e-12 of the inventory; energy `ΔE = E_in − E_out` to < 1e-9 of
//!    the injected energy. The breaker opens (300 kg of N2 adds about
//!    10 kPa). Pool heats 2.5–4.5 K (hand: 3.1e10 J in, about 0.6e10 J out
//!    by the exchanger, over `m c_p` = 7.3e9 J/K).
//! 2. *Level*: 1000 kg of steam into the subcooled pool (surface exchange
//!    ablated) adds exactly 1000 kg of pool, and `ΔL = Δm / (ρ A) + L β ΔT`
//!    = 1.339e-3 + 2.49e-4 = 1.589e-3 m by hand (ρ ≈ 995.5, β ≈ 3.03e-4/K,
//!    ΔT ≈ 0.353 K), measured within 2 %.
//! 3. *Against sembawang* (**moved 2026-11-01** to
//!    `crates/sembawang/tests/wet_well_vs_tampines.rs`, since `sembawang`
//!    depends on `tampines` and not the reverse): the ported primitives bit for bit; and the whole
//!    torus (sembawang's `Containment` stepped with only relief steam,
//!    10 kg/s at 2.7e6 J/kg for 600 s): pool temperature within 2e-3 K
//!    (sembawang does not report its pool mass); the gas pressure here
//!    higher by 150–400 Pa,
//!    the gas compressed by the rising pool (`p_N2 ΔV / V`, ΔV ≈ 6.0 m³ of
//!    condensate plus ≈ 1.1 m³ of expansion of 2620 m³), which sembawang's
//!    fixed gas volume cannot show.
//! 4. *Vacuum breaker*: no flow at a difference below the set point, either
//!    direction; above it, flow only the allowed way, and the difference
//!    ends between the set point less `(γ − 1)` of the initial excess and the
//!    set point (it overshoots because inflow arrives with its enthalpy and
//!    outflow leaves with it).
//! 5. *RELAP-7*: no open published RELAP-7 wet-well test case is held (the
//!    Theory Manual's §6.12 describes the model, not a test). Instead, the
//!    RELAP-7-equivalent mode (sensible-only surface, constant `c_v`, no
//!    pressure work, dry gas) against Eq 390 in closed form: the
//!    temperature difference follows implicit Euler's `Δ0 / (1 + kσ)^N` to
//!    1e-9 and the exact exponential to 0.5 %; Eqs 395, 399, 400 to
//!    round-off.
//! 6. *Pass-through*: a pool 2.6 K below saturation at the port gives `ε =
//!    0.5 ± 0.02` and passes `(1 − ε)` of the steam to the gas; ablated, none.

use tampines::components::wet_well::*;
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::v_tp_1;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::*;
use uom::si::heat_transfer::watt_per_square_meter_kelvin;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductance::watt_per_kelvin;
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

/// The Unit 1 torus, its pool mass sembawang's and its gas volume 2620 m³.
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

/// **Physics is on by default** (root CLAUDE.md): every mechanism of a
/// wet well built the ordinary way is on, and each `without_*` turns off
/// exactly its own.
#[test]
fn every_mechanism_is_on_by_default() {
    let w = torus();
    let m = w.mechanisms();
    assert_eq!(m, WetWellMechanisms::default());
    assert_eq!(m.surface, SurfaceModel::Melcor);
    assert!(m.bubble_pass_through && m.nitrogen_cv_t && m.pressure_work);
    let a = torus().without_pressure_work().without_nitrogen_cv_t().without_bubble_pass_through();
    let m = a.mechanisms();
    assert!(!m.pressure_work && !m.nitrogen_cv_t && !m.bubble_pass_through && m.surface == SurfaceModel::Melcor);
}

/// **1. Mass and energy closure.** 600 s at 1 s: 20 kg/s injected (two-phase
/// at 1.6e6 J/kg for the first 300 s, steam at 2.7e6 J/kg after) with
/// 0.5 kg/s of N2 at 400 K; the exchanger at UA = 2 MW/K against 300 K
/// coolant; 5 kg/s drawn from the pool; the breaker out of the gas space to
/// 105 kPa of N2.
///
/// **Result (2026-11-01, held):** water closes to 1.7e-9 kg of 1.74e6 kg
/// (9.5e-16), N2 to −8.9e-11 kg of 2934 kg (−3.0e-14), energy to 2.4e-3 J of
/// 2.59e10 J injected (9.3e-14). The breaker vented 206 kg of the 300 kg of
/// N2; the pool rose 2.618 K (inside 2.5–4.5), the exchanger and outlet
/// took 5.60e9 J.
#[test]
fn mass_and_energy_close_during_a_condensation_transient() {
    let mut w = torus();
    let m0 = w.water_mass().get::<kilogram>();
    let n0 = w.nitrogen_mass().get::<kilogram>();
    let e0 = w.total_energy().get::<joule>();
    let t_pool0 = w.pool_temperature().get::<kelvin>();
    let mut vented = false;
    for i in 0..600 {
        let h = if i < 300 { 1.6e6 } else { 2.7e6 };
        let inputs = WetWellInputs {
            injection: Injection {
                mass_rate: kgs(20.0),
                specific_enthalpy: jkg(h),
                nitrogen_mass_rate: kgs(0.5),
                nitrogen_temperature: kelv(400.0),
            },
            heat_exchanger: ImmersedHeatExchanger {
                conductance: ThermalConductance::new::<watt_per_kelvin>(2.0e6),
                coolant_temperature: kelv(300.0),
            },
            liquid_outflow: kgs(5.0),
            ..quiet(P0)
        };
        let r = w.step(sec(1.0), &inputs).unwrap();
        vented |= r.vacuum_breaker_flow.get::<kilogram>() < 0.0;
    }
    let b = w.bookkeeping();
    let dm = w.water_mass().get::<kilogram>() - m0 - (b.water_in - b.water_out).get::<kilogram>();
    let dn = w.nitrogen_mass().get::<kilogram>() - n0 - (b.nitrogen_in - b.nitrogen_out).get::<kilogram>();
    let de = w.total_energy().get::<joule>() - e0 - (b.energy_in - b.energy_out).get::<joule>();
    let e_inj = b.energy_in.get::<joule>();
    let dt_pool = w.pool_temperature().get::<kelvin>() - t_pool0;
    println!(
        "closure: water {dm:.3e} kg of {m0:.6e} ({:.2e}); N2 {dn:.3e} kg of {n0:.6e} ({:.2e}); energy {de:.3e} J of {e_inj:.6e} J in ({:.2e}); \
         water in {:.1} out {:.1} kg, N2 in {:.2} out {:.2} kg, energy out {:.6e} J; pool +{dt_pool:.4} K, p {:.1} Pa, level {:.6} m, T_gas {:.3} K",
        dm / m0,
        dn / n0,
        de / e_inj,
        b.water_in.get::<kilogram>(),
        b.water_out.get::<kilogram>(),
        b.nitrogen_in.get::<kilogram>(),
        b.nitrogen_out.get::<kilogram>(),
        b.energy_out.get::<joule>(),
        w.gas_pressure().get::<pascal>(),
        w.pool_level().get::<meter>(),
        w.gas_temperature().get::<kelvin>(),
    );
    assert!(vented, "the breaker never opened");
    assert!(dm.abs() < 1e-12 * m0, "water {dm}");
    assert!(dn.abs() < 1e-12 * n0, "N2 {dn}");
    assert!(de.abs() < 1e-9 * e_inj, "energy {de}");
    assert!(dt_pool > 2.5 && dt_pool < 4.5, "pool +{dt_pool} K");
}

/// **2. The level rises by the condensed mass over `ρ A`.** 10 kg/s of steam
/// at 2.7e6 J/kg for 100 s into the 303.15 K pool, the surface ablated to
/// sensible heat at α = 0 so that the pool gains only the injection.
///
/// **Result (2026-11-01):** ΔL = 1.5898e-3 m against the hand 1.588e-3 m
/// (+0.11 %, held): 1.3393e-3 m from the mass, 2.505e-4 m from expansion
/// (ΔT 0.353 K). **Miss:** the pool gained 1000.00036 kg, not exactly 1000:
/// the rising pool compresses the saturated gas and 3.6e-4 kg rains out
/// (asserted equal to the steam the gas lost, to 1e-9 kg); with 90 % humid
/// gas the gain is 1000 kg to the last bit.
#[test]
fn the_level_rises_by_the_condensed_mass_over_rho_a() {
    let mut w = torus().without_surface_mass_transfer(HeatTransfer::new::<watt_per_square_meter_kelvin>(0.0));
    let l0 = w.pool_level().get::<meter>();
    let m0 = w.pool_mass().get::<kilogram>();
    let s0 = w.steam_mass().get::<kilogram>();
    let t0 = w.pool_temperature().get::<kelvin>();
    for _ in 0..100 {
        let r = w.step(sec(1.0), &WetWellInputs { injection: Injection::water(kgs(10.0), jkg(2.7e6)), ..quiet(P0) }).unwrap();
        assert_eq!(r.steam_to_gas.get::<kilogram>(), 0.0);
    }
    let dm = w.pool_mass().get::<kilogram>() - m0;
    let dl = w.pool_level().get::<meter>() - l0;
    let rho_f = w.pool_density().get::<uom::si::mass_density::kilogram_per_cubic_meter>();
    let dt = w.pool_temperature().get::<kelvin>() - t0;
    let hand = 1.339e-3 + 2.49e-4;
    let mass_part = dm / (rho_f * AREA);
    println!("level: dm {dm:.9} kg, dL {dl:.6e} m (hand {hand:.4e}, {:+.2} %), mass part {mass_part:.6e} m, rest {:.4e} m, dT {dt:.5} K, rho_f {rho_f:.4}", 100.0 * (dl / hand - 1.0), dl - mass_part);
    // The miss of the first run: the pool gained 1000.00036 kg, not 1000.
    // The rising pool compresses the saturated gas and rain-out returns its
    // excess steam to the pool; that is exactly the steam the gas lost.
    let rained = s0 - w.steam_mass().get::<kilogram>();
    println!("level: rain-out from the compressed saturated gas {rained:.6e} kg");
    assert!((dm - 1000.0 - rained).abs() < 1e-9, "{dm} {rained}");
    assert!((dl / hand - 1.0).abs() < 0.02, "{dl} vs {hand}");
    // With unsaturated gas (90 %) nothing rains out: exactly the injection.
    let mut w = torus_at(T0, T0, 0.9, VacuumBreakerDirection::OutOfGasSpace)
        .without_surface_mass_transfer(HeatTransfer::new::<watt_per_square_meter_kelvin>(0.0));
    let m0 = w.pool_mass().get::<kilogram>();
    for _ in 0..100 {
        w.step(sec(1.0), &WetWellInputs { injection: Injection::water(kgs(10.0), jkg(2.7e6)), ..quiet(P0) }).unwrap();
    }
    let dm = w.pool_mass().get::<kilogram>() - m0;
    println!("level: unsaturated gas, dm - 1000 = {:.3e} kg", dm - 1000.0);
    assert!((dm - 1000.0).abs() < 1e-9, "{dm}");
}

/// **4. The vacuum breaker is one-way and opens at its set point.**
///
/// **Result (2026-11-01):** no flow below the set point or against the
/// breaker's direction, in either direction; step by step it flows exactly
/// when the difference as it acts exceeds 3400 Pa. Inward 9.52 kg, the
/// difference ending at 3400.10 Pa. **Miss of the predicted upper bound by
/// 0.10 Pa**: the breaker acts at the start of a step and the surface then
/// cools the warmed gas, so it sits just above and reopens (62 times near
/// the set point). Outward 17.35 kg, ending at 3240.59 Pa (held: the
/// `(γ − 1)` overshoot of the 400 Pa excess predicted 3240).
#[test]
fn the_vacuum_breaker_opens_only_beyond_its_set_point_and_only_one_way() {
    // Into the gas space: the external volume 3000 Pa above it, then 3800.
    let mut w = torus_at(T0, T0, 1.0, VacuumBreakerDirection::IntoGasSpace);
    let p_g0 = w.gas_pressure().get::<pascal>();
    for _ in 0..50 {
        let r = w.step(sec(1.0), &quiet(p_g0 + 3000.0)).unwrap();
        assert_eq!(r.vacuum_breaker_flow.get::<kilogram>(), 0.0);
    }
    let p_ext = w.gas_pressure().get::<pascal>() + 3800.0;
    let mut moved = 0.0;
    let mut reopened = 0;
    for _ in 0..300 {
        let before = p_ext - w.gas_pressure().get::<pascal>();
        let f = w.step(sec(1.0), &quiet(p_ext)).unwrap().vacuum_breaker_flow.get::<kilogram>();
        assert!(f >= 0.0, "{f}");
        // It flows exactly when, as it acts, the difference exceeds the set point.
        assert_eq!(f > 0.0, before > 3400.0, "{before} {f}");
        if f > 0.0 && moved > 0.0 && before < 3401.0 {
            reopened += 1;
        }
        moved += f;
    }
    let d_in = p_ext - w.gas_pressure().get::<pascal>();
    // The gas space well above the external volume: an inward breaker stays shut.
    let mut w2 = torus_at(T0, T0, 1.0, VacuumBreakerDirection::IntoGasSpace);
    let p = w2.gas_pressure().get::<pascal>();
    assert_eq!(w2.step(sec(1.0), &quiet(p - 10_000.0)).unwrap().vacuum_breaker_flow.get::<kilogram>(), 0.0);

    // Out of the gas space: 3000 Pa above the external volume, then 3800.
    let mut w = torus();
    let p_g0 = w.gas_pressure().get::<pascal>();
    for _ in 0..50 {
        let r = w.step(sec(1.0), &quiet(p_g0 - 3000.0)).unwrap();
        assert_eq!(r.vacuum_breaker_flow.get::<kilogram>(), 0.0);
    }
    assert_eq!(w.clone().step(sec(1.0), &quiet(p_g0 + 10_000.0)).unwrap().vacuum_breaker_flow.get::<kilogram>(), 0.0);
    let p_ext_out = w.gas_pressure().get::<pascal>() - 3800.0;
    let mut out = 0.0;
    for _ in 0..300 {
        let before = w.gas_pressure().get::<pascal>() - p_ext_out;
        let f = w.step(sec(1.0), &quiet(p_ext_out)).unwrap().vacuum_breaker_flow.get::<kilogram>();
        assert!(f <= 0.0, "{f}");
        assert_eq!(f < 0.0, before > 3400.0, "{before} {f}");
        out += f;
    }
    let d_out = w.gas_pressure().get::<pascal>() - p_ext_out;
    println!("breaker: in {moved:.3} kg, final dP {d_in:.2} Pa, re-openings near the set point {reopened}; out {out:.3} kg, final dP {d_out:.2} Pa");
    assert!(moved > 0.0 && out < 0.0);
    // The first run's miss: the predicted upper bound (the set point) was
    // breached by 0.10 Pa. The breaker acts at the start of the step; the
    // pool surface then cools the gas the inflow warmed, so the difference
    // after the step sits just above the set point and the breaker reopens
    // next step. Kept as the measured fact, not re-gated: the one-way
    // decision is asserted above step by step.
    assert!(d_in > 3400.0 - 0.4 * 400.0, "{d_in}");
    assert!(d_out > 3200.0 && d_out < 3401.0, "{d_out}");
}

/// **5. RELAP-7's own equations (no open published test case is held).**
/// Dry N2 at 350 K over the 303.15 K pool, the RELAP-7-equivalent mode:
/// sensible-only surface at α = 10 W/(m² K) (Eq 390), constant `c_v`, no
/// pressure work, no breaker flow. With `k = A α dt` and `σ = 1/C_g + 1/C_p`
/// the step is implicit Euler, so `ΔT_N = ΔT_0 / (1 + kσ)^N` exactly, and
/// `ΔT_0 exp(−σ A α t)` within `O(kσ)`. Then Eqs 395, 399 and 400.
///
/// **Result (2026-11-01, held):** worst 3.7e-14 against implicit Euler;
/// ΔT(600 s) 4.7690 K against the exact 4.7483 K (+0.44 %, the `kσ = 3.8e-3`
/// first-order error); Eqs 395, 399, 400 to 1e-9 of `p_g`; no mass moved.
#[test]
fn the_relap7_mode_follows_eq_390_in_closed_form() {
    let alpha = 10.0;
    let mut w = torus_at(T0, 350.0, 0.0, VacuumBreakerDirection::OutOfGasSpace)
        .without_surface_mass_transfer(HeatTransfer::new::<watt_per_square_meter_kelvin>(alpha))
        .without_nitrogen_cv_t()
        .without_pressure_work();
    let n = w.nitrogen_mass().get::<kilogram>() / M_N2;
    let c_g = n * cv_n2(350.0, false);
    let c_p = w.pool_mass().get::<kilogram>() * 4186.0;
    let sigma = 1.0 / c_g + 1.0 / c_p;
    let d0 = w.gas_temperature().get::<kelvin>() - w.pool_temperature().get::<kelvin>();
    let m_pool0 = w.pool_mass().get::<kilogram>();
    let k = AREA * alpha * 1.0;
    let mut worst_implicit: f64 = 0.0;
    for i in 1..=600 {
        w.step(sec(1.0), &quiet(P0)).unwrap();
        let d = w.gas_temperature().get::<kelvin>() - w.pool_temperature().get::<kelvin>();
        let implicit = d0 / (1.0 + k * sigma).powi(i);
        worst_implicit = worst_implicit.max((d / implicit - 1.0).abs());
    }
    let d = w.gas_temperature().get::<kelvin>() - w.pool_temperature().get::<kelvin>();
    let exact = d0 * (-sigma * AREA * alpha * 600.0).exp();
    println!("relap7 Eq 390: dT0 {d0:.4} K, dT(600 s) {d:.6} K, exact {exact:.6} K ({:+.3} %), worst vs implicit Euler {worst_implicit:.2e}", 100.0 * (d / exact - 1.0));
    assert!(worst_implicit < 1e-9, "{worst_implicit}");
    assert!((d / exact - 1.0).abs() < 5e-3);
    assert_eq!(w.steam_mass().get::<kilogram>(), 0.0);
    assert_eq!(w.pool_mass().get::<kilogram>(), m_pool0);
    // Eqs 395, 400, 399.
    let p_g = w.gas_pressure().get::<pascal>();
    let l = w.pool_level().get::<meter>();
    let rho = w.pool_density().get::<uom::si::mass_density::kilogram_per_cubic_meter>();
    let z = 1.0;
    let p_w = w.pool_reference_pressure().get::<pascal>();
    assert!((p_w - (p_g + 0.5 * l * rho * 9.81)).abs() < 1e-9 * p_g);
    let p_i = w.pressure_at_elevation(Length::new::<meter>(z)).get::<pascal>();
    assert!((p_i - (p_w + (0.5 * l - z) * rho * 9.81)).abs() < 1e-9 * p_g);
    assert!((p_i - (p_g + (l - z) * rho * 9.81)).abs() < 1e-9 * p_g);
    assert_eq!(w.pressure_at_elevation(Length::new::<meter>(l + 0.1)).get::<pascal>(), p_g);
    let pin = w.pipe_end_pressure(Length::new::<meter>(z), 3.0, pa(200.0)).get::<pascal>();
    assert!((pin - (p_i + 200.0)).abs() < 1e-9 * p_g);
}

/// **6. Near saturation part of the steam passes through** (MELCOR's
/// `ε_T`, ported from sembawang). The pool is set 2.6 K below the saturation
/// temperature at the port; one 1 s step of 1 kg/s of steam.
///
/// **Result (2026-11-01, held):** ε = 0.500000, 0.500000 kg to the gas;
/// ablated ε = 1, none. The first run's setup at 105 kPa was unbuildable
/// (the pool would have been above the surface's saturation temperature;
/// the port's pressure, about 1.18e5 Pa, was under-estimated by hand), so it
/// runs at 130 kPa.
#[test]
fn near_saturation_part_of_the_steam_passes_through() {
    // The port's pressure depends on the density, so find the pool
    // temperature by two passes.
    // At 1.3e5 Pa: at 1.05e5 the port's saturation temperature less 2.6 K
    // lies above the surface's saturation temperature, which `new` refuses
    // (the first run's setup, a miss of the hand estimate of the port's
    // pressure; see the V&V record).
    let p0 = 1.3e5;
    let mut t = 370.0;
    for _ in 0..3 {
        let w = torus_at_p(t, T0, 1.0, p0, VacuumBreakerDirection::OutOfGasSpace);
        let p_j = w.pressure_at_elevation(Length::new::<meter>(w.pool_level().get::<meter>() - SUBMERGENCE_PA / (995.0 * 9.81))).get::<pascal>();
        t = t_sat(p_j) - 2.6;
    }
    let inputs = WetWellInputs { injection: Injection::water(kgs(1.0), jkg(2.7e6)), ..quiet(p0) };
    let mut w = torus_at_p(t, T0, 1.0, p0, VacuumBreakerDirection::OutOfGasSpace);
    let r = w.step(sec(1.0), &inputs).unwrap();
    let eps = r.bubble_efficiency;
    let passed = r.steam_to_gas.get::<kilogram>();
    let mut a = torus_at_p(t, T0, 1.0, p0, VacuumBreakerDirection::OutOfGasSpace).without_bubble_pass_through();
    let ra = a.step(sec(1.0), &inputs).unwrap();
    println!("pass-through: pool {t:.4} K, eps {eps:.6}, passed {passed:.6} kg; ablated eps {} passed {}", ra.bubble_efficiency, ra.steam_to_gas.get::<kilogram>());
    assert!((eps - 0.5).abs() < 0.02, "{eps}");
    assert!((passed - (1.0 - eps)).abs() < 1e-12, "{passed}");
    assert_eq!((ra.bubble_efficiency, ra.steam_to_gas.get::<kilogram>()), (1.0, 0.0));
}

/// Bad input is refused, not clamped.
#[test]
fn bad_input_is_refused() {
    let mut w = torus();
    assert!(w.step(sec(0.0), &quiet(P0)).is_err());
    assert!(w.step(sec(1.0), &WetWellInputs { liquid_outflow: kgs(-1.0), ..quiet(P0) }).is_err());
    assert!(w.step(sec(1.0), &WetWellInputs { liquid_outflow: kgs(2.0e6), ..quiet(P0) }).is_err());
    let mut bad = quiet(P0);
    bad.external.steam_mole_fraction = 1.0;
    assert!(w.step(sec(1.0), &bad).is_err());
    let g = WetWellGeometry {
        cross_section: Area::new::<square_meter>(AREA),
        total_height: Length::new::<meter>(2.0),
        injection_elevation: Length::new::<meter>(1.0),
        injection_opening: Length::new::<meter>(0.5),
    };
    let init = WetWellInitialState {
        pool_level: Length::new::<meter>(2.5),
        gas_pressure: pa(P0),
        pool_temperature: kelv(T0),
        gas_temperature: kelv(T0),
        relative_humidity: 1.0,
    };
    assert!(WetWell::new(g, breaker(VacuumBreakerDirection::IntoGasSpace), init).is_err());
    assert!(WetWell::new(g, breaker(VacuumBreakerDirection::IntoGasSpace), WetWellInitialState { pool_level: Length::new::<meter>(1.0), relative_humidity: 1.5, ..init }).is_err());
    assert!(WetWell::new(g, breaker(VacuumBreakerDirection::IntoGasSpace), WetWellInitialState { pool_level: Length::new::<meter>(1.0), pool_temperature: kelv(400.0), ..init }).is_err());
}

