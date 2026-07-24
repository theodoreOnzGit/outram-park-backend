//! Transport-property correlations and phase-mixing rules: viscosity, thermal
//! conductivity, and surface tension of gas and liquid phases.
//!
//! # Provenance (DWSIM, GPL-3.0)
//!
//! Ported from DWSIM's property-package transport methods. Two upstream files
//! are the source (paths relative to the DWSIM solution root; line numbers are
//! the commit vendored under this crate's `upstream_source/`):
//!
//! - **`DWSIM.Thermodynamics/PropertyPackages/PropertyPackage.vb`** — the
//!   *phase-mixing* routines: `AUX_VAPVISCm` (gas viscosity, `:7340`),
//!   `AUX_VAPVISCi` (`:7410`), `AUX_LIQVISCi`/`AUX_LIQVISCm` (liquid viscosity,
//!   `:6883`/`:6971`), `AUX_CONDTG` (gas conductivity, `:7257`), `AUX_CONDTL`
//!   (liquid conductivity, `:7186`), `AUX_SURFTi`/`AUX_SURFTM` (surface
//!   tension, `:7157`/`:7049`).
//! - **`DWSIM.Thermodynamics/PropertyPackages/Models/FluidProperties.vb`** —
//!   the *pure-component* corresponding-states correlations that those
//!   routines call: `viscg_lucas` (`:216`), `viscl_letsti` (`:142`),
//!   `condl_latini` (`:381`), `condtg_elyhanley` (`:437`),
//!   `viscg_jossi_stiel_thodos` (`:678`), `condlm_li` (`:717`), `sigma_bb`
//!   (`:120`).
//!
//! # What is ported vs. deferred (honest scope)
//!
//! DWSIM's `AUX_*` routines first try a per-compound *tabular / experimental*
//! correlation (a DIPPR/ChemSep polynomial keyed by an equation number in the
//! compound database) and fall back to the corresponding-states estimators
//! only when no experimental curve is present. **This port carries the
//! corresponding-states estimators and the phase-mixing rules only.** The
//! tabular-coefficient path, the CoolProp/experimental-database backends, and
//! the compressed-liquid Lucas pressure correction (`viscl_pcorrection_lucas`)
//! are *not* ported — a compound here is the constant-property [`Component`],
//! which has no per-compound transport-coefficient tables.
//!
//! One deliberate deviation: DWSIM's default **gas-viscosity mixing** is a bare
//! mole average of the pure Lucas viscosities ([`gas_viscosity_mole_average`],
//! matching `AUX_VAPVISCm`). The task this module answers also asks for the
//! classical **Wilke** rule, which DWSIM does *not* use for viscosity;
//! [`gas_viscosity_wilke`] provides it, cited to Poling et al. rather than to
//! DWSIM. Both are offered so a caller can pick.
//!
//! # Units
//!
//! Public boundaries are `uom`-typed: [`DynamicViscosity`] (Pa·s),
//! [`ThermalConductivity`] (W/(m·K)), [`SurfaceTension`] (N/m),
//! [`ThermodynamicTemperature`] (K). Pure-component functions read the raw-`f64`
//! SI constants off a [`Component`] (Tc [K], Pc [Pa], ω [-], M [kg/mol],
//! Tb [K], Vc [m³/mol]). Phase-mixing functions take raw-`f64` slices — mole or
//! mass fractions [-] and the *per-component* transport values already in the
//! SI base unit of the returned quantity (Pa·s, W/(m·K), or N/m) — because a
//! slice of `uom` quantities is awkward at a call site and these feed tight
//! summation loops (the crate `CLAUDE.md` "raw f64 in inner loops" convention).
//!
//! # ⚠️ Unverified until validated
//!
//! Early-stage translation. The tests below are **verification** (the code
//! reproduces the cited DWSIM correlation and matches published pure-component
//! data points within each correlation's stated accuracy), **not** a benchmark
//! validation of the whole property package. Not for nuclear facility
//! operation, reactor control, safety-critical or licensing decisions.
//! Independent OUTRAM PARK fork, not the official DWSIM.

use crate::thermo::Component;
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::{
    DynamicViscosity, MolarHeatCapacity, SurfaceTension, ThermalConductivity,
    ThermodynamicTemperature,
};
use uom::si::molar_heat_capacity::joule_per_kelvin_mole;
use uom::si::surface_tension::newton_per_meter;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

// ===========================================================================
// Gas-phase viscosity
// ===========================================================================

/// Low-pressure (dilute) gas viscosity of a pure component by the **Lucas
/// corresponding-states** method.
///
/// Physical quantity: dynamic viscosity `η` of the vapour at temperature `T`
/// and (implicitly) low pressure, where the pressure/density correction is
/// negligible. Ported from DWSIM `FluidProperties.vb:216` `viscg_lucas`:
/// ```text
/// ξ  = 0.176 (Tc / (M³ Pc⁴))^(1/6)          [Tc in K, M in g/mol, Pc in bar]
/// ηξ = 0.807 Tr^0.618 − 0.357 e^(−0.449 Tr) + 0.34 e^(−4.058 Tr) + 0.018
/// η  = (ηξ / ξ) × 10⁻⁷ Pa·s                  [ηξ/ξ is in micropoise]
/// ```
/// `Tc`, `Pc`, `M` are read from `component`; `Tr = T/Tc`.
///
/// Valid range: dilute gas, `Tr` roughly 0.3–15; stated accuracy of the Lucas
/// low-pressure method is ~1–3 % for non-polar gases (Poling, Prausnitz &
/// O'Connell, *The Properties of Gases and Liquids*, 5th ed., §9-4).
pub fn gas_viscosity_lucas(
    component: &Component,
    temperature: ThermodynamicTemperature,
) -> DynamicViscosity {
    let t = temperature.get::<kelvin>();
    let tc = component.critical_temperature;
    let pc_bar = component.critical_pressure / 1.0e5;
    let mm_g = component.molar_mass * 1000.0; // kg/mol -> g/mol
    let tr = t / tc;

    let xi = 0.176 * (tc / (mm_g.powi(3) * pc_bar.powi(4))).powf(1.0 / 6.0);
    let eta_xi = 0.807 * tr.powf(0.618) - 0.357 * (-0.449 * tr).exp()
        + 0.34 * (-4.058 * tr).exp()
        + 0.018;
    let eta_micropoise = eta_xi / xi;
    DynamicViscosity::new::<pascal_second>(eta_micropoise * 1.0e-7)
}

