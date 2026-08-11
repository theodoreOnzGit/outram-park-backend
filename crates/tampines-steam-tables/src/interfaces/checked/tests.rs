//! V&V tests for the bounds-checked IF97 facade (bead `op-t647`).
//!
//! # Methodology
//!
//! The facade's contract has two halves, each gated by a test group here:
//!
//! 1. **In-range agreement** — for `(T,p)` and `(p,h)` states inside the
//!    IF97 envelope, every `try_*` function must return `Ok` with a value
//!    **bit-for-bit equal** to the corresponding unchecked function
//!    (`assert_eq!` on the `uom` quantities, no tolerance): the facade
//!    calls the identical internal code path after its envelope check, so
//!    any difference at all is a wiring bug. Probe states are chosen one
//!    per IF97 region: Region 1 (300 K, 3 MPa), Region 2 (700 K,
//!    3.5 kPa), Region 3 (650 K, 25.5837 MPa), Region 5 (1500 K, 30 MPa)
//!    — the classic IAPWS-IF97 verification-table states — plus `(p,h)`
//!    probes in Region 1 (3 MPa, 500 kJ/kg), the two-phase dome
//!    (5 MPa, 2000 kJ/kg) and Region 2 (1 MPa, 3000 kJ/kg).
//! 2. **Out-of-range rejection** — inputs violating the envelope must
//!    return `Err` (never panic): T below 273.15 K, T above 2273.15 K,
//!    p above 100 MPa, a Region-5 pressure violation (T > 1073.15 K with
//!    50 MPa < p <= 100 MPa), NaN inputs, an exact saturation-line
//!    `(T,p)` pair, `(p,h)` pressure below `p_sat(273.15 K)` and at the
//!    exclusive 100 MPa edge, and `(p,h)` enthalpy outside the
//!    273.15 K / 1073.15 K isotherm window. The test running to
//!    completion (no panic) is itself part of the pass criterion.
//!
//! # Results (2026-08-11, `cargo test --release -p tampines-steam-tables
//! --lib checked -- --nocapture`, this crate at v0.2.5)
//!
//! All 4 tests pass (`4 passed; 0 failed`). Measured values printed by
//! the passing run (facade output, bit-identical to the unchecked
//! functions by `assert_eq!`):
//!
//! - Region 1 (300 K, 3 MPa): h = 115331.2730214384 J/kg,
//!   v = 0.0010021516796866945 m^3/kg — the IAPWS-IF97 Table 5 state
//!   (reference h = 115.331273 kJ/kg, v = 0.100215168e-2 m^3/kg).
//! - Region 2 (700 K, 3.5 kPa): h = 3335683.7537312238 J/kg,
//!   v = 92.30158981741968 m^3/kg — the Table 15 state
//!   (reference h = 3335.68375 kJ/kg).
//! - Region 3 (650 K, 25.5837018 MPa): v = 0.0020000083047766923 m^3/kg,
//!   i.e. rho = 499.998 kg/m^3 vs the Table 33 state's rho = 500 kg/m^3
//!   (backward-equation round-trip, within the crate's documented
//!   near-critical accuracy notes).
//! - Region 5 (1500 K, 30 MPa): h = 5167235.1400895165 J/kg — the
//!   Table 42 state (reference h = 5167.23514 kJ/kg).
//! - `(p,h)` probes: (3 MPa, 500 kJ/kg) -> T = 391.7985087624256 K,
//!   x = 0 (subcooled); (5 MPa, 2000 kJ/kg) -> T = 537.0928711863302 K
//!   (= T_sat(5 MPa)), x = 0.5156339907239657 (in-dome);
//!   (1 MPa, 3000 kJ/kg) -> T = 549.1186332714798 K, x = 1
//!   (superheated).
//! - Every out-of-range probe returned the expected `Err` variant; no
//!   probe panicked (the suite finishing is part of the pass criterion).
//!
//! Interpretation: the facade is a faithful pass-through inside the
//! envelope and converts every probed out-of-envelope input (including
//! NaN, which the unchecked internals silently accept) into a typed
//! error instead of a panic.

