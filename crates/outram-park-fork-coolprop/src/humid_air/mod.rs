//! Humid (moist) air properties — the CoolProp `HAPropsSI` backend.
//!
//! Ported from CoolProp `HumidAirProp.cpp`, following ASHRAE RP-1485 /
//! Herrmann–Kretzschmar–Gatley (2009): humid air as a **real-gas mixture** of
//! dry air and water vapour, virial-corrected via [`virials`] and closed by
//! the saturation **enhancement factor** in [`saturation`].
//!
//! # Coverage
//!
//! - **Inputs**: `(T, p, X)` where `X` is any *one* of humidity ratio `W`,
//!   relative humidity `R`, water mole fraction `ψ_w`, dew-point temperature
//!   `T_dp`, or wet-bulb temperature `T_wb` — dry-bulb temperature, pressure,
//!   plus one humidity measure. `(T, p, {W|R})` is the triple most
//!   psychrometric/HVAC and FHR-secondary-loop-air calculations use;
//!   `(T, p, T_wb)` and `(T, p, T_dp)` invert the two solved-for temperatures
//!   below back to a state (see [`psi_w_from_dew_point`],
//!   [`psi_w_from_wet_bulb`]).
//! - **Outputs**: `T`, `p`, `ψ_w`, `W`, `R`, specific enthalpy `H`, specific
//!   entropy `S`, specific volume `V` (all per kg dry air), the water
//!   partial pressure `p_w`, and the wet-bulb temperature `T_wb` and
//!   dew-point temperature `T_dp`. Also the derived-energy family: specific
//!   internal energy `U`, the isobaric and isochoric heat capacities `cp`,
//!   `cv`, the compressibility factor `Z`, and the humid-air-basis (per kg
//!   *humid* air) variants of the mass-specific quantities: `Hha`, `Sha`,
//!   `Vha`, `Uha`, `cp_ha`, `cv_ha`.
//! - **Range**: liquid-water branch only, `T > 273.16 K` (the IAPWS triple
//!   point) — the ice-sublimation branch is not ported (see [`saturation`]).
//!
//! **Not implemented**: any input triple that is not
//! `(T, p, {W|R|ψ_w|T_dp|T_wb})`. [`ha_props`] returns
//! [`HumidAirError::UnsupportedInputs`] for these — never a wrong number.
//!
//! # Caveats (read before relying on `S`, or on `T_wb`/`T_dp` near their
//! domain edges)
//!
//! - **`S`'s absolute value is on a different reference-state footing than
//!   CoolProp's own.** This port derives `S` from Gibbs' theorem plus the
//!   already-verified `Cp(T)` polynomials, anchored to an explicit ASHRAE
//!   zero-point convention (`s_a = 0` at 0 °C/1 atm; `s_w` anchored via
//!   Clausius-Clapeyron to the same convention `H` uses) — it does **not**
//!   replicate CoolProp's own `ensure_ref_offsets`/`calc_ideal_gas_alpha0`
//!   calibration (which uses the real ideal-gas Helmholtz term and a
//!   different set of historical reference points). `dS/dT = Cp/T` is
//!   verified to hold (a rigorous, model-independent thermodynamic identity —
//!   see `tests/humid_air_reference.rs`), so `S`'s *temperature dependence*
//!   is trustworthy; its *absolute value* likely carries a small offset
//!   (comparable in size to `H`'s own known ~6.6 kJ/kg mixture offset against
//!   ASHRAE tables — see that test file) that has not been independently
//!   confirmed against an external reference. Revisit if a CoolProp/`rfluids`
//!   oracle (`op-kbc.3`) becomes available.
//! - **Liquid-branch restriction is transitive.** Any solve whose dew point
//!   would fall below 273.16 K errors (`OutOfRange`) rather than extrapolate
//!   into the unported ice-sublimation branch — this includes `T_wb`/`T_dp`
//!   both as outputs *and* as inputs, and it is a real, easy-to-hit
//!   restriction: e.g. `T = 10 °C, R = 0.4` already has a sub-freezing dew
//!   point and errors.
//! - **`T_wb` inversion at the exact saturation boundary needed a targeted
//!   fix.** [`psi_w_from_wet_bulb`]'s general bisection is parameterised by a
//!   trial dew point rather than `ψ_w` directly (see its own doc comment for
//!   why bisecting `ψ_w` from `0` breaks the inner dew-point solve at bone-dry
//!   air) — but that bracket itself degenerates when `T_wb = T` exactly (100%
//!   RH), which a caller can easily hit by chaining this crate's own forward
//!   solve (`T_wb` computed to within the forward secant's convergence noise,
//!   ~1e-7 K, can land on *either* side of `T`). Handled with a direct
//!   `ψ_w = ψ_{w,s}(T, p)` short-circuit rather than trusting the bisection
//!   there; the general bounds check also had to be loosened from a `1e-9` K
//!   to a `1e-4` K tolerance for the same reason. Caught by round-tripping
//!   `(T,p,R) → T_wb → (T,p,T_wb) → R` at `R = 1` during verification — see
//!   `tests/humid_air_reference.rs`.
//!
//! # Units
//!
//! Mass-based SI throughout, **per kilogram of _dry_ air** for the extensive
//! specific properties (`H`, `V`), matching `HAPropsSI`:
//! temperature K, pressure Pa, humidity ratio `W` kg-water/kg-dry-air,
//! relative humidity `R` \[0, 1\], `H` J/kg-dry-air, `V` m³/kg-dry-air.

pub mod saturation;
pub mod virials;

use crate::fluid::Fluid;
use crate::vle;
use saturation::{MM_AIR, MM_WATER, R_BAR};

