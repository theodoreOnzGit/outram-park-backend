//! # V&V: two-region non-equilibrium control volume (gh:#1092, MELCOR CVH §5)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! Methodology and results are also in
//! `crates/tampines/verification_and_validation/control_volume.md`.
//!
//! **Predictions, written 2026-11-01 before the first run:**
//!
//! 1. *Closure* (a two-piece vessel, areas 10 and 20 m², 300 s at 1 s:
//!    0.5 kg/s of steam at 2.75e6 J/kg below the pool, 0.002 kg/s H2 plus
//!    0.01 kg/s N2 at 400 K through a second submerged port, a 0.3 kg/s
//!    drain at the bottom, a 0.01 kg/s vent at the top, every mechanism on):
//!    water and each non-condensable close to < 1e-12 of the inventory;
//!    energy `E − E0 − (E_in − E_out)` to < 1e-10 of `E_in`. The pool heats
//!    4–8 K (hand: 150 kg condensing gives 3.9e8 J over `m c` ≈ 6.3e7 J/K,
//!    less what the drain takes); the pressure rises 5–15 % (about 400 mol
//!    of gas into 3400, the vent taking about 100).
//! 2. *Wet well as a special case* (the Unit 1 torus of `tests/wet_well.rs`,
//!    N2 only, its breaker never opening, 600 s of 10 kg/s relief steam with
//!    0.5 kg/s N2, the heat exchanger and a 5 kg/s liquid outlet; the control
//!    volume with the wet well's models: constant-`c_p` liquid, ideal steam,
//!    the four-pass rain-out): pool and gas temperatures, pressure, pool and
//!    steam masses and level agree to < 1e-9 relative. Not bit for bit: the
//!    pressure here is Dalton's sum, `n_N2 R T/V + (m_s/V)(R/M) T`, against
//!    the wet well's `(n_N2 + m_s/M) R T / V`; the gas temperature is solved
//!    from a stored energy, not stored; the area comes from the table.
//! 3. *Swollen level* (`crates/sembawang/tests/swollen_level_vs_tampines.rs`):
//!    bit-identical to `sembawang::bwr_mark1::swollen_level`. Here: on a
//!    prismatic vessel the control volume's swollen level equals the hand
//!    value `z_b + (L − z_b)/(1 − α)`, `α = Q_v/(2 v0 A)`, to 1e-12, and a
//!    port between the collapsed and the swollen level draws liquid with the
//!    swell, atmosphere without it.
//! 4. *Non-prismatic level*: table (0, 0), (1, 2), (3, 12), (4, 20) m / m³;
//!    7 m³ of pool sits at `1 + (7 − 2)/10 · 2 = 2.000 m` by hand; the
//!    control volume's level equals the hand interpolation of its own pool
//!    volume `m/ρ` to 1e-12 m, and 2.000 m within 1e-3 m (the density at the
//!    actual pressure against the one used to set the mass).
//! 5. *Rain-out*: an atmosphere at 350 K holding 1.5 times its saturated
//!    steam ends one step at relative humidity 1 within 1e-3 (the pool's
//!    compression work, after the rain-out, warms the gas slightly), mass to
//!    1e-12 and energy to 1e-10 of the gas energy; `sembawang`'s four passes
//!    (the ablation) end subsaturated (RH < 0.99).

use tampines::components::wet_well::*;
use tampines::control_volume::*;
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::v_tp_1;
use uom::si::amount_of_substance::mole;
use uom::si::area::square_meter;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermal_conductance::watt_per_kelvin;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

fn m(x: f64) -> Length {
    Length::new::<meter>(x)
}
fn m3(x: f64) -> Volume {
    Volume::new::<cubic_meter>(x)
}
fn kg(x: f64) -> Mass {
    Mass::new::<kilogram>(x)
}
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
fn port(z: f64, h: f64) -> Port {
    Port { elevation: m(z), opening_height: m(h) }
}
fn rho_if97(t: f64, p: f64) -> f64 {
    1.0 / v_tp_1(kelv(t), pa(p)).get::<cubic_meter_per_kilogram>()
}
fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs().max(1e-300)
}