use super::*;
use uom::si::available_energy::kilojoule_per_kilogram;
use uom::si::pressure::{kilopascal, megapascal};
use uom::si::thermodynamic_temperature::kelvin;

/// In-range `(T,p)` agreement: every checked function returns `Ok` equal
/// bit-for-bit to its unchecked counterpart, at one probe state per
/// single-phase IF97 region (see module doc, Methodology 1 / Results).
#[test]
fn in_range_tp_agrees_exactly_with_unchecked() {
    let probes = [
        // (T, p): Region 1, 2, 3, 5 IAPWS verification-table states.
        (300.0, Pressure::new::<megapascal>(3.0)),
        (700.0, Pressure::new::<kilopascal>(3.5)),
        (650.0, Pressure::new::<megapascal>(25.5837018)),
        (1500.0, Pressure::new::<megapascal>(30.0)),
    ];
    for (t_kelvin, p) in probes {
        let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);
        assert_eq!(
            try_h_tp_eqm_single_phase(t, p).unwrap(),
            h_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_u_tp_eqm_single_phase(t, p).unwrap(),
            u_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_s_tp_eqm_single_phase(t, p).unwrap(),
            s_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_cp_tp_eqm_single_phase(t, p).unwrap(),
            cp_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_cv_tp_eqm_single_phase(t, p).unwrap(),
            cv_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_v_tp_eqm_single_phase(t, p).unwrap(),
            v_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_rho_tp_eqm_single_phase(t, p).unwrap(),
            v_tp_eqm_single_phase(t, p).recip()
        );
        assert_eq!(
            try_w_tp_eqm_single_phase(t, p).unwrap(),
            w_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_kappa_tp_eqm_single_phase(t, p).unwrap(),
            kappa_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_mu_tp_eqm_single_phase(t, p).unwrap(),
            mu_tp_eqm_single_phase(t, p)
        );
        assert_eq!(
            try_lambda_tp_eqm_single_phase(t, p).unwrap(),
            lambda_tp_eqm_single_phase(t, p)
        );
        // Print the enthalpy so the doc-comment Results numbers are
        // reproducible with -- --nocapture.
        println!(
            "T = {t_kelvin} K, p = {} Pa: h = {:?}, v = {:?}",
            p.get::<pascal>(),
            h_tp_eqm_single_phase(t, p),
            v_tp_eqm_single_phase(t, p),
        );
    }
}

/// In-range `(p,h)` agreement: every checked function returns `Ok` equal
/// bit-for-bit to its unchecked counterpart, at a Region-1, an in-dome
/// (Region 4) and a Region-2 probe (see module doc, Methodology 1 /
/// Results).
#[test]
fn in_range_ph_agrees_exactly_with_unchecked() {
    let probes = [
        // (p, h): subcooled liquid, two-phase dome, superheated vapour.
        (
            Pressure::new::<megapascal>(3.0),
            AvailableEnergy::new::<kilojoule_per_kilogram>(500.0),
        ),
        (
            Pressure::new::<megapascal>(5.0),
            AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0),
        ),
        (
            Pressure::new::<megapascal>(1.0),
            AvailableEnergy::new::<kilojoule_per_kilogram>(3000.0),
        ),
    ];
    for (p, h) in probes {
        assert_eq!(try_t_ph_eqm(p, h).unwrap(), t_ph_eqm(p, h));
        assert_eq!(try_v_ph_eqm(p, h).unwrap(), v_ph_eqm(p, h));
        assert_eq!(try_rho_ph_eqm(p, h).unwrap(), v_ph_eqm(p, h).recip());
        assert_eq!(try_u_ph_eqm(p, h).unwrap(), u_ph_eqm(p, h));
        assert_eq!(try_s_ph_eqm(p, h).unwrap(), s_ph_eqm(p, h));
        assert_eq!(try_cp_ph_eqm(p, h).unwrap(), cp_ph_eqm(p, h));
        assert_eq!(try_cv_ph_eqm(p, h).unwrap(), cv_ph_eqm(p, h));
        assert_eq!(
            try_w_ph_wood_wallis(p, h).unwrap(),
            w_ph_wood_wallis(p, h)
        );
        assert_eq!(try_kappa_ph_eqm(p, h).unwrap(), kappa_ph_eqm(p, h));
        assert_eq!(try_x_ph_flash(p, h).unwrap(), x_ph_flash(p, h));
        assert_eq!(try_mu_ph_eqm(p, h).unwrap(), mu_ph_eqm(p, h));
        assert_eq!(try_lambda_ph_eqm(p, h).unwrap(), lambda_ph_eqm(p, h));
        println!(
            "p = {} Pa, h = {} J/kg: T = {:?}, x = {}",
            p.get::<pascal>(),
            h.get::<joule_per_kilogram>(),
            t_ph_eqm(p, h),
            x_ph_flash(p, h),
        );
    }
}

