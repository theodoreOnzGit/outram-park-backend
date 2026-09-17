//! V&V — humid-air derived-energy outputs (`cp`, `cp_ha`, `cv`, `cv_ha`,
//! `Z`, internal energy `U`/`Uha`, and the humid-air-basis variants
//! `Hha`/`Sha`/`Vha`) against ASHRAE psychrometric approximations, the
//! ideal-gas Mayer relation, and model-independent thermodynamic identities.
//!
//! ## Why these references (no CoolProp oracle available)
//!
//! As documented in `humid_air_reference.rs`, no CoolProp/`rfluids` oracle is
//! wired into this environment yet (bead op-kbc.3), and this port's absolute
//! enthalpy/entropy carry an unresolved additive reference-state offset. Every
//! check here is therefore chosen to be **reference-state-independent**:
//!
//! - **Heat capacities** are derivatives (`dH/dT`, `dU/dT`), which cancel any
//!   additive `H`/`U` offset — so they can be checked against absolute
//!   external references (ASHRAE, the ideal-gas Mayer relation `cp − cv ≈ R`).
//! - **`U = H − p·V`** and the **humid-air-basis conversions** (`Xha = X/(1+W)`)
//!   are exact algebraic identities the implementation must satisfy to machine
//!   precision, independent of any reference state — a strong internal-
//!   consistency oracle.
//! - **`Z = p·v̄/(R̄T)`** is a direct re-expression of the mixture molar volume,
//!   which is already validated against the ideal-gas volume in
//!   `humid_air_reference.rs`; here we confirm it lands at the physically
//!   expected near-unity value and deviates correctly with pressure.
//!
//! All reference conditions use `T = 298.15 K (25 °C)`, `p = 101 325 Pa
//! (1 atm)`, the canonical psychrometric reference point.

use outram_park_fork_coolprop::humid_air::{ha_props, HumidAirParam as P};

const T: f64 = 298.15; // 25 C
const PATM: f64 = 101_325.0; // 1 atm
const R_BAR: f64 = 8.314_472; // J/mol/K
const MM_WATER: f64 = 0.018_015_268; // kg/mol
const MM_AIR: f64 = 0.028_966; // kg/mol

/// Evaluate an output at `(T, p, humidity)` where `humidity` is `(key, value)`.
fn at(out: P, t: f64, p: f64, humidity: (P, f64)) -> f64 {
    ha_props(out, (P::TDryBulb, t), (P::Pressure, p), humidity).expect("solve should converge")
}

/// Molar mass of humid air `M_ha` from the humidity ratio `W` — the same
/// mole-fraction weighting the implementation uses, recomputed here so the
/// Mayer-relation reference is self-contained.
fn m_ha_from_w(w: f64) -> f64 {
    let epsilon = MM_WATER / MM_AIR;
    let psi_w = w / (epsilon + w);
    MM_WATER * psi_w + (1.0 - psi_w) * MM_AIR
}

/// METHODOLOGY. Isobaric specific heat capacity `cp` (per kg dry air) of
/// **nearly dry** air at 25 °C / 1 atm (humidity ratio `W = 1e-9`), compared
/// against the ASHRAE Fundamentals dry-air value `cp ≈ 1006 J/(kg·K)`. `cp` is
/// a temperature derivative of `H`, so it is free of this port's absolute-`H`
/// reference-state offset. Pass criterion: within 0.5 % of 1006.
///
/// RESULTS (2026-07-16, this port). `cp_dry = 1006.55 J/(kg·K)`, a +0.05 %
/// deviation from the ASHRAE 1006 J/(kg·K) reference — well inside tolerance.
/// Interpretation: the finite-difference `dH/dT` reproduces the constant-
/// pressure heat capacity implied by the CoolProp ideal-gas enthalpy
/// polynomial (`d/dT` of `9.2486716590e-4·T² + 28.557221776·T + …`) to better
/// than the ASHRAE fit's own precision.
#[test]
fn cp_dry_air_matches_ashrae() {
    let cp = at(P::IsobaricHeatCapacity, T, PATM, (P::HumidityRatio, 1e-9));
    let reference = 1006.0;
    let rel = (cp - reference).abs() / reference;
    eprintln!(
        "cp_dry = {cp:.4} J/(kg.K), ASHRAE 1006, rel dev {:.4}%",
        rel * 100.0
    );
    assert!(
        rel < 5e-3,
        "cp_dry {cp} deviates {:.3}% from ASHRAE 1006",
        rel * 100.0
    );
}

