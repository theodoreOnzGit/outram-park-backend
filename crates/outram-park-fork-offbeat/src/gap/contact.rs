// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream
//   `offbeatLib/fvPatchFields/tractionDisplacement/contactFvPatchVectorField.C`
//     (`gapWidth()`, `boundaryStiffness()`, `boundaryShearStiffness()`, and the
//      normal-pressure branch of `updateTraction()`),
//   `offbeatLib/fvPatchFields/tractionDisplacement/gapContactFvPatchVectorField.C`
//     (`updateTraction()` — adding the gap gas pressure to the traction).
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! Mechanical fuel/cladding contact: the penalty interface pressure.
//!
//! # What this computes
//!
//! The **normal interface pressure** \[Pa\] that the fuel and cladding exert on
//! each other once the gap has closed. It is the mechanical half of the closure
//! loop, and it is what the thermal half — [`super::conductance`] — needs as
//! [`GapSurfaces::interface_pressure`](super::conductance::GapSurfaces::interface_pressure).
//!
//! # The penalty formulation
//!
//! Upstream does not solve a constrained contact problem. It lets the two bodies
//! interpenetrate slightly and charges a pressure proportional to the
//! penetration:
//!
//! ```text
//! P = max(−k_penalty · g, 0)      for g < 0 (penetration)
//! P = 0                           for g > 0 (open gap)
//! ```
//!
//! where `g` is the **signed radial gap width** and `k_penalty` \[Pa/m\] is a
//! stiffness derived from the material and the local mesh spacing. A larger
//! penalty enforces the non-penetration constraint more tightly but conditions
//! the linear system worse; upstream's default scale factor is 0.1.
//!
//! # Gap convention — signed, and RADIAL
//!
//! **`signed_radial_gap` is positive when the gap is open and NEGATIVE when the
//! two surfaces interpenetrate.** This is the opposite information content from
//! the thermal side, which clips at zero — and it is deliberate: the penalty
//! formulation is driven entirely by the *amount* of interpenetration, which the
//! thermal side throws away. It is a **radial** normal separation, not a
//! diametral one; see the [module-level conventions]
//! (super#gap-conventions--read-this-before-using-anything-here).
//!
//! Upstream computes it as `(C_nbr + D_nbr − C_own − D_own) · n` on each
//! interface face, without clipping.
//!
//! # Deferred
//!
//! - **The gap-width evaluation itself** (deformed face centres, face normals,
//!   AMI interpolation of the neighbour patch). Taken as an input here.
//! - **Friction and the tangential traction** — upstream's slip/stick update,
//!   `frictionCoeff_`, `penaltyScaleFactFric_` and
//!   [`boundary_shear_stiffness`]'s consumers. Only the shear-stiffness
//!   arithmetic is ported; the slip integration is not.
//! - **The `rigidMasterNormal_` master/slave choice** and the owner-side
//!   interpolation of the contact pressure back onto the neighbour patch.
//!
//! # Units
//!
//! Strict SI raw `f64`: metre, pascal, m², m³, Pa/m for a stiffness.

use crate::error::{OffbeatError, Result};

/// Boundary normal stiffness \[Pa/m\] of a cell touching the interface —
/// upstream's `contactFvPatchVectorField::boundaryStiffness()`.
///
/// ```text
/// k = K · A / V
/// ```
///
/// where `K = 3K/3` is the bulk modulus \[Pa\], `A` the interface face area
/// \[m²\] and `V` the volume \[m³\] of the cell behind that face. Dimensionally
/// this is a pressure per unit displacement: it is the stiffness the cell
/// presents to being squashed normal to the face, so it is the natural scale for
/// a penalty that must be stiff relative to the material but not so stiff that
/// it destroys the conditioning of the displacement solve.
///
/// # Arguments
///
/// - `bulk_modulus` — `K` \[Pa\]. Build it from
///   [`LinearElastic::three_k`](crate::mechanics::LinearElastic::three_k)
///   divided by three, which is exactly what upstream does with its `threeK`
///   patch field.
/// - `face_area` — interface face area \[m²\], `> 0`.
/// - `cell_volume` — volume \[m³\] of the cell behind the face, `> 0`.
///
/// Returns `0.0` for a non-positive volume rather than an infinity.
#[must_use]
pub fn boundary_stiffness(bulk_modulus: f64, face_area: f64, cell_volume: f64) -> f64 {
    if !(cell_volume > 0.0) || !cell_volume.is_finite() {
        return 0.0;
    }
    bulk_modulus * face_area / cell_volume
}