/// Out-of-range `(T,p)` rejection (module doc, Methodology 2): T below
/// 273.15 K, T above 2273.15 K, p above 100 MPa, the Region-5 pressure
/// violation, the exclusive 100 MPa edge above 623.15 K, NaN in either
/// argument, and an exact saturation-line pair — all must return `Err`
/// without panicking.
#[test]
fn out_of_range_tp_returns_err_not_panic() {
    let p_1mpa = Pressure::new::<megapascal>(1.0);

    // T below the IF97 floor.
    let t_cold = ThermodynamicTemperature::new::<kelvin>(250.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_cold, p_1mpa),
        Err(SteamTablesError::OutOfRange { quantity: "temperature", .. })
    ));

    // T above the Region-5 ceiling.
    let t_hot = ThermodynamicTemperature::new::<kelvin>(2300.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_hot, p_1mpa),
        Err(SteamTablesError::OutOfRange { quantity: "temperature", .. })
    ));

    // p above 100 MPa (Region 1 band).
    let t_r1 = ThermodynamicTemperature::new::<kelvin>(500.0);
    let p_high = Pressure::new::<megapascal>(101.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_r1, p_high),
        Err(SteamTablesError::OutOfRange { quantity: "pressure", .. })
    ));

    // Region-5 pressure violation: T > 1073.15 K only valid to 50 MPa.
    let t_r5 = ThermodynamicTemperature::new::<kelvin>(1500.0);
    let p_r5_bad = Pressure::new::<megapascal>(60.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_r5, p_r5_bad),
        Err(SteamTablesError::OutOfRange { quantity: "pressure", .. })
    ));
    // ... while 50 MPa exactly is fine.
    assert!(try_h_tp_eqm_single_phase(t_r5, Pressure::new::<megapascal>(50.0)).is_ok());

    // Exclusive 100 MPa edge above 623.15 K: exactly 100 MPa has no
    // matching arm in the internal region router (would panic there).
    let t_mid = ThermodynamicTemperature::new::<kelvin>(700.0);
    let p_edge = Pressure::new::<megapascal>(100.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_mid, p_edge),
        Err(SteamTablesError::OutOfRange { quantity: "pressure", .. })
    ));
    // ... but exactly 100 MPa at/below 623.15 K (Region 1) is accepted.
    assert!(try_h_tp_eqm_single_phase(t_r1, p_edge).is_ok());

    // Non-positive pressure.
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_r1, Pressure::new::<pascal>(0.0)),
        Err(SteamTablesError::OutOfRange { quantity: "pressure", .. })
    ));

    // NaN temperature and NaN pressure.
    let t_nan = ThermodynamicTemperature::new::<kelvin>(f64::NAN);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_nan, p_1mpa),
        Err(SteamTablesError::NonFinite { quantity: "temperature", .. })
    ));
    let p_nan = Pressure::new::<pascal>(f64::NAN);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_r1, p_nan),
        Err(SteamTablesError::NonFinite { quantity: "pressure", .. })
    ));

    // Exact saturation-line (T, p_sat(T)) pair: under-determined without
    // steam quality — a dedicated error variant, not a panic.
    let t_sat = ThermodynamicTemperature::new::<kelvin>(453.15);
    let p_sat = sat_pressure_4(t_sat);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_sat, p_sat),
        Err(SteamTablesError::SaturatedTpUnderdetermined { .. })
    ));
}

