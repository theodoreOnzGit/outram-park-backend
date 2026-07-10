//! V&V — humid-air (`HAPropsSI`-equivalent) properties against ASHRAE
//! psychrometric reference values.
//!
//! ## Methodology
//!
//! No CoolProp/`rfluids` oracle is available in this environment (op-kbc.3 —
//! `rfluids` dev-dependency wiring is a follow-up), so this test uses two
//! independent checks:
//!
//! 1. **Round-trip consistency**: `(T, p, R) → W` then `(T, p, W) → R` must
//!    recover the original relative humidity — a real-gas-model-independent
//!    check of the `ψ_w ↔ W ↔ R` inversions and the enhancement-factor solve.
//! 2. **`c_p` (finite-difference `dH/dT`) vs the ASHRAE `c_p` approximation**
//!    `c_p ≈ 1.006 + 1.86·W` kJ/(kg·K) (ASHRAE Fundamentals). `c_p` is used
//!    instead of absolute `H` because this port's ideal-gas enthalpy
//!    correlations (CoolProp's `FlagUseIdealGasEnthalpyCorrelations`
//!    polynomials — see `mod.rs::ideal_gas_molar_enthalpy_{water,air}`) carry
//!    an unresolved additive reference-state offset (CoolProp's own source
//!    comment on `hbar_0`: "not clear why getting rid of this term yields the
//!    correct values in the table... enthalpies are equal to an additive
//!    constant"). `dH/dT` cancels any such constant, so it is the correct
//!    reference-independent check; a raw-`H` ballpark against ASHRAE was
//!    tried first and found offset by a roughly constant ~6.6 kJ/kg — a
//!    reference-state artifact, not a physics error. Absolute `H`
//!    verification needs the `ensure_ref_offsets` calibration this port does
//!    not implement — bead op-kbc.14.
//!
//! 3. Specific volume `v` against the ideal-gas estimate `v ≈ R_a·T/p_a`
//!    (`p_a = p − p_w`) — `V` **is** reference-state-independent (a direct EOS
//!    quantity), so this is a real, if loose, oracle check.
//!
//! Debugging trail: both checks originally failed by ~3.5–5.5% — `c_p` low,
//! `V` high. Isolating `V`'s error (printing the molar-volume secant solve's
//! `B_m`/`C_m` inputs) found the third-virial mixture term `C_m` four orders
//! of magnitude too large; tracing that to its source found `virials.rs`'s
//! `c_aaw` was missing a `1/rhobarstar²` factor (`rhobarstar = 1000` for that
//! specific CoolProp correlation, unlike its sibling `c_aww` where
//! `rhobarstar = 1` makes the factor a no-op — an easy transcription trap).
//! Fixed in `virials.rs`; both checks now agree to <0.1%.
//!
//! ## Results (2026-07-10, this port)
//!
//! At `T = 298.15 K` (25 °C), `p = 101 325 Pa`, `W = 0.010 kg/kg`: round-trip
//! `R` recovers to `<1e-6`; `c_p` (finite difference, `ΔT = 0.1 K`) is
//! 1025.41 J/(kg·K) vs the ASHRAE estimate 1024.60 J/(kg·K) (0.08%); `v` is
//! 0.85789 m³/kg vs the ideal-gas estimate 0.85823 m³/kg (0.04%).

use outram_park_fork_coolprop::humid_air::{ha_props, HumidAirParam};

const T: f64 = 298.15; // 25 C
const P: f64 = 101_325.0;

#[test]
fn round_trip_r_to_w_to_r() {
    let r_in = 0.5;
    let w = ha_props(
        HumidAirParam::HumidityRatio,
        (HumidAirParam::TDryBulb, T),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::RelativeHumidity, r_in),
    )
    .expect("W from (T,p,R)");

    let r_out = ha_props(
        HumidAirParam::RelativeHumidity,
        (HumidAirParam::TDryBulb, T),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::HumidityRatio, w),
    )
    .expect("R from (T,p,W)");

    eprintln!("W(R=0.5) = {w:.6} kg/kg; round-trip R = {r_out:.8}");
    assert!((r_out - r_in).abs() < 1e-6, "round-trip R mismatch: {r_in} -> {w} -> {r_out}");
}

fn enthalpy_at(t: f64, w: f64) -> f64 {
    ha_props(
        HumidAirParam::Enthalpy,
        (HumidAirParam::TDryBulb, t),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::HumidityRatio, w),
    )
    .expect("H from (T,p,W)")
}

#[test]
fn cp_and_volume_vs_ashrae_simplified_formula() {
    let w = 0.010;

    // c_p = dH/dT by central finite difference — reference-state-independent
    // (see the module doc for why absolute H is not compared directly).
    let dt = 0.1;
    let cp = (enthalpy_at(T + dt, w) - enthalpy_at(T - dt, w)) / (2.0 * dt);
    let cp_ashrae = (1.006 + 1.86 * w) * 1000.0; // J/(kg.K)
    eprintln!("c_p(T=25C,p=101325,W=0.01) = {cp:.2} J/(kg_da.K); ASHRAE estimate = {cp_ashrae:.2} J/(kg_da.K)");
    assert!(
        ((cp - cp_ashrae) / cp_ashrae).abs() < 0.01,
        "c_p = {cp}, ASHRAE estimate = {cp_ashrae}, relative error too large"
    );

    let v = ha_props(
        HumidAirParam::Volume,
        (HumidAirParam::TDryBulb, T),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::HumidityRatio, w),
    )
    .expect("V from (T,p,W)");

    // Ideal-gas ballpark for V: p_a = p - p_w, v ~ R_specific_air * T / p_a.
    let p_w = ha_props(
        HumidAirParam::WaterPartialPressure,
        (HumidAirParam::TDryBulb, T),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::HumidityRatio, w),
    )
    .expect("p_w");
    let p_a = P - p_w;
    let r_specific_air = 287.05; // J/(kg.K)
    let v_ideal = r_specific_air * T / p_a;
    eprintln!("V(T=25C,p=101325,W=0.01) = {v:.5} m3/kg_da; ideal-gas estimate = {v_ideal:.5} m3/kg_da");
    assert!(
        ((v - v_ideal) / v_ideal).abs() < 0.01,
        "V = {v}, ideal-gas ballpark = {v_ideal}, relative error too large"
    );
}

#[test]
fn out_of_range_below_triple_point_is_rejected() {
    let res = ha_props(
        HumidAirParam::HumidityRatio,
        (HumidAirParam::TDryBulb, 250.0),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::RelativeHumidity, 0.5),
    );
    assert!(res.is_err(), "T below the triple point must error, not silently extrapolate");
}

#[test]
fn unsupported_output_is_rejected_not_wrong() {
    let res = ha_props(
        HumidAirParam::Entropy,
        (HumidAirParam::TDryBulb, T),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::RelativeHumidity, 0.5),
    );
    assert_eq!(res, Err(outram_park_fork_coolprop::humid_air::HumidAirError::UnsupportedInputs));
}
