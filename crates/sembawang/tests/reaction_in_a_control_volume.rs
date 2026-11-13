//! # V&V: atmosphere reactions in a `tampines` control volume (gh:#1100; moved to sembawang, gh:#1103)
//!
//! > Research, education and V&V only. AI draft, unreviewed.
//!
//! **Ported on 2026-11-01 from `tampines/tests/control_volume_reactions.rs`
//! (gh:#1103: `tampines` holds thermal hydraulics, no chemistry).** The
//! reactions are `sembawang::bwr_mark1::reaction` now: a burn at `h2`,
//! `co` mol/s becomes `tampines`' species sinks and sources plus the heat of
//! reaction ([`burning`], the way `ContainmentNetwork` drives its building).
//! The cases are the same, R1 to R6; the predictions were written before the
//! first run in `tampines`
//! (`crates/tampines/verification_and_validation/control_volume.md`,
//! "Atmosphere reactions"), and the port's prediction (every result the same
//! to round-off) and results are in
//! `verification_and_validation/bwr_mark1/containment_network.md`.

use sembawang::bwr_mark1::reaction;
use tampines::control_volume::*;
use tampines::cv_network::{CvNetwork, NetworkControl};
use tampines_steam_tables::gas_mixture::M_H2O;
use tampines_steam_tables::region_1_subcooled_liquid::intensive_properties::{h_tp_1, v_tp_1};
use tampines_steam_tables::region_2_vapour::h_tp_2;
use uom::si::amount_of_substance::mole;
use uom::si::available_energy::joule_per_kilogram;
use uom::si::energy::joule;
use uom::si::f64::*;
use uom::si::length::meter;
use uom::si::mass::kilogram;
use uom::si::mass_rate::kilogram_per_second;
use uom::si::pressure::pascal;
use uom::si::specific_volume::cubic_meter_per_kilogram;
use uom::si::thermodynamic_temperature::kelvin;
use uom::si::time::second;
use uom::si::volume::cubic_meter;

const R: f64 = 8.314_462;

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
fn mol(x: f64) -> AmountOfSubstance {
    AmountOfSubstance::new::<mole>(x)
}
fn sec(x: f64) -> Time {
    Time::new::<second>(x)
}
fn port(z: f64, h: f64) -> Port {
    Port { elevation: m(z), opening_height: m(h) }
}
/// A burn at `h2`, `co` mol/s (the former `AtmosphereReactions`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Burn {
    h2: f64,
    co: f64,
}
fn burn(h2: f64, co: f64) -> Burn {
    Burn { h2, co }
}

/// What a burning step did: H2 and CO burned, mol, and the stored-energy
/// release `Q`, J (the former report's `hydrogen_burned`,
/// `carbon_monoxide_burned` and `reaction_heat`).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Burned {
    h2: f64,
    co: f64,
    q: f64,
}

/// One step of `cv` with the burn `rx` (if any) applied as sembawang
/// applies it: `reaction::burn_inputs` on the start-of-step volume, its
/// species rates as `atmosphere_species` and its heat added to
/// `atmosphere_heat`.
fn burning(cv: &mut ControlVolume, inputs: &ControlVolumeInputs, rx: Option<Burn>) -> Result<(ControlVolumeStepReport, Burned), tampines::TampinesError> {
    let mut inp = inputs.clone();
    let mut b = Burned { h2: 0.0, co: 0.0, q: 0.0 };
    if let Some(r) = rx {
        let (rates, heat_w, x, q) = reaction::burn_inputs(cv, r.h2 * 1.0, r.co * 1.0, 1.0)?;
        inp.atmosphere_species = rates;
        inp.atmosphere_heat = inp.atmosphere_heat + Power::new::<uom::si::power::watt>(heat_w);
        b = Burned { h2: x.xi_h, co: x.xi_c, q };
    }
    let r = cv.step(sec(1.0), &inp)?;
    Ok((r, b))
}

/// A dry (no pool), portless prismatic volume of `v` m³ holding `gas` at `t`.
fn dry_volume(v: f64, gas: &[(NonCondensable, f64)], t: f64) -> ControlVolume {
    let table = VolumeAltitudeTable::new(&[(m(0.0), m3(0.0)), (m(1.0), m3(v))]).unwrap();
    let list: Vec<(NonCondensable, AmountOfSubstance)> = gas.iter().map(|(g, n)| (*g, mol(*n))).collect();
    ControlVolume::new(table, vec![], kg(0.0), kelv(t), GasMixture::new(kg(0.0), &list), kelv(t)).unwrap()
}

fn n_of(cv: &ControlVolume, g: NonCondensable) -> f64 {
    cv.atmosphere().ncg_moles[g.index()]
}

