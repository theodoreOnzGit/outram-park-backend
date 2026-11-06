// SPDX-License-Identifier: GPL-3.0
//
// TRISO-ATOPS fork — model variant selector (gh:#1015)
// -----------------------------------------------------
// NOT a port of upstream code. `TrisoAtopsVariant::Upstream` selects the
// ported behaviour of upstream TRISO-ATOPS de374c8 (MIT, Battelle Energy
// Alliance, LLC; see LICENSE.triso-atops / NOTICE.triso-atops), unchanged.
// `TrisoAtopsVariant::Atops2026Paper` selects the model as the published paper
// writes it, implemented here from the paper's printed equations (cited by
// page; no text, figure or table of the paper is reproduced).

//! # Model variant: upstream code, or the 2026 paper as written (gh:#1015)
//!
//! The upstream TRISO-ATOPS code (de374c8) and the paper that describes it
//! differ in several places. This module lets a caller choose which one runs.
//! **The default is [`TrisoAtopsVariant::Upstream`]**, the ported code, bit for
//! bit; the paper is an explicit, named option (maintainer decision on
//! gh:#1015, 2026-10-22: "(b) list it as atops2026paper option").
//!
//! ## The paper
//!
//! B. Stoyer, A. Raichart and D. Petti, "TRISO-ATOPS: A Mechanistic Source
//! Term Model for Gas-Cooled Reactors", *Nuclear Technology* (2026), published
//! online 17 August 2026, doi:[10.1080/00295450.2026.2671624](https://doi.org/10.1080/00295450.2026.2671624).
//! Received 27 October 2025, accepted 26 April 2026. The online-first PDF
//! prints no volume or issue; its pages are numbered 1-28. Page numbers below
//! are the printed ones. (Restricted literature: cited, never reproduced.)
//!
//! ## What the paper variant changes
//!
//! Each item is one field of [`PaperReadings`]; the text reading is the
//! default ([`PaperReadings::AS_WRITTEN`]), the upstream code's behaviour is
//! the alternative ([`PaperReadings::UPSTREAM_CODE`]). How each reading was
//! ranked, and what each does to the paper's own printed release numbers, is
//! in `crates/boon-lay/docs/triso-atops-2026-paper-reproduction.md`.
//!
//! | # | Item | Paper (page) | Upstream code |
//! |---|---|---|---|
//! | a | Booth sphere radius `a`, normal operation | root-mean-square of kernel radius and grain size, 150.8 µm for Table 3 (pp. 7-8, nomenclature p. 26) | `√(2·a_grain·r)`, 65.3 µm |
//! | b | Booth sphere radius, accident | the same `a`, Eq. (19) with `D' = D/a²` (pp. 11-12) | kernel radius `r` |
//! | c | Yttrium | Sr/Ba/Eu group, Sr diffusivities (Table 1 p. 6; p. 9) | "other" metal |
//! | d | Below a law's fitted range | nearest in-range temperature, every law (pp. 8-9) | clamped for Rb/Cs, Sr/Ba/Eu, Ag/Pd graphite only |
//! | e | Infinite series in Eqs. (6c), (6d), (8), (18), (19), (21) | summed to `∞` (pp. 7-8, 10-12) | 5000/1000/500/5000 terms; transient Booth zeroed below 1e-6 |
//! | f | "Other" fission metals | Booth Eqs. (6b)/(6c) with `D = 1e-19 m²/s` (pp. 6-7, 9) | fixed `<R/B>_fail = 1e-5` |
//! | g | Palladium in an accident | silver group: SiC diffusivity, breakthrough Eq. (18) (Table 1 p. 6; p. 11) | Booth on the Ag-kernel law |
//! | h | Long-lived noble gases, halogens, Te, Se | Booth Eq. (6c) on krypton's diffusivity (p. 5; p. 9) | empirical Eq. (6a) at any half-life |
//! | i | Birth rate of a long-lived nuclide | `1 − exp(−λ t_run time)`, Eqs. (4b), (4d) (pp. 4-5) | `1 − exp(−λ t_irrad)` |
//! | j | Accident fuel inventory | plate-out (and HPS) scaled by `t_irrad / t_run time`, no branch, Eqs. (22), (23), (28) (pp. 12-13) | unscaled, with a branch on `F·f − P > 0` |
//!
//! Items a-d are the four differences filed on gh:#1015; e-j are the further
//! places the paper's equations differ from the code, found by reading the
//! paper's model sections (s.II) in full against the port (maintainer scope,
//! 2026-10-22: "as close to paper as possible").
//!
//! **Not changed** (the paper does not specify them differently): the
//! breathing / venting integration (Eqs. (26)-(27): trapezoidal, as the code),
//! the attenuation cap `1e8` (p. 10), the empirical Eq. (6a) constants
//! (Table 4), the 20 % and 4 % half-life screens (p. 5, p. 11), the plate-out
//! and clean-up balances (Eqs. (11)-(17)), and the ideal-gas constant.
//! Parent-daughter coupling and the plate-out constant are run inputs, not
//! part of the variant: see [`crate::triso_atops_fork::run_selection::ParentDecayPolicy`]
//! and [`crate::triso_atops_fork::normal_operation::PlantConstants::k_plate`].
//!
//! ## Selecting it
//!
//! ```
//! use boon_lay::triso_atops_fork::variant::TrisoAtopsVariant;
//! let v = TrisoAtopsVariant::ATOPS2026PAPER;
//! assert_eq!(v.id(), "atops2026paper");
//! assert_eq!(TrisoAtopsVariant::from_id("atops2026paper"), Some(v));
//! assert_eq!(TrisoAtopsVariant::default(), TrisoAtopsVariant::Upstream);
//! ```
//!
//! The variant is the `variant` field of
//! [`PlantConstants`](crate::triso_atops_fork::normal_operation::PlantConstants)
//! for normal operation, and an explicit argument of the `*_for` accident
//! functions ([`crate::triso_atops_fork::release_models::release_fraction_transient_for`],
//! [`crate::triso_atops_fork::diffusion::integrate_diffusion_over_time_for`],
//! [`crate::triso_atops_fork::accident::release_activity_for`]).