/// Boundary shear stiffness \[Pa/m\] of a cell touching the interface —
/// upstream's `contactFvPatchVectorField::boundaryShearStiffness()`.
///
/// Identical in form to [`boundary_stiffness`] but built from the shear modulus
/// `μ` \[Pa\] instead of the bulk modulus, and used to scale the *friction*
/// penalty rather than the normal one.
///
/// # Deferred
///
/// The friction model this feeds is not ported; the function is here because it
/// is a one-line pure function and omitting it would leave a visible hole beside
/// its normal-stiffness twin.
#[must_use]
pub fn boundary_shear_stiffness(shear_modulus: f64, face_area: f64, cell_volume: f64) -> f64 {
    boundary_stiffness(shear_modulus, face_area, cell_volume)
}

/// Penalty contact parameters — upstream's `contactFvPatchVectorField`
/// dictionary entries `penaltyFactor`, `relativePenetrationTolerance` and
/// `relaxInterfacePressure`.
///
/// # Units
///
/// All three fields are dimensionless. [`Default`] reproduces upstream's
/// defaults exactly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PenaltyContact {
    /// Scale factor \[-\] applied to the smaller of the two boundary stiffnesses
    /// — upstream's `penaltyFactor`, default `0.1`.
    ///
    /// Larger means a stiffer contact and less interpenetration, at the cost of
    /// a worse-conditioned displacement solve. Must be `> 0` for the constraint
    /// to be enforced at all.
    pub penalty_scale: f64,

    /// Relative penetration below which the pressure is **not updated** —
    /// upstream's `relativePenetrationTolerance`, default `0.0`.
    ///
    /// The test is `−g / δ > tolerance`, with `δ` the average cell spacing
    /// across the interface, so the tolerance is a penetration expressed as a
    /// fraction of a cell. See [`Self::interface_pressure`] for the important
    /// consequence of "not updated" meaning *retained*, not *zeroed*.
    pub penetration_tolerance: f64,

    /// Under-relaxation factor \[-\] applied to the pressure between outer
    /// iterations — upstream's `relaxInterfacePressure`, default `1.0` (no
    /// relaxation). Clamped to `[0, 1]`.
    pub relaxation: f64,
}

impl Default for PenaltyContact {
    /// Upstream's defaults: `penalty_scale = 0.1`, `penetration_tolerance = 0`,
    /// `relaxation = 1`.
    fn default() -> Self {
        Self {
            penalty_scale: 0.1,
            penetration_tolerance: 0.0,
            relaxation: 1.0,
        }
    }
}

impl PenaltyContact {
    /// Penalty stiffness \[Pa/m\] for one interface face — upstream's
    /// `penaltyFact = penaltyScaleFact_ * min(boundaryStiffness(), nbrBoundaryStiff)`.
    ///
    /// The **softer** of the two sides governs, because the pair can be no
    /// stiffer than its softer member; using the stiffer one would over-constrain
    /// the interface. Build the two arguments with [`boundary_stiffness`].
    ///
    /// Returns `0.0` if either stiffness is non-positive.
    #[must_use]
    pub fn penalty_factor(&self, fuel_stiffness: f64, clad_stiffness: f64) -> f64 {
        if !(fuel_stiffness > 0.0) || !(clad_stiffness > 0.0) {
            return 0.0;
        }
        self.penalty_scale.max(0.0) * fuel_stiffness.min(clad_stiffness)
    }