/// An N2 atmosphere with steam at relative humidity `rh`, filling `v` at
/// `p` and `t` (ideal-gas moles; the control volume's own closure sets the
/// actual pressure).
fn n2_atmosphere(v: f64, p: f64, t: f64, rh: f64) -> GasMixture {
    let p_s = rh * p_sat_pa(t);
    let n = p * v / (R_GAS * t);
    GasMixture::new(kg(n * p_s / p * M_H2O), &[(NonCondensable::N2, AmountOfSubstance::new::<mole>(n * (1.0 - p_s / p)))])
}

/// Prediction 1: mass, energy and per-species closure over a transient.
///
/// **Result (2026-11-01, release):** water residual 2.1e-10 kg of
/// 1.49e4 kg (1.4e-14), N2 −1.8e-12 kg of 93.8 kg (1.9e-14), H2 1.1e-16 kg
/// of 0.59 kg, the absent gases exactly 0; energy −3.0e-7 J against `E_in`
/// = 4.17e8 J (−7.1e-16). The pool heated 6.31 K and the pressure rose
/// 8.64 % (RH ends 0.73). As predicted.
#[test]
fn closure_over_a_transient_with_injection_gas_drain_and_vent() {
    let table = VolumeAltitudeTable::new(&[(m(0.0), m3(0.0)), (m(2.0), m3(20.0)), (m(6.0), m3(100.0))]).unwrap();
    let ports = vec![port(0.3, 0.1), port(0.5, 0.05), port(0.0, 0.1), port(5.8, 0.1)];
    let pool = 15.0 * rho_if97(300.0, 1.0e5);
    let mut cv = ControlVolume::new(table, ports, kg(pool), kelv(300.0), n2_atmosphere(85.0, 1.0e5, 300.0, 0.5), kelv(300.0)).unwrap();
    let w0 = cv.water_mass().get::<kilogram>();
    let n0: Vec<f64> = cv.ncg_masses().iter().map(|x| x.get::<kilogram>()).collect();
    let e0 = cv.total_energy().get::<joule>();
    let (t_pool0, p0) = (cv.pool_temperature().get::<kelvin>(), cv.atmosphere_pressure().get::<pascal>());
    let mut inputs = ControlVolumeInputs::closed(4);
    inputs.ports[0] = PortInput::inflow(Source::water(kgs(0.5), jkg(2.75e6)));
    inputs.ports[1] = PortInput::inflow(Source::gas(&[(NonCondensable::H2, kgs(0.002)), (NonCondensable::N2, kgs(0.01))], kelv(400.0)));
    inputs.ports[2] = PortInput::outflow(kgs(0.3));
    inputs.ports[3] = PortInput::outflow(kgs(0.01));
    for _ in 0..300 {
        let r = cv.step(sec(1.0), &inputs).unwrap();
        assert_eq!(r.ports[2].draw, PortDraw::Liquid);
        assert_eq!(r.ports[3].draw, PortDraw::Atmosphere);
    }
    let b = cv.bookkeeping();
    let dw = cv.water_mass().get::<kilogram>() - w0 - (b.water_in - b.water_out).get::<kilogram>();
    println!("water residual {dw:e} kg of {w0:e}");
    assert!(dw.abs() < 1e-12 * w0);
    for g in NonCondensable::ALL {
        let i = g.index();
        let now = cv.ncg_masses()[i].get::<kilogram>();
        let d = now - n0[i] - (b.ncg_in[i] - b.ncg_out[i]).get::<kilogram>();
        println!("{g:?}: {now:e} kg, residual {d:e}");
        assert!(d.abs() <= 1e-12 * now.max(1e-30), "{g:?}");
    }
    let e_in = b.energy_in.get::<joule>();
    let de = cv.total_energy().get::<joule>() - e0 - (e_in - b.energy_out.get::<joule>());
    println!("energy residual {de:e} J of E_in {e_in:e} ({:e})", de / e_in);
    assert!(de.abs() < 1e-10 * e_in);
    let dt = cv.pool_temperature().get::<kelvin>() - t_pool0;
    let dp = cv.atmosphere_pressure().get::<pascal>() / p0 - 1.0;
    println!("pool +{dt} K, pressure {:+.2} %, RH {}", 100.0 * dp, cv.relative_humidity());
    assert!((4.0..8.0).contains(&dt), "pool heated {dt} K");
    assert!((0.05..0.15).contains(&dp), "pressure rose {dp}");
}

const AREA: f64 = 750.0;
const GAS_M3: f64 = 2620.0;
const POOL_KG: f64 = 1750.0 * 995.0;
const P0: f64 = 1.05e5;
const T0: f64 = 303.15;

