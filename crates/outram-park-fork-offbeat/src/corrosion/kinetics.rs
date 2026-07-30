// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/corrosion/corrosionModel/oxidationKineticsModel/`:
//   oxidationKineticsModel.{C,H}          -> OxidationKinetics (the enum itself)
//   EPRI_KWU_CE.{C,H}                     -> OxidationKinetics::EpriKwuCe
//   CathcartPawel.{C,H}                   -> OxidationKinetics::CathcartPawel
//   lowHighOxidationKineticsModel.{C,H}   -> OxidationKinetics::EpriKwuCeCathcartPawel
//   oxidationKineticsModels.{C,H}         -> the registered instantiation
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Oxide-growth kinetics — how thick the ZrO2 layer is after a timestep \[m\].
//!
//! # What these correlations compute
//!
//! Each variant of [`OxidationKinetics`] answers one question: given the oxide
//! thickness `S0` \[m\] at the start of a timestep, the temperature at the
//! **metal/oxide interface** `T` \[K\], the fast-neutron flux, and the step
//! length `dt` \[s\], what is the thickness `S` \[m\] at the end of the step?
//!
//! They are integrated forms, not rate equations: the correlation is written as
//! a closed-form growth law and evaluated over the whole step, so a caller does
//! not need a sub-stepping ODE integrator. [`thickness`] returns the new
//! thickness; [`growth`] returns the increment; [`growth_rate`] returns the
//! increment divided by `dt`, i.e. the **mean** rate over the step, not the
//! instantaneous rate at either end.
//!
//! [`thickness`]: OxidationKinetics::thickness
//! [`growth`]: OxidationKinetics::growth
//! [`growth_rate`]: OxidationKinetics::growth_rate
//!
//! # Which temperature
//!
//! `T` is the **metal/oxide interface** temperature, not the coolant
//! temperature and not the oxide's outer-surface temperature. Corrosion is
//! controlled by diffusion through the oxide, which is anchored at the metal
//! face. As the oxide thickens it insulates, so the interface runs hotter than
//! the surface and the reaction accelerates. Upstream computes the interface
//! temperature in `zircaloyOuterCorrosion::correct`; that calculation is ported
//! separately in [`super::thermal`], and its result is what should be fed here.
//!
//! # Units
//!
//! - thickness \[m\], timestep \[s\], temperature \[K\]
//! - fast flux \[n/(m²·s)\] — **SI**, converted to upstream's n/(cm²·s) basis
//!   inside the correlation. See the [module documentation](super) for why this
//!   conversion is done once at the boundary.
//!
//! # Validity ranges: `thickness` extrapolates, `thickness_checked` refuses
//!
//! [`thickness`](OxidationKinetics::thickness) evaluates the correlation
//! wherever it is asked, matching upstream, which enforces nothing.
//! [`thickness_checked`](OxidationKinetics::thickness_checked) returns
//! [`OffbeatError::OutOfRange`] outside the variant's stated temperature range,
//! [`OffbeatError::Unphysical`] for a negative thickness, timestep or flux, and
//! — uniquely — also refuses the 1800–1900 K window of
//! [`CathcartPawel`](OxidationKinetics::CathcartPawel), because upstream's
//! expression there is arithmetically broken. See that variant's documentation.
//!
//! # Reference
//!
//! Upstream attributes the constants of both correlations to:
//!
//! > Dunbar et al., *Fuel performance analysis of Cr-coated Zircaloy-4 cladding
//! > during a prototypical LOCA event using BISON*, Annals of Nuclear Energy
//! > **200** (2024) 110411. <https://doi.org/10.1016/j.anucene.2024.110411>
//!
//! This port has **not** independently checked the constants against that
//! paper; it reproduces the values in upstream's source. Do not cite it as
//! agreement with the reference.
//!
//! [`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
//! [`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical

// NaN-safe guards. Throughout this module a rejection test is written
// `!(x > 0.0)` rather than `x <= 0.0`, deliberately: the negated form is TRUE
// for NaN, so one comparison rejects negatives, zero and NaN together. Clippy's
// `neg_cmp_op_on_partial_ord` suggests the positive form, which would let a NaN
// through and propagate it into a physical result. The idiom is intentional.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::error::{OffbeatError, Result};

/// Seconds in a day — upstream's `3600.0*24.0`, used to convert this port's SI
/// timestep into the per-day basis of the EPRI/KWU/C-E rate constants.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// Conversion from SI fast flux \[n/(m²·s)\] to the \[n/(cm²·s)\] basis the
/// EPRI/KWU/C-E flux enhancement was fitted on. `1 n/m²/s = 1e-4 n/cm²/s`.
const FLUX_SI_TO_PER_CM2: f64 = 1.0e-4;

/// Oxide thickness \[m\] at which sub-transition (cubic) kinetics give way to
/// post-transition (linear) kinetics — upstream's `S_trans = 2e-6`.
///
/// Physically this is where the dense, protective oxide cracks and stops being
/// an effective diffusion barrier. It is a fitted constant, not a measured
/// property of a particular rod.
pub const TRANSITION_THICKNESS: f64 = 2.0e-6;

/// Cubic rate constant \[m³/day\] of the EPRI/KWU/C-E sub-transition law —
/// upstream's `C1 = 6.3e9*1e-18`.
const EPRI_C1: f64 = 6.3e9 * 1.0e-18;

/// Flux-independent term \[m/day\] of the EPRI/KWU/C-E post-transition rate
/// constant — upstream's `8.04e7*1e-6`.
const EPRI_C2_BASE: f64 = 8.04e7 * 1.0e-6;

/// Flux-enhanced term \[m/day\] of the EPRI/KWU/C-E post-transition rate
/// constant — upstream's `2.59e8*1e-6`, multiplied by `(7.46e-15*phi)^0.25`.
const EPRI_C2_FLUX: f64 = 2.59e8 * 1.0e-6;

/// Scaling of the fast flux inside the EPRI/KWU/C-E quarter-power flux term
/// \[cm²·s/n\] — upstream's `7.46e-15`, applied to a flux in n/(cm²·s).
const EPRI_FLUX_SCALE: f64 = 7.46e-15;

/// Activation temperature `Q1/R` \[K\] of the sub-transition law —
/// upstream's `32324 / 1.9872`, i.e. 32324 cal/mol divided by the gas constant
/// expressed as 1.9872 cal/(mol·K). Numerically 16266.10 K.
const EPRI_Q1_OVER_R: f64 = 32324.0 / 1.9872;

/// Activation temperature `Q2/R` \[K\] of the post-transition law —
/// upstream's `27374 / 1.9872`. Numerically 13775.16 K. Lower than
/// [`EPRI_Q1_OVER_R`], so the post-transition branch is less
/// temperature-sensitive than the sub-transition branch.
const EPRI_Q2_OVER_R: f64 = 27374.0 / 1.9872;

/// Pre-exponential \[m²/s\] of the Leistikow branch of upstream's
/// `CathcartPawel`, used below 1800 K.
const LEISTIKOW_A: f64 = 7.82e-6;

/// Activation temperature \[K\] of the Leistikow branch.
const LEISTIKOW_Q_OVER_R: f64 = 20214.0;

/// Pre-exponential \[m²/s\] of the Prater–Courtright branch, used at and above
/// 1900 K.
const PRATER_COURTRIGHT_A: f64 = 2.98e-3;

/// Activation temperature \[K\] of the Prater–Courtright branch.
const PRATER_COURTRIGHT_Q_OVER_R: f64 = 28420.0;

/// Lower edge \[K\] of upstream's Schanz interpolation window between the
/// Leistikow and Prater–Courtright branches.
const SCHANZ_LOW: f64 = 1800.0;

/// Upper edge \[K\] of upstream's Schanz interpolation window.
const SCHANZ_HIGH: f64 = 1900.0;

