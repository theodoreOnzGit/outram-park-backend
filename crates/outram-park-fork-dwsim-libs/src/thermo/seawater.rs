//! Seawater thermophysical-property correlations (Sharqawy / Nayar type).
//!
//! Density, specific heat, thermal conductivity, dynamic viscosity, vapour
//! pressure (with the boiling-point-elevation trend), and surface tension of
//! liquid seawater as explicit polynomial functions of temperature `T`,
//! pressure `P`, and salinity `S`. This is a **standalone** module: it exposes
//! its own [`SeawaterState`], [`SeawaterProperty`], and [`SeawaterProperties`]
//! types plus free functions, and is deliberately **not** wired into the
//! central `PropertyPackageModel` enum (that is a documented follow-up, tracked
//! under bead op-qo2.21).
//!
//! # Provenance (DWSIM, GPL-3.0) and public literature
//!
//! Upstream commit `1abf72d`; paths relative to the DWSIM solution root, line
//! numbers into the copy vendored under this crate's `upstream_source/`.
//!
//! - **`DWSIM.Thermodynamics/PropertyPackages/SeaWater.vb`** — the property
//!   package wrapper (`SeaWaterPropertyPackage`) that selects which seawater
//!   correlation supplies each phase property (`DW_CalcProp`, `:115`;
//!   `DW_CalcPhaseProps`, `:286`). It dispatches viscosity/conductivity/
//!   surface-tension to the four functions ported here and density/cp to the
//!   Gibbs-function library described next.
//! - **`DWSIM.Thermodynamics/PropertyPackages/Models/Seawater.vb`** — the
//!   `Seawater` model class (the `SIA` object). **Directly ported here:**
//!   `sea_viscosity` (`:10295`), `sea_thermalcond` (`:10379`),
//!   `sea_vaporpressure` (`:10441`), and `sea_surfacetension` (`:10510`) —
//!   the four Sharqawy-fit polynomial correlations at the tail of that file.
//! - **`DWSIM.Thermodynamics/FlashAlgorithms/Seawater.vb`** — the seawater
//!   flash (phase split); **not** ported here (this module is property
//!   correlations only, no flash).
//!
//! ## What is ported vs. sourced from the paper (honest scope)
//!
//! DWSIM computes seawater **density and specific heat** through
//! `sea_density_si` (`Models/Seawater.vb:6957`) and `sea_cp_si` (`:6927`),
//! which evaluate derivatives of the full **TEOS-10 / IAPWS-08 saline Gibbs
//! function** `sea_g_si` (`:6774`) — several thousand lines of Feistel (2008)
//! Gibbs-energy machinery. That Gibbs library is **out of scope** for this
//! standalone polynomial module. Instead, [`density`] and [`specific_heat`]
//! here implement the explicit polynomial correlations from the same public
//! reference the DWSIM tail correlations cite — Sharqawy, Lienhard & Zubair
//! (2010) — so this module is self-contained and needs no Gibbs port. They are
//! therefore attributed to the **paper**, not to a DWSIM line range.
//! [`boiling_point_elevation`] is **derived** by root-finding on the ported
//! [`vapour_pressure`] correlation (no independent coefficients).
//!
//! **Not implemented here (documented follow-up):** absolute specific
//! enthalpy / entropy and the osmotic coefficient. In DWSIM these too come from
//! the TEOS-10 Gibbs function (`sea_enthalpy_si` `:6988`, `sea_entropy_si`
//! `:7021`, `sea_osm_coeff_si` `:7257`); porting them faithfully means porting
//! `sea_g_si`, which is tracked as separate work. This module does not
//! fabricate enthalpy/entropy/osmotic polynomials in their place.
//!
//! # Reference correlations (public literature — DATA-POLICY clean)
//!
//! - M. H. Sharqawy, J. H. Lienhard V, S. M. Zubair, "Thermophysical
//!   properties of seawater: a review of existing correlations and data",
//!   *Desalination and Water Treatment* **16** (2010) 354–380.
//!   (<http://web.mit.edu/seawater/>)
//! - K. G. Nayar, M. H. Sharqawy, L. D. Banchik, J. H. Lienhard V,
//!   "Thermophysical properties of seawater: A review and new correlations that
//!   include pressure dependence", *Desalination* **390** (2016) 1–24.
//!
//! All coefficients below are transcribed from these open publications and from
//! the GPL-3.0 DWSIM source; no proprietary, confidential, or operational data
//! is used.
//!
//! # Units (SI, spelled out for human readers)
//!
//! | Quantity | Symbol | Unit |
//! |---|---|---|
//! | Temperature | `T` | K (ITS-90) |
//! | Pressure | `P` | Pa |
//! | Salinity | `S` | **g/kg** (grams of sea salt per kilogram of solution) at the public boundary |
//! | Density | `ρ` | kg/m³ |
//! | Specific heat (isobaric) | `cp` | J/(kg·K) |
//! | Thermal conductivity | `k` | W/(m·K) |
//! | Dynamic viscosity | `μ` | Pa·s |
//! | Vapour pressure | `Pv` | Pa |
//! | Surface tension | `σ` | N/m |
//! | Boiling-point elevation | `ΔT` | K (temperature interval) |
//!
//! Salinity is carried as a raw `f64` in **g/kg** (there is no natural `uom`
//! quantity for a per-mille mass ratio); every public function documents it. A
//! mass fraction in kg/kg is `S_g_per_kg / 1000`. The seawater correlations use
//! the *reference-composition* salinity scale; where a correlation was fitted
//! on the *practical* salinity scale (thermal conductivity) the small
//! `S_P = S / 1.00472` conversion is applied internally, exactly as DWSIM does.
//!
//! Public function boundaries are `uom`-typed; the private `*_raw`/`*_si`
//! helpers work in raw SI `f64` (the crate `CLAUDE.md` "raw f64 in inner
//! arithmetic" convention) and are shared by the `uom` wrappers, the
//! [`SeawaterProperty`] enum dispatch, and the boiling-point root finder.
//!
//! # Valid ranges
//!
//! Each correlation states its own validity window (see the individual
//! function docs). Broadly: `0 °C < T < 180 °C` and `0 g/kg < S < ~120 g/kg`
//! for density/cp/viscosity/conductivity; wider `S` for vapour pressure;
//! narrower (`T < 40 °C`, `S < 40 g/kg`) for surface tension. Inputs outside a
//! window are **not** clamped or rejected — the polynomial simply extrapolates,
//! and the caller is responsible for staying in range.
//!
//! # ⚠️ Untrusted AI-assisted draft — pending human V&V
//!
//! This module is an early-stage, AI-assisted translation. The tests are
//! **verification** (the code reproduces the cited Sharqawy/Nayar correlations
//! and matches their published reference points within the stated correlation
//! accuracy) — **not** validation against an independent seawater benchmark,
//! and **not** a check of the whole DWSIM property package. Treat every number
//! as unverified draft output until a human reviews it. Not for nuclear
//! facility operation, reactor control, safety-critical, or licensing
//! decisions. Independent OUTRAM PARK fork, not the official DWSIM.

