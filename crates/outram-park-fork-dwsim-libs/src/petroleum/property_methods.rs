//! Petroleum-fraction property correlations: critical constants, acentric
//! factor, molecular weight, kinematic viscosity, and specific-gravity
//! conversions for a *pseudo-component* (a narrow-boiling crude cut).
//!
//! # Provenance
//!
//! Faithful port of DWSIM (GPL-3.0),
//! `DWSIM.Thermodynamics/PetroleumCharacterization/PropertyMethods.vb`
//! (474 lines, whole file), from the pinned upstream clone
//! `/home/teddy0/Documents/research/dwsim-upstream`, branch `windows`, commit
//! `1abf72d1b6b41d3e9a8cc770d3cc4e8fc76e5766`. Upstream copyright: 2009 Daniel
//! Wagner O. de Medeiros and the DWSIM contributors. Licensed GPL-3.0; this
//! port is GPL-3.0-only.
//!
//! # What a "petroleum fraction" needs
//!
//! Every correlation here maps two easily-measured bulk quantities — the cut's
//! **mean boiling point** `Tb` and its **specific gravity at 15.6/15.6 °C**
//! (`d15`, i.e. `SG60`) — onto the EOS constants (`Tc`, `Pc`, `ω`, `M`) that a
//! cubic equation of state needs. Some later correlations use the
//! **Walther-ASTM viscosity parameters** `A` and `B` instead.
//!
//! # Units
//!
//! Public functions are `uom`-typed. In prose:
//!
//! | Symbol | Quantity | Unit |
//! |---|---|---|
//! | `pem_e` | mean/median boiling point of the cut | K |
//! | `d15` | specific gravity at 15.6/15.6 °C (water = 1) | dimensionless |
//! | `tc` / `pc` | critical temperature / pressure | K / Pa |
//! | `omega` | Pitzer acentric factor | dimensionless |
//! | `mw` | molecular weight | kg/mol on the API, g/mol inside the correlations |
//! | `v37`, `v98` | kinematic viscosity at 37.8 °C / 98.9 °C | m²/s |
//! | `a`, `b` | Walther-ASTM viscosity-temperature parameters | dimensionless |
//!
//! Internally the arithmetic is raw `f64` in the units each published
//! correlation was regressed in (g/mol for `M`, cSt for viscosity, K for
//! temperature), exactly as upstream — see each function's body comment.
//!
//! # Excluded DWSIM behavior
//!
//! Nothing in `PropertyMethods.vb` is excluded — all 20 public
//! `Shared Function`s and the one private helper (`CalcLogZ`, `:363-382`) are
//! ported. The VB overload sets (`Tc_Farah` ×3, `Pc_Farah` ×3, `MW_Riazi` ×2,
//! `MW_Farah` ×2) become distinct, explicitly-named Rust functions because
//! Rust has no overloading; the mapping is given in each function's docs.
//!
//! # ⚠️ Two upstream defects preserved here
//!
//! 1. [`pc_lee_kesler`] returns a pressure that is **≈10× too large** — the
//!    upstream unit conversion is wrong. See that function's docs for the
//!    arithmetic. It is ported bit-faithfully, not corrected.
//! 2. [`tc_farah_ab_sg_tb`]'s upstream *callers* pass its `d15` and `pem_e`
//!    arguments **swapped**. That is a defect of the caller, not of this
//!    function; see [`crate::petroleum::generate_compounds`].

use uom::si::f64::{KinematicViscosity, MolarMass, Pressure, Ratio, ThermodynamicTemperature};
use uom::si::kinematic_viscosity::square_meter_per_second;
use uom::si::molar_mass::gram_per_mole;
use uom::si::pressure::pascal;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

/// Specific gravity at 15.6/15.6 °C (`SG60`, water = 1) — a dimensionless
/// ratio. Named alias so a reader hovering in an editor sees the physical
/// meaning rather than a bare `Ratio`.
pub type SpecificGravity = Ratio;

/// Pitzer acentric factor `ω` — dimensionless.
pub type AcentricFactor = Ratio;

/// The Watson (UOP) characterisation factor `K_w = (1.8·Tb)^(1/3) / SG` —
/// dimensionless, ≈ 12.5 for paraffinic stocks and ≈ 10 for aromatic ones.
pub type WatsonK = Ratio;

// ===========================================================================
// Critical temperature
// ===========================================================================

/// Critical temperature by the **Riazi-Daubert (1985)** correlation.
///
/// `Tc = 9.5233 · exp(−9.314e-4·Tb − 0.544442·SG + 6.4791e-4·Tb·SG)
///       · Tb^0.81067 · SG^0.53691`  [K]
///
/// Ported from `PropertyMethods.vb:32-37` (`Tc_RiaziDaubert`). Upstream
/// computes an unused intermediate `t1` identical to the exponent and returns
/// `t2`; only `t2` is reproduced.
///
/// **Valid range** (upstream remark, `:31`): molecular weights between **70
/// and 300 g/mol**, i.e. roughly light naphtha through heavy gas oil.
///
/// - `pem_e` — mean boiling point of the fraction [K].
/// - `d15` — specific gravity at 15.6/15.6 °C [-].
#[must_use]
pub fn tc_riazi_daubert(
    pem_e: ThermodynamicTemperature,
    d15: SpecificGravity,
) -> ThermodynamicTemperature {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    let exponent = -0.0009314 * t - 0.544442 * d + 0.00064791 * t * d;
    ThermodynamicTemperature::new::<kelvin>(
        9.5233 * exponent.exp() * t.powf(0.81067) * d.powf(0.53691),
    )
}

/// Critical temperature by **Riazi's (2005)** heavy-fraction correlation.
///
/// `Tc = 35.9413 · exp(−6.9e-4·Tb − 1.4442·SG + 4.91e-4·Tb·SG)
///       · Tb^0.7293 · SG^1.2771`  [K]
///
/// Ported from `PropertyMethods.vb:60-65` (`Tc_Riazi`).
///
/// **Valid range** (upstream remark, `:59`): molecular weights **higher than
/// 300 g/mol** — the heavy-residue counterpart to [`tc_riazi_daubert`].
#[must_use]
pub fn tc_riazi_2005(
    pem_e: ThermodynamicTemperature,
    d15: SpecificGravity,
) -> ThermodynamicTemperature {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    let exponent = -0.00069 * t - 1.4442 * d + 0.000491 * t * d;
    ThermodynamicTemperature::new::<kelvin>(
        35.9413 * exponent.exp() * t.powf(0.7293) * d.powf(1.2771),
    )
}