use petir::mathf::RealMath;
use petir::specfunc::erfc;
use std::f64::consts::PI;
use uom::si::f64::Length;
use uom::si::length::meter;

/// Which TRISO-ATOPS model runs: the upstream code (default) or the paper.
///
/// See the [module docs](self) for the full citation and the list of
/// differences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrisoAtopsVariant {
    /// The ported upstream TRISO-ATOPS code at de374c8, bit for bit. The
    /// default: the port's contract is code-to-code agreement with upstream.
    #[default]
    Upstream,
    /// The model as printed in B. Stoyer, A. Raichart and D. Petti,
    /// "TRISO-ATOPS: A Mechanistic Source Term Model for Gas-Cooled Reactors",
    /// *Nuclear Technology* (2026), published online 17 August 2026,
    /// doi:10.1080/00295450.2026.2671624. Snake-case id `atops2026paper`.
    ///
    /// **It does not reproduce the paper's own printed tables.** Those were
    /// computed by the upstream code: `Upstream` reproduces them, and this
    /// variant misses, e.g., Table 19's Kr-85 by +62.8 % and Sr-90 by
    /// −56.1 % (`docs/triso-atops-2026-paper-reproduction.md`, 2026-10-22).
    ///
    /// The payload says how each place where the paper and the code differ is
    /// read. [`TrisoAtopsVariant::ATOPS2026PAPER`] uses
    /// [`PaperReadings::AS_WRITTEN`], the readings best supported by the
    /// paper's text; other readings exist for the sensitivity analysis in
    /// `docs/triso-atops-2026-paper-reproduction.md`.
    Atops2026Paper(PaperReadings),
}

impl TrisoAtopsVariant {
    /// The paper variant with every reading as the paper's text gives it.
    pub const ATOPS2026PAPER: Self = Self::Atops2026Paper(PaperReadings::AS_WRITTEN);

