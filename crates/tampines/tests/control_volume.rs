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
    // gh:#1114 N4: the same transient on the N2 stand-in (the ablation), for
    // the movement.
    let mut old = ControlVolume::new(
        VolumeAltitudeTable::new(&[(m(0.0), m3(0.0)), (m(2.0), m3(20.0)), (m(6.0), m3(100.0))]).unwrap(),
        vec![port(0.3, 0.1), port(0.5, 0.05), port(0.0, 0.1), port(5.8, 0.1)],
        kg(pool),
        kelv(300.0),
        n2_atmosphere(85.0, 1.0e5, 300.0, 0.5),
        kelv(300.0),
    )
    .unwrap()
    .without_ncg_transport_properties();
    for _ in 0..300 {
        old.step(sec(1.0), &inputs).unwrap();
    }
    let dt_old = old.pool_temperature().get::<kelvin>() - t_pool0;
    let dp_old = old.atmosphere_pressure().get::<pascal>() / p0 - 1.0;
    println!(
        "N4: pool +{dt_old:.6} -> +{dt:.6} K ({:+.3e} of itself), pressure {:+.5} -> {:+.5} % ({:+.3e} of itself), H2 mole fraction of the gas {:.4}",
        dt / dt_old - 1.0,
        100.0 * dp_old,
        100.0 * dp,
        dp / dp_old - 1.0,
        cv.atmosphere().ncg_moles[NonCondensable::H2.index()] / (cv.atmosphere().ncg_total_moles() + cv.atmosphere().steam_kg / M_H2O)
    );
    // Prediction N4: each moves by less than 1 % of itself.
    assert!((dt / dt_old - 1.0).abs() < 0.01 && (dp / dp_old - 1.0).abs() < 0.01);
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
/// (RH 1 from the start, steam passing through the pool), where ~~the
/// explicit surface step and rain-out at 1 s amplify any round-off~~
/// **CORRECTED 2026-11-02 (gh:#1118)** the four-pass rain-out amplifies any
/// round-off, at any step size, and the surface step does not
/// ([`wet_well_round_off_amplification_gh1118`]); the
/// growth starts when the gas first reaches saturation. The assertion is
/// therefore the explanation, not the original criterion: round-off for
/// the first 5 steps, and no more than 10x the wet well's own sensitivity.
/// The amplification itself is a property of ~~the wet well's explicit
/// step~~ the four-pass rain-out (gh:#1118), measured in the follow-up (it is not a divergence: both runs stay
/// physical and end at the same state to 1e-4).
#[test]
fn the_wet_well_is_a_special_case() {
    // Since 2026-11-02 (gh:#1118) the wet well's four passes are an
    // ablation; this record is taken on it, with the control volume's
    // four-pass ablation to match. The new defaults: below.
    let (ww, lt, z_inj, opening) = torus(T0);
    let mut ww = ww.without_equilibrium_rain_out();
    // The same wet well with its gas 1e-12 K warmer (3e-15 relative, a few
    // ulps): how far the wet well itself carries a round-off difference.
    let mut ww2 = torus(T0 + 1.0e-12).0.without_equilibrium_rain_out();
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
    // gh:#1114: each non-condensable on its own transport properties.
    assert!(mech.ncg_transport_properties);
    assert!(!cv.clone().without_ncg_transport_properties().mechanisms().ncg_transport_properties);
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

// ── gh:#1118 (a): the wet well's round-off amplification at saturation ──────

/// What one perturbed-twin run measured: the worst relative difference in
/// steam mass and in gas temperature over the run, the fraction of steps on
/// which the steam difference changed sign, and the fraction of steps on
/// which the base run's own steam mass zig-zagged (its second difference
/// changed sign), a period-2 sawtooth being the signature of an explicit
/// step beyond its stability limit.
#[derive(Debug, Clone, Copy)]
struct Twin {
    steam: f64,
    t_gas: f64,
    diff_sign_flips: f64,
    sawtooth: f64,
}

/// Steps a state and its twin (gas 1e-12 K warmer) through `t_end` s of the
/// special-case test's inputs at `dt`, and reports [`Twin`]. `step` advances
/// one state by `dt` and returns `(steam kg, gas K)`.
fn twin_run<S: Clone>(mut a: S, mut b: S, dt: f64, t_end: f64, mut step: impl FnMut(&mut S, f64) -> (f64, f64)) -> Twin {
    let n = (t_end / dt).round() as usize;
    let (mut ws, mut wt) = (0.0_f64, 0.0_f64);
    let (mut flips, mut saw) = (0usize, 0usize);
    let mut last_d = 0.0_f64;
    let mut hist: Vec<f64> = Vec::with_capacity(n);
    for _ in 0..n {
        let (sa, ta) = step(&mut a, dt);
        let (sb, tb) = step(&mut b, dt);
        ws = ws.max(rel(sb, sa));
        wt = wt.max(rel(tb, ta));
        let d = sb - sa;
        if d * last_d < 0.0 {
            flips += 1;
        }
        if d != 0.0 {
            last_d = d;
        }
        hist.push(sa);
    }
    for w in hist.windows(4) {
        if (w[3] - 2.0 * w[2] + w[1]) * (w[2] - 2.0 * w[1] + w[0]) < 0.0 {
            saw += 1;
        }
    }
    Twin { steam: ws, t_gas: wt, diff_sign_flips: flips as f64 / n as f64, sawtooth: saw as f64 / n.saturating_sub(3).max(1) as f64 }
}

fn special_case_ww_inputs() -> WetWellInputs {
    WetWellInputs {
        injection: Injection { mass_rate: kgs(10.0), specific_enthalpy: jkg(2.7e6), nitrogen_mass_rate: kgs(0.5), nitrogen_temperature: kelv(350.0) },
        external: ExternalVolume { pressure: pa(1.0e5), temperature: kelv(300.0), steam_mole_fraction: 0.0 },
        heat_exchanger: ImmersedHeatExchanger { conductance: ThermalConductance::new::<watt_per_kelvin>(1.0e6), coolant_temperature: kelv(300.0) },
        liquid_outflow: kgs(5.0),
    }
}

/// The control volume built on the torus of [`torus`] at gas temperature
/// `t_gas`, every mechanism at its default.
fn torus_cv(t_gas: f64) -> ControlVolume {
    let (ww, lt, z_inj, opening) = torus(t_gas);
    let gas = GasMixture::new(ww.steam_mass(), &[(NonCondensable::N2, AmountOfSubstance::new::<mole>(ww.nitrogen_mass().get::<kilogram>() / M_N2))]);
    ControlVolume::new(
        VolumeAltitudeTable::prismatic(Area::new::<square_meter>(AREA), m(lt)).unwrap(),
        vec![port(z_inj, opening), port(0.0, 0.1)],
        ww.pool_mass(),
        ww.pool_temperature(),
        gas,
        ww.gas_temperature(),
    )
    .unwrap()
}

fn special_case_cv_inputs() -> ControlVolumeInputs {
    let mut cv_in = ControlVolumeInputs::closed(2);
    cv_in.ports[0] = PortInput::inflow(Source::water(kgs(10.0), jkg(2.7e6)).with_gas(&[(NonCondensable::N2, kgs(0.5))], kelv(350.0)));
    cv_in.ports[1] = PortInput::outflow(kgs(5.0));
    cv_in.heat_exchanger = special_case_ww_inputs().heat_exchanger;
    cv_in
}

/// **gh:#1118 (a): where the wet well's round-off amplification comes from.**
///
/// **Methodology.** The special-case test's torus and inputs (600 s of 10
/// kg/s relief steam with 0.5 kg/s N2, the heat exchanger, a 5 kg/s
/// outlet), each model run twice, the twin's gas 1e-12 K warmer (3e-15
/// relative). Measured: the worst relative difference in steam mass and gas
/// temperature over 600 s, the fraction of steps on which the difference
/// changes sign, and the fraction of steps on which the base run's own steam
/// mass zig-zags (period-2 sawtooth). Variants: the wet well at `dt` = 1,
/// 0.5, 0.25, 0.1 s; the wet well with the surface's mass transfer ablated
/// (RELAP-7 Eq 390, `α` = 10 W/(m² K)); the control volume with the wet
/// well's conventions and the four-pass rain-out (the special case); the
/// same with the equilibrium rain-out (the control volume's default); and
/// the control volume at every default.
///
/// **Predictions, written 2026-11-02 before the first run.** The issue's
/// hypothesis is a stability limit of the explicit surface exchange and
/// rain-out at 1 s. If it holds:
/// 1. at `dt` = 1 s the difference alternates in sign from step to step
///    (an amplification factor near −2.5) and the base run's steam mass
///    zig-zags on most steps;
/// 2. halving the step brings the factor inside the unit circle (`|1 − k
///    dt|` with `k dt` ≈ 3.5 at 1 s gives 0.75 at 0.5 s), so at `dt` ≤ 0.5 s
///    the difference stays at round-off (< 1e-10 in steam mass);
/// 3. the sensible-only ablation shows no amplification (< 1e-10);
/// 4. the equilibrium rain-out does **not** remove it, because the
///    expanding step is the explicit evaporation computed from the
///    start-of-step state, which rain-out only clips (low confidence: about
///    even odds).
///
/// **Results (2026-11-02, release, Rust 1.99.1): predictions 2, 3 and 4
/// MISSED; the cause is the four-pass rain-out, not the explicit surface
/// step, and not a step-size stability limit.**
///
/// | Variant | worst steam / gas-T difference | sign flips | base sawtooth |
/// |---|---|---|---|
/// | wet well, `dt` 1 s | 1.14e-4 / 2.9e-5 | 0.45 | 0.60 |
/// | wet well, `dt` 0.5 s | 5.7e-5 / 1.5e-5 | 0.47 | 0.62 |
/// | wet well, `dt` 0.25 s | 2.9e-5 / 7.5e-6 | 0.48 | 0.62 |
/// | wet well, `dt` 0.1 s | 1.2e-5 / 3.1e-6 | 0.48 | 0.60 |
/// | wet well, sensible-only surface, 1 s | 9.9e-5 / 2.5e-5 | 0.43 | 0.62 |
/// | control volume, wet-well conventions, four passes, 1 / 0.5 / 0.25 s | 1.09e-4 / 5.4e-5 / 3.0e-5 (steam) | 0.46–0.47 | 0.60–0.63 |
/// | control volume, wet-well conventions, **equilibrium rain-out**, 1 / 0.5 / 0.25 s | 1.2e-13 / 1.3e-13 / 1.3e-13 (steam), ≤ 2.3e-14 (T) | 0.02–0.05 | 0 |
/// | control volume, every default, 1 / 0.5 / 0.25 s | 1.3e-13 / 1.3e-13 / 1.2e-13 (steam) | 0.04–0.06 | ≤ 0.002 |
///
/// - Prediction 1 held in form: the difference changes sign on about half
///   the steps and the base trajectory zig-zags on about 60 % of them.
/// - Prediction 2 missed: shortening the step does not remove it. The
///   bounded difference falls roughly in proportion to `dt` (1.1e-4 at 1 s,
///   1.2e-5 at 0.1 s), because it is bounded by the size of one step's
///   excess vapour, but it stays seven to eight orders above round-off.
///   There is no step size below which the map stops expanding, so it is
///   not the stability limit of an explicit relaxation.
/// - Prediction 3 missed: with no mass transfer at the surface (RELAP-7
///   Eq 390) it amplifies the same, so the explicit exchange is not the
///   cause.
/// - Prediction 4 missed: the control volume differs from the wet-well
///   special case only in its rain-out, and the equilibrium rain-out (its
///   default) takes the difference from 1.1e-4 to 1.2e-13, round-off, at
///   every step size, with no sawtooth.
/// - **The mechanism, stated.** `sembawang`'s four passes condense down to
///   saturation at the current temperature, leaving the latent heat in the
///   gas. That heat raises the gas's saturated capacity by more than the
///   mass condensed (by hand at 330 K and 1 bar in 2620 m³: about 13 kg/K
///   of capacity, times about 1.1 K per kg condensed against the gas's 2e6
///   J/K, a ratio of about 14). So the first pass ends far subsaturated and
///   the next three find nothing to condense. The deficit it leaves scales
///   with the excess it was given, with a gain above one, and the next
///   step's evaporation refills it. That map expands any difference in the
///   excess, and only the nonlinearity bounds it.
///
/// What is asserted is that finding: the wet well amplifies at both 1 s
/// and 0.1 s (> 1e-6), the sensible-only surface amplifies too, the
/// control volume's four passes amplify, and its equilibrium rain-out
/// stays at round-off (< 1e-11) at all three step sizes.
#[test]
fn wet_well_round_off_amplification_gh1118() {
    let ww_in = special_case_ww_inputs();
    let ww_step = |w: &mut WetWell, dt: f64| {
        w.step(sec(dt), &ww_in).unwrap();
        (w.steam_mass().get::<kilogram>(), w.gas_temperature().get::<kelvin>())
    };
    let cv_in = special_case_cv_inputs();
    let cv_step = |c: &mut ControlVolume, dt: f64| {
        c.step(sec(dt), &cv_in).unwrap();
        (c.atmosphere().steam_kg, c.atmosphere_temperature().get::<kelvin>())
    };
    // The wet well's four passes, its only rain-out when this was measured
    // and its ablation since (gh:#1118, 2026-11-02).
    let ww = |t: f64| torus(t).0.without_equilibrium_rain_out();
    let ww_default = |t: f64| torus(t).0;
    let r = twin_run(ww_default(T0), ww_default(T0 + 1e-12), 1.0, 600.0, ww_step);
    println!("wet well, its default equilibrium rain-out (since 2026-11-02), dt 1: {r:?}");
    assert!(r.steam < 1e-11 && r.t_gas < 1e-11, "{r:?}");
    for dt in [1.0, 0.5, 0.25, 0.1] {
        let r = twin_run(ww(T0), ww(T0 + 1e-12), dt, 600.0, ww_step);
        println!("wet well dt {dt}: {r:?}");
        if dt == 1.0 || dt == 0.1 {
            assert!(r.steam > 1e-6 && r.t_gas > 1e-7, "{dt}: {r:?}");
        }
        if dt == 1.0 {
            assert!(r.diff_sign_flips > 0.3 && r.sawtooth > 0.3, "{r:?}");
        }
    }
    let sens = |t: f64| ww(t).without_surface_mass_transfer(HeatTransfer::new::<uom::si::heat_transfer::watt_per_square_meter_kelvin>(10.0));
    let r = twin_run(sens(T0), sens(T0 + 1e-12), 1.0, 600.0, ww_step);
    println!("wet well, sensible-only surface: {r:?}");
    assert!(r.steam > 1e-6, "{r:?}");
    let special = |t: f64, eq: bool| {
        let c = torus_cv(t).without_real_liquid().unwrap().without_real_steam().unwrap();
        if eq { c } else { c.without_equilibrium_rain_out() }
    };
    for dt in [1.0, 0.5, 0.25] {
        let four = twin_run(special(T0, false), special(T0 + 1e-12, false), dt, 600.0, cv_step);
        let eq = twin_run(special(T0, true), special(T0 + 1e-12, true), dt, 600.0, cv_step);
        let def = twin_run(torus_cv(T0), torus_cv(T0 + 1e-12), dt, 600.0, cv_step);
        println!("cv wet-well conventions, four passes, dt {dt}: {four:?}");
        println!("cv wet-well conventions, equilibrium rain-out, dt {dt}: {eq:?}");
        println!("cv every default, dt {dt}: {def:?}");
        assert!(four.steam > 1e-6, "{dt}: {four:?}");
        for r in [eq, def] {
            assert!(r.steam < 1e-11 && r.t_gas < 1e-11 && r.sawtooth < 0.01, "{dt}: {r:?}");
        }
    }
}

/// **gh:#1118 (maintainer decision, 2026-11-02): the wet well's new default
/// against the control volume's.** Since the wet well's default rain-out is
/// the control volume's equilibrium solve, the special case is repeated with
/// both at their defaults. The control volume keeps the wet well's energy
/// conventions (constant-`c_p` liquid, ideal steam with constant `c_v`).
/// The inputs and the torus are [`the_wet_well_is_a_special_case`]'s.
///
/// **Prediction E4 (written before the change):** agreement to < 1e-9
/// relative over all 600 s in all six quantities. That is the original
/// prediction 2, which missed only because the four passes amplify
/// round-off. It is not expected bit for bit: Dalton's sum, the
/// energy-to-temperature solve, the 1e-13 bisection tolerance and the
/// control volume's fixed density inside its rain-out probe all differ.
///
/// **Result (2026-11-02, release): held.** Worst relative differences over
/// 600 s: pool temperature 2.4e-15, gas temperature 1.1e-11, pressure
/// 9.3e-12, pool mass 2.5e-15, steam mass 6.1e-11, level 3.0e-15. The
/// four-pass record of [`the_wet_well_is_a_special_case`] (up to 1.05e-4)
/// came from the four passes' amplification, not from the control volume.
#[test]
fn the_wet_well_is_a_special_case_at_both_defaults() {
    let (mut ww, lt, z_inj, opening) = torus(T0);
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
    .unwrap();
    let ww_in = special_case_ww_inputs();
    let cv_in = special_case_cv_inputs();
    let mut worst = [0.0_f64; 6];
    for _ in 0..600 {
        ww.step(sec(1.0), &ww_in).unwrap();
        cv.step(sec(1.0), &cv_in).unwrap();
        let a = [
            ww.pool_temperature().get::<kelvin>(),
            ww.gas_temperature().get::<kelvin>(),
            ww.gas_pressure().get::<pascal>(),
            ww.pool_mass().get::<kilogram>(),
            ww.steam_mass().get::<kilogram>(),
            ww.pool_level().get::<meter>(),
        ];
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
        }
    }
    println!("both defaults, worst relative [T_pool, T_gas, p, m_pool, m_steam, level]: {worst:?}");
    assert!(worst.iter().all(|&x| x < 1e-9), "{worst:?}");
}