/// Critical temperature by the **Lee-Kesler (1976)** correlation.
///
/// `Tc = 189.8 + 450.6·SG + (0.4244 + 0.1174·SG)·Tb
///       + (0.1441 − 1.0069·SG)·1e5 / Tb`  [K]
///
/// Ported from `PropertyMethods.vb:74-76` (`Tc_LeeKesler`). Upstream marks this
/// the **recommended** `Tc` method (`:73`), and it is the same expression
/// embedded in [`crate::petroleum::riazi`] (`Riazi.vb:289`).
///
/// **Valid range:** the Lee-Kesler regression basis is `Tb` ≈ 300-850 K and
/// `SG` ≈ 0.63-1.0 (light naphtha through vacuum residue).
#[must_use]
pub fn tc_lee_kesler(
    pem_e: ThermodynamicTemperature,
    d15: SpecificGravity,
) -> ThermodynamicTemperature {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    ThermodynamicTemperature::new::<kelvin>(
        189.8 + 450.6 * d + (0.4244 + 0.1174 * d) * t + (0.1441 - 1.0069 * d) * 100_000.0 / t,
    )
}

/// Critical temperature by the **API A/B method of Farah (2006)**, two-parameter
/// form: `Tc = 731.968 + 291.952·A − 704.998·B`  [K].
///
/// Ported from `PropertyMethods.vb:96-98` (`Tc_Farah(A, B)`).
///
/// - `a`, `b` — the Walther-ASTM viscosity-temperature parameters from
///   [`visc_walther_astm_a`] / [`visc_walther_astm_b`] [-].
///
/// **Valid range** (upstream remark, `:95`): `M` between 72 and 500 kg/kmol,
/// `Tc` 450-900 K, `Pc` 6.8e5-4.1e6 Pa.
#[must_use]
pub fn tc_farah_ab(a: f64, b: f64) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(731.968 + 291.952 * a - 704.998 * b)
}

/// Critical temperature by **Farah (2006)**, three-parameter form:
/// `Tc = 104.0061 + 38.75·A − 41.6097·B + 0.7831·Tb`  [K].
///
/// Ported from `PropertyMethods.vb:108-110` (`Tc_Farah(A, B, PEMe)`).
/// Same validity range as [`tc_farah_ab`].
#[must_use]
pub fn tc_farah_ab_tb(a: f64, b: f64, pem_e: ThermodynamicTemperature) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(
        104.0061 + 38.75 * a - 41.6097 * b + 0.7831 * pem_e.get::<kelvin>(),
    )
}

/// Critical temperature by **Farah (2006)**, four-parameter form:
/// `Tc = 196.793 + 90.205·A − 221.051·B + 309.534·SG + 0.524·Tb`  [K].
///
/// Ported from `PropertyMethods.vb:121-123` (`Tc_Farah(A, B, d15, PEMe)`).
/// Same validity range as [`tc_farah_ab`].
///
/// > **⚠️ Upstream callers swap the last two arguments.** Both
/// > `GenerateCompounds.vb:295` and `DistCurves.cs:704` invoke this overload as
/// > `Tc_Farah(vA, vB, NBP, SG)`, i.e. they pass the **boiling point** into the
/// > `d15` slot and the **specific gravity** into the `PEMe` slot. This
/// > function keeps the *declared* meaning of its parameters; the swap is
/// > reproduced at the call sites (see
/// > [`crate::petroleum::generate_compounds`]) so the port stays bit-faithful,
/// > and is flagged there too.
#[must_use]
pub fn tc_farah_ab_sg_tb(
    a: f64,
    b: f64,
    d15: SpecificGravity,
    pem_e: ThermodynamicTemperature,
) -> ThermodynamicTemperature {
    ThermodynamicTemperature::new::<kelvin>(
        196.793 + 90.205 * a - 221.051 * b
            + 309.534 * d15.get::<ratio>()
            + 0.524 * pem_e.get::<kelvin>(),
    )
}

// ===========================================================================
// Critical pressure
// ===========================================================================

/// Critical pressure by the **Riazi-Daubert (1985)** correlation.
///
/// `Pc = 3.1958e10 · exp(−0.008505·Tb − 4.8014·SG + 0.005749·Tb·SG)
///       · Tb^−0.4844 · SG^4.0846`  [Pa]
///
/// Ported from `PropertyMethods.vb:46-51` (`Pc_RiaziDaubert`).
///
/// **Valid range** (upstream remark, `:45`): molecular weights **70-300
/// g/mol**. Spot check: `Tb = 400 K`, `SG = 0.75` → `Pc ≈ 2.76e6 Pa`
/// (27.6 bar), physically sensible for a kerosene-range cut.
#[must_use]
pub fn pc_riazi_daubert(pem_e: ThermodynamicTemperature, d15: SpecificGravity) -> Pressure {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    let exponent = -0.008505 * t - 4.8014 * d + 0.005749 * t * d;
    Pressure::new::<pascal>(31_958_000_000.0 * exponent.exp() * t.powf(-0.4844) * d.powf(4.0846))
}

/// Critical pressure by the **Lee-Kesler (1976)** correlation, **as written
/// upstream** — see the warning below.
///
/// `Pc = 1e6 · 0.986923 · exp(5.689 − 0.0566/SG
///       − (0.43639 + 4.1216/SG + 0.21343/SG²)·1e-3·Tb
///       + (0.47579 + 1.182/SG + 0.15302/SG²)·1e-6·Tb²
///       − (2.4505 + 9.9099/SG²)·1e-10·Tb³)`
///
/// Ported from `PropertyMethods.vb:85-87` (`Pc_LeeKesler`), which documents the
/// return value as Pa.
///
/// > **⚠️ The upstream unit conversion is wrong: this returns ≈10× the correct
/// > pressure.** The `exp(...)` group is the standard Lee-Kesler expression
/// > converted to `Tb` in K with the result in **bar** (its constant 5.689 =
/// > 8.3634 − ln 14.5038, the psia→bar shift of the published form). Converting
/// > bar→Pa requires ×1e5; upstream multiplies by `1e6 × 0.986923 = 986923`.
/// > Worked example: `Tb = 400 K`, `SG = 0.75` gives `exp(...) ≈ 28.0` bar,
/// > i.e. `2.80e6 Pa`; [`pc_riazi_daubert`] independently gives
/// > **2.7635e6 Pa** for the same cut, but this function returns
/// > **2.7639e7 Pa** — a ratio of **10.0015**, measured 2026-08-11 by this
/// > module's own test `pc_lee_kesler_reproduces_upstream_ten_fold_unit_defect`.
/// >
/// > The defect is **reproduced, not fixed**, so this port matches DWSIM
/// > bit-for-bit. Downstream consequence: selecting
/// > [`crate::petroleum::CriticalPressureCorrelation::LeeKesler1976`] also
/// > corrupts the acentric factor, which divides by `Pc` (see
/// > [`acentric_factor_lee_kesler`]). Prefer
/// > [`crate::petroleum::CriticalPressureCorrelation::RiaziDaubert1985`], which
/// > is this port's default. Reported as a follow-up (see the module hand-off).
#[must_use]
pub fn pc_lee_kesler(pem_e: ThermodynamicTemperature, d15: SpecificGravity) -> Pressure {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    let exponent = 5.689 - 0.0566 / d - (0.43639 + 4.1216 / d + 0.21343 / d.powi(2)) * 0.001 * t
        + (0.47579 + 1.182 / d + 0.15302 / d.powi(2)) * 0.000_001 * t.powi(2)
        - (2.4505 + 9.9099 / d.powi(2)) * 0.000_000_000_1 * t.powi(3);
    Pressure::new::<pascal>(1_000_000.0 * 0.986923 * exponent.exp())
}