    /// Snake-case id: `"upstream"` or `"atops2026paper"`.
    #[must_use]
    pub fn id(&self) -> &'static str {
        match self {
            Self::Upstream => "upstream",
            Self::Atops2026Paper(_) => "atops2026paper",
        }
    }

    /// Parse a snake-case id. `"atops2026paper"` gives
    /// [`TrisoAtopsVariant::ATOPS2026PAPER`] (the as-written readings).
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "upstream" => Some(Self::Upstream),
            "atops2026paper" => Some(Self::ATOPS2026PAPER),
            _ => None,
        }
    }

    /// The readings this variant applies. [`TrisoAtopsVariant::Upstream`]
    /// is [`PaperReadings::UPSTREAM_CODE`]: every choice made as the code
    /// makes it, through the ported functions themselves.
    #[must_use]
    pub fn readings(&self) -> PaperReadings {
        match self {
            Self::Upstream => PaperReadings::UPSTREAM_CODE,
            Self::Atops2026Paper(r) => *r,
        }
    }
}

/// How each paper-versus-code difference is read. One field per row of the
/// table in the [module docs](self).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaperReadings {
    /// (a) Booth equivalent-sphere radius in normal operation.
    pub booth_radius: BoothRadius,
    /// (b) Booth radius in the accident Eq. (19).
    pub accident_booth_radius: AccidentBoothRadius,
    /// (c) Yttrium's transport group.
    pub yttrium: YttriumGroup,
    /// (d) Diffusion laws below their fitted range.
    pub low_temperature: LowTemperatureLaw,
    /// (e) The infinite series.
    pub series: SeriesEvaluation,
    /// (f) Release of the "other" fission metals.
    pub other_metals: OtherMetalRelease,
    /// (h) Normal-operation release of long-lived volatiles.
    pub long_lived_volatiles: LongLivedVolatileRelease,
    /// (i) The time in a long-lived nuclide's birth rate.
    pub birth_rate_time: BirthRateTime,
    /// (j) The accident fuel inventory.
    pub accident_inventory: AccidentInventory,
    /// Silver's accident inventory: all particles or failed ones.
    pub silver_accident_inventory: SilverAccidentInventory,
}

impl PaperReadings {
    /// Every reading as the paper's text gives it (the `atops2026paper`
    /// default). Item (g), palladium, has no field of its own: see
    /// [`PaperReadings::palladium_as_silver`].
    pub const AS_WRITTEN: Self = Self {
        booth_radius: BoothRadius::RmsOfRadii,
        accident_booth_radius: AccidentBoothRadius::SameAsNormalOperation,
        yttrium: YttriumGroup::StrontiumGroup,
        low_temperature: LowTemperatureLaw::NearestInRange,
        series: SeriesEvaluation::Converged,
        other_metals: OtherMetalRelease::BoothAtNominalDiffusivity,
        long_lived_volatiles: LongLivedVolatileRelease::BoothKryptonDiffusivity,
        birth_rate_time: BirthRateTime::RunTime,
        accident_inventory: AccidentInventory::NormalisedPlateOut,
        silver_accident_inventory: SilverAccidentInventory::AllParticles,
    };

    /// Every reading as the upstream code makes it. Selecting
    /// `Atops2026Paper(UPSTREAM_CODE)` runs the ported functions and so
    /// reproduces [`TrisoAtopsVariant::Upstream`] bit for bit (pinned by a
    /// test in `tests/mhtgr_stoyer_workflow.rs`).
    pub const UPSTREAM_CODE: Self = Self {
        booth_radius: BoothRadius::UpstreamGeometricMean,
        accident_booth_radius: AccidentBoothRadius::KernelRadius,
        yttrium: YttriumGroup::UpstreamOther,
        low_temperature: LowTemperatureLaw::UpstreamPartialClamp,
        series: SeriesEvaluation::UpstreamTruncated,
        other_metals: OtherMetalRelease::UpstreamFixedRb,
        long_lived_volatiles: LongLivedVolatileRelease::UpstreamEmpirical,
        birth_rate_time: BirthRateTime::IrradiationTime,
        accident_inventory: AccidentInventory::UpstreamUnnormalised,
        silver_accident_inventory: SilverAccidentInventory::AllParticles,
    };

    /// (g) Whether palladium takes silver's laws in an accident (SiC
    /// diffusivity, breakthrough Eq. (18)). Table 1 (p. 6) groups Pd with Ag
    /// and p. 11 applies Eq. (18) to the silver group; upstream gates its
    /// silver branch on `z == 47` only. Tied to the diffusivity reading:
    /// on whenever `low_temperature` is the paper's.
    #[must_use]
    pub fn palladium_as_silver(&self) -> bool {
        self.low_temperature == LowTemperatureLaw::NearestInRange
    }
}

