//! # V&V: the steam + non-condensable mixture (gh:#1093)
//!
//! Predictions written before the first run (2026-11-01):
//!
//! 1. The NASA-7 `c_v` at 298.15 K matches the JANAF/NIST values (N2
//!    20.81, ~~O2 21.00~~, CO ~~20.82~~, CO2 28.81, Ar 12.47 J/(mol K)) within 0.5 %;
//!    **CORRECTED 2026-11-01 (gh:#1098 item 3):** the values were from
//!    memory; re-checked against NIST-JANAF, O2 is 21.062 and CO 20.828
//!    (`c_p − R`), see the test's doc comment. The tolerance is unchanged;
//!    `u` integrates `c_v` (centred difference within 1e-6 relative) and is
//!    continuous at `T_mid`.
//! 2. Partial pressures sum to the total; non-condensables obey
//!    `p_i V = n_i R T` exactly.
//! 3. IF97 steam at a low partial pressure is within 0.5 % of ideal, and the
//!    IF97 partial pressure reproduces `ρ_s` to 1e-12.
//! 4. The dew point of humid N2 equals IF97 `T_sat(p_s)`; saturated steam
//!    reads relative humidity 1 within 1e-9.
//! 5. `temperature(U)` inverts `state` to 1e-9 K.
//! 6. With the ablations (`without_real_steam`, `without_ncg_cv_t`) the
//!    energy is `sembawang`'s formula exactly.

use super::*;

fn vol(v: f64) -> Volume {
    Volume::new::<cubic_meter>(v)
}
fn kel(t: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(t)
}

/// **1. NASA-7 heat capacities.** **Result (2026-11-01):** printed and
/// asserted.
///
/// **Reference values, re-checked 2026-11-01 (gh:#1098 item 3).** The first
/// version used `c_v` values recalled from memory (N2 20.81, ~~O2 21.00~~,
/// ~~CO 20.82~~, CO2 28.81, Ar 12.47 J/(mol K)). **CORRECTED 2026-11-01**:
/// they are now NIST-JANAF's ideal-gas `C°p(298.15 K)` (1 bar standard
/// state) less `R_GAS`:
///
/// | gas | JANAF table | `C°p`, J/(mol K) | `c_v = C°p − R` | old value |
/// |---|---|---|---|---|
/// | N2 | <https://janaf.nist.gov/tables/N-023.html> | 29.124 | 20.810 | 20.81 (right) |
/// | O2 | <https://janaf.nist.gov/tables/O-029.html> | 29.376 | 21.062 | ~~21.00~~ (wrong, −0.3 %) |
/// | CO | <https://janaf.nist.gov/tables/C-093.html> | 29.142 | 20.828 | ~~20.82~~ (rounded down) |
/// | CO2 | <https://janaf.nist.gov/tables/C-095.html> | 37.129 | 28.815 | 28.81 (right to its digits) |
/// | Ar | <https://janaf.nist.gov/tables/Ar-001.html> | 20.786 | 12.472 | 12.47 (right) |
///
/// Accessed 2026-11-01. **How they were read, stated plainly:** the
/// session's network blocked `janaf.nist.gov` and `webbook.nist.gov`
/// directly, so each `C°p` was read from a web-search extract of the JANAF
/// page named above (queried without the expected number for O2, CO, CO2
/// and Ar; with it for N2). A human opening the five tables once would
/// close this. The 0.5 % tolerance is unchanged.
///
/// **Result after the correction (2026-11-01, release):** held. N2
/// 20.7610 against 20.8095 (−0.233 %), O2 21.0637 against 21.0615
/// (+0.010 %), CO 20.8264 against 20.8275 (−0.006 %), CO2 28.8207 against
/// 28.8145 (+0.021 %), Ar 12.4717 against 12.4715 (+0.001 %). Against the
/// old O2 value the miss was +0.30 %; it was the reference that was off.
/// N2's −0.23 % is GRI-Mech's polynomial, unchanged by the correction.
#[test]
fn nasa7_heat_capacities_match_reference_values() {
    // NIST-JANAF C°p(298.15 K), J/(mol K), less R (see the doc comment).
    let refs = [
        (NonCondensable::N2, 29.124 - R_GAS),
        (NonCondensable::O2, 29.376 - R_GAS),
        (NonCondensable::CO, 29.142 - R_GAS),
        (NonCondensable::CO2, 37.129 - R_GAS),
        (NonCondensable::Ar, 20.786 - R_GAS),
    ];
    for (g, cv_ref) in refs {
        let cv = nasa7::cv_molar(g, 298.15);
        println!("{g:?}: c_v(298.15) = {cv:.4} J/(mol K) vs {cv_ref:.4} ({:+.3} %)", 100.0 * (cv / cv_ref - 1.0));
        assert!((cv / cv_ref - 1.0).abs() < 0.005);
    }
    for g in NonCondensable::ALL {
        for t in [350.0, 800.0, 1500.0, 2500.0] {
            let d = (nasa7::u_molar(g, t + 0.01) - nasa7::u_molar(g, t - 0.01)) / 0.02;
            assert!((d / nasa7::cv_molar(g, t) - 1.0).abs() < 1e-6, "{g:?} {t}");
        }
        let jump = nasa7::u_molar(g, 1000.0 + 1e-9) - nasa7::u_molar(g, 1000.0 - 1e-9);
        assert!(jump.abs() < 1e-3, "{g:?} jump {jump}");
    }
}