/// Critical pressure by **Farah (2006)**, two-parameter form:
/// `Pc = exp(20.0056 − 9.8758·ln A + 12.2326·ln B)`  [Pa].
///
/// Ported from `PropertyMethods.vb:132-134` (`Pc_Farah(A, B)`).
/// Requires `a > 0` and `b > 0`; returns `NaN` otherwise (`ln` of a
/// non-positive number), exactly as upstream.
#[must_use]
pub fn pc_farah_ab(a: f64, b: f64) -> Pressure {
    Pressure::new::<pascal>((20.0056 - 9.8758 * a.ln() + 12.2326 * b.ln()).exp())
}

/// Critical pressure by **Farah (2006)**, three-parameter form:
/// `Pc = exp(11.2037 − 0.5484·A + 1.9242·B + 510.1272/Tb)`  [Pa].
///
/// Ported from `PropertyMethods.vb:144-146` (`Pc_Farah(A, B, PEMe)`).
#[must_use]
pub fn pc_farah_ab_tb(a: f64, b: f64, pem_e: ThermodynamicTemperature) -> Pressure {
    Pressure::new::<pascal>(
        (11.2037 - 0.5484 * a + 1.9242 * b + 510.1272 / pem_e.get::<kelvin>()).exp(),
    )
}

/// Critical pressure by **Farah (2006)**, four-parameter form:
/// `Pc = exp(28.7605 + 0.7158·ln A − 0.2796·ln B + 2.3129·ln SG − 2.4027·ln Tb)`
/// [Pa].
///
/// Ported from `PropertyMethods.vb:157-159` (`Pc_Farah(A, B, PEMe, d15)`).
/// The parameter order matches upstream's declaration *and* its call sites
/// (`GenerateCompounds.vb:305`, `DistCurves.cs:721` pass `vA, vB, NBP, SG`), so
/// unlike [`tc_farah_ab_sg_tb`] there is no argument-order defect here.
#[must_use]
pub fn pc_farah_ab_tb_sg(
    a: f64,
    b: f64,
    pem_e: ThermodynamicTemperature,
    d15: SpecificGravity,
) -> Pressure {
    Pressure::new::<pascal>(
        (28.7605 + 0.7158 * a.ln() - 0.2796 * b.ln() + 2.3129 * d15.get::<ratio>().ln()
            - 2.4027 * pem_e.get::<kelvin>().ln())
        .exp(),
    )
}

// ===========================================================================
// Acentric factor
// ===========================================================================

/// Pitzer acentric factor `ω` by the **Lee-Kesler (1976)** vapour-pressure
/// method.
///
/// `ω = (−ln(Pc/101325) − 5.92714 + 6.09648/Tbr + 1.28862·ln Tbr − 0.169347·Tbr⁶)
///      / (15.2518 − 15.6875/Tbr − 13.4721·ln Tbr + 0.43577·Tbr⁶)`
/// with `Tbr = Tb/Tc`.
///
/// Ported from `PropertyMethods.vb:169-171` (`AcentricFactor_LeeKesler`).
/// Note that upstream normalises `Pc` by **101325 Pa (1 atm)**, so `Pc` must be
/// supplied in Pa.
///
/// **Valid range:** `Tbr < 1` (a subcritical normal boiling point) and `Pc` in
/// the physical range; it is the standard acentric-factor definition evaluated
/// at the normal boiling point.
///
/// - `tc` — critical temperature [K], `pc` — critical pressure [Pa],
///   `pemm` — *molar* mean boiling point of the fraction [K].
#[must_use]
pub fn acentric_factor_lee_kesler(
    tc: ThermodynamicTemperature,
    pc: Pressure,
    pemm: ThermodynamicTemperature,
) -> AcentricFactor {
    let tbr = pemm.get::<kelvin>() / tc.get::<kelvin>();
    let numerator =
        -(pc.get::<pascal>() / 101_325.0).ln() - 5.92714 + 6.09648 / tbr + 1.28862 * tbr.ln()
            - 0.169347 * tbr.powi(6);
    let denominator = 15.2518 - 15.6875 / tbr - 13.4721 * tbr.ln() + 0.43577 * tbr.powi(6);
    Ratio::new::<ratio>(numerator / denominator)
}

/// Pitzer acentric factor `ω` by **Korsten's (2000)** correlation.
///
/// `ω = 0.5899·(Tbr^1.3)/(1 − Tbr^1.3)·log10(Pc/101325) − 1`, `Tbr = Tb/Tc`.
///
/// Ported from `PropertyMethods.vb:181-183` (`AcentricFactor_Korsten`).
///
/// - `pemv` — *volumetric* mean boiling point of the fraction [K] (upstream's
///   parameter name; in practice the same cut mid-boiling-point).
#[must_use]
pub fn acentric_factor_korsten(
    tc: ThermodynamicTemperature,
    pc: Pressure,
    pemv: ThermodynamicTemperature,
) -> AcentricFactor {
    let tbr = pemv.get::<kelvin>() / tc.get::<kelvin>();
    Ratio::new::<ratio>(
        0.5899 * tbr.powf(1.3) / (1.0 - tbr.powf(1.3)) * (pc.get::<pascal>() / 101_325.0).log10()
            - 1.0,
    )
}

// ===========================================================================
// Molecular weight
// ===========================================================================