impl Default for PaperReadings {
    fn default() -> Self {
        Self::AS_WRITTEN
    }
}

/// (a) The Booth equivalent-sphere radius in normal operation.
///
/// The paper says "root-mean-square of the kernel diameter and the grain
/// diameter" on p. 7 and "root-mean-square of the kernel radius and the grain
/// size of the kernel" on p. 8; the nomenclature (p. 26) defines `a` as the
/// "root-mean-square of kernel radius and grain size". Ranked readings, with
/// the reasons, in `docs/triso-atops-2026-paper-reproduction.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoothRadius {
    /// `√((r² + a_grain²)/2)`: 150.8 µm for Table 3. The nomenclature's
    /// definition (p. 26), p. 8's, and the only reading inside p. 8's stated
    /// bound ("between the radius and grain size of the kernel"). Rank 1.
    RmsOfRadii,
    /// `√(((2r)² + a_grain²)/2)`: 301.3 µm. p. 7's "kernel diameter", with the
    /// grain size taken as the grain diameter (p. 24). Larger than the kernel
    /// radius, so outside p. 8's bound. Rank 2.
    RmsOfDiameters,
    /// `√(2·a_grain·r)`: 65.3 µm, upstream's `R_B_fail`. Not a reading of the
    /// text; the fallback. Rank 3.
    UpstreamGeometricMean,
}

/// (b) The Booth radius in the accident Eq. (19).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccidentBoothRadius {
    /// The normal-operation `a` of [`BoothRadius`]: Eq. (19) is Eq. (6c)
    /// with `D't` replaced by `∫D' dt`, and `D' = D/a²` (pp. 8, 11-12).
    SameAsNormalOperation,
    /// The kernel radius `r` (upstream `release_fraction`).
    KernelRadius,
}

/// (c) Yttrium's transport group.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YttriumGroup {
    /// Group 2 with Sr, Ba, Eu, on strontium's diffusivities (Table 1, p. 6;
    /// p. 9: "strontium's diffusivities are used for barium, yttrium, and
    /// europium").
    StrontiumGroup,
    /// Upstream: "other" metal (`<R/B>_fail = 1e-5`, `D = 1e-19 m²/s`).
    UpstreamOther,
}

/// (d) What a diffusion law does below its fitted temperature range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LowTemperatureLaw {
    /// Every Table 5 law is evaluated at the nearest in-range temperature
    /// (pp. 8-9): adds the kernel Kr law (Kr, Xe, I, Te, Se; 700 °C) and the
    /// SiC Ag law (700 °C) to upstream's clamps.
    NearestInRange,
    /// Upstream: only Rb/Cs, Sr/Ba/Eu and the Ag/Pd graphite law are clamped.
    UpstreamPartialClamp,
}

/// (e) How the infinite series are evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesEvaluation {
    /// To convergence, as the paper's `∞` upper limits say: the exponential
    /// series where it converges fast, the equivalent short-time `ierfc` /
    /// `erfc` series (Crank 1957) where it does not. No floor.
    Converged,
    /// Upstream's fixed term counts (Booth 4999, breakthrough 999,
    /// attenuation 250 odd terms, graphite 2500 odd terms) and the transient
    /// Booth's `1e-6` zero floor.
    UpstreamTruncated,
}

/// (f) Release of the fission metals with no Table 5 law.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtherMetalRelease {
    /// Booth Eq. (6b) (short-lived) or (6c) (long-lived) on `D = 1e-19 m²/s`:
    /// p. 9 gives that `D` to "nuclides that do not use Eq. (7)", and pp. 6-7
    /// compute fission metals' `<R/B>_fail` with the Booth models.
    BoothAtNominalDiffusivity,
    /// Upstream: a fixed `<R/B>_fail = 1e-5`.
    UpstreamFixedRb,
}

/// (h) Normal-operation release of long-lived noble gases, halogens, Te, Se.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LongLivedVolatileRelease {
    /// Eq. (6a) is for the *short-lived* volatiles (p. 6); long-lived nuclides
    /// use Eq. (6c) (p. 5), on krypton's diffusivity (p. 9).
    BoothKryptonDiffusivity,
    /// Upstream: Eq. (6a) at any half-life.
    UpstreamEmpirical,
}

