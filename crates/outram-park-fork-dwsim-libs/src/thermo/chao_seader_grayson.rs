//! Chao-Seader and Grayson-Streed semi-empirical hydrocarbon **K-value** property
//! packages — a pure-Rust port of DWSIM's two classic light-hydrocarbon methods.
//!
//! ## Provenance (GPLv3)
//!
//! Ported from **DWSIM** (Copyright 2009 Daniel Wagner O. de Medeiros), which is
//! licensed **GPL-3.0-or-later**; this port is therefore **GPL-3.0**. Upstream
//! gitignored clone, commit **`1abf72d`**, files (paths under
//! `DWSIM.Thermodynamics/`):
//!
//! - `PropertyPackages/Models/ChaoSeader.vb`
//!   — `class CS` (`CalcLiqActCoeff` L29-56, `CalcNu` L58-116,
//!   `CalcVapFugCoeff` L118-254).
//! - `PropertyPackages/Models/GraysonStreed.vb`
//!   — `class GS` (`CalcLiqActCoeff` L29-56, `CalcNu` L58-116,
//!   `CalcVapFugCoeff` L118-254). Identical framework to `CS`; only the `CalcNu`
//!   Pitzer coefficients for hydrogen / methane / the simple fluid differ.
//! - `PropertyPackages/ChaoSeader.vb`
//!   — the package glue: `DW_CalcFugCoeff` (L785-811) assembles the liquid
//!   fugacity coefficient as `nu0 * gamma` and the vapour as the RK `phi_v`;
//!   `RET_VVL` (L75-88), `RET_VCSAc` (L90-103), `RET_VCSS` (L105-118) fetch the
//!   per-compound liquid molar volume, "Chao-Seader acentricity", and solubility
//!   parameter.
//! - `PropertyPackages/GraysonStreed.vb` — the Grayson-Streed analogue of the
//!   same package glue.
//!
//! The GPLv3 attribution header from the upstream `.vb` files is preserved by
//! this block; do not strip it in refactors (workspace `CLAUDE.md`).
//!
//! ## The method
//!
//! Chao & Seader (1961) and Grayson & Streed (1963) compute the vapour-liquid
//! equilibrium ratio (K-value) of a light-hydrocarbon component as
//!
//! ```text
//!   K_i = gamma_i * nu0_i / phi_v_i
//! ```
//!
//! where each factor comes from a *different* model:
//!
//! - **`nu0_i`** — the pure-liquid fugacity coefficient of component `i`, from a
//!   Pitzer-type corresponding-states correlation
//!   `log10(nu0) = log10(nu0^(0)) + omega * log10(nu0^(1))`
//!   (`CalcNu`). Three coefficient sets are selected by molar mass: hydrogen
//!   (`M ≈ 2`), methane (`M ≈ 16`), and a generic "simple fluid" (everything
//!   else). **This is the only place Chao-Seader and Grayson-Streed differ** —
//!   Grayson-Streed refit the hydrogen / methane / simple-fluid coefficients to
//!   extend the method to higher temperature and to H₂-rich systems.
//! - **`gamma_i`** — the liquid-phase activity coefficient from the
//!   **regular-solution** (Scatchard-Hildebrand) model built on pure-component
//!   solubility parameters and liquid molar volumes (`CalcLiqActCoeff`).
//! - **`phi_v_i`** — the vapour-phase fugacity coefficient from the original
//!   **Redlich-Kwong** (1949) equation of state (`CalcVapFugCoeff`).
//!
//! ### Why the original Redlich-Kwong (not the crate's SRK)
//!
//! DWSIM's `CalcVapFugCoeff` uses the **original 1949 Redlich-Kwong** `a(T)`,
//! `a_i = 0.42748 R² Tc^2.5 / (Pc T^0.5)` — the `T^-0.5` temperature dependence
//! with **no acentric factor**. The crate's [`crate::thermo::cubic_eos`] `Srk`
//! variant is the *Soave* (1972) modification, whose `a(T)` carries the
//! `alpha(Tr, omega)` slope and so gives different vapour fugacities. To
//! reproduce the Chao-Seader / Grayson-Streed method faithfully this module
//! implements the original RK inline rather than reusing `cubic_eos::Srk`; the
//! shared cubic-root machinery is small and reproduced privately here to keep
//! the module standalone.
//!
//! ## Units (SI throughout the public surface)
//!
//! | Quantity | Symbol | Unit |
//! |---|---|---|
//! | temperature | `T` | K |
//! | pressure | `P` | Pa |
//! | liquid molar volume | `V_L` | m³/mol |
//! | solubility parameter | `delta` | (J/m³)^0.5  (= Pa^0.5) |
//! | acentric factor | `omega` | dimensionless |
//! | K-value, fugacity/activity coefficients | `K`, `phi`, `gamma`, `nu0` | dimensionless |
//!
//! DWSIM's `CalcLiqActCoeff` hard-codes `R = 8314470.0`; that constant is
//! `8.31447 × 10^6` and pairs with **cm³/mol** molar volumes so that
//! `V_L * delta^2 / (R T)` is dimensionless. This port keeps everything in strict
//! SI — `V_L` in **m³/mol**, `delta` in **(J/m³)^0.5**, and the true molar gas
//! constant `R = 8.314462…` J/(mol·K) — which is algebraically identical (the
//! `10^6` in DWSIM's constant exactly cancels the cm³→m³ factor).
//!
//! ## Applicability / valid ranges (light-hydrocarbon systems only)
//!
//! This is a **semi-empirical light-hydrocarbon** method, not a general EOS.
//! Documented limits (Chao & Seader 1961; Grayson & Streed 1963; Reid,
//! Prausnitz & Poling, *The Properties of Gases and Liquids*):
//!
//! - **Chao-Seader (1961):** `T ≈ 200–530 K` (`−100` to `500 °F`),
//!   `P ≤ ~14 MPa` (`2000 psia`); reduced temperature of any component
//!   `Tr < 0.93`; hydrocarbon liquid solutions (paraffins, olefins, aromatics,
//!   naphthenes) plus dissolved H₂ / CH₄ / light gases at **low** concentration.
//! - **Grayson-Streed (1963):** the same framework with refitted H₂ / CH₄ /
//!   simple-fluid coefficients, extending applicability to `T` up to `~890 K`
//!   (`~1150 °F`) and `P` up to `~20 MPa` (`~3000 psia`), and to **hydrogen-rich**
//!   systems — its main practical advantage over Chao-Seader.
//! - **Not** for: strongly polar or associating components (water, alcohols,
//!   acids), near-critical or supercritical mixtures, cryogenic distillation
//!   design, or any high-accuracy phase-equilibrium work — use a cubic EOS
//!   ([`crate::thermo::cubic_eos`]) there instead.
//!
//! ## ⚠️ Verification status
//!
//! **Untrusted AI-assisted draft pending human V&V** (workspace
//! `RESPONSIBLE_USE.md`). The tests below are **verification** ("implemented
//! correctly?"), not **validation** against Chao-Seader / Grayson-Streed
//! published charts — that benchmark validation is deferred. Independent OUTRAM
//! PARK fork, **not** the official DWSIM software.