/// Temperature \[K\] at which the combined low/high model hands over from
/// EPRI/KWU/C-E to Cathcart–Pawel — upstream's `EPRI_KWU_CE::upperLimit()`.
pub const LOW_HIGH_SWITCH_TEMPERATURE: f64 = 673.0;

/// Oxide-growth kinetics for Zircaloy in water or steam.
///
/// One variant per oxidation-kinetics model compiled by upstream OFFBEAT's
/// `oxidationKineticsModel/`. Each variant's documentation names the upstream
/// class and the string a user writes in the `oxidationKineticsModel` entry of
/// a case dictionary, so an OFFBEAT case can be translated variant by variant.
///
/// Dispatch is by `match`, never by a trait object, per the workspace
/// `CLAUDE.md` "No trait objects" rule.
///
/// # All variants are monotonic and start from `S0`
///
/// Every branch has the form "new thickness = f(old thickness, T, dt)" with
/// `f >= S0` for `dt >= 0`. Oxide does not un-grow; a caller integrating a
/// power history can chain [`thickness`](Self::thickness) step by step and the
/// result is monotonically non-decreasing. The unit tests assert this.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OxidationKinetics {
    /// Low-temperature (normal-operation) waterside oxidation — upstream
    /// `EPRI_KWU_CE`, `ClassName("EPRI-KWU-CE")`.
    ///
    /// The industry-standard two-regime LWR corrosion law.
    ///
    /// **Sub-transition (`S <= 2 µm`), cubic:**
    ///
    /// `S = (C1 · exp(−Q1/(R·T)) · dt_days + S0³)^(1/3)`
    ///
    /// with `C1 = 6.3e-9` m³/day and `Q1/R = 16266.10` K. Cubic in thickness
    /// means the *rate* falls as `1/S²`: the layer protects itself.
    ///
    /// **Post-transition (`S > 2 µm`), linear:**
    ///
    /// `S = S0 + C2(φ) · exp(−Q2/(R·T)) · dt_days`
    ///
    /// with `Q2/R = 13775.16` K and a flux-enhanced rate constant
    ///
    /// `C2(φ) = (80.4 + 259 · (7.46e-15 · φ_cm)^(1/4))` m/day,
    ///
    /// `φ_cm` being the fast flux in n/(cm²·s) — this port converts from the
    /// SI n/(m²·s) it takes. The quarter-power dependence is the empirical
    /// signature of irradiation-enhanced corrosion: fast neutrons damage the
    /// oxide and speed up transport through it. At zero flux `C2 = 80.4` m/day;
    /// at a typical PWR fast flux of `7e17` n/(m²·s) it is `300.57` m/day, i.e.
    /// irradiation multiplies the post-transition rate by 3.74.
    ///
    /// **Crossing the transition inside one step.** If `S0` is below 2 µm but
    /// the cubic law would carry it past, upstream splits the step: it finds
    /// the fraction of `dt` needed to reach exactly 2 µm by **linear
    /// interpolation** between `S0` and the cubic end-point, then applies the
    /// post-transition law for whatever is left. That interpolation is an
    /// approximation (the underlying law is cubic, not linear, in time), and it
    /// is reproduced here exactly. The resulting thickness is nevertheless a
    /// *continuous* function of `S0` and `dt` across the transition — the
    /// tests check this to 1e-12 relative.
    ///
    /// # Branch selection differs subtly from upstream, deliberately
    ///
    /// Upstream selects its branch on the **current outer-iteration estimate**
    /// of `S`, which it receives by non-`const` reference, not on `S0`. That
    /// makes its answer depend on solver iteration state, which a pure function
    /// cannot have. This port instead evaluates the branch that upstream's
    /// iteration **converges to**, which is well defined:
    ///
    /// - `S0 >= 2 µm` → post-transition;
    /// - else if the cubic result is `<= 2 µm` → sub-transition;
    /// - else → the crossing branch.
    ///
    /// This is not an approximation of upstream: it is upstream's fixed point,
    /// reached on its second outer iteration in every case. It differs from
    /// upstream only if a run stops after a single outer iteration of a step
    /// that crosses the transition.
    ///
    /// # Validity
    ///
    /// Upstream declares `lowerLimit() = 500` K and `upperLimit() = 673` K.
    /// Note that upstream's own combined model **never consults the lower
    /// limit** — see [`EpriKwuCeCathcartPawel`](Self::EpriKwuCeCathcartPawel) —
    /// so below 500 K upstream silently extrapolates. This port's
    /// [`thickness_checked`](Self::thickness_checked) enforces 500–673 K.
    EpriKwuCe,

    /// High-temperature (accident) steam oxidation — upstream `CathcartPawel`,
    /// `ClassName("Cathcart-Pawel")`.
    ///
    /// A **parabolic** law, appropriate above ~673 K where the oxide is no
    /// longer protective and growth is limited by oxygen diffusion:
    ///
    /// `S = sqrt(A · exp(−Q/(R·T)) · dt + S0²)`
    ///
    /// with `dt` in **seconds** (unlike
    /// [`EpriKwuCe`](Self::EpriKwuCe), which works in days) and three
    /// temperature branches:
    ///
    /// | Range | `A` \[m²/s\] | `Q/R` \[K\] | Source named upstream |
    /// |---|---|---|---|
    /// | `T < 1800 K` | `7.82e-6` | `20214` | Leistikow |
    /// | `1800 <= T < 1900 K` | interpolated | interpolated | "Procedure from G. Schanz — 2003" |
    /// | `T >= 1900 K` | `2.98e-3` | `28420` | Prater–Courtright |
    ///
    /// The 1800–1900 K window exists because Zircaloy undergoes a phase change
    /// there and neither fit applies; Schanz's procedure is to fit a single
    /// Arrhenius law through the two branch values at the window edges.
    ///
    /// Note that upstream names the class `CathcartPawel` but the constants it
    /// actually contains are attributed in its own comments to Leistikow and
    /// Prater–Courtright, which are different correlations. This port keeps
    /// upstream's class name for traceability and flags the mismatch rather
    /// than silently renaming it.
    ///
    /// # UPSTREAM DEFECT in the 1800–1900 K window, reproduced deliberately
    ///
    /// Upstream's interpolation branch is arithmetically broken, in two
    /// independent ways, and this port reproduces it verbatim so that a
    /// comparison against an OFFBEAT run is possible. **The values it produces
    /// are not physical and must not be used.**
    ///
    /// 1. **Missing parentheses.** Upstream writes
    ///    `log(k2 / 7.82e-6 * exp(-20214/1800))` where
    ///    `log(k2 / (7.82e-6 * exp(-20214/1800)))` was intended: C++ evaluates
    ///    `a / b * c` as `(a/b)*c`, so the Leistikow exponential is
    ///    **multiplied** instead of divided. The activation temperature comes
    ///    out as `−692375.6` K instead of `+75756.4` K — negative, so the rate
    ///    *decreases* with temperature inside the window, the opposite of an
    ///    Arrhenius law.
    /// 2. **Wrong pre-exponential.** Even with the parentheses fixed, upstream
    ///    forms `A = 7.82e-6 · exp(Q/R/1900)`, using the bare Leistikow
    ///    pre-exponential where the Leistikow *rate constant* at 1800 K
    ///    (`1.0377e-10` m²/s) belongs. That is a further factor of 8224.7.
    ///
    /// Measured consequence (this port, 2026-07-29): at 1850 K the effective
    /// rate constant is `1.4809e-1` m²/s, against `2.9e-10` m²/s for a sane
    /// interpolation between the two branches — about nine orders of magnitude
    /// too large. Starting from bare metal, one second of it gives 0.38 **m**
    /// of oxide.
    ///
    /// [`thickness`](Self::thickness) reproduces this. **[`thickness_checked`]
    /// refuses it** with [`OffbeatError::Unphysical`], and a unit test pins the
    /// defective numbers so that anyone fixing it upstream is forced to notice.
    ///
    /// # Validity
    ///
    /// Upstream declares `lowerLimit() = 673` K and `upperLimit() = GREAT`
    /// (unbounded). This port's checked path enforces 673 K to 2500 K — above
    /// roughly 2245 K the ZrO2 itself melts and no solid-layer growth law
    /// applies — and additionally rejects the 1800–1900 K window as described
    /// above.
    ///
    /// [`thickness_checked`]: Self::thickness_checked
    /// [`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical
    CathcartPawel,

    /// The combined operational-plus-accident model — upstream
    /// `lowHighOxidationKineticsModel<EPRI_KWU_CE, CathcartPawel>`, registered
    /// as `"EPRI-KWU-CE|Cathcart-Pawel"`.
    ///
    /// This is the variant a realistic LWR case selects, and the only
    /// instantiation of the templated low/high model that upstream actually
    /// compiles. It applies [`EpriKwuCe`](Self::EpriKwuCe) below
    /// [`LOW_HIGH_SWITCH_TEMPERATURE`] (673 K) and
    /// [`CathcartPawel`](Self::CathcartPawel) at or above it, per interface
    /// temperature, so one rod can be in normal-operation kinetics at its
    /// cold end and accident kinetics at a hot spot.
    ///
    /// # The switch is a jump, not a blend
    ///
    /// The two laws are independent fits with different functional forms
    /// (cubic/linear against parabolic), and upstream makes **no attempt** to
    /// match them at 673 K. There is a genuine discontinuity in the growth rate
    /// there. Measured at 673 K over a 1 s step from a 2 µm layer, with zero
    /// flux (this port, 2026-07-29): the EPRI/KWU/C-E branch grows
    /// `1.2008e-12` m and the Cathcart–Pawel branch `1.7653e-13` m — the rate
    /// **drops by a factor of 6.80** on crossing into the "accident" model.
    /// That is the opposite of the naive expectation, and it happens because
    /// the parabolic law is strongly self-limiting once a 2 µm layer already
    /// exists (its rate goes as `1/S`), whereas the post-transition linear law
    /// does not slow down at all. A unit test pins that ratio; it **documents**
    /// the discontinuity rather than endorsing it, and a caller crossing 673 K
    /// slowly should expect a visible kink in the oxide history.
    ///
    /// # Upstream never uses `EPRI_KWU_CE::lowerLimit()`
    ///
    /// Upstream's dispatcher tests only `T < lowTModel.upperLimit()`. The
    /// declared lower limit of 500 K is dead code, so an OFFBEAT run at, say,
    /// 400 K quietly extrapolates the low-temperature fit. This port's
    /// [`thickness`](Self::thickness) does the same for fidelity;
    /// [`thickness_checked`](Self::thickness_checked) enforces 500 K.
    EpriKwuCeCathcartPawel,
}