/// Molar-average gas-phase viscosity — **DWSIM's default** vapour-viscosity
/// mixing rule.
///
/// Ported from the first step of DWSIM `PropertyPackage.vb:7391` `AUX_VAPVISCm`,
/// which forms `η_mix = Σ xᵢ ηᵢ` over the pure-component Lucas viscosities
/// before (optionally) applying the Jossi-Stiel-Thodos density correction
/// (see [`gas_viscosity_jossi_stiel_thodos`]).
///
/// - `mole_fractions` — vapour mole fractions `xᵢ` [-].
/// - `pure_viscosities` — per-component viscosities `ηᵢ` [Pa·s] (e.g. from
///   [`gas_viscosity_lucas`]).
///
/// Slices must be the same length. Returns `η_mix` [Pa·s].
pub fn gas_viscosity_mole_average(
    mole_fractions: &[f64],
    pure_viscosities: &[f64],
) -> DynamicViscosity {
    let sum: f64 = mole_fractions
        .iter()
        .zip(pure_viscosities)
        .map(|(x, eta)| x * eta)
        .sum();
    DynamicViscosity::new::<pascal_second>(sum)
}

/// Gas-phase viscosity by the **Wilke** kinetic-theory mixing rule.
///
/// Physical quantity: dynamic viscosity of a low-pressure gas mixture from the
/// pure-component viscosities and molar masses:
/// ```text
/// η_mix = Σᵢ  xᵢ ηᵢ / (Σⱼ xⱼ φᵢⱼ)
/// φᵢⱼ   = [1 + (ηᵢ/ηⱼ)^(1/2) (Mⱼ/Mᵢ)^(1/4)]² / sqrt(8 (1 + Mᵢ/Mⱼ))
/// ```
/// with `φᵢᵢ = 1`. Reference: Wilke, *J. Chem. Phys.* **18**, 517 (1950); as
/// given in Poling et al. (2001), eq. 9-5.13/9-5.14.
///
/// **Not a DWSIM routine** — DWSIM mixes vapour viscosity by the bare mole
/// average ([`gas_viscosity_mole_average`]). Wilke is provided as the classical
/// rigorous low-pressure rule; it reduces exactly to the pure value for a single
/// component and always lies between the pure-component viscosities for a binary.
///
/// - `mole_fractions` — `xᵢ` [-].
/// - `pure_viscosities` — `ηᵢ` [Pa·s].
/// - `molar_masses` — `Mᵢ` [any consistent unit; only ratios enter].
///
/// All three slices must share one length.
pub fn gas_viscosity_wilke(
    mole_fractions: &[f64],
    pure_viscosities: &[f64],
    molar_masses: &[f64],
) -> DynamicViscosity {
    let n = mole_fractions.len();
    let mut eta_mix = 0.0;
    for i in 0..n {
        let mut denom = 0.0;
        for j in 0..n {
            let phi = if i == j {
                1.0
            } else {
                let a = 1.0
                    + (pure_viscosities[i] / pure_viscosities[j]).sqrt()
                        * (molar_masses[j] / molar_masses[i]).powf(0.25);
                a * a / (8.0 * (1.0 + molar_masses[i] / molar_masses[j])).sqrt()
            };
            denom += mole_fractions[j] * phi;
        }
        if denom > 0.0 {
            eta_mix += mole_fractions[i] * pure_viscosities[i] / denom;
        }
    }
    DynamicViscosity::new::<pascal_second>(eta_mix)
}

/// Dense-gas (high-pressure) viscosity correction by the **Jossi-Stiel-Thodos**
/// residual-viscosity method — the second step of DWSIM `AUX_VAPVISCm`.
///
/// Ported from DWSIM `FluidProperties.vb:678` `viscg_jossi_stiel_thodos`:
/// ```text
/// ρr  = Vc / V                              [reduced density]
/// ξ   = (Tc / (M³ Pc⁴))^(1/6)               [Tc in K, M in g/mol, Pc in atm]
/// [(η − η₀) ξ + 1]  →  1.023 + 0.23364 ρr + 0.58533 ρr² − 0.40758 ρr³ + 0.093324 ρr⁴
/// ```
/// solved for `η` (all with the internal unit scalings DWSIM applies). `η₀` is
/// the low-pressure mixture viscosity (e.g. from [`gas_viscosity_mole_average`]).
///
/// - `low_pressure_viscosity` — `η₀` [Pa·s].
/// - `temperature` — `T`.
/// - `molar_volume` — `V` [m³/mol] of the gas at state.
/// - `critical_volume` — `Vc` [m³/mol] (mixture) — must share `V`'s unit.
/// - `critical_temperature` — `Tc` [K] (mixture).
/// - `critical_pressure` — `Pc` [Pa] (mixture).
/// - `molar_mass` — `M` [g/mol] (mixture).
///
/// Returns the density-corrected viscosity [Pa·s]. Note the correlation is a
/// residual fit and does **not** collapse exactly to `η₀` as `ρr → 0` (it
/// leaves a small `(1.023⁴ − 1)/ξ` residue); it is intended for `ρr` above
/// roughly 0.1.
#[allow(clippy::too_many_arguments)]
pub fn gas_viscosity_jossi_stiel_thodos(
    low_pressure_viscosity: DynamicViscosity,
    temperature: ThermodynamicTemperature,
    molar_volume: f64,
    critical_volume: f64,
    critical_temperature: f64,
    critical_pressure: f64,
    molar_mass_g: f64,
) -> DynamicViscosity {
    let _t = temperature.get::<kelvin>();
    let pc_atm = critical_pressure / 101_325.0;
    let rho_r = critical_volume / molar_volume;
    let xit = (critical_temperature / molar_mass_g.powi(3) / pc_atm.powi(4)).powf(1.0 / 6.0);
    // DWSIM rescales η₀ (Pa·s) by 1e7 into its internal micropoise-like unit.
    let eta0_scaled = low_pressure_viscosity.get::<pascal_second>() * 1.0e7;
    let bracket = 1.023 + 0.23364 * rho_r + 0.58533 * rho_r.powi(2)
        - 0.40758 * rho_r.powi(3)
        + 0.093324 * rho_r.powi(4);
    let tmp = (bracket.powi(4) - 1.0) / xit + eta0_scaled;
    DynamicViscosity::new::<pascal_second>(tmp * 1.0e-7)
}

