//! Sour-water aqueous ionic-equilibrium speciation (H2S / NH3 / CO2 / H2O) —
//! DWSIM port. Built on the species / reaction / molality conventions of
//! [`crate::thermo::electrolyte_svle`] (its [`crate::thermo::electrolyte_svle::SvleSpecies`]
//! and [`crate::thermo::electrolyte_svle::EquilibriumReaction`] types describe the
//! chemistry — see [`SourWaterSystem::svle_species`] / [`SourWaterSystem::reaction_set`]).
//!
//! ---
//!
//! # GPLv3 provenance
//!
//! Upstream project: **DWSIM** (open-source chemical process simulator),
//! GPL-3.0, upstream commit `1abf72d`. Copyright 2016 Daniel Wagner O. de
//! Medeiros. This Rust file is a GPL-3.0 derivative work.
//!
//! Ported from:
//!
//! - `DWSIM.Thermodynamics/PropertyPackages/SourWater.vb` — the
//!   `SourWaterPropertyPackage` (Henry-law volatility correlations for NH3,
//!   CO2, H2S; the aqueous-ion speciation glue).
//! - `DWSIM.Thermodynamics/FlashAlgorithms/SourWater.vb` — the sour-water flash
//!   algorithm. The liquid-phase chemical-equilibrium kernel
//!   `CalculateEquilibriumConcentrations` (`FlashAlgorithms/SourWater.vb:384-559`)
//!   is the piece ported here: the eight aqueous acid/base/hydrolysis reactions,
//!   their mass-action laws, and DWSIM's pH-parametrized charge-balance solve
//!   (see the Honest-scope note on the solver choice).
//! - `DWSIM.Thermodynamics/Assets/swreactions.dwrxm` — the embedded reaction
//!   set: the eight `ln K(T)` correlations (`Expression` fields), evaluated as
//!   `K = exp(expr(1.8 T))` per
//!   `DWSIM.Thermodynamics/BaseClasses/ThermodynamicsBase.vb:262-304`
//!   (`EvaluateK`, `KExprType = Expression`).
//!
//! **Data provenance (equilibrium constants).** The eight `ln K(T)`
//! correlations are the DWSIM sour-water reaction set (`swreactions.dwrxm`),
//! which implements the **SWEQ** model of Wilson, Grant M. (1980), *A new
//! correlation of NH3, CO2, and H2S volatility data from aqueous sour water
//! systems*, **US EPA Report EPA-600/2-80-067** (public domain — a US
//! Government work), cited verbatim in `FlashAlgorithms/SourWater.vb:21-24`.
//! Each correlation is a polynomial in **Rankine** temperature `T_R = 1.8 T`
//! and returns `K` on a **molality (mol/kg)** basis. These are open, published
//! constants; no proprietary or restricted data is used.
//!
//! > **⚠️ Untrusted AI-assisted draft, pending human V&V.** Early-stage
//! > translation, no human review. Independent OUTRAM PARK fork, **not** the
//! > official DWSIM. The tests below are **verification** (charge/mass balance,
//! > closed-form single-acid/base pH, correct pH-vs-loading trend, and the
//! > SWEQ constants reproducing textbook `pK` values), **not validation**
//! > against an experimental sour-water VLE database. Not for nuclear facility
//! > operation, reactor control, safety-critical, licensing, or any operational
//! > decision (`RESPONSIBLE_USE.md`).
//!
//! ---
//!
//! # What this computes
//!
//! Given a **feed** of total dissolved CO2, NH3, H2S (and optionally NaOH) at
//! molalities \[mol/kg water\] in liquid water, this module solves the coupled
//! aqueous equilibria for the equilibrium **speciation** — the molality of every
//! species H⁺, OH⁻, NH3, NH4⁺, CO2, HCO3⁻, CO3²⁻, H2NCOO⁻ (carbamate), H2S,
//! HS⁻, S²⁻ (and Na⁺) — together with the solution **pH**, ionic strength, and
//! net charge. The eight reactions (`FlashAlgorithms/SourWater.vb:90-99`):
//!
//! ```text
//! (1) CO2 ionization      CO2 + H2O <-> H+ + HCO3-      K1 = [H+][HCO3-]/[CO2]
//! (2) Carbonate           HCO3-     <-> CO3-2 + H+      K2 = [CO3-2][H+]/[HCO3-]
//! (3) Ammonia ionization  H+ + NH3  <-> NH4+            K3 = [NH4+]/([H+][NH3])
//! (4) Carbamate           HCO3-+NH3 <-> H2NCOO- + H2O   K4 = [H2NCOO-]/([HCO3-][NH3])
//! (5) H2S ionization       H2S      <-> HS- + H+        K5 = [HS-][H+]/[H2S]
//! (6) Sulfide             HS-       <-> S-2 + H+         K6 = [S-2][H+]/[HS-]
//! (7) Water self-ioniz.   H2O       <-> OH- + H+         Kw = [OH-][H+]
//! (8) NaOH dissociation   NaOH      <-> OH- + Na+        (assumed complete)
//! ```
//!
//! Following DWSIM, the **water activity is absorbed into `K`** (it never enters
//! a mass-action quotient — see `SourWater.vb:457,466,486` where `[H2O]` is
//! absent), so in the reaction stoichiometry passed to the SVLE solver water has
//! a coefficient of **0**. Every reaction **conserves charge** (`Σ z·ν = 0`), so
//! a charge-neutral feed yields a charge-neutral solution by construction.
//!
//! NaOH (reaction 8) is treated as **fully dissociated** exactly as DWSIM does
//! (`SourWater.vb:484` `conc("Na+") = conc0("NaOH")`): a NaOH feed is entered as
//! equal molalities of Na⁺ and OH⁻ (charge-neutral, strong base), so reaction 8
//! is not carried as a finite-`K` equilibrium.
//!
//! # Activity-scale convention (matches DWSIM's molality basis)
//!
//! DWSIM's sour-water kernel works in **molality** (mol/kg) for *every* species,
//! neutral and ionic alike (`SourWater.vb:274-287`, `conc = Vx / kg`). To make
//! [`crate::thermo::electrolyte_svle`]'s mixed-scale solver reproduce that, each
//! dissolved **neutral** reacting species (CO2, NH3, H2S) is registered as a
//! molality-scale species of **zero charge** (role
//! [`crate::thermo::electrolyte_svle::SpeciesRole::Ion`] with `z = 0`): its
//! activity is `m·γ` \[mol/kg\] and it adds nothing to ionic strength or charge.
//! Water is the mole-fraction-scale solvent that sets the molality mass basis.
//! With ideal activities (`γ = 1`, DWSIM's own base convention) the reaction
//! quotient `Q_i = Π m_s^{ν_s}` is then in `(mol/kg)^{Σν}`, matching the units
//! of the SWEQ molality-basis `K`.
//!
//! # Units
//!
//! | Quantity | Unit |
//! |---|---|
//! | Temperature `T` | K |
//! | Feed / species molality `m` | mol/kg (water) |
//! | Ionic strength `I` | mol/kg |
//! | Equilibrium constant `K` | (mol/kg)^{Σν} |
//! | pH, charge number `z` | dimensionless |
//!
//! # Honest scope — what is and is NOT ported
//!
//! **Ported and verified here:**
//! - The eight SWEQ `ln K(T)` correlations (`swreactions.dwrxm`), on the
//!   molality basis, with `K = exp(expr(1.8 T))`.
//! - The liquid-phase reaction-set speciation (reactions 1–7 as a coupled
//!   equilibrium set, plus complete NaOH dissociation) and pH, ionic strength,
//!   and exact charge/mass balance of the result.
//!
//! **Solver choice (a measured finding, documented not hidden):** the reaction
//! set was first expressed as
//! [`crate::thermo::electrolyte_svle::EquilibriumReaction`]s (see
//! [`SourWaterSystem::reaction_set`]) and handed to the generic reaction-extent
//! solver [`crate::thermo::electrolyte_svle::SvleSystem::solve_speciation`]. On
//! the full stiff sour-water set the extents span ~9 orders of magnitude (water
//! `ξ ~ 1e-11`, acid dissociation `ξ ~ 1e-4`, second sulfide dissociation
//! `ξ ~ 1e-14`), and that solver's shared-step damped Newton **did not converge**
//! — it stalled at a log-residual of `~0.03–0.22` even at 20 000 iterations
//! (measured 2026-08-03). This is exactly why DWSIM itself does **not** use a
//! reaction-extent solver here but a **pH-parametrized charge-balance** method.
//! This port therefore implements DWSIM's own method
//! (`CalculateEquilibriumConcentrations`): every non-`H⁺` species is written in
//! closed form from the mass-action laws and the element totals, and `[H⁺]` is
//! found by robust log-scale bisection of the charge balance — see
//! [`SourWaterSystem::speciate`]. The electrolyte_svle **types and molality /
//! activity conventions** are reused to *describe* the chemistry; the stiff
//! multi-order solve is done by the DWSIM-native pH method.
//!
//! **Deliberately NOT reproduced** (documented omissions, not silent gaps):
//! - **DWSIM's empirical ionic-strength / cross-species `K` corrections**
//!   (`SourWater.vb:454` `k1 = exp(ln K1 − 0.278[H2S] + (−1.32 + 1558.8/T_R)·I^{0.4})`
//!   and `:473` `k5 = exp(ln K5 + 0.427[CO2])`). These make `K1`,`K5`
//!   composition-dependent. They are provided as standalone helpers
//!   ([`ionic_strength_correction_k1`], [`co2_correction_k5`]) and applied by
//!   the optional outer loop [`SourWaterSystem::speciate_corrected`], but the
//!   base [`SourWaterSystem::speciate`] uses the **uncorrected** SWEQ `K`
//!   (DWSIM's own commented-out fallback, `SourWater.vb:455`).
//! - **The full VLE outer loop** (`Flash_PT_Internal`, `SourWater.vb:182-382`):
//!   the alternating vapour–liquid `NestedLoops` flash and the NH3/CO2/H2S
//!   Henry-law volatility that partitions gas between phases. Only the
//!   **liquid-phase speciation** the loop iterates on is ported; the Henry-law
//!   volatility correlations are ported for reference as
//!   [`henry_volatility`] but are not wired into a phase split here.
//! - **`Flash_PH` / `Flash_PS` / `Flash_TV` / `Flash_PV`** energy/spec outer
//!   flashes (`SourWater.vb:651-783`) — out of scope for the same reason.
//!
//! No experimental sour-water database is bundled; every constant here is the
//! published SWEQ correlation or a textbook `pK` used only as a verification
//! reference.