/// (i) The time in a long-lived nuclide's birth rate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BirthRateTime {
    /// `t_run time`, Eqs. (4b) and (4d) (pp. 4-5).
    RunTime,
    /// Upstream: `t_irrad`.
    IrradiationTime,
}

/// (j) The accident fuel inventory of Eqs. (22), (23) and (28).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccidentInventory {
    /// As printed: plate-out (and HPS) scaled by `t_irrad / t_run time`
    /// (p. 12 text and Eqs. (22), (23), (28)), one formula, no branch.
    NormalisedPlateOut,
    /// Upstream: unscaled, and a different formula when `F·f − P ≤ 0`.
    UpstreamUnnormalised,
}

/// Silver's accident fuel inventory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SilverAccidentInventory {
    /// All particles: the coatings are no barrier to silver, so Eq. (5b)
    /// applies `<R/B>_fail` to every particle (p. 5), and Eq. (18) is the
    /// silver form of Eq. (6d). Upstream does the same. Rank 1.
    AllParticles,
    /// Eq. (23) taken literally for silver too: only failed particles. Rank 2.
    FailedParticles,
}

/// (a) The Booth equivalent-sphere radius for a reading, in metres.
#[must_use]
pub fn booth_radius_m(reading: BoothRadius, grain_size: Length, kernel_radius: Length) -> f64 {
    let g = grain_size.get::<meter>();
    let r = kernel_radius.get::<meter>();
    match reading {
        BoothRadius::RmsOfRadii => ((r * r + g * g) / 2.0).sqrt(),
        BoothRadius::RmsOfDiameters => ((4.0 * r * r + g * g) / 2.0).sqrt(),
        // Upstream's own expression, in upstream's operation order.
        BoothRadius::UpstreamGeometricMean => (g * 2.0 * r).sqrt(),
    }
}

// ---------------------------------------------------------------------------
// Converged series (item e). Each infinite series is evaluated by whichever
// of its two exact forms converges fast at the given argument: the
// exponential (long-time) series for tau >= TAU_SWITCH, the error-function
// (short-time) series below. Both forms are exact; the switch only chooses
// which one needs few terms. References: J. Crank, The Mathematics of
// Diffusion (the paper's ref. [10]): sphere Eq. (6.20)/(6.21), slab
// Eqs. (4.17)-(4.19), membrane time lag from the Laplace transform of
// Eq. (4.24a)'s problem (derivation in the function docs).
// ---------------------------------------------------------------------------

/// Where the converged series switch from the short-time to the long-time
/// form. Both are exact; at `tau = 0.1` each needs under 20 terms for 1e-18.
pub const TAU_SWITCH: f64 = 0.1;

/// Iterated complementary error function `ierfc(x) = ∫ₓ^∞ erfc(s) ds
/// = exp(−x²)/√π − x·erfc(x)`, for `x ≥ 0`.
///
/// Below `x = 10` the closed form is used (it cancels by about `2x²`, so at
/// most 2.3 digits are lost); above, the asymptotic series
/// `exp(−x²)/√π · Σ_k (−1)^{k+1} (2k−1)!!/(2x²)^k`, whose terms shrink by
/// `(2k+1)/(2x²) ≤ 0.1` for the first ten and so reach 1e-18 well before the
/// series turns.
#[must_use]
pub fn ierfc(x: f64) -> f64 {
    let inv_sqrt_pi = 1.0 / PI.sqrt();
    if x < 10.0 {
        return (-x * x).r_exp() * inv_sqrt_pi - x * erfc(x);
    }
    let e = (-x * x).r_exp();
    if e == 0.0 {
        return 0.0;
    }
    let y = 1.0 / (2.0 * x * x);
    let mut term = y; // k = 1: 1/(2x^2)
    let mut sum = term;
    let mut k = 1.0;
    while k < 60.0 {
        let next = -term * (2.0 * k + 1.0) * y;
        if next.abs() >= term.abs() || next.abs() < 1e-18 * sum.abs() {
            break;
        }
        sum += next;
        term = next;
        k += 1.0;
    }
    e * inv_sqrt_pi * sum
}

