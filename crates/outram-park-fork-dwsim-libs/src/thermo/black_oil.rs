//! Black-Oil property package — petroleum PVT correlations.
//!
//! Pure-Rust port of DWSIM's Black-Oil property package (GPL-3.0), Visual-Basic
//! reference source (all at commit `1abf72d`):
//!
//! - `DWSIM.Thermodynamics/PropertyPackages/Models/BlackOilProperties.vb`
//!   — the correlation engine `BlackOilProperties`:
//!     - `LiquidMolecularWeight` L47-49, `VaporMolecularWeight` L211-213,
//!       `LiquidNormalBoilingPoint` L43-46,
//!     - solution GOR `Rs` and bubble-point `Pb` (Standing) inside
//!       `LiquidDensity` L101-135 / `LiquidViscosity` L157-210,
//!     - saturated oil FVF `Bos` (Standing) L125 / L187,
//!     - oil density `LiquidDensity` L101-135,
//!     - dead / saturated / undersaturated oil viscosity (Beggs-Robinson,
//!       Vazquez-Beggs) `LiquidViscosity` L157-210,
//!     - gas pseudo-criticals `Ppc`/`Tpc` (Standing) L265-266 / L318-319,
//!     - gas compressibility `Z` (Dranchuk-Abou-Kassem)
//!       `VaporCompressibilityFactor` L308-359,
//!     - gas density `VaporDensity` L229-235,
//!     - gas viscosity (Dempsey/Standing) `VaporViscosity` L261-307.
//! - `DWSIM.Thermodynamics/PropertyPackages/BlackOil.vb`
//!   — property-package glue `BlackOilPropertyPackage`:
//!     - undersaturated oil FVF `Boss` with Vazquez-Beggs oil compressibility
//!       `C` and gas-gravity-at-100-psi correction `SGfg100`, gas FVF `Bg`,
//!       water FVF `Bw`, and the gas/oil/water stream split `DW_CalcXY`
//!       L594-719.
//!
//! **This is an untrusted AI-assisted draft pending human verification &
//! validation.** It reproduces the correlations' *form and internal
//! consistency* (verification); it is **not** validated against experimental
//! PVT data. Independent OUTRAM PARK fork, not the official DWSIM. Not for
//! nuclear facility operation, reactor control, safety-critical, or licensing
//! decisions.
//!
//! ## What black-oil is
//!
//! The black-oil model treats a reservoir hydrocarbon system as two components
//! — a non-volatile "oil" (stock-tank oil) and a "gas" that dissolves into and
//! evolves out of the oil as pressure changes — plus an inert water phase. The
//! PVT surface is captured by a small set of empirical correlations in the
//! oil's API gravity, the gas specific gravity, the producing gas-oil ratio,
//! temperature, and pressure. DWSIM uses the classic set:
//!
//! - **Standing (1947)** — solution gas-oil ratio `Rs`, bubble-point pressure
//!   `Pb`, and saturated oil formation volume factor `Bo`.
//! - **Vazquez & Beggs (1980)** — undersaturated oil compressibility (hence
//!   `Bo` above the bubble point) and undersaturated oil viscosity.
//! - **Beggs & Robinson (1975)** — dead-oil and saturated-oil viscosity.
//! - **Standing (1977)** — gas pseudo-critical properties.
//! - **Dranchuk & Abou-Kassem (1975)** — gas compressibility factor `Z`
//!   (an 11-constant fit of the Standing-Katz chart).
//! - **Dempsey (1965)** — gas viscosity (a polynomial in reduced pressure /
//!   temperature). *Note:* the task brief mentions a "Lee" gas-viscosity
//!   correlation; DWSIM does **not** use Lee-Gonzalez-Eakin here — it uses
//!   Dempsey — so this port follows DWSIM (Dempsey).
//!
//! ## Units — SI at the public boundary, field units internally
//!
//! Every public function takes and returns **SI**:
//!
//! | Quantity | SI unit (this API) |
//! |---|---|
//! | Pressure | Pa ([`Pascals`]) |
//! | Temperature | K ([`Kelvin`]) |
//! | Density | kg/m³ ([`KgPerCubicMetre`]) |
//! | Dynamic viscosity | Pa·s ([`PascalSeconds`]) |
//! | Solution GOR `Rs`, producing GOR | Sm³/Sm³ ([`GasOilRatio`]) |
//! | Oil / water FVF, gas FVF | m³/m³ (reservoir / standard, [`FormationVolumeFactor`]) |
//! | Molar mass | g/mol ([`GramsPerMole`]) |
//! | Compressibility factor `Z`, mole/mass fractions | dimensionless |
//!
//! The correlations were fit in **oilfield field units** (°API, psia, °F,
//! scf/STB, cP, rb/STB, rankine), so the inner arithmetic converts SI → field
//! units, evaluates the published form verbatim, and converts back. Each
//! conversion is a named `const` below with its factor spelled out. The gas
//! specific gravity (air = 1) and oil specific gravity (water = 1 at 60 °F) are
//! dimensionless inputs, not converted.
//!
//! **DWSIM's GOR conversion constant is `5.6738` scf/STB per Sm³/Sm³**
//! ([`SCF_STB_PER_SM3_SM3`]); the more common textbook value is 5.615. This
//! port preserves DWSIM's constant so single-point numbers match DWSIM, at the
//! cost of a ~1 % offset from a 5.615-based reference.
//!
//! **Gas constant:** the black-oil correlations hard-code `R = 8.314` in DWSIM;
//! this port preserves that (see [`R_DWSIM`]) so the gas-density and
//! `Z`-derived numbers reproduce DWSIM exactly.
//!
//! ## Design (crate `CLAUDE.md`)
//!
//! Documented raw `f64` (SI) at the boundary with named type aliases, matching
//! the rest of [`crate::thermo`]. Enum dispatch — **no `dyn` / `Box` /
//! lifetimes** — for the one genuine branch DWSIM makes: whether the oil is at
//! or below its bubble point ([`SaturationRegime`]), which selects the
//! saturated vs. undersaturated `Bo` and oil-viscosity forms.
//!
//! ## Honest scope — what is and is NOT ported
//!
//! **Ported (self-contained petroleum correlations):**
//! - [`api_gravity`] / [`oil_specific_gravity_from_api`] conversions,
//! - [`vapor_molecular_weight`], [`liquid_molecular_weight`],
//!   [`liquid_normal_boiling_point`],
//! - [`solution_gor`] (Standing `Rs`), [`bubble_point_pressure`] (Standing `Pb`),
//! - [`oil_fvf_saturated`], [`oil_compressibility`] (Vazquez-Beggs),
//!   [`oil_fvf`] (regime-dispatched),
//! - [`oil_density`] (DWSIM `LiquidDensity`, hydrocarbon part),
//! - [`gas_pseudo_critical_pressure`] / [`gas_pseudo_critical_temperature`],
//! - [`gas_compressibility_factor`] (Dranchuk-Abou-Kassem),
//! - [`gas_density`], [`gas_fvf`], [`water_fvf`],
//! - [`gas_viscosity`] (Dempsey), [`dead_oil_viscosity`],
//!   [`saturated_oil_viscosity`], [`undersaturated_oil_viscosity`],
//!   [`oil_viscosity`] (regime-dispatched),
//! - [`water_cut_blend`] (the `(100−BSW)/100·hc + BSW/100·water` mixing rule),
//! - [`stream_split`] (DWSIM `DW_CalcXY`, the gas/oil/water mass split).
//!
//! **NOT ported (depend on routines outside this standalone module):**
//! - the black-oil *vapour pressure* (`BlackOilProperties.VaporPressure`
//!   L50-73) and *enthalpy / Cp / Cv / thermal conductivity* (L74-260 tail),
//!   which call the Lee-Kesler petroleum-characterisation methods
//!   (`props1.Tc_LeeKesler`, `Pc_LeeKesler`, `AcentricFactor_LeeKesler`,
//!   `PROPS.Cpig_lk`, `condl_latini`, `condtg_elyhanley`) and the IAPWS-IF97
//!   water package — neither is present in this module. [`water_cut_blend`]
//!   exposes the mixing rule so a caller that *has* a water-property source can
//!   still assemble the blended value.
//! - the Twu two-reference-point measured-viscosity path
//!   (`LiquidViscosity` L159-161, `props1.ViscTwu`) — only the correlation
//!   path (`v1 = 0`) is ported.
//! - the DWSIM flash driver and CAPE-OPEN plumbing (not physics).