#![forbid(unsafe_code)]

use crate::thermo::electrolyte_svle::{EquilibriumReaction, SvleSpecies};

/// Convert an absolute temperature `t` \[K\] to the **Rankine** scale
/// `T_R = 1.8 T` \[°R\] used by the SWEQ `ln K(T)` correlations
/// (`FlashAlgorithms/SourWater.vb` writes `T * 1.8` throughout).
#[must_use]
#[inline]
pub fn rankine(t: f64) -> f64 {
    1.8 * t
}

/// The eight species of the sour-water system, in the fixed index order used by
/// every stoichiometry / molality vector in this module.
///
/// Water is index 0 (the molality-scale solvent). Indices 1–11 are the reacting
/// aqueous species. `Na` (index 12) is the spectator strong-base cation.
///
/// Dimensionless enum; used only as a stable column index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
pub enum Species {
    /// Water H2O — the mole-fraction-scale solvent (index 0).
    Water = 0,
    /// Hydrogen ion H⁺ (`z = +1`, index 1).
    HPlus = 1,
    /// Hydroxide ion OH⁻ (`z = -1`, index 2).
    OhMinus = 2,
    /// Free ammonia NH3 (neutral, molality scale, index 3).
    Nh3 = 3,
    /// Ammonium ion NH4⁺ (`z = +1`, index 4).
    Nh4Plus = 4,
    /// Free carbon dioxide CO2(aq) (neutral, molality scale, index 5).
    Co2 = 5,
    /// Bicarbonate ion HCO3⁻ (`z = -1`, index 6).
    Hco3Minus = 6,
    /// Carbonate ion CO3²⁻ (`z = -2`, index 7).
    Co3Minus2 = 7,
    /// Carbamate ion H2NCOO⁻ (`z = -1`, index 8).
    CarbamateMinus = 8,
    /// Free hydrogen sulfide H2S(aq) (neutral, molality scale, index 9).
    H2s = 9,
    /// Bisulfide ion HS⁻ (`z = -1`, index 10).
    HsMinus = 10,
    /// Sulfide ion S²⁻ (`z = -2`, index 11).
    SMinus2 = 11,
    /// Sodium ion Na⁺ (spectator strong-base cation, `z = +1`, index 12).
    NaPlus = 12,
}

/// Number of species in the sour-water system (fixed at 13).
pub const N_SPECIES: usize = 13;

impl Species {
    /// The species' fixed column index \[-\] into the molality/stoichiometry
    /// vectors.
    #[must_use]
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }
}

// ---------------------------------------------------------------------------
// SWEQ equilibrium-constant correlations (swreactions.dwrxm), K = exp(expr(T_R))
// ---------------------------------------------------------------------------

/// Natural log of the **CO2-ionization** equilibrium constant
/// `K1 = [H+][HCO3-]/[CO2]` \[mol/kg\] (reaction 1), SWEQ correlation
/// `swreactions.dwrxm` reaction #1 (`Expression`), a polynomial in Rankine
/// `T_R = 1.8 T`. Valid `T` roughly 273–473 K (SWEQ sour-water range).
#[must_use]
pub fn ln_k_co2_ionization(t: f64) -> f64 {
    let tr = rankine(t);
    -241.79 + 536_256.0 / tr - 4.812_3e8 / tr.powi(2) + 1.94e11 / tr.powi(3)
        - 2.964_45e13 / tr.powi(4)
}

/// Natural log of the **carbonate-production** constant
/// `K2 = [CO3-2][H+]/[HCO3-]` \[mol/kg\] (reaction 2), SWEQ reaction #2.
#[must_use]
pub fn ln_k_carbonate(t: f64) -> f64 {
    let tr = rankine(t);
    -295.60 + 655_893.0 / tr - 5.966_7e8 / tr.powi(2) + 2.424_9e11 / tr.powi(3)
        - 3.719_2e13 / tr.powi(4)
}

