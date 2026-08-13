// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
// `offbeatLib/fvPatchFields/zeroCurrent/oxidePickupFractionFvPatchScalarField.{C,H}`
// — the hydrogen-pickup boundary condition that consumes the
// `DOxideThickness`/`oxideThickness` surface fields produced by
// `offbeatLib/corrosion/`. Only the pickup *physics* is ported here; the
// finite-volume boundary-condition machinery is not (see `corrosion/mod.rs`).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Hydrogen pickup — how much of the corrosion hydrogen ends up in the metal.
//!
//! # Why this matters more than the oxide itself
//!
//! The corrosion reaction
//!
//! ```text
//! Zr + 2 H2O  ->  ZrO2 + 2 H2
//! ```
//!
//! liberates four hydrogen atoms for every zirconium atom consumed. Most of
//! that hydrogen leaves with the coolant, but a **pickup fraction** — 15% for
//! Zircaloy-4 and M5, up to 25% for ZIRLO, per upstream's own documentation —
//! diffuses into the cladding metal instead.
//!
//! Zirconium dissolves only a little hydrogen (roughly 80–100 wt-ppm at
//! operating temperature, far less when cold). Above that solubility limit the
//! excess precipitates as **zirconium hydride** platelets, which are brittle.
//! A rod that has picked up 600 wt-ppm has a cladding whose fracture toughness
//! is a fraction of the fresh material's, and it is hydride embrittlement —
//! not wall thinning and not the temperature rise — that sets the practical
//! burnup limit for LWR fuel. This module is therefore the point at which
//! corrosion becomes a **failure** problem.
//!
//! # The mass balance, in full
//!
//! Everything in this module follows from one chain of conversions, and it is
//! written out here because every constant below is a link in it.
//!
//! 1. An oxide layer `ΔS` \[m\] thick has consumed `ΔS / 1.56` \[m\] of metal
//!    wall — the Pilling–Bedworth ratio,
//!    [`PILLING_BEDWORTH_ZIRCONIUM`].
//! 2. Per mole of Zr consumed, 2 moles of H2 are released, i.e. `4·M_H` grams
//!    of hydrogen per `M_Zr` grams of zirconium. With upstream's atomic masses
//!    `M_H = 1.00784` and `M_Zr = 91.224`, that is a mass ratio of
//!    `4.4192e-2`.
//! 3. A fraction `f` of it enters the metal.
//! 4. Expressed as a concentration in the *whole* wall, the hydrogen mass per
//!    unit outer area is spread over the wall's volume per unit outer area —
//!    the reciprocal of the surface-to-volume ratio
//!    [`surface_to_volume`].
//! 5. Multiplying by `1e6` turns the mass fraction into **wt-ppm**.
//!
//! Collecting steps 1, 2 and 5 gives the single constant
//! [`HYDROGEN_PER_OXIDE_THICKNESS`] = `28328.13` wt-ppm·m, so that
//!
//! `ΔC \[wt-ppm\] = 28328.13 · f · ΔS \[m\] · (A/V) \[1/m\]`.
//!
//! # This is a wall average
//!
//! Real hydrogen is not uniform: it is driven towards the *cold* side of the
//! wall by the Soret (thermal-diffusion) effect, so the outer rim of an
//! operating rod holds several times the average, and that rim is where
//! hydrides crack. Capturing that needs a hydrogen transport solve — upstream
//! has one, in `physicsSubSolvers/elementTransport/transportSolvers/hydrogenTransport/`,
//! which is outside this module's scope. **A wall average under-predicts the
//! peak local concentration**, and any embrittlement assessment built on the
//! numbers here inherits that non-conservatism.
//!
//! # Units
//!
//! Lengths \[m\], hydrogen concentration \[wt-ppm\], ingress flux
//! \[wt-ppm·m/s\]. The flux unit looks odd until you notice that
//! `flux × area × time / volume` must come out in wt-ppm; it is the natural
//! unit for a boundary condition on a wt-ppm-valued diffusion field, which is
//! exactly what upstream's `oxidePickupFraction` is.
//!
//! # Status
//!
//! AI-assisted translation, reviewed by no human. The tests below establish
//! that pickup is bounded by the hydrogen the reaction actually liberates and
//! that the algebra matches upstream's — they are **not** validation against
//! measured hydrogen data. One test pins an upstream defect in the optional
//! volume-scaling factor; see [`PickupScaling::UpstreamVolumeFactor`].

// NaN-safe guards. Throughout this module a rejection test is written
// `!(x > 0.0)` rather than `x <= 0.0`, deliberately: the negated form is TRUE
// for NaN, so one comparison rejects negatives, zero and NaN together. Clippy's
// `neg_cmp_op_on_partial_ord` suggests the positive form, which would let a NaN
// through and propagate it into a physical result. The idiom is intentional.
#![allow(clippy::neg_cmp_op_on_partial_ord)]

use crate::error::{OffbeatError, Result};

use super::PILLING_BEDWORTH_ZIRCONIUM;

/// Atomic mass of hydrogen \[g/mol\] — upstream's `MH_ = 1.00784`.
pub const HYDROGEN_ATOMIC_MASS: f64 = 1.00784;

/// Atomic mass of zirconium \[g/mol\] — upstream's `MZr_ = 91.224`.
pub const ZIRCONIUM_ATOMIC_MASS: f64 = 91.224;