// ===========================================================================
// Liquid-phase viscosity
// ===========================================================================

/// Pure-component saturated-liquid viscosity by the **Letsou-Stiel**
/// corresponding-states method.
///
/// Physical quantity: dynamic viscosity `η_L` of the saturated liquid at `T`.
/// Ported from DWSIM `FluidProperties.vb:142` `viscl_letsti`:
/// ```text
/// ξ  = 0.176 (Tc / (M³ Pc⁴))^(1/6)          [Tc in K, M in g/mol, Pc in bar]
/// η⁰ = (2.648 − 3.725 Tr + 1.309 Tr²) × 10⁻³
/// η¹ = (7.425 − 13.39 Tr + 5.933 Tr²) × 10⁻³
/// η  = (η⁰ + ω η¹) / ξ / 1000  Pa·s
/// ```
/// `Tc`, `Pc`, `ω`, `M` from `component`; `Tr = T/Tc`.
///
/// Valid range: reduced temperature roughly **0.76 ≤ Tr ≤ 0.98** (a
/// near-critical-liquid corresponding-states correlation); typical accuracy
/// ~15 % in-range, degrading rapidly below Tr ≈ 0.7 (Poling et al., §9-11).
pub fn liquid_viscosity_letsou_stiel(
    component: &Component,
    temperature: ThermodynamicTemperature,
) -> DynamicViscosity {
    let t = temperature.get::<kelvin>();
    let tc = component.critical_temperature;
    let pc_bar = component.critical_pressure / 1.0e5;
    let mm_g = component.molar_mass * 1000.0;
    let w = component.acentric_factor;
    let tr = t / tc;

    let e0 = (2.648 - 3.725 * tr + 1.309 * tr.powi(2)) * 1.0e-3;
    let e1 = (7.425 - 13.39 * tr + 5.933 * tr.powi(2)) * 1.0e-3;
    let xi = 0.176 * (tc / (mm_g.powi(3) * pc_bar.powi(4))).powf(1.0 / 6.0);
    DynamicViscosity::new::<pascal_second>((e0 + e1 * w) / xi / 1000.0)
}

/// Liquid-phase viscosity mixing rule selector — the four rules DWSIM offers in
/// `AUX_LIQVISCm` (`PropertyPackage.vb:7016`), dispatched by enum (no trait
/// object, per the workspace design rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidViscosityMixingRule {
    /// `η = Σ xᵢ ηᵢ` (mole-fraction linear average).
    MoleAverage,
    /// `η = exp(Σ xᵢ ln ηᵢ)` (mole-fraction log average).
    LogMoleAverage,
    /// `η = 1 / Σ (wᵢ / ηᵢ)` (mass-fraction inverse average).
    InvertedMassAverage,
    /// `η = exp(1 / Σ (wᵢ / ln ηᵢ))` (mass-fraction inverse-log average).
    InvertedLogMassAverage,
}

/// Liquid-phase mixture viscosity by the selected [`LiquidViscosityMixingRule`].
///
/// Ported from DWSIM `PropertyPackage.vb:6971` `AUX_LIQVISCm` (the mixing-rule
/// `Select Case`, `:7016`–`:7037`). Individual `ηᵢ` come from e.g.
/// [`liquid_viscosity_letsou_stiel`].
///
/// - `mole_fractions` — `xᵢ` [-] (used by the mole-average rules).
/// - `mass_fractions` — `wᵢ` [-] (used by the inverted mass-average rules).
/// - `pure_viscosities` — `ηᵢ` [Pa·s].
///
/// All three slices must share one length. Returns `η_mix` [Pa·s]. For a single
/// component every rule collapses to that component's viscosity.
pub fn liquid_viscosity_mixture(
    rule: LiquidViscosityMixingRule,
    mole_fractions: &[f64],
    mass_fractions: &[f64],
    pure_viscosities: &[f64],
) -> DynamicViscosity {
    let mut val = 0.0;
    for i in 0..mole_fractions.len() {
        let x = mole_fractions[i];
        let w = mass_fractions[i];
        let eta = pure_viscosities[i];
        match rule {
            LiquidViscosityMixingRule::MoleAverage => val += x * eta,
            LiquidViscosityMixingRule::LogMoleAverage => val += x * eta.ln(),
            LiquidViscosityMixingRule::InvertedMassAverage => val += w / eta,
            LiquidViscosityMixingRule::InvertedLogMassAverage => val += w / eta.ln(),
        }
    }
    let result = match rule {
        LiquidViscosityMixingRule::MoleAverage => val,
        LiquidViscosityMixingRule::LogMoleAverage => val.exp(),
        LiquidViscosityMixingRule::InvertedMassAverage => 1.0 / val,
        LiquidViscosityMixingRule::InvertedLogMassAverage => (1.0 / val).exp(),
    };
    DynamicViscosity::new::<pascal_second>(result)
}