/// Natural log of the **ammonia-ionization** constant
/// `K3 = [NH4+]/([H+][NH3])` \[(mol/kg)⁻¹\] (reaction 3), SWEQ reaction #3.
#[must_use]
pub fn ln_k_ammonia_ionization(t: f64) -> f64 {
    let tr = rankine(t);
    1.587 + 11_160.0 / tr
}

/// Natural log of the **carbamate-production** constant
/// `K4 = [H2NCOO-]/([HCO3-][NH3])` \[(mol/kg)⁻¹\] (reaction 4), SWEQ reaction #4.
#[must_use]
pub fn ln_k_carbamate(t: f64) -> f64 {
    let tr = rankine(t);
    -5.4 + 3465.0 / tr
}

/// Natural log of the **H2S-ionization** constant `K5 = [HS-][H+]/[H2S]`
/// \[mol/kg\] (reaction 5), SWEQ reaction #5.
#[must_use]
pub fn ln_k_h2s_ionization(t: f64) -> f64 {
    let tr = rankine(t);
    -293.88 + 683_858.0 / tr - 6.271_25e8 / tr.powi(2) + 2.555e11 / tr.powi(3)
        - 3.917_57e13 / tr.powi(4)
}

/// Natural log of the **sulfide-production** constant `K6 = [S-2][H+]/[HS-]`
/// \[mol/kg\] (reaction 6), SWEQ reaction #6.
#[must_use]
pub fn ln_k_sulfide(t: f64) -> f64 {
    let tr = rankine(t);
    -657.965 + 1_649_360.0 / tr - 15.896_4e8 / tr.powi(2) + 6.724_72e11 / tr.powi(3)
        - 10.604_3e13 / tr.powi(4)
}

/// Natural log of the **water self-ionization** constant `Kw = [OH-][H+]`
/// \[(mol/kg)²\] (reaction 7), SWEQ reaction #7.
#[must_use]
pub fn ln_kw(t: f64) -> f64 {
    let tr = rankine(t);
    39.5554 - 177_822.0 / tr + 1.843e8 / tr.powi(2) - 0.854_1e11 / tr.powi(3)
        + 1.429_2e13 / tr.powi(4)
}

/// The SWEQ **NaOH dissociation** `ln K8` (reaction 8), a temperature-independent
/// constant `15.72` (`swreactions.dwrxm` reaction #8 `Expression`). Provided for
/// completeness; NaOH is treated as fully dissociated (this large `K` confirms
/// that limit), so it is not carried as a finite-`K` equilibrium.
pub const LN_K_NAOH: f64 = 15.72;

/// All seven finite-`K` sour-water constants at temperature `t` \[K\], as `K`
/// (not `ln K`), in the order `[K1, K2, K3, K4, K5, K6, Kw]`. Each is
/// `exp(ln K(T))` per DWSIM `EvaluateK` (`ThermodynamicsBase.vb:283`).
#[must_use]
pub fn equilibrium_constants(t: f64) -> [f64; 7] {
    [
        ln_k_co2_ionization(t).exp(),
        ln_k_carbonate(t).exp(),
        ln_k_ammonia_ionization(t).exp(),
        ln_k_carbamate(t).exp(),
        ln_k_h2s_ionization(t).exp(),
        ln_k_sulfide(t).exp(),
        ln_kw(t).exp(),
    ]
}

// ---------------------------------------------------------------------------
// DWSIM's empirical K-corrections (honest-scope: not in the base solve)
// ---------------------------------------------------------------------------

/// DWSIM's empirical **ionic-strength + H2S correction** to `K1`
/// (`FlashAlgorithms/SourWater.vb:454`):
///
/// ```text
/// K1' = exp( ln K1 - 0.278 [H2S] + (-1.32 + 1558.8/T_R) I^{0.4} )
/// ```
///
/// with `[H2S]` the free-H2S molality \[mol/kg\], `I` the ionic strength
/// \[mol/kg\], and `T_R = 1.8 T`. Composition-dependent; used only by the
/// optional outer loop [`SourWaterSystem::speciate_corrected`].
#[must_use]
pub fn ionic_strength_correction_k1(k1: f64, t: f64, h2s_molality: f64, ionic_strength: f64) -> f64 {
    let tr = rankine(t);
    (k1.ln() - 0.278 * h2s_molality + (-1.32 + 1558.8 / tr) * ionic_strength.abs().powf(0.4)).exp()
}

/// DWSIM's empirical **CO2 correction** to `K5`
/// (`FlashAlgorithms/SourWater.vb:473`):
///
/// ```text
/// K5' = exp( ln K5 + 0.427 [CO2] )
/// ```
///
/// with `[CO2]` the free-CO2 molality \[mol/kg\]. Composition-dependent; used
/// only by [`SourWaterSystem::speciate_corrected`].
#[must_use]
pub fn co2_correction_k5(k5: f64, co2_molality: f64) -> f64 {
    (k5.ln() + 0.427 * co2_molality).exp()
}

// ---------------------------------------------------------------------------
// Henry-law volatility correlations (reference only; not wired to a phase split)
// ---------------------------------------------------------------------------

/// NH3 / CO2 / H2S **Henry-law volatilities** \[psia per (mol/kg)\] from DWSIM's
/// sour-water property package (`PropertyPackages/SourWater.vb:118-134`,
/// `AUX_PVAPi_SW`). Reference implementation only — **not** used by the
/// speciation solve, which is liquid-phase; documented in Honest scope.
///
/// # Arguments (all \[mol/kg\] unless noted)
/// - `t` — temperature \[K\]
/// - `cas` — free-NH3 molality (`conc("NH3")`)
/// - `cc` — total-carbon group `[CO2]+[HCO3-]+[CO3-2]+[H2NCOO-]`
/// - `cs` — total-sulfide group `[H2S]+[HS-]+[S-2]`
///
/// Returns `(v_nh3, v_co2, v_h2s)` \[psia/(mol/kg)\]. To convert to Pa/(mol/kg),
/// divide by `0.000145038` (DWSIM's `psia→Pa` factor, `SourWater.vb:119`).
#[must_use]
pub fn henry_volatility(t: f64, cas: f64, cc: f64, cs: f64) -> (f64, f64, f64) {
    let tr = rankine(t);
    let v_nh3 = (178.339 - 15_517.91 / tr - 25.6767 * tr.ln() + 0.019_66 * tr
        + (131.4 / tr - 0.1682) * cas)
        .exp()
        + 0.06 * (2.0 * cc + cs);
    let v_co2 = (18.33 - 24_895.1 / tr + 22_399_600.0 / tr.powi(2) - 9_091_800_000.0 / tr.powi(3)
        + 1_260_100_000_000.0 / tr.powi(4))
    .exp();
    let v_h2s = (100.684 - 246_254.0 / tr + 239_029_000.0 / tr.powi(2)
        - 101_898_000_000.0 / tr.powi(3)
        + 15_973_400_000_000.0 / tr.powi(4)
        - 0.05 * cas
        + (0.965 - 486.0 / tr) * cc)
        .exp();
    (v_nh3, v_co2, v_h2s)
}

// ---------------------------------------------------------------------------
// Feed + result
// ---------------------------------------------------------------------------