// ── R5: zero reaction is the old code, bit for bit ─────────────────────────

/// The pin transient: a pool, steam, all six gases, a steam inflow below the
/// pool, an H2 + N2 inflow, a drain, a vent, atmosphere heat and a steam
/// sink, 60 steps of 1 s; 12 values every 10 steps. `n2_stand_in` runs it on
/// `without_ncg_transport_properties()`, the pool-surface physics the pins
/// were taken with (every non-condensable as N2; gh:#1114 made each gas's own
/// properties the default on 2026-11-02).
fn pin_transient(rx: Option<Burn>, n2_stand_in: bool) -> Vec<u64> {
    let table = VolumeAltitudeTable::new(&[(m(0.0), m3(0.0)), (m(2.0), m3(20.0)), (m(6.0), m3(100.0))]).unwrap();
    let ports = vec![port(0.3, 0.1), port(0.5, 0.05), port(0.0, 0.1), port(5.8, 0.1)];
    let rho = 1.0 / v_tp_1(kelv(320.0), Pressure::new::<pascal>(1.0e5)).get::<cubic_meter_per_kilogram>();
    let atm = GasMixture::new(
        kg(1.5),
        &[
            (NonCondensable::N2, mol(2500.0)),
            (NonCondensable::O2, mol(600.0)),
            (NonCondensable::H2, mol(80.0)),
            (NonCondensable::CO, mol(20.0)),
            (NonCondensable::CO2, mol(5.0)),
            (NonCondensable::Ar, mol(30.0)),
        ],
    );
    let cv = ControlVolume::new(table, ports, kg(15.0 * rho), kelv(320.0), atm, kelv(330.0)).unwrap();
    let mut cv = if n2_stand_in { cv.without_ncg_transport_properties() } else { cv };
    let mut inputs = ControlVolumeInputs::closed(4);
    inputs.ports[0] = PortInput::inflow(Source::water(kgs(0.5), AvailableEnergy::new::<joule_per_kilogram>(2.75e6)));
    inputs.ports[1] = PortInput::inflow(Source::gas(&[(NonCondensable::H2, kgs(0.002)), (NonCondensable::N2, kgs(0.01))], kelv(400.0)));
    inputs.ports[2] = PortInput::outflow(kgs(0.3));
    inputs.ports[3] = PortInput::outflow(kgs(0.01));
    inputs.atmosphere_heat = Power::new::<uom::si::power::watt>(2.0e4);
    inputs.atmosphere_steam_sink = kgs(0.001);
    let mut out = Vec::new();
    for k in 0..60 {
        let (r, b) = burning(&mut cv, &inputs, rx).unwrap();
        assert_eq!((b.h2, b.co, b.q), (0.0, 0.0, 0.0));
        assert_eq!(r.species_energy.get::<joule>(), 0.0);
        if k % 10 == 9 {
            let a = cv.atmosphere();
            out.push(cv.pool_temperature().get::<kelvin>().to_bits());
            out.push(cv.atmosphere_temperature().get::<kelvin>().to_bits());
            out.push(cv.atmosphere_pressure().get::<pascal>().to_bits());
            out.push(cv.total_energy().get::<joule>().to_bits());
            out.push(cv.pool_mass().get::<kilogram>().to_bits());
            out.push(a.steam_kg.to_bits());
            for n in a.ncg_moles {
                out.push(n.to_bits());
            }
        }
    }
    out
}