/// Hydrogen atoms liberated per zirconium atom consumed \[-\].
///
/// Four, from `Zr + 2 H2O -> ZrO2 + 2 H2`. Upstream writes the bare `4` in its
/// expression; naming it makes the stoichiometry visible.
pub const HYDROGEN_ATOMS_PER_ZIRCONIUM: f64 = 4.0;

/// Hydrogen liberated per metre of oxide grown, per metre of wall it is spread
/// over \[wt-ppm·m\].
///
/// `1e6 · 4 · M_H / (M_Zr · 1.56) = 28328.13`. This is the whole mass balance
/// of the [module documentation](self) collapsed into one number: with a 100%
/// pickup fraction, growing `ΔS` of oxide on a wall of surface-to-volume ratio
/// `A/V` adds `28328.13 · ΔS · A/V` wt-ppm of hydrogen.
///
/// For scale: a typical PWR wall (`A/V = 1866` 1/m) growing 60 µm of oxide
/// liberates `3172` wt-ppm, of which 15% — `476` wt-ppm — is picked up.
pub const HYDROGEN_PER_OXIDE_THICKNESS: f64 =
    1.0e6 * HYDROGEN_ATOMS_PER_ZIRCONIUM * HYDROGEN_ATOMIC_MASS
        / (ZIRCONIUM_ATOMIC_MASS * PILLING_BEDWORTH_ZIRCONIUM);

/// Outer-surface area per unit metal volume \[1/m\] of a cylindrical cladding
/// tube of inner radius `inner_radius` and outer radius `outer_radius` \[m\].
///
/// `A/V = 2·r_o / (r_o² − r_i²)`.
///
/// This is the geometric factor that turns a hydrogen **flux through the outer
/// surface** into a **concentration change in the wall**. Upstream never writes
/// it down, because its finite-volume discretisation gets it for free from the
/// real face areas and cell volumes; this port needs it explicitly.
///
/// # Thin-wall limit
///
/// For a thin wall `A/V → 1/(r_o − r_i)`, i.e. one over the wall thickness. The
/// exact expression is larger, because the outer surface is bigger than the
/// inner one. For a 17×17 PWR rod (`r_i = 4.18` mm, `r_o = 4.75` mm) the exact
/// value is `1866.4` 1/m against a thin-wall `1754.4` 1/m — **6.4% apart**,
/// which is enough to matter and small enough to hide, so the exact form is
/// used.
///
/// # Degenerate input
///
/// Returns `0.0` if `outer_radius <= inner_radius` or either radius is
/// negative, rather than a negative or infinite number. A zero surface-to-volume
/// ratio makes every downstream pickup zero, which is the only answer that
/// cannot invent hydrogen.
///
/// ```
/// use outram_park_fork_offbeat::corrosion::hydrogen::surface_to_volume;
///
/// let av = surface_to_volume(4.18e-3, 4.75e-3);
/// assert!((av - 1866.4).abs() < 0.1);
/// // Always at least the thin-wall value.
/// assert!(av > 1.0 / (4.75e-3 - 4.18e-3));
/// ```
#[must_use]
pub fn surface_to_volume(inner_radius: f64, outer_radius: f64) -> f64 {
    if !(outer_radius > inner_radius) || inner_radius < 0.0 || !outer_radius.is_finite() {
        return 0.0;
    }
    2.0 * outer_radius / (outer_radius * outer_radius - inner_radius * inner_radius)
}

/// Hydrogen \[wt-ppm\] that the corrosion reaction **liberates** while growing
/// `oxide_growth` \[m\] of oxide on a tube of the given radii \[m\], expressed
/// as a wall-average concentration.
///
/// This is the total released by `Zr + 2 H2O -> ZrO2 + 2 H2`, *before* any
/// pickup fraction is applied — i.e. the hard upper bound on
/// [`HydrogenPickupModel::pickup`]. Nothing in this module may exceed it, and a
/// unit test asserts exactly that.
///
/// Returns `0.0` for a non-positive growth or a degenerate geometry.
///
/// ```
/// use outram_park_fork_offbeat::corrosion::hydrogen::hydrogen_liberated;
///
/// // 60 um of oxide on a 17x17 PWR rod.
/// let total = hydrogen_liberated(6.0e-5, 4.18e-3, 4.75e-3);
/// assert!((total - 3172.2).abs() < 0.5);
/// ```
#[must_use]
pub fn hydrogen_liberated(oxide_growth: f64, inner_radius: f64, outer_radius: f64) -> f64 {
    if !(oxide_growth > 0.0) {
        return 0.0;
    }
    HYDROGEN_PER_OXIDE_THICKNESS * oxide_growth * surface_to_volume(inner_radius, outer_radius)
}