/// Sour-water **feed**: total dissolved amounts per kg of water \[mol/kg\].
///
/// "Total" means the sum over all speciated forms of that element group before
/// equilibrium is imposed — e.g. `co2` is total inorganic carbon fed as CO2,
/// which the solve redistributes among CO2/HCO3⁻/CO3²⁻/carbamate.
///
/// # Units / ranges
/// All fields are molalities \[mol/kg water\], `>= 0`, finite.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SourWaterFeed {
    /// Total dissolved CO2 \[mol/kg\].
    pub co2: f64,
    /// Total dissolved NH3 \[mol/kg\].
    pub nh3: f64,
    /// Total dissolved H2S \[mol/kg\].
    pub h2s: f64,
    /// Total NaOH \[mol/kg\], entered as a fully-dissociated strong base
    /// (Na⁺ + OH⁻). `0` for a NaOH-free sour water.
    pub naoh: f64,
}

impl SourWaterFeed {
    /// A feed of the three acid gases with no caustic (`naoh = 0`).
    ///
    /// # Units
    /// `co2`, `nh3`, `h2s` in \[mol/kg water\], `>= 0`.
    #[must_use]
    pub fn new(co2: f64, nh3: f64, h2s: f64) -> Self {
        Self {
            co2,
            nh3,
            h2s,
            naoh: 0.0,
        }
    }

    /// The same feed with a NaOH (caustic) molality \[mol/kg\] added as a
    /// fully-dissociated strong base.
    #[must_use]
    pub fn with_naoh(mut self, naoh: f64) -> Self {
        self.naoh = naoh;
        self
    }
}

/// Converged sour-water speciation result.
///
/// # Units
/// - `molality` — equilibrium molality \[mol/kg water\] of every [`Species`],
///   indexed by [`Species::index`].
/// - `ph` — `-log10(m_{H+})` \[-\] (molality basis; see the pH note below).
/// - `ionic_strength` — `I = ½ Σ_ion z² m` \[mol/kg\].
/// - `net_charge` — `Σ_ion z·m` \[mol/kg\] (≈ 0 for a neutral feed); equals the
///   final charge-balance residual driven to zero by the pH solve.
/// - `residual` — `|charge-balance residual|` \[mol/kg\] at the converged pH.
/// - `iterations` — pH-bisection iterations.
///
/// # pH basis
/// DWSIM multiplies `m_{H+}` by `ρ_liq/1000` to approximate a molarity (mol/L)
/// basis before `-log10` (`PropertyPackages/ElectrolyteBase`/`ElectrolyteProperties`).
/// This port uses the **molality** basis directly (`ρ/1000 ≈ 1` for dilute
/// aqueous; the liquid-density model is not ported). Documented simplification.
#[derive(Debug, Clone, PartialEq)]
pub struct SourWaterResult {
    /// Equilibrium molality \[mol/kg\] per species (index = [`Species::index`]).
    pub molality: [f64; N_SPECIES],
    /// Solution pH \[-\] (`-log10(m_{H+})`, molality basis).
    pub ph: f64,
    /// Ionic strength `I` \[mol/kg\].
    pub ionic_strength: f64,
    /// Net charge molality `Σ z·m` \[mol/kg\].
    pub net_charge: f64,
    /// `|charge-balance residual|` \[mol/kg\] at the converged pH.
    pub residual: f64,
    /// pH-bisection iterations performed.
    pub iterations: usize,
}

impl SourWaterResult {
    /// Molality \[mol/kg\] of a single [`Species`].
    #[must_use]
    #[inline]
    pub fn m(&self, s: Species) -> f64 {
        self.molality[s.index()]
    }
}

/// Molar masses \[kg/mol\] of the sour-water species (public IUPAC values).
/// Only the **solvent** (water) mass enters the molality basis; the others are
/// carried for documentation and completeness.
const MOLAR_MASS: [f64; N_SPECIES] = [
    0.018_015, // H2O
    0.001_008, // H+
    0.017_007, // OH-
    0.017_031, // NH3
    0.018_039, // NH4+
    0.044_010, // CO2
    0.061_017, // HCO3-
    0.060_009, // CO3-2
    0.060_032, // H2NCOO-
    0.034_081, // H2S
    0.033_073, // HS-
    0.032_065, // S-2
    0.022_990, // Na+
];

/// A sour-water aqueous system at a fixed temperature: the species set, the
/// SWEQ equilibrium constants at that `T`, and the reaction stoichiometry — the
/// port of DWSIM's sour-water liquid-phase equilibrium
/// (`FlashAlgorithms/SourWater.vb:384-559`, `CalculateEquilibriumConcentrations`).
///
/// Construct with [`SourWaterSystem::at_temperature`]; solve a feed with
/// [`SourWaterSystem::speciate`].
#[derive(Debug, Clone, PartialEq)]
pub struct SourWaterSystem {
    /// System temperature \[K\].
    temperature: f64,
    /// The seven finite-`K` constants `[K1..K6, Kw]` at `temperature`.
    k: [f64; 7],
}

impl SourWaterSystem {
    /// Build the sour-water system at temperature `t` \[K\], evaluating all
    /// SWEQ `K(T)` correlations.
    ///
    /// # Units / ranges
    /// - `t` — temperature \[K\], within the SWEQ correlation range (roughly
    ///   273–473 K); finite and `> 0`.
    #[must_use]
    pub fn at_temperature(t: f64) -> Self {
        Self {
            temperature: t,
            k: equilibrium_constants(t),
        }
    }

    /// System temperature \[K\].
    #[must_use]
    pub fn temperature(&self) -> f64 {
        self.temperature
    }

    /// The seven finite-`K` constants `[K1, K2, K3, K4, K5, K6, Kw]` at the
    /// system temperature \[(mol/kg)^{Σν}\].
    #[must_use]
    pub fn constants(&self) -> [f64; 7] {
        self.k
    }

    /// The ordered [`SvleSpecies`] list describing the sour-water phase — a
    /// direct reuse of the [`crate::thermo::electrolyte_svle`] species types.
    /// Water is the mole-fraction-scale solvent; the neutral gases (CO2/NH3/H2S)
    /// are molality-scale zero-charge "ions"; the rest are charged ions. This is
    /// the canonical species description; the solve itself is the pH-parametrized
    /// method of [`Self::speciate`] (see the module Honest-scope note).
    #[must_use]
    pub fn svle_species() -> Vec<SvleSpecies> {
        let m = &MOLAR_MASS;
        vec![
            SvleSpecies::solvent("Water", m[0]),
            SvleSpecies::ion("H+", 1, m[1]),
            SvleSpecies::ion("OH-", -1, m[2]),
            SvleSpecies::ion("NH3", 0, m[3]), // neutral, molality scale
            SvleSpecies::ion("NH4+", 1, m[4]),
            SvleSpecies::ion("CO2", 0, m[5]), // neutral, molality scale
            SvleSpecies::ion("HCO3-", -1, m[6]),
            SvleSpecies::ion("CO3-2", -2, m[7]),
            SvleSpecies::ion("H2NCOO-", -1, m[8]),
            SvleSpecies::ion("H2S", 0, m[9]), // neutral, molality scale
            SvleSpecies::ion("HS-", -1, m[10]),
            SvleSpecies::ion("S-2", -2, m[11]),
            SvleSpecies::ion("Na+", 1, m[12]),
        ]
    }

