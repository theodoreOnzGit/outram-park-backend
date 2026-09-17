//! V&V — the newly-ported Helmholtz **term types**, each exercised by a
//! representative fluid, checked against CoolProp's tabulated states.
//!
//! ## Methodology
//!
//! When the codegen (`dev/gen_fluid.py`) was extended from the initial
//! Power/Gaussian/NonAnalytic + Lead/LogTau/PlanckEinstein set to cover all 137
//! CoolProp fluids, six new term forms were added to the engine
//! (`src/eos.rs`). This test pins each new form to a fluid that uses it and
//! checks the engine against CoolProp's own tabulated reference states
//! (`dev/fluids/<Fluid>.json`, the `STATES` block — `T`, `rhomolar`, `p`,
//! `hmolar`, `smolar` — extracted at authoring time, not a runtime input):
//!
//! | Fluid     | New term type(s) validated                                   |
//! |-----------|--------------------------------------------------------------|
//! | Nitrogen  | ideal `Power`, ideal `PlanckEinsteinFunctionT`               |
//! | Fluorine  | residual `Exponential`, ideal `Power` + `PlanckEinsteinGeneralized` |
//! | Methanol  | residual `DoubleExponential` + `Exponential`, ideal `CP0PolyT` |
//! | R125      | residual `DoubleExponential` (Lemmon2005 lowering)           |
//! | Ammonia   | residual `GaoB` (and `Exponential`)                          |
//! | R22       | ideal `CP0Constant`                                          |
//! | n-Heptane | ideal `CP0AlyLee` (lowered to `CP0PolyT` + `PlanckEinsteinGeneralized`) |
//!
//! Two well-conditioned checks are asserted per fluid:
//!
//! - **`h` and `s` at the triple-liquid state.** At liquid density the residual
//!   derivatives (`αr_δ`, `αr_τ`) are strongly active, and `h`/`s` combine the
//!   residual *and* ideal contributions with no cancellation — so this is a
//!   simultaneous check of both halves of every new term. (Pressure at the
//!   triple-liquid state is *not* asserted: there `1 + δ·αr_δ ≈ 0`, a large
//!   cancellation that makes the *relative* pressure error meaningless while the
//!   *absolute* error stays ~0.02 Pa.)
//! - **`p` at the triple-vapour state**, a well-conditioned pressure (the vapour
//!   is near-ideal, `p ≈ ρRT`), as a direct pressure assertion.
//!
//! Pressure at the exact critical point is deliberately not asserted: `cp` and
//! the speed of sound diverge there (`∂p/∂ρ → 0`), and the EOS critical pressure
//! differs from the *experimental* tabulated `pc` by up to ~1.3 % for some
//! fluids (Fluorine) — an EOS-vs-experiment gap, not an engine error.
//!
//! ## Results (2026-07-08, this port; CoolProp fluid data as cloned)
//!
//! All seven fluids pass. Measured triple-liquid deviations
//! `|Δ|/(|ref| + 1)`: `h` ≤ 8.8e-9 and `s` ≤ 1.6e-6 for every fluid except
//! Ammonia (whose reference `h`/`s` are normalised to ~0, giving 2.0e-5 / 2.5e-5
//! against the `+1` floor) — i.e. agreement to the printed precision of the
//! reference values.
//!
//! Measured triple-vapour pressure deviations `|Δp|/p`: Nitrogen 1.9e-8,
//! Ammonia 1.2e-7, R125 9.6e-6, Fluorine 9.3e-7, R22 9.4e-4, methanol 2.9e-3,
//! n-heptane 3.3e-3. The three largest are **reference-precision-limited, not
//! engine error**: at ρ ~ 1e-4 mol/m³ the JSON prints the vapour density to only
//! ~3 significant figures, and `p ≈ ρRT` inherits that — the pressure deviation
//! equals the density's rounding (methanol printed ρ = 1.28e-4 vs the ideal-gas
//! ρ(p_ref) = 1.276e-4, +2.9e-3; n-heptane +3.3e-3; R22 −9.4e-4), verified in the
//! commit that added this test. The residual terms are, in any case, exercised
//! far more strongly by the dense triple-liquid `h`/`s` check above. Asserted
//! tolerances: 1e-4 on `h`/`s`, 5e-3 on `p` (bounding the ~3-sig-fig reference
//! density).

