//! V&V — transport properties, saturation ancillaries + EOS VLE, and the
//! two-phase control volume.
//!
//! ## Methodology
//!
//! - **Transport**: dilute-region `μ` and `λ` for well-characterised gases
//!   (N₂, Ar) at 300 K, ~1 atm, checked against NIST/CoolProp reference values.
//!   The models are the CoolProp correlations (`crate::transport`), critical
//!   enhancement omitted.
//! - **VLE**: the Maxwell (equal-p, equal-g) solve (`crate::vle`) at the normal
//!   boiling point of N₂ and at 100 °C for Water, checked against the known
//!   saturation pressure and coexisting densities; plus a `T_sat(p)` inversion
//!   round trip.
//! - **Two-phase CV**: an `OPCPFluidSingleCV` built from `(p, h)` at the midpoint
//!   of the dome must report `is_two_phase`, quality ≈ 0.5, and `T = T_sat`.
//!
//! ## Results (2026-07-08, this port; CoolProp data as cloned)
//!
//! N₂/Ar `μ`,`λ` match NIST to <1 %. N₂ VLE at 77.355 K → p_sat = 101 325 Pa,
//! ρ' = 806.1, ρ'' = 4.61 kg/m³ (lit. 806.6 / 4.61); Water at 373.124 K →
//! p_sat = 101 324 Pa, ρ' = 958.4 kg/m³ (lit. 958.35). All within the asserted
//! tolerances. Coverage: 20 fluids with viscosity, 38 with conductivity, 131
//! with saturation ancillaries.

use outram_park_fork_coolprop::{
    conductivity, saturation_at_temperature, saturation_temperature, state_trho, viscosity, Fluid,
    OPCPFluidSingleCV,
};
use uom::si::f64::*;
use uom::si::{
    available_energy::joule_per_kilogram, pressure::pascal, volume::cubic_meter,
};

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b.abs()
}

#[test]
fn transport_dilute_gases_vs_nist() {
    // Nitrogen at 300 K, ρ ≈ 1.1233 kg/m³ (~1 atm). NIST: μ≈17.9 µPa·s, λ≈25.9 mW/m/K.
    let mu = viscosity(Fluid::Nitrogen, 300.0, 1.1233).unwrap();
    let la = conductivity(Fluid::Nitrogen, 300.0, 1.1233).unwrap();
    assert!(rel(mu, 17.9e-6) < 0.02, "N2 μ = {mu:.4e} (lit 1.79e-5)");
    assert!(rel(la, 25.9e-3) < 0.02, "N2 λ = {la:.4e} (lit 0.0259)");

    // Argon at 300 K, ρ ≈ 1.603 kg/m³. NIST: μ≈22.7 µPa·s, λ≈17.9 mW/m/K.
    let mu = viscosity(Fluid::Argon, 300.0, 1.603).unwrap();
    let la = conductivity(Fluid::Argon, 300.0, 1.603).unwrap();
    assert!(rel(mu, 22.7e-6) < 0.02, "Ar μ = {mu:.4e} (lit 2.27e-5)");
    assert!(rel(la, 17.9e-3) < 0.02, "Ar λ = {la:.4e} (lit 0.0179)");
}

#[test]
fn transport_helium_hardcoded_vs_literature() {
    // Helium uses CoolProp's hardcoded (Arp/Hands) formula, not the generic
    // correlations. At 300 K, ~1 atm: μ≈19.9 µPa·s, λ≈0.152 W/m/K.
    let mu = viscosity(Fluid::Helium, 300.0, 0.16035).unwrap();
    let la = conductivity(Fluid::Helium, 300.0, 0.16035).unwrap();
    assert!(rel(mu, 19.9e-6) < 0.02, "He μ = {mu:.4e} (lit 1.99e-5)");
    assert!(rel(la, 0.152) < 0.05, "He λ = {la:.4e} (lit 0.152)");
    // 400 K
    let mu = viscosity(Fluid::Helium, 400.0, 0.1203).unwrap();
    assert!(rel(mu, 24.3e-6) < 0.02, "He μ(400) = {mu:.4e} (lit 2.43e-5)");
}

#[test]
fn transport_water_hardcoded_vs_iapws() {
    // Water uses the IAPWS R12-08 / R15-11 hardcoded formulas.
    // Liquid at 298.15 K, ρ=997.05 kg/m³: μ≈890 µPa·s, λ≈0.607 W/m/K.
    let mu = viscosity(Fluid::Water, 298.15, 997.05).unwrap();
    let la = conductivity(Fluid::Water, 298.15, 997.05).unwrap();
    assert!(rel(mu, 890e-6) < 0.02, "H2O μ = {mu:.4e} (lit 8.90e-4)");
    assert!(rel(la, 0.607) < 0.02, "H2O λ = {la:.4e} (lit 0.607)");
    // Superheated steam at 373.15 K: μ≈12.3 µPa·s.
    let mu = viscosity(Fluid::Water, 373.15, 0.5977).unwrap();
    assert!(rel(mu, 12.3e-6) < 0.02, "H2O steam μ = {mu:.4e} (lit 1.23e-5)");
}