/// Whether the pickup is scaled by upstream's optional `volFactor`.
///
/// Upstream's `oxidePickupFraction` boundary condition has a `volFactor`
/// switch, documented as taking "into account the reduced volume of clad vs the
/// growing oxide layer", leading to "a more precise H flux". This enum makes
/// that switch explicit rather than a boolean flag, because the two options
/// differ by more than an order of magnitude and a reader must be able to see
/// which is in use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickupScaling {
    /// No volume scaling — upstream's `volFactor false`, which is its default.
    ///
    /// The pickup is `f` times the hydrogen the reaction liberated, spread
    /// uniformly over the as-fabricated wall. This is the option to use.
    Uniform,

    /// Upstream's `volFactor true` scaling, **reproduced verbatim including its
    /// defect**.
    ///
    /// Upstream multiplies the ingress flux by
    ///
    /// ```text
    /// volFactor = (2·r_o·S̄ − S̄²) / (2·r_o·(w − S̄) − w² + S̄²)
    /// ```
    ///
    /// with `S̄` the mid-step mean oxide thickness and `w = r_o − r_i` the
    /// as-fabricated wall thickness.
    ///
    /// # UPSTREAM DEFECT, reproduced deliberately
    ///
    /// Read as areas divided by π, the **denominator** is exactly
    /// `(r_o − S̄)² − r_i²`, the cross-section of metal still remaining — which
    /// is the right thing for the stated intent. The **numerator**, however, is
    /// `r_o² − (r_o − S̄)²`, the cross-section of the *oxide*. The factor
    /// upstream computes is therefore
    ///
    /// `V_oxide / V_remaining_metal`,
    ///
    /// whereas the correction it describes — "the reduced volume of clad" —
    /// is `V_as_fabricated_metal / V_remaining_metal`, which needs the
    /// as-fabricated metal area `r_o² − r_i²` in the numerator instead.
    ///
    /// The two differ by **exactly one**: `intended = upstream + 1`, because
    /// the as-fabricated metal area is the oxide area plus the remaining metal
    /// area. That identity is asserted by a unit test, and it is what makes
    /// this a demonstrable transcription error rather than an opinion about
    /// which model is better.
    ///
    /// Measured consequence for a 17×17 PWR rod (`r_i = 4.18` mm,
    /// `r_o = 4.75` mm), this port, 2026-07-29:
    ///
    /// | mean oxide \[µm\] | upstream factor | intended factor | ratio |
    /// |---|---|---|---|
    /// | 10 | 0.018998 | 1.018998 | 53.6 |
    /// | 30 | 0.059114 | 1.059114 | 17.9 |
    /// | 60 | 0.125207 | 1.125207 | 9.0 |
    /// | 100 | 0.226501 | 1.226501 | 5.4 |
    ///
    /// So `volFactor true` **suppresses** hydrogen pickup by between five- and
    /// fifty-fold, most severely early in life, where the intended correction
    /// is a few percent enhancement. Selecting this variant reproduces an
    /// OFFBEAT run; it does not produce defensible hydrogen numbers. Prefer
    /// [`Uniform`](Self::Uniform).
    ///
    /// A second, smaller inconsistency is worth knowing if you are comparing
    /// case files: upstream's two constructors disagree on the default radii
    /// (`4.565`/`5.315` against `4.5`/`5.32`), and those magnitudes are
    /// millimetres while its own usage documentation writes them in metres
    /// (`0.004565`). They are overwritten from the dictionary whenever
    /// `volFactor` is on, so nothing depends on them — but this port takes
    /// **metres**, with no defaults.
    UpstreamVolumeFactor,
}

/// How much of the corrosion hydrogen enters the cladding metal.
///
/// One variant for "no pickup modelled" and one for upstream's
/// `oxidePickupFraction` boundary condition. Dispatch is by `match`, never by a
/// trait object, per the workspace `CLAUDE.md` "No trait objects" rule.
///
/// # Units
///
/// Radii \[m\], oxide thicknesses \[m\], results \[wt-ppm\] or
/// \[wt-ppm·m/s\].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HydrogenPickupModel {
    /// No hydrogen pickup is modelled — every result is exactly zero.
    ///
    /// This is the state of an OFFBEAT case that runs corrosion but does not
    /// put an `oxidePickupFraction` boundary condition on its hydrogen field,
    /// which is the default. It is **not** a statement that no hydrogen is
    /// picked up in reality; it is a statement that this run does not track it.
    None,

    /// Upstream `oxidePickupFraction` — a fixed fraction of the liberated
    /// hydrogen enters the metal.
    ///
    /// `ingress flux \[wt-ppm·m/s\] = 1e6 · 4 · f / 1.56 · (M_H/M_Zr) ·
    /// dS/dt · volFactor`
    ///
    /// which is upstream's expression exactly, and equals
    /// [`HYDROGEN_PER_OXIDE_THICKNESS`] `· f · dS/dt · volFactor`.
    ///
    /// # Assumptions and limitations
    ///
    /// - The pickup fraction is **constant** — independent of temperature,
    ///   burnup, oxide thickness and alloy chemistry. Real pickup fractions
    ///   drift over life, so this is a life-average value, and matching a
    ///   measured end-of-life hydrogen content by tuning `f` conceals that.
    /// - The result is a **wall average**; see the
    ///   [module documentation](self) on the Soret effect.
    /// - No hydrogen ever leaves. There is no desorption term and no
    ///   solubility limit here.
    OxidePickupFraction {
        /// Fraction of the liberated hydrogen that enters the metal \[-\], in
        /// `[0, 1]` — upstream's `pickupFraction`, default `0.15`.
        ///
        /// Upstream's own guidance, from
        /// `oxidePickupFractionFvPatchScalarField.H`: Zircaloy-4 `0.15`–`0.2`;
        /// ZIRLO and optimized ZIRLO `0.25`; M5 `0.15`.
        ///
        /// Values outside `[0, 1]` are unphysical — more than all the hydrogen
        /// cannot be absorbed — and are rejected by
        /// [`pickup_checked`](HydrogenPickupModel::pickup_checked).
        pickup_fraction: f64,

        /// Cladding inner radius \[**m**\] — upstream's `rInner`.
        ///
        /// Note the unit: metres, matching upstream's own usage documentation
        /// (`0.004565`), not the millimetre-magnitude numbers in upstream's
        /// constructor defaults. About `4.18e-3` m for a 17×17 PWR rod.
        inner_radius: f64,

        /// Cladding outer radius \[**m**\] — upstream's `rOuter`. About
        /// `4.75e-3` m for a 17×17 PWR rod. Must exceed `inner_radius`.
        outer_radius: f64,

        /// Whether upstream's optional volume scaling is applied. See
        /// [`PickupScaling`] — and read
        /// [`UpstreamVolumeFactor`](PickupScaling::UpstreamVolumeFactor)'s
        /// documentation before selecting it.
        scaling: PickupScaling,
    },
}