/// A humid-air intensive property — usable as either an input key or an output
/// selector, mirroring `HAPropsSI`'s string parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumidAirParam {
    /// Dry-bulb temperature `T` \[K\].
    TDryBulb,
    /// Pressure `p` \[Pa\].
    Pressure,
    /// Humidity ratio `W` \[kg water / kg dry air\] (CoolProp `W`/`Omega`).
    HumidityRatio,
    /// Relative humidity `R` \[0, 1\] (CoolProp `R`/`RH`).
    RelativeHumidity,
    /// Water-vapour mole fraction `ψ_w` \[-\] (CoolProp `psi_w`/`Y`).
    WaterMoleFraction,
    /// Wet-bulb temperature `T_wb` \[K\] (CoolProp `B`/`Twb`). Usable as
    /// either an input or an output (see the module doc's caveats section
    /// for a sharp edge at the exact saturation boundary).
    TWetBulb,
    /// Dew-point temperature `T_dp` \[K\] (CoolProp `D`/`Tdp`). Usable as
    /// either an input or an output.
    TDewPoint,
    /// Mixture specific enthalpy per kg dry air \[J/kg\] (CoolProp `H`/`Hda`).
    Enthalpy,
    /// Mixture specific entropy per kg dry air \[J/(kg·K)\] (CoolProp
    /// `S`/`Sda`). See the module doc's caveats section — this port's
    /// absolute reference-state convention differs from CoolProp's own.
    Entropy,
    /// Mixture specific volume per kg dry air \[m³/kg\] (CoolProp `V`/`Vda`).
    Volume,
    /// Partial pressure of water vapour `p_w` \[Pa\] (CoolProp `P_w`).
    WaterPartialPressure,
    /// Mixture specific enthalpy per kg **humid** air \[J/kg\] (CoolProp
    /// `Hha`). Equals [`Enthalpy`](Self::Enthalpy) divided by `(1 + W)`.
    EnthalpyHumidAir,
    /// Mixture specific entropy per kg **humid** air \[J/(kg·K)\] (CoolProp
    /// `Sha`). Equals [`Entropy`](Self::Entropy) divided by `(1 + W)`. Carries
    /// the same absolute-reference caveat as [`Entropy`](Self::Entropy) — see
    /// the module doc.
    EntropyHumidAir,
    /// Mixture specific volume per kg **humid** air \[m³/kg\] (CoolProp `Vha`).
    /// Equals [`Volume`](Self::Volume) divided by `(1 + W)`.
    VolumeHumidAir,
    /// Mixture specific internal energy per kg **dry** air \[J/kg\] (CoolProp
    /// `U`/`Uda`). `u = h − p·v` on the mixture molar basis, then converted
    /// per kg dry air the same way [`Enthalpy`](Self::Enthalpy) is.
    InternalEnergy,
    /// Mixture specific internal energy per kg **humid** air \[J/kg\] (CoolProp
    /// `Uha`). Equals [`InternalEnergy`](Self::InternalEnergy) divided by
    /// `(1 + W)`.
    InternalEnergyHumidAir,
    /// Isobaric (constant-`p`) specific heat capacity per kg **dry** air
    /// \[J/(kg·K)\] (CoolProp `C`/`cp`). Central finite difference of the
    /// mixture molar enthalpy `h̄` with respect to `T` at fixed `p` and `ψ_w`
    /// (matching CoolProp's own `Cp` implementation), converted to a dry-air
    /// mass basis. Equals [`IsobaricHeatCapacityHumidAir`](Self::IsobaricHeatCapacityHumidAir)
    /// times `(1 + W)`.
    IsobaricHeatCapacity,
    /// Isobaric (constant-`p`) specific heat capacity per kg **humid** air
    /// \[J/(kg·K)\] (CoolProp `Cha`/`cp_ha`).
    IsobaricHeatCapacityHumidAir,
    /// Isochoric (constant-`v`) specific heat capacity per kg **dry** air
    /// \[J/(kg·K)\] (CoolProp `CV`). Central finite difference of the mixture
    /// molar internal energy `ū` with respect to `T` at fixed molar volume
    /// `v̄` and `ψ_w` (the pressure is re-solved from the EOS at each `T`,
    /// matching CoolProp), converted to a dry-air mass basis. Equals
    /// [`IsochoricHeatCapacityHumidAir`](Self::IsochoricHeatCapacityHumidAir)
    /// times `(1 + W)`.
    IsochoricHeatCapacity,
    /// Isochoric (constant-`v`) specific heat capacity per kg **humid** air
    /// \[J/(kg·K)\] (CoolProp `CVha`/`cv_ha`).
    IsochoricHeatCapacityHumidAir,
    /// Compressibility factor `Z = p·v̄ / (R̄·T)` \[-\] (CoolProp `Z`), where
    /// `v̄` is the mixture molar volume \[m³/mol\] and `R̄ = 8.314472
    /// J/(mol·K)`. Dimensionless; identical on either mass basis.
    CompressibilityFactor,
}

/// A fully-resolved humid-air state (all properties per kg **dry** air, SI).
///
/// Produced once the three input constraints are solved for the base
/// `(T, p, ψ_w)` triple; every [`HumidAirParam`] is then a direct read.
/// See the module doc's caveats section for `entropy`'s absolute-reference
/// caveat.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HumidAirState {
    /// Dry-bulb temperature \[K\].
    pub t_dry_bulb: f64,
    /// Total pressure \[Pa\].
    pub pressure: f64,
    /// Water-vapour mole fraction `ψ_w` \[-\].
    pub water_mole_fraction: f64,
    /// Humidity ratio `W` \[kg water / kg dry air\].
    pub humidity_ratio: f64,
    /// Relative humidity `R` \[0, 1\].
    pub relative_humidity: f64,
    /// Specific enthalpy \[J/kg dry air\].
    pub enthalpy: f64,
    /// Specific entropy \[J/(kg dry air·K)\]. See the module doc's caveats
    /// section — temperature dependence is verified, absolute value not yet
    /// cross-checked against an external reference.
    pub entropy: f64,
    /// Specific volume \[m³/kg dry air\].
    pub volume: f64,
}

