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
//!    `(T,p)` pair, `(p,h)` pressure below `p_sat(273.15 K)`, and `(p,h)`
//!    enthalpy outside the 273.15 K / 1073.15 K isotherm window. The test
//!    running to completion (no panic) is itself part of the pass
//!    criterion. The **inclusive** 100 MPa ceiling is checked from the
//!    other side: exactly 100 MPa must be *accepted* by both families
//!    (bead `op-cv1c`; it was carved out as exclusive above 623.15 K
//!    until the region router was fixed on 2026-08-11).
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
        assert_eq!(try_w_ph_wood_wallis(p, h).unwrap(), w_ph_wood_wallis(p, h));
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
/// violation, NaN in either argument, and an exact saturation-line pair —
/// all must return `Err` without panicking. Also asserts the positive
/// case at the inclusive ceiling: exactly 100 MPa is accepted at
/// 273.15 / 500 / 623.15 / 700 / 863.15 / 1073.15 K (bead `op-cv1c`).
#[test]
fn out_of_range_tp_returns_err_not_panic() {
    let p_1mpa = Pressure::new::<megapascal>(1.0);

    // T below the IF97 floor.
    let t_cold = ThermodynamicTemperature::new::<kelvin>(250.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_cold, p_1mpa),
        Err(SteamTablesError::OutOfRange {
            quantity: "temperature",
            ..
        })
    ));

    // T above the Region-5 ceiling.
    let t_hot = ThermodynamicTemperature::new::<kelvin>(2300.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_hot, p_1mpa),
        Err(SteamTablesError::OutOfRange {
            quantity: "temperature",
            ..
        })
    ));

    // p above 100 MPa (Region 1 band).
    let t_r1 = ThermodynamicTemperature::new::<kelvin>(500.0);
    let p_high = Pressure::new::<megapascal>(101.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_r1, p_high),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));

    // Region-5 pressure violation: T > 1073.15 K only valid to 50 MPa.
    let t_r5 = ThermodynamicTemperature::new::<kelvin>(1500.0);
    let p_r5_bad = Pressure::new::<megapascal>(60.0);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_r5, p_r5_bad),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));
    // ... while 50 MPa exactly is fine.
    assert!(try_h_tp_eqm_single_phase(t_r5, Pressure::new::<megapascal>(50.0)).is_ok());

    // INCLUSIVE 100 MPa edge (bead `op-cv1c`): exactly 100 MPa is a valid
    // IF97 pressure at every temperature up to 1073.15 K, so it must be
    // ACCEPTED in the Region-1 band, above 623.15 K, and on the 1073.15 K
    // isotherm alike. It used to be rejected above 623.15 K because the
    // internal router's half-open range panicked there.
    let p_edge = Pressure::new::<megapascal>(100.0);
    for t_edge in [273.15_f64, 500.0, 623.15, 700.0, 863.15, 1073.15] {
        assert!(
            try_h_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(t_edge), p_edge)
                .is_ok(),
            "exactly 100 MPa must be accepted at T = {t_edge} K"
        );
    }
    // ... while a Region-5 temperature at 100 MPa is still rejected (its
    // ceiling is 50 MPa).
    assert!(matches!(
        try_h_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(1500.0), p_edge),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));

    // Non-positive pressure.
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_r1, Pressure::new::<pascal>(0.0)),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));

    // NaN temperature and NaN pressure.
    let t_nan = ThermodynamicTemperature::new::<kelvin>(f64::NAN);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_nan, p_1mpa),
        Err(SteamTablesError::NonFinite {
            quantity: "temperature",
            ..
        })
    ));
    let p_nan = Pressure::new::<pascal>(f64::NAN);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_r1, p_nan),
        Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            ..
        })
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
/// below `p_sat(273.15 K)`, pressure above 100 MPa, enthalpy below the
/// 273.15 K isotherm, enthalpy above the 1073.15 K isotherm (Region 5 /
/// beyond), and NaN in either argument — all must return `Err` without
/// panicking. Exactly 100 MPa must be **accepted** (inclusive ceiling,
/// bead `op-cv1c`).
#[test]
fn out_of_range_ph_returns_err_not_panic() {
    let h_mid = AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0);

    // Pressure below p_sat(273.15 K) ~= 611.213 Pa.
    let p_low = Pressure::new::<pascal>(100.0);
    assert!(matches!(
        try_t_ph_eqm(p_low, h_mid),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));

    // Pressure above 100 MPa is rejected ...
    assert!(matches!(
        try_t_ph_eqm(Pressure::new::<megapascal>(101.0), h_mid),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));
    // ... but exactly 100 MPa is now ACCEPTED (bead `op-cv1c`): the edge
    // is inclusive in IF97, and the internal (p,h) validity check no
    // longer panics there.
    assert!(try_t_ph_eqm(Pressure::new::<megapascal>(100.0), h_mid).is_ok());

    // Enthalpy below the 273.15 K isotherm at 1 MPa (~1 kJ/kg floor).
    let p_1mpa = Pressure::new::<megapascal>(1.0);
    let h_cold = AvailableEnergy::new::<kilojoule_per_kilogram>(-50.0);
    assert!(matches!(
        try_t_ph_eqm(p_1mpa, h_cold),
        Err(SteamTablesError::OutOfRange {
            quantity: "specific enthalpy",
            ..
        })
    ));

    // Enthalpy above the 1073.15 K isotherm (Region 5 / beyond): no
    // IAPWS-IF97 backward (p,h) correlation exists there.
    let h_hot = AvailableEnergy::new::<kilojoule_per_kilogram>(5000.0);
    assert!(matches!(
        try_t_ph_eqm(p_1mpa, h_hot),
        Err(SteamTablesError::OutOfRange {
            quantity: "specific enthalpy",
            ..
        })
    ));

    // NaN pressure and NaN enthalpy.
    assert!(matches!(
        try_t_ph_eqm(Pressure::new::<pascal>(f64::NAN), h_mid),
        Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_t_ph_eqm(p_1mpa, AvailableEnergy::new::<joule_per_kilogram>(f64::NAN)),
        Err(SteamTablesError::NonFinite {
            quantity: "specific enthalpy",
            ..
        })
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

// =====================================================================
//  bead op-dt3.26 — (p,s), two-phase (T,p,x), alpha_v/kappa_t, and the
//  checked control-volume constructors.
// =====================================================================
//
// # Methodology (shared by every test below)
//
// Same two-halved contract as the tests above, extended with the two
// contract clauses the new families add:
//
// 1. **In-range agreement** — `assert_eq!` (bit-for-bit, no tolerance)
//    against the unchecked function at probe states chosen one per IF97
//    region.
// 2. **Out-of-range rejection** — every envelope violation, in each
//    direction independently, must return the typed `Err` and never panic;
//    the test running to completion is itself part of the pass criterion.
// 3. **NaN rejection** — `NaN` in *each* argument separately must return
//    `SteamTablesError::NonFinite`. This clause exists because the
//    internals' range comparisons are silently `false` on `NaN`: without
//    it a `NaN` state flows through undetected (and, for the two-phase
//    family, survives the internal quality clamp to produce a silent
//    `NaN` answer).
// 4. **Exact boundary acceptance** — the envelope edges that are
//    inclusive must be *accepted*, not merely "not crashed": 100 MPa,
//    50 MPa in Region 5, 273.15 K, 1073.15 K, and steam quality exactly
//    0 and exactly 1.
//
// # Gate-completeness evidence (2026-08-11)
//
// Every gate below was additionally validated by an exhaustive sweep of
// its *accepted* set, checking that no wrapped call panics anywhere the
// gate says `Ok`. The sweep harness used `catch_unwind` as a measuring
// instrument only and was deleted afterwards — the facade itself contains
// no `catch_unwind` and never will. Measured on 2026-08-11, release mode,
// this crate at v0.2.5:
//
// | Sweep | Accepted points | Panics leaked |
// |---|---|---|
// | `(p,s)`, 11 functions x ~33 000 states | 363 608 | **0** |
// | `(p,s)` throat mass flux | 162 394 | **0** |
// | two-phase `(T,p,x)`, 11 functions | 151 808 | **0** |
// | `(T,p)` + `(p,h)` `alpha_v`/`kappa_t` | 175 429 | **0** |
// | checked `TampinesSteamTableCV` constructors | 352 381 | **0** |
//
// (`cargo test --release -p tampines-steam-tables --lib
// checked::leak_sweep_tmp -- --ignored --nocapture`, finished in 46.66 s,
// `5 passed; 0 failed`.)
//
// Total 1 205 620 accepted states, zero surviving panics. Interpretation:
// each gate's accepted set is a subset of the internals' panic-free set
// over the region sampled, which is what "the gate excludes every
// reachable panic" means operationally. It is evidence, not a proof — the
// sweeps are grids, and the `unreachable`-by-argument rows in each
// module's panic table (see e.g. [`super::ps`]) carry the rest of the
// weight.
//
// # Results (2026-08-11, `cargo test --release -p tampines-steam-tables
// --lib checked -- --nocapture`, this crate at v0.2.5)
//
// All 10 tests in this module pass (`10 passed; 0 failed`). Every value
// below was printed by the passing run and is bit-identical to the
// unchecked function's output by `assert_eq!`; nothing here is quoted from
// a reference table.
//
// `(p,s)` probes (`in_range_ps_agrees_exactly_with_unchecked`):
//
// - (3 MPa, 1.5 kJ/(kg K)) -> T = 390.8152451294368 K, x = 0 (subcooled
//   liquid), h = 495 862.31675088144 J/kg, G_throat =
//   1 448 202.2993261286 kg/(m^2 s).
// - (5 MPa, 4.5 kJ/(kg K)) -> T = 537.0928711863302 K (= T_sat(5 MPa),
//   matching the (p,h) dome probe above to all printed digits),
//   x = 0.5172865101319891, h = 2 002 709.6774255047 J/kg, G_throat =
//   15 055.655960795446 kg/(m^2 s).
// - (1 MPa, 7.0 kJ/(kg K)) -> T = 540.948199770606 K, x = 1
//   (superheated), h = 2 982 256.6850884897 J/kg, G_throat =
//   2 318.020911986677 kg/(m^2 s).
//
// Two-phase `(T,p,x)` probes
// (`in_range_tpx_agrees_exactly_with_unchecked`), on the saturation line
// at T = 453.15 K where p_sat = 1 002 634.5688120957 Pa — the state the
// single-phase gate must reject as under-determined:
//
// - x = 0.5 -> h = 1 770 203.7044324419 J/kg, v = 0.09749449707326513
//   m^3/kg (mixture, mid-dome).
// - x = 0   -> h =   763 187.9981829452 J/kg, v = 0.0011273889575307014
//   m^3/kg (saturated liquid / bubble point).
//
// `alpha_v` / `kappa_T` on the `(T,p)` gate
// (`alpha_v_and_kappa_t_are_gated`), one probe per single-phase region:
//
// - Region 1 (300 K, 3 MPa): alpha_v = 2.7735453342661365e-4 1/K,
//   kappa_T = 4.4638212280219354e-10 1/Pa.
// - Region 2 (700 K, 3.5 kPa): alpha_v = 1.428787358438332e-3 1/K,
//   kappa_T = 2.857254611712557e-4 1/Pa.
// - Region 3 (650 K, 25.5837018 MPa): alpha_v = 1.6865853326945377e-2
//   1/K, kappa_T = 3.45521057008391e-8 1/Pa — both an order of magnitude
//   larger than their Region-1 values, as expected approaching the
//   critical point.
// - Region 5 (1500 K, 30 MPa): alpha_v = 7.169507536003152e-4 1/K,
//   kappa_T = 3.328812532599134e-8 1/Pa.
//
// `alpha_v` / `kappa_T` on the `(p,h)` gate:
//
// - (3 MPa, 500 kJ/kg): alpha_v = 8.441406338032403e-4 1/K, kappa_T =
//   5.21869978400271e-10 1/Pa.
// - (5 MPa, 2000 kJ/kg, in-dome): alpha_v = 3.5810254734986863e-3 1/K,
//   kappa_T = 1.412479192599842e-7 1/Pa.
// - (1 MPa, 3000 kJ/kg): alpha_v = 2.0484941357037114e-3 1/K, kappa_T =
//   1.0316037560089962e-6 1/Pa.
//
// Control-volume constructors (`checked_control_volume_constructors`):
// every checked constructor returns a `TampinesSteamTableCV` comparing
// equal (derived `PartialEq`, i.e. all six fields) to the unchecked one.
// The saturation-line round trip T -> p_sat(T) -> T_sat(p_sat) closes to
// 453.1500000000005 K from 453.15 K, i.e. 5e-13 K.
//
// Every out-of-range, NaN and boundary probe behaved as specified: each
// violated bound returned its own typed `Err` variant, each `NaN`
// argument returned `NonFinite` naming that argument, and every inclusive
// edge (100 MPa, 50 MPa in Region 5, 273.15 K, 623.15 K, 1073.15 K,
// 2273.15 K, x = 0, x = 1) was accepted. Two asymmetries are asserted
// explicitly because they are easy to get wrong:
//
// - `p = p_sat(273.15 K)` exactly is **accepted** by both the `(p,h)` and
//   the `(p,s)` gate. The `(p,s)` gate rejected it until bead `op-znjx`
//   (2026-08-11) made the internal `(p,s)` validity check evaluate its
//   lower entropy bound with the Region-1 forward equation `s_tp_1`, the
//   way the `(p,h)` check already used `h_tp_1`; the facade's exclusive
//   pressure floor was relaxed to inclusive in the same change. See
//   `ps_flash_accepts_exact_triple_point_isobar`.
// - `p = 100 MPa` exactly is accepted by `try_t_ps_eqm` but rejected by
//   `try_mass_flux_ps_eqm_throat`, whose finite-difference step would
//   walk over the ceiling.
//
// Interpretation: the new families are faithful pass-throughs inside their
// envelopes and convert every probed out-of-envelope input into a typed
// error instead of a panic — including inputs the unchecked internals
// would not merely mishandle but silently accept (`NaN`, and a steam
// quality outside `[0,1]`, which they clamp).

use crate::interfaces::functional_programming::ph_flash_eqm::{alpha_v_ph_eqm, kappa_t_ph_eqm};
use crate::interfaces::functional_programming::ps_flash_eqm::{
    alpha_v_ps_eqm, cp_ps_eqm, cv_ps_eqm, h_ps_eqm, kappa_ps_eqm, kappa_t_ps_eqm,
    mass_flux_ps_eqm_throat, t_ps_eqm, v_ps_eqm, w_ps_wood_wallis, x_ps_flash,
};
use crate::interfaces::functional_programming::pt_flash_eqm::multiphase_flashing::{
    alpha_v_tp_eqm_two_phase, cp_tp_eqm_two_phase, cv_tp_eqm_two_phase, h_tp_eqm_two_phase,
    kappa_t_tp_eqm as kappa_t_tp_eqm_two_phase, kappa_tp_eqm_two_phase, s_tp_eqm_two_phase,
    u_tp_eqm_two_phase, v_tp_eqm_two_phase, w_tp_eqm_two_phase,
};
use crate::interfaces::functional_programming::pt_flash_eqm::{
    alpha_v_tp_eqm_single_phase, kappa_t_tp_eqm,
};
use crate::interfaces::object_oriented_programming::TampinesSteamTableCV;
use crate::region_4_vap_liq_equilibrium::sat_temp_4;
use uom::si::specific_heat_capacity::kilojoule_per_kilogram_kelvin;
use uom::si::volume::cubic_meter;

/// `p_sat(273.15 K)` in Pa, computed exactly as the internals do.
fn p_triple() -> Pressure {
    sat_pressure_4(ThermodynamicTemperature::new::<kelvin>(273.15))
}

/// In-range `(p,s)` agreement (Methodology 1): every checked `(p,s)`
/// function returns `Ok` equal bit-for-bit to its unchecked counterpart,
/// at a Region-1, an in-dome (Region 4) and a Region-2 probe.
#[test]
fn in_range_ps_agrees_exactly_with_unchecked() {
    let probes = [
        // (p, s): subcooled liquid, two-phase dome, superheated vapour.
        (
            Pressure::new::<megapascal>(3.0),
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(1.5),
        ),
        (
            Pressure::new::<megapascal>(5.0),
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(4.5),
        ),
        (
            Pressure::new::<megapascal>(1.0),
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(7.0),
        ),
    ];
    for (p, s) in probes {
        assert_eq!(try_t_ps_eqm(p, s).unwrap(), t_ps_eqm(p, s));
        assert_eq!(try_v_ps_eqm(p, s).unwrap(), v_ps_eqm(p, s));
        assert_eq!(try_rho_ps_eqm(p, s).unwrap(), v_ps_eqm(p, s).recip());
        assert_eq!(try_h_ps_eqm(p, s).unwrap(), h_ps_eqm(p, s));
        assert_eq!(try_x_ps_flash(p, s).unwrap(), x_ps_flash(p, s));
        assert_eq!(try_cp_ps_eqm(p, s).unwrap(), cp_ps_eqm(p, s));
        assert_eq!(try_cv_ps_eqm(p, s).unwrap(), cv_ps_eqm(p, s));
        assert_eq!(try_w_ps_wood_wallis(p, s).unwrap(), w_ps_wood_wallis(p, s));
        assert_eq!(try_kappa_ps_eqm(p, s).unwrap(), kappa_ps_eqm(p, s));
        assert_eq!(try_alpha_v_ps_eqm(p, s).unwrap(), alpha_v_ps_eqm(p, s));
        assert_eq!(try_kappa_t_ps_eqm(p, s).unwrap(), kappa_t_ps_eqm(p, s));
        assert_eq!(
            try_mass_flux_ps_eqm_throat(p, s).unwrap(),
            mass_flux_ps_eqm_throat(p, s)
        );
        println!(
            "p = {} Pa, s = {} J/(kg K): T = {:?}, x = {}, h = {:?}, G = {:?}",
            p.get::<pascal>(),
            s.get::<kilojoule_per_kilogram_kelvin>() * 1000.0,
            t_ps_eqm(p, s),
            x_ps_flash(p, s),
            h_ps_eqm(p, s),
            mass_flux_ps_eqm_throat(p, s),
        );
    }
}

/// Out-of-range `(p,s)` rejection (Methodology 2/3/4): pressure below and
/// **at** `p_sat(273.15 K)`, pressure above 100 MPa, entropy below the
/// 273.15 K isotherm, entropy above the 1073.15 K isotherm, and NaN in
/// each argument — all must return `Err` without panicking. Exactly
/// 100 MPa must be **accepted**.
#[test]
fn out_of_range_ps_returns_err_not_panic() {
    let s_mid = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(4.0);
    let p_1mpa = Pressure::new::<megapascal>(1.0);

    // Pressure strictly below p_sat(273.15 K).
    assert!(matches!(
        try_t_ps_eqm(Pressure::new::<pascal>(100.0), s_mid),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));

    // Pressure EXACTLY p_sat(273.15 K): ACCEPTED, exactly as in the (p,h)
    // family. This was rejected until bead `op-znjx` (2026-08-11), because
    // the (p,s) validity check evaluated s_tp_eqm_single_phase on the
    // 273.15 K isotherm and the (T,p) router sends that pressure to
    // Region 4; the check now uses the Region-1 forward equation s_tp_1.
    // See `ps_flash_accepts_exact_triple_point_isobar` below for the
    // dedicated regression test.
    assert!(try_t_ps_eqm(p_triple(), s_mid).is_ok());
    // ... and so is one ULP above it, as it always was.
    let p_just_above =
        Pressure::new::<pascal>(f64::from_bits(p_triple().get::<pascal>().to_bits() + 1));
    assert!(try_t_ps_eqm(p_just_above, s_mid).is_ok());

    // Pressure above 100 MPa rejected; exactly 100 MPa accepted.
    assert!(matches!(
        try_t_ps_eqm(Pressure::new::<megapascal>(101.0), s_mid),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));
    let p_100 = Pressure::new::<megapascal>(100.0);
    assert!(try_t_ps_eqm(p_100, s_mid).is_ok());

    // Entropy below the 273.15 K isotherm and above the 1073.15 K one.
    assert!(matches!(
        try_t_ps_eqm(
            p_1mpa,
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(-5.0)
        ),
        Err(SteamTablesError::OutOfRange {
            quantity: "specific entropy",
            ..
        })
    ));
    assert!(matches!(
        try_t_ps_eqm(
            p_1mpa,
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(20.0)
        ),
        Err(SteamTablesError::OutOfRange {
            quantity: "specific entropy",
            ..
        })
    ));

    // NaN in each argument.
    assert!(matches!(
        try_t_ps_eqm(Pressure::new::<pascal>(f64::NAN), s_mid),
        Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_t_ps_eqm(
            p_1mpa,
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(f64::NAN)
        ),
        Err(SteamTablesError::NonFinite {
            quantity: "specific entropy",
            ..
        })
    ));

    // Every checked (p,s) function shares the guard.
    let p_bad = Pressure::new::<pascal>(100.0);
    assert!(try_v_ps_eqm(p_bad, s_mid).is_err());
    assert!(try_rho_ps_eqm(p_bad, s_mid).is_err());
    assert!(try_h_ps_eqm(p_bad, s_mid).is_err());
    assert!(try_x_ps_flash(p_bad, s_mid).is_err());
    assert!(try_cp_ps_eqm(p_bad, s_mid).is_err());
    assert!(try_cv_ps_eqm(p_bad, s_mid).is_err());
    assert!(try_w_ps_wood_wallis(p_bad, s_mid).is_err());
    assert!(try_kappa_ps_eqm(p_bad, s_mid).is_err());
    assert!(try_alpha_v_ps_eqm(p_bad, s_mid).is_err());
    assert!(try_kappa_t_ps_eqm(p_bad, s_mid).is_err());
    assert!(try_mass_flux_ps_eqm_throat(p_bad, s_mid).is_err());

    // The throat mass flux carries the STRICTER gate: exactly 100 MPa is
    // accepted by check_ps_envelope but rejected here, because the
    // internal finite-difference step p + p*1e-5 walks over the ceiling.
    assert!(try_t_ps_eqm(p_100, s_mid).is_ok());
    assert!(matches!(
        try_mass_flux_ps_eqm_throat(p_100, s_mid),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));
}

/// Regression test for bead `op-znjx`: the `(p,s)` flash must accept the
/// **exact** triple-point isobar `p = p_sat(273.15 K)`, matching the
/// `(p,h)` sibling.
///
/// # Methodology
///
/// `ps_flash_eqm::validity_range::is_below_isotherm_t_273_15` used to
/// evaluate its lower entropy bound with `s_tp_eqm_single_phase(273.15 K,
/// p)`. At `p == p_sat(273.15 K)` bit-for-bit the `(T,p)` region router
/// matches its `pres == p_sat_reg4_pascal` arm, returns `Region4`, and the
/// Region-4 `(T,p)` entropy arm panics ("two-phase (T,p) state
/// (IAPWS-IF97 Region 4) is under-determined without steam quality x") —
/// so every `(p,s)` flash on that isobar panicked, while the same call one
/// ULP higher succeeded. The fix evaluates the bound with the Region-1
/// forward equation `s_tp_1` instead, exactly as the `(p,h)` check already
/// does with `h_tp_1`.
///
/// The check performed here, at `s = 4.0 kJ/(kg K)` (an in-dome state on
/// that isobar):
///
/// 1. the **unchecked** `t_ps_eqm(p_sat(273.15 K), s)` returns without
///    panicking (the test running to completion is part of the pass
///    criterion);
/// 2. the temperature it returns agrees with the value at the pressure one
///    ULP above — the neighbouring pressure that already worked before the
///    fix — to within 1e-9 K. One ULP of a 611 Pa `f64` is about
///    1.1e-13 Pa, so the two states are physically the same point and any
///    disagreement beyond round-off would mean the two pressures take
///    different region branches;
/// 3. the temperature is physically sane: `T_sat(p_sat(273.15 K)) =
///    273.15 K` to within 1e-6 K, since an in-dome `(p,s)` state flashes
///    to the saturation temperature of its pressure;
/// 4. the **checked** facade `try_t_ps_eqm` now returns `Ok` at that same
///    pressure, where it previously returned `OutOfRange` from the
///    exclusive pressure floor that worked around this defect.
///
/// # Results (measured 2026-08-11, `cargo test --release -p
/// tampines-steam-tables --lib checked`, crate v0.2.5)
///
/// - `p_sat(273.15 K) = 611.2126774443449 Pa`; one ULP above is
///   `611.212677444345 Pa`.
/// - `t_ps_eqm(p_sat(273.15 K), 4.0 kJ/(kg K)) = 273.15000000000003 K`
///   (before the fix: panic, see Methodology).
/// - `t_ps_eqm(p_one_ulp_above, 4.0 kJ/(kg K)) = 273.15000000000003 K` —
///   difference `0 K`, well inside the 1e-9 K tolerance.
/// - Deviation from `T_sat` at that pressure: `2.8e-14 K`, inside the
///   1e-6 K tolerance.
/// - `try_t_ps_eqm(p_sat(273.15 K), 4.0 kJ/(kg K))` returned `Ok`
///   (before the relaxation of the facade's floor: `Err(OutOfRange)`).
///
/// Interpretation: the triple-point isobar is a valid IF97 state and the
/// `(p,s)` family now resolves it identically to its immediate neighbour,
/// with no discontinuity at the saturation-pressure boundary. The `(p,s)`
/// and `(p,h)` validity checks are now symmetric.
#[test]
fn ps_flash_accepts_exact_triple_point_isobar() {
    let s_mid = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(4.0);
    let p_exact = p_triple();
    let p_just_above =
        Pressure::new::<pascal>(f64::from_bits(p_exact.get::<pascal>().to_bits() + 1));

    // (1) The unchecked flash no longer panics on the exact isobar.
    let t_exact = t_ps_eqm(p_exact, s_mid);
    let t_just_above = t_ps_eqm(p_just_above, s_mid);

    println!(
        "op-znjx: p_exact = {} Pa -> T = {} K; p_one_ulp_above = {} Pa -> T = {} K",
        p_exact.get::<pascal>(),
        t_exact.get::<kelvin>(),
        p_just_above.get::<pascal>(),
        t_just_above.get::<kelvin>(),
    );

    // (2) Agreement with the neighbouring pressure that always worked.
    let dt = (t_exact.get::<kelvin>() - t_just_above.get::<kelvin>()).abs();
    assert!(
        dt < 1e-9,
        "T at p_sat(273.15 K) ({} K) disagrees with T one ULP above ({} K) by {} K",
        t_exact.get::<kelvin>(),
        t_just_above.get::<kelvin>(),
        dt,
    );

    // (3) Physically sane: an in-dome (p,s) state flashes to T_sat(p).
    let t_sat = sat_temp_4(p_exact);
    let dt_sat = (t_exact.get::<kelvin>() - t_sat.get::<kelvin>()).abs();
    println!(
        "op-znjx: T_sat(p_exact) = {} K, |T - T_sat| = {} K",
        t_sat.get::<kelvin>(),
        dt_sat,
    );
    assert!(
        dt_sat < 1e-6,
        "T at p_sat(273.15 K) ({} K) is not the saturation temperature ({} K)",
        t_exact.get::<kelvin>(),
        t_sat.get::<kelvin>(),
    );

    // (4) The checked facade accepts the same point now that its floor is
    // inclusive.
    assert_eq!(try_t_ps_eqm(p_exact, s_mid).unwrap(), t_exact);
}

/// In-range two-phase `(T,p,x)` agreement (Methodology 1): every checked
/// two-phase function returns `Ok` equal bit-for-bit to its unchecked
/// counterpart, at an on-saturation-line probe (which the single-phase
/// gate must reject) and at single-phase Region-1/2/5 probes (where `x` is
/// ignored by the underlying equations).
#[test]
fn in_range_tpx_agrees_exactly_with_unchecked() {
    let t_sat = ThermodynamicTemperature::new::<kelvin>(453.15);
    let probes = [
        // On the saturation line, mid-dome: the point of this family.
        (t_sat, sat_pressure_4(t_sat), 0.5),
        // Bubble and dew point exactly.
        (t_sat, sat_pressure_4(t_sat), 0.0),
        (t_sat, sat_pressure_4(t_sat), 1.0),
        // Off the saturation line: Region 1, Region 2, Region 5.
        (
            ThermodynamicTemperature::new::<kelvin>(300.0),
            Pressure::new::<megapascal>(3.0),
            0.0,
        ),
        (
            ThermodynamicTemperature::new::<kelvin>(700.0),
            Pressure::new::<kilopascal>(3.5),
            1.0,
        ),
        (
            ThermodynamicTemperature::new::<kelvin>(1500.0),
            Pressure::new::<megapascal>(30.0),
            0.5,
        ),
    ];
    for (t, p, x) in probes {
        assert_eq!(
            try_h_tp_eqm_two_phase(t, p, x).unwrap(),
            h_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_u_tp_eqm_two_phase(t, p, x).unwrap(),
            u_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_s_tp_eqm_two_phase(t, p, x).unwrap(),
            s_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_cp_tp_eqm_two_phase(t, p, x).unwrap(),
            cp_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_cv_tp_eqm_two_phase(t, p, x).unwrap(),
            cv_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_v_tp_eqm_two_phase(t, p, x).unwrap(),
            v_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_rho_tp_eqm_two_phase(t, p, x).unwrap(),
            v_tp_eqm_two_phase(t, p, x).recip()
        );
        assert_eq!(
            try_w_tp_eqm_two_phase(t, p, x).unwrap(),
            w_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_kappa_tp_eqm_two_phase(t, p, x).unwrap(),
            kappa_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_alpha_v_tp_eqm_two_phase(t, p, x).unwrap(),
            alpha_v_tp_eqm_two_phase(t, p, x)
        );
        assert_eq!(
            try_kappa_t_tp_eqm_two_phase(t, p, x).unwrap(),
            kappa_t_tp_eqm_two_phase(t, p, x)
        );
        println!(
            "T = {} K, p = {} Pa, x = {}: h = {:?}, v = {:?}",
            t.get::<kelvin>(),
            p.get::<pascal>(),
            x,
            h_tp_eqm_two_phase(t, p, x),
            v_tp_eqm_two_phase(t, p, x),
        );
    }
}

/// Out-of-range two-phase `(T,p,x)` rejection (Methodology 2/3/4): T below
/// 273.15 K, T above 2273.15 K, p above the band ceiling, non-positive p,
/// quality below 0 and above 1, and NaN in each of the three arguments.
/// Also asserts the positive cases at every inclusive edge: 273.15 K,
/// 1073.15 K, 2273.15 K, 100 MPa, 50 MPa in Region 5, and `x` exactly 0
/// and exactly 1.
#[test]
fn out_of_range_tpx_returns_err_not_panic() {
    let p_1mpa = Pressure::new::<megapascal>(1.0);
    let t_r1 = ThermodynamicTemperature::new::<kelvin>(500.0);

    // Temperature out of range, both directions.
    assert!(matches!(
        try_h_tp_eqm_two_phase(ThermodynamicTemperature::new::<kelvin>(250.0), p_1mpa, 0.5),
        Err(SteamTablesError::OutOfRange {
            quantity: "temperature",
            ..
        })
    ));
    assert!(matches!(
        try_h_tp_eqm_two_phase(ThermodynamicTemperature::new::<kelvin>(2300.0), p_1mpa, 0.5),
        Err(SteamTablesError::OutOfRange {
            quantity: "temperature",
            ..
        })
    ));

    // Pressure out of range, both directions (and the Region-5 ceiling).
    assert!(matches!(
        try_h_tp_eqm_two_phase(t_r1, Pressure::new::<megapascal>(101.0), 0.5),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_h_tp_eqm_two_phase(t_r1, Pressure::new::<pascal>(0.0), 0.5),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_h_tp_eqm_two_phase(
            ThermodynamicTemperature::new::<kelvin>(1500.0),
            Pressure::new::<megapascal>(60.0),
            0.5
        ),
        Err(SteamTablesError::OutOfRange {
            quantity: "pressure",
            ..
        })
    ));

    // Quality out of range, both directions — the internals would CLAMP.
    assert!(matches!(
        try_h_tp_eqm_two_phase(t_r1, p_1mpa, -0.01),
        Err(SteamTablesError::QualityOutOfRange { .. })
    ));
    assert!(matches!(
        try_h_tp_eqm_two_phase(t_r1, p_1mpa, 1.01),
        Err(SteamTablesError::QualityOutOfRange { .. })
    ));

    // NaN in each of the three arguments.
    assert!(matches!(
        try_h_tp_eqm_two_phase(
            ThermodynamicTemperature::new::<kelvin>(f64::NAN),
            p_1mpa,
            0.5
        ),
        Err(SteamTablesError::NonFinite {
            quantity: "temperature",
            ..
        })
    ));
    assert!(matches!(
        try_h_tp_eqm_two_phase(t_r1, Pressure::new::<pascal>(f64::NAN), 0.5),
        Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_h_tp_eqm_two_phase(t_r1, p_1mpa, f64::NAN),
        Err(SteamTablesError::NonFinite {
            quantity: "steam quality",
            ..
        })
    ));

    // Inclusive edges must be ACCEPTED.
    let p_100 = Pressure::new::<megapascal>(100.0);
    for t_edge in [273.15_f64, 623.15, 1073.15] {
        let t = ThermodynamicTemperature::new::<kelvin>(t_edge);
        assert!(
            try_h_tp_eqm_two_phase(t, p_100, 0.0).is_ok(),
            "100 MPa at T = {t_edge} K, x = 0 must be accepted"
        );
        assert!(
            try_h_tp_eqm_two_phase(t, p_100, 1.0).is_ok(),
            "100 MPa at T = {t_edge} K, x = 1 must be accepted"
        );
    }
    let t_r5_top = ThermodynamicTemperature::new::<kelvin>(2273.15);
    assert!(try_h_tp_eqm_two_phase(t_r5_top, Pressure::new::<megapascal>(50.0), 0.5).is_ok());

    // The saturation line, which the SINGLE-PHASE gate must reject, is
    // exactly what this family accepts.
    let t_sat = ThermodynamicTemperature::new::<kelvin>(453.15);
    let p_sat = sat_pressure_4(t_sat);
    assert!(matches!(
        try_h_tp_eqm_single_phase(t_sat, p_sat),
        Err(SteamTablesError::SaturatedTpUnderdetermined { .. })
    ));
    assert!(try_h_tp_eqm_two_phase(t_sat, p_sat, 0.5).is_ok());

    // Every checked two-phase function shares the guard.
    let bad_t = ThermodynamicTemperature::new::<kelvin>(250.0);
    assert!(try_u_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_s_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_cp_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_cv_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_v_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_rho_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_w_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_kappa_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_alpha_v_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
    assert!(try_kappa_t_tp_eqm_two_phase(bad_t, p_1mpa, 0.5).is_err());
}

/// `alpha_v` / `kappa_T` on the pre-existing `(T,p)` and `(p,h)` gates:
/// in-range agreement, out-of-range rejection in each direction, NaN
/// rejection in each argument, and acceptance at the inclusive 100 MPa /
/// 273.15 K / 1073.15 K edges.
#[test]
fn alpha_v_and_kappa_t_are_gated() {
    // (T,p) in-range agreement, one probe per single-phase region.
    let tp_probes = [
        (300.0, Pressure::new::<megapascal>(3.0)),
        (700.0, Pressure::new::<kilopascal>(3.5)),
        (650.0, Pressure::new::<megapascal>(25.5837018)),
        (1500.0, Pressure::new::<megapascal>(30.0)),
    ];
    for (t_kelvin, p) in tp_probes {
        let t = ThermodynamicTemperature::new::<kelvin>(t_kelvin);
        assert_eq!(
            try_alpha_v_tp_eqm_single_phase(t, p).unwrap(),
            alpha_v_tp_eqm_single_phase(t, p)
        );
        assert_eq!(try_kappa_t_tp_eqm(t, p).unwrap(), kappa_t_tp_eqm(t, p));
        println!(
            "T = {t_kelvin} K, p = {} Pa: alpha_v = {:?}, kappa_T = {:?}",
            p.get::<pascal>(),
            alpha_v_tp_eqm_single_phase(t, p),
            kappa_t_tp_eqm(t, p),
        );
    }

    // (p,h) in-range agreement: Region 1, dome, Region 2.
    let ph_probes = [
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
    for (p, h) in ph_probes {
        assert_eq!(try_alpha_v_ph_eqm(p, h).unwrap(), alpha_v_ph_eqm(p, h));
        assert_eq!(try_kappa_t_ph_eqm(p, h).unwrap(), kappa_t_ph_eqm(p, h));
        println!(
            "p = {} Pa, h = {} J/kg: alpha_v = {:?}, kappa_T = {:?}",
            p.get::<pascal>(),
            h.get::<joule_per_kilogram>(),
            alpha_v_ph_eqm(p, h),
            kappa_t_ph_eqm(p, h),
        );
    }

    // Out-of-range, each direction.
    let p_1mpa = Pressure::new::<megapascal>(1.0);
    let t_r1 = ThermodynamicTemperature::new::<kelvin>(500.0);
    assert!(try_alpha_v_tp_eqm_single_phase(
        ThermodynamicTemperature::new::<kelvin>(250.0),
        p_1mpa
    )
    .is_err());
    assert!(try_alpha_v_tp_eqm_single_phase(
        ThermodynamicTemperature::new::<kelvin>(2300.0),
        p_1mpa
    )
    .is_err());
    assert!(try_kappa_t_tp_eqm(t_r1, Pressure::new::<megapascal>(101.0)).is_err());
    assert!(try_kappa_t_tp_eqm(t_r1, Pressure::new::<pascal>(0.0)).is_err());
    assert!(try_alpha_v_ph_eqm(
        Pressure::new::<pascal>(100.0),
        AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0)
    )
    .is_err());
    assert!(try_kappa_t_ph_eqm(
        Pressure::new::<megapascal>(101.0),
        AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0)
    )
    .is_err());
    assert!(try_alpha_v_ph_eqm(
        p_1mpa,
        AvailableEnergy::new::<kilojoule_per_kilogram>(-50.0)
    )
    .is_err());
    assert!(try_alpha_v_ph_eqm(
        p_1mpa,
        AvailableEnergy::new::<kilojoule_per_kilogram>(5000.0)
    )
    .is_err());

    // Exact saturation-line (T,p): under-determined for the single-phase
    // properties, so a typed error rather than a panic.
    let t_sat = ThermodynamicTemperature::new::<kelvin>(453.15);
    assert!(matches!(
        try_alpha_v_tp_eqm_single_phase(t_sat, sat_pressure_4(t_sat)),
        Err(SteamTablesError::SaturatedTpUnderdetermined { .. })
    ));

    // NaN, each argument.
    assert!(matches!(
        try_alpha_v_tp_eqm_single_phase(ThermodynamicTemperature::new::<kelvin>(f64::NAN), p_1mpa),
        Err(SteamTablesError::NonFinite {
            quantity: "temperature",
            ..
        })
    ));
    assert!(matches!(
        try_kappa_t_tp_eqm(t_r1, Pressure::new::<pascal>(f64::NAN)),
        Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_alpha_v_ph_eqm(
            Pressure::new::<pascal>(f64::NAN),
            AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0)
        ),
        Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_kappa_t_ph_eqm(p_1mpa, AvailableEnergy::new::<joule_per_kilogram>(f64::NAN)),
        Err(SteamTablesError::NonFinite {
            quantity: "specific enthalpy",
            ..
        })
    ));

    // Inclusive edges accepted.
    let p_100 = Pressure::new::<megapascal>(100.0);
    for t_edge in [273.15_f64, 623.15, 1073.15] {
        let t = ThermodynamicTemperature::new::<kelvin>(t_edge);
        assert!(try_alpha_v_tp_eqm_single_phase(t, p_100).is_ok());
        assert!(try_kappa_t_tp_eqm(t, p_100).is_ok());
    }
    assert!(try_alpha_v_ph_eqm(
        p_100,
        AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0)
    )
    .is_ok());
}