/// Sum `f(n)` for `n = 1, 2, ...` until a term is below `1e-18` of the
/// running sum's magnitude (or exactly zero), capped at 100 000 terms.
fn sum_until_negligible(mut f: impl FnMut(f64) -> f64) -> f64 {
    let mut sum = 0.0;
    let mut n = 1.0;
    while n < 1e5 {
        let t = f(n);
        sum += t;
        if t == 0.0 || t.abs() < 1e-18 * sum.abs() {
            break;
        }
        n += 1.0;
    }
    sum
}

/// Fractional release from a sphere with uniform initial concentration and
/// zero surface concentration, `1 − 6 Σ_{n≥1} exp(−(nπ)² τ)/(nπ)²`, summed to
/// convergence: Eqs. (6c) and (19) of the paper (p. 7, p. 11), `τ = D't` or
/// `∫D' dt`.
///
/// Short-time form (Crank Eq. 6.21): `6√τ (π^{-1/2} + 2 Σ ierfc(n/√τ)) − 3τ`.
#[must_use]
pub fn sphere_release(tau: f64) -> f64 {
    if tau <= 0.0 {
        return 0.0;
    }
    let f = if tau < TAU_SWITCH {
        let s = 1.0 / tau.sqrt();
        let tail = sum_until_negligible(|n| ierfc(n * s));
        6.0 * tau.sqrt() * (1.0 / PI.sqrt() + 2.0 * tail) - 3.0 * tau
    } else {
        1.0 - 6.0 / (PI * PI)
            * sum_until_negligible(|n| (-(n * PI) * (n * PI) * tau).r_exp() / (n * n))
    };
    f.clamp(0.0, 1.0)
}

/// The dimensionless time-lag permeation `Q̃(τ) = Q_t/(l·C₁)` through a
/// membrane of thickness `l`, constant concentration `C₁` on one face, zero
/// on the other, initially empty, `τ = Dt/l²` (the constant-source problem the
/// paper's Eq. (6d) and (18) are written from, p. 7, Crank ref. [10]).
///
/// Long-time form: `τ − 1/6 − (2/π²) Σ (−1)ⁿ exp(−n²π²τ)/n²`.
/// Short-time form: `4√τ Σ_{m≥0} ierfc((2m+1)/(2√τ))`, from the Laplace
/// transform `Q̄ = D C₁ q / (s² sinh(q l))`, `q = √(s/D)`, expanded in
/// `exp(−(2m+1) q l)` and inverted with `L⁻¹[e^{−k√s}/s^{3/2}] = 2√t ierfc(k/(2√t))`.
#[must_use]
pub fn membrane_permeation(tau: f64) -> f64 {
    if tau <= 0.0 {
        return 0.0;
    }
    let q = if tau < TAU_SWITCH {
        let h = 1.0 / (2.0 * tau.sqrt());
        // m = 0, 1, 2, ... as n - 1.
        4.0 * tau.sqrt() * sum_until_negligible(|n| ierfc((2.0 * n - 1.0) * h))
    } else {
        let s = sum_until_negligible(|n| {
            let sign = if (n as u64) % 2 == 0 { 1.0 } else { -1.0 };
            sign * (-(n * PI) * (n * PI) * tau).r_exp() / (n * n)
        });
        tau - 1.0 / 6.0 - 2.0 / (PI * PI) * s
    };
    q.max(0.0)
}

/// The silver breakthrough release, Eq. (6d) without its decay-constant
/// factor (p. 8) and Eq. (18) (p. 11), summed to convergence:
/// `3Dt/(a r) − a/(2r) − (6a/r) Σ (−1)ⁿ exp(−(nπ)²τ)/(nπ)²`, which is exactly
/// `(3a/r)·Q̃(τ)` with `τ = Dt/a²` (substitute [`membrane_permeation`]'s
/// long-time form). Clamped to `[0, 1]` (p. 8: "set equal to 1.0").
#[must_use]
pub fn breakthrough_release(tau: f64, sic_thickness_m: f64, kernel_radius_m: f64) -> f64 {
    (3.0 * sic_thickness_m / kernel_radius_m * membrane_permeation(tau)).clamp(0.0, 1.0)
}