/// METHODOLOGY. Isobaric specific heat capacity `cp` (per kg dry air) of moist
/// air at 25 °C / 1 atm / 50 % RH, compared against the ASHRAE Fundamentals
/// moist-air approximation `cp ≈ 1006 + 1860·W J/(kg·K)` (with `1.86 kJ/(kg·K)`
/// the vapour contribution). The humidity ratio `W` is taken from this crate's
/// own `(T,p,R) → W` solve. Reference-state-independent (a `dH/dT`). Pass
/// criterion: within 1 % of the ASHRAE linear fit (which is itself ~1 %
/// accurate, using temperature-independent coefficients).
///
/// RESULTS (2026-07-16, this port). At `W = 0.009926 kg/kg`: `cp = 1025.27
/// J/(kg·K)` vs ASHRAE `1006 + 1860·0.009926 = 1024.46 J/(kg·K)`, a +0.08 %
/// deviation. Also verified `cp = cp_ha·(1 + W)` exactly (dry- vs humid-air
/// basis): `cp_ha = 1015.19 J/(kg·K)`, `cp_ha·(1+W) = 1025.27 J/(kg·K)`.
/// Interpretation: the moist-air heat capacity, including the water-vapour
/// contribution, is reproduced to well within the ASHRAE approximation's own
/// accuracy, and the two mass bases are consistent by construction.
#[test]
fn cp_moist_air_matches_ashrae() {
    let humidity = (P::RelativeHumidity, 0.5);
    let w = at(P::HumidityRatio, T, PATM, humidity);
    let cp = at(P::IsobaricHeatCapacity, T, PATM, humidity);
    let cp_ha = at(P::IsobaricHeatCapacityHumidAir, T, PATM, humidity);
    let reference = 1006.0 + 1860.0 * w;
    let rel = (cp - reference).abs() / reference;
    eprintln!(
        "W = {w:.6}, cp = {cp:.4}, ASHRAE {reference:.4}, rel dev {:.4}%",
        rel * 100.0
    );
    assert!(
        rel < 1e-2,
        "cp {cp} deviates {:.3}% from ASHRAE {reference}",
        rel * 100.0
    );
    // Dry- vs humid-air basis identity: cp = cp_ha * (1 + W).
    assert!(
        (cp - cp_ha * (1.0 + w)).abs() / cp < 1e-9,
        "cp != cp_ha*(1+W)"
    );
}

