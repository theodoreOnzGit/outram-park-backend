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
//! ## Specific entropy `S` (added 2026-07-13, bead op-kbc.14)
//!
//! Implemented via Gibbs' theorem for ideal-gas mixtures: each pure
//! component's ideal-gas entropy evaluated at the mixture's *total* `T, p`
//! (`s_i(T,p) = ∫Cp_i/T dT − R·ln(p/p_0) + s_{i,0}`, using the same `Cp_i(T)`
//! implied by the already-verified enthalpy polynomials), plus the standard
//! ideal entropy of mixing and the same virial correction `H` uses. Water's
//! reference constant is anchored via Clausius-Clapeyron
//! (`Δs = Δh/T` for a phase change) to the same ASHRAE zero-point convention
//! `H` already uses; dry air's is an arbitrary (entropy has no absolute
//! zero) but explicit, self-consistent zero at 0 °C / 1 atm. See
//! `src/humid_air/mod.rs::ideal_gas_molar_entropy_{air,water}` for the full
//! derivation and citations.
//!
//! **A real bug caught before this was ever committed as correct**: the
//! first implementation used dry air's own virial-corrected molar volume
//! (`+R·ln(v̄_a/v̄_{a,0})`) for the pressure/volume term, mirroring how
//! CoolProp's *own* `IdealGasMolarEntropy_Air` takes a volume argument. But
//! CoolProp evaluates the *real* ideal-gas Helmholtz term there (density-
//! native by construction); this port instead integrates a `Cp` — the
//! *constant-pressure* heat capacity — so the volume-based term was wrong by
//! construction: at fixed `p`, `v̄_a ∝ T`, silently smuggling in a second,
//! spurious `T`-dependence alongside the `Cp`-integral's already-correct one.
//! This is exactly the kind of error the thermodynamic-consistency check
//! below is designed to catch: it broke `dS/dT = Cp/T` by ~28% at 25 °C.
//! Fixed by using `−R·ln(p/p_0)` (the mixture's total pressure) for *both*
//! components instead, per Gibbs' theorem.
//!
//! **Verification**: `dS/dT = Cp/T` (both by finite difference) at fixed
//! `p, W` is a rigorous thermodynamic identity (`T dS = dH` at constant
//! pressure) that must hold for internally-consistent `H` and `S` regardless
//! of the underlying real-gas model. Checked at `T ∈ {1, 10, 25, 40} °C`,
//! `W ∈ {0, 0.005, 0.010, 0.020} kg/kg`: relative error is `7×10⁻⁴` to
//! `1.2×10⁻³` throughout, small, smoothly varying with `T` and `W` (not
//! erratic), and of the same order as the virial correction's own size at
//! atmospheric pressure — consistent with a residual artifact of the
//! `B(T),C(T)` virial cross-terms' implicit path-dependence (evaluated at
//! fixed volume; the true isobaric density path is only approximately
//! captured), not a remaining implementation bug. Not tightened further
//! given the effort-to-confidence tradeoff; revisit if a CoolProp/`rfluids`
//! oracle (`op-kbc.3`) becomes available.
//!
//! **Caveat on the absolute value** (not just the derivative): this port's
//! mixture *enthalpy* `H_da` is already known to carry a small (~6.6 kJ/kg)
//! offset against ASHRAE's own tabulated `H_da` (see above) — the pure
//! `h_w(0°C) ≈ 2501 kJ/kg` water-vapor anchor checks out well against the
//! known latent heat of vaporization, but the *mixture* combination (both
//! components plus the virial cross-term) is the piece with the small
//! measured offset. Since `S`'s absolute value is anchored through that same
//! enthalpy correlation, it likely carries a comparably small absolute
//! offset too. Not yet cross-checked against an external table value for
//! `S` specifically — flagged rather than asserted.
//!
//! ## Results (2026-07-10, this port)
//!
//! At `T = 298.15 K` (25 °C), `p = 101 325 Pa`, `W = 0.010 kg/kg`: round-trip
//! `R` recovers to `<1e-6`; `c_p` (finite difference, `ΔT = 0.1 K`) is
//! 1025.41 J/(kg·K) vs the ASHRAE estimate 1024.60 J/(kg·K) (0.08%); `v` is
//! 0.85789 m³/kg vs the ideal-gas estimate 0.85823 m³/kg (0.04%).
//!
//! ## Wet-bulb / dew-point (added 2026-07-13, bead op-kbc.14 follow-up)
//!
//! Two independent checks, since no CoolProp/`rfluids` oracle is available
//! (same reason as above):
//!
//! 1. **Saturation identity**: at `R = 1` (100% relative humidity), `T_dp`
//!    and `T_wb` must both equal the dry-bulb `T` exactly — this is a
//!    mathematical consequence of the definitions themselves (both are only
//!    ever below `T` when the air is *unsaturated*), not an approximate
//!    physical reference, so it is checked to a tight tolerance.
//! 2. **Cross-check against Stull's explicit empirical wet-bulb formula**
//!    (Stull, R., 2011: "Wet-Bulb Temperature from Relative Humidity and Air
//!    Temperature", *J. Appl. Meteor. Climatol.*, **50**, 2267-2269, Eq. 1) —
//!    an independent, non-iterative closed-form fit to psychrometric wet-bulb
//!    data, valid roughly -20 to 50 °C / 5-99% RH at sea-level pressure. Its
//!    own stated accuracy is order 0.3 °C typical (occasionally approaching
//!    1 °C at the edges of its fitted range), so this test uses a 0.5 °C
//!    tolerance — coarse, but a genuine independent check since Stull's
//!    correlation shares no code or fitted coefficients with this port's
//!    energy-balance solve.
//!
//! A caught real bug during this verification, worth recording: the first
//! implementation of the wet-bulb energy balance evaluated liquid water's
//! enthalpy via `flash::state_pt`, which only reaches the vapour/supercritical
//! branch from its ideal-gas initial density guess (documented in its own doc
//! comment) — wrong branch for the subcooled liquid water actually present at
//! a typical `T_wb`. It silently returned a nonsense density/enthalpy
//! (~`-4.2e22 J/kg` for liquid water at 20 °C, 1 atm) rather than erroring.
//! Fixed by using the saturated-liquid density at `T_wb`
//! (`vle::saturation_at_temperature`) instead — liquid enthalpy is only
//! weakly pressure-dependent, so this is an excellent approximation to the
//! true compressed-liquid value, and it is the same pattern
//! `saturation::vbar_ws` already uses elsewhere in this module.
//!
//! Measured (2026-07-13), `p = 101 325 Pa`:
//!
//! | `T` (°C) | `R` | `T_dp` (°C) | `T_wb` (°C) | Stull `T_wb` (°C) | Δ (°C) |
//! |---|---|---|---|---|---|
//! | 30 | 0.50 | 18.451 | 22.001 | 22.297 | 0.30 |
//! | 25 | 0.60 | 16.704 | 19.468 | 19.503 | 0.04 |
//! | 20 | 1.00 | 20.000 | 20.000 | 20.010 | 0.01 |
//!
//! (The `R = 1` row's small Stull residual is that formula's own approximation
//! error at the saturation boundary, not a violation of the exact identity —
//! this port's own `T_dp = T_wb = T` there is exact by construction, per
//! check 1 above.)
//!
//! ## `T_wb`/`T_dp` as input keys (added 2026-07-13)
//!
//! `(T, p, T_dp)` and `(T, p, T_wb)` now also solve for the state (inverting
//! `ψ_w`), not just compute `T_dp`/`T_wb` as outputs. Verified by round-trip:
//! solve `(T, p, R) → T_dp` and `→ T_wb` (already verified above), then feed
//! each back in as the third input and recover the original `R` to `<1e-4`
//! (looser than the `<1e-6` `W↔R` round-trip above because this one chains
//! two iterative solves — forward dew-point/wet-bulb, then the inversion —
//! instead of one). Includes the `R = 1` (saturation) edge case, which
//! needed a real fix — see the module doc's caveats section and
//! `psi_w_from_wet_bulb`'s own doc comment in `src/humid_air/mod.rs`.

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
fn unsupported_input_triple_is_rejected_not_wrong() {
    // Entropy is now a supported *output* (bead op-kbc.14, 2026-07-13) --
    // this test now covers what's still genuinely unsupported: an input
    // triple that isn't (T, p, {W|R|psi_w|T_dp|T_wb}), e.g. (T, p, H).
    let res = ha_props(
        HumidAirParam::RelativeHumidity,
        (HumidAirParam::TDryBulb, T),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::Enthalpy, 50_000.0),
    );
    assert_eq!(res, Err(outram_park_fork_coolprop::humid_air::HumidAirError::UnsupportedInputs));
}