/// Failure modes of a humid-air solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumidAirError {
    /// The requested input set is not a solvable/supported triple.
    UnsupportedInputs,
    /// An inner iteration (the enhancement factor, molar-volume, dew-point,
    /// or wet-bulb solve) did not converge.
    NonConvergent,
    /// An input is out of the valid range (`T ≤ 273.16 K`, `p ≤ 0`, …).
    OutOfRange,
}

/// One `(parameter, value)` input constraint for [`ha_props`].
pub type HaInput = (HumidAirParam, f64);

/// Evaluate a humid-air `output` property from three input constraints — the
/// typed analogue of CoolProp's
/// `HAPropsSI(output, k1, v1, k2, v2, k3, v3)`.
///
/// Humid air has three degrees of freedom, so exactly three inputs are
/// required. Only `(T, p, {W|R|ψ_w|T_dp|T_wb})` triples are supported as
/// *inputs* (see the module doc).
///
/// # Errors
/// [`HumidAirError`] if the input triple or requested output is unsupported,
/// an input is out of range, or an inner solve fails to converge.
pub fn ha_props(
    output: HumidAirParam,
    in1: HaInput,
    in2: HaInput,
    in3: HaInput,
) -> Result<f64, HumidAirError> {
    let state = solve_state(in1, in2, in3)?;
    // T_wb/T_dp are iterative solves in their own right, not plain fields of
    // the base state -- computed on demand so every other output (the
    // overwhelming majority of calls) doesn't pay their cost.
    if matches!(output, HumidAirParam::TDewPoint) {
        return dew_point_temperature(state.t_dry_bulb, state.pressure, state.water_mole_fraction);
    }
    if matches!(output, HumidAirParam::TWetBulb) {
        return wet_bulb_temperature(state.t_dry_bulb, state.pressure, state.water_mole_fraction);
    }
    // The derived-energy family (cp/cv/Z/U + humid-air-basis variants) is not a
    // plain field of the base state: Z and U re-solve the molar volume, and the
    // heat capacities finite-difference it -- all fallible -- so they are
    // dispatched here rather than in the infallible field-read match below.
    let (t, p, psi_w, w) = (
        state.t_dry_bulb,
        state.pressure,
        state.water_mole_fraction,
        state.humidity_ratio,
    );
    match output {
        HumidAirParam::InternalEnergy => {
            return Ok(mass_internal_energy_per_kg_dry_air(t, p, psi_w)?)
        }
        HumidAirParam::InternalEnergyHumidAir => {
            return Ok(mass_internal_energy_per_kg_humid_air(t, p, psi_w)?)
        }
        HumidAirParam::IsobaricHeatCapacity => return Ok(cp_ha_mass(t, p, psi_w)? * (1.0 + w)),
        HumidAirParam::IsobaricHeatCapacityHumidAir => return Ok(cp_ha_mass(t, p, psi_w)?),
        HumidAirParam::IsochoricHeatCapacity => return Ok(cv_ha_mass(t, p, psi_w)? * (1.0 + w)),
        HumidAirParam::IsochoricHeatCapacityHumidAir => return Ok(cv_ha_mass(t, p, psi_w)?),
        HumidAirParam::CompressibilityFactor => return Ok(compressibility_factor(t, p, psi_w)?),
        _ => {}
    }
    Ok(match output {
        HumidAirParam::TDryBulb => state.t_dry_bulb,
        HumidAirParam::Pressure => state.pressure,
        HumidAirParam::HumidityRatio => state.humidity_ratio,
        HumidAirParam::RelativeHumidity => state.relative_humidity,
        HumidAirParam::WaterMoleFraction => state.water_mole_fraction,
        HumidAirParam::Enthalpy => state.enthalpy,
        HumidAirParam::Entropy => state.entropy,
        HumidAirParam::Volume => state.volume,
        HumidAirParam::WaterPartialPressure => state.water_mole_fraction * state.pressure,
        // Humid-air-basis (per kg humid air) variants: the dry-air-basis value
        // divided by (1 + W), since (1 + W) = kg humid air / kg dry air.
        HumidAirParam::EnthalpyHumidAir => state.enthalpy / (1.0 + state.humidity_ratio),
        HumidAirParam::EntropyHumidAir => state.entropy / (1.0 + state.humidity_ratio),
        HumidAirParam::VolumeHumidAir => state.volume / (1.0 + state.humidity_ratio),
        HumidAirParam::TWetBulb
        | HumidAirParam::TDewPoint
        | HumidAirParam::InternalEnergy
        | HumidAirParam::InternalEnergyHumidAir
        | HumidAirParam::IsobaricHeatCapacity
        | HumidAirParam::IsobaricHeatCapacityHumidAir
        | HumidAirParam::IsochoricHeatCapacity
        | HumidAirParam::IsochoricHeatCapacityHumidAir
        | HumidAirParam::CompressibilityFactor => unreachable!(),
    })
}