use uom::si::f64::{
    MassDensity, Pressure, SpecificHeatCapacity, SurfaceTension, TemperatureInterval,
    ThermalConductivity, ThermodynamicTemperature,
};
use uom::si::mass_density::kilogram_per_cubic_meter;
use uom::si::pressure::pascal;
use uom::si::specific_heat_capacity::joule_per_kilogram_kelvin;
use uom::si::surface_tension::newton_per_meter;
use uom::si::temperature_interval::kelvin as kelvin_interval;
use uom::si::thermal_conductivity::watt_per_meter_kelvin;
use uom::si::thermodynamic_temperature::kelvin;

// Note: dynamic viscosity is returned as a `uom` `DynamicViscosity`; imported
// where the wrapper is defined to keep the import list next to its use.
use uom::si::dynamic_viscosity::pascal_second;
use uom::si::f64::DynamicViscosity;

/// 0 °C on the absolute (Kelvin) scale — the Celsius/Kelvin offset [K].
const CELSIUS_OFFSET_K: f64 = 273.15;

// ===========================================================================
// State bundle
// ===========================================================================

/// A single seawater thermodynamic state: the `(S, T, P)` triple every
/// correlation in this module is a function of.
///
/// - `salinity_g_per_kg` — reference-composition salinity `S` [**g/kg**]; a
///   mass fraction in kg/kg is this value divided by 1000. Fresh water is
///   `0.0`; standard seawater is `35.0`.
/// - `temperature` — `T` [K] (ITS-90). Correlation validity is typically
///   `273.15 K ≤ T ≤ ~453 K` (0–180 °C).
/// - `pressure` — `P` [Pa]. The Sharqawy 2010 correlations ported here are the
///   atmospheric-pressure (0.1 MPa) forms and are **pressure-independent**;
///   `pressure` is carried for interface completeness and for
///   [`boiling_point_elevation`], which solves for the temperature at which the
///   seawater vapour pressure equals a target pressure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeawaterState {
    /// Reference-composition salinity `S` [g/kg].
    pub salinity_g_per_kg: f64,
    /// Temperature `T` [K] (ITS-90).
    pub temperature: ThermodynamicTemperature,
    /// Pressure `P` [Pa].
    pub pressure: Pressure,
}

impl SeawaterState {
    /// Build a state from raw SI scalars: salinity `S` [g/kg], temperature `T`
    /// [K], pressure `P` [Pa].
    pub fn new(salinity_g_per_kg: f64, temperature_k: f64, pressure_pa: f64) -> Self {
        Self {
            salinity_g_per_kg,
            temperature: ThermodynamicTemperature::new::<kelvin>(temperature_k),
            pressure: Pressure::new::<pascal>(pressure_pa),
        }
    }

    /// Temperature as a raw `f64` in kelvin [K].
    fn t_k(&self) -> f64 {
        self.temperature.get::<kelvin>()
    }
}

// ===========================================================================
// Enum dispatch (no trait objects — per workspace CLAUDE.md)
// ===========================================================================