/// The 72 values taken from the code **before** the reaction input existed
/// (2026-11-01, release, Rust 1.99.1, the same transient).
const PIN: [u64; 72] = [0x4074034a36fc8a64, 0x4074c7d300f01a1c, 0x40fa70d11021895f, 0x41e5fbf1f5491585, 0x40ccfdc2a6c02215, 0x3ff878d3f8f27f4a, 0x40a389c1921623c2, 0x4056750973146aa5, 0x4082bad613229eba, 0x4033fa7df2470fb7, 0x4013fa7df2470fb7, 0x403df7bceb6a9792, 0x4074069414bc9fad, 0x4074eeadfd02d509, 0x40faba6edacea139, 0x41e61593e884b875, 0x40ccfebd465e77ba, 0x3ff8f51c0b34a5f5, 0x40a38b836be13e07, 0x4058e966979a0a0d, 0x4082b5ae4d9d02a5, 0x4033f4fe30a77a49, 0x4013f4fe30a77a49, 0x403def7d48fb3773, 0x407409dd9b955a97, 0x407514923fd85e3c, 0x40fb033d1dfe7240, 0x41e62f34608dd5b8, 0x40ccffb7cb3d4691, 0x3ff9749a6e18353d, 0x40a38d45906f2c8b, 0x405b5d17d2203f09, 0x4082b088b14d610c, 0x4033ef80bd1f5674, 0x4013ef80bd1f5674, 0x403de7411baf01b2, 0x40740d26cd1c44f6, 0x40753981b7e7f60f, 0x40fb4b3a2611b272, 0x41e648d35e13bcbf, 0x40cd00b236be26b6, 0x3ff9f7221fc2d2f6, 0x40a38f08029a4c6b, 0x405dd01d87db5d37, 0x4082ab653fde1d6d, 0x4033ea05997574ba, 0x4013ea05997574ba, 0x403ddf0866302f1a, 0x4074106faa60e3a7, 0x40755d7edb07fc63, 0x40fb92653a261992, 0x41e66270e1c78343, 0x40cd01ac89e99bc1, 0x3ffa7c914c349ef9, 0x40a390cac5168b36, 0x4060213c0f4f76e1, 0x4082a643fad28a2d, 0x4033e48cc746f9ca, 0x4013e48cc746f9ca, 0x403dd6d32aea76b4, 0x407413b83410437e, 0x4075808c909e071a, 0x40fbd8be6db9af37, 0x41e67c0cec5b111b, 0x40cd02a6c589e3a9, 0x3ffb04cdf0d27fa0, 0x40a3928dda7a4b19, 0x40615a13fe6c84af, 0x4082a124e38f9830, 0x4033df164810a253, 0x4013df164810a253, 0x403dcea16c18f382];

/// R5. **Result (2026-11-01, in `tampines`):** all 72 values bit-identical,
/// with the input absent, `none()` and explicit zero rates. Ported: the
/// same 72 values, the species input absent and a zero burn through
/// `reaction::burn_inputs` (which then sets no species and no heat).
///
/// **Re-pointed 2026-11-04:** gh:#1114 (`093f59312`, 2026-11-02) gave each
/// non-condensable its own transport properties at the pool surface by
/// default, a deliberate physics change that moved this transient (first
/// value 320.2056188 -> 320.2056195 K; found by `git bisect` with this test
/// as the oracle, it passes at the pinning commit `0cb0324bc` on the same
/// host). As that commit did for its own goldens, the pin is kept on the
/// ablation that restores the old physics, where the zero reaction must
/// still reproduce it bit for bit; at the new default the two zero-reaction
/// arms must be bit-identical to each other.
#[test]
fn r5_zero_reaction_is_the_old_code_bit_for_bit() {
    assert_eq!(AtmosphereSpeciesRates::default(), AtmosphereSpeciesRates::none());
    assert_eq!(ControlVolumeInputs::closed(0).atmosphere_species, AtmosphereSpeciesRates::none());
    assert_eq!(pin_transient(None, false), pin_transient(Some(burn(0.0, 0.0)), false));
    for rx in [None, Some(burn(0.0, 0.0))] {
        let v = pin_transient(rx, true);
        assert_eq!(v.len(), PIN.len());
        for (i, (a, b)) in v.iter().zip(PIN.iter()).enumerate() {
            assert_eq!(a, b, "value {i} ({} vs {}) with {rx:?}", f64::from_bits(*a), f64::from_bits(*b));
        }
    }
}

// ── R1: heat of reaction at 298.15 K against JANAF ─────────────────────────

/// `−Δ_r H(298.15 K)` per mole of fuel, measured through one step: burn
/// `xi` mol in a dry 10 m³ volume at 298.15 K, then the heat to bring the
/// products back to 298.15 K at constant volume (the stored energy less the
/// products' code `U(298.15 K)`) plus `V Δp` (to an enthalpy).
fn measured_heat_of_reaction(fuel: NonCondensable, xi: f64) -> (f64, f64) {
    let t = 298.15;
    let mut cv = dry_volume(10.0, &[(fuel, 4.0), (NonCondensable::O2, 40.0)], t);
    let model = cv.gas_model();
    let st = |cv: &ControlVolume| model.state(&cv.atmosphere(), m3(10.0), kelv(t)).unwrap();
    let p0 = st(&cv).pressure.get::<pascal>();
    let inputs = ControlVolumeInputs::closed(0);
    let rx = if fuel == NonCondensable::H2 { burn(xi, 0.0) } else { burn(0.0, xi) };
    let (_, b) = burning(&mut cv, &inputs, Some(rx)).unwrap();
    let burned = if fuel == NonCondensable::H2 { b.h2 } else { b.co };
    assert_eq!(burned, xi);
    let s1 = st(&cv);
    let q_v = cv.total_energy().get::<joule>() - s1.internal_energy.get::<joule>();
    let dh = (q_v - 10.0 * (s1.pressure.get::<pascal>() - p0)) / xi;
    (dh, cv.atmosphere_temperature().get::<kelvin>())
}