#![forbid(unsafe_code)]

use crate::thermo::component::Component;

/// Molar gas constant `R` [J/(mol·K)] — CODATA 2018.
///
/// DWSIM's `.vb` sources hard-code the rounded `R = 8.314` in `CalcVapFugCoeff`
/// and `R = 8.31447e6` (SI × 10⁶, paired with cm³/mol) in `CalcLiqActCoeff`;
/// this port uses the exact value consistently in strict SI. The difference from
/// `8.314` is < 0.006 % and does not affect the method's semi-empirical accuracy.
const R: f64 = 8.31446261815324;

/// Selector for the two hydrocarbon K-value coefficient sets.
///
/// Enum dispatch (workspace `CLAUDE.md`: no `dyn`/`Box`) — the set of models is
/// closed and known at compile time. The variant selects **only** the
/// `CalcNu` Pitzer coefficients (`nu0`); the regular-solution activity model and
/// the Redlich-Kwong vapour model are shared byte-for-byte between the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HydrocarbonKModel {
    /// **Chao-Seader** (1961). Coefficients from
    /// `Models/ChaoSeader.vb::CalcNu` (L69-104). Valid `~200–530 K`,
    /// `P ≤ ~14 MPa`; the classic light-hydrocarbon method.
    ChaoSeader,
    /// **Grayson-Streed** (1963). Coefficients from
    /// `Models/GraysonStreed.vb::CalcNu` (L69-104). Same framework, refitted
    /// H₂ / CH₄ / simple-fluid coefficients extending to `~890 K`, `~20 MPa`,
    /// and hydrogen-rich systems.
    GraysonStreed,
}

/// Which of the three `CalcNu` coefficient sets a component uses, chosen by molar
/// mass exactly as DWSIM does (`Convert.ToInt32(VMW(i))`): `2` → hydrogen,
/// `16` → methane, anything else → the generic simple fluid.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NuSpecies {
    Hydrogen,
    Methane,
    SimpleFluid,
}

impl NuSpecies {
    /// Classify by molar mass `M` [kg/mol]. Mirrors DWSIM's integer-molar-mass
    /// test: `M·1000` rounded to the nearest integer equal to 2 → hydrogen,
    /// 16 → methane, else the simple fluid.
    fn from_molar_mass(molar_mass_kg_per_mol: f64) -> Self {
        match (molar_mass_kg_per_mol * 1000.0).round() as i64 {
            2 => Self::Hydrogen,
            16 => Self::Methane,
            _ => Self::SimpleFluid,
        }
    }
}