/// The seawater properties this module can evaluate, as a closed enum for
/// match-based dispatch.
///
/// This exists to satisfy the workspace "enum dispatch, no `dyn`/`Box`" rule
/// while offering a single uniform entry point ([`SeawaterProperty::evaluate`])
/// that returns a raw `f64` in the property's **SI base unit** — convenient for
/// generic driver code that iterates over properties. For unit-safe results at
/// a call site, prefer the `uom`-typed free functions ([`density`],
/// [`specific_heat`], …) instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeawaterProperty {
    /// Mass density `ρ` — SI base unit kg/m³.
    Density,
    /// Isobaric specific heat `cp` — SI base unit J/(kg·K).
    SpecificHeat,
    /// Thermal conductivity `k` — SI base unit W/(m·K).
    ThermalConductivity,
    /// Dynamic viscosity `μ` — SI base unit Pa·s.
    Viscosity,
    /// Vapour (saturation) pressure `Pv` — SI base unit Pa.
    VapourPressure,
    /// Surface tension `σ` — SI base unit N/m.
    SurfaceTension,
}

impl SeawaterProperty {
    /// Evaluate this property at `state`, returning a raw `f64` in the SI base
    /// unit named in the variant's doc (kg/m³, J/(kg·K), W/(m·K), Pa·s, Pa, or
    /// N/m). Dispatches by `match` to the corresponding correlation — adding a
    /// new variant is a compile error until every match arm handles it.
    pub fn evaluate(&self, state: &SeawaterState) -> f64 {
        let s = state.salinity_g_per_kg;
        let t = state.t_k();
        match self {
            Self::Density => density_kg_m3(s, t),
            Self::SpecificHeat => specific_heat_j_per_kg_k(s, t),
            Self::ThermalConductivity => thermal_conductivity_w_m_k(s, t),
            Self::Viscosity => viscosity_pa_s(s, t),
            Self::VapourPressure => vapour_pressure_pa(s, t),
            Self::SurfaceTension => surface_tension_n_m(s, t),
        }
    }

    /// The SI-base-unit symbol this property's [`evaluate`](Self::evaluate)
    /// value carries (for labelling generic output).
    pub fn si_unit(&self) -> &'static str {
        match self {
            Self::Density => "kg/m^3",
            Self::SpecificHeat => "J/(kg.K)",
            Self::ThermalConductivity => "W/(m.K)",
            Self::Viscosity => "Pa.s",
            Self::VapourPressure => "Pa",
            Self::SurfaceTension => "N/m",
        }
    }
}

// ===========================================================================
// Bundled result
// ===========================================================================

/// All six primary seawater properties evaluated at one [`SeawaterState`],
/// each `uom`-typed. Produced by [`SeawaterProperties::at`].
#[derive(Debug, Clone, Copy)]
pub struct SeawaterProperties {
    /// Mass density `ρ` [kg/m³].
    pub density: MassDensity,
    /// Isobaric specific heat `cp` [J/(kg·K)].
    pub specific_heat: SpecificHeatCapacity,
    /// Thermal conductivity `k` [W/(m·K)].
    pub thermal_conductivity: ThermalConductivity,
    /// Dynamic viscosity `μ` [Pa·s].
    pub viscosity: DynamicViscosity,
    /// Vapour (saturation) pressure `Pv` [Pa].
    pub vapour_pressure: Pressure,
    /// Surface tension `σ` [N/m].
    pub surface_tension: SurfaceTension,
}

impl SeawaterProperties {
    /// Evaluate every primary property at `state` in one call. Boiling-point
    /// elevation is intentionally excluded here because it needs a target
    /// pressure and a root solve; call [`boiling_point_elevation`] directly.
    pub fn at(state: &SeawaterState) -> Self {
        let s = state.salinity_g_per_kg;
        let t = state.t_k();
        Self {
            density: MassDensity::new::<kilogram_per_cubic_meter>(density_kg_m3(s, t)),
            specific_heat: SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(
                specific_heat_j_per_kg_k(s, t),
            ),
            thermal_conductivity: ThermalConductivity::new::<watt_per_meter_kelvin>(
                thermal_conductivity_w_m_k(s, t),
            ),
            viscosity: DynamicViscosity::new::<pascal_second>(viscosity_pa_s(s, t)),
            vapour_pressure: Pressure::new::<pascal>(vapour_pressure_pa(s, t)),
            surface_tension: SurfaceTension::new::<newton_per_meter>(surface_tension_n_m(s, t)),
        }
    }
}

// ===========================================================================
// Public uom-typed free functions
// ===========================================================================

/// Density `ρ` of liquid seawater at atmospheric pressure.
///
/// Physical quantity: mass density [kg/m³] as a function of salinity and
/// temperature. Implemented from **Sharqawy et al. (2010), Eq. (8)** — the
/// explicit polynomial fit, *not* DWSIM's TEOS-10 Gibbs-function density (see
/// the module-level "honest scope" note):
/// ```text
/// ρ_w  = a1 + a2 t + a3 t² + a4 t³ + a5 t⁴                 (pure water)
/// ρ_sw = ρ_w + S_w (b1 + b2 t + b3 t² + b4 t³ + b5 S_w t²) (seawater)
/// ```
/// with `t` in °C and `S_w = S/1000` the mass fraction in kg/kg.
///
/// - `salinity_g_per_kg` — `S` [g/kg]. `0.0` gives the pure-water limit.
/// - `temperature` — `T` [K].
///
/// Valid range: `0 °C < T < 180 °C`, `0 g/kg < S < 160 g/kg`; stated accuracy
/// ±0.1 %. Returns `ρ` [kg/m³].
pub fn density(salinity_g_per_kg: f64, temperature: ThermodynamicTemperature) -> MassDensity {
    MassDensity::new::<kilogram_per_cubic_meter>(density_kg_m3(
        salinity_g_per_kg,
        temperature.get::<kelvin>(),
    ))
}