/// R1. JANAF (Chase 1998): `−Δ_r H(298.15)` = 241.826 kJ/mol for
/// `H2 + ½O2 → H2O(g)` (the LHV) and 393.522 − 110.527 = 282.995 kJ/mol for
/// `CO + ½O2 → CO2`. Also the IF97-to-JANAF steam link: IF97's
/// `M (h_ig(298.15) − h_liq(298.15, 1 bar))` against JANAF's
/// `Δ_f H°(H2O,g) − Δ_f H°(H2O,l)` = −241.826 + 285.830 = 44.004 kJ/mol,
/// and the code's "from 0 K" enthalpies at 298.15 K against JANAF's
/// `H°(298.15) − H°(0)` (printed: why the anchor is 298.15 K).
///
/// **Result (2026-11-01):** see the V&V doc (numbers printed with
/// `--nocapture`).
#[test]
fn r1_heat_of_reaction_at_298_against_janaf() {
    let (dh_h2, t_h2) = measured_heat_of_reaction(NonCondensable::H2, 2.0);
    let (dh_co, t_co) = measured_heat_of_reaction(NonCondensable::CO, 2.0);
    let e_h2 = dh_h2 / 241_826.0 - 1.0;
    let e_co = dh_co / 282_995.0 - 1.0;
    println!("R1 H2: {:.3} J/mol vs JANAF 241826 ({:+.3e}); burned gas reached {t_h2:.1} K [{dh_h2:.10}]", dh_h2, e_h2);
    println!("R1 CO: {:.3} J/mol vs JANAF 282995 ({:+.3e}); vs the issue's 282980 ({:+.3e}); gas {t_co:.1} K", dh_co, e_co, dh_co / 282_980.0 - 1.0);
    assert!(e_h2.abs() < 1e-3 && e_co.abs() < 1e-3);

    let cv = dry_volume(10.0, &[(NonCondensable::H2, 1.0)], 298.15);
    let q = reaction::energy_per_mole(&cv);
    println!("R1 stored-energy release per mole (code references): H2 {:.3}, CO {:.3} J/mol", q[0], q[1]);

    // IF97 to JANAF.
    let h_g = h_tp_2(kelv(298.15), Pressure::new::<pascal>(1.0)).get::<joule_per_kilogram>();
    let h_l = h_tp_1(kelv(298.15), Pressure::new::<pascal>(1.0e5)).get::<joule_per_kilogram>();
    let dh_vap = M_H2O * (h_g - h_l);
    println!("R1 IF97 h_ig(298.15) {h_g:.3} J/kg; M(h_ig − h_l) = {dh_vap:.3} J/mol vs JANAF 44004 ({:+.3e})", dh_vap / 44_004.0 - 1.0);
    assert!((dh_vap / 44_004.0 - 1.0).abs() < 1e-3);

    // The code's "from 0 K" enthalpy at 298.15 K against JANAF H°(298.15) − H°(0).
    let janaf = [
        (NonCondensable::H2, 8_468.0),
        (NonCondensable::O2, 8_683.0),
        (NonCondensable::N2, 8_670.0),
        (NonCondensable::CO, 8_671.0),
        (NonCondensable::CO2, 9_364.0),
        (NonCondensable::Ar, 6_197.0),
    ];
    for (label, model) in [("default", GasMixtureModel::new()), ("all NASA-7", GasMixtureModel { heat_capacity: tampines_steam_tables::gas_mixture::NcgHeatCapacity::Nasa7, ..GasMixtureModel::new() })] {
        for (g, h0) in janaf {
            let h = model.ncg_u(g, 298.15) + R * 298.15;
            println!("R1 {label:>10} {g:?}: code h(298.15) from 0 K {h:.1} J/mol, JANAF H(298.15)-H(0) {h0:.0}, diff {:+.1}", h - h0);
        }
    }
}

// ── R2: adiabatic constant-volume complete burn, against a hand calculation ─