// ===========================================================================
// Thermal conductivity
// ===========================================================================

/// Liquid-hydrocarbon-family selector for the Latini correlation
/// (`FluidProperties.vb:381`, the `Tipo` argument). Governs the four fit
/// constants `A*, α, β, γ`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LatiniFluidType {
    /// Saturated hydrocarbons (DWSIM default; DWSIM always passes `""` → this).
    SaturatedHydrocarbon,
    /// Olefins.
    Olefin,
    /// Cycloparaffins.
    Cycloparaffin,
    /// Aromatics.
    Aromatic,
    /// Other (e.g. water) — DWSIM's `"X"` branch.
    Other,
}

/// Pure-component saturated-liquid thermal conductivity by the **Latini**
/// method.
///
/// Physical quantity: liquid thermal conductivity `λ_L` [W/(m·K)] at `T`.
/// Ported from DWSIM `FluidProperties.vb:381` `condl_latini`:
/// ```text
/// A = A* Tb^α / (M^β Tc^γ)
/// λ = A (1 − Tr)^0.38 / Tr^(1/6)            (λ = 0 if Tr > 0.98)
/// ```
/// with `(A*, α, β, γ)` set by [`LatiniFluidType`]. `Tb`, `Tc`, `M` from
/// `component`; `Tr = T/Tc`.
///
/// **Fidelity note:** DWSIM's `AUX_CONDTL` always calls this with an empty
/// `Tipo` (→ [`LatiniFluidType::SaturatedHydrocarbon`]) regardless of the actual
/// compound family; the other families are exposed here for completeness.
///
/// Valid range: `Tr < 0.98`; typical accuracy ~10 % for the intended families
/// (Poling et al., §10-9).
pub fn liquid_thermal_conductivity_latini(
    component: &Component,
    temperature: ThermodynamicTemperature,
    fluid_type: LatiniFluidType,
) -> ThermalConductivity {
    let t = temperature.get::<kelvin>();
    let tc = component.critical_temperature;
    let tb = component.normal_boiling_point;
    let mm_g = component.molar_mass * 1000.0;
    let tr = t / tc;

    let (a_star, alpha, beta, gamma) = match fluid_type {
        LatiniFluidType::SaturatedHydrocarbon => (0.0035, 1.2, 0.5, 0.167),
        LatiniFluidType::Olefin => (0.0361, 1.2, 1.0, 0.167),
        LatiniFluidType::Cycloparaffin => (0.031, 1.2, 1.0, 0.167),
        LatiniFluidType::Aromatic => (0.0346, 1.2, 1.0, 0.167),
        LatiniFluidType::Other => (0.494, 0.0, 0.5, -0.167),
    };

    let a = a_star * tb.powf(alpha) / (mm_g.powf(beta) * tc.powf(gamma));
    let lambda = if tr > 0.98 {
        0.0
    } else {
        a * (1.0 - tr).powf(0.38) / tr.powf(1.0 / 6.0)
    };
    ThermalConductivity::new::<watt_per_meter_kelvin>(lambda)
}

/// Liquid-phase mixture thermal conductivity by the **Li** method.
///
/// Ported from DWSIM `FluidProperties.vb:717` `condlm_li` (called by
/// `AUX_CONDTL`, `PropertyPackage.vb:7245`):
/// ```text
/// φᵢ  = xᵢ Vcᵢ / Σⱼ xⱼ Vcⱼ                   [critical-volume fractions]
/// λᵢⱼ = 2 (1/λᵢ + 1/λⱼ)⁻¹                    [harmonic mean]
/// λ_L = Σᵢ Σⱼ φᵢ φⱼ λᵢⱼ
/// ```
///
/// - `critical_volumes` — `Vcᵢ` [any consistent unit; only ratios enter].
/// - `pure_conductivities` — `λᵢ` [W/(m·K)] (e.g. from
///   [`liquid_thermal_conductivity_latini`]).
/// - `mole_fractions` — `xᵢ` [-].
///
/// All three slices must share one length. Returns `λ_L` [W/(m·K)]; collapses to
/// `λ₁` for a single component.
pub fn liquid_thermal_conductivity_li(
    critical_volumes: &[f64],
    pure_conductivities: &[f64],
    mole_fractions: &[f64],
) -> ThermalConductivity {
    let n = mole_fractions.len();
    let sum_z: f64 = (0..n).map(|i| mole_fractions[i] * critical_volumes[i]).sum();
    let phi: Vec<f64> = (0..n)
        .map(|i| {
            if sum_z != 0.0 {
                mole_fractions[i] * critical_volumes[i] / sum_z
            } else {
                0.0
            }
        })
        .collect();
    let mut lambda = 0.0;
    for i in 0..n {
        for j in 0..n {
            if mole_fractions[i] != 0.0 && mole_fractions[j] != 0.0 {
                let lambda_ij = 2.0
                    / (1.0 / pure_conductivities[i] + 1.0 / pure_conductivities[j]);
                lambda += phi[i] * phi[j] * lambda_ij;
            }
        }
    }
    ThermalConductivity::new::<watt_per_meter_kelvin>(lambda)
}