impl HydrocarbonKModel {
    /// The ten `nu0` correlation coefficients `[A0..A9]` for a component of molar
    /// mass `M` [kg/mol], selecting the hydrogen / methane / simple-fluid set.
    ///
    /// These are the published Chao-Seader / Grayson-Streed constants, ported
    /// verbatim from `Models/ChaoSeader.vb::CalcNu` (L69-104) and
    /// `Models/GraysonStreed.vb::CalcNu` (L69-104). Dimensionless (they act on
    /// `log10` of reduced properties).
    fn nu0_coefficients(self, molar_mass_kg_per_mol: f64) -> [f64; 10] {
        let species = NuSpecies::from_molar_mass(molar_mass_kg_per_mol);
        match (self, species) {
            // ---- Chao-Seader (Models/ChaoSeader.vb::CalcNu) ----
            (Self::ChaoSeader, NuSpecies::Hydrogen) => [
                1.96718, 1.02972, -0.054009, 0.0005288, 0.0, 0.008585, 0.0, 0.0, 0.0, 0.0,
            ],
            (Self::ChaoSeader, NuSpecies::Methane) => [
                2.4384, -2.2455, -0.34084, 0.00212, -0.00223, 0.10486, -0.03691, 0.0, 0.0, 0.0,
            ],
            (Self::ChaoSeader, NuSpecies::SimpleFluid) => [
                5.75748, -3.01761, -4.985, 2.02299, 0.0, 0.08427, 0.26667, -0.31138, -0.02655,
                0.02883,
            ],
            // ---- Grayson-Streed (Models/GraysonStreed.vb::CalcNu) ----
            (Self::GraysonStreed, NuSpecies::Hydrogen) => [
                1.50709, 2.74283, -0.0211, 0.00011, 0.0, 0.008585, 0.0, 0.0, 0.0, 0.0,
            ],
            (Self::GraysonStreed, NuSpecies::Methane) => [
                1.36822, -1.54831, 0.0, 0.02889, -0.01076, 0.10486, -0.02529, 0.0, 0.0, 0.0,
            ],
            (Self::GraysonStreed, NuSpecies::SimpleFluid) => [
                2.05135, -2.10889, 0.0, -0.19396, 0.02282, 0.08852, 0.0, -0.00872, -0.00353,
                0.00203,
            ],
        }
    }
}

/// A single component's data for the Chao-Seader / Grayson-Streed method.
///
/// Bundles the critical constants + acentric factor (via [`Component`]) with the
/// three extra regular-solution / correlation inputs DWSIM stores as compound
/// constant properties (`RET_VVL`, `RET_VCSS`, `RET_VCSAc` in
/// `PropertyPackages/ChaoSeader.vb`). Owned by value — no lifetimes, no `Box`
/// (workspace `CLAUDE.md`).
///
/// ## Fields & units
///
/// - [`component`](Self::component): critical `Tc` [K], `Pc` [Pa], molar mass
///   [kg/mol] (drives both the RK vapour term and the `nu0`-set selection).
/// - [`liquid_molar_volume`](Self::liquid_molar_volume): pure-liquid molar volume
///   `V_L` [m³/mol] — DWSIM `Chao_Seader_Liquid_Molar_Volume`.
/// - [`solubility_parameter`](Self::solubility_parameter): Hildebrand solubility
///   parameter `delta` [(J/m³)^0.5 = Pa^0.5] — DWSIM
///   `Chao_Seader_Solubility_Parameter`.
/// - [`cs_acentricity`](Self::cs_acentricity): the acentric factor `omega` used in
///   the `nu0` Pitzer correction — DWSIM `Chao_Seader_Acentricity`. Kept separate
///   from `component.acentric_factor` because DWSIM stores a method-specific value
///   (often the standard acentric factor, but tabulated independently).
#[derive(Debug, Clone, PartialEq)]
pub struct HydrocarbonSpecies {
    /// Critical constants, molar mass, and acentric factor.
    pub component: Component,
    /// Pure-liquid molar volume `V_L` [m³/mol]. Must be finite and > 0.
    pub liquid_molar_volume: f64,
    /// Hildebrand solubility parameter `delta` [(J/m³)^0.5]. Must be finite ≥ 0.
    pub solubility_parameter: f64,
    /// Acentric factor `omega` [-] used in the `nu0` Pitzer correction.
    pub cs_acentricity: f64,
}

impl HydrocarbonSpecies {
    /// Assemble a species from its [`Component`] and the three regular-solution
    /// inputs. See the struct docs for units. No validation beyond what
    /// [`Component`] already enforces on the critical constants; callers pass
    /// physically meaningful `V_L > 0` and `delta ≥ 0`.
    #[must_use]
    pub fn new(
        component: Component,
        liquid_molar_volume: f64,
        solubility_parameter: f64,
        cs_acentricity: f64,
    ) -> Self {
        Self {
            component,
            liquid_molar_volume,
            solubility_parameter,
            cs_acentricity,
        }
    }
}

/// Error from the hydrocarbon K-value routines.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum HydrocarbonKError {
    /// Composition / species slice lengths disagree.
    #[error("slice length mismatch: {what} has {got} entries, expected {expected}")]
    LengthMismatch {
        /// Which slice was the wrong length.
        what: &'static str,
        /// Length received.
        got: usize,
        /// Length required.
        expected: usize,
    },
    /// An empty species list was supplied.
    #[error("at least one component is required")]
    Empty,
    /// A non-finite or non-physical input (T, P, or a composition entry).
    #[error("non-physical input `{what}` = {value}")]
    NonPhysical {
        /// Name of the offending input.
        what: &'static str,
        /// Offending value.
        value: f64,
    },
}