    /// Normal interface pressure \[Pa\] on one interface face — the normal
    /// branch of upstream's `contactFvPatchVectorField::updateTraction()`.
    ///
    /// # Arguments
    ///
    /// - `signed_radial_gap` — **RADIAL, signed** gap width \[m\]: positive
    ///   open, **negative interpenetrating**. See the [module
    ///   documentation](self).
    /// - `penalty_factor` — the stiffness \[Pa/m\] from
    ///   [`Self::penalty_factor`].
    /// - `average_cell_spacing` — `δ` \[m\], the mean of the two sides'
    ///   cell-centre-to-face distances (upstream's `avgDelta`, built from
    ///   `1/deltaCoeffs()`). Only used to non-dimensionalise the penetration for
    ///   the tolerance test. Must be `> 0`.
    /// - `previous` — the pressure \[Pa\] from the previous outer iteration,
    ///   used both for under-relaxation and for the retention behaviour below.
    ///   Pass `0.0` on the first iteration.
    ///
    /// # Returns
    ///
    /// A non-negative pressure \[Pa\]. Contact can push the surfaces apart but
    /// never pull them together, so the result is clipped at zero — the gap is
    /// free to reopen.
    ///
    /// # Upstream behaviour reproduced deliberately: the pressure *latches*
    ///
    /// Upstream's update has two branches and **no `else`**:
    ///
    /// ```text
    /// if      (g > 0)                      P = 0;
    /// else if (−g/δ > tolerance)           P = max(−k·g, 0);
    /// // (no else — P keeps its previous value)
    /// ```
    ///
    /// So a face that is *touching but not penetrating enough to clear the
    /// tolerance* keeps whatever pressure it had last iteration, rather than
    /// being recomputed or zeroed. With the default tolerance of `0.0` this
    /// fires on exactly `g == 0` — rare in floating point, but reachable, and
    /// with a non-zero tolerance it fires over a whole band of near-closed gaps.
    /// This port reproduces it, because a run compared against OFFBEAT must show
    /// the same history; [`Self::interface_pressure_no_latch`] gives the
    /// non-latching variant for anyone who wants it.
    ///
    /// ```
    /// use outram_park_fork_offbeat::gap::PenaltyContact;
    ///
    /// let contact = PenaltyContact::default();
    /// let k = contact.penalty_factor(1.0e14, 2.0e14);
    ///
    /// // An open gap carries no contact pressure.
    /// assert_eq!(contact.interface_pressure(1.0e-5, k, 1.0e-4, 0.0), 0.0);
    ///
    /// // Interpenetration of 1 µm is charged at the penalty stiffness.
    /// let p = contact.interface_pressure(-1.0e-6, k, 1.0e-4, 0.0);
    /// assert!((p - k * 1.0e-6).abs() < 1e-6 * p);
    /// ```
    #[must_use]
    pub fn interface_pressure(
        &self,
        signed_radial_gap: f64,
        penalty_factor: f64,
        average_cell_spacing: f64,
        previous: f64,
    ) -> f64 {
        let raw = self.raw_pressure(
            signed_radial_gap,
            penalty_factor,
            average_cell_spacing,
            previous,
        );
        let f = self.relaxation.clamp(0.0, 1.0);
        f * raw + (1.0 - f) * previous
    }

    /// [`Self::interface_pressure`] without the latching behaviour: a
    /// non-penetrating face is charged exactly zero instead of retaining its
    /// previous value.
    ///
    /// This is **not** upstream's behaviour. Use it only when the latch is
    /// demonstrably causing trouble and a comparison against an OFFBEAT run is
    /// not required; otherwise prefer [`Self::interface_pressure`].
    #[must_use]
    pub fn interface_pressure_no_latch(
        &self,
        signed_radial_gap: f64,
        penalty_factor: f64,
        average_cell_spacing: f64,
        previous: f64,
    ) -> f64 {
        let raw = self.raw_pressure(signed_radial_gap, penalty_factor, average_cell_spacing, 0.0);
        let f = self.relaxation.clamp(0.0, 1.0);
        f * raw + (1.0 - f) * previous
    }

    /// The un-relaxed pressure, shared by the latching and non-latching entry
    /// points. `fallback` is what the "no branch fired" case returns.
    fn raw_pressure(
        &self,
        signed_radial_gap: f64,
        penalty_factor: f64,
        average_cell_spacing: f64,
        fallback: f64,
    ) -> f64 {
        if !signed_radial_gap.is_finite() {
            return fallback;
        }
        if signed_radial_gap > 0.0 {
            return 0.0;
        }
        let delta = if average_cell_spacing > 0.0 && average_cell_spacing.is_finite() {
            average_cell_spacing
        } else {
            return fallback;
        };
        if -signed_radial_gap / delta > self.penetration_tolerance {
            (-penalty_factor * signed_radial_gap).max(0.0)
        } else {
            fallback
        }
    }