/// GRI-Mech 3.0 NASA-7 coefficients, all seven (`a5` the enthalpy constant,
/// carrying GRI's own formation enthalpy), from Cantera v3.0.0
/// `data/gri30.yaml` (BSD-3-Clause; GRI-Mech open), downloaded 2026-11-01.
/// `(T_low, T_mid, T_high, low[7], high[7])`. Independent of the code under
/// test: the code keeps only five `c_p` coefficients, uses MELCOR's fits for
/// N2 and H2 and IF97 for steam, and JANAF's formation enthalpies.
type Gri = (f64, f64, f64, [f64; 7], [f64; 7]);
const GRI_H2: Gri = (
    200.0,
    1000.0,
    3500.0,
    [2.34433112, 7.98052075e-03, -1.9478151e-05, 2.01572094e-08, -7.37611761e-12, -917.935173, 0.683010238],
    [3.3372792, -4.94024731e-05, 4.99456778e-07, -1.79566394e-10, 2.00255376e-14, -950.158922, -3.20502331],
);
const GRI_O2: Gri = (
    200.0,
    1000.0,
    3500.0,
    [3.78245636, -2.99673416e-03, 9.84730201e-06, -9.68129509e-09, 3.24372837e-12, -1063.94356, 3.65767573],
    [3.28253784, 1.48308754e-03, -7.57966669e-07, 2.09470555e-10, -2.16717794e-14, -1088.45772, 5.45323129],
);
const GRI_H2O: Gri = (
    200.0,
    1000.0,
    3500.0,
    [4.19864056, -2.0364341e-03, 6.52040211e-06, -5.48797062e-09, 1.77197817e-12, -3.02937267e+04, -0.849032208],
    [3.03399249, 2.17691804e-03, -1.64072518e-07, -9.7041987e-11, 1.68200992e-14, -3.00042971e+04, 4.9667701],
);
const GRI_N2: Gri = (
    300.0,
    1000.0,
    5000.0,
    [3.298677, 1.4082404e-03, -3.963222e-06, 5.641515e-09, -2.444854e-12, -1020.8999, 3.950372],
    [2.92664, 1.4879768e-03, -5.68476e-07, 1.0097038e-10, -6.753351e-15, -922.7977, 5.980528],
);

/// GRI absolute molar enthalpy, J/mol: `h/R = a0 T + a1 T²/2 + a2 T³/3 +
/// a3 T⁴/4 + a4 T⁵/5 + a5`.
fn gri_h(g: &Gri, t: f64) -> f64 {
    let a = if t < g.1 { &g.3 } else { &g.4 };
    R * (t * (a[0] + t * (a[1] / 2.0 + t * (a[2] / 3.0 + t * (a[3] / 4.0 + t * a[4] / 5.0)))) + a[5])
}
/// GRI absolute molar internal energy (ideal gas), J/mol.
fn gri_u(g: &Gri, t: f64) -> f64 {
    gri_h(g, t) - R * t
}