impl OxidationKinetics {
    /// Oxide thickness \[m\] at the end of a timestep.
    ///
    /// # Parameters
    ///
    /// - `previous_thickness` — oxide thickness `S0` \[m\] at the start of the
    ///   step. Zero for fresh cladding. Negative values are treated as zero.
    /// - `interface_temperature` — metal/oxide interface temperature \[K\].
    ///   Must be strictly positive; see [`super::thermal`] for how to obtain
    ///   it from a surface temperature and an oxide thickness.
    /// - `fast_flux` — fast-neutron flux \[n/(m²·s)\]. Used only by the
    ///   post-transition branch of [`EpriKwuCe`](Self::EpriKwuCe); ignored by
    ///   [`CathcartPawel`](Self::CathcartPawel). Negative values are treated as
    ///   zero.
    /// - `time_step` — step length `dt` \[s\]. Negative values are treated as
    ///   zero.
    ///
    /// # Behaviour outside the validity range
    ///
    /// **Extrapolates, matching upstream** — no clamping. Use
    /// [`thickness_checked`](Self::thickness_checked) when the inputs may be
    /// out of range and you need to know. A non-positive temperature returns
    /// `previous_thickness` unchanged rather than producing a `NaN`; that is a
    /// guard this port adds, not upstream behaviour.
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::OxidationKinetics;
    ///
    /// let model = OxidationKinetics::EpriKwuCe;
    ///
    /// // Fresh cladding, one day at 600 K with no fast flux.
    /// let s = model.thickness(0.0, 600.0, 0.0, 86_400.0);
    /// assert!(s > 0.0 && s < 2.0e-6, "still sub-transition: {s} m");
    ///
    /// // Zero elapsed time cannot grow oxide.
    /// assert_eq!(model.thickness(1.0e-6, 600.0, 0.0, 0.0), 1.0e-6);
    /// ```
    #[must_use]
    pub fn thickness(
        &self,
        previous_thickness: f64,
        interface_temperature: f64,
        fast_flux: f64,
        time_step: f64,
    ) -> f64 {
        let s0 = previous_thickness.max(0.0);
        let dt = time_step.max(0.0);
        let flux = fast_flux.max(0.0);
        if !(interface_temperature > 0.0) || !s0.is_finite() || !dt.is_finite() {
            return s0;
        }
        match self {
            Self::EpriKwuCe => epri_kwu_ce(s0, interface_temperature, flux, dt),
            Self::CathcartPawel => cathcart_pawel(s0, interface_temperature, dt),
            Self::EpriKwuCeCathcartPawel => {
                if interface_temperature < LOW_HIGH_SWITCH_TEMPERATURE {
                    epri_kwu_ce(s0, interface_temperature, flux, dt)
                } else {
                    cathcart_pawel(s0, interface_temperature, dt)
                }
            }
        }
    }

    /// Increase in oxide thickness \[m\] over the step, i.e.
    /// [`thickness`](Self::thickness) minus `previous_thickness`.
    ///
    /// This is upstream's `DOxideThickness`. It is what the hydrogen-pickup
    /// model consumes ([`super::hydrogen`]) and what the metal-loss
    /// calculation divides by the Pilling–Bedworth ratio. Always `>= 0`.
    ///
    /// Same parameters and same out-of-range behaviour as
    /// [`thickness`](Self::thickness).
    #[must_use]
    pub fn growth(
        &self,
        previous_thickness: f64,
        interface_temperature: f64,
        fast_flux: f64,
        time_step: f64,
    ) -> f64 {
        let s0 = previous_thickness.max(0.0);
        self.thickness(s0, interface_temperature, fast_flux, time_step) - s0
    }

    /// **Mean** oxide growth rate \[m/s\] over the step — the increment divided
    /// by `time_step`.
    ///
    /// Not the instantaneous rate: the sub-transition law is cubic, so the rate
    /// falls across a long step and this returns the average. Returns `0.0` for
    /// a zero or negative `time_step`.
    ///
    /// Same parameters and same out-of-range behaviour as
    /// [`thickness`](Self::thickness).
    #[must_use]
    pub fn growth_rate(
        &self,
        previous_thickness: f64,
        interface_temperature: f64,
        fast_flux: f64,
        time_step: f64,
    ) -> f64 {
        if !(time_step > 0.0) {
            return 0.0;
        }
        self.growth(
            previous_thickness,
            interface_temperature,
            fast_flux,
            time_step,
        ) / time_step
    }