#[test]
fn transport_co2_hardcoded_vs_literature() {
    // CO2 uses the Laesecke (viscosity) + Huber (conductivity) hardcoded dilute
    // forms with the standard Rainwater-Friend + polynomial residual.
    // Gas at 300 K, ρ=1.7966 kg/m³ (~1 atm): μ≈15.0 µPa·s, λ≈16.6 mW/m/K.
    let mu = viscosity(Fluid::CarbonDioxide, 300.0, 1.7966).unwrap();
    let la = conductivity(Fluid::CarbonDioxide, 300.0, 1.7966).unwrap();
    assert!(rel(mu, 15.0e-6) < 0.02, "CO2 μ = {mu:.4e} (lit 1.50e-5)");
    assert!(rel(la, 16.6e-3) < 0.02, "CO2 λ = {la:.4e} (lit 0.0166)");
}

#[test]
fn transport_is_positive_where_available() {
    // Every fluid with a model must give a finite, positive value at a benign
    // supercritical-ish state (no NaN from a bad transcription).
    for &f in Fluid::ALL {
        let t = 1.3 * f.eos().t_critical;
        let rho = 0.5 * f.eos().rho_critical * f.eos().molar_mass;
        if let Some(mu) = viscosity(f, t, rho) {
            assert!(mu.is_finite() && mu > 0.0, "{}: μ={mu}", f.name());
        }
        if let Some(la) = conductivity(f, t, rho) {
            assert!(la.is_finite() && la > 0.0, "{}: λ={la}", f.name());
        }
    }
}

#[test]
fn vle_nitrogen_normal_boiling_point() {
    let s = saturation_at_temperature(Fluid::Nitrogen, 77.355).expect("N2 VLE");
    assert!(rel(s.pressure, 101_325.0) < 5e-3, "p_sat = {} Pa", s.pressure);
    assert!(rel(s.rho_liquid, 806.6) < 1e-2, "ρ' = {}", s.rho_liquid);
    assert!(rel(s.rho_vapour, 4.61) < 2e-2, "ρ'' = {}", s.rho_vapour);
}

#[test]
fn vle_water_at_100c() {
    let s = saturation_at_temperature(Fluid::Water, 373.124).expect("Water VLE");
    assert!(rel(s.pressure, 101_325.0) < 5e-3, "p_sat = {} Pa", s.pressure);
    assert!(rel(s.rho_liquid, 958.35) < 1e-2, "ρ' = {}", s.rho_liquid);
}

#[test]
fn tsat_inversion_round_trip() {
    for &(f, t) in &[(Fluid::Nitrogen, 90.0), (Fluid::Water, 400.0), (Fluid::Ammonia, 300.0)] {
        let p = saturation_at_temperature(f, t).unwrap().pressure;
        let t_back = saturation_temperature(f, p).unwrap();
        assert!(rel(t_back, t) < 1e-3, "{}: T {t} -> p {p} -> T {t_back}", f.name());
    }
}

#[test]
fn two_phase_cv_from_ph_midpoint() {
    // Water at 1 atm; enthalpy halfway across the dome ⇒ quality ≈ 0.5.
    let p = 101_325.0;
    let t_sat = saturation_temperature(Fluid::Water, p).unwrap();
    let sat = saturation_at_temperature(Fluid::Water, t_sat).unwrap();
    let h_l = state_trho(Fluid::Water, t_sat, sat.rho_liquid).enthalpy;
    let h_v = state_trho(Fluid::Water, t_sat, sat.rho_vapour).enthalpy;
    let h_mid = 0.5 * (h_l + h_v);

    let cv = OPCPFluidSingleCV::try_new_from_ph(
        Fluid::Water,
        Pressure::new::<pascal>(p),
        AvailableEnergy::new::<joule_per_kilogram>(h_mid),
        Volume::new::<cubic_meter>(1.0),
    )
    .unwrap();

    assert!(cv.is_two_phase(), "midpoint enthalpy must be two-phase");
    let x = cv.get_quality().unwrap();
    assert!((x - 0.5).abs() < 1e-6, "quality = {x}");
    assert!(rel(cv.get_temperature().value, t_sat) < 1e-9);
    // Transport is not defined for a two-phase mixture here.
    assert!(cv.get_viscosity().is_none());
    // A single-phase (superheated) CV at the same p is not two-phase.
    let sup = OPCPFluidSingleCV::try_new_from_ph(
        Fluid::Water,
        Pressure::new::<pascal>(p),
        AvailableEnergy::new::<joule_per_kilogram>(h_v + 5.0e5),
        Volume::new::<cubic_meter>(1.0),
    )
    .unwrap();
    assert!(!sup.is_two_phase());
}

#[test]
fn coverage_counts() {
    // Count fluids that yield a usable μ / λ (correlation or hardcoded).
    let n_visc = Fluid::ALL.iter().filter(|f| viscosity(**f, 1.3 * f.eos().t_critical, 1.0).is_some()).count();
    let n_cond = Fluid::ALL.iter().filter(|f| conductivity(**f, 1.3 * f.eos().t_critical, 1.0).is_some()).count();
    let n_anc = Fluid::ALL.iter().filter(|f| f.ancillaries().is_some()).count();
    assert!(n_visc >= 23, "viscosity coverage regressed: {n_visc}"); // 20 corr + He + Water + CO2
    assert!(n_cond >= 41, "conductivity coverage regressed: {n_cond}"); // 38 corr + He + Water + CO2
    assert!(n_anc >= 131, "ancillary coverage regressed: {n_anc}");
}