/// Pure-liquid fugacity coefficient `nu0` of one component [dimensionless].
///
/// Port of `Models/ChaoSeader.vb::CalcNu` / `Models/GraysonStreed.vb::CalcNu`
/// (L58-116), the Pitzer-type corresponding-states correlation
///
/// ```text
///   log10(nu0^(0)) = A0 + A1/Tr + A2 Tr + A3 Tr^2 + A4 Tr^3
///                    + (A5 + A6 Tr + A7 Tr^2) Pr + (A8 + A9 Tr) Pr^2 - log10(Pr)
///   log10(nu0^(1)) = -4.23893 + 8.65808 Tr - 1.2206/Tr - 3.15224 Tr^3
///                    - 0.025 (Pr - 0.6)
///   nu0 = 10 ^ ( log10(nu0^(0)) + omega * log10(nu0^(1)) )
/// ```
///
/// with `Tr = T/Tc`, `Pr = P/Pc`, and `omega = species.cs_acentricity`. The
/// `A0..A9` set is chosen from the component molar mass by `model`
/// (hydrogen / methane / simple fluid) — this is the sole Chao-Seader vs
/// Grayson-Streed difference.
///
/// **Units:** `t` [K], `p` [Pa]; returns dimensionless `nu0`. **Valid range:**
/// `Tr < ~0.93` (see the module-level applicability notes); outside it the
/// correlation extrapolates without warning.
///
/// # Errors
/// [`HydrocarbonKError::NonPhysical`] if `t` or `p` is not finite and positive.
pub fn pure_liquid_fugacity_coefficient(
    model: HydrocarbonKModel,
    species: &HydrocarbonSpecies,
    t: f64,
    p: f64,
) -> Result<f64, HydrocarbonKError> {
    check_positive("t", t)?;
    check_positive("p", p)?;
    let c = &species.component;
    let a = model.nu0_coefficients(c.molar_mass);
    let tr = t / c.critical_temperature;
    let pr = p / c.critical_pressure;
    let omega = species.cs_acentricity;

    let log_v0 = a[0]
        + a[1] / tr
        + a[2] * tr
        + a[3] * tr * tr
        + a[4] * tr * tr * tr
        + (a[5] + a[6] * tr + a[7] * tr * tr) * pr
        + (a[8] + a[9] * tr) * pr * pr
        - pr.log10();
    let log_v1 =
        -4.23893 + 8.65808 * tr - 1.2206 / tr - 3.15224 * tr * tr * tr - 0.025 * (pr - 0.6);
    let log_v = log_v0 + omega * log_v1;
    Ok(10.0_f64.powf(log_v))
}

/// Liquid-phase activity coefficients `gamma_i` [dimensionless] from the
/// **regular-solution** (Scatchard-Hildebrand) model.
///
/// Port of `CalcLiqActCoeff` (L29-56 in both Models files):
///
/// ```text
///   delta_bar = ( sum_j x_j V_L,j delta_j ) / ( sum_j x_j V_L,j )   (volume-avg)
///   ln(gamma_i) = V_L,i (delta_i - delta_bar)^2 / (R T)
/// ```
///
/// `delta_bar` is the volume-fraction-weighted mean solubility parameter of the
/// mixture. `gamma_i ≥ 1` always (regular solutions show only positive deviations
/// from Raoult's law); `gamma_i = 1` for a pure component or when every `delta`
/// is equal.
///
/// **Units:** `x` mole fractions [-] (need not be normalised — only volume
/// fractions matter); `t` [K]; each `V_L` [m³/mol]; each `delta` [(J/m³)^0.5].
/// Returns one `gamma_i` per species.
///
/// # Errors
/// [`HydrocarbonKError::Empty`] if no species; [`HydrocarbonKError::LengthMismatch`]
/// if `x.len() != species.len()`; [`HydrocarbonKError::NonPhysical`] for a
/// non-finite `t`, non-positive `t`, or non-finite `x`.
pub fn liquid_activity_coefficients(
    species: &[HydrocarbonSpecies],
    x: &[f64],
    t: f64,
) -> Result<Vec<f64>, HydrocarbonKError> {
    if species.is_empty() {
        return Err(HydrocarbonKError::Empty);
    }
    if x.len() != species.len() {
        return Err(HydrocarbonKError::LengthMismatch {
            what: "x",
            got: x.len(),
            expected: species.len(),
        });
    }
    check_positive("t", t)?;
    for &xi in x {
        if !xi.is_finite() {
            return Err(HydrocarbonKError::NonPhysical {
                what: "x",
                value: xi,
            });
        }
    }

    let mut sum_v = 0.0;
    let mut sum_vs = 0.0;
    for (sp, &xi) in species.iter().zip(x) {
        sum_v += xi * sp.liquid_molar_volume;
        sum_vs += xi * sp.liquid_molar_volume * sp.solubility_parameter;
    }
    let delta_bar = sum_vs / sum_v;

    Ok(species
        .iter()
        .map(|sp| {
            let d = sp.solubility_parameter - delta_bar;
            let ln_gamma = sp.liquid_molar_volume * d * d / (R * t);
            ln_gamma.exp()
        })
        .collect())
}