/// Solve the base humid-air state from three input constraints.
///
/// Recognises `(T, p, {W|R|ψ_w|T_dp|T_wb})` (in any order), inverts whichever
/// humidity measure was given to the water mole fraction `ψ_w`, then fills
/// every field of [`HumidAirState`].
fn solve_state(in1: HaInput, in2: HaInput, in3: HaInput) -> Result<HumidAirState, HumidAirError> {
    let inputs = [in1, in2, in3];
    let t = find(&inputs, HumidAirParam::TDryBulb).ok_or(HumidAirError::UnsupportedInputs)?;
    let p = find(&inputs, HumidAirParam::Pressure).ok_or(HumidAirError::UnsupportedInputs)?;
    if !(t.is_finite() && t > saturation::T_TRIPLE) {
        return Err(HumidAirError::OutOfRange);
    }
    if !(p.is_finite() && p > 0.0) {
        return Err(HumidAirError::OutOfRange);
    }

    let epsilon = MM_WATER / MM_AIR;
    let psi_w = if let Some(w) = find(&inputs, HumidAirParam::HumidityRatio) {
        w / (epsilon + w)
    } else if let Some(r) = find(&inputs, HumidAirParam::RelativeHumidity) {
        r * saturation_mole_fraction(t, p)?
    } else if let Some(psi) = find(&inputs, HumidAirParam::WaterMoleFraction) {
        psi
    } else if let Some(tdp) = find(&inputs, HumidAirParam::TDewPoint) {
        psi_w_from_dew_point(p, tdp)?
    } else if let Some(twb) = find(&inputs, HumidAirParam::TWetBulb) {
        psi_w_from_wet_bulb(t, p, twb)?
    } else {
        return Err(HumidAirError::UnsupportedInputs);
    };
    if !(psi_w.is_finite() && (0.0..1.0).contains(&psi_w)) {
        return Err(HumidAirError::OutOfRange);
    }

    let w = humidity_ratio_from_psi_w(psi_w);
    let psi_ws = saturation_mole_fraction(t, p)?;
    let r = psi_w / psi_ws;

    let vbar = molar_volume(t, p, psi_w)?;
    let hbar = molar_enthalpy(t, psi_w, vbar);
    let sbar = molar_entropy(t, p, psi_w, vbar);
    let m_ha = MM_WATER * psi_w + (1.0 - psi_w) * MM_AIR; // kg_ha / mol_ha

    Ok(HumidAirState {
        t_dry_bulb: t,
        pressure: p,
        water_mole_fraction: psi_w,
        humidity_ratio: w,
        relative_humidity: r,
        enthalpy: hbar * (1.0 + w) / m_ha,
        entropy: sbar * (1.0 + w) / m_ha,
        volume: vbar * (1.0 + w) / m_ha,
    })
}

fn find(inputs: &[HaInput; 3], key: HumidAirParam) -> Option<f64> {
    inputs.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Humidity ratio `W` \[kg water / kg dry air\] from the water mole fraction
/// `psi_w` \[-\]. `W = ψ_w·(M_w/M_a) / (1 − ψ_w)` (RP-1485 Eq. 3.2).
fn humidity_ratio_from_psi_w(psi_w: f64) -> f64 {
    psi_w * (MM_WATER / MM_AIR) / (1.0 - psi_w)
}

/// Saturation water mole fraction `ψ_{w,s}(T, p)` \[-\] including the
/// enhancement factor `f` (RP-1485 Eq. 3.24): `ψ_{w,s} = f(T,p) · p_ws(T)/p`.
fn saturation_mole_fraction(t: f64, p: f64) -> Result<f64, HumidAirError> {
    let p_ws = saturation::p_ws(t)?;
    let f = saturation::enhancement_factor(t, p)?;
    Ok(f * p_ws / p)
}

/// Molar volume of humid air `v̄(T, p, ψ_w)` \[m³/mol_ha\] — secant solve of
/// the virial-truncated real-gas equation of state
/// `p = R̄T/v̄ · (1 + B_m/v̄ + C_m/v̄²)` (RP-1485 Eq. 3.3).
fn molar_volume(t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    let bm = virials::b_mix(t, psi_w);
    let cm = virials::c_mix(t, psi_w);
    let residual = |v: f64| (p - R_BAR * t / v * (1.0 + bm / v + cm / (v * v))) / p;

    let v0 = R_BAR * t / p;
    let mut x1 = v0;
    let mut x2 = v0 + 1e-6;
    let mut y1 = residual(x1);
    let mut v = x2;
    for _ in 0..100 {
        let y2 = residual(x2);
        if (y2 - y1).abs() < 1e-300 {
            break;
        }
        let x3 = x2 - y2 / (y2 - y1) * (x2 - x1);
        let change = (y2 / (y2 - y1) * (x2 - x1)).abs();
        x1 = x2;
        y1 = y2;
        x2 = x3;
        v = x2;
        if change < 1e-11 {
            break;
        }
    }
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(HumidAirError::NonConvergent)
    }
}

/// Ideal-gas molar enthalpy of water vapour \[J/mol\] — CoolProp's simplified
/// polynomial correlation (`FlagUseIdealGasEnthalpyCorrelations`), used in
/// place of the full IAPWS-95 `α⁰` + reference-state-offset evaluation.
fn ideal_gas_molar_enthalpy_water(t: f64) -> f64 {
    2.7030251618e-03 * t * t + 3.1994361015e+01 * t + 3.6123174929e+04
}

/// Ideal-gas molar enthalpy of dry air \[J/mol\] — CoolProp's simplified
/// polynomial correlation (`FlagUseIdealGasEnthalpyCorrelations`).
fn ideal_gas_molar_enthalpy_air(t: f64) -> f64 {
    9.2486716590e-04 * t * t + 2.8557221776e+01 * t - 7.8616129429e+03
}

/// Mixture molar enthalpy `h̄(T, ψ_w, v̄)` \[J/mol_ha\] (RP-1485 Eq. 3.4):
/// mole-fraction-weighted ideal-gas enthalpies plus the virial correction.
fn molar_enthalpy(t: f64, psi_w: f64, vbar: f64) -> f64 {
    let hbar_w = ideal_gas_molar_enthalpy_water(t);
    let hbar_a = ideal_gas_molar_enthalpy_air(t);
    let bm = virials::b_mix(t, psi_w);
    let d_bm_dt = virials::d_b_mix_dt(t, psi_w);
    let cm = virials::c_mix(t, psi_w);
    let d_cm_dt = virials::d_c_mix_dt(t, psi_w);
    (1.0 - psi_w) * hbar_a
        + psi_w * hbar_w
        + R_BAR * t * ((bm - t * d_bm_dt) / vbar + (cm - t / 2.0 * d_cm_dt) / (vbar * vbar))
}