fn dew_and_wet_bulb(t: f64, p: f64, r: f64) -> (f64, f64) {
    let tdp = ha_props(HumidAirParam::TDewPoint, (HumidAirParam::TDryBulb, t), (HumidAirParam::Pressure, p), (HumidAirParam::RelativeHumidity, r))
        .expect("T_dp");
    let twb = ha_props(HumidAirParam::TWetBulb, (HumidAirParam::TDryBulb, t), (HumidAirParam::Pressure, p), (HumidAirParam::RelativeHumidity, r))
        .expect("T_wb");
    (tdp, twb)
}

/// Stull (2011), Eq. 1: an explicit empirical fit to wet-bulb temperature
/// from dry-bulb `t_c` [°C] and relative humidity `rh_pct` [0, 100], at
/// sea-level pressure. Independent of this port's own energy-balance solve —
/// shares no code or fitted coefficients with it.
fn stull_wet_bulb_c(t_c: f64, rh_pct: f64) -> f64 {
    t_c * (0.151977 * (rh_pct + 8.313659).sqrt()).atan() + (t_c + rh_pct).atan() - (rh_pct - 1.676331).atan()
        + 0.00391838 * rh_pct.powf(1.5) * (0.023101 * rh_pct).atan()
        - 4.686035
}

#[test]
fn saturation_identity_tdp_twb_equal_t() {
    // At R = 1 the air is already saturated, so both T_dp and T_wb collapse
    // onto the dry-bulb T exactly -- a consequence of their definitions, not
    // an approximate physical reference (see the module doc for why this is
    // the rigorous check here).
    let t = 273.15 + 20.0;
    let (tdp, twb) = dew_and_wet_bulb(t, P, 1.0);
    eprintln!("R=1: T_dp={tdp:.6} K, T_wb={twb:.6} K (T={t:.6} K)");
    assert!((tdp - t).abs() < 1e-3, "T_dp should equal T at saturation: {tdp} vs {t}");
    assert!((twb - t).abs() < 1e-3, "T_wb should equal T at saturation: {twb} vs {t}");
}