/// The Unit 1 torus of `tests/wet_well.rs`, its breaker never opening.
fn torus(t_gas: f64) -> (WetWell, f64, f64, f64) {
    let level = POOL_KG / (rho_if97(T0, P0) * AREA);
    let submergence = 11_800.0 / (995.0 * 9.81);
    let opening = (4.0 * 20.0 / (96.0 * std::f64::consts::PI)).sqrt();
    let lt = level + GAS_M3 / AREA;
    let ww = WetWell::new(
        WetWellGeometry {
            cross_section: Area::new::<square_meter>(AREA),
            total_height: m(lt),
            injection_elevation: m(level - submergence),
            injection_opening: m(opening),
        },
        VacuumBreaker {
            direction: VacuumBreakerDirection::OutOfGasSpace,
            opening_pressure_difference: pa(1.0e9),
            flow_area: Area::new::<square_meter>(0.5),
            discharge_coefficient: 0.6,
        },
        WetWellInitialState { pool_level: m(level), gas_pressure: pa(P0), pool_temperature: kelv(T0), gas_temperature: kelv(t_gas), relative_humidity: 1.0 },
    )
    .unwrap();
    (ww, lt, level - submergence, opening)
}

/// Prediction 2: the wet well (gh:#1075) as a special case.
///
/// **Result (2026-11-01, release): prediction 2 MISSED, reason found.** The
/// first 5 steps agree to 6.5e-15 relative (round-off, as predicted), but
/// the difference then grows by roughly 2.5x per step from step 7 and
/// saturates over the 600 s at: pool temperature 9.2e-11, gas temperature
/// 2.7e-5, pressure 2.3e-5, pool mass 4.9e-9, steam mass 1.05e-4, level
/// 3.8e-9 (relative). The cause is the wet well's own dynamics, not the
/// control volume: the same wet well started with its gas 1e-12 K warmer
/// (3e-15 relative) differs from the original by 9.5e-11, 2.9e-5, 2.5e-5,
/// 5.3e-9, 1.14e-4, 4.1e-9, i.e. by as much. The gas sits at saturation
/// (RH 1 from the start, steam passing through the pool), where the
/// explicit surface step and rain-out at 1 s amplify any round-off; the
/// growth starts when the gas first reaches saturation. The assertion is
/// therefore the explanation, not the original criterion: round-off for
/// the first 5 steps, and no more than 10x the wet well's own sensitivity.
/// The amplification itself is a property of the wet well's explicit
/// step worth a follow-up (it is not a divergence: both runs stay
/// physical and end at the same state to 1e-4).
#[test]
fn the_wet_well_is_a_special_case() {
    let (mut ww, lt, z_inj, opening) = torus(T0);
    // The same wet well with its gas 1e-12 K warmer (3e-15 relative, a few
    // ulps): how far the wet well itself carries a round-off difference.
    let (mut ww2, ..) = torus(T0 + 1.0e-12);
    let gas = GasMixture::new(ww.steam_mass(), &[(NonCondensable::N2, AmountOfSubstance::new::<mole>(ww.nitrogen_mass().get::<kilogram>() / M_N2))]);
    let mut cv = ControlVolume::new(
        VolumeAltitudeTable::prismatic(Area::new::<square_meter>(AREA), m(lt)).unwrap(),
        vec![port(z_inj, opening), port(0.0, 0.1)],
        ww.pool_mass(),
        ww.pool_temperature(),
        gas,
        ww.gas_temperature(),
    )
    .unwrap()
    .without_real_liquid()
    .unwrap()
    .without_real_steam()
    .unwrap()
    .without_equilibrium_rain_out();
    let hx = ImmersedHeatExchanger { conductance: ThermalConductance::new::<watt_per_kelvin>(1.0e6), coolant_temperature: kelv(300.0) };
    let ww_in = WetWellInputs {
        injection: Injection { mass_rate: kgs(10.0), specific_enthalpy: jkg(2.7e6), nitrogen_mass_rate: kgs(0.5), nitrogen_temperature: kelv(350.0) },
        external: ExternalVolume { pressure: pa(1.0e5), temperature: kelv(300.0), steam_mole_fraction: 0.0 },
        heat_exchanger: hx,
        liquid_outflow: kgs(5.0),
    };
    let mut cv_in = ControlVolumeInputs::closed(2);
    cv_in.ports[0] = PortInput::inflow(Source::water(kgs(10.0), jkg(2.7e6)).with_gas(&[(NonCondensable::N2, kgs(0.5))], kelv(350.0)));
    cv_in.ports[1] = PortInput::outflow(kgs(5.0));
    cv_in.heat_exchanger = hx;
    let mut worst = [0.0_f64; 6];
    let mut worst_self = [0.0_f64; 6];
    let mut worst_early = 0.0_f64;
    let snap_ww = |w: &WetWell| {
        [
            w.pool_temperature().get::<kelvin>(),
            w.gas_temperature().get::<kelvin>(),
            w.gas_pressure().get::<pascal>(),
            w.pool_mass().get::<kilogram>(),
            w.steam_mass().get::<kilogram>(),
            w.pool_level().get::<meter>(),
        ]
    };
    for k in 0..600 {
        ww.step(sec(1.0), &ww_in).unwrap();
        ww2.step(sec(1.0), &ww_in).unwrap();
        cv.step(sec(1.0), &cv_in).unwrap();
        let a = snap_ww(&ww);
        let b = snap_ww(&ww2);
        let c = [
            cv.pool_temperature().get::<kelvin>(),
            cv.atmosphere_temperature().get::<kelvin>(),
            cv.atmosphere_pressure().get::<pascal>(),
            cv.pool_mass().get::<kilogram>(),
            cv.atmosphere().steam_kg,
            cv.pool_level().get::<meter>(),
        ];
        for i in 0..6 {
            worst[i] = worst[i].max(rel(c[i], a[i]));
            worst_self[i] = worst_self[i].max(rel(b[i], a[i]));
            if k < 5 {
                worst_early = worst_early.max(rel(c[i], a[i]));
            }
        }
    }
    println!("worst relative differences [T_pool, T_gas, p, m_pool, m_steam, level]");
    println!("  control volume vs wet well: {worst:?}");
    println!("  wet well vs itself, gas 1e-12 K warmer: {worst_self:?}");
    println!("  first 5 steps: {worst_early:e}");
    println!(
        "end: T_pool {} K, T_gas {} K, p {} Pa, steam {} kg",
        ww.pool_temperature().get::<kelvin>(),
        ww.gas_temperature().get::<kelvin>(),
        ww.gas_pressure().get::<pascal>(),
        ww.steam_mass().get::<kilogram>()
    );
    // MISS of prediction 2 (< 1e-9 over 600 s): see the doc comment. What is
    // asserted is the explanation: round-off agreement before the wet well's
    // own amplification sets in, and no more difference than the wet well
    // shows against itself after a 3e-15 perturbation (factor 10 headroom).
    assert!(worst_early < 1e-13, "{worst_early:e}");
    for i in 0..6 {
        assert!(worst[i] < 10.0 * worst_self[i].max(1e-13), "{i}: {worst:?} vs {worst_self:?}");
    }
}