/// Fractional release from a slab of half-thickness `l` (here `a_graph`),
/// uniform initial concentration, zero surface concentration:
/// `Σ_{n odd} 8(1 − exp(−(nπ)²τ/4))/(nπ)² = 1 − Σ_{n odd} 8 exp(−(nπ)²τ/4)/(nπ)²`,
/// `τ = ∫D dt / a_graph²`: the paper's Eq. (21) (p. 12), summed to
/// convergence. Short-time form (Crank Eq. 4.19):
/// `2√τ (π^{-1/2} + 2 Σ_{n≥1} (−1)ⁿ ierfc(n/√τ))`.
#[must_use]
pub fn slab_release(tau: f64) -> f64 {
    if tau <= 0.0 {
        return 0.0;
    }
    let f = if tau < TAU_SWITCH {
        let s = 1.0 / tau.sqrt();
        let tail = sum_until_negligible(|n| {
            let sign = if (n as u64) % 2 == 0 { 1.0 } else { -1.0 };
            sign * ierfc(n * s)
        });
        2.0 * tau.sqrt() * (1.0 / PI.sqrt() + 2.0 * tail)
    } else {
        1.0 - sum_until_negligible(|k| {
            let n = 2.0 * k - 1.0;
            8.0 / ((n * PI) * (n * PI)) * (-(n * PI) * (n * PI) * tau / 4.0).r_exp()
        })
    };
    f.clamp(0.0, 1.0)
}