#![forbid(unsafe_code)]

// ---------------------------------------------------------------------------
// Named type aliases — SI at the public boundary (crate `CLAUDE.md`).
// ---------------------------------------------------------------------------

/// Absolute pressure, pascals (Pa). SI.
pub type Pascals = f64;
/// Absolute temperature, kelvin (K). SI.
pub type Kelvin = f64;
/// Mass density, kilograms per cubic metre (kg/m³). SI.
pub type KgPerCubicMetre = f64;
/// Dynamic (absolute) viscosity, pascal-seconds (Pa·s). SI.
pub type PascalSeconds = f64;
/// Molar mass, grams per mole (g/mol) — i.e. kg/kmol. This is the customary
/// molar-mass magnitude DWSIM carries (e.g. air ≈ 28.97).
pub type GramsPerMole = f64;
/// Gas-oil ratio, standard-m³ gas per standard-m³ oil (Sm³/Sm³). Used for both
/// the producing GOR (an input) and the solution GOR `Rs` (a result).
pub type GasOilRatio = f64;
/// Formation volume factor — reservoir volume per standard (stock-tank) volume,
/// m³/m³. Dimensionless in value; the name records the reservoir/standard basis.
pub type FormationVolumeFactor = f64;
/// API gravity of the stock-tank oil, degrees API (°API). SI-adjacent field
/// unit; `°API = 141.5 / SG − 131.5` with `SG` referred to water at 60 °F.
pub type ApiGravity = f64;
/// Specific gravity, dimensionless. Oil SG is referred to water at 60 °F;
/// gas SG is referred to air (SG_air = 1).
pub type SpecificGravity = f64;
/// A dimensionless quantity (compressibility factor, mole/mass fraction).
pub type Dimensionless = f64;

// ---------------------------------------------------------------------------
// Conversion constants — each factor spelled out with its meaning.
// ---------------------------------------------------------------------------

/// psia per pascal. `1 Pa = 0.000145038 psi` (DWSIM's rounded factor, used
/// verbatim so numbers match — `BlackOilProperties.vb` L109/L272/L314).
pub const PSIA_PER_PA: f64 = 0.000_145_038;

/// Pascals per psia — the exact reciprocal `1 / PSIA_PER_PA` (≈ 6894.76 Pa/psi),
/// used to convert field-unit pressures back to SI at the boundary.
pub const PA_PER_PSIA: f64 = 1.0 / PSIA_PER_PA;

/// scf/STB per Sm³/Sm³ — DWSIM's gas-oil-ratio conversion constant
/// (`GORss = GOR * 5.6738`, `BlackOil.vb` L613, `BlackOilProperties.vb` L115).
/// The common textbook value is 5.615; DWSIM's 5.6738 is preserved here so the
/// port reproduces DWSIM.
pub const SCF_STB_PER_SM3_SM3: f64 = 5.6738;

/// Gas constant hard-coded by DWSIM's black-oil routines, J/(mol·K)
/// (`BlackOilProperties.vb` L232). Preserved (rather than CODATA `R`) so the
/// gas-density path reproduces DWSIM to all figures.
pub const R_DWSIM: f64 = 8.314;

/// Convert kelvin to the correlations' native degrees Fahrenheit.
/// `°F = (T[K] − 273.15) · 9/5 + 32` (`BlackOilProperties.vb` L107).
#[inline]
fn fahrenheit_from_kelvin(t_k: Kelvin) -> f64 {
    (t_k - 273.15) * 9.0 / 5.0 + 32.0
}

/// Convert kelvin to degrees Rankine (absolute Fahrenheit) via °F.
/// `°R = °F + 459.67` (`BlackOilProperties.vb` L108).
#[inline]
fn rankine_from_kelvin(t_k: Kelvin) -> f64 {
    fahrenheit_from_kelvin(t_k) + 459.67
}

/// Base-10 logarithm helper (DWSIM writes `Log(x)/Log(10)` for `log10`).
#[inline]
fn log10(x: f64) -> f64 {
    x.log10()
}

// ---------------------------------------------------------------------------
// Saturation regime — the one enum branch DWSIM makes (enum dispatch, no dyn).
// ---------------------------------------------------------------------------

/// Whether the oil is **at/below** its bubble point (gas still dissolving as
/// pressure drops — "saturated") or **above** it (single-phase liquid being
/// compressed — "undersaturated").
///
/// DWSIM selects the oil FVF and oil-viscosity correlation form by comparing
/// the evaluation pressure `P` with the bubble-point pressure `Pb`
/// (`BlackOil.vb` L638, `BlackOilProperties.vb` L204). This enum makes that
/// closed choice explicit for enum dispatch — no trait objects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturationRegime {
    /// `P < Pb`: gas is coming out of solution; use the Standing saturated
    /// forms (`Bos`, `muos`).
    Saturated,
    /// `P >= Pb`: no free gas; use the Vazquez-Beggs undersaturated forms
    /// (`Boss`, `muoss`) that correct the bubble-point value for compression.
    Undersaturated,
}

impl SaturationRegime {
    /// Classify by comparing pressure to the bubble-point pressure (both Pa).
    ///
    /// DWSIM uses the strict test `If Ppsia < Pb Then <saturated> Else
    /// <undersaturated>` (`BlackOil.vb` L638), so exactly `P = Pb` is treated
    /// as undersaturated here, matching DWSIM.
    #[inline]
    pub fn classify(pressure_pa: Pascals, bubble_point_pa: Pascals) -> Self {
        if pressure_pa < bubble_point_pa {
            SaturationRegime::Saturated
        } else {
            SaturationRegime::Undersaturated
        }
    }
}

// ---------------------------------------------------------------------------
// Gravities & molecular weights.
// ---------------------------------------------------------------------------

/// API gravity of the stock-tank oil from its specific gravity.
///
/// `°API = 141.5 / SG_oil − 131.5` (`BlackOilProperties.vb` L111 /
/// `BlackOil.vb` L609). Physical quantity: oil density expressed on the API
/// scale. Valid for `SG_oil` in roughly `0.6..1.08` (≈ 100..−5 °API); light
/// crudes are 30–45 °API, heavy crudes 10–22 °API.
///
/// - `oil_sg` — oil specific gravity (water = 1 at 60 °F), dimensionless.
/// - returns — API gravity, °API.
#[inline]
pub fn api_gravity(oil_sg: SpecificGravity) -> ApiGravity {
    141.5 / oil_sg - 131.5
}

/// Oil specific gravity from API gravity — inverse of [`api_gravity`].
///
/// `SG_oil = 141.5 / (°API + 131.5)`. Convenience for callers who carry API.
///
/// - `api` — API gravity, °API.
/// - returns — oil specific gravity (water = 1), dimensionless.
#[inline]
pub fn oil_specific_gravity_from_api(api: ApiGravity) -> SpecificGravity {
    141.5 / (api + 131.5)
}

/// Apparent molecular weight of the black-oil "gas", g/mol.
///
/// `MW_gas = SG_gas · 28.97` — the gas gravity times the molar mass of air
/// (`BlackOilProperties.vb` L211-213). Valid for `SG_gas` ≈ 0.55..1.5.
///
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - returns — gas molar mass, g/mol.
#[inline]
pub fn vapor_molecular_weight(gas_sg: SpecificGravity) -> GramsPerMole {
    gas_sg * 28.97
}