/// METHODOLOGY. Isochoric specific heat capacity `cv` at 25 °C / 1 atm /
/// 50 % RH, checked two ways: (a) the dry- vs humid-air basis identity
/// `cv = cv_ha·(1 + W)` (exact, machine precision); (b) the ideal-gas **Mayer
/// relation** `cp − cv = R` — on a per-mol-humid-air basis `c̄p − c̄v = R̄`, so
/// per kg humid air `cp_ha − cv_ha ≈ R̄ / M_ha`. Both `cp` and `cv` are `T`
/// derivatives, so reference-state-independent. Pass criterion: identity to
/// 1e-9 relative; Mayer relation within 1 % (a real gas departs slightly from
/// the ideal `R`).
///
/// RESULTS (2026-07-16, this port). `cv = 732.19 J/(kg·K)`, `cv_ha = 724.99
/// J/(kg·K)`, `cv_ha·(1+W) = 732.19` (identity holds). `cp_ha − cv_ha =
/// 290.20 J/(kg·K)` vs `R̄/M_ha = 288.76 J/(kg·K)`, a +0.50 % departure.
/// Interpretation: the constant-volume heat capacity (from `dU/dT` at fixed
/// molar volume, pressure re-solved from the EOS at each `T`) satisfies the
/// Mayer relation to within the expected small real-gas correction — a
/// non-trivial check, since `cv` exercises the forward EOS pressure evaluation
/// that `cp` does not.
#[test]
fn cv_dry_basis_and_mayer_relation() {
    let humidity = (P::RelativeHumidity, 0.5);
    let w = at(P::HumidityRatio, T, PATM, humidity);
    let cv = at(P::IsochoricHeatCapacity, T, PATM, humidity);
    let cv_ha = at(P::IsochoricHeatCapacityHumidAir, T, PATM, humidity);
    let cp_ha = at(P::IsobaricHeatCapacityHumidAir, T, PATM, humidity);
    assert!(
        (cv - cv_ha * (1.0 + w)).abs() / cv < 1e-9,
        "cv != cv_ha*(1+W)"
    );

    let mayer_ref = R_BAR / m_ha_from_w(w);
    let mayer = cp_ha - cv_ha;
    let rel = (mayer - mayer_ref).abs() / mayer_ref;
    eprintln!(
        "cp_ha - cv_ha = {mayer:.4}, R_bar/M_ha = {mayer_ref:.4}, dev {:.4}%",
        rel * 100.0
    );
    assert!(rel < 1e-2, "Mayer relation off by {:.3}%", rel * 100.0);
}

/// METHODOLOGY. Internal energy `U`/`Uha` must satisfy the exact thermodynamic
/// identity `U = H − p·V` (dry-air basis) and `Uha = Hha − p·Vha` (humid-air
/// basis). This holds *regardless* of the shared absolute reference-state
/// offset in `H` and `U` (it cancels in `H − U = p·V`), so it is a strict
/// machine-precision oracle on the internal-energy implementation. Evaluated
/// at 25 °C / 1 atm / 50 % RH. Pass criterion: 1e-9 relative.
///
/// RESULTS (2026-07-16, this port). `U = -36499.60 J/kg`, `H − p·V =
/// -36499.60 J/kg` (agree to displayed precision); `Uha = -36140.85 J/kg`,
/// `Hha − p·Vha = -36140.85 J/kg`. Interpretation: `U = h̄ − p·v̄` on the molar
/// basis converts to both mass bases consistently with `H` and `V`; the large
/// negative absolute value is the known enthalpy-reference-offset artifact
/// (see module doc / `humid_air_reference.rs`), not a physics error — the
/// identity it must obey holds exactly.
#[test]
fn internal_energy_equals_h_minus_pv() {
    let humidity = (P::RelativeHumidity, 0.5);
    let h = at(P::Enthalpy, T, PATM, humidity);
    let v = at(P::Volume, T, PATM, humidity);
    let u = at(P::InternalEnergy, T, PATM, humidity);
    let hha = at(P::EnthalpyHumidAir, T, PATM, humidity);
    let vha = at(P::VolumeHumidAir, T, PATM, humidity);
    let uha = at(P::InternalEnergyHumidAir, T, PATM, humidity);
    eprintln!("U = {u:.4}, H - pV = {:.4}", h - PATM * v);
    eprintln!("Uha = {uha:.4}, Hha - pVha = {:.4}", hha - PATM * vha);
    assert!((u - (h - PATM * v)).abs() / u.abs() < 1e-9, "U != H - pV");
    assert!(
        (uha - (hha - PATM * vha)).abs() / uha.abs() < 1e-9,
        "Uha != Hha - pVha"
    );
}