/// Reference state for this port's ideal-gas entropy convention: 0 °C
/// (273.15 K, the ASHRAE/ITS-90 Celsius-scale zero — not the 273.16 K IAPWS
/// triple point, though the two differ by only 0.01 K), 101 325 Pa. See the
/// module doc's entropy caveats section for why this specific convention was
/// chosen and what it does *not* guarantee.
const T0_ENTROPY_REF: f64 = 273.15;
const P0_ENTROPY_REF: f64 = 101_325.0;

/// Ideal-gas molar entropy of dry air `s_a(T, p)` \[J/(mol·K)\], referenced
/// to `s_a = 0` at `(T_0, p_0)` — an arbitrary but explicit and
/// self-consistent choice (entropy is only ever defined up to an additive
/// constant; see the module doc). `Cp_a(T) = 2·9.2486716590×10⁻⁴·T +
/// 2.8557221776×10¹` is the exact derivative of
/// [`ideal_gas_molar_enthalpy_air`]'s polynomial, so `dS/dT = Cp_a/T` holds
/// by construction, not by coincidence — **only** if the pressure dependence
/// is `-R·ln(p/p_0)` (Gibbs' theorem: ideal-gas-mixture components are each
/// evaluated at the *total* pressure `p`, with the mixing correction added
/// separately in [`molar_entropy`], not the component's own partial
/// pressure). An earlier version of this function used `+R·ln(v̄_a/v̄_{a,0})`
/// (dry air's own virial-corrected molar volume) instead — CoolProp's actual
/// `IdealGasMolarEntropy_Air` does use a volume argument, but that is because
/// it evaluates the *real* ideal-gas Helmholtz term (density-native by
/// construction), not a `Cp`-integral. Reusing a volume argument here, where
/// `Cp` is explicitly the *constant-pressure* heat capacity, silently
/// introduced a second, spurious `T`-dependence (`v̄_a ∝ T` at fixed `p`)
/// alongside the already-correct `b·ln(T/T_0)` term from the `Cp`-integral,
/// breaking `dS/dT = Cp/T` by ~28% at 25 °C. Caught by the thermodynamic
/// consistency check in `tests/humid_air_reference.rs` before this was ever
/// committed as correct.
fn ideal_gas_molar_entropy_air(t: f64, p: f64) -> f64 {
    let a = 9.2486716590e-04;
    let b = 2.8557221776e+01;
    2.0 * a * (t - T0_ENTROPY_REF) + b * (t / T0_ENTROPY_REF).ln()
        - R_BAR * (p / P0_ENTROPY_REF).ln()
}

/// Ideal-gas molar entropy of water vapour `s_w(T, p)` \[J/(mol·K)\].
///
/// Unlike dry air, water's reference constant is not arbitrary here: it is
/// anchored via the Clausius-Clapeyron relation (`Δs = Δh/T` for a
/// constant-`T,p` phase change) to the *same* ASHRAE zero-point convention
/// already implicit in [`ideal_gas_molar_enthalpy_water`] — saturated liquid
/// water's enthalpy (and, by the same convention, entropy) is zero at the
/// triple point, so saturated water *vapour*'s entropy there equals its
/// enthalpy there divided by `T_0` (confirmed consistent: this port's
/// `ideal_gas_molar_enthalpy_water(273.15 K) ≈ 45 064 J/mol ≈ 2501 kJ/kg`,
/// matching the standard tabulated latent heat of vaporization of water at
/// 0 °C to 3 figures). `Cp_w(T) = 2·2.7030251618×10⁻³·T + 3.1994361015×10¹`
/// is the exact derivative of that same enthalpy polynomial, so `dS/dT =
/// Cp_w/T` holds by construction here too, using the *total* pressure `p`
/// for the same Gibbs'-theorem reason [`ideal_gas_molar_entropy_air`]'s doc
/// comment explains.
fn ideal_gas_molar_entropy_water(t: f64, p: f64) -> f64 {
    let a = 2.7030251618e-03;
    let b = 3.1994361015e+01;
    let s_w0 = ideal_gas_molar_enthalpy_water(T0_ENTROPY_REF) / T0_ENTROPY_REF;
    2.0 * a * (t - T0_ENTROPY_REF) + b * (t / T0_ENTROPY_REF).ln()
        - R_BAR * (p / P0_ENTROPY_REF).ln()
        + s_w0
}

/// Mixture molar entropy `s̄(T, p, ψ_w, v̄)` \[J/(mol_ha·K)\]: mole-fraction-
/// weighted pure-component ideal-gas entropies (Gibbs' theorem — each
/// evaluated at the mixture's total `T, p`), the same virial correction
/// [`molar_enthalpy`] uses (`(B+T·dB/dT)/v̄ + …`, this time without the extra
/// factor of `T` since it is already an entropy not an enthalpy term), and
/// the ideal entropy of mixing `−R·[(1−ψ_w)ln(1−ψ_w) + ψ_w·ln(ψ_w)]`.
fn molar_entropy(t: f64, p: f64, psi_w: f64, vbar: f64) -> f64 {
    let bm = virials::b_mix(t, psi_w);
    let d_bm_dt = virials::d_b_mix_dt(t, psi_w);
    let cm = virials::c_mix(t, psi_w);
    let d_cm_dt = virials::d_c_mix_dt(t, psi_w);
    let virial_term =
        R_BAR * ((bm + t * d_bm_dt) / vbar + (cm + t * d_cm_dt) / (2.0 * vbar * vbar));

    let mixing_term = if psi_w > 0.0 && psi_w < 1.0 {
        -R_BAR * ((1.0 - psi_w) * (1.0 - psi_w).ln() + psi_w * psi_w.ln())
    } else {
        0.0
    };

    let sbar_a = ideal_gas_molar_entropy_air(t, p);
    let sbar_w = ideal_gas_molar_entropy_water(t, p);

    (1.0 - psi_w) * sbar_a + psi_w * sbar_w - virial_term + mixing_term
}