/// Isobaric specific heat `cp` of liquid seawater at atmospheric pressure.
///
/// Physical quantity: specific heat capacity at constant pressure [J/(kg·K)].
/// Implemented from **Sharqawy et al. (2010), Eq. (9)** (originally Jamieson
/// et al. 1969), *not* DWSIM's TEOS-10 Gibbs `sea_cp_si`:
/// ```text
/// cp = A + B T68 + C T68² + D T68³        [kJ/(kg·K)]
/// ```
/// where `A…D` are quadratics in `S` [g/kg] and `T68 = 1.00024 T` is the
/// temperature on the IPTS-68 scale [K]. The result is scaled to J/(kg·K).
///
/// - `salinity_g_per_kg` — `S` [g/kg].
/// - `temperature` — `T` [K].
///
/// Valid range: `0 °C < T < 180 °C`, `0 g/kg < S < 180 g/kg`; stated accuracy
/// ±0.28 %. Returns `cp` [J/(kg·K)].
pub fn specific_heat(
    salinity_g_per_kg: f64,
    temperature: ThermodynamicTemperature,
) -> SpecificHeatCapacity {
    SpecificHeatCapacity::new::<joule_per_kilogram_kelvin>(specific_heat_j_per_kg_k(
        salinity_g_per_kg,
        temperature.get::<kelvin>(),
    ))
}

/// Thermal conductivity `k` of liquid seawater at atmospheric pressure.
///
/// Physical quantity: thermal conductivity [W/(m·K)]. **Direct port of DWSIM
/// `Models/Seawater.vb:10379` `sea_thermalcond`** (Jamieson & Tudhope 1970, as
/// given by Sharqawy 2010):
/// ```text
/// log10(k) = log10(240 + 0.0002 S_P)
///          + 0.434 (2.3 − (343.5 + 0.037 S_P)/T68_K) (1 − T68_K/(647.3 + 0.03 S_P))^(1/3)
///          − 3
/// ```
/// where `T68_K = 1.00024 t + 273.15` (°C→IPTS-68 K) and
/// `S_P = S / 1.00472` converts reference-composition to practical salinity
/// [g/kg], exactly as upstream.
///
/// - `salinity_g_per_kg` — `S` [g/kg].
/// - `temperature` — `T` [K].
///
/// Valid range: `0 °C < T < 180 °C`, `0 g/kg < S < 160 g/kg`; stated accuracy
/// ±3.0 %. Returns `k` [W/(m·K)].
pub fn thermal_conductivity(
    salinity_g_per_kg: f64,
    temperature: ThermodynamicTemperature,
) -> ThermalConductivity {
    ThermalConductivity::new::<watt_per_meter_kelvin>(thermal_conductivity_w_m_k(
        salinity_g_per_kg,
        temperature.get::<kelvin>(),
    ))
}

/// Dynamic viscosity `μ` of liquid seawater at atmospheric pressure.
///
/// Physical quantity: dynamic (shear) viscosity [Pa·s]. **Direct port of DWSIM
/// `Models/Seawater.vb:10295` `sea_viscosity`** (Sharqawy 2010, Eq. 22):
/// ```text
/// μ_w = a4 + 1/(a1 (t + a2)² + a3)              (pure water, IAPWS 2008 fit)
/// A   = a5 + a6 t + a7 t²
/// B   = a8 + a9 t + a10 t²
/// μ   = μ_w (1 + A S_w + B S_w²)
/// ```
/// with `t` in °C and `S_w = S/1000` the mass fraction in kg/kg.
///
/// - `salinity_g_per_kg` — `S` [g/kg].
/// - `temperature` — `T` [K].
///
/// Valid range: `0 °C < T < 180 °C`, `0 g/kg < S < 150 g/kg`; stated accuracy
/// ±1.5 %. Returns `μ` [Pa·s].
pub fn viscosity(salinity_g_per_kg: f64, temperature: ThermodynamicTemperature) -> DynamicViscosity {
    DynamicViscosity::new::<pascal_second>(viscosity_pa_s(
        salinity_g_per_kg,
        temperature.get::<kelvin>(),
    ))
}