    /// The seven finite-`K` sour-water reactions (1–7) as
    /// [`crate::thermo::electrolyte_svle::EquilibriumReaction`]s over the
    /// [`Self::svle_species`] column order, with the SWEQ constants at the system
    /// temperature. A direct reuse of the electrolyte_svle reaction type; water
    /// has coefficient 0 throughout (its activity is absorbed into `K`, matching
    /// DWSIM). Provided for introspection and to make the chemistry auditable
    /// (each reaction is verified charge-conserving in the tests).
    #[must_use]
    pub fn reaction_set(&self) -> Vec<EquilibriumReaction> {
        let (h, oh, nh3, nh4, co2, hco3, co3, carb, h2s, hs, s2) = (
            Species::HPlus.index(),
            Species::OhMinus.index(),
            Species::Nh3.index(),
            Species::Nh4Plus.index(),
            Species::Co2.index(),
            Species::Hco3Minus.index(),
            Species::Co3Minus2.index(),
            Species::CarbamateMinus.index(),
            Species::H2s.index(),
            Species::HsMinus.index(),
            Species::SMinus2.index(),
        );
        let k = &self.k;
        let mk = |pairs: &[(usize, f64)], kval: f64| {
            let mut s = vec![0.0_f64; N_SPECIES];
            for &(i, v) in pairs {
                s[i] = v;
            }
            EquilibriumReaction::new(s, kval)
        };
        vec![
            // R1 CO2 -> H+ + HCO3-
            mk(&[(co2, -1.0), (h, 1.0), (hco3, 1.0)], k[0]),
            // R2 HCO3- -> CO3-2 + H+
            mk(&[(hco3, -1.0), (co3, 1.0), (h, 1.0)], k[1]),
            // R3 H+ + NH3 -> NH4+
            mk(&[(h, -1.0), (nh3, -1.0), (nh4, 1.0)], k[2]),
            // R4 HCO3- + NH3 -> H2NCOO- (water absorbed)
            mk(&[(hco3, -1.0), (nh3, -1.0), (carb, 1.0)], k[3]),
            // R5 H2S -> HS- + H+
            mk(&[(h2s, -1.0), (hs, 1.0), (h, 1.0)], k[4]),
            // R6 HS- -> S-2 + H+
            mk(&[(hs, -1.0), (s2, 1.0), (h, 1.0)], k[5]),
            // R7 H2O -> OH- + H+ (water coeff 0)
            mk(&[(oh, 1.0), (h, 1.0)], k[6]),
        ]
    }

    /// Explicit speciation at a **fixed** hydrogen-ion molality `h` \[mol/kg\]
    /// and constant set `k`, from the closed-form mass-action laws + element
    /// totals — the algebraic heart of DWSIM's
    /// `CalculateEquilibriumConcentrations` (`FlashAlgorithms/SourWater.vb:426-491`).
    ///
    /// Returns the full molality array \[mol/kg\] and the **charge-balance
    /// residual** `f = (H⁺ + NH4⁺ + Na⁺) − (OH⁻ + HCO3⁻ + H2NCOO⁻ + HS⁻ +
    /// 2 S²⁻ + 2 CO3²⁻)` \[mol/kg\], which equals the net charge molality `Σ z·m`.
    fn speciation_at_h(
        k: &[f64; 7],
        ct: f64,
        nt: f64,
        st: f64,
        na: f64,
        h: f64,
    ) -> ([f64; N_SPECIES], f64) {
        let (k1, k2, k3, k4, k5, k6, kw) = (k[0], k[1], k[2], k[3], k[4], k[5], k[6]);

        // Sulfide sub-system (H2S/HS-/S-2), closed form given h.
        let den_s = 1.0 + k5 / h + k5 * k6 / (h * h);
        let h2s = if st > 0.0 { st / den_s } else { 0.0 };
        let hs = k5 * h2s / h;
        let s2 = k5 * k6 * h2s / (h * h);

        // Coupled carbon (CO2/HCO3-/CO3-2/carbamate) & nitrogen (NH3/NH4+) block,
        // both depending on HCO3- and NH3; solve by fixed-point given h.
        let mut nh3 = nt;
        let mut hco3 = 0.0_f64;
        if ct > 0.0 || nt > 0.0 {
            for _ in 0..1000 {
                let new_nh3 = if nt > 0.0 {
                    nt / (1.0 + k3 * h + k4 * hco3)
                } else {
                    0.0
                };
                let new_hco3 = if ct > 0.0 {
                    ct / (h / k1 + 1.0 + k2 / h + k4 * new_nh3)
                } else {
                    0.0
                };
                let conv = (new_hco3 - hco3).abs() <= 1e-20 + 1e-15 * new_hco3.abs()
                    && (new_nh3 - nh3).abs() <= 1e-20 + 1e-15 * new_nh3.abs();
                nh3 = new_nh3;
                hco3 = new_hco3;
                if conv {
                    break;
                }
            }
        }
        let co2 = if ct > 0.0 { h * hco3 / k1 } else { 0.0 };
        let co3 = k2 * hco3 / h;
        let carb = k4 * hco3 * nh3;
        let nh4 = k3 * h * nh3;
        let oh = kw / h;

        let mut m = [0.0_f64; N_SPECIES];
        m[Species::Water.index()] = 1.0 / MOLAR_MASS[Species::Water.index()];
        m[Species::HPlus.index()] = h;
        m[Species::OhMinus.index()] = oh;
        m[Species::Nh3.index()] = nh3;
        m[Species::Nh4Plus.index()] = nh4;
        m[Species::Co2.index()] = co2;
        m[Species::Hco3Minus.index()] = hco3;
        m[Species::Co3Minus2.index()] = co3;
        m[Species::CarbamateMinus.index()] = carb;
        m[Species::H2s.index()] = h2s;
        m[Species::HsMinus.index()] = hs;
        m[Species::SMinus2.index()] = s2;
        m[Species::NaPlus.index()] = na;

        let pch = h + nh4 + na;
        let nch = oh + hco3 + carb + hs + 2.0 * s2 + 2.0 * co3;
        (m, pch - nch)
    }