/// Apparent molecular weight of the black-oil liquid, g/mol.
///
/// Water-cut-weighted blend of a hydrocarbon MW correlation and water (18):
/// `MW = (100−BSW)/100 · ((ln(1.07 − SG_oil) − 3.56073)/(−2.93886))^10
/// + BSW/100 · 18` (`BlackOilProperties.vb` L47-49). Valid for `SG_oil < 1.07`
/// (the `ln(1.07 − SG_oil)` argument must stay positive) and `BSW` in `0..100`.
///
/// - `oil_sg` — oil specific gravity (water = 1), dimensionless.
/// - `bsw_percent` — basic sediment & water cut, **percent** (0..100).
/// - returns — liquid molar mass, g/mol.
#[inline]
pub fn liquid_molecular_weight(oil_sg: SpecificGravity, bsw_percent: f64) -> GramsPerMole {
    let hc = ((( 1.07 - oil_sg).ln() - 3.560_73) / -2.938_86).powi(10);
    (100.0 - bsw_percent) / 100.0 * hc + bsw_percent / 100.0 * 18.0
}

/// Normal boiling point of the (water-free) oil pseudo-component, K.
///
/// `NBP = 1080 − exp(6.97996 − 0.01964 · MW^(2/3))`, with `MW` the water-free
/// liquid molecular weight ([`liquid_molecular_weight`] at `BSW = 0`)
/// (`BlackOilProperties.vb` L43-46, L56). Feeds the (deferred) Lee-Kesler
/// caloric routines; exposed here because it is self-contained.
///
/// - `oil_sg` — oil specific gravity (water = 1), dimensionless.
/// - returns — normal boiling point, K.
#[inline]
pub fn liquid_normal_boiling_point(oil_sg: SpecificGravity) -> Kelvin {
    let mw = liquid_molecular_weight(oil_sg, 0.0);
    1080.0 - (6.979_96 - 0.019_64 * mw.powf(2.0 / 3.0)).exp()
}

// ---------------------------------------------------------------------------
// Standing solution GOR, bubble point, and oil FVF.
// ---------------------------------------------------------------------------

/// Solution gas-oil ratio `Rs` (Standing 1947), Sm³/Sm³.
///
/// Volume of gas dissolved in the oil at the given pressure and temperature:
/// `Rs = SG_gas · ((P[psia]/18.2 + 1.4) · 10^(0.0125·API − 0.00091·T[°F]))^1.2048`
/// (`BlackOilProperties.vb` L119). Evaluated in field units and returned in SI
/// by dividing the scf/STB result by [`SCF_STB_PER_SM3_SM3`].
///
/// Standing's fit is intended for `100 < P < 4000` psia, `100 < T < 258` °F,
/// `16 < °API < 64`, `0.59 < SG_gas < 0.95`; it is a smooth extrapolation
/// outside that box. Below the bubble point `Rs` is the physically dissolved
/// gas; at/above `Pb` the correlation keeps rising and should be capped at the
/// producing GOR by the caller (DWSIM does this via `Pb`).
///
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - `oil_sg` — oil specific gravity (water = 1), dimensionless.
/// - returns — solution GOR `Rs`, Sm³/Sm³.
pub fn solution_gor(
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
    oil_sg: SpecificGravity,
) -> GasOilRatio {
    let tf = fahrenheit_from_kelvin(temperature_k);
    let api = api_gravity(oil_sg);
    let p_psia = pressure_pa * PSIA_PER_PA;
    let rs_scf_stb =
        gas_sg * ((p_psia / 18.2 + 1.4) * 10f64.powf(0.0125 * api - 0.00091 * tf)).powf(1.2048);
    rs_scf_stb / SCF_STB_PER_SM3_SM3
}

/// Bubble-point pressure `Pb` (Standing 1947), Pa.
///
/// The pressure at which the producing gas-oil ratio just becomes fully
/// dissolved — the exact inverse of [`solution_gor`]:
/// `Pb = 18.2 · ((GOR[scf/STB]/SG_gas)^(1/1.2048) · 10^(0.00091·T[°F] −
/// 0.0125·API) − 1.4)` (`BlackOilProperties.vb` L121). By construction,
/// `solution_gor(Pb, T, …)` recovers the input GOR (see the V&V test).
///
/// Same applicability box as [`solution_gor`].
///
/// - `gor_sm3_sm3` — producing gas-oil ratio, Sm³/Sm³.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - `oil_sg` — oil specific gravity (water = 1), dimensionless.
/// - returns — bubble-point pressure `Pb`, Pa.
pub fn bubble_point_pressure(
    gor_sm3_sm3: GasOilRatio,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
    oil_sg: SpecificGravity,
) -> Pascals {
    let tf = fahrenheit_from_kelvin(temperature_k);
    let api = api_gravity(oil_sg);
    let gor_ss = gor_sm3_sm3 * SCF_STB_PER_SM3_SM3;
    let pb_psia = 18.2
        * ((gor_ss / gas_sg).powf(1.0 / 1.2048) * 10f64.powf(0.00091 * tf - 0.0125 * api) - 1.4);
    pb_psia * PA_PER_PSIA
}

/// Saturated oil formation volume factor `Bo` (Standing 1947), m³/m³.
///
/// Reservoir volume of oil-plus-dissolved-gas per stock-tank volume, at or
/// below the bubble point:
/// `Bo = 0.9759 + 0.00012 · (Rs[scf/STB]·(SG_gas/SG_oil)^0.5 + 1.25·T[°F])^1.2`
/// (`BlackOilProperties.vb` L125). Increases with dissolved gas `Rs`.
///
/// - `rs_sm3_sm3` — solution GOR `Rs`, Sm³/Sm³ (from [`solution_gor`]).
/// - `temperature_k` — temperature, K.
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - `oil_sg` — oil specific gravity (water = 1), dimensionless.
/// - returns — saturated oil FVF `Bo`, m³/m³ (reservoir per standard).
pub fn oil_fvf_saturated(
    rs_sm3_sm3: GasOilRatio,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
    oil_sg: SpecificGravity,
) -> FormationVolumeFactor {
    let tf = fahrenheit_from_kelvin(temperature_k);
    let rs = rs_sm3_sm3 * SCF_STB_PER_SM3_SM3;
    0.9759 + 0.00012 * (rs * (gas_sg / oil_sg).sqrt() + 1.25 * tf).powf(1.2)
}

/// Undersaturated oil compressibility coefficient `C` (Vazquez-Beggs 1980),
/// dimensionless exponent used in `Boss = Bos · (Pb/P)^C`.
///
/// From DWSIM's `DW_CalcXY` (`BlackOil.vb` L632-634):
/// `SGfg100 = SG_gas · (1 + 0.00005912·API·T[°F]·log10(P[psia]/114.7))`
/// (the gas gravity corrected to a 100-psia separator), then
/// `C = 0.0001 · (2.81·GOR + 3.1·T[°F] + 171/SGfg100 − 118·SGfg100 − 1102)`,
/// where DWSIM feeds `GOR` in **Sm³/Sm³** (its `BO_GOR`). Reproduced verbatim.
///
/// - `gor_sm3_sm3` — producing gas-oil ratio, Sm³/Sm³.
/// - `pressure_pa` — pressure (used as the separator pressure, per DWSIM), Pa.
/// - `temperature_k` — temperature (used as the separator temperature), K.
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - `oil_sg` — oil specific gravity (water = 1), dimensionless.
/// - returns — the compressibility exponent `C`, dimensionless.
pub fn oil_compressibility(
    gor_sm3_sm3: GasOilRatio,
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
    oil_sg: SpecificGravity,
) -> Dimensionless {
    let tf = fahrenheit_from_kelvin(temperature_k);
    let api = api_gravity(oil_sg);
    let p_psia = pressure_pa * PSIA_PER_PA;
    let sg_fg_100 = gas_sg * (1.0 + 0.000_059_12 * api * tf * log10(p_psia / 114.7));
    0.0001 * (2.81 * gor_sm3_sm3 + 3.1 * tf + 171.0 / sg_fg_100 - 118.0 * sg_fg_100 - 1102.0)
}