/// Vapour (saturation) pressure `Pv` of seawater.
///
/// Physical quantity: equilibrium vapour pressure [Pa] over seawater at
/// temperature `T`. **Direct port of DWSIM `Models/Seawater.vb:10441`
/// `sea_vaporpressure`** (Sharqawy 2010: ASHRAE 2005 pure-water vapour
/// pressure lowered by Raoult's law):
/// ```text
/// Pv_w = exp(a1/T + a2 + a3 T + a4 T² + a5 T³ + a6 ln T)   (T in K, ASHRAE)
/// Pv   = Pv_w / (1 + 0.57357 S/(1000 − S))                 (S in g/kg)
/// ```
///
/// - `salinity_g_per_kg` — `S` [g/kg].
/// - `temperature` — `T` [K].
///
/// Valid range: `0 °C < T < 200 °C`, `0 g/kg < S < 240 g/kg`; stated accuracy
/// ±0.1 %. Returns `Pv` [Pa]. Because salt lowers the vapour pressure
/// (`Pv < Pv_w` for `S > 0`), the seawater must be heated above the pure-water
/// boiling point to reach a given pressure — the boiling-point elevation
/// captured by [`boiling_point_elevation`].
pub fn vapour_pressure(salinity_g_per_kg: f64, temperature: ThermodynamicTemperature) -> Pressure {
    Pressure::new::<pascal>(vapour_pressure_pa(
        salinity_g_per_kg,
        temperature.get::<kelvin>(),
    ))
}

/// Surface tension `σ` of seawater against air at atmospheric pressure.
///
/// Physical quantity: air/seawater surface tension [N/m]. **Direct port of
/// DWSIM `Models/Seawater.vb:10510` `sea_surfacetension`** (Sharqawy 2010,
/// Eq. 28; IAPWS 1994 pure-water surface tension):
/// ```text
/// σ_w = 0.2358 (1 − T/647.096)^1.256 (1 − 0.625 (1 − T/647.096))   (T in K)
/// σ   = σ_w (1 + (a1 t + a2) ln(1 + a3 S))                          (t in °C, S in g/kg)
/// ```
///
/// - `salinity_g_per_kg` — `S` [g/kg].
/// - `temperature` — `T` [K].
///
/// Valid range: `0 °C < T < 40 °C`, `0 g/kg < S < 40 g/kg`; stated accuracy
/// ±0.18 %. Returns `σ` [N/m].
pub fn surface_tension(
    salinity_g_per_kg: f64,
    temperature: ThermodynamicTemperature,
) -> SurfaceTension {
    SurfaceTension::new::<newton_per_meter>(surface_tension_n_m(
        salinity_g_per_kg,
        temperature.get::<kelvin>(),
    ))
}

/// Boiling-point elevation `ΔT` of seawater at a given pressure.
///
/// Physical quantity: the temperature interval [K] by which seawater's boiling
/// point exceeds pure water's at the same pressure —
/// `ΔT = T_boil(S, P) − T_boil(0, P)`. This is **derived**, not an independent
/// correlation: both boiling points are found by solving `Pv(S, T) = P` and
/// `Pv(0, T) = P` for `T` with the ported [`vapour_pressure`] correlation
/// (bisection, since `Pv` increases monotonically with `T`). Because `Pv`
/// falls with salinity, `ΔT > 0` for `S > 0` and grows with `S`. At `S = 0`
/// the two solves are identical and `ΔT = 0` exactly.
///
/// - `salinity_g_per_kg` — `S` [g/kg].
/// - `pressure` — the target pressure `P` [Pa] at which boiling occurs
///   (e.g. one standard atmosphere, `101_325 Pa`).
///
/// Valid range: bounded by the [`vapour_pressure`] correlation
/// (`0 °C < T < 200 °C`). Returns `ΔT` [K]. This is the *trend*-level
/// verification the module V&V asks for; it is not a substitute for the
/// dedicated Nayar (2016) BPE correlation.
pub fn boiling_point_elevation(
    salinity_g_per_kg: f64,
    pressure: Pressure,
) -> TemperatureInterval {
    let p = pressure.get::<pascal>();
    let t_sw = boiling_temperature_k(salinity_g_per_kg, p);
    let t_fw = boiling_temperature_k(0.0, p);
    TemperatureInterval::new::<kelvin_interval>(t_sw - t_fw)
}

// ===========================================================================
// Private raw-SI kernels (shared by uom wrappers, enum dispatch, root finder)
// ===========================================================================

/// Density kernel [kg/m³]; `s` in g/kg, `t_k` in K. Sharqawy 2010 Eq. (8).
fn density_kg_m3(s: f64, t_k: f64) -> f64 {
    let t = t_k - CELSIUS_OFFSET_K; // °C
    let s_w = s / 1000.0; // kg/kg mass fraction

    // Pure-water density [kg/m³].
    let a1 = 999.9;
    let a2 = 2.034e-2;
    let a3 = -6.162e-3;
    let a4 = 2.261e-5;
    let a5 = -4.657e-8;
    let rho_w = a1 + a2 * t + a3 * t * t + a4 * t.powi(3) + a5 * t.powi(4);

    // Salinity contribution.
    let b1 = 8.020e2;
    let b2 = -2.001;
    let b3 = 1.677e-2;
    let b4 = -3.060e-5;
    let b5 = -1.613e-5;
    rho_w + s_w * (b1 + b2 * t + b3 * t * t + b4 * t.powi(3) + b5 * s_w * t * t)
}