impl HydrogenPickupModel {
    /// Upstream's default Zircaloy-4 pickup on a 17×17 PWR rod: a 15% pickup
    /// fraction, no volume scaling.
    ///
    /// `inner_radius` and `outer_radius` in metres. This is the constructor to
    /// reach for; it matches upstream's defaults (`pickupFraction 0.15`,
    /// `volFactor false`).
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::HydrogenPickupModel;
    ///
    /// let model = HydrogenPickupModel::zircaloy_4(4.18e-3, 4.75e-3);
    /// // 60 um of oxide over life.
    /// let h = model.pickup(0.0, 6.0e-5);
    /// assert!((h - 475.8).abs() < 0.5, "{h} wt-ppm");
    /// ```
    #[must_use]
    pub fn zircaloy_4(inner_radius: f64, outer_radius: f64) -> Self {
        Self::OxidePickupFraction {
            pickup_fraction: 0.15,
            inner_radius,
            outer_radius,
            scaling: PickupScaling::Uniform,
        }
    }

    /// Increase in the wall-average hydrogen concentration \[wt-ppm\] caused by
    /// growing `oxide_growth` \[m\] of oxide.
    ///
    /// # Parameters
    ///
    /// - `oxide_thickness_before` — oxide thickness \[m\] at the **start** of
    ///   the step. Used only by
    ///   [`UpstreamVolumeFactor`](PickupScaling::UpstreamVolumeFactor)
    ///   scaling, which needs the mid-step mean
    ///   `S̄ = oxide_thickness_before + oxide_growth/2`; ignored entirely by
    ///   [`Uniform`](PickupScaling::Uniform).
    /// - `oxide_growth` — oxide grown over the step \[m\], i.e.
    ///   [`CorrosionStep::oxide_growth`](super::state::CorrosionStep::oxide_growth).
    ///   Non-positive values give `0.0`.
    ///
    /// The timestep does not appear: the pickup depends on how much oxide grew,
    /// not on how long it took. Use [`ingress_flux`](Self::ingress_flux) if you
    /// need the rate form.
    ///
    /// # Behaviour outside the valid range
    ///
    /// Extrapolates without complaint, matching upstream — a pickup fraction of
    /// 3.0 will happily return three times the liberated hydrogen. Use
    /// [`pickup_checked`](Self::pickup_checked) when the parameters came from
    /// user input.
    #[must_use]
    pub fn pickup(&self, oxide_thickness_before: f64, oxide_growth: f64) -> f64 {
        match self {
            Self::None => 0.0,
            Self::OxidePickupFraction {
                pickup_fraction,
                inner_radius,
                outer_radius,
                scaling,
            } => {
                if !(oxide_growth > 0.0) {
                    return 0.0;
                }
                let liberated = hydrogen_liberated(oxide_growth, *inner_radius, *outer_radius);
                let factor = scaling.factor(
                    oxide_thickness_before.max(0.0) + 0.5 * oxide_growth,
                    *inner_radius,
                    *outer_radius,
                );
                pickup_fraction * liberated * factor
            }
        }
    }

    /// Hydrogen ingress flux \[wt-ppm·m/s\] through the cladding outer surface
    /// — upstream's `ingressRate_`.
    ///
    /// This is the quantity upstream imposes as a boundary condition on its
    /// hydrogen field, and is the form to use if you are driving a hydrogen
    /// transport solve rather than a wall average. It is
    /// [`pickup`](Self::pickup) divided by the timestep and by the
    /// surface-to-volume ratio:
    ///
    /// `flux = HYDROGEN_PER_OXIDE_THICKNESS · f · (ΔS/Δt) · volFactor`
    ///
    /// # Parameters
    ///
    /// As [`pickup`](Self::pickup), plus `time_step` \[s\]. A non-positive
    /// timestep gives `0.0`.
    #[must_use]
    pub fn ingress_flux(
        &self,
        oxide_thickness_before: f64,
        oxide_growth: f64,
        time_step: f64,
    ) -> f64 {
        match self {
            Self::None => 0.0,
            Self::OxidePickupFraction {
                pickup_fraction,
                inner_radius,
                outer_radius,
                scaling,
            } => {
                if !(oxide_growth > 0.0) || !(time_step > 0.0) {
                    return 0.0;
                }
                let factor = scaling.factor(
                    oxide_thickness_before.max(0.0) + 0.5 * oxide_growth,
                    *inner_radius,
                    *outer_radius,
                );
                HYDROGEN_PER_OXIDE_THICKNESS * pickup_fraction * (oxide_growth / time_step) * factor
            }
        }
    }