/// Molar mass of humid air `M_ha` \[kg_ha / mol_ha\]: the mole-fraction-
/// weighted mean of the water and dry-air molar masses (RP-1485). Used to
/// convert every molar property `X̄` \[·/mol_ha\] to a per-kg-humid-air value
/// (`X̄ / M_ha`) or a per-kg-dry-air value (`X̄·(1 + W) / M_ha`).
fn m_ha(psi_w: f64) -> f64 {
    MM_WATER * psi_w + (1.0 - psi_w) * MM_AIR
}

/// Forward EOS pressure `p(T, v̄, ψ_w)` \[Pa\] — the virial-truncated real-gas
/// equation of state `p = R̄T/v̄ · (1 + B_m/v̄ + C_m/v̄²)` (RP-1485 Eq. 3.3)
/// evaluated *forward* (given `v̄`, return `p`), the inverse of the secant
/// solve in [`molar_volume`]. Used by the constant-volume heat-capacity finite
/// difference, where the pressure must be re-evaluated as `T` changes at fixed
/// `v̄` (CoolProp's `Pressure`).
fn eos_pressure(t: f64, vbar: f64, psi_w: f64) -> f64 {
    let bm = virials::b_mix(t, psi_w);
    let cm = virials::c_mix(t, psi_w);
    R_BAR * t / vbar * (1.0 + bm / vbar + cm / (vbar * vbar))
}

/// Mixture molar internal energy `ū = h̄ − p·v̄` \[J/mol_ha\] (CoolProp
/// `MolarInternalEnergy`). `p` is the pressure consistent with the supplied
/// `v̄` — either the state pressure, or (in the `c_v` finite difference) the
/// EOS pressure at the perturbed temperature and fixed volume.
fn molar_internal_energy(t: f64, psi_w: f64, vbar: f64, p: f64) -> f64 {
    molar_enthalpy(t, psi_w, vbar) - p * vbar
}

/// Specific internal energy per kg **dry** air \[J/kg\] (CoolProp
/// `MassInternalEnergy_per_kgda`): `ū·(1 + W) / M_ha`.
fn mass_internal_energy_per_kg_dry_air(t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    let vbar = molar_volume(t, p, psi_w)?;
    let ubar = molar_internal_energy(t, psi_w, vbar, p);
    let w = humidity_ratio_from_psi_w(psi_w);
    Ok(ubar * (1.0 + w) / m_ha(psi_w))
}

/// Specific internal energy per kg **humid** air \[J/kg\] (CoolProp
/// `MassInternalEnergy_per_kgha`): `ū / M_ha`.
fn mass_internal_energy_per_kg_humid_air(t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    let vbar = molar_volume(t, p, psi_w)?;
    let ubar = molar_internal_energy(t, psi_w, vbar, p);
    Ok(ubar / m_ha(psi_w))
}

/// Isobaric specific heat capacity per kg **humid** air `c_{p,ha}(T, p, ψ_w)`
/// \[J/(kg·K)\] (CoolProp `GIVEN_CPHA`).
///
/// Central finite difference of the mixture molar enthalpy `h̄` with respect
/// to `T` at fixed `p` and `ψ_w`, `c̄_p = (h̄(T+dT) − h̄(T−dT)) / (2·dT)` with
/// `dT = 10⁻³ K`, divided by `M_ha`. The molar volume is re-solved at each
/// perturbed temperature (constant pressure), matching CoolProp exactly.
fn cp_ha_mass(t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    let dt = 1e-3;
    let vbar_1 = molar_volume(t - dt, p, psi_w)?;
    let vbar_2 = molar_volume(t + dt, p, psi_w)?;
    let hbar_1 = molar_enthalpy(t - dt, psi_w, vbar_1);
    let hbar_2 = molar_enthalpy(t + dt, psi_w, vbar_2);
    let cp_bar = (hbar_2 - hbar_1) / (2.0 * dt); // [J/mol_ha/K]
    Ok(cp_bar / m_ha(psi_w))
}

/// Isochoric specific heat capacity per kg **humid** air `c_{v,ha}(T, p, ψ_w)`
/// \[J/(kg·K)\] (CoolProp `GIVEN_CVHA`).
///
/// Central finite difference of the mixture molar internal energy `ū` with
/// respect to `T` at *fixed molar volume* `v̄` (the value solved at the base
/// `(T, p, ψ_w)`) and fixed `ψ_w`, with `dT = 10⁻³ K`. At each perturbed
/// temperature the pressure is re-evaluated from the EOS at the held-constant
/// `v̄` (via [`eos_pressure`]), matching CoolProp exactly, then divided by
/// `M_ha`.
fn cv_ha_mass(t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    let dt = 1e-3;
    let vbar = molar_volume(t, p, psi_w)?; // held constant
    let p_1 = eos_pressure(t - dt, vbar, psi_w);
    let p_2 = eos_pressure(t + dt, vbar, psi_w);
    let ubar_1 = molar_internal_energy(t - dt, psi_w, vbar, p_1);
    let ubar_2 = molar_internal_energy(t + dt, psi_w, vbar, p_2);
    let cv_bar = (ubar_2 - ubar_1) / (2.0 * dt); // [J/mol_ha/K]
    Ok(cv_bar / m_ha(psi_w))
}

/// Compressibility factor `Z = p·v̄ / (R̄·T)` \[-\] (CoolProp
/// `GIVEN_COMPRESSIBILITY_FACTOR`), with `v̄` the mixture molar volume
/// \[m³/mol_ha\] and `R̄ = 8.314472 J/(mol·K)` ([`R_BAR`]). Dimensionless and
/// basis-independent (the same number per kg dry or per kg humid air).
fn compressibility_factor(t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    let vbar = molar_volume(t, p, psi_w)?;
    Ok(p * vbar / (R_BAR * t))
}