/// **gh:#1117: a port's outflow split by its liquid fraction.** A 10 m²
/// vessel, 2 m of water at 320 K, N2 at 1.2 bar and RH 0.5 above, a port at
/// 1.9 m. One 1 s step drawing 2 kg with `liquid_fraction` 0.25.
///
/// **Predictions (written before the run):**
/// - the port's water out is 0.5 kg of liquid plus the steam share of 1.5
///   kg of atmosphere;
/// - its non-condensables out are the N2 share of the 1.5 kg;
/// - water, N2 and energy close against the port's report to < 1e-12;
/// - `Some(1.0)` on a submerged port, and `Some(0.0)` on a port above the
///   level, give the point draw's result bit for bit;
/// - a fraction outside [0, 1] is refused.
///
/// **Result (2026-11-02, release):** held (assertions below; printed
/// line).
#[test]
fn a_port_outflow_splits_by_its_liquid_fraction() {
    let build = || {
        ControlVolume::new(
            VolumeAltitudeTable::prismatic(Area::new::<square_meter>(10.0), m(4.0)).unwrap(),
            vec![port(1.9, 0.4), port(3.5, 0.1)],
            kg(20.0 * rho_if97(320.0, 1.2e5)),
            kelv(320.0),
            n2_atmosphere(20.0, 1.2e5, 320.0, 0.5),
            kelv(320.0),
        )
        .unwrap()
    };
    let mut cv = build();
    let (w0, n0, e0) = (cv.water_mass().get::<kilogram>(), cv.ncg_masses()[0].get::<kilogram>(), cv.total_energy().get::<joule>());
    let a = cv.atmosphere();
    let x_s = a.steam_kg / a.total_mass().get::<kilogram>();
    let mut inputs = ControlVolumeInputs::closed(2);
    inputs.ports[0] = PortInput { liquid_fraction: Some(0.25), ..PortInput::outflow(kgs(2.0)) };
    let r = cv.step(sec(1.0), &inputs).unwrap();
    let pt = &r.ports[0];
    let (w_out, n_out, e_out) = (pt.water_out.get::<kilogram>(), pt.ncg_out[0].get::<kilogram>(), pt.energy_out.get::<joule>());
    let dw = cv.water_mass().get::<kilogram>() - w0 + w_out;
    let dn = cv.ncg_masses()[0].get::<kilogram>() - n0 + n_out;
    let de = cv.total_energy().get::<joule>() - e0 + e_out;
    println!("split: water out {w_out:.12} kg (0.5 + steam {:.12}), N2 out {n_out:.12} kg (expected {:.12}); closure water {dw:.1e} kg, N2 {dn:.1e} kg, energy {de:.1e} J", 1.5 * x_s, 1.5 * (1.0 - x_s));
    assert!((w_out - (0.5 + 1.5 * x_s)).abs() < 1e-12 && (n_out - 1.5 * (1.0 - x_s)).abs() < 1e-12);
    assert!(dw.abs() < 1e-12 * w0 && dn.abs() < 1e-12 * n0 && de.abs() < 1e-12 * e0.abs());
    // The ends of the range are the point draw, bit for bit.
    for (p, f) in [(0usize, 1.0), (1, 0.0)] {
        let (mut x, mut y) = (build(), build());
        let mut ix = ControlVolumeInputs::closed(2);
        ix.ports[p] = PortInput::outflow(kgs(0.3));
        let mut iy = ix.clone();
        iy.ports[p].liquid_fraction = Some(f);
        let (rx, ry) = (x.step(sec(1.0), &ix).unwrap(), y.step(sec(1.0), &iy).unwrap());
        assert_eq!(rx.ports[p], ry.ports[p], "port {p}");
        assert_eq!(x.total_energy().get::<joule>().to_bits(), y.total_energy().get::<joule>().to_bits());
        assert_eq!(x.pool_mass().get::<kilogram>().to_bits(), y.pool_mass().get::<kilogram>().to_bits());
    }
    let mut bad = ControlVolumeInputs::closed(2);
    bad.ports[0] = PortInput { liquid_fraction: Some(1.5), ..PortInput::outflow(kgs(0.1)) };
    assert!(build().step(sec(1.0), &bad).is_err());
}