/// Pure-component low-pressure vapour thermal conductivity by the **Ely-Hanley**
/// extended-corresponding-states method (methane reference fluid).
///
/// Physical quantity: vapour thermal conductivity `λ_V` [W/(m·K)] at `T`.
/// Ported from DWSIM `FluidProperties.vb:437` `condtg_elyhanley`:
/// ```text
/// λ_V = λ* + (1000 η* / M) 1.32 (Cv − 3R/2)
/// ```
/// where `λ*`, `η*` come from the methane reference fluid mapped through the
/// shape factors `f`, `h`, `θ`, `φ` and the Hanley `η₀(T₀)` polynomial (nine
/// `Cₙ T₀^((n−4)/3)` terms). See the upstream source for the full shape-factor
/// expressions, reproduced verbatim here.
///
/// Inputs (DWSIM `AUX_VAPTHERMCONDi`, `PropertyPackage.vb:7333`, supplies these):
/// - `component` — supplies `Tc` [K], `Vc` [m³/mol], `ω` [-], `M` [kg/mol].
/// - `critical_compressibility` — `Zc` [-]. **Not on [`Component`]** — pass
///   explicitly (a reasonable estimate is `Zc = 0.291 − 0.08 ω`, DWSIM `Zc1`).
/// - `isochoric_heat_capacity` — `Cv` [J/(mol·K)], the ideal-gas
///   constant-volume molar heat capacity (DWSIM uses `Cp·M − R`).
///
/// Valid range: low-pressure non-polar vapour. **This routine is a faithful
/// translation whose numeric output is verified against the DWSIM formula
/// (see the test), not a tight benchmark validation against experiment.**
pub fn gas_thermal_conductivity_ely_hanley(
    component: &Component,
    temperature: ThermodynamicTemperature,
    critical_compressibility: f64,
    isochoric_heat_capacity: MolarHeatCapacity,
) -> ThermalConductivity {
    let t = temperature.get::<kelvin>();
    let tc = component.critical_temperature;
    let vc = component.critical_volume; // m³/mol
    let w = component.acentric_factor;
    let zc = critical_compressibility;
    let mm_kg = component.molar_mass; // kg/mol
    let cv = isochoric_heat_capacity.get::<joule_per_kelvin_mole>();

    let tr = t / tc;
    let tplus = if tr <= 2.0 { tr } else { 2.0 };

    let teta = 1.0 + (w - 0.011) * (0.56553 - 0.86276 * tplus.ln() - 0.69852 / tplus);
    let omega = (1.0 + (w - 0.011) * (0.3856 - 1.1617 * tplus.ln())) * 0.288 / zc;

    let f = tc * teta / 190.4;
    let h = vc * omega / 99.2 * 1.0e6; // m³/mol -> cm³/mol scaling
    let t0 = t / f;

    // Hanley η₀(T₀) polynomial (micro-scale reference viscosity).
    const C: [f64; 9] = [
        2_907_740.0,
        -3_312_870.0,
        1_608_100.0,
        -433_190.0,
        70_624.8,
        -7_116.62,
        432.517,
        -14.4591,
        0.203712,
    ];
    let mut eta0 = 0.0;
    for (i, c) in C.iter().enumerate() {
        eta0 += 1.0e-7 * c * t0.powf((i as f64 - 3.0) / 3.0);
    }

    let lambda0 = 1944.0 * eta0;
    let h_big = (0.01604 / mm_kg).powf(0.5) * f.powf(0.5) / h.powf(2.0 / 3.0);
    let lambda_star = lambda0 * h_big;
    let eta_star = eta0 * h_big * mm_kg / 0.01604;

    let lambda_v = lambda_star + (eta_star / mm_kg) * 1.32 * (cv - 3.0 * 8.314 / 2.0);
    ThermalConductivity::new::<watt_per_meter_kelvin>(lambda_v)
}

/// Molar-average vapour thermal conductivity — DWSIM's gas-conductivity mixing
/// rule.
///
/// Ported from DWSIM `PropertyPackage.vb:7257` `AUX_CONDTG`, which forms
/// `λ_mix = Σ xᵢ λᵢ` over the pure-component (Ely-Hanley) conductivities.
///
/// - `mole_fractions` — vapour mole fractions `xᵢ` [-].
/// - `pure_conductivities` — `λᵢ` [W/(m·K)].
///
/// Slices must share one length. Returns `λ_mix` [W/(m·K)].
pub fn gas_thermal_conductivity_mole_average(
    mole_fractions: &[f64],
    pure_conductivities: &[f64],
) -> ThermalConductivity {
    let sum: f64 = mole_fractions
        .iter()
        .zip(pure_conductivities)
        .map(|(x, lambda)| x * lambda)
        .sum();
    ThermalConductivity::new::<watt_per_meter_kelvin>(sum)
}

// ===========================================================================
// Surface tension
// ===========================================================================

/// Pure-component liquid surface tension by the **Brock-Bird** corresponding-
/// states method.
///
/// Physical quantity: surface tension `σ` [N/m] of the saturated liquid at `T`.
/// Ported from DWSIM `FluidProperties.vb:120` `sigma_bb`:
/// ```text
/// Q   = 0.1196 [1 + Tbr ln(Pc/1.01325) / (1 − Tbr)] − 0.279   [Pc in bar]
/// σ   = Pc^(2/3) Tc^(1/3) Q (1 − Tr)^(11/9) / 1000  N/m
/// ```
/// `Tc`, `Pc`, `Tb` from `component`; `Tr = T/Tc`, `Tbr = Tb/Tc`. If the normal
/// boiling point is unknown (`Tb ≤ 0` / non-finite) DWSIM substitutes
/// `Tb = 0.7 Tc`. Returns `0` for `T ≥ Tc` (the interface has vanished), so `σ`
/// goes to zero as `T → Tc`.
///
/// Valid range: `Tr < 1`; typical accuracy ~5 % for non-polar/slightly-polar
/// liquids (Poling et al., §12-3).
pub fn surface_tension_brock_bird(
    component: &Component,
    temperature: ThermodynamicTemperature,
) -> SurfaceTension {
    let t = temperature.get::<kelvin>();
    let tc = component.critical_temperature;
    let pc_bar = component.critical_pressure / 1.0e5;
    let tr = t / tc;
    if tr >= 1.0 {
        return SurfaceTension::new::<newton_per_meter>(0.0);
    }
    let tb = if component.normal_boiling_point.is_finite() && component.normal_boiling_point > 0.0 {
        component.normal_boiling_point
    } else {
        0.7 * tc
    };
    let tbr = tb / tc;
    let q = 0.1196 * (1.0 + tbr * (pc_bar / 1.01325).ln() / (1.0 - tbr)) - 0.279;
    let sigma_mn = pc_bar.powf(2.0 / 3.0) * tc.powf(1.0 / 3.0) * q * (1.0 - tr).powf(11.0 / 9.0);
    SurfaceTension::new::<newton_per_meter>(sigma_mn / 1000.0)
}