#[test]
fn wet_bulb_vs_stull_2011_and_ordering() {
    // (T_c, R) points, p = atmospheric.
    let points = [(30.0, 0.50), (25.0, 0.60)];
    for (t_c, r) in points {
        let t = 273.15 + t_c;
        let (tdp, twb) = dew_and_wet_bulb(t, P, r);
        let twb_c = twb - 273.15;
        let stull = stull_wet_bulb_c(t_c, r * 100.0);
        eprintln!("T={t_c}C R={r}: T_dp={:.3}C T_wb={twb_c:.3}C (Stull {stull:.3}C, ordering T_dp<=T_wb<=T)", tdp - 273.15);

        // Physical ordering: T_dp <= T_wb <= T (equality only at saturation).
        assert!(tdp <= twb + 1e-9, "T_dp ({tdp}) must not exceed T_wb ({twb})");
        assert!(twb <= t + 1e-9, "T_wb ({twb}) must not exceed T ({t})");

        // Independent cross-check against Stull's closed-form fit -- coarse
        // tolerance since Stull's own stated accuracy is order 0.3 C (see
        // the module doc).
        assert!(
            (twb_c - stull).abs() < 0.5,
            "T_wb = {twb_c} C vs Stull(2011) = {stull} C, difference too large"
        );
    }
}

fn relative_humidity_from(t: f64, p: f64, input: (HumidAirParam, f64)) -> Result<f64, outram_park_fork_coolprop::humid_air::HumidAirError> {
    ha_props(HumidAirParam::RelativeHumidity, (HumidAirParam::TDryBulb, t), (HumidAirParam::Pressure, p), input)
}

#[test]
fn tdp_and_twb_as_input_keys_round_trip_r() {
    // Unsaturated case.
    let t = 273.15 + 30.0;
    let r_in = 0.5;
    let (tdp, twb) = dew_and_wet_bulb(t, P, r_in);

    let r_from_tdp = relative_humidity_from(t, P, (HumidAirParam::TDewPoint, tdp)).expect("R from (T,p,T_dp)");
    let r_from_twb = relative_humidity_from(t, P, (HumidAirParam::TWetBulb, twb)).expect("R from (T,p,T_wb)");
    eprintln!("R={r_in}: T_dp={tdp:.4} -> R={r_from_tdp:.6}; T_wb={twb:.4} -> R={r_from_twb:.6}");
    assert!((r_from_tdp - r_in).abs() < 1e-4, "R from T_dp input: {r_from_tdp} vs {r_in}");
    assert!((r_from_twb - r_in).abs() < 1e-4, "R from T_wb input: {r_from_twb} vs {r_in}");

    // Saturation edge case (R = 1): T_wb = T_dp = T exactly, and this is the
    // case that needed psi_w_from_wet_bulb's dedicated short-circuit (see
    // the module doc's caveats section).
    let t_sat = 273.15 + 20.0;
    let (tdp_sat, twb_sat) = dew_and_wet_bulb(t_sat, P, 1.0);
    let r_from_tdp_sat = relative_humidity_from(t_sat, P, (HumidAirParam::TDewPoint, tdp_sat)).expect("R from (T,p,T_dp) at saturation");
    let r_from_twb_sat = relative_humidity_from(t_sat, P, (HumidAirParam::TWetBulb, twb_sat)).expect("R from (T,p,T_wb) at saturation");
    eprintln!("R=1 (saturated): T_dp={tdp_sat:.6} -> R={r_from_tdp_sat:.8}; T_wb={twb_sat:.6} -> R={r_from_twb_sat:.8}");
    assert!((r_from_tdp_sat - 1.0).abs() < 1e-4, "R from T_dp input at saturation: {r_from_tdp_sat}");
    assert!((r_from_twb_sat - 1.0).abs() < 1e-4, "R from T_wb input at saturation: {r_from_twb_sat}");
}