/// Molecular weight by **Winn's (1957)** correlation:
/// `M = 5.805e-5 · Tb^2.3776 / SG^0.9371`  [g/mol → returned as kg/mol].
///
/// Ported from `PropertyMethods.vb:192-194` (`MW_Winn`). Upstream's docstring
/// dates it 1957 while its `Select Case` labels say "Winn (1956)"; both refer
/// to the same nomograph-derived correlation.
#[must_use]
pub fn mw_winn(pem_e: ThermodynamicTemperature, d15: SpecificGravity) -> MolarMass {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    MolarMass::new::<gram_per_mole>(0.00005805 * t.powf(2.3776) / d.powf(0.9371))
}

/// Molecular weight by **Riazi's (1986)** `Tb`/`SG` correlation:
/// `M = 42.965 · exp(2.097e-4·Tb − 7.78·SG + 2.08476e-3·Tb·SG)
///      · Tb^1.26007 · SG^4.98308`  [g/mol → returned as kg/mol].
///
/// Ported from `PropertyMethods.vb:203-207` (`MW_Riazi(PEMe, d15)`).
///
/// **Valid range** (upstream remark, `:202`): light and medium fractions,
/// `Tb` ≈ 36-560 °C (309-833 K) and `SG` ≈ 0.63-0.9688.
#[must_use]
pub fn mw_riazi(pem_e: ThermodynamicTemperature, d15: SpecificGravity) -> MolarMass {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    let exponent = 0.0002097 * t - 7.78 * d + 0.00208476 * t * d;
    MolarMass::new::<gram_per_mole>(42.965 * exponent.exp() * t.powf(1.26007) * d.powf(4.98308))
}

/// Molecular weight by **Riazi's (1986)** *viscosity-based* correlation for
/// heavy fractions:
/// `M = 223.56 · v37^(1.2228·SG − 1.2435) · v98^(−3.038·SG + 3.4758) · SG^−0.6665`
///
/// Ported from `PropertyMethods.vb:217-219` (`MW_Riazi(v37, v98, d15)`).
///
/// > **Unit caveat, faithful to upstream.** Upstream documents `v37`/`v98` as
/// > kinematic viscosities "at 37.8 °C / 98.9 °C" but — unlike its sibling
/// > routines — does **not** convert them from m²/s to cSt before raising them
/// > to a power. This port therefore takes the viscosities in **m²/s** and
/// > feeds the raw SI number into the correlation, exactly as DWSIM does.
/// > (Nothing in the ported code path calls this function, so the behaviour is
/// > preserved rather than reinterpreted.)
///
/// **Valid range:** heavy fractions (upstream remark, `:216`).
#[must_use]
pub fn mw_riazi_viscosity(
    v37: KinematicViscosity,
    v98: KinematicViscosity,
    d15: SpecificGravity,
) -> MolarMass {
    let a = v37.get::<square_meter_per_second>();
    let b = v98.get::<square_meter_per_second>();
    let d = d15.get::<ratio>();
    MolarMass::new::<gram_per_mole>(
        223.56 * a.powf(1.2228 * d - 1.2435) * b.powf(-3.038 * d + 3.4758) * d.powf(-0.6665),
    )
}

/// Molecular weight by the **Lee-Kesler (1974)** correlation:
///
/// ```text
/// M = −12272.6 + 9486.4·SG + (8.3741 − 5.9917·SG)·Tb
///     + (1 − 0.77084·SG − 0.02058·SG²)(0.7465 − 222.466/Tb)·1e7/Tb
///     + (1 − 0.80882·SG − 0.02226·SG²)(0.3228 − 17.335/Tb)·1e12/Tb³
/// ```
///
/// Ported from `PropertyMethods.vb:228-234` (`MW_LeeKesler`).
///
/// **Valid range** (upstream remark, `:227`): `Tb` **below 750 K**.
#[must_use]
pub fn mw_lee_kesler(pem_e: ThermodynamicTemperature, d15: SpecificGravity) -> MolarMass {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    let t1 = -12272.6 + 9486.4 * d + (8.3741 - 5.9917 * d) * t;
    let t2 = (1.0 - 0.77084 * d - 0.02058 * d.powi(2)) * (0.7465 - 222.466 / t) * 10_000_000.0 / t;
    let t3 =
        (1.0 - 0.80882 * d - 0.02226 * d.powi(2)) * (0.3228 - 17.335 / t) * 1_000_000_000_000.0
            / t.powi(3);
    MolarMass::new::<gram_per_mole>(t1 + t2 + t3)
}

/// Molecular weight by the **API A/B method of Farah (2006)**, two-parameter
/// form: `M = exp(6.8117 + 1.3372·A − 3.6283·B)`  [g/mol → kg/mol].
///
/// Ported from `PropertyMethods.vb:243-245` (`MW_Farah(A, B)`).
/// **Valid range** (upstream remark, `:242`): `M` between 72 and 500 kg/kmol.
#[must_use]
pub fn mw_farah_ab(a: f64, b: f64) -> MolarMass {
    MolarMass::new::<gram_per_mole>((6.8117 + 1.3372 * a - 3.6283 * b).exp())
}

/// Molecular weight by **Farah (2006)**, four-parameter form:
/// `M = exp(4.0397 + 0.1362·A − 0.3406·B − 0.9988·SG + 0.0039·Tb)`
/// [g/mol → kg/mol].
///
/// Ported from `PropertyMethods.vb:256-258` (`MW_Farah(A, B, d15, PEMe)`).
/// **Valid range** (upstream remark, `:255`): `M` between 72 and 500 kg/kmol.
#[must_use]
pub fn mw_farah_ab_sg_tb(
    a: f64,
    b: f64,
    d15: SpecificGravity,
    pem_e: ThermodynamicTemperature,
) -> MolarMass {
    MolarMass::new::<gram_per_mole>(
        (4.0397 + 0.1362 * a - 0.3406 * b - 0.9988 * d15.get::<ratio>()
            + 0.0039 * pem_e.get::<kelvin>())
        .exp(),
    )
}

// ===========================================================================
// Viscosity
// ===========================================================================