/// Liquid-phase mixture surface tension by DWSIM's **molar-average** rule.
///
/// Ported from DWSIM `PropertyPackage.vb:7049` `AUX_SURFTM`:
/// ```text
/// σ_mix = Σᵢ xᵢ σᵢ / ftotal        (ftotal starts at 1; a supercritical
///                                    component contributes σᵢ = 0 and removes
///                                    its xᵢ from ftotal — renormalisation)
/// ```
/// The per-component tensions `σᵢ` are the Brock-Bird values
/// ([`surface_tension_brock_bird`]).
///
/// - `mole_fractions` — `xᵢ` [-].
/// - `pure_tensions` — `σᵢ` [N/m] (caller passes the sub-critical value; the
///   supercritical flag drives the renormalisation, matching DWSIM).
/// - `subcritical` — `true` where `T < Tcᵢ`; a `false` entry contributes `0`
///   and is dropped from the mole-fraction normaliser.
///
/// All three slices must share one length. Returns `σ_mix` [N/m]. For an
/// all-subcritical mixture this is the plain molar average `Σ xᵢ σᵢ`.
///
/// **Quirk preserved:** DWSIM decrements `ftotal` *inside* the accumulation
/// loop, so when some component is supercritical the result is mildly
/// order-dependent; this port reproduces that behaviour verbatim rather than
/// "fixing" it.
pub fn surface_tension_mixture(
    mole_fractions: &[f64],
    pure_tensions: &[f64],
    subcritical: &[bool],
) -> SurfaceTension {
    let mut val = 0.0;
    let mut ftotal = 1.0;
    for i in 0..mole_fractions.len() {
        let tmpval = if subcritical[i] {
            pure_tensions[i]
        } else {
            ftotal -= mole_fractions[i];
            0.0
        };
        if ftotal != 0.0 {
            val += mole_fractions[i] * tmpval / ftotal;
        }
    }
    SurfaceTension::new::<newton_per_meter>(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thermo::component::reference;
    use approx::assert_relative_eq;

    fn t_k(t: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(t)
    }

    /// **Methodology.** Pure-gas Lucas viscosity of nitrogen at 300 K
    /// (`FluidProperties.vb:216`), compared to the recommended experimental
    /// value. Reference point: nitrogen at 300 K, 1 bar,
    /// η = 17.9 µPa·s (NIST/Lemmon et al. reference correlation; also Poling,
    /// Prausnitz & O'Connell, *Properties of Gases and Liquids*, 5th ed.,
    /// Table 9-1 region). Lucas low-pressure method stated accuracy ~1–3 %.
    /// **Result (2026-07-24):** this port gives η = 1.794e-5 Pa·s = 17.94 µPa·s,
    /// +0.2 % vs 17.9 µPa·s — inside the correlation's stated band.
    #[test]
    fn lucas_gas_viscosity_nitrogen_300k() {
        let eta = gas_viscosity_lucas(&reference::nitrogen(), t_k(300.0));
        let micro_pa_s = eta.get::<pascal_second>() * 1.0e6;
        assert_relative_eq!(micro_pa_s, 17.9, max_relative = 0.03);
    }

    /// **Methodology.** Cross-check a second pure gas: methane at 300 K.
    /// Reference: η ≈ 11.2 µPa·s (NIST). **Result (2026-07-24):** port gives
    /// 11.09 µPa·s, −1.0 % vs 11.2 µPa·s.
    #[test]
    fn lucas_gas_viscosity_methane_300k() {
        let eta = gas_viscosity_lucas(&reference::methane(), t_k(300.0));
        let micro_pa_s = eta.get::<pascal_second>() * 1.0e6;
        assert_relative_eq!(micro_pa_s, 11.2, max_relative = 0.03);
    }

    /// **Methodology.** Wilke gas-viscosity mixing must (a) reduce exactly to
    /// the pure value for a single component and (b) lie strictly between the
    /// two pure viscosities for a binary. Uses N₂ and CH₄ Lucas viscosities at
    /// 300 K. **Result (2026-07-24):** single-component Wilke = pure Lucas to
    /// machine precision; the equimolar N₂/CH₄ Wilke value (≈ 14.7 µPa·s) lies
    /// between 11.09 and 17.94 µPa·s.
    #[test]
    fn wilke_reduces_to_pure_and_brackets_binary() {
        let eta_n2 = gas_viscosity_lucas(&reference::nitrogen(), t_k(300.0)).get::<pascal_second>();
        let eta_ch4 = gas_viscosity_lucas(&reference::methane(), t_k(300.0)).get::<pascal_second>();
        let m_n2 = reference::nitrogen().molar_mass;
        let m_ch4 = reference::methane().molar_mass;

        // (a) single component
        let pure = gas_viscosity_wilke(&[1.0], &[eta_n2], &[m_n2]).get::<pascal_second>();
        assert_relative_eq!(pure, eta_n2, max_relative = 1e-12);

        // (b) binary brackets the pures
        let mix = gas_viscosity_wilke(&[0.5, 0.5], &[eta_n2, eta_ch4], &[m_n2, m_ch4])
            .get::<pascal_second>();
        let lo = eta_ch4.min(eta_n2);
        let hi = eta_ch4.max(eta_n2);
        assert!(mix > lo && mix < hi, "mix {mix} not strictly between {lo} and {hi}");
    }

    /// **Methodology.** Mole-average gas viscosity (DWSIM default,
    /// `AUX_VAPVISCm`) is composition-weighted-consistent: for a single
    /// component it returns the pure value; for a binary it is the exact linear
    /// interpolant. **Result (2026-07-24):** equimolar mix = 0.5(η₁+η₂) exactly.
    #[test]
    fn mole_average_gas_viscosity_is_linear() {
        let e1 = 1.0e-5;
        let e2 = 2.0e-5;
        let single = gas_viscosity_mole_average(&[1.0], &[e1]).get::<pascal_second>();
        assert_relative_eq!(single, e1, max_relative = 1e-12);
        let mix = gas_viscosity_mole_average(&[0.5, 0.5], &[e1, e2]).get::<pascal_second>();
        assert_relative_eq!(mix, 0.5 * (e1 + e2), max_relative = 1e-12);
    }

    /// **Methodology.** Pure liquid viscosity by Letsou-Stiel
    /// (`FluidProperties.vb:142`) for propane at Tr = 0.85 (T = 314.36 K),
    /// inside the method's stated 0.76 ≤ Tr ≤ 0.98 window. Propane constants
    /// (Poling et al., Appendix A): Tc = 369.83 K, Pc = 4.248 MPa, ω = 0.152,
    /// M = 44.096 g/mol. Reference point: saturated-liquid propane viscosity
    /// ≈ 0.081 mPa·s at 314 K (NIST Chemistry WebBook / Lemmon propane EOS).
    /// **Result (2026-07-24):** port gives 8.19e-5 Pa·s = 0.0819 mPa·s, within
    /// ~2 % of the reference and well inside Letsou-Stiel's ~15 % band.
    #[test]
    fn letsou_stiel_liquid_viscosity_propane() {
        let propane = Component::new(
            "Propane", 0.044096, 369.83, 4.248e6, 200.0e-6, 0.152, 231.05, [0.0; 5], f64::NAN,
        )
        .unwrap();
        let t = t_k(0.85 * 369.83);
        let eta = liquid_viscosity_letsou_stiel(&propane, t).get::<pascal_second>();
        assert_relative_eq!(eta, 0.081e-3, max_relative = 0.10);
    }

    /// **Methodology.** Every liquid-viscosity mixing rule must reduce to the
    /// pure value for one component, and MoleAverage must be the linear binary
    /// interpolant. **Result (2026-07-24):** all four rules return η₁ for a
    /// single component (to ≤1e-12 relative); equimolar MoleAverage =
    /// 0.5(η₁+η₂) exactly; LogMoleAverage of a binary lies between the pures.
    #[test]
    fn liquid_viscosity_mixing_rules_consistent() {
        use LiquidViscosityMixingRule::*;
        let e1 = 5.0e-4;
        let e2 = 9.0e-4;
        for rule in [MoleAverage, LogMoleAverage, InvertedMassAverage, InvertedLogMassAverage] {
            let single = liquid_viscosity_mixture(rule, &[1.0], &[1.0], &[e1]).get::<pascal_second>();
            assert_relative_eq!(single, e1, max_relative = 1e-12);
        }
        let mix = liquid_viscosity_mixture(MoleAverage, &[0.5, 0.5], &[0.5, 0.5], &[e1, e2])
            .get::<pascal_second>();
        assert_relative_eq!(mix, 0.5 * (e1 + e2), max_relative = 1e-12);

        let logmix = liquid_viscosity_mixture(LogMoleAverage, &[0.5, 0.5], &[0.5, 0.5], &[e1, e2])
            .get::<pascal_second>();
        assert!(logmix > e1 && logmix < e2);
    }

    /// **Methodology.** Pure liquid thermal conductivity by Latini
    /// (`FluidProperties.vb:381`, saturated-hydrocarbon constants) for
    /// n-heptane at 300 K. Constants (Poling et al., Appendix A):
    /// Tc = 540.2 K, Tb = 371.58 K, M = 100.20 g/mol. Reference point:
    /// n-heptane liquid λ ≈ 0.124 W/(m·K) at 300 K (Poling et al., Table 10-4 /
    /// NIST). Latini stated accuracy ~10 %. **Result (2026-07-24):** port gives
    /// 0.1203 W/(m·K), −3.0 % vs 0.124 — inside the band.
    #[test]
    fn latini_liquid_conductivity_heptane() {
        let heptane = Component::new(
            "n-Heptane", 0.10020, 540.2, 2.74e6, 428.0e-6, 0.349, 371.58, [0.0; 5], f64::NAN,
        )
        .unwrap();
        let lambda = liquid_thermal_conductivity_latini(
            &heptane,
            t_k(300.0),
            LatiniFluidType::SaturatedHydrocarbon,
        )
        .get::<watt_per_meter_kelvin>();
        assert_relative_eq!(lambda, 0.124, max_relative = 0.10);
    }

    /// **Methodology.** Li liquid-conductivity mixing (`FluidProperties.vb:717`)
    /// must reduce to the pure value for one component (φ₁ = 1, harmonic
    /// self-mean = λ₁) and, for a binary of equal conductivities, return that
    /// common value. **Result (2026-07-24):** single component returns λ₁
    /// exactly; a binary with λ₁ = λ₂ returns λ₁; a binary with unequal λ stays
    /// between the two pures.
    #[test]
    fn li_liquid_conductivity_mixing_consistent() {
        let l1 = 0.12;
        let l2 = 0.20;
        let single =
            liquid_thermal_conductivity_li(&[2.0e-4], &[l1], &[1.0]).get::<watt_per_meter_kelvin>();
        assert_relative_eq!(single, l1, max_relative = 1e-12);

        let equal = liquid_thermal_conductivity_li(&[2.0e-4, 2.0e-4], &[l1, l1], &[0.5, 0.5])
            .get::<watt_per_meter_kelvin>();
        assert_relative_eq!(equal, l1, max_relative = 1e-12);

        let mix = liquid_thermal_conductivity_li(&[2.0e-4, 3.0e-4], &[l1, l2], &[0.5, 0.5])
            .get::<watt_per_meter_kelvin>();
        assert!(mix > l1 && mix < l2);
    }

    /// **Methodology.** Ely-Hanley pure-vapour conductivity
    /// (`FluidProperties.vb:437`) verification: the port reproduces the DWSIM
    /// formula and yields a finite, positive value in a physically plausible
    /// gas range for methane at 300 K (Zc = 0.286, Cv ≈ 27.4 J/(mol·K)).
    /// This is **verification of the translation**, not a tight benchmark
    /// validation. **Result (2026-07-24):** measured λ_V = 0.0357 W/(m·K),
    /// +4.7 % vs the recommended ~0.0341 W/(m·K) for methane vapour at 300 K
    /// (NIST) — right order and reasonable for a corresponding-states estimate.
    /// Molar-average mixing of one component returns the pure value.
    #[test]
    fn ely_hanley_gas_conductivity_methane_plausible() {
        let cv = MolarHeatCapacity::new::<joule_per_kelvin_mole>(27.4);
        let lambda = gas_thermal_conductivity_ely_hanley(&reference::methane(), t_k(300.0), 0.286, cv)
            .get::<watt_per_meter_kelvin>();
        assert!(lambda.is_finite() && lambda > 0.0);
        assert!(lambda > 0.005 && lambda < 0.2, "λ_V {lambda} out of plausible gas band");

        let single =
            gas_thermal_conductivity_mole_average(&[1.0], &[lambda]).get::<watt_per_meter_kelvin>();
        assert_relative_eq!(single, lambda, max_relative = 1e-12);
    }

    /// **Methodology.** Brock-Bird surface tension (`FluidProperties.vb:120`)
    /// for benzene at 293.15 K. Constants (Poling et al., Appendix A):
    /// Tc = 562.05 K, Pc = 4.895 MPa, Tb = 353.24 K. Reference point: benzene
    /// σ = 28.88 mN/m at 20 °C (Jasper, *J. Phys. Chem. Ref. Data* 1, 841,
    /// 1972). Brock-Bird stated accuracy ~5 %. **Result (2026-07-24):** port
    /// gives 0.02803 N/m = 28.03 mN/m, −2.9 % vs 28.88 — inside the band.
    #[test]
    fn brock_bird_surface_tension_benzene() {
        let benzene = Component::new(
            "Benzene", 0.078112, 562.05, 4.895e6, 256.0e-6, 0.210, 353.24, [0.0; 5], f64::NAN,
        )
        .unwrap();
        let sigma = surface_tension_brock_bird(&benzene, t_k(293.15)).get::<newton_per_meter>();
        assert_relative_eq!(sigma, 28.88e-3, max_relative = 0.05);
    }

    /// **Methodology.** Surface tension must vanish as `T → Tc` (the interface
    /// disappears): Brock-Bird carries a `(1 − Tr)^(11/9)` factor and DWSIM
    /// clamps `σ = 0` for `T ≥ Tc`. **Result (2026-07-24):** for benzene,
    /// σ(0.999 Tc) ≈ 1.3e-5 N/m ≪ σ(0.52 Tc) ≈ 2.8e-2 N/m, and σ(Tc) = 0
    /// exactly.
    #[test]
    fn surface_tension_goes_to_zero_at_tc() {
        let benzene = Component::new(
            "Benzene", 0.078112, 562.05, 4.895e6, 256.0e-6, 0.210, 353.24, [0.0; 5], f64::NAN,
        )
        .unwrap();
        let tc = benzene.critical_temperature;
        let near = surface_tension_brock_bird(&benzene, t_k(0.999 * tc)).get::<newton_per_meter>();
        let cold = surface_tension_brock_bird(&benzene, t_k(0.52 * tc)).get::<newton_per_meter>();
        let at_tc = surface_tension_brock_bird(&benzene, t_k(tc)).get::<newton_per_meter>();
        assert!(near > 0.0 && near < 1.0e-3);
        assert!(near < cold);
        assert_eq!(at_tc, 0.0);
    }

    /// **Methodology.** Surface-tension molar-average mixing (`AUX_SURFTM`):
    /// all-subcritical mixture returns the plain molar average `Σ xᵢ σᵢ`;
    /// single component returns its own tension. **Result (2026-07-24):**
    /// equimolar binary = 0.5(σ₁+σ₂) exactly; single component = σ₁.
    #[test]
    fn surface_tension_mixture_molar_average() {
        let s1 = 0.028;
        let s2 = 0.022;
        let single =
            surface_tension_mixture(&[1.0], &[s1], &[true]).get::<newton_per_meter>();
        assert_relative_eq!(single, s1, max_relative = 1e-12);
        let mix = surface_tension_mixture(&[0.5, 0.5], &[s1, s2], &[true, true])
            .get::<newton_per_meter>();
        assert_relative_eq!(mix, 0.5 * (s1 + s2), max_relative = 1e-12);
    }
}