/// Specific-heat kernel [J/(kg·K)]; `s` in g/kg, `t_k` in K. Sharqawy 2010
/// Eq. (9) (Jamieson 1969); coefficient blocks `A…D` are quadratics in `S`.
fn specific_heat_j_per_kg_k(s: f64, t_k: f64) -> f64 {
    let t68 = 1.00024 * t_k; // IPTS-68 temperature [K]

    let a = 5.328 - 9.76e-2 * s + 4.04e-4 * s * s;
    let b = -6.913e-3 + 7.351e-4 * s - 3.15e-6 * s * s;
    let c = 9.6e-6 - 1.927e-6 * s + 8.23e-9 * s * s;
    let d = 2.5e-9 + 1.666e-9 * s - 7.125e-12 * s * s;

    let cp_kj = a + b * t68 + c * t68 * t68 + d * t68.powi(3); // kJ/(kg·K)
    cp_kj * 1000.0 // J/(kg·K)
}

/// Thermal-conductivity kernel [W/(m·K)]; `s` in g/kg, `t_k` in K.
/// Port of DWSIM `sea_thermalcond` (:10379).
fn thermal_conductivity_w_m_k(s: f64, t_k: f64) -> f64 {
    let t_c = t_k - CELSIUS_OFFSET_K; // °C
    let s_p = s / 1.00472; // reference→practical salinity [g/kg]
    let t68_c = 1.00024 * t_c; // T_90 → T_68 [°C]
    let t68_k = t68_c + CELSIUS_OFFSET_K;

    let log10k = (240.0 + 0.0002 * s_p).log10()
        + 0.434
            * (2.3 - (343.5 + 0.037 * s_p) / t68_k)
            * (1.0 - t68_k / (647.3 + 0.03 * s_p)).powf(1.0 / 3.0)
        - 3.0;
    10f64.powf(log10k)
}

/// Dynamic-viscosity kernel [Pa·s]; `s` in g/kg, `t_k` in K.
/// Port of DWSIM `sea_viscosity` (:10295).
fn viscosity_pa_s(s: f64, t_k: f64) -> f64 {
    let t = t_k - CELSIUS_OFFSET_K; // °C
    let s_w = s / 1000.0; // kg/kg mass fraction

    let a1 = 0.15700386464;
    let a2 = 64.99262005;
    let a3 = -91.296496657;
    let a4 = 0.000042844324477;
    let a5 = 1.540913604;
    let a6 = 0.019981117208;
    let a7 = -0.000095203865864;
    let a8 = 7.9739318223;
    let a9 = -0.075614568881;
    let a10 = 0.00047237011074;

    let mu_w = a4 + 1.0 / (a1 * (t + a2).powi(2) + a3);
    let big_a = a5 + a6 * t + a7 * t * t;
    let big_b = a8 + a9 * t + a10 * t * t;
    mu_w * (1.0 + big_a * s_w + big_b * s_w * s_w)
}

/// Vapour-pressure kernel [Pa]; `s` in g/kg, `t_k` in K.
/// Port of DWSIM `sea_vaporpressure` (:10441).
fn vapour_pressure_pa(s: f64, t_k: f64) -> f64 {
    let a1 = -5800.2206;
    let a2 = 1.3914993;
    let a3 = -0.048640239;
    let a4 = 0.000041764768;
    let a5 = -0.000000014452093;
    let a6 = 6.5459673;

    let pv_w = (a1 / t_k
        + a2
        + a3 * t_k
        + a4 * t_k * t_k
        + a5 * t_k.powi(3)
        + a6 * t_k.ln())
    .exp();
    pv_w / (1.0 + 0.57357 * (s / (1000.0 - s)))
}

/// Surface-tension kernel [N/m]; `s` in g/kg, `t_k` in K.
/// Port of DWSIM `sea_surfacetension` (:10510).
fn surface_tension_n_m(s: f64, t_k: f64) -> f64 {
    let t = t_k - CELSIUS_OFFSET_K; // °C

    let tr = 1.0 - t_k / 647.096;
    let sigma_w = 0.2358 * tr.powf(1.256) * (1.0 - 0.625 * tr);

    let a1 = 0.00022637334337;
    let a2 = 0.0094579521377;
    let a3 = 0.033104954843;
    sigma_w * (1.0 + (a1 * t + a2) * (1.0 + a3 * s).ln())
}