/// Checked `TampinesSteamTableCV` constructors: in-range agreement with
/// the unchecked constructors (the struct is `PartialEq`, so equality is
/// checked on the whole control volume), out-of-range rejection in each
/// direction, NaN rejection in each argument, and acceptance at the
/// inclusive edges including `x = 0` and `x = 1`.
#[test]
fn checked_control_volume_constructors() {
    let vol = Volume::new::<cubic_meter>(1.0);
    let t = ThermodynamicTemperature::new::<kelvin>(500.0);
    let p = Pressure::new::<megapascal>(3.0);

    // In-range agreement: same control volume as the unchecked ctor.
    assert_eq!(
        try_new_from_tp_quality(t, p, vol, 0.0).unwrap(),
        TampinesSteamTableCV::new_from_tp_quality(t, p, vol, 0.0)
    );
    assert_eq!(
        try_new_from_tp_quality_0(t, p, vol).unwrap(),
        TampinesSteamTableCV::new_from_tp_quality_0(t, p, vol)
    );
    assert_eq!(
        try_new_from_tp_quality_1(t, p, vol).unwrap(),
        TampinesSteamTableCV::new_from_tp_quality_1(t, p, vol)
    );
    let h = AvailableEnergy::new::<kilojoule_per_kilogram>(2000.0);
    assert_eq!(
        try_new_from_ph(Pressure::new::<megapascal>(5.0), h, vol).unwrap(),
        TampinesSteamTableCV::new_from_ph(Pressure::new::<megapascal>(5.0), h, vol)
    );
    let s = SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(4.5);
    assert_eq!(
        try_new_from_ps(Pressure::new::<megapascal>(5.0), s, vol).unwrap(),
        TampinesSteamTableCV::new_from_ps(Pressure::new::<megapascal>(5.0), s, vol)
    );
    let t_sat = ThermodynamicTemperature::new::<kelvin>(453.15);
    assert_eq!(
        try_new_from_sat_temp_quality(t_sat, 0.5, vol).unwrap(),
        TampinesSteamTableCV::new_from_sat_temp_quality(t_sat, 0.5, vol)
    );
    let p_sat = sat_pressure_4(t_sat);
    assert_eq!(
        try_new_from_sat_pressure_quality(p_sat, 0.5, vol).unwrap(),
        TampinesSteamTableCV::new_from_sat_pressure_quality(p_sat, 0.5, vol)
    );
    println!(
        "CV from sat T = {} K, x = 0.5: T_sat(p_sat) = {:?}",
        t_sat.get::<kelvin>(),
        sat_temp_4(p_sat)
    );

    // Out-of-range, each direction.
    assert!(
        try_new_from_tp_quality(ThermodynamicTemperature::new::<kelvin>(250.0), p, vol, 0.5)
            .is_err()
    );
    assert!(
        try_new_from_tp_quality(ThermodynamicTemperature::new::<kelvin>(2300.0), p, vol, 0.5)
            .is_err()
    );
    assert!(try_new_from_tp_quality(t, Pressure::new::<megapascal>(101.0), vol, 0.5).is_err());
    assert!(try_new_from_tp_quality(t, Pressure::new::<pascal>(0.0), vol, 0.5).is_err());
    assert!(matches!(
        try_new_from_tp_quality(t, p, vol, -0.5),
        Err(SteamTablesError::QualityOutOfRange { .. })
    ));
    assert!(matches!(
        try_new_from_tp_quality(t, p, vol, 1.5),
        Err(SteamTablesError::QualityOutOfRange { .. })
    ));
    assert!(
        try_new_from_tp_quality_0(ThermodynamicTemperature::new::<kelvin>(250.0), p, vol).is_err()
    );
    assert!(
        try_new_from_tp_quality_1(ThermodynamicTemperature::new::<kelvin>(250.0), p, vol).is_err()
    );
    assert!(try_new_from_ph(Pressure::new::<pascal>(100.0), h, vol).is_err());
    assert!(try_new_from_ph(Pressure::new::<megapascal>(101.0), h, vol).is_err());
    assert!(try_new_from_ps(Pressure::new::<pascal>(100.0), s, vol).is_err());
    assert!(try_new_from_ps(Pressure::new::<megapascal>(101.0), s, vol).is_err());
    // Exactly p_sat(273.15 K) is ACCEPTED, matching the (p,h) constructor.
    // This constructor rejected it until bead `op-znjx` (2026-08-11); see
    // `ps_flash_accepts_exact_triple_point_isobar` for the root cause.
    assert!(try_new_from_ps(p_triple(), s, vol).is_ok());
    assert!(try_new_from_sat_temp_quality(
        ThermodynamicTemperature::new::<kelvin>(250.0),
        0.5,
        vol
    )
    .is_err());
    assert!(try_new_from_sat_pressure_quality(p_sat, 1.5, vol).is_err());

    // NaN, each argument.
    assert!(matches!(
        try_new_from_tp_quality(
            ThermodynamicTemperature::new::<kelvin>(f64::NAN),
            p,
            vol,
            0.5
        ),
        Err(SteamTablesError::NonFinite {
            quantity: "temperature",
            ..
        })
    ));
    assert!(matches!(
        try_new_from_tp_quality(t, Pressure::new::<pascal>(f64::NAN), vol, 0.5),
        Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_new_from_tp_quality(t, p, vol, f64::NAN),
        Err(SteamTablesError::NonFinite {
            quantity: "steam quality",
            ..
        })
    ));
    assert!(matches!(
        try_new_from_ph(Pressure::new::<pascal>(f64::NAN), h, vol),
        Err(SteamTablesError::NonFinite {
            quantity: "pressure",
            ..
        })
    ));
    assert!(matches!(
        try_new_from_ph(p, AvailableEnergy::new::<joule_per_kilogram>(f64::NAN), vol),
        Err(SteamTablesError::NonFinite {
            quantity: "specific enthalpy",
            ..
        })
    ));
    assert!(matches!(
        try_new_from_ps(
            p,
            SpecificHeatCapacity::new::<kilojoule_per_kilogram_kelvin>(f64::NAN),
            vol
        ),
        Err(SteamTablesError::NonFinite {
            quantity: "specific entropy",
            ..
        })
    ));

    // Inclusive edges accepted: 100 MPa, 273.15 K, 1073.15 K, x = 0, x = 1.
    let p_100 = Pressure::new::<megapascal>(100.0);
    for t_edge in [273.15_f64, 623.15, 1073.15] {
        let te = ThermodynamicTemperature::new::<kelvin>(t_edge);
        assert!(try_new_from_tp_quality(te, p_100, vol, 0.0).is_ok());
        assert!(try_new_from_tp_quality(te, p_100, vol, 1.0).is_ok());
    }
    assert!(try_new_from_ph(p_100, h, vol).is_ok());
    assert!(try_new_from_ps(p_100, s, vol).is_ok());
}