/// Kinematic viscosity at **37.8 °C (100 °F)** by **Abbott's (1971)** method.
///
/// Uses the Watson factor `Kw = (1.8·Tb)^(1/3)/SG` and the API gravity
/// `API = 141.5/SG − 131.5`:
///
/// ```text
/// log10 v[cSt] = 4.39371 − 1.94733·Kw + 0.12769·Kw² + 3.2629e-4·API²
///                − 0.0118246·Kw·API
///                + (0.171617·Kw² + 10.9943·API + 0.0950663·API²
///                   − 0.860218·Kw·API) / (API + 50.3642 − 4.78231·Kw)
/// ```
///
/// Ported from `PropertyMethods.vb:267-276` (`Visc37_Abbott`). Returned in
/// **m²/s** (upstream converts cSt → m²/s by ×1e-6).
///
/// **Valid range:** petroleum fractions with `Kw` ≈ 10-13 and API ≈ 0-60;
/// the rational term's denominator can vanish for extreme `Kw`/API pairs.
#[must_use]
pub fn visc37_abbott(pem_e: ThermodynamicTemperature, d15: SpecificGravity) -> KinematicViscosity {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    let kw = (1.8 * t).powf(1.0 / 3.0) / d;
    let api = 141.5 / d - 131.5;
    let log_v = 4.39371 - 1.94733 * kw + 0.12769 * kw.powi(2) + 0.00032629 * api.powi(2)
        - 0.0118246 * kw * api
        + (0.171617 * kw.powi(2) + 10.9943 * api + 0.0950663 * api.powi(2) - 0.860218 * kw * api)
            / (api + 50.3642 - 4.78231 * kw);
    KinematicViscosity::new::<square_meter_per_second>(10.0_f64.powf(log_v) * 0.000_001)
}

/// Kinematic viscosity at **98.9 °C (210 °F)** by **Abbott's (1971)** method.
///
/// ```text
/// log10 v[cSt] = −0.463634 − 0.166532·API + 5.13447e-4·API²
///                − 0.00848995·API·Kw
///                + (0.080325·Kw + 1.24899·API + 0.19768·API²)
///                  / (API + 26.786 − 2.6296·Kw)
/// ```
///
/// Ported from `PropertyMethods.vb:285-294` (`Visc98_Abbott`). Returned in
/// m²/s. Same validity caveats as [`visc37_abbott`].
#[must_use]
pub fn visc98_abbott(pem_e: ThermodynamicTemperature, d15: SpecificGravity) -> KinematicViscosity {
    let t = pem_e.get::<kelvin>();
    let d = d15.get::<ratio>();
    let kw = (1.8 * t).powf(1.0 / 3.0) / d;
    let api = 141.5 / d - 131.5;
    let log_v = -0.463634 - 0.166532 * api + 0.000513447 * api.powi(2) - 0.00848995 * api * kw
        + (0.080325 * kw + 1.24899 * api + 0.19768 * api.powi(2)) / (api + 26.786 - 2.6296 * kw);
    KinematicViscosity::new::<square_meter_per_second>(10.0_f64.powf(log_v) * 0.000_001)
}

/// Kinematic viscosity at an arbitrary temperature by **Beg and Amin's (1989)**
/// method.
///
/// `B = exp(5.471 + 0.00342·T50)`, `A = −0.0339·API^0.188 + 0.241·T50/B`,
/// `v = A·exp(B/T)·1e-6`  [m²/s].
///
/// Ported from `PropertyMethods.vb:304-312` (`ViscT_Beg_Amin`).
///
/// - `temperature` — evaluation temperature [K].
/// - `t50_astm` — the 50 %-vaporised temperature of the **ASTM D86** curve [K].
/// - `d15` — specific gravity at 15.6/15.6 °C [-].
#[must_use]
pub fn visc_t_beg_amin(
    temperature: ThermodynamicTemperature,
    t50_astm: ThermodynamicTemperature,
    d15: SpecificGravity,
) -> KinematicViscosity {
    let t = temperature.get::<kelvin>();
    let t50 = t50_astm.get::<kelvin>();
    let api = 141.5 / d15.get::<ratio>() - 131.5;
    let b = (5.471 + 0.00342 * t50).exp();
    let a = -0.0339 * api.powf(0.188) + 0.241 * t50 / b;
    KinematicViscosity::new::<square_meter_per_second>(a * (b / t).exp() * 0.000_001)
}

/// The **A-parameter of the Walther-ASTM** viscosity-temperature equation,
/// fitted through two `(T, v)` points.
///
/// `A = log10 Z₂ + B·log10 T₂`, with `B` from [`visc_walther_astm_b`] and
/// `Z = v[cSt] + 0.7 + (correction terms)` from the ASTM D341 `Z` function.
///
/// Ported from `PropertyMethods.vb:323-337` (`ViscWaltherASTM_A`).
/// Dimensionless. Feeds Farah's `Tc`/`Pc`/`M` correlations above.
#[must_use]
pub fn visc_walther_astm_a(
    t1: ThermodynamicTemperature,
    v1: KinematicViscosity,
    t2: ThermodynamicTemperature,
    v2: KinematicViscosity,
) -> f64 {
    let tt1 = t1.get::<kelvin>().log10();
    let tt2 = t2.get::<kelvin>().log10();
    let vc1 = v1.get::<square_meter_per_second>() * 1_000_000.0;
    let vc2 = v2.get::<square_meter_per_second>() * 1_000_000.0;
    let logz1 = calc_log_z(vc1);
    let logz2 = calc_log_z(vc2);
    let b = (logz2 - logz1) / (tt1 - tt2);
    logz2 + b * tt2
}

/// The **B-parameter of the Walther-ASTM** viscosity-temperature equation,
/// fitted through two `(T, v)` points:
/// `B = (log10 Z₂ − log10 Z₁) / (log10 T₁ − log10 T₂)`.
///
/// Ported from `PropertyMethods.vb:348-361` (`ViscWaltherASTM_B`).
/// Dimensionless.
#[must_use]
pub fn visc_walther_astm_b(
    t1: ThermodynamicTemperature,
    v1: KinematicViscosity,
    t2: ThermodynamicTemperature,
    v2: KinematicViscosity,
) -> f64 {
    let tt1 = t1.get::<kelvin>().log10();
    let tt2 = t2.get::<kelvin>().log10();
    let vc1 = v1.get::<square_meter_per_second>() * 1_000_000.0;
    let vc2 = v2.get::<square_meter_per_second>() * 1_000_000.0;
    (calc_log_z(vc2) - calc_log_z(vc1)) / (tt1 - tt2)
}

/// ASTM D341 `log10(log10 Z)` where `Z = v + 0.7 + c − d + e − f + g − h`,
/// with the low-viscosity correction terms `c…h` switched on progressively as
/// `v` (in cSt) drops below 2, 1.65, 0.9, 0.3, and 0.24.
///
/// Private helper, ported from `PropertyMethods.vb:363-382` (`CalcLogZ`).
/// The nested `If` structure is preserved exactly, including VB's implicit
/// zero-initialisation of the terms that the branches do not reach.
fn calc_log_z(vc: f64) -> f64 {
    let (mut c, mut d, mut e, mut f, mut g, mut h) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    if vc < 2.0 {
        c = (-1.14883 - 2.65868 * vc).exp();
        if vc < 1.65 {
            d = (-0.0038138 - 12.5645 * vc).exp();
            if vc < 0.9 {
                e = (5.46491 - 37.6289 * vc).exp();
                if vc < 0.3 {
                    f = (13.0458 - 74.6851 * vc).exp();
                    g = (37.4619 - 192.643 * vc).exp();
                    if vc < 0.24 {
                        h = (80.4945 - 400.468 * vc).exp();
                    }
                }
            }
        }
    }
    (vc + 0.7 + c - d + e - f + g - h).log10().log10()
}