/// Dew-point temperature `T_dp` \[K\] (CoolProp `HumidAirProp.cpp`'s
/// `DewpointTemperature`): the temperature at which, cooled at constant
/// pressure and constant water mole fraction `ψ_w`, the air would just reach
/// saturation (`ψ_w = ψ_{w,s}(T_dp, p)`).
///
/// Solved by secant iteration on the same residual CoolProp uses
/// (`ψ_w · p − p_{ws}(T_dp)·f(T_dp, p)`, here expressed via
/// [`saturation_mole_fraction`]), seeded from this crate's own IAPWS-95 water
/// saturation-temperature solve ([`crate::vle::saturation_temperature`]) in
/// place of CoolProp's IF97 `Tsat97` — both are full Helmholtz-EOS saturation
/// solves and agree to IAPWS tolerances (same substitution [`saturation::p_ws`]
/// already makes). Liquid-water branch only, matching the rest of this module:
/// returns [`HumidAirError::OutOfRange`] if the solve would cross the triple
/// point (273.16 K). Takes `t` (dry-bulb temperature) for call-site symmetry
/// with [`wet_bulb_temperature`], matching CoolProp's own signature — the dew
/// point depends only on `p` and `ψ_w`, not on the current dry-bulb `t`.
fn dew_point_temperature(_t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    if (1.0 - psi_w) < 1e-16 {
        // Dry air: no dew point exists.
        return Err(HumidAirError::OutOfRange);
    }
    let p_w = psi_w * p;
    // CoolProp's triple-point pressure of water, Pa.
    let t0 = if p_w > 611.654_724_163_794_4 {
        vle::saturation_temperature(Fluid::Water, p).ok_or(HumidAirError::NonConvergent)? - 1.0
    } else {
        268.0
    };

    let residual =
        |tdp: f64| -> Result<f64, HumidAirError> { Ok(psi_w - saturation_mole_fraction(tdp, p)?) };

    let mut x1 = t0;
    let mut x2 = t0 + 0.1;
    let mut y1 = residual(x1)?;
    let mut tdp = x2;
    for _ in 0..100 {
        let y2 = residual(x2)?;
        if (y2 - y1).abs() < 1e-300 {
            break;
        }
        let x3 = x2 - y2 / (y2 - y1) * (x2 - x1);
        x1 = x2;
        y1 = y2;
        x2 = x3;
        tdp = x2;
        if y2.abs() < 1e-5 {
            break;
        }
    }
    if tdp.is_finite() && tdp > saturation::T_TRIPLE {
        Ok(tdp)
    } else {
        Err(HumidAirError::OutOfRange)
    }
}

/// Wet-bulb (thermodynamic, adiabatic-saturation) temperature `T_wb` \[K\]
/// (CoolProp `HumidAirProp.cpp`'s `WetbulbTemperature`/`WetBulbSolver`): the
/// temperature at which the enthalpy gained evaporating water into the air
/// adiabatically, down to saturation, balances the enthalpy of the
/// evaporated liquid water. CoolProp's own residual (`LHS − RHS` below) is
/// ported as-is; only the root-finding differs.
///
/// `T_wb` always lies between the dew point and the dry-bulb temperature
/// (standard psychrometric fact — they coincide only at saturation, `R = 1`).
/// That gives a bracket that is always valid and comfortably clear of the
/// water-saturation-temperature edge case CoolProp's own upstream comments
/// document needing special-case handling for (its Brent bracket can
/// straddle `T_sat(p)`, see the `#2255`/`#2690` history in
/// `HumidAirProp.cpp`) — this port sidesteps that by construction instead of
/// replicating the clamping logic. Bisection on `[T_dp, T]`; returns
/// [`HumidAirError::NonConvergent`] if the two ends don't bracket a root
/// (should not happen for physically valid inputs).
fn wet_bulb_temperature(t: f64, p: f64, psi_w: f64) -> Result<f64, HumidAirError> {
    let w = humidity_ratio_from_psi_w(psi_w);
    let vbar = molar_volume(t, p, psi_w)?;
    let m_ha = MM_WATER * psi_w + (1.0 - psi_w) * MM_AIR;
    let lhs = molar_enthalpy(t, psi_w, vbar) * (1.0 + w) / m_ha;

    let residual = |twb: f64| -> Result<f64, HumidAirError> {
        let psi_ws = saturation_mole_fraction(twb, p)?;
        let w_s = humidity_ratio_from_psi_w(psi_ws);
        let m_ha_wb = MM_WATER * psi_ws + (1.0 - psi_ws) * MM_AIR;
        let vbar_wb = molar_volume(twb, p, psi_ws)?;
        // Liquid water enthalpy at (T_wb, p): `flash::state_pt` only reaches
        // the vapour/supercritical branch (see its own doc), which is the
        // wrong branch for the subcooled liquid water present at a typical
        // T_wb. Use the saturated-liquid density at T_wb instead -- liquid
        // enthalpy is only weakly pressure-dependent, so this is an excellent
        // approximation to the true compressed-liquid value at the (usually
        // near-atmospheric) total pressure `p`, and it reuses the same
        // saturated-liquid pattern `saturation::vbar_ws` already relies on.
        let rho_liquid = vle::saturation_at_temperature(Fluid::Water, twb)
            .ok_or(HumidAirError::NonConvergent)?
            .rho_liquid;
        let h_w = crate::props::state_trho(Fluid::Water, twb, rho_liquid).enthalpy;
        let rhs = molar_enthalpy(twb, psi_ws, vbar_wb) * (1.0 + w_s) / m_ha_wb + (w - w_s) * h_w;
        Ok(lhs - rhs)
    };

    let t_dp = dew_point_temperature(t, p, psi_w)?;
    if t <= t_dp + 1e-9 {
        // Already at (or past) saturation: T_wb = T_dp = T.
        return Ok(t_dp);
    }

    let mut lo = t_dp;
    let mut hi = t;
    let mut r_lo = residual(lo)?;
    let r_hi = residual(hi)?;
    if r_lo == 0.0 {
        return Ok(lo);
    }
    if r_hi == 0.0 {
        return Ok(hi);
    }
    if r_lo.signum() == r_hi.signum() {
        return Err(HumidAirError::NonConvergent);
    }
    for _ in 0..100 {
        let mid = 0.5 * (lo + hi);
        let r_mid = residual(mid)?;
        if r_mid.abs() < 1e-6 || (hi - lo) < 1e-9 {
            return Ok(mid);
        }
        if r_mid.signum() == r_lo.signum() {
            lo = mid;
            r_lo = r_mid;
        } else {
            hi = mid;
        }
    }
    Ok(0.5 * (lo + hi))
}