/// Vapour-phase fugacity coefficients `phi_v,i` [dimensionless] from the original
/// **Redlich-Kwong** (1949) equation of state.
///
/// Port of `CalcVapFugCoeff` (L118-254 in both Models files). The RK pure-component
/// parameters and van-der-Waals mixing (with `k_ij = 0`, as DWSIM's `CalcVapFugCoeff`
/// hard-codes) are
///
/// ```text
///   a_i = 0.42748 R^2 Tc_i^2.5 / (Pc_i T^0.5)      b_i = 0.08664 R Tc_i / Pc_i
///   a_ij = sqrt(a_i a_j)   a_mix = sum_i sum_j y_i y_j a_ij   b_mix = sum_i y_i b_i
///   A = a_mix P / (R T)^2   B = b_mix P / (R T)
/// ```
///
/// `Z` is the **largest** real root of `Z^3 - Z^2 + (A - B - B^2) Z - A B = 0`
/// (the vapour root), and
///
/// ```text
///   ln(phi_i) = b_i (Z-1)/b_mix - ln(Z - B)
///               + A/B ( b_i/b_mix - 2 sqrt(a_i/a_mix) ) ln( (Z + B) / Z )
/// ```
///
/// **Units:** `y` vapour mole fractions [-] (normalised); `t` [K], `p` [Pa].
/// Returns one `phi_v,i` per species. As `P → 0`, every `phi_v,i → 1`
/// (ideal-gas limit).
///
/// # Errors
/// [`HydrocarbonKError::Empty`] / [`HydrocarbonKError::LengthMismatch`] /
/// [`HydrocarbonKError::NonPhysical`] under the same conditions as the other
/// routines.
pub fn vapor_fugacity_coefficients(
    species: &[HydrocarbonSpecies],
    y: &[f64],
    t: f64,
    p: f64,
) -> Result<Vec<f64>, HydrocarbonKError> {
    if species.is_empty() {
        return Err(HydrocarbonKError::Empty);
    }
    if y.len() != species.len() {
        return Err(HydrocarbonKError::LengthMismatch {
            what: "y",
            got: y.len(),
            expected: species.len(),
        });
    }
    check_positive("t", t)?;
    check_positive("p", p)?;
    for &yi in y {
        if !yi.is_finite() {
            return Err(HydrocarbonKError::NonPhysical {
                what: "y",
                value: yi,
            });
        }
    }

    let n = species.len();
    let rt = R * t;

    let ai: Vec<f64> = species
        .iter()
        .map(|sp| {
            let c = &sp.component;
            0.42748 * R * R * c.critical_temperature.powf(2.5) / (c.critical_pressure * t.sqrt())
        })
        .collect();
    let bi: Vec<f64> = species
        .iter()
        .map(|sp| {
            let c = &sp.component;
            0.08664 * R * c.critical_temperature / c.critical_pressure
        })
        .collect();

    // a_mix = sum_i sum_j y_i y_j sqrt(a_i a_j); b_mix = sum_i y_i b_i.
    let mut a_mix = 0.0;
    for i in 0..n {
        for j in 0..n {
            a_mix += y[i] * y[j] * (ai[i] * ai[j]).sqrt();
        }
    }
    let b_mix: f64 = (0..n).map(|i| y[i] * bi[i]).sum();

    let big_a = a_mix * p / (rt * rt);
    let big_b = b_mix * p / rt;

    // Z^3 - Z^2 + (A - B - B^2) Z - A B = 0 (RK: u = 1, w = 0).
    let c2 = -1.0;
    let c1 = big_a - big_b - big_b * big_b;
    let c0 = -big_a * big_b;
    let roots = real_cubic_roots(c2, c1, c0);
    // Vapour root: the largest real root (CalcVapFugCoeff takes temp1(2,0)).
    let zv = roots.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    Ok((0..n)
        .map(|i| {
            let t1 = bi[i] * (zv - 1.0) / b_mix;
            let t2 = -(zv - big_b).ln();
            let t3 = big_a / big_b * (bi[i] / b_mix - 2.0 * (ai[i] / a_mix).sqrt());
            let t4 = ((zv + big_b) / zv).ln();
            (t1 + t2 + t3 * t4).exp()
        })
        .collect())
}

/// Vapour-liquid equilibrium **K-values** `K_i = gamma_i * nu0_i / phi_v,i`
/// [dimensionless] for a light-hydrocarbon mixture.
///
/// Assembles the three sub-models exactly as DWSIM's package glue does
/// (`PropertyPackages/ChaoSeader.vb::DW_CalcFugCoeff` L785-811: the liquid
/// fugacity coefficient is `nu0 * gamma`, the vapour is `phi_v`, and
/// `K = phi_liquid / phi_vapour`):
///
/// - `nu0_i` from [`pure_liquid_fugacity_coefficient`] (`model`-dependent),
/// - `gamma_i` from [`liquid_activity_coefficients`] (uses `x`),
/// - `phi_v,i` from [`vapor_fugacity_coefficients`] (uses `y`).
///
/// `x` and `y` are independent here — a self-consistent flash iterates `y` to the
/// value implied by `K_i x_i` and re-solves; that outer loop belongs to the flash
/// layer, not this property model (workspace `CLAUDE.md`: Layer-5 loop logic lives
/// outside the math building blocks).
///
/// **Units:** `x`, `y` mole fractions [-]; `t` [K], `p` [Pa]. Returns one `K_i`
/// per species. **Valid range:** the light-hydrocarbon applicability window in the
/// module docs; `K_i > 1` for the more volatile components, `< 1` for the heavier.
///
/// # Errors
/// Propagates [`HydrocarbonKError`] from the three sub-models (empty list, length
/// mismatch, non-physical `T`/`P`/composition).
pub fn k_values(
    model: HydrocarbonKModel,
    species: &[HydrocarbonSpecies],
    x: &[f64],
    y: &[f64],
    t: f64,
    p: f64,
) -> Result<Vec<f64>, HydrocarbonKError> {
    if species.is_empty() {
        return Err(HydrocarbonKError::Empty);
    }
    let gamma = liquid_activity_coefficients(species, x, t)?;
    let phi_v = vapor_fugacity_coefficients(species, y, t, p)?;
    let mut k = Vec::with_capacity(species.len());
    for (i, sp) in species.iter().enumerate() {
        let nu0 = pure_liquid_fugacity_coefficient(model, sp, t, p)?;
        k.push(gamma[i] * nu0 / phi_v[i]);
    }
    Ok(k)
}