/// Kinematic viscosity at an arbitrary temperature by **Twu's method**,
/// interpolating/extrapolating from two known `(T, v)` points on the
/// ASTM-D341 double-log line.
///
/// Ported from `PropertyMethods.vb:394-418` (`ViscTwu`). Internally converts
/// K → °R (×1.8) and m²/s → cSt (×1e6) exactly as upstream, and inverts the
/// `Z` function with Twu's explicit back-correlation.
///
/// - `temperature` — evaluation temperature [K].
/// - `t1`, `t2` — temperatures of the two reference viscosities [K].
/// - `v1`, `v2` — the two reference kinematic viscosities [m²/s].
///
/// **Valid range:** both reference points must be liquid-phase and
/// `t1 != t2`; the ASTM-D341 line is a good description for `v` between ~0.3
/// and ~10⁴ cSt.
#[must_use]
pub fn visc_twu(
    temperature: ThermodynamicTemperature,
    t1: ThermodynamicTemperature,
    t2: ThermodynamicTemperature,
    v1: KinematicViscosity,
    v2: KinematicViscosity,
) -> KinematicViscosity {
    let vk1 = v1.get::<square_meter_per_second>() * 1_000_000.0;
    let vk2 = v2.get::<square_meter_per_second>() * 1_000_000.0;
    let t = 1.8 * temperature.get::<kelvin>();
    let tt1 = 1.8 * t1.get::<kelvin>();
    let tt2 = 1.8 * t2.get::<kelvin>();

    let z1 = vk1 + 0.7 + (-1.47 - 1.84 * vk1 - 0.51 * vk1.powi(2)).exp();
    let z2 = vk2 + 0.7 + (-1.47 - 1.84 * vk2 - 0.51 * vk2.powi(2)).exp();

    let b = (z1.ln().ln() - z2.ln().ln()) / (tt1.ln() - tt2.ln());
    let z = (z1.ln().ln() + b * (t.ln() - tt1.ln())).exp().exp();

    let w = z - 0.7;
    let v = w - (-0.7487 - 3.295 * w + 0.6119 * w.powi(2) - 0.3193 * w.powi(3)).exp();
    KinematicViscosity::new::<square_meter_per_second>(v * 0.000_001)
}

// ===========================================================================
// Specific-gravity conversions
// ===========================================================================

/// Specific gravity at 15.6 °C estimated from two kinematic viscosities:
/// `SG = 0.7717 · v37[cSt]^0.1157 · v98[cSt]^−0.1616`.
///
/// Ported from `PropertyMethods.vb:427-432` (`d15_v37v98`). Upstream notes the
/// author is unknown (`:426`). Inputs in m²/s (converted to cSt internally).
#[must_use]
pub fn d15_from_viscosities(v37: KinematicViscosity, v98: KinematicViscosity) -> SpecificGravity {
    let vc37 = v37.get::<square_meter_per_second>() * 1_000_000.0;
    let vc98 = v98.get::<square_meter_per_second>() * 1_000_000.0;
    Ratio::new::<ratio>(0.7717 * vc37.powf(0.1157) * vc98.powf(-0.1616))
}

/// Convert specific gravity at 15.6 °C to specific gravity at 20 °C.
///
/// Piecewise: `d20 = −0.0166·d15² + 1.0311·d15 − 0.0182` for `d15 < 0.934`,
/// else `d20 = 1.2394·d15³ − 3.7387·d15² + 4.7524·d15 − 1.2566`.
///
/// Ported from `PropertyMethods.vb:440-446` (`d20d15`). Note upstream's XML
/// doc mislabels both the parameter and the return as "d20"; the body makes the
/// direction unambiguous (`d15` in, `d20` out).
#[must_use]
pub fn d20_from_d15(d15: SpecificGravity) -> SpecificGravity {
    let d = d15.get::<ratio>();
    let out = if d < 0.934 {
        -0.0166 * d.powi(2) + 1.0311 * d - 0.0182
    } else {
        1.2394 * d.powi(3) - 3.7387 * d.powi(2) + 4.7524 * d - 1.2566
    };
    Ratio::new::<ratio>(out)
}

/// Convert specific gravity at 20 °C to specific gravity at 15.6 °C.
///
/// Piecewise: `d15 = 0.0638·d20² + 0.8769·d20 + 0.0628` for `d20 < 0.639`,
/// else `d15 = 0.0156·d20² + 0.9706·d20 + 0.0175`.
///
/// Ported from `PropertyMethods.vb:454-460` (`d15d20`).
#[must_use]
pub fn d15_from_d20(d20: SpecificGravity) -> SpecificGravity {
    let d = d20.get::<ratio>();
    let out = if d < 0.639 {
        0.0638 * d.powi(2) + 0.8769 * d + 0.0628
    } else {
        0.0156 * d.powi(2) + 0.9706 * d + 0.0175
    };
    Ratio::new::<ratio>(out)
}

/// Specific gravity at 15.6 °C from molecular weight, by **Riazi and
/// Al-Sahhaf's (1996)** single-carbon-number (SCN) correlation:
/// `SG = 1.07 − exp(3.56073 − 2.93886·M^0.1)` with `M` in g/mol.
///
/// Ported from `PropertyMethods.vb:468-470` (`d15_Riazi`).
///
/// **Valid range** (upstream remark, `:467`): SCN groups, i.e. `M` from about
/// 84 g/mol (C6) upwards. The expression saturates at `SG → 1.07` for very
/// large `M` and turns negative-argument nonsense below `M ≈ 20 g/mol`.
#[must_use]
pub fn d15_riazi(mw: MolarMass) -> SpecificGravity {
    let m = mw.get::<gram_per_mole>();
    Ratio::new::<ratio>(1.07 - (3.56073 - 2.93886 * m.powf(0.1)).exp())
}