// Reference values are transcribed verbatim from CoolProp's JSON, so the
// integer/fraction digit grouping is theirs, not ours.
#![allow(clippy::inconsistent_digit_grouping)]

use outram_park_fork_coolprop::{state_trho, Fluid};

/// Combined relative/absolute deviation, robust for near-zero reference values.
fn dev(val: f64, reference: f64) -> f64 {
    (val - reference).abs() / (reference.abs() + 1.0)
}

/// Assert triple-liquid `h`,`s` (mass→molar via `mm`) and triple-vapour `p`.
#[allow(clippy::too_many_arguments)]
fn check(
    fluid: Fluid,
    mm: f64,
    // triple liquid: (T, rhomolar, hmolar_ref, smolar_ref)
    t_l: f64,
    rhomolar_l: f64,
    h_l_ref: f64,
    s_l_ref: f64,
    // triple vapour: (T, rhomolar, p_ref)
    t_v: f64,
    rhomolar_v: f64,
    p_v_ref: f64,
) {
    let name = fluid.name();

    let liq = state_trho(fluid, t_l, rhomolar_l * mm);
    let dh = dev(liq.enthalpy * mm, h_l_ref);
    let ds = dev(liq.entropy * mm, s_l_ref);
    assert!(
        dh < 1e-4,
        "{name}: triple-liquid h_molar dev {dh:.2e} (got {}, ref {h_l_ref})",
        liq.enthalpy * mm
    );
    assert!(
        ds < 1e-4,
        "{name}: triple-liquid s_molar dev {ds:.2e} (got {}, ref {s_l_ref})",
        liq.entropy * mm
    );

    let vap = state_trho(fluid, t_v, rhomolar_v * mm);
    let dp = (vap.pressure - p_v_ref).abs() / p_v_ref;
    // 5e-3 bounds the reference vapour density's ~3-sig-fig precision at
    // ρ ~ 1e-4 mol/m³ (see module docs); the residual is checked hard above.
    assert!(
        dp < 5e-3,
        "{name}: triple-vapour pressure dev {dp:.2e} (got {}, ref {p_v_ref})",
        vap.pressure
    );
}

#[test]
fn nitrogen_ideal_power_and_planck_einstein_function_t() {
    check(
        Fluid::Nitrogen,
        0.028_013_48,
        63.151,
        30957.310_275,
        -4222.6109,
        67.9513,
        63.151,
        24.069_588,
        12519.795_379,
    );
}

#[test]
fn fluorine_residual_exponential_and_ideal_generalized() {
    check(
        Fluid::Fluorine,
        0.037_996_81,
        53.4811,
        44917.310_369,
        -1778.2909,
        -26.0725,
        53.4811,
        0.537_249,
        238.810_332,
    );
}

#[test]
fn methanol_double_exponential_and_cp0_polyt() {
    check(
        Fluid::Methanol,
        0.032_042_16,
        175.61,
        28230.396_622,
        -12439.5748,
        -49.5241,
        175.61,
        0.000_128,
        0.186_350,
    );
}

#[test]
fn r125_lemmon2005_double_exponential() {
    check(
        Fluid::R125,
        0.120_021_4,
        172.52,
        14086.490_950,
        10457.4795,
        58.8377,
        172.52,
        2.038_125,
        2914.046_009,
    );
}

#[test]
fn ammonia_gaob_bell_term() {
    check(
        Fluid::Ammonia,
        0.017_030_52,
        195.495,
        43090.208_806,
        0.5071,
        0.0019,
        195.495,
        3.742_198,
        6055.813_575,
    );
}

#[test]
fn r22_ideal_cp0_constant() {
    check(
        Fluid::R22,
        0.086_468,
        115.73,
        19906.534_061,
        2559.4876,
        6.5813,
        115.73,
        0.000_394,
        0.379_475,
    );
}

#[test]
fn heptane_ideal_cp0_aly_lee() {
    check(
        Fluid::NHeptane,
        0.100_202,
        182.55,
        7745.685_209,
        -41621.0135,
        -153.9828,
        182.55,
        0.000_116,
        0.175_490,
    );
}