    /// [`pickup`](Self::pickup), but returning an error instead of
    /// extrapolating.
    ///
    /// # Errors
    ///
    /// - [`OffbeatError::Unphysical`] for a pickup fraction outside `[0, 1]`
    ///   (more hydrogen than the reaction produced cannot be absorbed), a
    ///   negative oxide growth, a negative starting thickness, or a geometry
    ///   with `outer_radius <= inner_radius` or a negative `inner_radius`.
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::{HydrogenPickupModel, PickupScaling};
    ///
    /// let impossible = HydrogenPickupModel::OxidePickupFraction {
    ///     pickup_fraction: 1.5, // more than all of it
    ///     inner_radius: 4.18e-3,
    ///     outer_radius: 4.75e-3,
    ///     scaling: PickupScaling::Uniform,
    /// };
    /// assert!(impossible.pickup_checked(0.0, 1.0e-5).is_err());
    /// ```
    ///
    /// [`OffbeatError::Unphysical`]: crate::error::OffbeatError::Unphysical
    pub fn pickup_checked(&self, oxide_thickness_before: f64, oxide_growth: f64) -> Result<f64> {
        if oxide_growth < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "oxide growth for hydrogen pickup",
                value: oxide_growth,
                unit: "m",
                reason: "an oxide layer does not shrink, so it cannot release hydrogen",
            });
        }
        if oxide_thickness_before < 0.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "oxide thickness for hydrogen pickup",
                value: oxide_thickness_before,
                unit: "m",
                reason: "an oxide layer cannot have negative thickness",
            });
        }
        match self {
            Self::None => Ok(0.0),
            Self::OxidePickupFraction {
                pickup_fraction,
                inner_radius,
                outer_radius,
                ..
            } => {
                if !(0.0..=1.0).contains(pickup_fraction) {
                    return Err(OffbeatError::Unphysical {
                        quantity: "hydrogen pickup fraction",
                        value: *pickup_fraction,
                        unit: "-",
                        reason: "a fraction of the liberated hydrogen must lie in [0, 1]",
                    });
                }
                if !(outer_radius > inner_radius) || *inner_radius < 0.0 {
                    return Err(OffbeatError::Unphysical {
                        quantity: "cladding outer radius",
                        value: *outer_radius,
                        unit: "m",
                        reason: "the outer radius must exceed a non-negative inner radius",
                    });
                }
                Ok(self.pickup(oxide_thickness_before, oxide_growth))
            }
        }
    }
}