/// Prediction 3 (here): the swollen level on a prismatic vessel by hand, and
/// the port draw it decides.
///
/// **Result (2026-11-01, release):** 0.2 kg/s of N2 through a port 1.8 m
/// deep raises 0.1845 m³/s of gas, `α` = 0.0308; collapsed level
/// 1.99999901 m, swollen 2.06346501 m, the hand value to 1e-15 m. A port
/// at 2.02 m draws liquid with the swell and atmosphere without it. (The draft used 0.05 kg/s; a hand estimate, before the first run,
/// gave a 1.6 cm swell that would not reach the port, so it was raised.)
#[test]
fn swollen_level_by_hand_and_the_draw_it_decides() {
    let a = 10.0;
    let table = VolumeAltitudeTable::prismatic(Area::new::<square_meter>(a), m(5.0)).unwrap();
    let pool = 20.0 * rho_if97(300.0, 1.0e5);
    // A port 2 cm above the collapsed level (about 2 m).
    let build = |swell: bool| {
        let cv = ControlVolume::new(table.clone(), vec![port(0.2, 0.1), port(2.02, 0.05)], kg(pool), kelv(300.0), n2_atmosphere(30.0, 1.0e5, 300.0, 0.5), kelv(300.0)).unwrap();
        if swell { cv } else { cv.without_level_swell() }
    };
    for swell in [true, false] {
        let mut cv = build(swell);
        let z_c0 = cv.pool_level().get::<meter>();
        assert!(z_c0 < 2.02 && z_c0 > 1.99, "{z_c0}");
        let mut inputs = ControlVolumeInputs::closed(2);
        inputs.ports[0] = PortInput::inflow(Source::gas(&[(NonCondensable::N2, kgs(0.2))], kelv(300.0)));
        let r = cv.step(sec(1.0), &inputs).unwrap();
        let q_v = r.vapour_flow_m3_s;
        let z_c = cv.pool_level().get::<meter>();
        let alpha = (q_v / a / (2.0 * BUBBLE_RISE_M_S)).min(MAX_POOL_VOID);
        let hand = z_c / (1.0 - alpha);
        let z_s = cv.swollen_level().get::<meter>();
        println!("swell {swell}: Q_v {q_v:e} m3/s, alpha {alpha:e}, collapsed {z_c} m, swollen {z_s} m (hand {hand} m)");
        assert!((z_s - hand).abs() < 1e-12, "{z_s} vs {hand}");
        assert!(z_s > 2.02);
        let donor = cv.port_donor(1).unwrap();
        let expect = if swell { PortDraw::Liquid } else { PortDraw::Atmosphere };
        assert_eq!(donor.draw, expect);
        // And the next step's outflow draws the same.
        let mut out = ControlVolumeInputs::closed(2);
        out.ports[1] = PortInput::outflow(kgs(0.001));
        assert_eq!(cv.step(sec(1.0), &out).unwrap().ports[1].draw, expect);
    }
}