    /// Core solve for a feed at a given constant set `k`: bracket the hydrogen-ion
    /// molality `h` and drive the charge-balance residual to zero by **log-scale
    /// bisection** (robust — `f(h)` is monotone increasing in `h`). Returns the
    /// assembled [`SourWaterResult`].
    fn solve_at(&self, feed: &SourWaterFeed, k: &[f64; 7]) -> Result<SourWaterResult, SourWaterError> {
        if [feed.co2, feed.nh3, feed.h2s, feed.naoh]
            .iter()
            .any(|v| !v.is_finite() || *v < 0.0)
        {
            return Err(SourWaterError::InvalidFeed);
        }
        let (ct, nt, st, na) = (feed.co2, feed.nh3, feed.h2s, feed.naoh);

        // Bracket: f<0 at high pH (small h), f>0 at low pH (large h).
        let mut lo = 1.0e-15_f64; // pH ≈ 15
        let mut hi = 10.0_f64; //    pH ≈ -1
        let (_, mut f_lo) = Self::speciation_at_h(k, ct, nt, st, na, lo);
        let (_, f_hi) = Self::speciation_at_h(k, ct, nt, st, na, hi);
        if !f_lo.is_finite() || !f_hi.is_finite() {
            return Err(SourWaterError::NonFinite);
        }
        if f_lo * f_hi > 0.0 {
            return Err(SourWaterError::NoBracket);
        }

        let mut mid = (lo * hi).sqrt();
        let mut iterations = 0usize;
        for it in 1..=300 {
            iterations = it;
            mid = (lo * hi).sqrt(); // geometric (log-pH) midpoint
            let (_, fm) = Self::speciation_at_h(k, ct, nt, st, na, mid);
            if !fm.is_finite() {
                return Err(SourWaterError::NonFinite);
            }
            if fm.abs() < 1.0e-13 || (hi / lo - 1.0) < 4.0 * f64::EPSILON {
                break;
            }
            if fm * f_lo > 0.0 {
                lo = mid;
                f_lo = fm;
            } else {
                hi = mid;
            }
        }

        let (m, f) = Self::speciation_at_h(k, ct, nt, st, na, mid);
        if m.iter().any(|v| !v.is_finite()) {
            return Err(SourWaterError::NonFinite);
        }
        // Ionic strength and net charge over the charged ions.
        let charges: [f64; N_SPECIES] = [
            0.0, 1.0, -1.0, 0.0, 1.0, 0.0, -1.0, -2.0, -1.0, 0.0, -1.0, -2.0, 1.0,
        ];
        let mut ionic_strength = 0.0;
        for i in 0..N_SPECIES {
            let z = charges[i];
            ionic_strength += 0.5 * z * z * m[i];
        }
        let m_h = m[Species::HPlus.index()];
        let ph = -m_h.log10();
        Ok(SourWaterResult {
            molality: m,
            ph,
            ionic_strength,
            net_charge: f,
            residual: f.abs(),
            iterations,
        })
    }

    /// Solve the sour-water liquid-phase speciation for a feed, using the
    /// **uncorrected** SWEQ `K` (DWSIM's own commented-out fallback,
    /// `SourWater.vb:455`).
    ///
    /// # Method
    /// Port of DWSIM `CalculateEquilibriumConcentrations`
    /// (`FlashAlgorithms/SourWater.vb:384-559`): every non-`H⁺` species is written
    /// in closed form from the mass-action laws and the element totals, and the
    /// hydrogen-ion molality is found from the **charge-balance** condition by
    /// robust log-scale bisection. This reproduces the same equilibrium the
    /// [`Self::reaction_set`] describes; see the module Honest-scope note on why
    /// the generic reaction-extent solver
    /// ([`crate::thermo::electrolyte_svle::SvleSystem::solve_speciation`]) is
    /// **not** used for the full stiff set. Mass balance and charge neutrality
    /// hold by construction of the closed forms.
    ///
    /// # Units / ranges
    /// - `feed` — total dissolved molalities \[mol/kg\] ([`SourWaterFeed`]),
    ///   finite and `>= 0`.
    ///
    /// # Errors
    /// [`SourWaterError`] on an invalid feed, a non-finite intermediate, or a
    /// failed pH bracket.
    pub fn speciate(&self, feed: &SourWaterFeed) -> Result<SourWaterResult, SourWaterError> {
        self.solve_at(feed, &self.k)
    }

    /// Solve with DWSIM's empirical **ionic-strength / cross-species `K`
    /// corrections** to `K1` and `K5` (`SourWater.vb:454,473`) applied via an
    /// outer fixed-point loop around [`Self::speciate`].
    ///
    /// # Method
    /// Starting from the base (uncorrected) speciation, recompute `K1` from the
    /// free-H2S molality and ionic strength ([`ionic_strength_correction_k1`])
    /// and `K5` from the free-CO2 molality ([`co2_correction_k5`]), re-solve, and
    /// repeat until the pH changes by less than `outer_tol` \[pH units\] or
    /// `max_outer` iterations elapse. This reproduces DWSIM's composition-
    /// dependent `K` behaviour. When neither H2S nor CO2 is present the
    /// correction is inert and one pass suffices.
    ///
    /// # Units / ranges
    /// - `feed` — as [`Self::speciate`].
    /// - `outer_tol` — pH convergence tolerance \[-\] for the outer loop.
    /// - `max_outer` — maximum outer iterations.
    ///
    /// # Errors
    /// [`SourWaterError`] from any inner solve.
    pub fn speciate_corrected(
        &self,
        feed: &SourWaterFeed,
        outer_tol: f64,
        max_outer: usize,
    ) -> Result<SourWaterResult, SourWaterError> {
        let mut result = self.speciate(feed)?;
        let mut k = self.k;
        let t = self.temperature;
        for _ in 0..max_outer {
            let h2s = result.m(Species::H2s);
            let co2 = result.m(Species::Co2);
            let istr = result.ionic_strength;
            k[0] = ionic_strength_correction_k1(self.k[0], t, h2s, istr);
            k[4] = co2_correction_k5(self.k[4], co2);
            let next = self.solve_at(feed, &k)?;
            let dph = (next.ph - result.ph).abs();
            result = next;
            if dph < outer_tol {
                break;
            }
        }
        Ok(result)
    }
}

/// Error conditions for the sour-water speciation solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SourWaterError {
    /// A feed molality was negative or non-finite.
    #[error("invalid feed: molalities must be finite and >= 0")]
    InvalidFeed,
    /// A non-finite value appeared during the solve.
    #[error("non-finite value during the sour-water solve")]
    NonFinite,
    /// The pH bracket could not be established (residual same sign at both ends).
    #[error("failed to bracket the charge-balance root in pH")]
    NoBracket,
}

#[cfg(test)]
mod tests {
    //! # V&V — verification of the DWSIM SourWater speciation port
    //!
    //! **Methodology (shared).** These are *verification* checks against
    //! **analytic**, **closed-form**, or **public-literature** references — the
    //! SWEQ `ln K(T)` correlations reproducing textbook `pK` values, closed-form
    //! single weak-acid / weak-base pH, exact charge/mass balance of the
    //! closed-form speciation, and the correct pH-vs-loading trend — **not** validation
    //! against an experimental sour-water VLE database. Numbers below were
    //! measured on **2026-08-03** running
    //! `cargo test -p outram-park-fork-dwsim-libs --lib --release`.
    //!
    //! **Scope (honesty).** The SWEQ constants are the published EPA-600/2-80-067
    //! (Wilson 1980) correlations; nothing here is claimed experimentally
    //! accurate beyond that, and nothing is cleared for nuclear / safety-critical
    //! use.

    use super::*;

    /// **Methodology.** The SWEQ `ln K(T)` correlations must reproduce, at
    /// 25 °C, the textbook aqueous `pK` values (order-of-magnitude verification
    /// of the constants and the `K = exp(expr(1.8 T))` decoding). References
    /// (CRC / standard aqueous chemistry): `pKw ≈ 14.0`, `pKa1(H2S) ≈ 7.0`,
    /// `pKa1(CO2) ≈ 6.35`, `pKa(NH4+) ≈ 9.25`. SWEQ is an engineering
    /// correlation, so agreement to `< 1 pK unit` is the pass criterion.
    /// **Result (2026-08-03).** `pKw = 13.970`, `pKa1(H2S) = 7.082`,
    /// `pKa1(CO2) = 6.808`, `pKa(NH4+) = 9.720` — all within `≈ 0.5 pK` of the
    /// textbook values. PASS.
    #[test]
    fn sweq_constants_reproduce_textbook_pk() {
        let t = 298.15;
        let k = equilibrium_constants(t);
        let pkw = -k[6].log10();
        let pka1_h2s = -k[4].log10(); // K5 = [HS-][H+]/[H2S] = Ka1
        let pka1_co2 = -k[0].log10(); // K1 = [H+][HCO3-]/[CO2] = Ka1
        let pka_nh4 = k[2].log10(); // K3 = 1/Ka(NH4+), so pKa = +log10(K3)
        assert!((pkw - 14.0).abs() < 1.0, "pKw = {pkw}");
        assert!((pka1_h2s - 7.0).abs() < 1.0, "pKa1(H2S) = {pka1_h2s}");
        assert!((pka1_co2 - 6.35).abs() < 1.0, "pKa1(CO2) = {pka1_co2}");
        assert!((pka_nh4 - 9.25).abs() < 1.0, "pKa(NH4+) = {pka_nh4}");
    }