/// Invert [`d15_riazi`]: molecular weight from specific gravity,
/// `M = ((ln(1.07 − SG) − 3.56073) / −2.93886)^10`  [g/mol → kg/mol].
///
/// This closed-form inverse is not a standalone `PropertyMethods.vb` function;
/// upstream inlines it at `GenerateCompounds.vb:133`, `:186` and
/// `Riazi.vb:244`, `:255`. Factored out here so the three call sites share one
/// documented implementation.
///
/// **Valid range:** `SG < 1.07` strictly; returns `NaN` otherwise.
#[must_use]
pub fn mw_from_d15_riazi(d15: SpecificGravity) -> MolarMass {
    let sg = d15.get::<ratio>();
    MolarMass::new::<gram_per_mole>((((1.07 - sg).ln() - 3.56073) / -2.93886).powi(10))
}

/// Normal boiling point from molecular weight, the Riazi-Al-Sahhaf SCN
/// relation `Tb = 1080 − exp(6.97996 − 0.01964·M^(2/3))`  [K], `M` in g/mol.
///
/// Like [`mw_from_d15_riazi`], upstream inlines this rather than exposing it:
/// `GenerateCompounds.vb:91`, `:99`, `:134`, `Riazi.vb:155`, `:213`, `:224`,
/// `:245`. Factored out here for one documented implementation.
///
/// **Valid range:** `M` up to about 1500 g/mol; the expression asymptotes to
/// `Tb → 1080 K` and becomes meaningless beyond that.
#[must_use]
pub fn tb_from_mw_riazi(mw: MolarMass) -> ThermodynamicTemperature {
    let m = mw.get::<gram_per_mole>();
    ThermodynamicTemperature::new::<kelvin>(1080.0 - (6.97996 - 0.01964 * m.powf(2.0 / 3.0)).exp())
}

/// Invert [`tb_from_mw_riazi`]: molecular weight from normal boiling point,
/// `M = (1/0.01964 · (6.97996 − ln(1080 − Tb)))^1.5`  [g/mol → kg/mol].
///
/// Inlined upstream at `Riazi.vb:265`, `:276` and `DistCurves.cs:568`.
///
/// **Valid range:** `Tb < 1080 K` strictly; returns `NaN` at or above that.
#[must_use]
pub fn mw_from_tb_riazi(tb: ThermodynamicTemperature) -> MolarMass {
    let t = tb.get::<kelvin>();
    MolarMass::new::<gram_per_mole>((1.0 / 0.01964 * (6.97996 - (1080.0 - t).ln())).powf(1.5))
}

/// The Watson (UOP) characterisation factor `K_w = (1.8·Tb)^(1/3) / SG` [-].
///
/// Inlined upstream at `GenerateCompounds.vb:331` and `DistCurves.cs:739`
/// (the latter using the truncated exponent `0.33333` rather than `1/3`);
/// this implementation uses the exact `1/3`, matching `GenerateCompounds.vb`.
///
/// **Interpretation:** ≈12.9 for paraffinic stocks, ≈11.8 for naphthenic,
/// ≈10 for highly aromatic.
#[must_use]
pub fn watson_k(tb: ThermodynamicTemperature, d15: SpecificGravity) -> WatsonK {
    Ratio::new::<ratio>((1.8 * tb.get::<kelvin>()).powf(1.0 / 3.0) / d15.get::<ratio>())
}

/// API gravity from specific gravity at 60 °F: `API = 141.5/SG − 131.5` [-].
///
/// Inlined upstream at `PropertyMethods.vb:271`, `:289`, `:307` and
/// `QualityCheck.vb:182`.
#[must_use]
pub fn api_gravity(d15: SpecificGravity) -> Ratio {
    Ratio::new::<ratio>(141.5 / d15.get::<ratio>() - 131.5)
}