/// Prediction 4: a non-prismatic vessel's level from the table.
///
/// **Result (2026-11-01, release):** the table's own values exact
/// (`V(2) = 7`, `z(7) = 2`, areas 2, 5, 8); pool 7.0000000047 m³, level
/// 2.0000000009 m, equal to the hand interpolation of that volume to the
/// last bit and to 2.000 m within 1e-9 m. As predicted.
#[test]
fn non_prismatic_level_against_the_hand_table() {
    let table = VolumeAltitudeTable::new(&[(m(0.0), m3(0.0)), (m(1.0), m3(2.0)), (m(3.0), m3(12.0)), (m(4.0), m3(20.0))]).unwrap();
    assert_eq!(table.volume_below(m(2.0)).get::<cubic_meter>(), 7.0);
    assert_eq!(table.height_of_volume(m3(7.0)).get::<meter>(), 2.0);
    assert_eq!(table.area_at(m(0.5)).get::<square_meter>(), 2.0);
    assert_eq!(table.area_at(m(2.0)).get::<square_meter>(), 5.0);
    assert_eq!(table.area_at(m(3.5)).get::<square_meter>(), 8.0);
    let rho0 = rho_if97(300.0, 1.0e5);
    let cv = ControlVolume::new(table, vec![], kg(7.0 * rho0), kelv(300.0), n2_atmosphere(13.0, 1.0e5, 300.0, 0.5), kelv(300.0)).unwrap();
    let v_pool = cv.pool_mass().get::<kilogram>() / cv.pool_density().get::<kilogram_per_cubic_meter>();
    let hand = 1.0 + (v_pool - 2.0) / 10.0 * 2.0;
    let level = cv.pool_level().get::<meter>();
    println!("pool {v_pool} m3, level {level} m, hand {hand} m, gas {} m3", cv.atmosphere_volume().get::<cubic_meter>());
    assert!((level - hand).abs() < 1e-12);
    assert!((level - 2.0).abs() < 1e-3);
    assert!((cv.atmosphere_volume().get::<cubic_meter>() - (20.0 - v_pool)).abs() < 1e-12);
}