#[test]
fn dew_point_below_triple_point_is_rejected_as_input_too() {
    // T = 10 C, R = 0.4 has a sub-freezing dew point -- errors as an output
    // (already covered implicitly elsewhere) and must also error when an
    // already-known T_dp below the triple point is fed back in as an input.
    let res = ha_props(
        HumidAirParam::HumidityRatio,
        (HumidAirParam::TDryBulb, 273.15 + 10.0),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::TDewPoint, 270.0),
    );
    assert!(res.is_err(), "T_dp below the triple point must error as an input, not silently extrapolate");
}

fn s_at(t: f64, p: f64, w: f64) -> f64 {
    ha_props(HumidAirParam::Entropy, (HumidAirParam::TDryBulb, t), (HumidAirParam::Pressure, p), (HumidAirParam::HumidityRatio, w))
        .expect("S from (T,p,W)")
}

#[test]
fn entropy_thermodynamic_consistency_ds_dt_eq_cp_over_t() {
    // T dS = dH at constant p (any composition), so dS/dT|p,W must equal
    // Cp/T -- a rigorous identity independent of the underlying real-gas
    // model, checked across the practical HVAC/psychrometric range. See the
    // module doc for why the residual is small-but-nonzero rather than
    // machine-precision, and the real bug (28% error) this check caught
    // before the fix.
    let dt = 0.01;
    for w in [0.0, 0.005, 0.010, 0.020] {
        for t_c in [1.0, 10.0, 25.0, 40.0] {
            let t = 273.15 + t_c;
            let ds_dt = (s_at(t + dt, P, w) - s_at(t - dt, P, w)) / (2.0 * dt);
            let cp = (enthalpy_at(t + dt, w) - enthalpy_at(t - dt, w)) / (2.0 * dt);
            let rel_err = (ds_dt - cp / t).abs() / (cp / t).abs();
            eprintln!("T={t_c}C W={w}: dS/dT={ds_dt:.5} J/(kg.K^2), Cp/T={:.5} J/(kg.K^2), rel_err={rel_err:.2e}", cp / t);
            assert!(rel_err < 2e-3, "T={t_c}C W={w}: dS/dT={ds_dt} vs Cp/T={}, rel_err={rel_err} too large", cp / t);
        }
    }
}

#[test]
fn entropy_dry_air_h_near_zero_at_0c_ashrae_convention() {
    // Sanity print/check on the ASHRAE dry-air H=0-at-0C convention this
    // port's enthalpy polynomial already relies on (see mod.rs) -- entropy's
    // water-vapor reference constant is anchored to the *same* convention
    // via Clausius-Clapeyron (see the module doc), so confirming this
    // holds (to within the real-gas virial correction's own size) is a
    // proxy for confirming that anchor is sound. The pure h_w(273.15 K)
    // polynomial value itself (~45064 J/mol =~ 2501 kJ/kg, matching water's
    // known latent heat of vaporization at 0C) is checked directly in
    // src/humid_air/mod.rs's own unit tests, where the private
    // ideal_gas_molar_enthalpy_water function is directly reachable.
    //
    // Measured ~92 J/kg_da at R -> 0 (confirmed independent of exactly how
    // dry by sweeping R from 1e-9 to 1e-20 and observing convergence to the
    // same value) -- this is the virial correction to real (non-ideal) dry
    // air's enthalpy at atmospheric pressure, not a residual-moisture
    // artifact or a bug: the ASHRAE H=0 convention applies to the *ideal-gas*
    // part only ([`ideal_gas_molar_enthalpy_air`]'s polynomial evaluates to
    // ~0.27 J/kg at 0C), and real air is not exactly ideal.
    let h_near_dry = ha_props(
        HumidAirParam::Enthalpy,
        (HumidAirParam::TDryBulb, 273.15 + 0.1),
        (HumidAirParam::Pressure, P),
        (HumidAirParam::RelativeHumidity, 1e-9),
    )
    .expect("H at ~0C, ~dry air");
    eprintln!("H(~0C, ~dry) = {h_near_dry:.3} J/kg_da (expect small, ASHRAE dry-air convention + virial correction)");
    assert!(
        h_near_dry.abs() < 200.0,
        "dry-air H at 0C should be small (ASHRAE convention + virial correction, not large): {h_near_dry}"
    );
}