/// The graphite attenuation factor of Eq. (8) (p. 10),
/// `AF = 1/(1 − S)`, `S = Σ_{n odd} (4/(nπ)) sin(nπ/2) exp(−(nπ)²τ/4)`,
/// `τ = D_g t_irrad / a_graph²`, summed to convergence and capped at `1e8`
/// (p. 10). `S` is the centre concentration of a slab of half-thickness
/// `a_graph` (Crank Eq. 4.17); for small `τ` the complement `1 − S` is taken
/// from the short-time form `2 Σ_{m≥0} (−1)^m erfc((2m+1)/(2√τ))` so it keeps
/// its digits where `S` is within round-off of 1.
#[must_use]
pub fn attenuation_factor_converged(tau: f64, cap: f64) -> f64 {
    let one_minus_s = if tau <= 0.0 {
        0.0
    } else if tau < TAU_SWITCH {
        let h = 1.0 / (2.0 * tau.sqrt());
        2.0 * sum_until_negligible(|n| {
            let sign = if (n as u64) % 2 == 1 { 1.0 } else { -1.0 };
            sign * erfc((2.0 * n - 1.0) * h)
        })
    } else {
        1.0 - sum_until_negligible(|k| {
            let n = 2.0 * k - 1.0;
            // sin(n pi / 2) for odd n is exactly (-1)^(k+1).
            let sign = if (k as u64) % 2 == 1 { 1.0 } else { -1.0 };
            sign * 4.0 / (n * PI) * (-(n * PI) * (n * PI) * tau / 4.0).r_exp()
        })
    };
    if one_minus_s <= 0.0 {
        return cap;
    }
    (1.0 / one_minus_s).min(cap)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The two exact forms of each series agree where they meet: both sides
    /// of [`TAU_SWITCH`] evaluated by brute force (200 000 terms of the
    /// exponential series, which at `τ ≥ 1e-3` converges to round-off).
    ///
    /// Methodology: for `τ ∈ {1e-3, 0.02, 0.0999, 0.1001, 0.5}`, compare each
    /// converged function with a long direct sum. Pass: 1e-9 relative to
    /// `max(|value|, 1e-3)`.
    /// Result (2026-10-22): pass (worst printed by the test).
    #[test]
    fn converged_series_match_brute_force_sums() {
        let brute_sphere = |tau: f64| {
            let mut s = 0.0;
            for n in (1..200_000).rev() {
                let n = n as f64;
                s += (-(n * PI) * (n * PI) * tau).r_exp() / ((n * PI) * (n * PI));
            }
            1.0 - 6.0 * s
        };
        let brute_slab = |tau: f64| {
            let mut s = 0.0;
            for k in (1..200_000).rev() {
                let n = (2 * k - 1) as f64;
                s += 8.0 / ((n * PI) * (n * PI)) * (-(n * PI) * (n * PI) * tau / 4.0).r_exp();
            }
            1.0 - s
        };
        let brute_membrane = |tau: f64| {
            let mut s = 0.0;
            for n in (1..200_000).rev() {
                let sign = if n % 2 == 0 { 1.0 } else { -1.0 };
                let n = n as f64;
                s += sign * (-(n * PI) * (n * PI) * tau).r_exp() / (n * n);
            }
            tau - 1.0 / 6.0 - 2.0 / (PI * PI) * s
        };
        let mut worst = 0.0f64;
        for tau in [1e-3, 0.02, 0.0999, 0.1001, 0.5] {
            for (a, b) in [
                (sphere_release(tau), brute_sphere(tau)),
                (slab_release(tau), brute_slab(tau)),
                (membrane_permeation(tau), brute_membrane(tau)),
            ] {
                // Relative, floored at 1e-3: at tau = 1e-3 the membrane value
                // is ~1e-110 and the brute-force sum is pure round-off.
                worst = worst.max((a - b).abs() / b.abs().max(1e-3));
            }
        }
        println!("converged vs brute force, worst relative difference {worst:.3e}");
        assert!(worst < 1e-9, "{worst:e}");
    }

    /// Short-time limits: sphere `6√(τ/π) − 3τ`, slab `2√(τ/π)`, and the
    /// membrane exponentially small, at `τ = 1e-6` where every correction
    /// term is below 1e-300.
    #[test]
    fn short_time_limits_are_exact() {
        let tau: f64 = 1e-6;
        assert!((sphere_release(tau) - (6.0 * (tau / PI).sqrt() - 3.0 * tau)).abs() < 1e-18);
        assert!((slab_release(tau) - 2.0 * (tau / PI).sqrt()).abs() < 1e-18);
        assert_eq!(membrane_permeation(tau), 0.0);
        assert_eq!(attenuation_factor_converged(tau, 1e8), 1e8);
        assert_eq!(sphere_release(0.0), 0.0);
    }

    /// `ierfc` against its definition at two points either side of the
    /// asymptotic switch, and `ierfc(0) = 1/√π`.
    #[test]
    fn ierfc_matches_closed_form_and_asymptote() {
        assert!((ierfc(0.0) - 1.0 / PI.sqrt()).abs() < 1e-16);
        // Continuity across x = 10: the closed form at 10 - 1e-9 and the
        // asymptotic series at 10 + 1e-9 differ by the function's own slope,
        // d ln ierfc / dx ~ -2x = -20, i.e. about 4e-8 relative.
        let a = ierfc(10.0 - 1e-9);
        let b = ierfc(10.0 + 1e-9);
        assert!((a - b).abs() / a < 1e-7, "{a:e} {b:e}");
        // Leading asymptote at x = 20.
        let x: f64 = 20.0;
        let lead = (-x * x).r_exp() / PI.sqrt() / (2.0 * x * x);
        assert!((ierfc(x) / lead - 1.0).abs() < 4.0 / (x * x));
    }

    /// The radius readings for Table 3's kernel radius 213 µm and grain size
    /// 10 µm: 150.8, 301.3 and 65.3 µm.
    #[test]
    fn booth_radius_readings_for_table_3() {
        let g = Length::new::<meter>(1.0e-5);
        let r = Length::new::<meter>(2.13e-4);
        let um = |x: BoothRadius| booth_radius_m(x, g, r) * 1e6;
        assert!((um(BoothRadius::RmsOfRadii) - 150.779).abs() < 1e-3);
        assert!((um(BoothRadius::RmsOfDiameters) - 301.310).abs() < 1e-3);
        assert!((um(BoothRadius::UpstreamGeometricMean) - 65.269).abs() < 1e-3);
    }

    /// Ids round-trip; the default is upstream.
    #[test]
    fn ids_round_trip() {
        for v in [TrisoAtopsVariant::Upstream, TrisoAtopsVariant::ATOPS2026PAPER] {
            assert_eq!(TrisoAtopsVariant::from_id(v.id()), Some(v));
        }
        assert_eq!(TrisoAtopsVariant::from_id("paper"), None);
        assert_eq!(
            TrisoAtopsVariant::Upstream.readings(),
            PaperReadings::UPSTREAM_CODE
        );
        assert_eq!(PaperReadings::default(), PaperReadings::AS_WRITTEN);
    }
}