/// Prediction 5: rain-out of a supersaturated atmosphere.
///
/// **Result (2026-11-01, release):** RH 1.493 → 0.99997 after one step
/// (0.252 kg rained out, 0.0044 kg condensed on the pool surface, the gas
/// warmed 350 → 360.4 K by the latent heat it keeps); water and energy
/// changed by exactly 0 (no boundary flow). `sembawang`'s four passes
/// condense the whole excess at 350 K (5.21 kg) in the first pass and leave
/// its latent heat in 40 m³ of gas, which reaches 601 K at RH 0.006: the
/// ablation is far from saturation for a strong supersaturation, which is
/// why equilibrium rain-out is the default. As predicted.
#[test]
fn rain_out_ends_saturated_and_conserves() {
    for equilibrium in [true, false] {
        let table = VolumeAltitudeTable::prismatic(Area::new::<square_meter>(10.0), m(5.0)).unwrap();
        let pool = 10.0 * rho_if97(340.0, 1.0e5);
        let v_gas = 40.0;
        let t = 350.0;
        let mut gas = n2_atmosphere(v_gas, 1.0e5, t, 0.0);
        // 1.5 times the steam that saturates 40 m3 at 350 K (IF97 saturated vapour).
        let rho_g = 1.0 / tampines_steam_tables::region_2_vapour::v_tp_2(kelv(t), pa(p_sat_pa(t))).get::<cubic_meter_per_kilogram>();
        gas.steam_kg = 1.5 * rho_g * v_gas;
        let cv = ControlVolume::new(table, vec![], kg(pool), kelv(340.0), gas, kelv(t)).unwrap();
        let mut cv = if equilibrium { cv } else { cv.without_equilibrium_rain_out() };
        let rh0 = cv.relative_humidity();
        let (w0, e0) = (cv.water_mass().get::<kilogram>(), cv.total_energy().get::<joule>());
        let gas_e = 1.0e7; // scale: the atmosphere's energy is of order 1e7 J
        let r = cv.step(sec(1.0), &ControlVolumeInputs::closed(0)).unwrap();
        let rh = cv.relative_humidity();
        let dw = cv.water_mass().get::<kilogram>() - w0;
        let de = cv.total_energy().get::<joule>() - e0;
        println!(
            "equilibrium {equilibrium}: RH {rh0} -> {rh}, rained out {} kg, surface {} kg, T_gas {} K, dW {dw:e} kg, dE {de:e} J",
            r.rain_out.get::<kilogram>(),
            r.surface_evaporation.get::<kilogram>(),
            cv.atmosphere_temperature().get::<kelvin>()
        );
        assert!(rh0 > 1.4);
        assert!(dw.abs() < 1e-12 * w0);
        assert!(de.abs() < 1e-10 * gas_e);
        if equilibrium {
            assert!((rh - 1.0).abs() < 1e-3, "RH {rh}");
        } else {
            assert!(rh < 0.99, "RH {rh}");
        }
    }
}

/// The defaults are the physics: every mechanism on, IF97 on both sides
/// (pins the CLAUDE.md "correct physics is the default" rule).
#[test]
fn correct_physics_is_the_default() {
    let table = VolumeAltitudeTable::prismatic(Area::new::<square_meter>(1.0), m(2.0)).unwrap();
    let cv = ControlVolume::new(table, vec![port(0.0, 0.1)], kg(900.0), kelv(300.0), n2_atmosphere(1.0, 1.0e5, 300.0, 0.5), kelv(300.0)).unwrap();
    assert_eq!(cv.mechanisms(), ControlVolumeMechanisms::default());
    let mech = cv.mechanisms();
    assert!(mech.bubble_pass_through && mech.pressure_work && mech.level_swell && mech.equilibrium_rain_out);
    assert_eq!(mech.liquid, LiquidModel::Iapws97);
    assert_eq!(mech.surface, SurfaceModel::Melcor);
    assert_eq!(cv.gas_model(), GasMixtureModel::new());
    let ab = cv
        .clone()
        .without_bubble_pass_through()
        .without_pressure_work()
        .without_ncg_cv_t()
        .unwrap()
        .without_surface_mass_transfer(uom::si::f64::HeatTransfer::new::<uom::si::heat_transfer::watt_per_square_meter_kelvin>(5.0));
    assert!(!ab.mechanisms().bubble_pass_through && !ab.mechanisms().pressure_work);
    assert_ne!(ab.gas_model(), GasMixtureModel::new());
    // A liquid donor carries IF97's enthalpy at the port's pressure.
    let d = cv.port_donor(0).unwrap();
    assert_eq!(d.draw, PortDraw::Liquid);
    assert!(d.pressure.get::<pascal>() > cv.atmosphere_pressure().get::<pascal>());
    assert!((d.specific_enthalpy.get::<joule_per_kilogram>() - 112_700.0).abs() < 300.0, "{:?}", d.specific_enthalpy);
    assert!(cv.port_donor(1).is_err());
    // The sensible-only ablation steps and conserves.
    let mut ab = ab;
    let e0 = ab.total_energy().get::<joule>();
    ab.step(sec(1.0), &ControlVolumeInputs::closed(1)).unwrap();
    assert!((ab.total_energy().get::<joule>() - e0).abs() < 1e-6);
}

