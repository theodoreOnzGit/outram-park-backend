//! V&V — the Chung et al. (1988) generalized corresponding-states viscosity
//! (`transport::ViscosityModel::Chung`, `transport::chung_viscosity`, bead
//! op-kbc.17), the fallback used for fluids with no fluid-specific fit.
//!
//! ## Methodology
//!
//! Cyclopentane and isopentane are the two fluids CoolProp itself assigns the
//! `Chung` viscosity type to (`TRANSPORT.viscosity.type == "Chung"` in
//! `dev/fluids/{Cyclopentane,Isopentane}.json`) — the only ones this port
//! wires it into (`dev/gen_fluid.py::build_viscosity`). Both are checked in
//! the low-density gas phase at 25 °C / ~1 atm against typical literature
//! vapour-viscosity magnitudes for light C5 hydrocarbons (order 7 µPa·s), a
//! 10% ballpark rather than a pinned reference (no NIST/CoolProp oracle is
//! available in this environment — see bead op-kbc.3). This is a sanity check
//! on the method's overall scale and sign, not a precision verification.
//!
//! ## Results (2026-07-10, this port)
//!
//! At `T = 298.15 K`, `ρ = 2 kg/m³` (dilute gas): isopentane
//! `μ = 6.98e-6 Pa·s`; cyclopentane `μ = 7.39e-6 Pa·s` — both within the
//! expected order-of-magnitude ballpark for light-hydrocarbon vapour
//! viscosity at ambient temperature.

use outram_park_fork_coolprop::{transport::viscosity, Fluid};

fn rel(a: f64, b: f64) -> f64 {
    (a - b).abs() / b
}

#[test]
fn chung_viscosity_isopentane_and_cyclopentane_gas_phase_ballpark() {
    let mu_isopentane = viscosity(Fluid::Isopentane, 298.15, 2.0)
        .expect("isopentane should have a Chung viscosity model");
    let mu_cyclopentane = viscosity(Fluid::Cyclopentane, 298.15, 2.0)
        .expect("cyclopentane should have a Chung viscosity model");
    println!("mu(isopentane, 298.15K, 2 kg/m3)   = {mu_isopentane:.4e} Pa.s");
    println!("mu(cyclopentane, 298.15K, 2 kg/m3)  = {mu_cyclopentane:.4e} Pa.s");

    assert!(
        rel(mu_isopentane, 6.9e-6) < 0.10,
        "isopentane mu = {mu_isopentane:.4e} (ballpark 6.9e-6)"
    );
    assert!(
        rel(mu_cyclopentane, 7.4e-6) < 0.10,
        "cyclopentane mu = {mu_cyclopentane:.4e} (ballpark 7.4e-6)"
    );
}

#[test]
fn chung_viscosity_increases_with_density_at_fixed_temperature() {
    // Physical sanity: viscosity should increase (not decrease or blow up)
    // as density rises from dilute gas toward liquid-like density at fixed T.
    let t = 298.15;
    let mu_dilute = viscosity(Fluid::Isopentane, t, 2.0).unwrap();
    let mu_denser = viscosity(Fluid::Isopentane, t, 50.0).unwrap();
    println!(
        "mu(isopentane, {t}K, 2 kg/m3) = {mu_dilute:.4e}; mu(..., 50 kg/m3) = {mu_denser:.4e}"
    );
    assert!(
        mu_denser > mu_dilute,
        "viscosity should increase with density at fixed T"
    );
    assert!(mu_denser.is_finite() && mu_denser > 0.0);
}