/// Oil formation volume factor `Bo`, regime-dispatched, m³/m³.
///
/// Reproduces DWSIM's `DW_CalcXY` branch (`BlackOil.vb` L625-638):
/// - **Saturated** (`P < Pb`): the Standing value `Bos` from
///   [`oil_fvf_saturated`] at the *local* `Rs`.
/// - **Undersaturated** (`P >= Pb`): compress the bubble-point value,
///   `Boss = Bos(Pb) · (Pb/P)^C`, with `Bos(Pb)` evaluated at the saturating
///   `Rs` (i.e. the full producing GOR) and `C` from [`oil_compressibility`].
///
/// The [`SaturationRegime`] enum makes the branch explicit (enum dispatch).
/// `Bo` increases monotonically with dissolved gas below `Pb`. Above `Pb` the
/// trend is set by the *sign* of the Vazquez-Beggs compressibility `C`
/// ([`oil_compressibility`]): with the textbook positive `C`, `Bo` decreases
/// under compression; but `C` can go **negative** at low producing GOR (a known
/// Vazquez-Beggs artifact — e.g. `C = −0.024` for the worked example below), in
/// which case DWSIM's `Bos·(Pb/P)^C` makes `Bo` rise slightly above `Pb`. This
/// port reproduces that behaviour verbatim rather than clamping it.
///
/// - `gor_sm3_sm3` — producing gas-oil ratio, Sm³/Sm³.
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` / `oil_sg` — specific gravities (air = 1 / water = 1).
/// - returns — oil FVF `Bo`, m³/m³ (reservoir per standard).
pub fn oil_fvf(
    gor_sm3_sm3: GasOilRatio,
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
    oil_sg: SpecificGravity,
) -> FormationVolumeFactor {
    let pb = bubble_point_pressure(gor_sm3_sm3, temperature_k, gas_sg, oil_sg);
    match SaturationRegime::classify(pressure_pa, pb) {
        SaturationRegime::Saturated => {
            let rs = solution_gor(pressure_pa, temperature_k, gas_sg, oil_sg);
            oil_fvf_saturated(rs, temperature_k, gas_sg, oil_sg)
        }
        SaturationRegime::Undersaturated => {
            // Bos evaluated at the saturating Rs (= producing GOR), per DWSIM.
            let bos = oil_fvf_saturated(gor_sm3_sm3, temperature_k, gas_sg, oil_sg);
            let c = oil_compressibility(gor_sm3_sm3, pressure_pa, temperature_k, gas_sg, oil_sg);
            let p_psia = pressure_pa * PSIA_PER_PA;
            let pb_psia = pb * PSIA_PER_PA;
            bos * (pb_psia / p_psia).powf(c)
        }
    }
}

/// Hydrocarbon (water-free) live-oil density, kg/m³ — DWSIM `LiquidDensity`.
///
/// `rho_oil = (SG_oil·997 + Rs[scf/STB]/5.6738) / Bos`
/// (`BlackOilProperties.vb` L129-131), i.e. the stock-tank oil mass plus the
/// dissolved-gas mass, divided by the saturated FVF `Bos`.
///
/// **Faithful-port note:** DWSIM's `LiquidDensity` always uses the *saturated*
/// `Bos` (even above `Pb`) and adds the dissolved-gas term as `Rs/5.6738`
/// (without a gas-density factor). The separate `DW_CalcXY` density
/// (`BlackOil.vb` L691) instead uses `rhog0·Rs/5.6738` and the regime-correct
/// `Bo`. Both are DWSIM inconsistencies; this function reproduces
/// `LiquidDensity` exactly. The water-cut blend is left to [`water_cut_blend`].
///
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` / `oil_sg` — specific gravities.
/// - returns — hydrocarbon live-oil density, kg/m³.
pub fn oil_density(
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
    oil_sg: SpecificGravity,
) -> KgPerCubicMetre {
    let rs = solution_gor(pressure_pa, temperature_k, gas_sg, oil_sg);
    let bos = oil_fvf_saturated(rs, temperature_k, gas_sg, oil_sg);
    let rs_scf = rs * SCF_STB_PER_SM3_SM3;
    let rho_oil_std = oil_sg * 997.0;
    (rho_oil_std + rs_scf / SCF_STB_PER_SM3_SM3) / bos
}

// ---------------------------------------------------------------------------
// Gas pseudo-criticals, Z, density, FVF, viscosity.
// ---------------------------------------------------------------------------

/// Gas pseudo-critical pressure (Standing 1977), Pa.
///
/// `Ppc[psia] = 677 + 15·SG_gas − 37.5·SG_gas²`
/// (`BlackOilProperties.vb` L265 / L318). Valid for `SG_gas` ≈ 0.55..1.0
/// (natural-gas systems). Returned in SI.
///
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - returns — pseudo-critical pressure, Pa.
#[inline]
pub fn gas_pseudo_critical_pressure(gas_sg: SpecificGravity) -> Pascals {
    let ppc_psia = 677.0 + 15.0 * gas_sg - 37.5 * gas_sg * gas_sg;
    ppc_psia * PA_PER_PSIA
}

/// Gas pseudo-critical temperature (Standing 1977), K.
///
/// `Tpc[°R] = 168 + 325·SG_gas − 12.5·SG_gas²`
/// (`BlackOilProperties.vb` L266 / L319). Valid for `SG_gas` ≈ 0.55..1.0.
/// Converted from rankine to kelvin (`K = °R · 5/9`).
///
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - returns — pseudo-critical temperature, K.
#[inline]
pub fn gas_pseudo_critical_temperature(gas_sg: SpecificGravity) -> Kelvin {
    let tpc_rankine = 168.0 + 325.0 * gas_sg - 12.5 * gas_sg * gas_sg;
    tpc_rankine * 5.0 / 9.0
}

/// Gas compressibility factor `Z` (Dranchuk & Abou-Kassem 1975), dimensionless.
///
/// Solves the 11-constant DAK fit of the Standing-Katz `Z`-chart by fixed-point
/// iteration on the reduced density `rho_pr = 0.27·Ppr/(Z·Tpr)`
/// (`BlackOilProperties.vb` L308-359). Iterates until `|ΔZ| < 1e-4` or 1000
/// iterations (DWSIM's exact loop guard). Valid over `0.2 ≤ Ppr ≤ 30`,
/// `1.0 ≤ Tpr ≤ 3.0`.
///
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - returns — compressibility factor `Z`, dimensionless.
pub fn gas_compressibility_factor(
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
) -> Dimensionless {
    let trank = rankine_from_kelvin(temperature_k);
    let p_psia = pressure_pa * PSIA_PER_PA;

    let ppc = 677.0 + 15.0 * gas_sg - 37.5 * gas_sg * gas_sg;
    let tpc = 168.0 + 325.0 * gas_sg - 12.5 * gas_sg * gas_sg;

    // Dranchuk-Abou-Kassem constants (BlackOilProperties.vb L323-333).
    let a1 = 0.3265;
    let a2 = -1.07;
    let a3 = -0.5339;
    let a4 = 0.01569;
    let a5 = -0.05165;
    let a6 = 0.5475;
    let a7 = -0.7361;
    let a8 = 0.1844;
    let a9 = 0.1056;
    let a10 = 0.6134;
    let a11 = 0.721;

    let ppr = p_psia / ppc;
    let tpr = trank / tpc;

    let mut z = 1.0_f64;
    let mut cnt = 0_u32;
    loop {
        let rhopr = 0.27 * ppr / (z * tpr);
        let c1 = a1 + a2 / tpr + a3 / tpr.powi(3) + a4 / tpr.powi(4) + a5 / tpr.powi(5);
        let c2 = a6 + a7 / tpr + a8 / (tpr * tpr);
        let c3 = a7 / tpr + a8 / (tpr * tpr);
        let z_ant = z;
        z = 1.0 + c1 * rhopr + c2 * rhopr.powi(2) - a9 * c3 * rhopr.powi(5)
            + a10 * (1.0 + a11 * rhopr * rhopr) * (rhopr * rhopr / tpr.powi(3))
                * (-a11 * rhopr * rhopr).exp();
        cnt += 1;
        if (z - z_ant).abs() < 0.0001 || cnt > 1000 {
            break;
        }
    }
    z
}