/// **S1: species sources and sinks on the atmosphere conserve** (gh:#1103).
///
/// Prediction (V&V doc, written before the run): water, every gas and the
/// energy close against the bookkeeping to < 1e-12; each step's energy
/// equals `Σ (m/M) h` at the start-of-step enthalpies to 1e-12; an
/// oversized sink is refused.
///
/// **Result (2026-11-01, release):** see the V&V doc.
#[test]
fn s1_species_sources_and_sinks_conserve() {
    let table = VolumeAltitudeTable::prismatic(Area::new::<square_meter>(2.0), m(5.0)).unwrap();
    let pool = 2.0 * rho_if97(330.0, 1.0e5);
    let n = 1.0e5 * 8.0 / (R_GAS * 330.0);
    let gas = GasMixture::new(
        kg(0.5 * n * 0.1 * 0.018_015),
        &[
            (NonCondensable::N2, AmountOfSubstance::new::<mole>(0.75 * n)),
            (NonCondensable::H2, AmountOfSubstance::new::<mole>(0.1 * n)),
            (NonCondensable::O2, AmountOfSubstance::new::<mole>(0.1 * n)),
        ],
    );
    let mut cv = ControlVolume::new(table, vec![port(0.2, 0.1)], kg(pool), kelv(330.0), gas, kelv(330.0)).unwrap();
    let mass = |cv: &ControlVolume| (cv.water_mass().get::<kilogram>(), cv.ncg_masses().map(|x| x.get::<kilogram>()));
    let (w0, g0) = mass(&cv);
    let e0 = cv.total_energy().get::<joule>();
    let mut rates = AtmosphereSpeciesRates::none();
    rates.steam = kgs(0.01);
    rates.ncg[NonCondensable::CO2.index()] = kgs(0.02);
    rates.ncg[NonCondensable::H2.index()] = kgs(-0.0002);
    rates.ncg[NonCondensable::O2.index()] = kgs(-0.0016);
    let mut inputs = ControlVolumeInputs::closed(1);
    inputs.atmosphere_species = rates;
    let mut worst_e_step: f64 = 0.0;
    for _ in 0..100 {
        let h = cv.atmosphere_species_molar_enthalpies().map(|x| x.value);
        let r = cv.step(sec(1.0), &inputs).unwrap();
        let mut e = 0.01 / 0.018_015 * h[0];
        for g in NonCondensable::ALL {
            e += rates.ncg[g.index()].get::<kilogram_per_second>() / g.molar_mass() * h[1 + g.index()];
        }
        worst_e_step = worst_e_step.max(rel(r.species_energy.get::<joule>(), e));
        assert_eq!(r.species_water.get::<kilogram>(), 0.01);
    }
    let b = cv.bookkeeping();
    let (w, g) = mass(&cv);
    let dw = rel(w - w0, b.water_in.get::<kilogram>() - b.water_out.get::<kilogram>());
    let mut dg: f64 = 0.0;
    for q in 0..6 {
        let want = b.ncg_in[q].get::<kilogram>() - b.ncg_out[q].get::<kilogram>();
        dg = dg.max(((g[q] - g0[q]) - want).abs() / g0.iter().sum::<f64>());
    }
    let de = ((cv.total_energy().get::<joule>() - e0) - (b.energy_in.get::<joule>() - b.energy_out.get::<joule>())).abs() / e0.abs();
    println!("S1: water {dw:e}, gas {dg:e}, energy {de:e} of E0; per-step energy against the start-of-step enthalpies {worst_e_step:e}; p {} Pa, T {} K", cv.atmosphere_pressure().value, cv.atmosphere_temperature().value);
    assert!(dw < 1e-12 && dg < 1e-12 && de < 1e-12, "{dw:e} {dg:e} {de:e}");
    assert!(worst_e_step < 1e-12, "{worst_e_step:e}");
    // An oversized sink is refused.
    let mut big = ControlVolumeInputs::closed(1);
    big.atmosphere_species.ncg[NonCondensable::H2.index()] = kgs(-1.0e3);
    assert!(cv.step(sec(1.0), &big).is_err());
}