impl PickupScaling {
    /// The dimensionless multiplier this scaling applies to the ingress flux.
    ///
    /// `1.0` for [`Uniform`](Self::Uniform); upstream's `volFactor` expression,
    /// verbatim, for [`UpstreamVolumeFactor`](Self::UpstreamVolumeFactor).
    ///
    /// - `mean_oxide_thickness` — mid-step mean oxide thickness `S̄` \[m\].
    /// - `inner_radius`, `outer_radius` — cladding radii \[m\].
    ///
    /// Returns `0.0` rather than a `NaN` or a negative number if the geometry
    /// is degenerate or the oxide has eaten the whole wall (`S̄ >= w`), which
    /// would otherwise make upstream's denominator vanish or go negative.
    ///
    /// **Read [`UpstreamVolumeFactor`](Self::UpstreamVolumeFactor)'s
    /// documentation before using it** — upstream's numerator is the oxide
    /// cross-section where the as-fabricated metal cross-section belongs.
    #[must_use]
    pub fn factor(&self, mean_oxide_thickness: f64, inner_radius: f64, outer_radius: f64) -> f64 {
        match self {
            Self::Uniform => 1.0,
            Self::UpstreamVolumeFactor => {
                let s = mean_oxide_thickness.max(0.0);
                let wall = outer_radius - inner_radius;
                if !(wall > 0.0) || s >= wall || inner_radius < 0.0 {
                    return 0.0;
                }
                // Upstream, verbatim:
                //   (2*rOuter*oxAvg - oxAvg^2)
                // / (2*rOuter*(cladTh - oxAvg) - cladTh^2 + oxAvg^2)
                let numerator = 2.0 * outer_radius * s - s * s;
                let denominator = 2.0 * outer_radius * (wall - s) - wall * wall + s * s;
                if !(denominator > 0.0) {
                    return 0.0;
                }
                numerator / denominator
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Inner radius \[m\] of a 17×17 PWR fuel rod.
    const R_INNER: f64 = 4.18e-3;
    /// Outer radius \[m\] of a 17×17 PWR fuel rod.
    const R_OUTER: f64 = 4.75e-3;

    fn zy4() -> HydrogenPickupModel {
        HydrogenPickupModel::zircaloy_4(R_INNER, R_OUTER)
    }

    /// Self-consistency check against the closed form of the mass balance, not
    /// validation: the collected constant must equal the chain of conversions
    /// it stands for, and the surface-to-volume ratio must equal its geometric
    /// definition and sit just above the thin-wall limit.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// `HYDROGEN_PER_OXIDE_THICKNESS = 28328.128` wt-ppm·m. For the 17×17 rod,
    /// `A/V = 1866.368` 1/m against a thin-wall `1754.386` 1/m — the exact
    /// value is 6.38% higher.
    #[test]
    fn the_mass_balance_constants_are_what_they_claim_to_be() {
        let by_hand = 1.0e6 * 4.0 * 1.00784 / (91.224 * 1.56);
        assert!((HYDROGEN_PER_OXIDE_THICKNESS - by_hand).abs() < 1.0e-9);
        assert!((HYDROGEN_PER_OXIDE_THICKNESS - 28_328.128).abs() < 1.0e-3);

        let av = surface_to_volume(R_INNER, R_OUTER);
        let expected = 2.0 * R_OUTER / (R_OUTER * R_OUTER - R_INNER * R_INNER);
        assert!((av - expected).abs() < 1.0e-9);
        assert!((av - 1866.368).abs() < 1.0e-3);

        let thin_wall = 1.0 / (R_OUTER - R_INNER);
        assert!(av > thin_wall);
        assert!((av / thin_wall - 1.0638).abs() < 1.0e-4);
    }

    /// **Reference-checked against the reaction stoichiometry**, which is an
    /// exact, non-fabricated reference: `Zr + 2 H2O -> ZrO2 + 2 H2` liberates
    /// four hydrogen atoms per zirconium, and a *fraction* of them is picked
    /// up. **Pickup can therefore never exceed what was liberated.**
    ///
    /// # Methodology
    ///
    /// - Inputs: pickup fractions across `[0, 1]`, oxide growths from 1 nm to
    ///   200 µm, on the 17×17 PWR geometry, with both scalings.
    /// - Reference: [`hydrogen_liberated`], computed independently from the
    ///   stoichiometry and the Pilling–Bedworth ratio.
    /// - Pass criterion: `pickup <= liberated` for every combination, exactly
    ///   equal at `f = 1` with [`PickupScaling::Uniform`], and exactly zero at
    ///   `f = 0`.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// The bound holds for all 72 combinations. At `f = 1` and
    /// [`PickupScaling::Uniform`] the pickup equals the liberated hydrogen to
    /// better than 1e-12 relative — the two are computed by different code
    /// paths, so this also confirms the pickup path applies no stray factor.
    /// Absolute values for 60 µm of oxide: `3172.24` wt-ppm liberated,
    /// `475.84` wt-ppm picked up at `f = 0.15`.
    ///
    /// # A caveat, and a third consequence of the `volFactor` defect
    ///
    /// The bound is **unconditional only for [`PickupScaling::Uniform`]**.
    /// Upstream's `volFactor` is a ratio of areas that grows without limit as
    /// the oxide consumes the wall, and it crosses `1.0` at a mean oxide
    /// thickness of `275.9` µm on this geometry — beyond which upstream's model
    /// would pick up **more hydrogen than the reaction released**. That is
    /// physically impossible and is a further symptom of the wrong numerator
    /// documented in
    /// [`PickupScaling::UpstreamVolumeFactor`]; the intended factor is bounded
    /// by `V_as_fabricated / V_remaining`, which cannot break the balance while
    /// any metal remains. The crossover is measured and asserted below. It sits
    /// far past any realistic oxide, but it is recorded rather than hidden.
    #[test]
    fn pickup_never_exceeds_the_hydrogen_the_reaction_liberates() {
        for &fraction in &[0.0, 0.05, 0.15, 0.25, 0.5, 1.0] {
            for &growth in &[1.0e-9, 1.0e-7, 1.0e-6, 1.0e-5, 6.0e-5, 2.0e-4] {
                let liberated = hydrogen_liberated(growth, R_INNER, R_OUTER);
                assert!(liberated > 0.0);

                for scaling in [PickupScaling::Uniform, PickupScaling::UpstreamVolumeFactor] {
                    let model = HydrogenPickupModel::OxidePickupFraction {
                        pickup_fraction: fraction,
                        inner_radius: R_INNER,
                        outer_radius: R_OUTER,
                        scaling,
                    };
                    let picked = model.pickup(0.0, growth);
                    assert!(
                        picked >= 0.0 && picked <= liberated * (1.0 + 1.0e-12),
                        "f={fraction} growth={growth} {scaling:?}: {picked} > {liberated}"
                    );
                }

                // f = 1, Uniform: everything liberated is picked up, exactly.
                let all = HydrogenPickupModel::OxidePickupFraction {
                    pickup_fraction: 1.0,
                    inner_radius: R_INNER,
                    outer_radius: R_OUTER,
                    scaling: PickupScaling::Uniform,
                };
                let picked = all.pickup(0.0, growth);
                assert!((picked / liberated - 1.0).abs() < 1.0e-12);
            }
        }

        // f = 0 picks up nothing at all.
        let none = HydrogenPickupModel::OxidePickupFraction {
            pickup_fraction: 0.0,
            inner_radius: R_INNER,
            outer_radius: R_OUTER,
            scaling: PickupScaling::Uniform,
        };
        assert_eq!(none.pickup(0.0, 6.0e-5), 0.0);

        // The recorded absolute values.
        assert!((hydrogen_liberated(6.0e-5, R_INNER, R_OUTER) - 3172.24).abs() < 0.01);
        assert!((zy4().pickup(0.0, 6.0e-5) - 475.84).abs() < 0.01);

        // The caveat: upstream's volFactor crosses 1.0 at a mean oxide
        // thickness of 275.9 um on this geometry, past which the mass balance
        // is broken. Measured by bisection so the number is the code's, not a
        // hand-derived one.
        let factor = |s: f64| PickupScaling::UpstreamVolumeFactor.factor(s, R_INNER, R_OUTER);
        assert!(factor(2.0e-4) < 1.0);
        assert!(factor(3.5e-4) > 1.0);
        let (mut low, mut high) = (2.0e-4, 3.5e-4);
        for _ in 0..80 {
            let mid = 0.5 * (low + high);
            if factor(mid) < 1.0 {
                low = mid;
            } else {
                high = mid;
            }
        }
        let crossover = 0.5 * (low + high);
        assert!(
            (crossover * 1.0e6 - 275.9).abs() < 0.1,
            "recorded volFactor crossover drifted: {} um",
            crossover * 1.0e6
        );
        assert!(crossover < R_OUTER - R_INNER, "still inside the wall");
    }

    /// Self-consistency check, not validation: pickup is linear in both the
    /// pickup fraction and the oxide grown, and accumulates over a chained
    /// history to exactly the same answer as one big step.
    ///
    /// Linearity is what makes the [`PickupScaling::Uniform`] model
    /// path-independent, and it is worth pinning because a caller integrating
    /// day by day must get the same hydrogen as one who integrates yearly.
    #[test]
    fn uniform_pickup_is_linear_and_path_independent() {
        let model = zy4();

        // Linear in growth.
        let single = model.pickup(0.0, 6.0e-5);
        assert!((model.pickup(0.0, 1.2e-4) / single - 2.0).abs() < 1.0e-12);

        // Linear in the pickup fraction.
        let doubled = HydrogenPickupModel::OxidePickupFraction {
            pickup_fraction: 0.30,
            inner_radius: R_INNER,
            outer_radius: R_OUTER,
            scaling: PickupScaling::Uniform,
        };
        assert!((doubled.pickup(0.0, 6.0e-5) / single - 2.0).abs() < 1.0e-12);

        // Path independent: 600 steps of 0.1 um equals one step of 60 um.
        let mut total = 0.0;
        let mut thickness = 0.0;
        for _ in 0..600 {
            total += model.pickup(thickness, 1.0e-7);
            thickness += 1.0e-7;
        }
        assert!(
            (total / single - 1.0).abs() < 1.0e-12,
            "chained {total} vs single-step {single}"
        );
    }

    /// Self-consistency check: the flux form and the concentration form are the
    /// same statement, related by the timestep and the surface-to-volume ratio.
    #[test]
    fn ingress_flux_and_pickup_are_consistent() {
        let model = zy4();
        let growth = 1.0e-6;
        let dt = 86_400.0;
        let av = surface_to_volume(R_INNER, R_OUTER);

        let flux = model.ingress_flux(0.0, growth, dt);
        let concentration = model.pickup(0.0, growth);
        assert!((flux * dt * av / concentration - 1.0).abs() < 1.0e-12);

        // Non-positive timesteps and growths give exactly zero.
        assert_eq!(model.ingress_flux(0.0, growth, 0.0), 0.0);
        assert_eq!(model.ingress_flux(0.0, growth, -1.0), 0.0);
        assert_eq!(model.ingress_flux(0.0, 0.0, dt), 0.0);
        assert_eq!(model.pickup(0.0, 0.0), 0.0);
        assert_eq!(model.pickup(0.0, -1.0e-6), 0.0);

        // The `None` model is inert.
        assert_eq!(HydrogenPickupModel::None.pickup(1.0e-5, 1.0e-6), 0.0);
        assert_eq!(
            HydrogenPickupModel::None.ingress_flux(1.0e-5, 1.0e-6, dt),
            0.0
        );
        assert_eq!(
            HydrogenPickupModel::None
                .pickup_checked(0.0, 1.0e-6)
                .unwrap(),
            0.0
        );
    }

    /// **Documents a defect in upstream OFFBEAT that this port reproduces
    /// deliberately.** Upstream's optional `volFactor` computes
    /// `V_oxide / V_remaining_metal` where the stated intent —
    /// "the reduced volume of clad vs the growing oxide layer" — requires
    /// `V_as_fabricated_metal / V_remaining_metal`.
    ///
    /// # Methodology
    ///
    /// - Inputs: the 17×17 PWR geometry (`r_i = 4.18` mm, `r_o = 4.75` mm) with
    ///   mean oxide thicknesses of 10, 30, 60 and 100 µm.
    /// - Reference: an exact algebraic identity, not a fitted number. The
    ///   as-fabricated metal cross-section equals the oxide cross-section plus
    ///   the remaining metal cross-section, so, over upstream's own
    ///   denominator, `intended = upstream + 1` **exactly**. That identity is
    ///   the proof that the numerator is the wrong area, and it is what this
    ///   test asserts.
    /// - Pass criterion: the identity holds to 1e-12 relative, and upstream's
    ///   own denominator is verified to equal `(r_o − S̄)² − r_i²`.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// | mean oxide \[µm\] | upstream | intended | ratio |
    /// |---|---|---|---|
    /// | 10 | 0.018998 | 1.018998 | 53.64 |
    /// | 30 | 0.059114 | 1.059114 | 17.92 |
    /// | 60 | 0.125207 | 1.125207 | 8.99 |
    /// | 100 | 0.226501 | 1.226501 | 5.42 |
    ///
    /// The identity `intended − upstream = 1` holds to machine precision at
    /// every point.
    ///
    /// # Interpretation
    ///
    /// `volFactor true` suppresses hydrogen pickup by 5× to 54×, worst early in
    /// life, where the intended correction is a 2% *enhancement*. This test
    /// asserts the defective values so that a fix upstream cannot land here
    /// unnoticed; it **documents** them and does not endorse them.
    /// [`PickupScaling::Uniform`] is the option to use.
    #[test]
    fn upstream_volume_factor_uses_the_wrong_numerator_reproducing_upstream() {
        let wall = R_OUTER - R_INNER;

        for (micron, recorded) in [
            (10.0, 0.018_998),
            (30.0, 0.059_114),
            (60.0, 0.125_207),
            (100.0, 0.226_501),
        ] {
            let s = micron * 1.0e-6;
            let upstream = PickupScaling::UpstreamVolumeFactor.factor(s, R_INNER, R_OUTER);
            assert!(
                (upstream - recorded).abs() < 1.0e-6,
                "{micron} um: {upstream} vs recorded {recorded}"
            );

            // Upstream's denominator IS the remaining metal cross-section.
            let denominator = 2.0 * R_OUTER * (wall - s) - wall * wall + s * s;
            let remaining_metal = (R_OUTER - s) * (R_OUTER - s) - R_INNER * R_INNER;
            assert!((denominator / remaining_metal - 1.0).abs() < 1.0e-12);

            // ...and its numerator is the OXIDE cross-section, not the
            // as-fabricated metal one. Hence the exact off-by-one.
            let intended = (R_OUTER * R_OUTER - R_INNER * R_INNER) / remaining_metal;
            assert!(
                (intended - upstream - 1.0).abs() < 1.0e-12,
                "{micron} um: intended {intended} - upstream {upstream} != 1"
            );
            assert!(intended > 1.0, "the intended correction is an enhancement");
        }

        // The consequence on a real pickup number.
        let uniform = zy4();
        let scaled = HydrogenPickupModel::OxidePickupFraction {
            pickup_fraction: 0.15,
            inner_radius: R_INNER,
            outer_radius: R_OUTER,
            scaling: PickupScaling::UpstreamVolumeFactor,
        };
        let plain = uniform.pickup(0.0, 6.0e-5);
        let suppressed = scaled.pickup(0.0, 6.0e-5);
        assert!(
            suppressed < plain / 5.0,
            "volFactor should suppress pickup: {suppressed} vs {plain}"
        );
    }

    /// Degenerate geometry is reported rather than turned into a `NaN`, an
    /// infinity or a negative hydrogen concentration.
    #[test]
    fn degenerate_geometry_and_parameters_degrade_safely() {
        // Surface-to-volume of a nonsensical tube.
        assert_eq!(surface_to_volume(5.0e-3, 4.0e-3), 0.0);
        assert_eq!(surface_to_volume(4.0e-3, 4.0e-3), 0.0);
        assert_eq!(surface_to_volume(-1.0e-3, 4.0e-3), 0.0);
        assert_eq!(hydrogen_liberated(1.0e-5, 5.0e-3, 4.0e-3), 0.0);
        assert_eq!(hydrogen_liberated(-1.0e-5, R_INNER, R_OUTER), 0.0);

        // The volume factor when the oxide has eaten the wall.
        assert_eq!(
            PickupScaling::UpstreamVolumeFactor.factor(1.0e-2, R_INNER, R_OUTER),
            0.0
        );
        assert_eq!(
            PickupScaling::UpstreamVolumeFactor.factor(1.0e-5, 5.0e-3, 4.0e-3),
            0.0
        );
        assert_eq!(PickupScaling::Uniform.factor(1.0e-2, R_INNER, R_OUTER), 1.0);

        // The checked path.
        let backwards = HydrogenPickupModel::OxidePickupFraction {
            pickup_fraction: 0.15,
            inner_radius: 5.0e-3,
            outer_radius: 4.0e-3,
            scaling: PickupScaling::Uniform,
        };
        assert!(matches!(
            backwards.pickup_checked(0.0, 1.0e-5),
            Err(OffbeatError::Unphysical { .. })
        ));
        assert_eq!(backwards.pickup(0.0, 1.0e-5), 0.0);

        for fraction in [-0.1, 1.5, f64::NAN] {
            let model = HydrogenPickupModel::OxidePickupFraction {
                pickup_fraction: fraction,
                inner_radius: R_INNER,
                outer_radius: R_OUTER,
                scaling: PickupScaling::Uniform,
            };
            assert!(matches!(
                model.pickup_checked(0.0, 1.0e-5),
                Err(OffbeatError::Unphysical { .. })
            ));
        }

        assert!(matches!(
            zy4().pickup_checked(0.0, -1.0e-6),
            Err(OffbeatError::Unphysical { .. })
        ));
        assert!(matches!(
            zy4().pickup_checked(-1.0e-6, 1.0e-6),
            Err(OffbeatError::Unphysical { .. })
        ));
        assert!(zy4().pickup_checked(0.0, 1.0e-6).is_ok());
    }
}