/// R2. 1 m³, dry, 8 % H2 in air (O2 : N2 = 21 : 79), 300 K, 1 bar, burned to
/// completion over 20 steps (the last ones capped by the H2 left), and in
/// one step. The hand calculation: `Σ n_i u_i(300) = Σ n'_i u_i(T_end)` with
/// GRI's absolute `u` for H2, O2, N2 and H2O(g) (ideal gases), solved by
/// bisection; `p_end = n' R T_end / V`.
///
/// **Result (2026-11-01):** see the V&V doc.
#[test]
fn r2_adiabatic_complete_burn_against_a_hand_calculation() {
    let (v, t0, p0) = (1.0, 300.0, 1.0e5);
    let n = p0 * v / (R * t0);
    let (nh, no, nn) = (0.08 * n, 0.92 * n * 0.21, 0.92 * n * 0.79);
    let gas = [(NonCondensable::H2, nh), (NonCondensable::O2, no), (NonCondensable::N2, nn)];
    // The hand calculation.
    let u0 = nh * gri_u(&GRI_H2, t0) + no * gri_u(&GRI_O2, t0) + nn * gri_u(&GRI_N2, t0);
    let (po, pw) = (no - 0.5 * nh, nh);
    let u_end = |t: f64| po * gri_u(&GRI_O2, t) + pw * gri_u(&GRI_H2O, t) + nn * gri_u(&GRI_N2, t);
    let (mut lo, mut hi) = (300.0, 3000.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if u_end(mid) > u0 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    let t_hand = 0.5 * (lo + hi);
    // Attribution only (not a gate; it reuses the code's N2 fit): the same
    // hand calculation with the code's default (MELCOR) N2 energy in place
    // of GRI's, to see how much of the difference the N2 fit carries.
    let model = GasMixtureModel::new();
    let n2_code = |t: f64| model.ncg_u(NonCondensable::N2, t);
    let u0b = nh * gri_u(&GRI_H2, t0) + no * gri_u(&GRI_O2, t0) + nn * n2_code(t0);
    let u_endb = |t: f64| po * gri_u(&GRI_O2, t) + pw * gri_u(&GRI_H2O, t) + nn * n2_code(t);
    let (mut lo, mut hi) = (300.0, 3000.0);
    for _ in 0..200 {
        let mid = 0.5 * (lo + hi);
        if u_endb(mid) > u0b {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    println!("R2 hand with the code's MELCOR N2 (attribution): T_end {:.2} K", 0.5 * (lo + hi));
    let p_hand = (po + pw + nn) * R * t_hand / v;
    println!("R2 GRI's own Δ_f H(H2O, g, 298.15) = {:.1} J/mol (JANAF −241826)", gri_h(&GRI_H2O, 298.15) - gri_h(&GRI_H2, 298.15) - 0.5 * gri_h(&GRI_O2, 298.15));
    println!("R2 hand: T_end {t_hand:.2} K, p_end {:.4} bar ({:.3} mol H2, {:.3} O2, {:.3} N2)", p_hand / 1e5, nh, no, nn);

    // The control volume, 20 steps.
    let mut cv = dry_volume(v, &gas, t0);
    let e0 = cv.total_energy().get::<joule>();
    let p_cv0 = cv.atmosphere_pressure().get::<pascal>();
    let inputs = ControlVolumeInputs::closed(0);
    let mut q = 0.0;
    for _ in 0..20 {
        q += burning(&mut cv, &inputs, Some(burn(nh / 16.0, 0.0))).unwrap().1.q;
    }
    assert_eq!(n_of(&cv, NonCondensable::H2), 0.0);
    let (t_cv, p_cv) = (cv.atmosphere_temperature().get::<kelvin>(), cv.atmosphere_pressure().get::<pascal>());
    let e_t = (t_cv - t_hand) / (t_hand - t0);
    let e_p = p_cv / p_hand - 1.0;
    println!("R2 code (20 steps, full precision): T_end {t_cv:.10} K, p_end {p_cv:.6} Pa");
    println!("R2 code (20 steps): T_end {t_cv:.2} K, p_end {:.4} bar (initial {:.5} bar); ΔT error {e_t:+.3e} of the rise, p error {e_p:+.3e}", p_cv / 1e5, p_cv0 / 1e5);
    let de = cv.total_energy().get::<joule>() - e0 - q;
    println!("R2 energy closure {de:e} J of {q:e} J released");
    assert!(de.abs() < 1e-9 * q);

    // One step: the same end state (energy and composition are path-independent).
    let mut one = dry_volume(v, &gas, t0);
    burning(&mut one, &inputs, Some(burn(2.0 * nh, 0.0))).unwrap();
    let t_one = one.atmosphere_temperature().get::<kelvin>();
    println!("R2 code (1 step): T_end {t_one:.6} K vs {t_cv:.6} K");
    assert!((t_one - t_cv).abs() < 1e-6 * t_cv);

    assert!(e_t.abs() < 1e-2, "temperature rise off by {e_t}");
    assert!(e_p.abs() < 1e-2, "pressure off by {e_p}");
}

// ── R3: limiting reactant ──────────────────────────────────────────────────

/// R3. **Result (2026-11-01):** as predicted (exact zeros; see the V&V doc).
#[test]
fn r3_limiting_reactant() {
    let rel = |a: f64, b: f64| (a - b).abs() / b.abs();
    // O2-starved.
    let mut cv = dry_volume(10.0, &[(NonCondensable::H2, 10.0), (NonCondensable::O2, 2.0), (NonCondensable::N2, 30.0)], 300.0);
    let inputs = ControlVolumeInputs::closed(0);
    let rx = Some(burn(100.0, 0.0));
    let (_, r) = burning(&mut cv, &inputs, rx).unwrap();
    println!("R3 O2-starved: burned {:?}, H2 {}, O2 {}, steam {} kg, T {:.1} K", r.h2, n_of(&cv, NonCondensable::H2), n_of(&cv, NonCondensable::O2), cv.atmosphere().steam_kg, cv.atmosphere_temperature().get::<kelvin>());
    assert_eq!(n_of(&cv, NonCondensable::O2), 0.0);
    assert!(rel(n_of(&cv, NonCondensable::H2), 6.0) < 1e-14);
    assert!(rel(r.h2, 4.0) < 1e-14);
    assert!(rel(cv.atmosphere().steam_kg, 4.0 * M_H2O) < 1e-14);
    // Further steps burn nothing.
    let (_, r) = burning(&mut cv, &inputs, rx).unwrap();
    assert_eq!((r.h2, r.q), (0.0, 0.0));

    // Fuel-limited, O2 in excess.
    let mut cv = dry_volume(10.0, &[(NonCondensable::H2, 3.0), (NonCondensable::O2, 10.0), (NonCondensable::N2, 30.0)], 300.0);
    let (_, r) = burning(&mut cv, &inputs, rx).unwrap();
    assert_eq!(n_of(&cv, NonCondensable::H2), 0.0);
    assert_eq!(r.h2, 3.0);
    assert!(rel(n_of(&cv, NonCondensable::O2), 8.5) < 1e-14);

    // Both fuels O2-starved: the asked ratio (3 : 1) is kept. (The first
    // run held only 10 mol of each fuel, so the fuel cap made the asked
    // extents 10 : 10 before the oxygen scaled them: the case, not the
    // volume, was wrong; 50 mol of each keeps the fuel cap out of it.)
    let mut cv = dry_volume(10.0, &[(NonCondensable::H2, 50.0), (NonCondensable::CO, 50.0), (NonCondensable::O2, 2.0), (NonCondensable::N2, 30.0)], 300.0);
    let (_, r) = burning(&mut cv, &inputs, Some(burn(30.0, 10.0))).unwrap();
    let (xh, xc) = (r.h2, r.co);
    println!("R3 both starved: ξ_H {xh}, ξ_C {xc}, CO2 {}", n_of(&cv, NonCondensable::CO2));
    assert_eq!(n_of(&cv, NonCondensable::O2), 0.0);
    assert!(rel(xh, 3.0) < 1e-14 && rel(xc, 1.0) < 1e-14);
    assert!(rel(n_of(&cv, NonCondensable::CO2), 1.0) < 1e-14 && rel(n_of(&cv, NonCondensable::CO), 49.0) < 1e-14);
    assert!(cv.atmosphere().ncg_moles.iter().all(|n| *n >= 0.0));

    // A negative or non-finite rate is refused.
    assert!(burning(&mut cv, &inputs, Some(burn(-1.0, 0.0))).is_err());
    assert!(burning(&mut cv, &inputs, Some(burn(0.0, f64::NAN))).is_err());
}

// ── R4: elements and energy over a transient ───────────────────────────────

fn mw(g: NonCondensable) -> f64 {
    g.molar_mass()
}

/// `[H, O, C, N, Ar]` atoms, mol, in water `w` kg and gas masses `g` kg.
fn elements(w: f64, g: &[f64; 6]) -> [f64; 5] {
    use NonCondensable::*;
    let n = |x: NonCondensable| g[x.index()] / mw(x);
    let nw = w / M_H2O;
    [2.0 * nw + 2.0 * n(H2), nw + 2.0 * n(O2) + n(CO) + 2.0 * n(CO2), n(CO) + n(CO2), 2.0 * n(N2), n(Ar)]
}

/// R4. The pin transient's volume with both reactions on, 60 steps.
///
/// **Result (2026-11-01):** see the V&V doc.
#[test]
fn r4_elements_and_energy_close() {
    let table = VolumeAltitudeTable::new(&[(m(0.0), m3(0.0)), (m(2.0), m3(20.0)), (m(6.0), m3(100.0))]).unwrap();
    let ports = vec![port(0.3, 0.1), port(0.5, 0.05), port(0.0, 0.1), port(5.8, 0.1)];
    let rho = 1.0 / v_tp_1(kelv(320.0), Pressure::new::<pascal>(1.0e5)).get::<cubic_meter_per_kilogram>();
    let atm = GasMixture::new(
        kg(1.5),
        &[
            (NonCondensable::N2, mol(2500.0)),
            (NonCondensable::O2, mol(600.0)),
            (NonCondensable::H2, mol(80.0)),
            (NonCondensable::CO, mol(20.0)),
            (NonCondensable::CO2, mol(5.0)),
            (NonCondensable::Ar, mol(30.0)),
        ],
    );
    let mut cv = ControlVolume::new(table, ports, kg(15.0 * rho), kelv(320.0), atm, kelv(330.0)).unwrap();
    let mass = |cv: &ControlVolume| (cv.water_mass().get::<kilogram>(), cv.ncg_masses().map(|x| x.get::<kilogram>()));
    let (w0, g0) = mass(&cv);
    let el0 = elements(w0, &g0);
    let e0 = cv.total_energy().get::<joule>();
    let mut inputs = ControlVolumeInputs::closed(4);
    inputs.ports[0] = PortInput::inflow(Source::water(kgs(0.5), AvailableEnergy::new::<joule_per_kilogram>(2.75e6)));
    inputs.ports[1] = PortInput::inflow(Source::gas(&[(NonCondensable::H2, kgs(0.002)), (NonCondensable::N2, kgs(0.01))], kelv(400.0)));
    inputs.ports[2] = PortInput::outflow(kgs(0.3));
    inputs.ports[3] = PortInput::outflow(kgs(0.01));
    inputs.atmosphere_heat = Power::new::<uom::si::power::watt>(2.0e4);
    inputs.atmosphere_steam_sink = kgs(0.001);
    let (mut sh, mut sc, mut sq) = (0.0, 0.0, 0.0);
    for _ in 0..60 {
        let (_, r) = burning(&mut cv, &inputs, Some(burn(1.5, 0.5))).unwrap();
        sh += r.h2;
        sc += r.co;
        sq += r.q;
    }
    let b = cv.bookkeeping();
    let (w1, g1) = mass(&cv);
    let el1 = elements(w1, &g1);
    let mut gb = [0.0; 6];
    for k in 0..6 {
        gb[k] = (b.ncg_in[k] - b.ncg_out[k]).get::<kilogram>();
    }
    let elb = elements((b.water_in - b.water_out).get::<kilogram>(), &gb);
    println!("R4 burned {sh:.4} mol H2, {sc:.4} mol CO, {sq:.4e} J; T_gas {:.2} K, H2 left {:.3} mol", cv.atmosphere_temperature().get::<kelvin>(), n_of(&cv, NonCondensable::H2));
    for (i, name) in ["H", "O", "C", "N", "Ar"].iter().enumerate() {
        let d = el1[i] - el0[i] - elb[i];
        println!("R4 {name}: {:.6e} mol, residual {d:+.3e} ({:+.3e})", el1[i], d / el1[i]);
        assert!(d.abs() < 1e-12 * el1[i], "{name}");
    }
    let e_in = b.energy_in.get::<joule>();
    // Since the move (gh:#1103) the burn's species and its heat cross the
    // volume's boundary (`atmosphere_species`, `atmosphere_heat`), so the
    // bookkeeping already holds them: no separate reaction term.
    let de = cv.total_energy().get::<joule>() - e0 - (e_in - b.energy_out.get::<joule>());
    println!("R4 energy residual {de:e} J of E_in {e_in:e} ({:+.3e})", de / e_in);
    assert!(de.abs() < 1e-10 * e_in);
    // H2 burned at the asked 1.5 mol/s throughout; CO until it ran out (the
    // vent took a little of the 20 mol).
    assert!(sh == 90.0 && sc > 19.0 && n_of(&cv, NonCondensable::CO) == 0.0, "the reactions ran: {sh}, {sc}");
}

// ── R6: the network ───────────────────────────────────────────────────────

/// R6. A one-volume network driven as `ContainmentNetwork` drives its
/// building: each step `reaction::burn_inputs` on the network's
/// start-of-step volume, then `set_atmosphere_species` and
/// `set_atmosphere_heat`. The volume burns as it would alone, and
/// `water + gases + energy = start + boundary_inflow` to round-off (the
/// species and the heat cross the boundary; `reaction_totals` is gone).
#[test]
fn r6_network_setter_and_totals() {
    let gas = [(NonCondensable::H2, 5.0), (NonCondensable::CO, 2.0), (NonCondensable::O2, 20.0), (NonCondensable::N2, 80.0)];
    let mut net = CvNetwork::new(NetworkControl::default());
    let i = net.add_volume(dry_volume(5.0, &gas, 300.0));
    assert!(net.set_atmosphere_species(7, AtmosphereSpeciesRates::none()).is_err());
    let (w0, g0, e0) = (net.water_mass().get::<kilogram>(), net.ncg_masses(), net.total_energy().get::<joule>());
    let mut alone = dry_volume(5.0, &gas, 300.0);
    let inputs = ControlVolumeInputs::closed(0);
    let mut q_total = 0.0;
    for _ in 0..8 {
        let (rates, heat_w, _, q) = reaction::burn_inputs(net.volume(i).unwrap(), 1.0, 0.25, 1.0).unwrap();
        net.set_atmosphere_species(i, rates).unwrap();
        net.set_atmosphere_heat(i, Power::new::<uom::si::power::watt>(heat_w)).unwrap();
        net.step(sec(1.0)).unwrap();
        burning(&mut alone, &inputs, Some(burn(1.0, 0.25))).unwrap();
        q_total += q;
    }
    let v = net.volume(i).unwrap();
    assert_eq!(v.atmosphere_temperature(), alone.atmosphere_temperature());
    assert_eq!(n_of(v, NonCondensable::H2), 0.0);
    let (bw, bg, be) = net.boundary_inflow();
    let dw = net.water_mass().get::<kilogram>() - w0 - bw;
    let de = net.total_energy().get::<joule>() - e0 - be;
    println!("R6 water residual {dw:e} kg; energy {de:e} J of {q_total:e} J released");
    assert!(dw.abs() < 1e-14 && de.abs() < 1e-9 * q_total);
    let g1 = net.ncg_masses();
    for k in 0..6 {
        let d = g1[k] - g0[k] - bg[k];
        assert!(d.abs() < 1e-14, "gas {k}: {d}");
    }
}