/// Gas mass density, kg/m³ — DWSIM `VaporDensity`.
///
/// Real-gas density `rho = P·MW / (Z·R·T)` written as DWSIM does
/// (`BlackOilProperties.vb` L229-235):
/// `rho = 1 / (R·Z·T/P) · (SG_gas·29.97)/1000`.
///
/// **Faithful-port note:** DWSIM multiplies by `SG_gas·29.97` here, whereas its
/// own [`vapor_molecular_weight`] uses `SG_gas·28.97` (air's molar mass). The
/// `29.97` is almost certainly a DWSIM typo; it is reproduced verbatim (a ~3.5 %
/// density bias) and flagged rather than silently "fixed", per the
/// untrusted-draft policy. Uses [`R_DWSIM`] = 8.314.
///
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - returns — gas density, kg/m³.
pub fn gas_density(
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
) -> KgPerCubicMetre {
    let z = gas_compressibility_factor(pressure_pa, temperature_k, gas_sg);
    let molar_volume = R_DWSIM * z * temperature_k / pressure_pa; // m³/mol-ish (DWSIM units)
    1.0 / molar_volume * (gas_sg * 29.97) / 1000.0
}

/// Gas formation volume factor `Bg`, reservoir-m³ per standard-m³ (m³/m³).
///
/// `Bg = 0.02827 · Z · T[°R] / P[psia]` (`BlackOil.vb` L683). DWSIM's constant
/// yields `Bg` in **reservoir ft³ / scf**; the ratio is dimensionless in value,
/// so it is returned as-is (the caller treats it as a volume ratio). Decreases
/// with pressure.
///
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - returns — gas FVF `Bg`, volume ratio (see note), dimensionless.
pub fn gas_fvf(
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
) -> FormationVolumeFactor {
    let z = gas_compressibility_factor(pressure_pa, temperature_k, gas_sg);
    let trank = rankine_from_kelvin(temperature_k);
    let p_psia = pressure_pa * PSIA_PER_PA;
    0.02827 * z * trank / p_psia
}

/// Gas viscosity (Dempsey 1965 / Standing), Pa·s — DWSIM `VaporViscosity`.
///
/// Low-pressure viscosity `mug1` (Standing) scaled by the Dempsey polynomial in
/// reduced pressure/temperature (`BlackOilProperties.vb` L261-307):
/// `mug1 = (1.709e-5 − 2.062e-6·SG_gas)·T[°F] + 0.008188 − 0.00615·log10(SG_gas)`,
/// then `mug = mug1/Tpr · exp(C(Ppr,Tpr))` with the 16-constant Dempsey `C`.
/// DWSIM returns `mug·0.001`, i.e. the polynomial is in cP and the result is
/// converted to Pa·s (`1 cP = 1e-3 Pa·s`). Valid `1 ≤ Tpr ≤ 3`, `1 ≤ Ppr ≤ 20`.
///
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` — gas specific gravity (air = 1), dimensionless.
/// - returns — gas dynamic viscosity, Pa·s.
pub fn gas_viscosity(
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
) -> PascalSeconds {
    let ppc = 677.0 + 15.0 * gas_sg - 37.5 * gas_sg * gas_sg;
    let tpc = 168.0 + 325.0 * gas_sg - 12.5 * gas_sg * gas_sg;

    let tf = fahrenheit_from_kelvin(temperature_k);
    let trank = rankine_from_kelvin(temperature_k);
    let p_psia = pressure_pa * PSIA_PER_PA;

    let ppr = p_psia / ppc;
    let tpr = trank / tpc;

    let mug1 = (0.000_017_09 - 0.000_002_062 * gas_sg) * tf + 0.008_188
        - 0.006_15 * log10(gas_sg);

    // Dempsey C-polynomial constants (BlackOilProperties.vb L284-299).
    let a: [f64; 16] = [
        -2.462_118_2,
        2.970_547_14,
        -0.286_264_054,
        0.008_054_205_22,
        2.808_609_49,
        -3.498_033_05,
        0.360_373_02,
        -0.010_443_241_3,
        -0.793_385_684,
        1.396_433_06,
        -0.149_144_925,
        0.004_410_155_12,
        0.083_938_717_8,
        -0.186_408_848,
        0.020_336_788_1,
        -0.000_609_579_263,
    ];

    let c = a[0] + a[1] * ppr + a[2] * ppr.powi(2) + a[3] * ppr.powi(3)
        + tpr * (a[4] + a[5] * ppr + a[6] * ppr.powi(2) + a[7] * ppr.powi(3))
        + tpr.powi(2) * (a[8] + a[9] * ppr + a[10] * ppr.powi(2) + a[11] * ppr.powi(3))
        + tpr.powi(3) * (a[12] + a[13] * ppr + a[14] * ppr.powi(2) + a[15] * ppr.powi(3));

    let mug = mug1 / tpr * c.exp();
    mug * 0.001
}

/// Dead-oil (gas-free) viscosity (Beggs & Robinson 1975), Pa·s.
///
/// `mu_od[cP] = −1 + 10^(10^(1.8653 − 0.025086·API − 0.5644·log10(T[°F])))`
/// (`BlackOilProperties.vb` L196). Valid `70 < T < 295` °F, `16 < °API < 58`.
/// Returned in Pa·s (`·1e-3`).
///
/// - `temperature_k` — temperature, K.
/// - `oil_sg` — oil specific gravity (water = 1), dimensionless.
/// - returns — dead-oil dynamic viscosity, Pa·s.
pub fn dead_oil_viscosity(temperature_k: Kelvin, oil_sg: SpecificGravity) -> PascalSeconds {
    let tf = fahrenheit_from_kelvin(temperature_k);
    let api = api_gravity(oil_sg);
    let mu_od_cp = -1.0 + 10f64.powf(10f64.powf(1.8653 - 0.025086 * api - 0.5644 * log10(tf)));
    mu_od_cp * 1e-3
}

/// Saturated (gas-charged, at/below `Pb`) oil viscosity
/// (Beggs & Robinson 1975), Pa·s.
///
/// `mu_os[cP] = 10.715·(Rs+100)^(−0.515) · mu_od^(5.44·(Rs+150)^(−0.338))`
/// with `Rs` in scf/STB and `mu_od` in cP (`BlackOilProperties.vb` L198).
/// Dissolved gas thins the oil, so `mu_os` falls as `Rs` rises.
///
/// - `rs_sm3_sm3` — solution GOR `Rs`, Sm³/Sm³.
/// - `dead_oil_visc_pa_s` — dead-oil viscosity, Pa·s (from [`dead_oil_viscosity`]).
/// - returns — saturated live-oil dynamic viscosity, Pa·s.
pub fn saturated_oil_viscosity(
    rs_sm3_sm3: GasOilRatio,
    dead_oil_visc_pa_s: PascalSeconds,
) -> PascalSeconds {
    let rs = rs_sm3_sm3 * SCF_STB_PER_SM3_SM3;
    let mu_od_cp = dead_oil_visc_pa_s * 1e3;
    let mu_os_cp =
        10.715 * (rs + 100.0).powf(-0.515) * mu_od_cp.powf(5.44 * (rs + 150.0).powf(-0.338));
    mu_os_cp * 1e-3
}

/// Undersaturated (above `Pb`) oil viscosity (Vazquez & Beggs 1980), Pa·s.
///
/// Corrects the bubble-point (saturated-at-full-GOR) viscosity `mu_os_sat` for
/// compression above `Pb`:
/// `mu_oss = mu_os_sat · (Pb/P)^(2.6·P[psia]^1.187 · 10^(−3.9e-5·P[psia] − 5))`
/// (`BlackOilProperties.vb` L202). `mu_os_sat` is [`saturated_oil_viscosity`]
/// evaluated at the *producing GOR* (the `GORss` term at L200).
///
/// **Faithful-port note (apparent DWSIM sign inversion):** the standard
/// Vazquez-Beggs undersaturated form uses `(P/Pb)^m` with `m > 0`, so viscosity
/// *rises* above `Pb`. DWSIM instead wrote `(Pb/P)^m` (numerator/denominator
/// swapped), so its viscosity *falls* above `Pb` — the opposite of the expected
/// physical trend and almost certainly a DWSIM bug. It is reproduced verbatim
/// here and flagged, per the untrusted-draft policy.
///
/// - `mu_os_saturated_pa_s` — saturated viscosity at the bubble point, Pa·s.
/// - `pressure_pa` — pressure, Pa.
/// - `bubble_point_pa` — bubble-point pressure `Pb`, Pa.
/// - returns — undersaturated oil dynamic viscosity, Pa·s.
pub fn undersaturated_oil_viscosity(
    mu_os_saturated_pa_s: PascalSeconds,
    pressure_pa: Pascals,
    bubble_point_pa: Pascals,
) -> PascalSeconds {
    let p_psia = pressure_pa * PSIA_PER_PA;
    let pb_psia = bubble_point_pa * PSIA_PER_PA;
    let exponent = 2.6 * p_psia.powf(1.187) * 10f64.powf(-0.000_039 * p_psia - 5.0);
    mu_os_saturated_pa_s * (pb_psia / p_psia).powf(exponent)
}

/// Hydrocarbon live-oil viscosity, regime-dispatched, Pa·s — DWSIM
/// `LiquidViscosity` (correlation path, `v1 = 0`).
///
/// Reproduces `BlackOilProperties.vb` L163-206:
/// - **Saturated** (`P < Pb`): `mu_os` at the local `Rs`.
/// - **Undersaturated** (`P >= Pb`): `mu_oss`, i.e. the bubble-point-saturated
///   viscosity (`mu_os` at the *producing GOR*) compressed via
///   [`undersaturated_oil_viscosity`].
///
/// The [`SaturationRegime`] enum selects the branch (enum dispatch). The Twu
/// two-point measured-viscosity path (`v1 ≠ 0`) is **not** ported; this is the
/// correlation-only path. Water-cut blending is left to [`water_cut_blend`].
///
/// - `gor_sm3_sm3` — producing gas-oil ratio, Sm³/Sm³.
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` / `oil_sg` — specific gravities (air = 1 / water = 1).
/// - returns — hydrocarbon live-oil dynamic viscosity, Pa·s.
pub fn oil_viscosity(
    gor_sm3_sm3: GasOilRatio,
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
    oil_sg: SpecificGravity,
) -> PascalSeconds {
    let mu_od = dead_oil_viscosity(temperature_k, oil_sg);
    let pb = bubble_point_pressure(gor_sm3_sm3, temperature_k, gas_sg, oil_sg);
    match SaturationRegime::classify(pressure_pa, pb) {
        SaturationRegime::Saturated => {
            let rs = solution_gor(pressure_pa, temperature_k, gas_sg, oil_sg);
            saturated_oil_viscosity(rs, mu_od)
        }
        SaturationRegime::Undersaturated => {
            // Saturated viscosity evaluated at the producing GOR (= Rs at Pb).
            let mu_os_sat = saturated_oil_viscosity(gor_sm3_sm3, mu_od);
            undersaturated_oil_viscosity(mu_os_sat, pressure_pa, pb)
        }
    }
}