    /// **Methodology.** **Single weak acid (H2S) closed-form pH.** For dilute
    /// total H2S at molality `C` with `Ka1 = K5`, the dominant equilibrium is
    /// `H2S ⇌ HS⁻ + H⁺` and (neglecting the tiny second dissociation and water)
    /// `[H⁺] ≈ √(Ka1·C)`, `pH ≈ ½(pKa1 − log10 C)`. At `C = 0.1 mol/kg`,
    /// `Ka1 = 8.28e-8` ⇒ `[H⁺] ≈ 9.10e-5`, `pH ≈ 4.041`. Pass: solver pH within
    /// `0.05` of the closed form, and charge neutrality `< 1e-12 mol/kg`.
    /// **Result (2026-08-03).** Solver `pH = 4.04109` (closed form `4.04089`),
    /// `[H⁺] = 9.09730e-5 mol/kg`, `[HS⁻] = 9.09729e-5`, `[S²⁻] = 1.76e-14`
    /// (negligible), net charge `= 6.4e-14 mol/kg`, residual `= 6.4e-14`,
    /// 33 pH-bisections. Matches the closed form to `< 0.001 pH`. PASS.
    #[test]
    fn h2s_single_acid_ph_closed_form() {
        let sys = SourWaterSystem::at_temperature(298.15);
        let c = 0.1;
        let feed = SourWaterFeed::new(0.0, 0.0, c);
        let res = sys.speciate(&feed).unwrap();

        let ka1 = sys.constants()[4];
        let h_closed = (ka1 * c).sqrt();
        let ph_closed = -h_closed.log10();
        assert!(
            (res.ph - ph_closed).abs() < 0.05,
            "pH {} vs closed-form {}",
            res.ph,
            ph_closed
        );
        assert!(res.net_charge.abs() < 1e-12, "net charge = {}", res.net_charge);
        assert!(res.residual < 1e-8, "residual = {}", res.residual);
    }

    /// **Methodology.** **Single weak base (NH3) closed-form pH.** For dilute
    /// total NH3 at molality `C`, `NH3 + H2O ⇌ NH4⁺ + OH⁻` with
    /// `Kb = Kw/Ka(NH4⁺) = Kw·K3`, so `[OH⁻] ≈ √(Kb·C)` and
    /// `pH = pKw + log10[OH⁻]`. At `C = 0.1 mol/kg`, `Kb = 6.3e-5` ⇒
    /// `[OH⁻] ≈ 2.34e-3`, `pOH ≈ 2.63`, `pH ≈ 11.34`. Pass: solver pH within
    /// `0.1` of the closed form; charge neutrality `< 1e-12`.
    /// **Result (2026-08-03).** Solver `pH = 11.34017` (closed form `11.34532`),
    /// `[OH⁻] = 2.34349e-3`, `[NH4⁺] = 2.34349e-3 mol/kg`, net charge
    /// `= -4.8e-14 mol/kg`. Matches closed form to `< 0.006 pH`. PASS.
    #[test]
    fn nh3_single_base_ph_closed_form() {
        let sys = SourWaterSystem::at_temperature(298.15);
        let c = 0.1;
        let feed = SourWaterFeed::new(0.0, c, 0.0);
        let res = sys.speciate(&feed).unwrap();

        let kw = sys.constants()[6];
        let k3 = sys.constants()[2];
        let kb = kw * k3;
        let oh_closed = (kb * c).sqrt();
        let ph_closed = -kw.log10() + oh_closed.log10();
        assert!(
            (res.ph - ph_closed).abs() < 0.1,
            "pH {} vs closed-form {}",
            res.ph,
            ph_closed
        );
        assert!(res.net_charge.abs() < 1e-12, "net charge = {}", res.net_charge);
    }

    /// **Methodology.** **Charge neutrality** must hold to `< 1e-12 mol/kg` for a
    /// mixed acid-gas feed (every reaction conserves charge, feed is neutral).
    /// Feed: `CO2 = 0.05`, `NH3 = 0.05`, `H2S = 0.02 mol/kg` at 40 °C. Pass:
    /// `|Σ z·m| < 1e-12`.
    /// **Result (2026-08-03).** Solved `pH = 7.155`, net charge molality
    /// `= -9.4e-14 mol/kg` (bisection round-off only). PASS.
    #[test]
    fn mixed_feed_charge_neutral() {
        let sys = SourWaterSystem::at_temperature(313.15);
        let feed = SourWaterFeed::new(0.05, 0.05, 0.02);
        let res = sys.speciate(&feed).unwrap();
        assert!(
            res.net_charge.abs() < 1e-12,
            "net charge = {} (residual {})",
            res.net_charge,
            res.residual
        );
    }

    /// **Methodology.** **Element mass balance** (`< 1e-9 mol/kg`). Total
    /// sulfur `[H2S]+[HS⁻]+[S²⁻]`, total carbon `[CO2]+[HCO3⁻]+[CO3²⁻]+
    /// [H2NCOO⁻]`, and total nitrogen `[NH3]+[NH4⁺]+[H2NCOO⁻]` must equal their
    /// feed totals (conservation is structural in the closed forms, which are
    /// built from the totals). Feed: `CO2 = 0.05`, `NH3 = 0.08`,
    /// `H2S = 0.03 mol/kg`. Pass: each element residual `< 1e-9`.
    /// **Result (2026-08-03).** `ΔS = 0`, `ΔC = -6.9e-18`, `ΔN = 0` mol/kg —
    /// conservation exact to round-off. PASS.
    #[test]
    fn element_mass_balance_closes() {
        let sys = SourWaterSystem::at_temperature(298.15);
        let feed = SourWaterFeed::new(0.05, 0.08, 0.03);
        let res = sys.speciate(&feed).unwrap();

        let total_s =
            res.m(Species::H2s) + res.m(Species::HsMinus) + res.m(Species::SMinus2);
        let total_c = res.m(Species::Co2)
            + res.m(Species::Hco3Minus)
            + res.m(Species::Co3Minus2)
            + res.m(Species::CarbamateMinus);
        let total_n = res.m(Species::Nh3)
            + res.m(Species::Nh4Plus)
            + res.m(Species::CarbamateMinus);
        assert!((total_s - feed.h2s).abs() < 1e-9, "ΔS = {}", total_s - feed.h2s);
        assert!((total_c - feed.co2).abs() < 1e-9, "ΔC = {}", total_c - feed.co2);
        assert!((total_n - feed.nh3).abs() < 1e-9, "ΔN = {}", total_n - feed.nh3);
    }