    /// [`thickness`](Self::thickness), but returning an error instead of
    /// extrapolating.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] for a non-positive interface temperature,
    ///   a negative previous thickness, a negative timestep, or a negative fast
    ///   flux — none of which has a meaning.
    /// - [`OffbeatError::Unphysical`] when
    ///   [`CathcartPawel`](Self::CathcartPawel) would be evaluated in the
    ///   1800–1900 K window, whose upstream expression is arithmetically
    ///   broken. See that variant's documentation for the measured numbers.
    /// - [`OffbeatError::OutOfRange`] when the interface temperature lies
    ///   outside the variant's stated validity range: 500–673 K for
    ///   [`EpriKwuCe`](Self::EpriKwuCe), 673–2500 K for
    ///   [`CathcartPawel`](Self::CathcartPawel), 500–2500 K for the combined
    ///   model.
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::OxidationKinetics;
    ///
    /// let model = OxidationKinetics::EpriKwuCe;
    /// // 900 K is far above the low-temperature fit's 673 K upper limit.
    /// assert!(model.thickness_checked(0.0, 900.0, 0.0, 3600.0).is_err());
    /// // ... but the unchecked path still returns a number, as upstream does.
    /// assert!(model.thickness(0.0, 900.0, 0.0, 3600.0).is_finite());
    /// ```
    ///
    /// [`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical
    /// [`OffbeatError::OutOfRange`]: crate::error::OffbeatError::OutOfRange
    pub fn thickness_checked(
        &self,
        previous_thickness: f64,
        interface_temperature: f64,
        fast_flux: f64,
        time_step: f64,
    ) -> Result<f64> {
        if !(interface_temperature > 0.0) {
            return Err(OffbeatError::Unphysical {
                quantity: "oxidation kinetics interface temperature",
                value: interface_temperature,
                unit: "K",
                reason: "absolute temperature must be strictly positive",
            });
        }
        if previous_thickness < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "oxide thickness",
                value: previous_thickness,
                unit: "m",
                reason: "an oxide layer cannot have negative thickness",
            });
        }
        if time_step < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "oxidation timestep",
                value: time_step,
                unit: "s",
                reason: "oxide growth is not integrated backwards in time",
            });
        }
        if fast_flux < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "fast-neutron flux",
                value: fast_flux,
                unit: "n/(m^2 s)",
                reason: "a neutron flux cannot be negative",
            });
        }

        let (low, high) = self.temperature_range();
        if interface_temperature < low || interface_temperature > high {
            return Err(OffbeatError::OutOfRange {
                quantity: self.name(),
                value: interface_temperature,
                low,
                high,
                unit: "K",
            });
        }

        // The Cathcart-Pawel interpolation window is not merely extrapolation:
        // upstream's expression there is arithmetically broken (see the variant
        // documentation), so refuse it outright on the checked path.
        let uses_high_temperature_branch = match self {
            Self::CathcartPawel => true,
            Self::EpriKwuCeCathcartPawel => interface_temperature >= LOW_HIGH_SWITCH_TEMPERATURE,
            Self::EpriKwuCe => false,
        };
        if uses_high_temperature_branch
            && (SCHANZ_LOW..SCHANZ_HIGH).contains(&interface_temperature)
        {
            return Err(OffbeatError::Unphysical {
                quantity: "Cathcart-Pawel Schanz interpolation window",
                value: interface_temperature,
                unit: "K",
                reason: "upstream's 1800-1900 K interpolation has a sign-inverted \
                         activation energy and a pre-exponential wrong by ~8225x",
            });
        }

        Ok(self.thickness(
            previous_thickness,
            interface_temperature,
            fast_flux,
            time_step,
        ))
    }

    /// [`growth`](Self::growth), but returning an error instead of
    /// extrapolating.
    ///
    /// # Errors
    ///
    /// The same errors as [`thickness_checked`](Self::thickness_checked).
    pub fn growth_checked(
        &self,
        previous_thickness: f64,
        interface_temperature: f64,
        fast_flux: f64,
        time_step: f64,
    ) -> Result<f64> {
        let s = self.thickness_checked(
            previous_thickness,
            interface_temperature,
            fast_flux,
            time_step,
        )?;
        Ok(s - previous_thickness.max(0.0))
    }

    /// Human-readable name of this correlation, for error messages.
    fn name(&self) -> &'static str {
        match self {
            Self::EpriKwuCe => "EPRI/KWU/C-E oxidation kinetics",
            Self::CathcartPawel => "Cathcart-Pawel oxidation kinetics",
            Self::EpriKwuCeCathcartPawel => "EPRI/KWU/C-E | Cathcart-Pawel oxidation kinetics",
        }
    }

    /// `(low, high)` interface-temperature validity range \[K\].
    ///
    /// The low limits are upstream's declared `lowerLimit()`; the 2500 K upper
    /// bound on the high-temperature branch is this port's, because upstream
    /// declares `GREAT` and a solid ZrO2 growth law cannot survive the oxide's
    /// own melting point (~2245 K).
    fn temperature_range(&self) -> (f64, f64) {
        match self {
            Self::EpriKwuCe => (500.0, 673.0),
            Self::CathcartPawel => (673.0, 2500.0),
            Self::EpriKwuCeCathcartPawel => (500.0, 2500.0),
        }
    }
}

/// EPRI/KWU/C-E low-temperature oxide thickness \[m\] after `dt` \[s\].
///
/// Faithful translation of upstream `EPRI_KWU_CE::correctOxideThickness`, with
/// the branch chosen at upstream's converged fixed point rather than from
/// solver iteration state — see
/// [`OxidationKinetics::EpriKwuCe`] for why, and why the two agree.
///
/// `s0` \[m\], `temperature` \[K\], `flux` \[n/(m²·s)\], `dt` \[s\]; all
/// assumed already non-negative and finite, and `temperature > 0`.
fn epri_kwu_ce(s0: f64, temperature: f64, flux: f64, dt: f64) -> f64 {
    let dt_days = dt / SECONDS_PER_DAY;

    // Post-transition rate constant [m/day], flux-enhanced. Upstream's `phi` is
    // in n/(cm^2 s); this port takes SI and converts here.
    let flux_per_cm2 = flux * FLUX_SI_TO_PER_CM2;
    let c2 = EPRI_C2_BASE + EPRI_C2_FLUX * (EPRI_FLUX_SCALE * flux_per_cm2).powf(0.25);

    let post_rate = c2 * (-EPRI_Q2_OVER_R / temperature).exp();

    if s0 >= TRANSITION_THICKNESS {
        // Already cracked: linear growth.
        return s0 + post_rate * dt_days;
    }

    // Sub-transition cubic law, evaluated over the whole step.
    let cubic_gain = EPRI_C1 * (-EPRI_Q1_OVER_R / temperature).exp() * dt_days;
    let s_pre = (cubic_gain + s0 * s0 * s0).cbrt();

    if s_pre <= TRANSITION_THICKNESS {
        return s_pre;
    }

    // The step crosses the transition. Upstream estimates the time to reach
    // S_trans by LINEAR interpolation between s0 and the cubic end-point, then
    // spends the remainder of the step on the post-transition law.
    let dt_pre = (TRANSITION_THICKNESS - s0) / (s_pre - s0) * dt;
    let dt_post_days = (dt - dt_pre) / SECONDS_PER_DAY;
    TRANSITION_THICKNESS + post_rate * dt_post_days
}

/// Cathcart–Pawel high-temperature oxide thickness \[m\] after `dt` \[s\].
///
/// Faithful translation of upstream `CathcartPawel::correctOxideThickness`,
/// **including the broken 1800–1900 K interpolation** — see
/// [`OxidationKinetics::CathcartPawel`].
///
/// `s0` \[m\], `temperature` \[K\], `dt` \[s\]; all assumed already
/// non-negative and finite, and `temperature > 0`.
fn cathcart_pawel(s0: f64, temperature: f64, dt: f64) -> f64 {
    let (a, q_over_r) = cathcart_pawel_arrhenius(temperature);
    let gain = a * (-q_over_r / temperature).exp() * dt;
    let squared = gain + s0 * s0;
    if squared <= 0.0 || !squared.is_finite() {
        return s0;
    }
    squared.sqrt()
}