/// **2–4. Dalton, IF97 steam, dew point.** 1 m³ at 350 K: 30 mol N2,
/// 5 mol O2, 2 mol H2 and 0.1 kg of steam. **Result (2026-11-01):** printed
/// and asserted.
#[test]
fn dalton_partial_pressures_and_if97_steam() {
    let m = GasMixtureModel::new();
    let mix = GasMixture::new(
        Mass::new::<kilogram>(0.1),
        &[
            (NonCondensable::N2, AmountOfSubstance::new::<mole>(30.0)),
            (NonCondensable::O2, AmountOfSubstance::new::<mole>(5.0)),
            (NonCondensable::H2, AmountOfSubstance::new::<mole>(2.0)),
        ],
    );
    let s = m.state(&mix, vol(1.0), kel(350.0)).unwrap();
    let sum: f64 = s.ncg_partial_pressures.iter().sum::<f64>() + s.steam_partial_pressure.value;
    assert!((sum / s.pressure.value - 1.0).abs() < 1e-15);
    assert_eq!(s.ncg_partial_pressures[0], 30.0 * R_GAS * 350.0 / 1.0);
    let ideal = 0.1 / 1.0 * R_GAS / M_H2O * 350.0;
    let p_s = s.steam_partial_pressure.value;
    let rho_back = 1.0 / v_tp_2(kel(350.0), s.steam_partial_pressure).get::<cubic_meter_per_kilogram>();
    let dew = s.dew_point.unwrap().get::<kelvin>();
    let tsat = sat_temp_4(s.steam_partial_pressure).get::<kelvin>();
    println!("mixture: p {:.2} Pa, p_s {p_s:.3} Pa (ideal {ideal:.3}, {:+.4} %), RH {:.4}, dew {dew:.4} K", s.pressure.value, 100.0 * (p_s / ideal - 1.0), s.relative_humidity);
    assert!((p_s / ideal - 1.0).abs() < 0.005);
    assert!((rho_back / 0.1 - 1.0).abs() < 1e-12);
    assert_eq!(dew, tsat);
    // Saturated steam alone at 400 K: RH 1.
    let p_sat = sat_pressure_4(kel(400.0)).get::<pascal>();
    let rho_g = 1.0 / v_tp_2(kel(400.0), Pressure::new::<pascal>(p_sat)).get::<cubic_meter_per_kilogram>();
    let sat = m.state(&GasMixture { steam_kg: rho_g * 2.0, ..GasMixture::default() }, vol(2.0), kel(400.0)).unwrap();
    println!("saturated steam at 400 K: RH {:.12}", sat.relative_humidity);
    assert!((sat.relative_humidity - 1.0).abs() < 1e-9);
    assert!((mix.total_mass().value - (0.1 + 30.0 * melcor_ncg::M_N2 + 5.0 * 0.031_998 + 2.0 * melcor_ncg::M_H2)).abs() < 1e-15);
}