    /// **Methodology.** **Correct pH-vs-loading trend.** Adding acid gas (H2S)
    /// must lower pH monotonically; adding base (NH3) must raise it. Checked by
    /// sweeping H2S loading (NH3 = 0) upward and NH3 loading (H2S = 0) upward.
    /// Pass: pH strictly decreasing in H2S loading, strictly increasing in NH3
    /// loading.
    /// **Result (2026-08-03).** H2S `{0.001, 0.01, 0.1} mol/kg` →
    /// `pH {5.043, 4.542, 4.041}` (decreasing); NH3 `{0.001, 0.01, 0.1}` →
    /// `pH {10.294, 10.829, 11.340}` (increasing). Both trends correct. PASS.
    #[test]
    fn ph_trend_with_loading() {
        let sys = SourWaterSystem::at_temperature(298.15);

        let mut last = f64::INFINITY;
        for &c in &[0.001, 0.01, 0.1] {
            let res = sys.speciate(&SourWaterFeed::new(0.0, 0.0, c)).unwrap();
            assert!(res.ph < last, "H2S pH not decreasing: {} !< {}", res.ph, last);
            last = res.ph;
        }

        let mut last = f64::NEG_INFINITY;
        for &c in &[0.001, 0.01, 0.1] {
            let res = sys.speciate(&SourWaterFeed::new(0.0, c, 0.0)).unwrap();
            assert!(res.ph > last, "NH3 pH not increasing: {} !> {}", res.ph, last);
            last = res.ph;
        }
    }

    /// **Methodology.** **NaOH (caustic) raises pH** and stays charge neutral.
    /// A caustic-dosed sour water must be more basic than the same acid-gas feed
    /// without caustic. Feed: `H2S = 0.05 mol/kg`, with and without
    /// `NaOH = 0.05 mol/kg`. Pass: pH(with NaOH) > pH(without); both neutral.
    /// **Result (2026-08-03).** Without caustic `pH = 4.192`; with 0.05 mol/kg
    /// NaOH `pH = 9.858` (the caustic converts most H2S to HS⁻); net charge
    /// `= 5.4e-14` / `1.0e-14 mol/kg` respectively. PASS.
    #[test]
    fn naoh_raises_ph() {
        let sys = SourWaterSystem::at_temperature(298.15);
        let plain = sys
            .speciate(&SourWaterFeed::new(0.0, 0.0, 0.05))
            .unwrap();
        let caustic = sys
            .speciate(&SourWaterFeed::new(0.0, 0.0, 0.05).with_naoh(0.05))
            .unwrap();
        assert!(
            caustic.ph > plain.ph,
            "caustic pH {} !> plain pH {}",
            caustic.ph,
            plain.ph
        );
        assert!(caustic.net_charge.abs() < 1e-12, "net charge = {}", caustic.net_charge);
    }

    /// **Methodology.** The empirical `K`-corrections must be **inert when both
    /// H2S and CO2 are absent** (no `[H2S]`, no `[CO2]`, and `I` enters only via
    /// K1 which is off): a caustic-free NH3-only feed must give the same pH from
    /// [`SourWaterSystem::speciate`] and [`SourWaterSystem::speciate_corrected`].
    /// For an H2S feed the correction shifts `K5` by `exp(0.427·[CO2]) = 1`
    /// (CO2 = 0) but `K1` is off (no carbon), so the sulfide speciation is
    /// unchanged too. Pass: corrected pH equals base pH to `< 1e-6`.
    /// **Result (2026-08-03).** NH3-only: base `pH = 11.34017`, corrected
    /// `pH = 11.34017` (Δ `= 0`). H2S-only: base `pH = 4.04109`, corrected
    /// `pH = 4.04109` (Δ `= 0`). PASS (correction correctly inert without
    /// cross-species coupling).
    #[test]
    fn corrections_inert_without_coupling() {
        let sys = SourWaterSystem::at_temperature(298.15);
        for feed in [
            SourWaterFeed::new(0.0, 0.1, 0.0),
            SourWaterFeed::new(0.0, 0.0, 0.1),
        ] {
            let base = sys.speciate(&feed).unwrap();
            let corr = sys.speciate_corrected(&feed, 1e-6, 20).unwrap();
            assert!(
                (base.ph - corr.ph).abs() < 1e-6,
                "base {} vs corrected {}",
                base.ph,
                corr.ph
            );
        }
    }

    /// **Methodology.** **Henry-law volatility correlations** (reference port)
    /// must be finite and positive at a representative sour-water state, and CO2
    /// must be far more volatile than NH3 in a dilute solution (a qualitative
    /// physical check; the correlations are `psia/(mol/kg)`). Inputs:
    /// `T = 313.15 K`, `CAS = CC = CS = 0.01 mol/kg`.
    /// **Result (2026-08-03).** `v_NH3 = 0.471`, `v_CO2 = 590.2`,
    /// `v_H2S = 218.4` psia/(mol/kg) — all finite, positive, with
    /// `v_CO2 > v_H2S > v_NH3` as expected. PASS.
    #[test]
    fn henry_volatility_reference_sane() {
        let (v_nh3, v_co2, v_h2s) = henry_volatility(313.15, 0.01, 0.01, 0.01);
        for v in [v_nh3, v_co2, v_h2s] {
            assert!(v.is_finite() && v > 0.0, "volatility {v} not finite/positive");
        }
        assert!(v_co2 > v_nh3, "CO2 should be more volatile than NH3");
    }

    /// **Methodology.** The reaction-set description reused from
    /// [`crate::thermo::electrolyte_svle`] must be **self-consistent**: exactly
    /// [`N_SPECIES`] species, seven finite-`K` reactions, each with a
    /// species-length stoichiometry, a positive `K`, and — the physically
    /// essential property — **charge conservation** `Σ_s z_s ν_s = 0` (so a
    /// neutral feed stays neutral, the basis of the charge-neutrality tests).
    /// Charges follow the [`Species`] assignments.
    /// **Result (2026-08-03).** 13 species, 7 reactions; every reaction's
    /// `Σ z·ν = 0` (exactly), every `K > 0`. PASS.
    #[test]
    fn reaction_set_is_charge_conserving() {
        let sys = SourWaterSystem::at_temperature(298.15);
        let species = SourWaterSystem::svle_species();
        assert_eq!(species.len(), N_SPECIES);
        let rxns = sys.reaction_set();
        assert_eq!(rxns.len(), 7);
        let charges: [f64; N_SPECIES] = [
            0.0, 1.0, -1.0, 0.0, 1.0, 0.0, -1.0, -2.0, -1.0, 0.0, -1.0, -2.0, 1.0,
        ];
        for (r, rx) in rxns.iter().enumerate() {
            assert_eq!(rx.stoich.len(), N_SPECIES, "reaction {r} wrong length");
            assert!(rx.k > 0.0, "reaction {r} K = {}", rx.k);
            let dz: f64 = rx
                .stoich
                .iter()
                .zip(charges.iter())
                .map(|(nu, z)| nu * z)
                .sum();
            assert!(dz.abs() < 1e-12, "reaction {r} charge imbalance {dz}");
            // Water (index 0) is absorbed into K — coefficient must be 0.
            assert_eq!(rx.stoich[Species::Water.index()], 0.0, "reaction {r} uses water");
        }
    }
}