/// Specific gravity at 60 °F from API gravity: `SG = 141.5 / (131.5 + API)` [-].
///
/// The inverse of [`api_gravity`]; inlined upstream at `DistCurves.cs:657`.
#[must_use]
pub fn specific_gravity_from_api(api: Ratio) -> SpecificGravity {
    Ratio::new::<ratio>(141.5 / (131.5 + api.get::<ratio>()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sg(v: f64) -> SpecificGravity {
        Ratio::new::<ratio>(v)
    }
    fn tk(v: f64) -> ThermodynamicTemperature {
        ThermodynamicTemperature::new::<kelvin>(v)
    }

    /// **Methodology.** For a kerosene-range cut (`Tb = 400 K`, `SG = 0.75`,
    /// `M ≈ 140 g/mol`, comfortably inside the Riazi-Daubert 70-300 g/mol
    /// validity window), the four `Tc` correlations and `Pc_RiaziDaubert` must
    /// land in the physically expected bands for such a fraction: `Tc` between
    /// 500 and 700 K, `Pc` between 1.5 and 4 MPa (Riazi, *Characterization and
    /// Properties of Petroleum Fractions*, ASTM MNL50, 2005, Table 2.1).
    ///
    /// **Results (2026-08-11, this port).**
    /// `Tc_RiaziDaubert = 583.978 K`, `Tc_LeeKesler = 579.961 K`,
    /// `Tc_Riazi2005 = 585.352 K` — all three agree to within 0.93 %.
    /// `Pc_RiaziDaubert = 2.7635e6 Pa` (27.6 bar). All inside the stated
    /// bands; test passes.
    #[test]
    fn critical_constants_are_physical_for_a_kerosene_cut() {
        let tb = tk(400.0);
        let d = sg(0.75);
        for tc in [
            tc_riazi_daubert(tb, d),
            tc_lee_kesler(tb, d),
            tc_riazi_2005(tb, d),
        ] {
            let v = tc.get::<kelvin>();
            assert!((500.0..700.0).contains(&v), "Tc = {v} K out of band");
        }
        let pc = pc_riazi_daubert(tb, d).get::<pascal>();
        assert!((1.5e6..4.0e6).contains(&pc), "Pc = {pc} Pa out of band");
    }

    /// **Methodology.** Pin the documented ≈10× defect of [`pc_lee_kesler`]
    /// (see its docs) so a future "fix" cannot land silently: assert the
    /// upstream value is between 8× and 12× the independently-correlated
    /// Riazi-Daubert value for the same cut.
    ///
    /// **Results (2026-08-11, this port).**
    /// `Pc_LeeKesler = 2.76391e7 Pa` versus
    /// `Pc_RiaziDaubert = 2.76350e6 Pa`, ratio **10.0015**. The defect is
    /// reproduced exactly. Test passes.
    #[test]
    fn pc_lee_kesler_reproduces_upstream_ten_fold_unit_defect() {
        let tb = tk(400.0);
        let d = sg(0.75);
        let ratio_lk_rd =
            pc_lee_kesler(tb, d).get::<pascal>() / pc_riazi_daubert(tb, d).get::<pascal>();
        assert!(
            (8.0..12.0).contains(&ratio_lk_rd),
            "expected the upstream ~10x defect, got ratio {ratio_lk_rd}"
        );
    }

    /// **Methodology.** Characterise a mid-range cut (`Tb = 450 K`,
    /// `SG = 0.78`) with all three molecular-weight correlations. Riazi (1986)
    /// and Winn are both `Tb`/`SG` power-law fits over the same light/medium
    /// range and are expected to agree closely (gate: within 10 %). Lee-Kesler
    /// (1974) is a different functional form with a large negative constant
    /// group and is **not** expected to agree at this boiling point; the gate
    /// for it is only that it stays positive and under-predicts.
    ///
    /// **Results (2026-08-11, this port).**
    /// `MW_Riazi = 145.195 g/mol`, `MW_Winn = 149.004 g/mol` — **2.6 %**
    /// apart. `MW_LeeKesler = 65.305 g/mol`, i.e. **2.28x lower** than the
    /// other two. That divergence is a genuine property of the Lee-Kesler
    /// correlation near the bottom of its `Tb < 750 K` window, not a porting
    /// error: its `-12272.6 + 9486.4*SG` constant group dominates at low `Tb`.
    /// The test records the divergence rather than hiding it. Test passes.
    #[test]
    fn molecular_weight_correlations_agree_within_a_factor() {
        let tb = tk(450.0);
        let d = sg(0.78);
        let riazi = mw_riazi(tb, d).get::<gram_per_mole>();
        let winn = mw_winn(tb, d).get::<gram_per_mole>();
        let lee_kesler = mw_lee_kesler(tb, d).get::<gram_per_mole>();
        for v in [riazi, winn, lee_kesler] {
            assert!(v > 0.0, "non-positive MW {v}");
        }
        assert!(
            ((riazi - winn) / winn).abs() < 0.10,
            "Riazi and Winn should agree: {riazi} vs {winn}"
        );
        // Documented divergence, pinned so a regression stays visible.
        assert!(
            lee_kesler < riazi,
            "Lee-Kesler is expected to under-predict here: {lee_kesler} vs {riazi}"
        );
    }

    /// **Methodology.** [`d15_riazi`] and [`mw_from_d15_riazi`] are exact
    /// analytic inverses; likewise [`tb_from_mw_riazi`] and
    /// [`mw_from_tb_riazi`], and [`api_gravity`] /
    /// [`specific_gravity_from_api`]. Round-trip each over the SCN range
    /// `M = 100…400 g/mol` and require < 1e-8 relative closure.
    ///
    /// **Results (2026-08-11, this port).** Max observed relative error
    /// **2.1e-15** for the SG round-trip and **1.3e-15** for the Tb round-trip
    /// over `M` in {100, 150, 220, 300, 400} g/mol; the API round-trip closes
    /// to better than 1e-12. Test passes.
    #[test]
    fn scn_correlations_round_trip() {
        for m in [100.0_f64, 150.0, 220.0, 300.0, 400.0] {
            let mw = MolarMass::new::<gram_per_mole>(m);
            let back = mw_from_d15_riazi(d15_riazi(mw)).get::<gram_per_mole>();
            assert!(
                ((back - m) / m).abs() < 1.0e-8,
                "SG round-trip {m} -> {back}"
            );
            let back_tb = mw_from_tb_riazi(tb_from_mw_riazi(mw)).get::<gram_per_mole>();
            assert!(
                ((back_tb - m) / m).abs() < 1.0e-8,
                "Tb round-trip {m} -> {back_tb}"
            );
        }
        for s in [0.7_f64, 0.8, 0.9] {
            let back = specific_gravity_from_api(api_gravity(sg(s))).get::<ratio>();
            assert!(
                ((back - s) / s).abs() < 1.0e-12,
                "API round-trip {s} -> {back}"
            );
        }
    }

    /// **Methodology.** Twu's viscosity interpolation must reproduce its own
    /// anchor points: evaluating [`visc_twu`] *at* `T1` and *at* `T2` must
    /// return `v1` and `v2`. Anchors are the Abbott estimates at 37.8 °C and
    /// 98.9 °C for a `Tb = 450 K`, `SG = 0.78` cut. Pass criterion: 2 %
    /// relative.
    ///
    /// **Results (2026-08-11, this port).** `v37 = 1.01113 cSt`,
    /// `v98 = 0.519330 cSt`; Twu returns **1.01108 cSt** and **0.519391 cSt**
    /// at the two anchors — relative errors **5.0e-5** and **1.2e-4** (the
    /// residual is the inexactness of Twu's explicit `Z`-inversion, not of the
    /// port). Test passes.
    #[test]
    fn twu_reproduces_its_anchor_points() {
        let tb = tk(450.0);
        let d = sg(0.78);
        let t1 = tk(37.8 + 273.15);
        let t2 = tk(98.9 + 273.15);
        let v1 = visc37_abbott(tb, d);
        let v2 = visc98_abbott(tb, d);
        for (t, v) in [(t1, v1), (t2, v2)] {
            let got = visc_twu(t, t1, t2, v1, v2).get::<square_meter_per_second>();
            let want = v.get::<square_meter_per_second>();
            assert!(
                ((got - want) / want).abs() < 0.02,
                "Twu at anchor: got {got}, want {want}"
            );
        }
    }

    /// **Methodology.** The Walther-ASTM `(A, B)` pair fitted through two
    /// `(T, v)` points must reproduce those points when the ASTM-D341 line
    /// `log10 log10 Z = A − B log10 T` is evaluated back at `T1` and `T2`.
    /// Pass criterion: `|log10 log10 Z| ` closure < 1e-10.
    ///
    /// **Results (2026-08-11, this port).** `A = 9.771412`, `B = 4.169484`;
    /// the fitted line reproduces `log10 log10 Z` at both anchors to better
    /// than the 1e-10 gate. Test passes.
    #[test]
    fn walther_astm_parameters_reproduce_their_anchors() {
        let tb = tk(450.0);
        let d = sg(0.78);
        let t1 = tk(311.0);
        let t2 = tk(372.0);
        let v1 = visc37_abbott(tb, d);
        let v2 = visc98_abbott(tb, d);
        let a = visc_walther_astm_a(t1, v1, t2, v2);
        let b = visc_walther_astm_b(t1, v1, t2, v2);
        for (t, v) in [(t1, v1), (t2, v2)] {
            let logz = calc_log_z(v.get::<square_meter_per_second>() * 1.0e6);
            let predicted = a - b * t.get::<kelvin>().log10();
            assert!(
                (predicted - logz).abs() < 1.0e-10,
                "Walther closure: {predicted} vs {logz}"
            );
        }
    }
}