/// Water formation volume factor `Bw`, m³/m³ — DWSIM `DW_CalcXY`.
///
/// Temperature-and-pressure polynomial (`BlackOil.vb` L697-701):
/// `Bw = A1 + A2·P[psia] + A3·P[psia]²`, with `A1,A2,A3` quadratic in `T[°F]`.
/// Near unity for reservoir brine.
///
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - returns — water FVF `Bw`, m³/m³ (reservoir per standard).
pub fn water_fvf(pressure_pa: Pascals, temperature_k: Kelvin) -> FormationVolumeFactor {
    let tf = fahrenheit_from_kelvin(temperature_k);
    let p_psia = pressure_pa * PSIA_PER_PA;
    let a1 = 0.9911 + 0.000_063_5 * tf + 0.000_000_85 * tf * tf;
    let a2 = -0.000_001_093 - 0.000_000_003_497 * tf + 0.000_000_000_004_57 * tf * tf;
    let a3 = -0.000_000_000_05 + 0.000_000_000_000_642_9 * tf - 0.000_000_000_000_001_43 * tf * tf;
    a1 + a2 * p_psia + a3 * p_psia * p_psia
}

/// Water-cut blending rule — `(100−BSW)/100·hc + BSW/100·water`.
///
/// Every DWSIM black-oil liquid property is the water-cut-weighted average of a
/// hydrocarbon correlation value and the corresponding water value (e.g.
/// `BlackOilProperties.vb` L133 for density). The water value comes from
/// DWSIM's IAPWS-IF97 package, which is **not** part of this standalone module,
/// so this helper takes the water value as an explicit argument: a caller that
/// has a water-property source (density, viscosity, …) can assemble the blend.
///
/// - `hydrocarbon_value` — the hydrocarbon-phase property (any unit).
/// - `water_value` — the water-phase property in the **same unit**.
/// - `bsw_percent` — basic sediment & water cut, percent (0..100).
/// - returns — the blended property, same unit as the inputs.
#[inline]
pub fn water_cut_blend(hydrocarbon_value: f64, water_value: f64, bsw_percent: f64) -> f64 {
    (100.0 - bsw_percent) / 100.0 * hydrocarbon_value + bsw_percent / 100.0 * water_value
}

/// Gas / oil / water mass split of a black-oil stream (dimensionless fractions).
///
/// The mass fractions returned by DWSIM's `DW_CalcXY` (`BlackOil.vb`
/// L640-711), which drive the black-oil flash. In reservoir conditions the
/// free-gas fraction is the produced GOR minus the dissolved `Rs`, converted to
/// mass through the phase densities:
/// `x_g = (rho_g·Bg·(GOR−Rs)/5.6738) / denom`,
/// `x_w = rho_w·Bw·WOR / denom`, `x_o = 1 − x_g − x_w` (clamped at 0),
/// with `denom = rho_g0·GOR + rho_o0 + rho_w0·WOR`, `WOR = BSW/(100−BSW)`,
/// and standard-condition densities `rho_w0 = 997`, `rho_g0 = SG_gas·1.22`,
/// `rho_o0 = SG_oil·997`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StreamSplit {
    /// Free-gas mass fraction, dimensionless (0..1).
    pub gas_fraction: Dimensionless,
    /// Oil mass fraction, dimensionless (0..1).
    pub oil_fraction: Dimensionless,
    /// Water mass fraction, dimensionless (0..1).
    pub water_fraction: Dimensionless,
}