/// METHODOLOGY. The humid-air-basis (per kg *humid* air) variants must equal
/// their dry-air-basis counterparts divided by `(1 + W)`, since `(1 + W)` is
/// the mass of humid air per kg of dry air: `Hha = H/(1+W)`, `Sha = S/(1+W)`,
/// `Vha = V/(1+W)`. An exact algebraic identity — a machine-precision oracle,
/// reference-state-independent. Evaluated at 25 °C / 1 atm / 50 % RH. Pass
/// criterion: 1e-12 relative.
///
/// RESULTS (2026-07-16, this port). `W = 0.009926`; `H = 50415.88`,
/// `Hha = 49920.35 = H/(1+W)`; `S = 203.41`, `Sha = 201.41 = S/(1+W)`;
/// `V = 0.857789`, `Vha = 0.849358 = V/(1+W)` (all SI, per kg). Interpretation:
/// the basis conversion is applied consistently across enthalpy, entropy, and
/// volume.
#[test]
fn humid_air_basis_conversions_exact() {
    let humidity = (P::RelativeHumidity, 0.5);
    let w = at(P::HumidityRatio, T, PATM, humidity);
    for (dry, wet) in [
        (P::Enthalpy, P::EnthalpyHumidAir),
        (P::Entropy, P::EntropyHumidAir),
        (P::Volume, P::VolumeHumidAir),
    ] {
        let x = at(dry, T, PATM, humidity);
        let xha = at(wet, T, PATM, humidity);
        let expected = x / (1.0 + w);
        eprintln!("{dry:?}: X = {x:.6}, Xha = {xha:.6}, X/(1+W) = {expected:.6}");
        assert!(
            (xha - expected).abs() / xha.abs() < 1e-12,
            "{wet:?} != {dry:?}/(1+W)"
        );
    }
}

/// METHODOLOGY. Compressibility factor `Z = p·v̄/(R̄T)` at 25 °C, checked for
/// the physically expected behaviour: (a) at 1 atm, moist air is very nearly
/// ideal but slightly *below* unity (real air is below its Boyle temperature,
/// negative second virial coefficient), literature `Z ≈ 0.9996`; (b) `Z`
/// departs further from unity at elevated pressure. `Z` is a direct re-
/// expression of the already-validated mixture molar volume (see
/// `humid_air_reference.rs`), so this confirms the new output wiring and the
/// physical trend. Pass criterion: `0.995 < Z(1 atm) < 1.0` and
/// `Z(10 bar) < Z(1 atm)`.
///
/// RESULTS (2026-07-16, this port). `Z(1 atm) = 0.99963`, matching the
/// literature real-gas value for ambient air (~0.9996) to 4 figures;
/// `Z(10 bar) = 0.99698 < Z(1 atm)`. Interpretation: the virial-truncated EOS
/// gives the correct sign and magnitude of the departure from ideal-gas
/// behaviour, and the pressure dependence is monotonic in the expected
/// direction.
#[test]
fn compressibility_factor_physical() {
    let humidity = (P::RelativeHumidity, 0.5);
    let z_atm = at(P::CompressibilityFactor, T, PATM, humidity);
    let z_10bar = at(P::CompressibilityFactor, T, 1.0e6, humidity);
    eprintln!("Z(1 atm) = {z_atm:.8}, Z(10 bar) = {z_10bar:.8}");
    assert!(
        (0.995..1.0).contains(&z_atm),
        "Z(1 atm) = {z_atm} out of expected near-unity band"
    );
    assert!(
        z_10bar < z_atm,
        "Z should decrease with pressure: {z_10bar} !< {z_atm}"
    );
    // Reconstruct Z from the molar volume V·M_ha/(1+W) to confirm the formula.
    let v = at(P::Volume, T, PATM, humidity);
    let w = at(P::HumidityRatio, T, PATM, humidity);
    let vbar = v * m_ha_from_w(w) / (1.0 + w);
    let z_check = PATM * vbar / (R_BAR * T);
    assert!(
        (z_atm - z_check).abs() < 1e-9,
        "Z {z_atm} != p*vbar/(R*T) {z_check}"
    );
}