/// **5–6. Inverting the energy; the ablations are sembawang's formulas.**
/// **Result (2026-11-01):** printed and asserted.
#[test]
fn temperature_from_energy_and_the_ablations() {
    let mix = GasMixture::new(
        Mass::new::<kilogram>(2.0),
        &[(NonCondensable::N2, AmountOfSubstance::new::<mole>(100.0)), (NonCondensable::CO2, AmountOfSubstance::new::<mole>(10.0))],
    );
    for m in [GasMixtureModel::new(), GasMixtureModel::new().without_real_steam(), GasMixtureModel::new().without_ncg_cv_t()] {
        let u = m.state(&mix, vol(10.0), kel(420.0)).unwrap().internal_energy;
        let t = m.temperature(&mix, vol(10.0), u, kel(300.0)).unwrap().get::<kelvin>();
        println!("{m:?}: T back {t:.12} K");
        assert!((t - 420.0).abs() < 1e-9);
        let c = m.heat_capacity(&mix, vol(10.0), kel(420.0)).unwrap().value;
        let fd = (m.state(&mix, vol(10.0), kel(420.01)).unwrap().internal_energy.value - m.state(&mix, vol(10.0), kel(419.99)).unwrap().internal_energy.value) / 0.02;
        assert!((c / fd - 1.0).abs() < 1e-5, "{c} vs {fd}");
    }
    // sembawang's formulas: N2 + H2 with its constants and ideal steam.
    let m = GasMixtureModel::new().without_real_steam().without_ncg_cv_t();
    let mix2 = GasMixture::new(
        Mass::new::<kilogram>(3.0),
        &[(NonCondensable::N2, AmountOfSubstance::new::<mole>(50.0)), (NonCondensable::H2, AmountOfSubstance::new::<mole>(7.0))],
    );
    let s = m.state(&mix2, vol(5.0), kel(400.0)).unwrap();
    let sem = 50.0 * 20.8 * 400.0 + 7.0 * 20.5 * 400.0 + 3.0 * (2.5060e6 + 1500.0 * (400.0 - 373.15));
    assert!((s.internal_energy.value / sem - 1.0).abs() < 1e-15);
    assert_eq!(s.steam_partial_pressure.value, 3.0 / 5.0 * R_GAS / M_H2O * 400.0);
    // MELCOR fits in the default for N2 and H2.
    let d = GasMixtureModel::new();
    assert_eq!(d.ncg_u(NonCondensable::N2, 600.0), melcor_ncg::u_n2(600.0, true));
    assert_eq!(GasMixtureModel { heat_capacity: NcgHeatCapacity::Nasa7, ..d }.ncg_u(NonCondensable::N2, 600.0), nasa7::u_molar(NonCondensable::N2, 600.0));
    // Refusals.
    assert!(d.state(&mix, vol(0.0), kel(400.0)).is_err());
    assert!(d.state(&GasMixture { steam_kg: -1.0, ..mix }, vol(1.0), kel(400.0)).is_err());
    assert!(d.state(&mix, vol(1.0), kel(250.0)).is_err(), "IF97 steam below 273.15 K");
}

/// **Steam below the triple point is refused (gh:#1115, maintainer decision
/// 2026-11-02).** Pins the refusal and its message, through both entry
/// points: `state` at 250 K, and `temperature` from an energy below the
/// mixture's energy at 273.16 K (what a cold adiabatic blowdown asks for).
/// The ideal-steam ablation is not refused (it has no IF97 bound), which this
/// test also pins so a reader knows the refusal is the IF97 closure's.
/// **Result (2026-11-02, release):** passes.
#[test]
fn steam_below_the_triple_point_is_refused_with_an_explicit_message() {
    let mix = GasMixture::new(Mass::new::<kilogram>(0.01), &[(NonCondensable::N2, AmountOfSubstance::new::<mole>(100.0))]);
    let d = GasMixtureModel::new();
    let explicit = |e: GasMixtureError| match e {
        GasMixtureError::InvalidInput(m) => {
            println!("{m}");
            assert!(m.contains(STEAM_BELOW_TRIPLE_POINT), "{m}");
        }
        other => panic!("expected the explicit refusal, got {other:?}"),
    };
    explicit(d.state(&mix, vol(10.0), kel(250.0)).unwrap_err());
    // 0.01 kg in 10 m3 is subsaturated at 273.16 K (p_s ≈ 126 Pa < 611.7 Pa).
    let u_tp = d.state(&mix, vol(10.0), kel(273.16)).unwrap().internal_energy;
    let colder = Energy::new::<joule>(u_tp.get::<joule>() - 1000.0);
    explicit(d.temperature(&mix, vol(10.0), colder, kel(280.0)).unwrap_err());
    // Just above, it inverts as before.
    let u_warm = d.state(&mix, vol(10.0), kel(275.0)).unwrap().internal_energy;
    assert!((d.temperature(&mix, vol(10.0), u_warm, kel(300.0)).unwrap().get::<kelvin>() - 275.0).abs() < 1e-9);
    // The ideal-steam ablation has no IF97 bound and is not refused.
    assert!(d.without_real_steam().state(&mix, vol(10.0), kel(250.0)).is_ok());
}