/// Compute the gas/oil/water mass split — DWSIM `DW_CalcXY` (`BlackOil.vb`
/// L640-711).
///
/// Uses the regime-correct `Bo` ([`oil_fvf`]), the gas FVF `Bg` ([`gas_fvf`]),
/// the water FVF `Bw` ([`water_fvf`]), and the `DW_CalcXY` density definitions
/// (which, note, differ subtly from [`oil_density`] — see that function's
/// note). Reproduces DWSIM's arithmetic verbatim, including its
/// standard-condition gas density `rho_g0 = SG_gas·1.22`.
///
/// - `gor_sm3_sm3` — producing gas-oil ratio, Sm³/Sm³.
/// - `pressure_pa` — pressure, Pa.
/// - `temperature_k` — temperature, K.
/// - `gas_sg` / `oil_sg` — specific gravities (air = 1 / water = 1).
/// - `bsw_percent` — basic sediment & water cut, percent (0..100).
/// - returns — the [`StreamSplit`] mass fractions.
pub fn stream_split(
    gor_sm3_sm3: GasOilRatio,
    pressure_pa: Pascals,
    temperature_k: Kelvin,
    gas_sg: SpecificGravity,
    oil_sg: SpecificGravity,
    bsw_percent: f64,
) -> StreamSplit {
    let rs = solution_gor(pressure_pa, temperature_k, gas_sg, oil_sg);
    let rs_scf = rs * SCF_STB_PER_SM3_SM3;
    let gor_ss = gor_sm3_sm3 * SCF_STB_PER_SM3_SM3;
    let wor = bsw_percent / (100.0 - bsw_percent);

    let bo = oil_fvf(gor_sm3_sm3, pressure_pa, temperature_k, gas_sg, oil_sg);
    let bg = gas_fvf(pressure_pa, temperature_k, gas_sg);
    let bw = water_fvf(pressure_pa, temperature_k);

    let rho_w0 = 997.0;
    let rho_g0 = gas_sg * 1.22;
    let rho_o0 = oil_sg * 997.0;

    // DW_CalcXY densities (BlackOil.vb L691-703).
    let _rho_oil = (rho_o0 + rho_g0 * rs_scf / SCF_STB_PER_SM3_SM3) / bo;
    let rho_gas = rho_g0 / bg;
    let rho_water = rho_w0 / bw;

    let denom = rho_g0 * gor_sm3_sm3 + rho_o0 + rho_w0 * wor;
    let xg = (rho_gas * bg * (gor_ss - rs_scf) / SCF_STB_PER_SM3_SM3) / denom;
    let xw = rho_water * bw * wor / denom;
    let mut xo = 1.0 - xg - xw;
    if xo < 0.0 {
        xo = 0.0;
    }

    StreamSplit {
        gas_fraction: xg,
        oil_fraction: xo,
        water_fraction: xw,
    }
}