/// Invert the dew point to the water mole fraction `ψ_w`: exact and
/// non-iterative, since by definition `ψ_w = ψ_{w,s}(T_dp, p)` — the dew
/// point *is* the temperature at which the current water content would just
/// saturate. Independent of the current dry-bulb `t`, matching
/// [`dew_point_temperature`]'s own signature note.
fn psi_w_from_dew_point(p: f64, tdp: f64) -> Result<f64, HumidAirError> {
    saturation_mole_fraction(tdp, p)
}

/// Invert the wet-bulb temperature to the water mole fraction `ψ_w`, given
/// the dry-bulb `t`.
///
/// Parameterised by a *trial dew point* rather than `ψ_w` directly, bisected
/// over `T_dp ∈ (273.16 K, T_wb]` (recall `T_dp ≤ T_wb` always, with equality
/// only at saturation — so `T_wb` itself is a safe, if loose, upper bracket).
/// Each trial `T_dp` converts to `ψ_w` exactly via [`psi_w_from_dew_point`]
/// (no iteration), then drives [`wet_bulb_temperature`] to the target `twb`.
///
/// This indirection matters: bisecting over `ψ_w` directly and starting from
/// `ψ_w = 0` (bone-dry air) was tried first and found to break
/// [`wet_bulb_temperature`]'s own internal dew-point computation, which has
/// no finite answer at `ψ_w = 0` (dry air has no dew point) — a
/// [`HumidAirError::OutOfRange`] from deep inside the bisection instead of a
/// clean bracket-failure signal. Parameterising by `T_dp` and keeping it
/// bounded away from the triple point by construction avoids that failure
/// mode entirely, since every trial `ψ_w` this way already has a known-valid
/// dew point (the one that produced it).
fn psi_w_from_wet_bulb(t: f64, p: f64, twb: f64) -> Result<f64, HumidAirError> {
    // 1e-4 K, not 1e-9: a T_wb computed by the forward solve above can land
    // a little (secant-convergence-noise) on either side of T at the exact
    // saturation boundary -- see the (t - twb).abs() branch just below.
    if !(twb.is_finite() && twb > saturation::T_TRIPLE && twb <= t + 1e-4) {
        return Err(HumidAirError::OutOfRange);
    }
    if (t - twb).abs() < 1e-6 {
        // T_wb = T only at saturation (R = 1): psi_w is exactly the
        // saturation mole fraction at the dry-bulb T. Handled directly
        // rather than falling into the bisection below, whose bracket
        // (T_dp up to T_wb) degenerates at this exact boundary and can
        // trip the inner dew-point secant's domain guard (see the module
        // doc's caveats section) instead of converging cleanly.
        return saturation_mole_fraction(t, p);
    }
    let residual = |tdp: f64| -> Result<f64, HumidAirError> {
        let psi = psi_w_from_dew_point(p, tdp)?;
        Ok(wet_bulb_temperature(t, p, psi)? - twb)
    };

    let mut lo = saturation::T_TRIPLE + 1e-6;
    let mut hi = twb;
    let r_lo = residual(lo)?;
    let r_hi = residual(hi)?;
    if r_hi.abs() < 1e-9 {
        return psi_w_from_dew_point(p, hi);
    }
    if r_lo.signum() == r_hi.signum() {
        return Err(HumidAirError::NonConvergent);
    }
    let mut tdp = 0.5 * (lo + hi);
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        let r_mid = residual(mid)?;
        tdp = mid;
        if r_mid.abs() < 1e-6 || (hi - lo) < 1e-9 {
            break;
        }
        if r_mid.signum() == r_lo.signum() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    psi_w_from_dew_point(p, tdp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_vapor_h_at_0c_matches_latent_heat_of_vaporization() {
        // Pins down the assumption ideal_gas_molar_entropy_water's
        // Clausius-Clapeyron reference constant depends on: this port's
        // ASHRAE-consistent water-vapor enthalpy polynomial, evaluated at
        // 0 C, should closely match water's known latent heat of
        // vaporization at 0 C (~2501 kJ/kg, e.g. IAPWS/steam-table
        // references). If this ever drifted (e.g. a future edit to the
        // enthalpy polynomial), entropy's absolute value would silently
        // drift with it -- this test exists so that drift is caught here,
        // not discovered downstream in a psychrometric-chart comparison.
        let h_w_0c_j_per_mol = ideal_gas_molar_enthalpy_water(T0_ENTROPY_REF);
        let h_w_0c_j_per_kg = h_w_0c_j_per_mol / MM_WATER;
        eprintln!(
            "h_w(0C) = {:.1} J/mol = {:.2} kJ/kg (expect ~2501 kJ/kg)",
            h_w_0c_j_per_mol,
            h_w_0c_j_per_kg / 1000.0
        );
        assert!(
            (h_w_0c_j_per_kg / 1000.0 - 2501.0).abs() < 5.0,
            "h_w(0C) = {} kJ/kg, expected ~2501 kJ/kg (latent heat of vaporization)",
            h_w_0c_j_per_kg / 1000.0
        );
    }
}