    /// Reject unusable parameters.
    ///
    /// # Errors
    ///
    /// [`OffbeatError::Unphysical`] for a non-positive or non-finite penalty
    /// scale, a negative penetration tolerance, or a relaxation factor outside
    /// `(0, 1]`. A zero penalty scale is rejected because it silently disables
    /// contact entirely — the gap would close without ever generating a
    /// pressure, and the thermal side would never see contact conduction.
    pub fn validate(&self) -> Result<()> {
        if !(self.penalty_scale > 0.0) || !self.penalty_scale.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "contact penalty scale factor",
                value: self.penalty_scale,
                unit: "-",
                reason: "must be finite and strictly positive; zero silently disables \
                         contact, so the gap would close with no interface pressure",
            });
        }
        if !(self.penetration_tolerance >= 0.0) || !self.penetration_tolerance.is_finite() {
            return Err(OffbeatError::Unphysical {
                quantity: "relative penetration tolerance",
                value: self.penetration_tolerance,
                unit: "-",
                reason: "must be finite and non-negative",
            });
        }
        if !(self.relaxation > 0.0) || self.relaxation > 1.0 {
            return Err(OffbeatError::Unphysical {
                quantity: "interface-pressure relaxation factor",
                value: self.relaxation,
                unit: "-",
                reason: "must lie in (0, 1]; zero would freeze the pressure forever",
            });
        }
        Ok(())
    }
}