/// The `(A, Q/R)` pair \[m²/s\], \[K\] of upstream's `CathcartPawel` at
/// `temperature` \[K\].
///
/// Three branches: Leistikow below 1800 K, Prater–Courtright at and above
/// 1900 K, and upstream's **defective** Schanz interpolation in between.
///
/// The interpolation branch is a compile-time constant expression in upstream,
/// so it is one here too. Its measured values (this port, 2026-07-29) are
/// `Q/R = −692375.60` K and `A = 4.2927e-164` m²/s, against the `+75756.40` K
/// and `1.9687e8` m²/s a correct Schanz interpolation of the same two branches
/// would give. Reproduced deliberately; see
/// [`OxidationKinetics::CathcartPawel`].
fn cathcart_pawel_arrhenius(temperature: f64) -> (f64, f64) {
    if temperature < SCHANZ_LOW {
        (LEISTIKOW_A, LEISTIKOW_Q_OVER_R)
    } else if temperature < SCHANZ_HIGH {
        // Upstream, verbatim:
        //   QsR = 1900*1800/(1900-1800)
        //       * log( 2.98e-3*exp(-28420/1900) / 7.82e-6 * exp(-20214/1800) );
        //   As  = 7.82e-6*exp(QsR/1900);
        // The `/ 7.82e-6 * exp(...)` is `(a/b)*c` in C++, not `a/(b*c)`.
        let ratio = PRATER_COURTRIGHT_A * (-PRATER_COURTRIGHT_Q_OVER_R / SCHANZ_HIGH).exp()
            / LEISTIKOW_A
            * (-LEISTIKOW_Q_OVER_R / SCHANZ_LOW).exp();
        let q_over_r = SCHANZ_HIGH * SCHANZ_LOW / (SCHANZ_HIGH - SCHANZ_LOW) * ratio.ln();
        let a = LEISTIKOW_A * (q_over_r / SCHANZ_HIGH).exp();
        (a, q_over_r)
    } else {
        (PRATER_COURTRIGHT_A, PRATER_COURTRIGHT_Q_OVER_R)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One day, in seconds.
    const DAY: f64 = 86_400.0;

    /// A representative PWR fast flux \[n/(m²·s)\] — 7e13 n/(cm²·s).
    const PWR_FAST_FLUX: f64 = 7.0e17;

    /// Every variant, for sweeps that should hold for all of them.
    const ALL: [OxidationKinetics; 3] = [
        OxidationKinetics::EpriKwuCe,
        OxidationKinetics::CathcartPawel,
        OxidationKinetics::EpriKwuCeCathcartPawel,
    ];

    /// Self-consistency check, not validation: a corrosion law that produces
    /// oxide out of nothing is broken. Zero elapsed time must leave the layer
    /// exactly as it was, for every variant, at every temperature — including
    /// inside the defective Cathcart–Pawel window, where the rate constant is
    /// nine orders of magnitude too large but is still multiplied by `dt = 0`.
    #[test]
    fn zero_time_grows_no_oxide() {
        for model in ALL {
            for temperature in [520.0, 600.0, 672.9, 700.0, 1200.0, 1850.0, 2100.0] {
                for s0 in [0.0, 1.0e-7, 2.0e-6, 5.0e-5] {
                    let s = model.thickness(s0, temperature, PWR_FAST_FLUX, 0.0);
                    assert_eq!(
                        s, s0,
                        "{model:?} at {temperature} K grew oxide in zero time from {s0}"
                    );
                    assert_eq!(model.growth(s0, temperature, PWR_FAST_FLUX, 0.0), 0.0);
                    assert_eq!(model.growth_rate(s0, temperature, PWR_FAST_FLUX, 0.0), 0.0);
                }
            }
        }
    }

    /// Self-consistency check, not validation: starting from bare metal at
    /// `t = 0`, every law must give exactly zero, and any positive step must
    /// give something strictly positive.
    #[test]
    fn fresh_cladding_starts_at_zero_and_then_grows() {
        for model in ALL {
            let temperature = match model {
                OxidationKinetics::CathcartPawel => 1200.0,
                _ => 600.0,
            };
            assert_eq!(model.thickness(0.0, temperature, 0.0, 0.0), 0.0);
            let s = model.thickness(0.0, temperature, 0.0, DAY);
            assert!(
                s > 0.0,
                "{model:?} did not grow any oxide in a day at {temperature} K"
            );
        }
    }

    /// Self-consistency check, not validation: oxide does not un-grow. Chained
    /// step by step over a long history the thickness must be monotonically
    /// non-decreasing, and a longer single step must never give less than a
    /// shorter one.
    #[test]
    fn oxide_growth_is_monotonic_in_time() {
        for model in ALL {
            let temperature = match model {
                OxidationKinetics::CathcartPawel => 1000.0,
                _ => 620.0,
            };

            // Chained integration over 4000 days.
            let mut s = 0.0;
            for _ in 0..4000 {
                let next = model.thickness(s, temperature, PWR_FAST_FLUX, DAY);
                assert!(
                    next >= s,
                    "{model:?} shrank the oxide from {s} to {next} at {temperature} K"
                );
                assert!(next.is_finite(), "{model:?} produced {next}");
                s = next;
            }
            assert!(s > 0.0);

            // Longer single steps give thicker oxide.
            let mut previous = 0.0;
            for days in [1.0, 10.0, 100.0, 1000.0, 4000.0] {
                let s = model.thickness(0.0, temperature, PWR_FAST_FLUX, days * DAY);
                assert!(
                    s >= previous,
                    "{model:?}: {days} days gave {s} m, less than the shorter step's {previous} m"
                );
                previous = s;
            }
        }
    }

    /// Self-consistency check, not validation: corrosion is thermally
    /// activated, so at fixed thickness and fixed step every law must grow
    /// **strictly faster** as the interface gets hotter. This is the test that
    /// catches a sign error in an activation temperature.
    ///
    /// The 1800–1900 K window is deliberately skipped: upstream's expression
    /// there has a *negative* activation temperature and therefore violates
    /// this property. That defect is pinned by its own test below.
    #[test]
    fn growth_increases_with_temperature_arrhenius_sign() {
        // EPRI/KWU/C-E, both branches.
        for s0 in [0.0, 1.0e-6, 3.0e-6] {
            let mut previous = -1.0;
            for temperature in [500.0, 550.0, 600.0, 650.0, 673.0] {
                let g = OxidationKinetics::EpriKwuCe.growth(s0, temperature, 0.0, DAY);
                assert!(
                    g > previous,
                    "EPRI/KWU/C-E at {temperature} K grew {g} m, not more than {previous} m"
                );
                previous = g;
            }
        }

        // Cathcart-Pawel, avoiding the broken interpolation window.
        for s0 in [0.0, 1.0e-5] {
            let mut previous = -1.0;
            for temperature in [700.0, 900.0, 1200.0, 1500.0, 1799.0] {
                let g = OxidationKinetics::CathcartPawel.growth(s0, temperature, 0.0, 1.0);
                assert!(
                    g > previous,
                    "Cathcart-Pawel at {temperature} K grew {g} m, not more than {previous} m"
                );
                previous = g;
            }
            let mut previous = -1.0;
            for temperature in [1900.0, 2000.0, 2200.0] {
                let g = OxidationKinetics::CathcartPawel.growth(s0, temperature, 0.0, 1.0);
                assert!(g > previous);
                previous = g;
            }
        }
    }

    /// Self-consistency check against the closed form of upstream's algebra,
    /// not validation. Both EPRI/KWU/C-E branches are checked against their own
    /// analytic solutions to machine precision, which pins the seconds-to-days
    /// conversion, the rate constants, and the SI-to-per-cm² flux conversion.
    ///
    /// # Methodology
    ///
    /// - Sub-transition: from `S0 = 0`, one day at 600 K with zero flux must
    ///   give exactly `(C1 exp(−Q1/RT))^(1/3)` with `C1 = 6.3e-9` m³/day and
    ///   `Q1/R = 32324/1.9872` K.
    /// - Post-transition: from `S0 = 3 µm` (already past 2 µm), one day at
    ///   600 K must give exactly `S0 + C2(φ) exp(−Q2/RT)`.
    /// - Flux term: at `φ = 7e17` n/(m²·s) SI = `7e13` n/(cm²·s), `C2` must be
    ///   `80.4 + 259·(7.46e-15·7e13)^(1/4)`.
    /// - Tolerance: 1e-15 relative — round-off only.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// All three agree with the closed form to round-off. The absolute values
    /// are `S(1 day, 600 K, φ=0) = 2.19709e-7` m sub-transition;
    /// `C2(7e17 SI) = 300.570` m/day, giving a post-transition increment of
    /// `3.21479e-8` m/day at 600 K against `8.59928e-9` m/day at zero flux.
    #[test]
    fn epri_branches_match_their_closed_forms() {
        let model = OxidationKinetics::EpriKwuCe;
        let t = 600.0;

        // Sub-transition, from bare metal.
        let s = model.thickness(0.0, t, 0.0, DAY);
        let expected = (EPRI_C1 * (-EPRI_Q1_OVER_R / t).exp()).cbrt();
        assert!(s < TRANSITION_THICKNESS, "must stay sub-transition: {s}");
        assert!(
            (s / expected - 1.0).abs() < 1.0e-15,
            "sub-transition {s} != closed form {expected}"
        );
        assert!(
            (s - 2.197_088e-7).abs() < 1.0e-12,
            "recorded value drifted: {s}"
        );

        // Post-transition, zero flux.
        let s0 = 3.0e-6;
        let s = model.thickness(s0, t, 0.0, DAY);
        let expected = s0 + EPRI_C2_BASE * (-EPRI_Q2_OVER_R / t).exp();
        assert!(
            (s / expected - 1.0).abs() < 1.0e-15,
            "post-transition {s} != closed form {expected}"
        );

        // Post-transition, PWR flux — pins the SI -> per-cm^2 conversion.
        let s = model.thickness(s0, t, PWR_FAST_FLUX, DAY);
        let c2 = EPRI_C2_BASE + EPRI_C2_FLUX * (EPRI_FLUX_SCALE * 7.0e13).powf(0.25);
        let expected = s0 + c2 * (-EPRI_Q2_OVER_R / t).exp();
        assert!(
            (s / expected - 1.0).abs() < 1.0e-15,
            "flux-enhanced {s} != closed form {expected}"
        );
        assert!(
            (c2 - 300.570).abs() < 0.01,
            "recorded C2 at PWR flux drifted: {c2} m/day"
        );
        // Irradiation enhancement, recorded: 3.74x over zero flux.
        assert!((c2 / EPRI_C2_BASE - 3.7384).abs() < 0.001);
    }

    /// Self-consistency check, not validation: the physical point of the
    /// transition is that growth **accelerates**. Just past 2 µm the linear
    /// post-transition rate must exceed the cubic sub-transition rate at the
    /// same thickness and temperature.
    ///
    /// # Results (measured 2026-07-29, this port, zero flux)
    ///
    /// Ratio of post-transition to sub-transition instantaneous rate at
    /// `S = 2 µm`: **14.19** at 550 K, **9.73** at 600 K, **7.07** at 650 K.
    /// The ratio falls with temperature because the sub-transition law has the
    /// larger activation temperature (16266 K against 13775 K), so the two
    /// branches converge as the metal gets hotter — a real feature of the fit,
    /// not a port artefact.
    #[test]
    fn post_transition_growth_is_faster_than_sub_transition() {
        for (temperature, expected_ratio) in [(550.0, 14.19), (600.0, 9.73), (650.0, 7.07)] {
            // Instantaneous sub-transition rate at S_trans: d/dt of the cubic law.
            let sub = EPRI_C1 * (-EPRI_Q1_OVER_R / temperature).exp()
                / (3.0 * TRANSITION_THICKNESS * TRANSITION_THICKNESS);
            let post = EPRI_C2_BASE * (-EPRI_Q2_OVER_R / temperature).exp();
            let ratio = post / sub;
            assert!(
                ratio > 1.0,
                "post-transition must be faster at {temperature} K, got {ratio}"
            );
            assert!(
                (ratio - expected_ratio).abs() < 0.01,
                "recorded ratio at {temperature} K drifted: {ratio} vs {expected_ratio}"
            );
        }
    }

    /// **Reference-checked against upstream's own construction**, not against
    /// measured data: upstream's crossing branch is written so that the
    /// pre- and post-transition laws meet *at* the transition thickness. This
    /// test establishes that this port's branch selection reproduces that
    /// continuity — and it is the check that would fail if the crossing branch
    /// were mis-transcribed.
    ///
    /// # Methodology
    ///
    /// - Inputs: `T = 600` K, `dt = 1` day, zero flux; `S0 = 2 µm ± ε` for
    ///   `ε` in `{1e-12, 1e-10, 1e-9}` m.
    /// - Reference: the exact one-sided limits agree, so the jump must vanish
    ///   linearly in `ε`. Analytically the difference is
    ///   `ε·(1 + r_post/r_pre)` where `r_post/r_pre` is the rate ratio of the
    ///   previous test, 9.73 at 600 K — so the predicted slope is 10.73.
    /// - Pass criterion: `|S(2 µm + ε) − S(2 µm − ε)| < 11·ε`.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// | ε \[m\] | jump \[m\] | jump/ε |
    /// |---|---|---|
    /// | 1e-12 | 1.073e-11 | 10.73 |
    /// | 1e-10 | 1.073e-09 | 10.73 |
    /// | 1e-09 | 9.715e-09 | 9.72 |
    ///
    /// The ratio matches the predicted 10.73 to three significant figures and
    /// the jump goes to zero with `ε`. **The oxide thickness is therefore
    /// continuous across the transition; only its slope is not.** The kink is
    /// physical — that is what "transition" means — and it is not a
    /// discontinuity to be pinned as a defect.
    #[test]
    fn the_two_epri_branches_meet_continuously_at_the_transition() {
        let model = OxidationKinetics::EpriKwuCe;
        let t = 600.0;
        for epsilon in [1.0e-12, 1.0e-10, 1.0e-9] {
            let below = model.thickness(TRANSITION_THICKNESS - epsilon, t, 0.0, DAY);
            let above = model.thickness(TRANSITION_THICKNESS + epsilon, t, 0.0, DAY);
            let jump = (above - below).abs();
            assert!(
                jump < 11.0 * epsilon,
                "jump {jump:e} at eps {epsilon:e} exceeds the predicted 10.73*eps"
            );
        }

        // The crossing branch reduces to the pure sub-transition branch when
        // the step ends exactly at the transition, and to the pure
        // post-transition branch when it starts there.
        let at_transition = model.thickness(TRANSITION_THICKNESS, t, 0.0, DAY);
        let pure_post = TRANSITION_THICKNESS + EPRI_C2_BASE * (-EPRI_Q2_OVER_R / t).exp();
        assert!((at_transition / pure_post - 1.0).abs() < 1.0e-15);
    }

    /// **Documents a discontinuity in upstream that this port reproduces
    /// deliberately.** The combined model switches from the EPRI/KWU/C-E fit to
    /// Cathcart–Pawel at exactly 673 K with no blending, so the growth rate
    /// jumps.
    ///
    /// # Methodology
    ///
    /// - Inputs: `S0 = 2 µm`, `dt = 1` s, zero flux, at 672.999 K and
    ///   673.0 K — either side of upstream's switch.
    /// - Reference: none exists; the two fits are independent. The test records
    ///   the measured ratio.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// EPRI/KWU/C-E gives `1.20078e-12` m in one second; Cathcart–Pawel gives
    /// `1.76533e-13` m. The rate **falls by a factor of 6.80** at the switch,
    /// i.e. the ratio above/below is `0.14702`.
    ///
    /// # Interpretation
    ///
    /// The drop is real and comes from the functional forms: the
    /// post-transition EPRI law is *linear* in time and does not slow down as
    /// the layer thickens, whereas the parabolic Cathcart–Pawel law's rate goes
    /// as `1/S` and is already heavily self-limited by the 2 µm layer that
    /// exists at the switch. From bare metal the ordering reverses. This is
    /// upstream's behaviour, not a port artefact, and it is the reason an
    /// OFFBEAT oxide history shows a kink at 673 K. The test asserts the
    /// discontinuity so that anyone who smooths it — here or upstream — is
    /// forced to notice and to re-baseline the comparison. It **documents**
    /// the behaviour; it does not endorse it.
    #[test]
    fn the_low_high_switch_is_discontinuous_reproducing_upstream() {
        let combined = OxidationKinetics::EpriKwuCeCathcartPawel;
        let s0 = TRANSITION_THICKNESS;

        let below = combined.growth(s0, LOW_HIGH_SWITCH_TEMPERATURE - 1.0e-3, 0.0, 1.0);
        let above = combined.growth(s0, LOW_HIGH_SWITCH_TEMPERATURE, 0.0, 1.0);

        // The combined model really does dispatch to the two sub-models.
        assert_eq!(
            below,
            OxidationKinetics::EpriKwuCe.growth(s0, LOW_HIGH_SWITCH_TEMPERATURE - 1.0e-3, 0.0, 1.0)
        );
        assert_eq!(
            above,
            OxidationKinetics::CathcartPawel.growth(s0, LOW_HIGH_SWITCH_TEMPERATURE, 0.0, 1.0)
        );

        assert!(
            (below - 1.200_777e-12).abs() < 1.0e-17,
            "recorded EPRI growth at the switch drifted: {below}"
        );
        assert!(
            (above - 1.765_326e-13).abs() < 1.0e-18,
            "recorded Cathcart-Pawel growth at the switch drifted: {above}"
        );

        let ratio = above / below;
        assert!(
            (ratio - 0.147_015).abs() < 1.0e-4,
            "recorded switch discontinuity drifted: {ratio}x (was 0.147015x)"
        );
        assert!(
            ratio < 1.0,
            "the rate falls across the switch, it does not rise"
        );
    }

    /// **Documents a defect in upstream OFFBEAT that this port reproduces
    /// deliberately.** Upstream's 1800–1900 K Schanz interpolation in
    /// `CathcartPawel.C` is arithmetically broken in two independent ways —
    /// a missing pair of parentheses and a wrong pre-exponential — and the
    /// resulting rate constant is unusable.
    ///
    /// # Methodology
    ///
    /// - Inputs: the interpolation branch's constants, evaluated exactly as
    ///   upstream's C++ evaluates them, and compared against the Leistikow and
    ///   Prater–Courtright branch values at the two window edges.
    /// - Reference: a *correct* Schanz interpolation must (a) have a positive
    ///   activation temperature and (b) reproduce the two branch rate
    ///   constants at 1800 K and 1900 K. Both are computable in closed form
    ///   from the same four upstream constants, so this is a genuine check, not
    ///   an invented number.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// | Quantity | upstream, as written | correct Schanz |
    /// |---|---|---|
    /// | `Q/R` \[K\] | `−692375.60` | `+75756.40` |
    /// | `A` \[m²/s\] | `4.2927e-164` | `1.9687e+08` |
    /// | rate constant at 1850 K \[m²/s\] | `1.4809e-01` | `≈2.9e-10` |
    ///
    /// The branch values it is supposed to interpolate between are
    /// `1.0377e-10` m²/s at 1800 K and `9.5079e-10` m²/s at 1900 K, so
    /// upstream is **nine orders of magnitude high** in the middle of the
    /// window, and its rate *falls* with temperature because `Q/R` is negative.
    /// One second at 1850 K from bare metal gives `0.385 m` of oxide.
    ///
    /// # Interpretation
    ///
    /// The window is unusable and this port refuses it on the checked path
    /// ([`OxidationKinetics::thickness_checked`] returns
    /// [`OffbeatError::Unphysical`](crate::error::OffbeatError::Unphysical)),
    /// while [`OxidationKinetics::thickness`] reproduces it so an OFFBEAT run
    /// can still be compared. This test asserts the *defective* numbers. It
    /// **documents** them; it does not endorse them, and it exists so that a
    /// fix upstream cannot land here unnoticed.
    #[test]
    fn cathcart_pawel_interpolation_window_is_broken_reproducing_upstream() {
        let (a, q_over_r) = cathcart_pawel_arrhenius(1850.0);

        // Defect 1: the activation temperature comes out NEGATIVE.
        assert!(
            q_over_r < 0.0,
            "upstream's interpolated Q/R should be negative: {q_over_r}"
        );
        assert!(
            (q_over_r / -692_375.604_346_975_7 - 1.0).abs() < 1.0e-12,
            "recorded upstream Q/R drifted: {q_over_r}"
        );
        assert!(
            (a / 4.292_702_265_773_73e-164 - 1.0).abs() < 1.0e-12,
            "recorded upstream A drifted: {a}"
        );

        // What a correct Schanz interpolation of the SAME four constants gives.
        let k_low = LEISTIKOW_A * (-LEISTIKOW_Q_OVER_R / SCHANZ_LOW).exp();
        let k_high = PRATER_COURTRIGHT_A * (-PRATER_COURTRIGHT_Q_OVER_R / SCHANZ_HIGH).exp();
        let correct_q =
            SCHANZ_HIGH * SCHANZ_LOW / (SCHANZ_HIGH - SCHANZ_LOW) * (k_high / k_low).ln();
        assert!(correct_q > 0.0);
        assert!((correct_q / 75_756.395_653_024_3 - 1.0).abs() < 1.0e-12);

        // Defect 2: even with the parentheses fixed, upstream's pre-exponential
        // uses the bare Leistikow prefactor instead of the rate constant.
        let correct_a = k_low * (correct_q / SCHANZ_LOW).exp();
        let upstream_style_a = LEISTIKOW_A * (correct_q / SCHANZ_HIGH).exp();
        assert!(
            (upstream_style_a / correct_a - 8224.7).abs() < 1.0,
            "recorded pre-exponential error drifted: {}x",
            upstream_style_a / correct_a
        );

        // The consequence: metre-scale oxide in one second.
        let s = OxidationKinetics::CathcartPawel.thickness(0.0, 1850.0, 0.0, 1.0);
        assert!(
            (s - 0.384_820_7).abs() < 1.0e-6,
            "recorded broken-window thickness drifted: {s} m in 1 s"
        );

        // ...and the checked path refuses it outright.
        assert!(matches!(
            OxidationKinetics::CathcartPawel.thickness_checked(0.0, 1850.0, 0.0, 1.0),
            Err(OffbeatError::Unphysical { .. })
        ));
        assert!(matches!(
            OxidationKinetics::EpriKwuCeCathcartPawel.thickness_checked(0.0, 1850.0, 0.0, 1.0),
            Err(OffbeatError::Unphysical { .. })
        ));
        // The window edges themselves are fine.
        assert!(OxidationKinetics::CathcartPawel
            .thickness_checked(0.0, 1799.9, 0.0, 1.0)
            .is_ok());
        assert!(OxidationKinetics::CathcartPawel
            .thickness_checked(0.0, 1900.0, 0.0, 1.0)
            .is_ok());
    }

    /// Self-consistency check, not validation: the Cathcart–Pawel law is
    /// parabolic, so `S² − S0²` must be exactly linear in `dt` and the
    /// thickness from bare metal must scale as `sqrt(dt)`.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// At 1473 K the effective rate constant is `8.57783e-12` m²/s, giving
    /// `92.617 µm` of oxide after 1000 s from bare metal. Doubling the time
    /// multiplies the thickness by `sqrt(2)` to within 1e-15 relative.
    #[test]
    fn cathcart_pawel_is_parabolic_in_time() {
        let model = OxidationKinetics::CathcartPawel;
        let t = 1473.0;

        let k = LEISTIKOW_A * (-LEISTIKOW_Q_OVER_R / t).exp();
        assert!(
            (k / 8.577_829e-12 - 1.0).abs() < 1.0e-6,
            "recorded k drifted: {k}"
        );

        let s1000 = model.thickness(0.0, t, 0.0, 1000.0);
        assert!(
            (s1000 / 9.261_657e-5 - 1.0).abs() < 1.0e-6,
            "recorded thickness drifted: {s1000} m"
        );

        // sqrt(dt) scaling from bare metal.
        let s2000 = model.thickness(0.0, t, 0.0, 2000.0);
        assert!((s2000 / s1000 / 2.0_f64.sqrt() - 1.0).abs() < 1.0e-15);

        // S^2 - S0^2 linear in dt, from a non-zero start.
        let s0 = 1.0e-5;
        for dt in [1.0, 10.0, 100.0, 1000.0] {
            let s = model.thickness(s0, t, 0.0, dt);
            let gain = s * s - s0 * s0;
            assert!((gain / (k * dt) - 1.0).abs() < 1.0e-12, "dt={dt}: {gain}");
        }
    }

    /// Self-consistency check, not validation: the flux enhancement must be
    /// monotone in flux, must vanish at zero flux, and must follow the stated
    /// quarter power exactly.
    #[test]
    fn flux_enhancement_is_monotone_and_quarter_power() {
        let model = OxidationKinetics::EpriKwuCe;
        let s0 = 3.0e-6; // post-transition, where flux matters
        let t = 620.0;

        let mut previous = model.growth(s0, t, 0.0, DAY);
        for flux in [1.0e15, 1.0e16, 1.0e17, 7.0e17, 1.0e18] {
            let g = model.growth(s0, t, flux, DAY);
            assert!(g > previous, "flux {flux:e} did not increase growth");
            previous = g;
        }

        // Sub-transition growth is flux-independent, by construction.
        let cold = model.growth(0.0, t, 0.0, DAY);
        let irradiated = model.growth(0.0, t, 1.0e18, DAY);
        assert_eq!(cold, irradiated);

        // Quarter power: multiplying the flux by 16 doubles the flux term.
        let term =
            |flux: f64| EPRI_C2_FLUX * (EPRI_FLUX_SCALE * flux * FLUX_SI_TO_PER_CM2).powf(0.25);
        assert!((term(1.6e18) / term(1.0e17) - 2.0).abs() < 1.0e-12);
        assert_eq!(term(0.0), 0.0);
    }

    /// The documented contract of the checked path: it refuses unphysical
    /// inputs and out-of-range temperatures, while the unchecked path
    /// extrapolates in the manner of upstream.
    #[test]
    fn checked_path_refuses_what_the_unchecked_path_extrapolates() {
        let model = OxidationKinetics::EpriKwuCe;

        // Out of range: EPRI's fit stops at 673 K.
        assert!(matches!(
            model.thickness_checked(0.0, 900.0, 0.0, DAY),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert!(matches!(
            model.thickness_checked(0.0, 400.0, 0.0, DAY),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert!(model.thickness(0.0, 900.0, 0.0, DAY).is_finite());

        // Unphysical inputs.
        for bad in [
            model.thickness_checked(0.0, -5.0, 0.0, DAY),
            model.thickness_checked(0.0, 0.0, 0.0, DAY),
            model.thickness_checked(-1.0e-6, 600.0, 0.0, DAY),
            model.thickness_checked(0.0, 600.0, 0.0, -DAY),
            model.thickness_checked(0.0, 600.0, -1.0e17, DAY),
        ] {
            assert!(
                matches!(bad, Err(OffbeatError::Unphysical { .. })),
                "{bad:?}"
            );
        }

        // A non-positive temperature returns the previous thickness rather
        // than a NaN on the unchecked path.
        assert_eq!(model.thickness(1.0e-6, -5.0, 0.0, DAY), 1.0e-6);
        assert_eq!(model.thickness(1.0e-6, 0.0, 0.0, DAY), 1.0e-6);

        // Negative inputs are floored, not propagated.
        assert_eq!(model.thickness(-1.0, 600.0, 0.0, 0.0), 0.0);
        assert_eq!(model.thickness(1.0e-6, 600.0, 0.0, -DAY), 1.0e-6);

        // growth_checked agrees with growth where both are defined.
        let g = model.growth_checked(1.0e-6, 600.0, 0.0, DAY).unwrap();
        assert_eq!(g, model.growth(1.0e-6, 600.0, 0.0, DAY));

        // The combined model accepts both regimes.
        let combined = OxidationKinetics::EpriKwuCeCathcartPawel;
        assert!(combined.thickness_checked(0.0, 600.0, 0.0, DAY).is_ok());
        assert!(combined.thickness_checked(0.0, 1200.0, 0.0, 1.0).is_ok());
        assert!(matches!(
            combined.thickness_checked(0.0, 3000.0, 0.0, 1.0),
            Err(OffbeatError::OutOfRange { .. })
        ));
    }

    /// Self-consistency check, not validation: the mean growth rate over a step
    /// is the increment divided by the step, and integrating a long history in
    /// small steps must reproduce the two-regime shape — slow cubic growth,
    /// then a sharp acceleration once 2 µm is passed.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// Daily steps at a 600 K interface with `φ = 1e18` n/(m²·s):
    /// `0.2197 µm` after 1 day, `1.0198 µm` after 100 days, `1.9783 µm` after
    /// 730 days — still sub-transition, the layer having taken over two years
    /// to approach 2 µm — then `13.70 µm` at 1095 days and `17.31 µm` at 1200
    /// days, once it has cracked and gone linear. The post-transition rate is
    /// `0.0344 µm/day`, so the first three years produce less oxide than the
    /// three months after transition. These are the model's own numbers at a
    /// *fixed* interface temperature and are recorded as a regression
    /// baseline, **not** as a prediction of any real rod.
    #[test]
    fn integrated_history_shows_the_two_regimes() {
        let model = OxidationKinetics::EpriKwuCe;
        let t = 600.0;
        let flux = 1.0e18;

        let mut s = 0.0;
        let mut recorded = Vec::new();
        for day in 1..=1200 {
            let rate = model.growth_rate(s, t, flux, DAY);
            let next = model.thickness(s, t, flux, DAY);
            assert!(
                (rate * DAY - (next - s)).abs() < 1.0e-18,
                "mean rate must equal increment/dt"
            );
            s = next;
            if matches!(day, 1 | 100 | 730 | 1095 | 1200) {
                recorded.push((day, s));
            }
        }

        let micron = 1.0e-6;
        let expected = [
            (1, 0.2197),
            (100, 1.0198),
            (730, 1.9783),
            (1095, 13.7008),
            (1200, 17.3070),
        ];
        for ((day, value), (expected_day, expected_micron)) in recorded.iter().zip(expected) {
            assert_eq!(*day, expected_day);
            let in_micron = value / micron;
            assert!(
                (in_micron - expected_micron).abs() < 1.0e-3,
                "day {day}: {in_micron} um, recorded {expected_micron} um"
            );
        }
    }
}