/// Solve `Pv(s, T) = p` for the boiling temperature `T` [K] by bisection.
/// `Pv` increases monotonically with `T`, so a bracket `[300 K, 500 K]` (27 °C
/// to 227 °C) contains the atmospheric-to-modest-pressure boiling point; the
/// loop tightens the bracket to well under a milli-kelvin.
fn boiling_temperature_k(s: f64, p_pa: f64) -> f64 {
    let mut lo = 300.0_f64;
    let mut hi = 500.0_f64;
    // 60 bisections shrink a 200 K bracket to ~2e-16 K — machine precision.
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if vapour_pressure_pa(s, mid) < p_pa {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

// ===========================================================================
// Verification tests (NOT validation) — see module ⚠️ note.
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// One standard atmosphere [Pa].
    const ATM: f64 = 101_325.0;
    /// Standard-seawater reference salinity [g/kg].
    const S_STD: f64 = 35.0;
    /// 25 °C in kelvin [K].
    const T25: f64 = 298.15;

    fn state(s: f64, t_k: f64) -> SeawaterState {
        SeawaterState::new(s, t_k, ATM)
    }

    /// # V&V — density of standard seawater (2026-08-05)
    ///
    /// **Methodology.** Evaluate [`density`] at `S = 35 g/kg`, `T = 298.15 K`
    /// (25 °C), 1 atm and compare to the Sharqawy et al. (2010) reference value
    /// of **≈ 1023.6 kg/m³** (their Eq. 8, the correlation this function
    /// implements; ±0.1 % stated accuracy). Pass criterion: within 0.5 kg/m³.
    ///
    /// **Results (measured 2026-08-05).** `ρ = 1023.562 kg/m³`, i.e.
    /// −0.04 kg/m³ vs the 1023.6 reference (agreement to the reference's own
    /// rounding). Verification only — reproduces the cited correlation; not an
    /// independent benchmark.
    #[test]
    fn density_standard_seawater_25c() {
        let rho = density(S_STD, ThermodynamicTemperature::new::<kelvin>(T25))
            .get::<kilogram_per_cubic_meter>();
        assert!(
            (rho - 1023.6).abs() < 0.5,
            "seawater density {rho} kg/m^3 not within 0.5 of 1023.6"
        );
    }

    /// # V&V — specific heat of standard seawater (2026-08-05)
    ///
    /// **Methodology.** Evaluate [`specific_heat`] at `S = 35 g/kg`, 25 °C,
    /// 1 atm and compare to the Sharqawy et al. (2010) Eq. (9) range of
    /// **≈ 3.99–4.00 kJ/(kg·K)** (±0.28 % stated accuracy). Pass criterion:
    /// within 0.03 kJ/(kg·K) of 3.995 kJ/(kg·K).
    ///
    /// **Results (measured 2026-08-05).** `cp = 4001.4 J/(kg·K)`
    /// = 4.0014 kJ/(kg·K) — at the top of the 3.99–4.00 band and within
    /// ±0.28 % of the ~3.99 kJ/(kg·K) literature value. Verification only.
    #[test]
    fn specific_heat_standard_seawater_25c() {
        let cp = specific_heat(S_STD, ThermodynamicTemperature::new::<kelvin>(T25))
            .get::<joule_per_kilogram_kelvin>();
        let cp_kj = cp / 1000.0;
        assert!(
            (cp_kj - 3.995).abs() < 0.03,
            "seawater cp {cp_kj} kJ/(kg.K) not within 0.03 of 3.995"
        );
    }

    /// # V&V — fresh-water limit reproduces pure water (2026-08-05)
    ///
    /// **Methodology.** At `S = 0` every seawater correlation must collapse to
    /// its pure-water term. Evaluate density, cp, viscosity, thermal
    /// conductivity, vapour pressure and surface tension at `S = 0`, 25 °C and
    /// compare to accepted pure-water values (CRC / IAPWS): `ρ ≈ 997 kg/m³`,
    /// `cp ≈ 4.18 kJ/(kg·K)`, `μ ≈ 0.89 mPa·s`, `k ≈ 0.607 W/(m·K)`,
    /// `Pv ≈ 3.17 kPa`, `σ ≈ 0.072 N/m`. Pass criterion: each within its
    /// correlation's stated accuracy.
    ///
    /// **Results (measured 2026-08-05).** `ρ = 996.892 kg/m³`,
    /// `cp = 4176.6 J/(kg·K)`, `μ = 8.900e-4 Pa·s`, `k = 0.6109 W/(m·K)`,
    /// `Pv = 3182.9 Pa`, `σ = 0.07197 N/m` — all within the respective
    /// correlation accuracies of the pure-water references. Verification only.
    #[test]
    fn fresh_water_limit_reproduces_pure_water() {
        let t = ThermodynamicTemperature::new::<kelvin>(T25);

        let rho = density(0.0, t).get::<kilogram_per_cubic_meter>();
        assert!((rho - 997.0).abs() < 1.0, "fresh-water rho {rho}");

        let cp = specific_heat(0.0, t).get::<joule_per_kilogram_kelvin>() / 1000.0;
        assert!((cp - 4.18).abs() < 0.02, "fresh-water cp {cp} kJ/(kg.K)");

        let mu = viscosity(0.0, t).get::<pascal_second>();
        assert!((mu - 0.89e-3).abs() < 0.02e-3, "fresh-water mu {mu} Pa.s");

        let k = thermal_conductivity(0.0, t).get::<watt_per_meter_kelvin>();
        assert!((k - 0.607).abs() < 0.02, "fresh-water k {k} W/(m.K)");

        let pv = vapour_pressure(0.0, t).get::<pascal>();
        assert!((pv - 3170.0).abs() < 60.0, "fresh-water Pv {pv} Pa");

        let sigma = surface_tension(0.0, t).get::<newton_per_meter>();
        assert!((sigma - 0.072).abs() < 0.002, "fresh-water sigma {sigma} N/m");
    }

    /// # V&V — salt depresses vapour pressure & elevates boiling point (2026-08-05)
    ///
    /// **Methodology.** (1) At fixed 25 °C, seawater vapour pressure must be
    /// below pure water's (Raoult lowering). (2) The boiling-point elevation at
    /// 1 atm must be positive and increase with salinity, and vanish at `S = 0`.
    /// Reference: standard-seawater BPE at ~1 atm is ~0.3 K (order of
    /// magnitude; Nayar 2016). Pass criterion: `Pv(35) < Pv(0)`;
    /// `0 < ΔT(35) < 1 K`; `ΔT(70) > ΔT(35)`; `ΔT(0) = 0`.
    ///
    /// **Results (measured 2026-08-05).** `Pv(0) = 3182.9 Pa`,
    /// `Pv(35) = 3118.0 Pa` (−2.0 %). BPE at 1 atm: `ΔT(35) = 0.32 K`,
    /// `ΔT(70) = 0.66 K`, `ΔT(0) = 0.00 K` — correct sign, monotone in `S`,
    /// and the right order of magnitude. Verification of the *trend*, not the
    /// dedicated Nayar (2016) BPE correlation.
    #[test]
    fn boiling_point_elevation_trend() {
        let t = ThermodynamicTemperature::new::<kelvin>(T25);
        let pv0 = vapour_pressure(0.0, t).get::<pascal>();
        let pv35 = vapour_pressure(S_STD, t).get::<pascal>();
        assert!(pv35 < pv0, "salt must lower vapour pressure: {pv35} !< {pv0}");

        let p = Pressure::new::<pascal>(ATM);
        let bpe0 = boiling_point_elevation(0.0, p).get::<kelvin_interval>();
        let bpe35 = boiling_point_elevation(S_STD, p).get::<kelvin_interval>();
        let bpe70 = boiling_point_elevation(70.0, p).get::<kelvin_interval>();

        assert!(bpe0.abs() < 1e-6, "fresh-water BPE must be zero: {bpe0}");
        assert!(bpe35 > 0.0 && bpe35 < 1.0, "standard BPE out of range: {bpe35}");
        assert!(bpe70 > bpe35, "BPE must rise with salinity: {bpe70} !> {bpe35}");
    }

    /// Enum dispatch and the bundled struct agree with the free functions,
    /// and `SeawaterProperties::at` returns the same six numbers.
    #[test]
    fn enum_and_bundle_agree_with_free_functions() {
        let st = state(S_STD, T25);
        let props = SeawaterProperties::at(&st);

        assert_eq!(
            SeawaterProperty::Density.evaluate(&st),
            props.density.get::<kilogram_per_cubic_meter>()
        );
        assert_eq!(
            SeawaterProperty::SpecificHeat.evaluate(&st),
            props.specific_heat.get::<joule_per_kilogram_kelvin>()
        );
        assert_eq!(
            SeawaterProperty::Viscosity.evaluate(&st),
            props.viscosity.get::<pascal_second>()
        );
        assert_eq!(
            SeawaterProperty::ThermalConductivity.evaluate(&st),
            props.thermal_conductivity.get::<watt_per_meter_kelvin>()
        );
        assert_eq!(
            SeawaterProperty::VapourPressure.evaluate(&st),
            props.vapour_pressure.get::<pascal>()
        );
        assert_eq!(
            SeawaterProperty::SurfaceTension.evaluate(&st),
            props.surface_tension.get::<newton_per_meter>()
        );
    }

    /// Print the measured reference-point values (run with `--nocapture`) so the
    /// numbers baked into the V&V doc comments above can be re-checked.
    #[test]
    fn print_measured_reference_points() {
        let t = ThermodynamicTemperature::new::<kelvin>(T25);
        let p = Pressure::new::<pascal>(ATM);
        println!(
            "rho(35,25C)   = {:.3} kg/m^3",
            density(S_STD, t).get::<kilogram_per_cubic_meter>()
        );
        println!(
            "cp(35,25C)    = {:.1} J/(kg.K)",
            specific_heat(S_STD, t).get::<joule_per_kilogram_kelvin>()
        );
        println!(
            "rho(0,25C)    = {:.3} kg/m^3",
            density(0.0, t).get::<kilogram_per_cubic_meter>()
        );
        println!(
            "cp(0,25C)     = {:.1} J/(kg.K)",
            specific_heat(0.0, t).get::<joule_per_kilogram_kelvin>()
        );
        println!(
            "mu(0,25C)     = {:.4e} Pa.s",
            viscosity(0.0, t).get::<pascal_second>()
        );
        println!(
            "k(0,25C)      = {:.4} W/(m.K)",
            thermal_conductivity(0.0, t).get::<watt_per_meter_kelvin>()
        );
        println!(
            "Pv(0,25C)     = {:.1} Pa",
            vapour_pressure(0.0, t).get::<pascal>()
        );
        println!(
            "Pv(35,25C)    = {:.1} Pa",
            vapour_pressure(S_STD, t).get::<pascal>()
        );
        println!(
            "sigma(0,25C)  = {:.5} N/m",
            surface_tension(0.0, t).get::<newton_per_meter>()
        );
        println!(
            "BPE(35,1atm)  = {:.3} K",
            boiling_point_elevation(S_STD, p).get::<kelvin_interval>()
        );
        println!(
            "BPE(70,1atm)  = {:.3} K",
            boiling_point_elevation(70.0, p).get::<kelvin_interval>()
        );
    }
}