// ---------------------------------------------------------------------------
// Verification tests (methodology + measured results, dated 2026-08-05).
//
// STATUS: untrusted AI-assisted draft, verification only (correlation form &
// internal consistency), NOT validated against experimental PVT data. Sources:
// Standing (1947), Vazquez & Beggs (1980), Beggs & Robinson (1975),
// Dranchuk & Abou-Kassem (1975), Dempsey (1965) — public petroleum-engineering
// literature. Reference numbers computed independently in Python from the same
// published formulae and cross-checked against this port (see the crate
// hand-off notes for 2026-08-05).
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // Shared worked example (a light crude):
    //   API = 35 (SG_oil = 141.5/166.5 = 0.849850), SG_gas = 0.75,
    //   T = 200 °F = 366.483 K, producing GOR = 500 scf/STB = 88.1244 Sm³/Sm³.
    const OIL_SG: f64 = 0.849_849_849_849_85; // API 35
    const GAS_SG: f64 = 0.75;
    const T_200F_K: f64 = (200.0 - 32.0) * 5.0 / 9.0 + 273.15; // 366.4833… K
    const GOR_SI: f64 = 500.0 / SCF_STB_PER_SM3_SM3; // 88.1244 Sm³/Sm³

    fn approx(a: f64, b: f64, rel: f64) {
        assert!(
            (a - b).abs() <= rel * b.abs().max(1e-30),
            "got {a}, expected {b} (rel tol {rel})"
        );
    }

    /// METHODOLOGY: the Standing bubble-point `Pb` is the exact inverse of the
    /// Standing solution GOR `Rs`. Computing `Pb` from a producing GOR and then
    /// evaluating `Rs` at that `Pb` must recover the producing GOR. This is a
    /// closed-form round-trip verification (no external reference number needed)
    /// with pass criterion `|Rs(Pb) − GOR|/GOR < 1e-9`.
    ///
    /// RESULT (2026-08-05): for API 35, SG_gas 0.75, T 200 °F, GOR 500 scf/STB,
    /// `Pb = 2205.28 psia = 1.52049e7 Pa`, and `Rs(Pb) = 500.0000 scf/STB`
    /// (relative error < 1e-12). Round-trip verified.
    #[test]
    fn rs_pb_round_trip() {
        let pb = bubble_point_pressure(GOR_SI, T_200F_K, GAS_SG, OIL_SG);
        approx(pb, 1.520_485e7, 1e-4); // Pb ≈ 15.2049 MPa
        let rs = solution_gor(pb, T_200F_K, GAS_SG, OIL_SG);
        approx(rs, GOR_SI, 1e-9); // Rs(Pb) == producing GOR
    }

    /// METHODOLOGY: Rs (Standing) must increase monotonically with pressure
    /// below the bubble point; Bo (Standing saturated) must increase
    /// monotonically with dissolved Rs. Pass criterion: strict monotonicity
    /// across a pressure sweep 500 → 2000 psia.
    ///
    /// RESULT (2026-08-05), computed by this port and cross-checked in Python:
    ///   P=500 psia:  Rs=87.595 scf/STB, Bos=1.10325
    ///   P=1000 psia: Rs=196.028 scf/STB, Bos=1.15143
    ///   P=2000 psia: Rs=445.101 scf/STB, Bos=1.27… (rising)
    /// Both trends strictly increasing. Verified.
    #[test]
    fn rs_and_bo_monotonic() {
        let psia = |p: f64| p * PA_PER_PSIA;
        let rs_500 = solution_gor(psia(500.0), T_200F_K, GAS_SG, OIL_SG);
        let rs_1000 = solution_gor(psia(1000.0), T_200F_K, GAS_SG, OIL_SG);
        let rs_2000 = solution_gor(psia(2000.0), T_200F_K, GAS_SG, OIL_SG);
        assert!(rs_500 < rs_1000 && rs_1000 < rs_2000);

        let bo_500 = oil_fvf_saturated(rs_500, T_200F_K, GAS_SG, OIL_SG);
        let bo_1000 = oil_fvf_saturated(rs_1000, T_200F_K, GAS_SG, OIL_SG);
        let bo_2000 = oil_fvf_saturated(rs_2000, T_200F_K, GAS_SG, OIL_SG);
        assert!(bo_500 < bo_1000 && bo_1000 < bo_2000);

        // Cross-checked absolute values (rel tol 1e-4).
        approx(rs_500 * SCF_STB_PER_SM3_SM3, 87.595, 1e-4);
        approx(bo_500, 1.103_25, 1e-4);
    }

    /// METHODOLOGY: Rs is defined so that below the bubble point it is the
    /// dissolved gas; a LIMITING CASE is that Rs stays finite and small as
    /// P → low pressure, and that Bo → its low-Rs floor. Pass criterion:
    /// at P = 1 psia, Rs > 0 and small, and Bo near the `0.9759 + …` Standing
    /// floor (dominated by the `1.25·T` temperature term at negligible Rs).
    ///
    /// RESULT (2026-08-05): at P = 1 psia (T 200 °F), Rs = 2.3938 scf/STB
    /// (0.42190 Sm³/Sm³) and Bos = 1.06739. Small residual dissolved gas and a
    /// Bo just above the temperature-only floor. Limiting behaviour verified.
    #[test]
    fn low_pressure_limit() {
        let rs = solution_gor(1.0 * PA_PER_PSIA, T_200F_K, GAS_SG, OIL_SG);
        assert!(rs > 0.0 && rs * SCF_STB_PER_SM3_SM3 < 5.0);
        approx(rs, 0.421_902, 1e-4);
        let bo = oil_fvf_saturated(rs, T_200F_K, GAS_SG, OIL_SG);
        approx(bo, 1.067_391, 1e-4);
    }

    /// METHODOLOGY: the Dranchuk-Abou-Kassem Z-solver must converge to the
    /// Standing-Katz chart value; at 2000 psia / 200 °F / SG_gas 0.75 the
    /// reduced coordinates are Ppr ≈ 3.00, Tpr ≈ 1.63, where Z ≈ 0.84 on the
    /// Standing-Katz chart. Pass criterion: 0.80 < Z < 0.88 and Z < 1
    /// (real-gas compression). Gas density and viscosity must be positive and
    /// physically ordered.
    ///
    /// RESULT (2026-08-05): Z = 0.83954 (Ppr = 2.9978, Tpr = 1.6299),
    /// gas density = 121.169 kg/m³ (NOTE: DWSIM's `29.97` MW typo inflates this
    /// ~3.5 %), gas viscosity = 1.6902e-5 Pa·s. All within the expected band.
    #[test]
    fn gas_z_density_viscosity() {
        let p = 2000.0 * PA_PER_PSIA;
        let z = gas_compressibility_factor(p, T_200F_K, GAS_SG);
        approx(z, 0.839_541, 1e-4);
        assert!(z < 1.0);

        let rho = gas_density(p, T_200F_K, GAS_SG);
        approx(rho, 121.169, 1e-3);

        let mu = gas_viscosity(p, T_200F_K, GAS_SG);
        approx(mu, 1.690_219e-5, 1e-4);
    }

    /// METHODOLOGY: dead-oil viscosity (Beggs-Robinson) must exceed live-oil
    /// (gas-charged) viscosity — dissolved gas thins the oil. Pass criterion:
    /// `mu_saturated < mu_dead` at 2000 psia.
    ///
    /// RESULT (2026-08-05): mu_dead = 2.0774e-3 Pa·s (2.0774 cP),
    /// mu_saturated (Rs=445.1 scf/STB) = 6.6075e-4 Pa·s (0.6607 cP).
    /// Live oil is ~3.1× thinner. Ordering verified.
    #[test]
    fn oil_viscosity_gas_thins() {
        let mu_dead = dead_oil_viscosity(T_200F_K, OIL_SG);
        approx(mu_dead, 2.077_441e-3, 1e-4);

        let rs = solution_gor(2000.0 * PA_PER_PSIA, T_200F_K, GAS_SG, OIL_SG);
        let mu_sat = saturated_oil_viscosity(rs, mu_dead);
        approx(mu_sat, 6.607_474e-4, 1e-4);
        assert!(mu_sat < mu_dead);
    }

    /// METHODOLOGY: above the bubble point DWSIM applies the undersaturated
    /// branch (`Boss`, `muoss`). This test VERIFIES the port reproduces DWSIM's
    /// *literal* formulas — including two behaviours that run opposite to the
    /// textbook and are documented as faithful-port artifacts on [`oil_fvf`] /
    /// [`undersaturated_oil_viscosity`]:
    ///   (a) the Vazquez-Beggs compressibility `C` is negative at this low GOR
    ///       (`C = −0.024`), so `Bo` RISES slightly above `Pb`;
    ///   (b) DWSIM's `(Pb/P)^m` viscosity factor (numerator/denominator swapped
    ///       vs. the standard `(P/Pb)^m`) makes viscosity FALL above `Pb`.
    /// Pass criterion: `classify` returns `Undersaturated`, and `oil_fvf` /
    /// `oil_viscosity` equal the independently computed `Boss` / `muoss`.
    ///
    /// RESULT (2026-08-05): `Pb = 2205.28 psia`. At P = 4000 psia (>Pb):
    /// `Bo = 1.31653` (vs. bubble-point `Bos = 1.29784`, i.e. risen — artifact
    /// (a)); `mu = 5.06006e-4 Pa·s` (vs. bubble-point `muossat = 6.20463e-4
    /// Pa·s`, i.e. fallen — artifact (b)). Both match the reference to < 1e-4
    /// relative. DWSIM behaviour reproduced.
    #[test]
    fn undersaturated_regime_matches_dwsim() {
        let pb = bubble_point_pressure(GOR_SI, T_200F_K, GAS_SG, OIL_SG);
        let p_above = 4000.0 * PA_PER_PSIA;
        assert_eq!(
            SaturationRegime::classify(p_above, pb),
            SaturationRegime::Undersaturated
        );

        // (a) Bo rises above Pb here (negative Vazquez-Beggs C) — faithful port.
        let bo_pb = oil_fvf_saturated(GOR_SI, T_200F_K, GAS_SG, OIL_SG);
        let bo_above = oil_fvf(GOR_SI, p_above, T_200F_K, GAS_SG, OIL_SG);
        approx(bo_pb, 1.297_836, 1e-4);
        approx(bo_above, 1.316_532, 1e-4);
        assert!(bo_above > bo_pb);
        approx(
            oil_compressibility(GOR_SI, p_above, T_200F_K, GAS_SG, OIL_SG),
            -0.024_020,
            1e-3,
        );

        // (b) Viscosity falls above Pb (DWSIM (Pb/P)^m inversion) — faithful port.
        let mu_od = dead_oil_viscosity(T_200F_K, OIL_SG);
        let mu_pb = saturated_oil_viscosity(GOR_SI, mu_od);
        let mu_above = oil_viscosity(GOR_SI, p_above, T_200F_K, GAS_SG, OIL_SG);
        approx(mu_pb, 6.204_630e-4, 1e-4);
        approx(mu_above, 5.060_056e-4, 1e-4);
        assert!(mu_above < mu_pb);
    }

    /// METHODOLOGY: molecular-weight and boiling-point helpers are pure algebra;
    /// verify against independently computed values and sanity ranges.
    ///
    /// RESULT (2026-08-05): MW_gas = 0.75·28.97 = 21.7275 g/mol;
    /// MW_liquid(API 35, BSW 0) = 235.432 g/mol; NBP = 571.67 K. All checked.
    #[test]
    fn molecular_weights_and_nbp() {
        approx(vapor_molecular_weight(GAS_SG), 21.7275, 1e-9);
        approx(liquid_molecular_weight(OIL_SG, 0.0), 235.432_2, 1e-4);
        approx(liquid_normal_boiling_point(OIL_SG), 571.672, 1e-3);
        // Water-cut blend endpoints.
        approx(water_cut_blend(10.0, 20.0, 0.0), 10.0, 1e-12);
        approx(water_cut_blend(10.0, 20.0, 100.0), 20.0, 1e-12);
        approx(water_cut_blend(10.0, 20.0, 50.0), 15.0, 1e-12);
    }

    /// METHODOLOGY: the gas/oil/water stream split must return three
    /// non-negative fractions summing to 1 (unless oil is clamped at 0). Water
    /// FVF must be near unity. Pass criterion: `|xg+xo+xw − 1| < 1e-9`,
    /// all fractions ≥ 0, `0.9 < Bw < 1.1`.
    ///
    /// RESULT (2026-08-05): at 2000 psia, 200 °F, GOR 500 scf/STB, BSW 20 %,
    /// `Bw = 1.03467`; the split fractions are non-negative and sum to 1.
    #[test]
    fn stream_split_and_bw() {
        let bw = water_fvf(2000.0 * PA_PER_PSIA, T_200F_K);
        approx(bw, 1.034_666, 1e-4);

        let s = stream_split(GOR_SI, 2000.0 * PA_PER_PSIA, T_200F_K, GAS_SG, OIL_SG, 20.0);
        assert!(s.gas_fraction >= 0.0 && s.oil_fraction >= 0.0 && s.water_fraction >= 0.0);
        approx(
            s.gas_fraction + s.oil_fraction + s.water_fraction,
            1.0,
            1e-9,
        );
    }
}