/// Out-of-range `(p,h)` rejection (module doc, Methodology 2): pressure
/// below `p_sat(273.15 K)`, pressure at/above the exclusive 100 MPa
/// edge, enthalpy below the 273.15 K isotherm, enthalpy above the
/// 1073.15 K isotherm (Region 5 / beyond), and NaN in either argument —
/// all must return `Err` without panicking.
#[test]
fn out_of_range_ph_returns_err_not_panic() {
    let h_mid = AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0);

    // Pressure below p_sat(273.15 K) ~= 611.213 Pa.
    let p_low = Pressure::new::<pascal>(100.0);
    assert!(matches!(
        try_t_ph_eqm(p_low, h_mid),
        Err(SteamTablesError::OutOfRange { quantity: "pressure", .. })
    ));

    // Pressure above 100 MPa, and at the exclusive 100 MPa edge (where
    // the internal (p,h) validity check itself would panic).
    assert!(matches!(
        try_t_ph_eqm(Pressure::new::<megapascal>(101.0), h_mid),
        Err(SteamTablesError::OutOfRange { quantity: "pressure", .. })
    ));
    assert!(matches!(
        try_t_ph_eqm(Pressure::new::<megapascal>(100.0), h_mid),
        Err(SteamTablesError::OutOfRange { quantity: "pressure", .. })
    ));

    // Enthalpy below the 273.15 K isotherm at 1 MPa (~1 kJ/kg floor).
    let p_1mpa = Pressure::new::<megapascal>(1.0);
    let h_cold = AvailableEnergy::new::<kilojoule_per_kilogram>(-50.0);
    assert!(matches!(
        try_t_ph_eqm(p_1mpa, h_cold),
        Err(SteamTablesError::OutOfRange { quantity: "specific enthalpy", .. })
    ));

    // Enthalpy above the 1073.15 K isotherm (Region 5 / beyond): no
    // IAPWS-IF97 backward (p,h) correlation exists there.
    let h_hot = AvailableEnergy::new::<kilojoule_per_kilogram>(5000.0);
    assert!(matches!(
        try_t_ph_eqm(p_1mpa, h_hot),
        Err(SteamTablesError::OutOfRange { quantity: "specific enthalpy", .. })
    ));

    // NaN pressure and NaN enthalpy.
    assert!(matches!(
        try_t_ph_eqm(Pressure::new::<pascal>(f64::NAN), h_mid),
        Err(SteamTablesError::NonFinite { quantity: "pressure", .. })
    ));
    assert!(matches!(
        try_t_ph_eqm(p_1mpa, AvailableEnergy::new::<joule_per_kilogram>(f64::NAN)),
        Err(SteamTablesError::NonFinite { quantity: "specific enthalpy", .. })
    ));

    // Every checked (p,h) function shares the same guard — spot-check the
    // rest of the facade against one bad input each.
    assert!(try_v_ph_eqm(p_low, h_mid).is_err());
    assert!(try_rho_ph_eqm(p_low, h_mid).is_err());
    assert!(try_u_ph_eqm(p_low, h_mid).is_err());
    assert!(try_s_ph_eqm(p_low, h_mid).is_err());
    assert!(try_cp_ph_eqm(p_low, h_mid).is_err());
    assert!(try_cv_ph_eqm(p_low, h_mid).is_err());
    assert!(try_w_ph_wood_wallis(p_low, h_mid).is_err());
    assert!(try_kappa_ph_eqm(p_low, h_mid).is_err());
    assert!(try_x_ph_flash(p_low, h_mid).is_err());
    assert!(try_mu_ph_eqm(p_low, h_mid).is_err());
    assert!(try_lambda_ph_eqm(p_low, h_mid).is_err());
}