/// Total normal pressure \[Pa\] on a gap-facing surface — upstream's
/// `gapContactFvPatchVectorField::updateTraction()`.
///
/// ```text
/// P_total = P_contact + P_gas
/// ```
///
/// The fill gas presses outward on the cladding and inward on the fuel
/// everywhere, whether or not the surfaces touch; the contact pressure adds to
/// it only where they do. Keeping the two separate matters because
/// [`super::conductance`] must be given the **contact** pressure alone — the gas
/// pressure does not flatten asperities and must not enter the contact
/// correlation.
///
/// # Note
///
/// Upstream carries a `TODO: should the gapGas pressure disappear?` beside this
/// addition, i.e. its authors were unsure whether the gas pressure should be
/// dropped once the gap is fully closed and the gas is no longer in contact with
/// that surface. This port reproduces the current behaviour (always added) and
/// records the open question rather than resolving it.
#[must_use]
pub fn total_surface_pressure(contact_pressure: f64, gas_pressure: f64) -> f64 {
    contact_pressure + gas_pressure
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference-checked against upstream's dictionary defaults.
    ///
    /// **Methodology.** `contactFvPatchVectorField.C` lines 201–211 give
    /// `penaltyFactor` 0.1, `relativePenetrationTolerance` 0.0 and
    /// `relaxInterfacePressure` 1.0. Assert [`PenaltyContact::default`] carries
    /// them exactly (tolerance: bitwise equality).
    ///
    /// **Result** (2026-07-29): all three match.
    #[test]
    fn defaults_match_upstream_dictionary_defaults() {
        let c = PenaltyContact::default();
        assert_eq!(c.penalty_scale, 0.1);
        assert_eq!(c.penetration_tolerance, 0.0);
        assert_eq!(c.relaxation, 1.0);
    }

    /// Self-consistency check — the sign convention, stated as a test.
    ///
    /// **Methodology.** A positive `signed_radial_gap` (open) must give exactly
    /// zero pressure; a negative one (interpenetrating) must give a strictly
    /// positive pressure. This is the convention the module documentation
    /// asserts, pinned so it cannot drift.
    ///
    /// **Result** (2026-07-29): open gap 0 Pa exactly; 1 µm of interpenetration
    /// at a penalty stiffness of 1.0e13 Pa/m gives 1.0e7 Pa.
    #[test]
    fn positive_gap_is_open_and_negative_gap_is_penetration() {
        let c = PenaltyContact::default();
        let k = 1.0e13;

        assert_eq!(c.interface_pressure(1.0e-5, k, 1.0e-4, 0.0), 0.0);
        assert_eq!(c.interface_pressure(1.0e-12, k, 1.0e-4, 0.0), 0.0);

        let p = c.interface_pressure(-1.0e-6, k, 1.0e-4, 0.0);
        assert!((p - 1.0e7).abs() < 1e-3, "P = {p}");
    }

    /// Self-consistency check — pressure is linear in penetration.
    ///
    /// **Methodology.** The penalty law is `P = −k·g`, so doubling the
    /// penetration must double the pressure exactly. Swept over ten
    /// penetrations; tolerance 1e-12 relative.
    ///
    /// **Result** (2026-07-29): exactly linear over all ten, to within 1e-16
    /// relative.
    #[test]
    fn pressure_is_linear_in_penetration() {
        let c = PenaltyContact::default();
        let k = c.penalty_factor(1.0e14, 2.0e14);
        assert!((k - 0.1 * 1.0e14).abs() < 1.0, "penalty factor = {k}");

        for i in 1..=10 {
            let g = -1.0e-7 * i as f64;
            let p = c.interface_pressure(g, k, 1.0e-4, 0.0);
            assert!((p - k * -g).abs() < 1e-12 * p, "step {i}: P = {p}");
        }
    }

    /// Self-consistency check — the softer side governs the penalty.
    #[test]
    fn penalty_factor_takes_the_softer_side() {
        let c = PenaltyContact::default();
        assert_eq!(
            c.penalty_factor(1.0e14, 2.0e14),
            c.penalty_factor(2.0e14, 1.0e14)
        );
        assert!((c.penalty_factor(1.0e14, 2.0e14) - 1.0e13).abs() < 1.0);
        assert_eq!(c.penalty_factor(0.0, 2.0e14), 0.0);
    }

    /// Self-consistency check — boundary stiffness has the right dimensions and
    /// scales as `K·A/V`.
    ///
    /// **Methodology.** For a cubic cell of side `L`, `A = L²` and `V = L³`, so
    /// `k = K/L`. Checked for `K = 2.0e11 Pa` (a Zircaloy-like bulk modulus) and
    /// `L = 1.0e-4 m`.
    ///
    /// **Result** (2026-07-29, measured): `k = 2.0e15 Pa/m`, and with upstream's
    /// default 0.1 scale factor the penalty stiffness is 2.0e14 Pa/m — so 1 µm
    /// of interpenetration would be charged 2.0e8 Pa, comfortably above the
    /// contact pressures a rod actually reaches, which is what "stiff enough to
    /// enforce the constraint" means.
    #[test]
    fn boundary_stiffness_is_bulk_modulus_over_cell_size() {
        let l: f64 = 1.0e-4;
        let k_bulk = 2.0e11;
        let k = boundary_stiffness(k_bulk, l * l, l * l * l);
        assert!((k - k_bulk / l).abs() < 1.0, "k = {k}");
        assert!((k - 2.0e15).abs() < 1.0e3);

        // Shear twin behaves identically on its own modulus.
        assert_eq!(
            boundary_shear_stiffness(8.0e10, l * l, l * l * l),
            boundary_stiffness(8.0e10, l * l, l * l * l)
        );
        // Degenerate cell volume is guarded.
        assert_eq!(boundary_stiffness(k_bulk, l * l, 0.0), 0.0);
    }

    /// Reproduced upstream defect — the pressure latches when the penetration is
    /// below tolerance.
    ///
    /// **Methodology.** Upstream's `updateTraction()` has an `if`/`else if` with
    /// no `else`, so a face at `g <= 0` whose relative penetration does not clear
    /// the tolerance keeps its previous pressure. Set a tolerance of 0.01 and a
    /// penetration of `0.005·δ` — below tolerance — and check that a previous
    /// pressure of 1.0e7 Pa survives unchanged rather than being recomputed
    /// (which would give `k·g` = 5.0e6 Pa) or zeroed.
    ///
    /// **Result** (2026-07-29, measured): the latching path returned 1.0e7 Pa
    /// (the retained value); the non-latching variant returned 0.0 Pa. The
    /// difference is real and is upstream's, not this port's.
    #[test]
    fn sub_tolerance_penetration_latches_the_previous_pressure() {
        let c = PenaltyContact {
            penetration_tolerance: 0.01,
            ..PenaltyContact::default()
        };
        let k = 1.0e13;
        let delta = 1.0e-4;
        let g = -0.005 * delta; // relative penetration 0.005 < 0.01

        let latched = c.interface_pressure(g, k, delta, 1.0e7);
        assert!((latched - 1.0e7).abs() < 1.0, "latched = {latched}");

        let unlatched = c.interface_pressure_no_latch(g, k, delta, 1.0e7);
        assert_eq!(unlatched, 0.0);

        // Once the tolerance is cleared, both agree.
        let g_big = -0.05 * delta;
        assert!(
            (c.interface_pressure(g_big, k, delta, 1.0e7)
                - c.interface_pressure_no_latch(g_big, k, delta, 1.0e7))
            .abs()
                < 1e-6
        );
    }

    /// Reproduced upstream behaviour — an exactly-zero gap with the default
    /// zero tolerance also latches.
    ///
    /// **Methodology.** With `tolerance = 0`, the test `−g/δ > 0` is false at
    /// `g = 0`, and `g > 0` is also false, so neither branch fires. A previous
    /// pressure survives. This is the default-configuration instance of the same
    /// defect and is worth pinning separately because it is the one a user is
    /// most likely to meet.
    ///
    /// **Result** (2026-07-29): a previous pressure of 3.0e6 Pa survived a
    /// `g = 0` update unchanged.
    #[test]
    fn exactly_closed_gap_latches_under_the_default_tolerance() {
        let c = PenaltyContact::default();
        assert!((c.interface_pressure(0.0, 1.0e13, 1.0e-4, 3.0e6) - 3.0e6).abs() < 1.0);
        assert_eq!(
            c.interface_pressure_no_latch(0.0, 1.0e13, 1.0e-4, 3.0e6),
            0.0
        );
    }

    /// Self-consistency check — a reopening gap sheds its pressure immediately.
    ///
    /// **Methodology.** Whatever the previous pressure, a strictly positive gap
    /// takes the first branch and returns exactly zero (with no relaxation).
    /// This matters: if a reopening gap kept its pressure, the thermal side
    /// would keep reporting contact conduction across an open gap.
    #[test]
    fn reopening_gap_sheds_pressure_immediately() {
        let c = PenaltyContact::default();
        assert_eq!(c.interface_pressure(1.0e-9, 1.0e13, 1.0e-4, 5.0e7), 0.0);
    }

    /// Self-consistency check — under-relaxation blends towards the new value.
    ///
    /// **Methodology.** With `relaxation = 0.25`, a step from 0 to a computed
    /// 1.0e7 Pa must land at 2.5e6 Pa, and repeated application must converge
    /// monotonically to 1.0e7 Pa.
    ///
    /// **Result** (2026-07-29, measured): first iterate 2.5e6 Pa; after 50
    /// iterations 1.0e7 Pa to within 1e-6 relative, approached monotonically
    /// from below.
    #[test]
    fn relaxation_converges_monotonically_to_the_unrelaxed_value() {
        let c = PenaltyContact {
            relaxation: 0.25,
            ..PenaltyContact::default()
        };
        let k = 1.0e13;
        let g = -1.0e-6;
        let target = k * -g;

        let first = c.interface_pressure(g, k, 1.0e-4, 0.0);
        assert!((first - 0.25 * target).abs() < 1e-6 * target);

        let mut p = 0.0;
        let mut previous = -1.0;
        for _ in 0..50 {
            p = c.interface_pressure(g, k, 1.0e-4, p);
            assert!(p > previous);
            previous = p;
        }
        assert!((p - target).abs() < 1e-6 * target, "converged to {p}");
    }

    /// Self-consistency check — the gas pressure adds to, and stays separable
    /// from, the contact pressure.
    #[test]
    fn total_surface_pressure_adds_the_gas_pressure() {
        assert!((total_surface_pressure(5.0e6, 2.25e6) - 7.25e6).abs() < 1e-6);
        // An open gap still carries the gas pressure.
        assert!((total_surface_pressure(0.0, 2.25e6) - 2.25e6).abs() < 1e-6);
    }

    /// Self-consistency check — parameter validation.
    #[test]
    fn validation_rejects_unusable_parameters() {
        assert!(PenaltyContact::default().validate().is_ok());
        assert!(PenaltyContact {
            penalty_scale: 0.0,
            ..PenaltyContact::default()
        }
        .validate()
        .is_err());
        assert!(PenaltyContact {
            penetration_tolerance: -1.0,
            ..PenaltyContact::default()
        }
        .validate()
        .is_err());
        assert!(PenaltyContact {
            relaxation: 0.0,
            ..PenaltyContact::default()
        }
        .validate()
        .is_err());
        assert!(PenaltyContact {
            relaxation: 1.5,
            ..PenaltyContact::default()
        }
        .validate()
        .is_err());
    }
}