/// Reject a non-finite or non-positive scalar input.
fn check_positive(what: &'static str, value: f64) -> Result<(), HydrocarbonKError> {
    if !value.is_finite() || value <= 0.0 {
        Err(HydrocarbonKError::NonPhysical { what, value })
    } else {
        Ok(())
    }
}

/// Real roots of the monic cubic `x³ + c₂ x² + c₁ x + c₀ = 0`, ascending.
///
/// Cardano's method on the depressed cubic (mirrors the proven helper in
/// [`crate::thermo::cubic_eos`], reproduced privately so this module is
/// standalone). `Δ = (q/2)² + (p/3)³ > 0` → one real root; else three real roots
/// via the trigonometric form.
fn real_cubic_roots(c2: f64, c1: f64, c0: f64) -> Vec<f64> {
    let shift = c2 / 3.0;
    let p = c1 - c2 * c2 / 3.0;
    let q = 2.0 * c2 * c2 * c2 / 27.0 - c2 * c1 / 3.0 + c0;

    let half_q = q / 2.0;
    let third_p = p / 3.0;
    let disc = half_q * half_q + third_p * third_p * third_p;

    let mut roots = Vec::with_capacity(3);
    if disc > 0.0 {
        let sqrt_disc = disc.sqrt();
        let u = (-half_q + sqrt_disc).cbrt();
        let v = (-half_q - sqrt_disc).cbrt();
        roots.push(u + v - shift);
    } else if p.abs() < 1e-300 {
        roots.push(-shift);
    } else {
        let m = 2.0 * (-third_p).sqrt();
        let theta = (3.0 * q / (p * m)).clamp(-1.0, 1.0).acos() / 3.0;
        for k in 0..3 {
            let t = m * (theta - 2.0 * std::f64::consts::PI * f64::from(k) / 3.0).cos();
            roots.push(t - shift);
        }
    }
    roots.sort_by(|a, b| a.partial_cmp(b).unwrap());
    roots
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Public-literature regular-solution / Chao-Seader parameters ---------
    //
    // Molar volumes and solubility parameters below are the classic Chao-Seader /
    // regular-solution "hypothetical liquid" parameters tabulated in the public
    // literature: Chao, K.C. & Seader, J.D., "A general correlation of vapor-liquid
    // equilibria in hydrocarbon mixtures", AIChE J. 7(4):598-605 (1961); and Reid,
    // Prausnitz & Poling, "The Properties of Gases and Liquids". Solubility
    // parameters originally tabulated in (cal/cm³)^0.5 are converted to SI
    // (J/m³)^0.5 via 1 (cal/cm³)^0.5 = sqrt(4.184 J/cal · 1e6 cm³/m³) = 2045.5
    // (J/m³)^0.5. Molar volumes cm³/mol → m³/mol via 1e-6.
    //
    // Data-policy note (workspace DATA_POLICY.md): public-literature data only.

    /// (cal/cm³)^0.5 → (J/m³)^0.5.
    fn cal_cm3_to_si(delta_cal: f64) -> f64 {
        delta_cal * (4.184_f64 * 1.0e6).sqrt()
    }

    /// Methane as a Chao-Seader hypothetical liquid: Tc = 190.56 K, Pc = 4.599 MPa,
    /// M = 16.043 g/mol, ω = 0.011; V_L = 52 cm³/mol, δ = 5.68 (cal/cm³)^0.5.
    fn methane() -> HydrocarbonSpecies {
        let c = crate::thermo::component::reference::methane();
        HydrocarbonSpecies::new(c, 52.0e-6, cal_cm3_to_si(5.68), 0.011)
    }

    /// n-Decane: Tc = 617.7 K, Pc = 2.11 MPa, M = 142.28 g/mol, ω = 0.490;
    /// V_L = 196 cm³/mol, δ = 7.72 (cal/cm³)^0.5.
    fn n_decane() -> HydrocarbonSpecies {
        let c = Component::new(
            "n-Decane",
            0.14228,
            617.7,
            2.11e6,
            624.0e-6,
            0.490,
            447.3,
            [0.0; 5],
            f64::NAN,
        )
        .expect("n-decane constants valid");
        HydrocarbonSpecies::new(c, 196.0e-6, cal_cm3_to_si(7.72), 0.490)
    }

    /// n-Pentane: Tc = 469.7 K, Pc = 3.37 MPa, M = 72.15 g/mol, ω = 0.251;
    /// V_L = 116 cm³/mol, δ = 7.02 (cal/cm³)^0.5.
    fn n_pentane() -> HydrocarbonSpecies {
        let c = Component::new(
            "n-Pentane",
            0.07215,
            469.7,
            3.37e6,
            311.0e-6,
            0.251,
            309.2,
            [0.0; 5],
            f64::NAN,
        )
        .expect("n-pentane constants valid");
        HydrocarbonSpecies::new(c, 116.0e-6, cal_cm3_to_si(7.02), 0.251)
    }

    /// **Methodology.** The `nu0` coefficient set is selected purely by rounded
    /// molar mass (DWSIM's `Convert.ToInt32(VMW(i))`). Verify the classifier maps
    /// H₂ → hydrogen (2), CH₄ → methane (16), and n-pentane → simple fluid.
    ///
    /// **Results (2026-08-05):** `M=0.002016` → `Hydrogen`, `M=0.016043` →
    /// `Methane`, `M=0.07215` → `SimpleFluid`. Passes.
    #[test]
    fn nu_species_classifier() {
        assert_eq!(NuSpecies::from_molar_mass(0.002016), NuSpecies::Hydrogen);
        assert_eq!(NuSpecies::from_molar_mass(0.016043), NuSpecies::Methane);
        assert_eq!(NuSpecies::from_molar_mass(0.07215), NuSpecies::SimpleFluid);
    }

    /// **Methodology (verification of the `nu0` correlation).** Evaluate
    /// `pure_liquid_fugacity_coefficient` for the *simple-fluid* set at a
    /// reference reduced state and check it against a hand evaluation of the
    /// published Chao-Seader correlation with the same coefficients. Choose
    /// `Tr = 0.8`, `Pr = 0.5`, `ω = 0` (isolates `nu0^(0)`):
    ///
    /// ```text
    ///   log10 nu0^(0) = 5.75748 - 3.01761/0.8 - 4.985·0.8 + 2.02299·0.8^3
    ///                   + (0.08427 + 0.26667·0.8 - 0.31138·0.8^2)·0.5
    ///                   + (-0.02655 + 0.02883·0.8)·0.5^2 - log10(0.5)
    ///                 = -0.358288…  →  nu0 = 10^(-0.358288) = 0.438027…
    /// ```
    ///
    /// **Results (2026-08-05):** implementation returns `nu0 = 0.4380271…`,
    /// matching a second independent evaluation of the same published correlation
    /// to < 1e-9. Verifies the correlation is transcribed correctly
    /// (verification, not benchmark validation).
    #[test]
    fn nu0_simple_fluid_matches_hand_evaluation() {
        // Build a simple-fluid species with ω=0 at Tr=0.8, Pr=0.5.
        let c = Component::new(
            "sf",
            0.05,
            500.0,
            4.0e6,
            f64::NAN,
            0.0,
            f64::NAN,
            [0.0; 5],
            f64::NAN,
        )
        .unwrap();
        let sp = HydrocarbonSpecies::new(c, 100.0e-6, 15000.0, 0.0);
        let t = 0.8 * 500.0;
        let p = 0.5 * 4.0e6;
        let nu0 =
            pure_liquid_fugacity_coefficient(HydrocarbonKModel::ChaoSeader, &sp, t, p).unwrap();

        // Hand evaluation of the same correlation.
        let (tr, pr) = (0.8_f64, 0.5_f64);
        let a = [
            5.75748, -3.01761, -4.985, 2.02299, 0.0, 0.08427, 0.26667, -0.31138, -0.02655, 0.02883,
        ];
        let log_v0 = a[0]
            + a[1] / tr
            + a[2] * tr
            + a[3] * tr.powi(2)
            + a[4] * tr.powi(3)
            + (a[5] + a[6] * tr + a[7] * tr.powi(2)) * pr
            + (a[8] + a[9] * tr) * pr.powi(2)
            - pr.log10();
        let expected = 10.0_f64.powf(log_v0);
        assert!(
            (nu0 - expected).abs() < 1e-9,
            "nu0={nu0} expected={expected}"
        );
        assert!((nu0 - 0.4380271).abs() < 1e-5, "nu0={nu0}");
    }

    /// **Methodology (CS vs GS divergence).** The *only* difference between the
    /// two models is the `nu0` coefficient set for hydrogen / methane / simple
    /// fluid. Evaluate `nu0` for **methane** at the same `(T,P)` under both
    /// models and show they differ materially; then confirm the *n-pentane*
    /// simple-fluid `nu0` also differs (both sets refit).
    ///
    /// **Results (2026-08-05):** at `T=310.93 K` (100 °F), `P=6.895 MPa`
    /// (1000 psia): methane `nu0_CS = 2.201570`, `nu0_GS = 2.082659`
    /// (relative difference ≈ 5.4 %); the two are not equal, confirming the H₂/CH₄
    /// refit. n-Pentane simple-fluid `nu0` likewise differs between models. This
    /// is a measured divergence, not a benchmark match.
    #[test]
    fn chao_seader_vs_grayson_streed_methane_diverge() {
        let ch4 = methane();
        let t = 310.93; // 100 °F
        let p = 6.895e6; // 1000 psia
        let cs =
            pure_liquid_fugacity_coefficient(HydrocarbonKModel::ChaoSeader, &ch4, t, p).unwrap();
        let gs =
            pure_liquid_fugacity_coefficient(HydrocarbonKModel::GraysonStreed, &ch4, t, p).unwrap();
        assert!(cs.is_finite() && gs.is_finite());
        assert!(
            (cs - gs).abs() / cs > 0.01,
            "CS and GS methane nu0 should differ >1%: cs={cs} gs={gs}"
        );

        // Simple-fluid (n-pentane) also refit between the two models.
        let c5 = n_pentane();
        let cs5 =
            pure_liquid_fugacity_coefficient(HydrocarbonKModel::ChaoSeader, &c5, t, p).unwrap();
        let gs5 =
            pure_liquid_fugacity_coefficient(HydrocarbonKModel::GraysonStreed, &c5, t, p).unwrap();
        assert!((cs5 - gs5).abs() / cs5 > 1e-6, "cs5={cs5} gs5={gs5}");
    }

    /// **Methodology (regular-solution limiting checks).** The Scatchard-Hildebrand
    /// model must give (i) `gamma = 1` for a pure component, (ii) `gamma_i = 1`
    /// for every component when all solubility parameters are equal, and
    /// (iii) `gamma_i > 1` (positive deviation) for a mixture of dissimilar
    /// components.
    ///
    /// **Results (2026-08-05):** (i) pure methane → `gamma = 1.0` exactly;
    /// (ii) two components with δ equalised → both `gamma = 1.0` to 1e-12;
    /// (iii) methane (δ≈11.6 MPa^0.5) + n-decane (δ≈15.8 MPa^0.5) at x=[0.5,0.5],
    /// 310.93 K → both `gamma > 1`. Passes.
    #[test]
    fn regular_solution_limiting_gammas() {
        // (i) pure component.
        let g = liquid_activity_coefficients(&[methane()], &[1.0], 310.93).unwrap();
        assert!((g[0] - 1.0).abs() < 1e-12, "pure gamma={}", g[0]);

        // (ii) equal solubility parameters → ideal.
        let mut a = methane();
        let mut b = n_decane();
        a.solubility_parameter = 14000.0;
        b.solubility_parameter = 14000.0;
        let g = liquid_activity_coefficients(&[a, b], &[0.5, 0.5], 310.93).unwrap();
        assert!((g[0] - 1.0).abs() < 1e-12 && (g[1] - 1.0).abs() < 1e-12);

        // (iii) dissimilar → positive deviation.
        let g =
            liquid_activity_coefficients(&[methane(), n_decane()], &[0.5, 0.5], 310.93).unwrap();
        assert!(g[0] > 1.0 && g[1] > 1.0, "gammas={g:?}");
    }

    /// **Methodology (RK vapour ideal-gas limit).** As `P → 0` the Redlich-Kwong
    /// fugacity coefficients must approach 1. Evaluate at a very low pressure.
    ///
    /// **Results (2026-08-05):** for methane + n-decane vapour at `y=[0.9,0.1]`,
    /// `T=310.93 K`, `P=100 Pa`: both `phi_v` within 1e-3 of 1.0. Passes.
    #[test]
    fn rk_vapor_ideal_gas_limit() {
        let sp = [methane(), n_decane()];
        let phi = vapor_fugacity_coefficients(&sp, &[0.9, 0.1], 310.93, 100.0).unwrap();
        for pv in phi {
            assert!((pv - 1.0).abs() < 1e-3, "phi_v={pv}");
        }
    }

    /// **Methodology (assembled K-value, qualitative verification).** For a
    /// light-hydrocarbon binary (methane + n-decane), the volatile component
    /// (methane) must have `K > 1` and the heavy component (n-decane) `K < 1`,
    /// and `K_methane(CS) != K_methane(GS)` because the two models use different
    /// methane `nu0` coefficients. Composition `x=[0.3,0.7]` (liquid),
    /// `y=[0.95,0.05]` (vapour), `T=310.93 K` (100 °F), `P=3.447 MPa` (500 psia).
    ///
    /// **Results (2026-08-05):** Chao-Seader `K_methane = 5.65871` (`> 1`),
    /// `K_decane = 0.000814` (`< 1`); Grayson-Streed `K_methane = 5.18070`,
    /// differing from CS by ≈ 8.4 %. Relative volatility `K_CH4/K_C10 ≈ 6950`,
    /// physically sensible for a light + heavy paraffin pair. These are this
    /// implementation's own measured outputs (verification, not validation against
    /// published K-charts — that benchmark is deferred; see honest-scope note).
    #[test]
    fn assembled_kvalues_qualitative() {
        let sp = [methane(), n_decane()];
        let x = [0.3, 0.7];
        let y = [0.95, 0.05];
        let (t, p) = (310.93, 3.447e6);

        let k_cs = k_values(HydrocarbonKModel::ChaoSeader, &sp, &x, &y, t, p).unwrap();
        let k_gs = k_values(HydrocarbonKModel::GraysonStreed, &sp, &x, &y, t, p).unwrap();

        // Volatile methane K>1, heavy n-decane K<1.
        assert!(k_cs[0] > 1.0, "K_methane(CS)={}", k_cs[0]);
        assert!(k_cs[1] < 1.0, "K_decane(CS)={}", k_cs[1]);
        // Relative volatility ordering.
        assert!(k_cs[0] > k_cs[1]);
        // CS and GS differ for methane (different nu0 set).
        assert!(
            (k_cs[0] - k_gs[0]).abs() / k_cs[0] > 0.01,
            "K_methane CS={} GS={}",
            k_cs[0],
            k_gs[0]
        );
        // All finite & positive.
        for &kv in k_cs.iter().chain(k_gs.iter()) {
            assert!(kv.is_finite() && kv > 0.0, "kv={kv}");
        }
    }

    /// **Methodology (error handling).** Non-physical inputs must be rejected.
    /// **Results (2026-08-05):** empty species → `Empty`; mismatched `x` length →
    /// `LengthMismatch`; negative `T` → `NonPhysical`. Passes.
    #[test]
    fn input_validation() {
        assert_eq!(
            liquid_activity_coefficients(&[], &[], 300.0).unwrap_err(),
            HydrocarbonKError::Empty
        );
        assert!(matches!(
            liquid_activity_coefficients(&[methane()], &[0.5, 0.5], 300.0).unwrap_err(),
            HydrocarbonKError::LengthMismatch { .. }
        ));
        assert!(matches!(
            pure_liquid_fugacity_coefficient(HydrocarbonKModel::ChaoSeader, &methane(), -1.0, 1e5)
                .unwrap_err(),
            HydrocarbonKError::NonPhysical { .. }
        ));
    }
}
