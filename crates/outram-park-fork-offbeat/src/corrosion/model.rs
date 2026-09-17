// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Derived from OFFBEAT (https://gitlab.com/foam-for-nuclear/offbeat),
// upstream commit 80e84450a115b0c411e1bfa5d166379f6bf6c084, GPL-3.0.
// Corresponds to upstream `offbeatLib/corrosion/corrosionModel/`:
//   corrosionModel.{C,H}           -> CorrosionModel::Constant
//                                     (upstream TypeName "fromLatestTime")
//   zircaloyOuterCorrosion.{C,H}   -> CorrosionModel::ZircaloyOuter
// and to the `corrosion.{C,H}` base class, whose TypeName is "constant" and
// which likewise leaves the oxide fields untouched.
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice.

//! The patch-level corrosion model: kinetics plus metal loss plus hydrogen.

use crate::error::Result;

use super::hydrogen::HydrogenPickupModel;
use super::kinetics::OxidationKinetics;
use super::state::{CorrosionState, CorrosionStep};
use super::PILLING_BEDWORTH_ZIRCONIUM;

/// A complete waterside-corrosion model for one cladding surface.
///
/// One variant per corrosion model registered in upstream OFFBEAT's
/// `corrosionModel` run-time selection table. Each variant's documentation
/// names the upstream class and the string a user writes in a case dictionary,
/// so an OFFBEAT case can be translated variant by variant.
///
/// Dispatch is by `match`, never by a trait object, per the workspace
/// `CLAUDE.md` "No trait objects" rule.
///
/// # What a corrosion model owns
///
/// Three coupled results, delivered together as a [`CorrosionStep`]:
///
/// 1. **Oxide thickness** \[m\] and its increment — from the
///    [`OxidationKinetics`] this model carries.
/// 2. **Metal loss** \[m\] — the increment divided by the Pilling–Bedworth
///    ratio 1.56, i.e. the inward wall displacement a moving-mesh driver must
///    apply.
/// 3. **Hydrogen pickup** \[wt-ppm\] — from the [`HydrogenPickupModel`] this
///    model carries.
///
/// # What it does not own
///
/// The mesh. Upstream's corrosion model additionally rewrites the boundary
/// thermal conductivity in place and drives a topology changer that adds and
/// removes cell layers; neither ports without a live mesh. The thermal
/// calculation is available as a pure function in [`super::thermal`], and the
/// topology changer is deferred — see the [module documentation](super).
///
/// # Units
///
/// Raw `f64`, strict SI, except hydrogen in wt-ppm. See [`CorrosionState`] and
/// [`CorrosionStep`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CorrosionModel {
    /// The oxide layer does not evolve — upstream `corrosionModel`,
    /// `TypeName("fromLatestTime")`, and equivalently the `corrosion` base
    /// class, `TypeName("constant")`.
    ///
    /// Every step returns [`CorrosionStep::unchanged`]: whatever oxide was
    /// there stays there, no metal is consumed, no hydrogen is picked up. This
    /// is the **default** in an OFFBEAT case that does not ask for corrosion,
    /// and it is what a case uses when it wants to *impose* a fixed oxide
    /// profile read from a file rather than compute one.
    ///
    /// It is not "no oxide": a non-zero
    /// [`CorrosionState::oxide_thickness`] is carried through unchanged, so
    /// the layer's thermal effect via [`super::thermal`] still applies.
    ///
    /// Upstream notes that `fromLatestTime` is deprecated in favour of
    /// `constant`; both name the same do-nothing behaviour, and this port
    /// carries one variant for both.
    Constant,

    /// Zircaloy outer-surface (waterside) corrosion — upstream
    /// `zircaloyOuterCorrosion`, `TypeName("zircaloyOuterCorrosion")`.
    ///
    /// The real model. It grows the oxide with the [`OxidationKinetics`] given,
    /// converts the growth to metal loss with the Pilling–Bedworth ratio
    /// (upstream's `updateDMetalThickness`, which is literally
    /// `DS_metal = DS_oxide / 1.56`), and — going one step beyond upstream's
    /// corrosion class, which leaves this to a separate boundary condition —
    /// computes the hydrogen that goes into the metal.
    ///
    /// # This is the *outer* surface only
    ///
    /// Upstream models waterside corrosion on the cladding's coolant-facing
    /// surface. Inner-surface (fuel-side) oxidation, which consumes the small
    /// amount of oxygen released by the fuel and by residual moisture, is a
    /// different and much slower process, and neither upstream nor this port
    /// models it.
    ///
    /// # Choosing the parts
    ///
    /// - `kinetics` — for a realistic LWR case use
    ///   [`OxidationKinetics::EpriKwuCeCathcartPawel`], which is the only
    ///   combined model upstream compiles. The single-regime variants are for
    ///   studying one branch in isolation.
    /// - `hydrogen` — [`HydrogenPickupModel::None`] to skip hydrogen entirely,
    ///   or [`HydrogenPickupModel::zircaloy_4`] for upstream's defaults.
    ///
    /// See [`CorrosionModel::zircaloy_outer_default`] for the usual
    /// combination.
    ///
    /// [`CorrosionState::oxide_thickness`]: super::state::CorrosionState::oxide_thickness
    ZircaloyOuter {
        /// Which oxide-growth law to use.
        kinetics: OxidationKinetics,
        /// Which hydrogen-pickup model to use, if any.
        hydrogen: HydrogenPickupModel,
    },
}

impl CorrosionModel {
    /// Zircaloy waterside corrosion with the combined
    /// EPRI/KWU/C-E + Cathcart–Pawel kinetics and **no** hydrogen pickup —
    /// upstream's usual case setup.
    ///
    /// This matches the configuration in upstream's own `corrosionByPatch`
    /// usage example (`corrosionModel zircaloyOuterCorrosion;
    /// oxidationKineticsModel EPRI-KWU-CE|Cathcart-Pawel;`), where hydrogen
    /// pickup is a separate opt-in boundary condition on the hydrogen field.
    ///
    /// Use [`with_hydrogen_pickup`](Self::with_hydrogen_pickup) to add it.
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::{CorrosionModel, CorrosionState};
    ///
    /// let model = CorrosionModel::zircaloy_outer_default();
    /// let step = model.step(&CorrosionState::fresh(600.0, 86_400.0));
    /// assert!(step.oxide_growth > 0.0);
    /// assert_eq!(step.hydrogen_pickup, 0.0);
    /// ```
    #[must_use]
    pub fn zircaloy_outer_default() -> Self {
        Self::ZircaloyOuter {
            kinetics: OxidationKinetics::EpriKwuCeCathcartPawel,
            hydrogen: HydrogenPickupModel::None,
        }
    }

    /// The same model with a hydrogen-pickup submodel attached.
    ///
    /// Returns `self` unchanged for [`Constant`](Self::Constant), which has no
    /// growth for hydrogen to come from.
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::{
    ///     CorrosionModel, CorrosionState, HydrogenPickupModel,
    /// };
    ///
    /// let model = CorrosionModel::zircaloy_outer_default()
    ///     .with_hydrogen_pickup(HydrogenPickupModel::zircaloy_4(4.18e-3, 4.75e-3));
    ///
    /// let mut state = CorrosionState::fresh(600.0, 86_400.0);
    /// state.oxide_thickness = 4.0e-5;
    /// let step = model.step(&state);
    /// assert!(step.hydrogen_pickup > 0.0);
    /// ```
    #[must_use]
    pub fn with_hydrogen_pickup(self, hydrogen: HydrogenPickupModel) -> Self {
        match self {
            Self::Constant => Self::Constant,
            Self::ZircaloyOuter { kinetics, .. } => Self::ZircaloyOuter { kinetics, hydrogen },
        }
    }

    /// Advance one boundary face by one timestep.
    ///
    /// This is the whole model in one call: kinetics, metal loss and hydrogen
    /// pickup, all consistent with each other because they are all derived from
    /// the same oxide increment.
    ///
    /// # Behaviour outside the valid range
    ///
    /// Extrapolates, matching upstream. Use
    /// [`step_checked`](Self::step_checked) when the inputs may be out of
    /// range.
    ///
    /// # Example — integrating a life history
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::{
    ///     CorrosionModel, CorrosionState, HydrogenPickupModel,
    /// };
    /// use outram_park_fork_offbeat::materials::MaterialState;
    ///
    /// let model = CorrosionModel::zircaloy_outer_default()
    ///     .with_hydrogen_pickup(HydrogenPickupModel::zircaloy_4(4.18e-3, 4.75e-3));
    ///
    /// let mut state = CorrosionState::fresh(600.0, 86_400.0);
    /// state.fast_flux = 7.0e17;
    /// let mut cladding = MaterialState::fresh(600.0);
    ///
    /// for _ in 0..1500 {
    ///     let step = model.step(&state);
    ///     step.apply_to(&mut cladding);
    ///     state = state.advanced(&step);
    /// }
    ///
    /// assert!(state.oxide_thickness > 2.0e-6, "past transition after four years");
    /// assert!(cladding.hydrogen_content > 0.0);
    /// ```
    #[must_use]
    pub fn step(&self, state: &CorrosionState) -> CorrosionStep {
        match self {
            Self::Constant => CorrosionStep::unchanged(state.oxide_thickness),
            Self::ZircaloyOuter { kinetics, hydrogen } => {
                let before = state.oxide_thickness.max(0.0);
                let thickness = kinetics.thickness(
                    before,
                    state.interface_temperature,
                    state.fast_flux,
                    state.time_step,
                );
                let growth = (thickness - before).max(0.0);
                CorrosionStep {
                    oxide_thickness: thickness,
                    oxide_growth: growth,
                    metal_loss: Self::metal_loss(growth),
                    hydrogen_pickup: hydrogen.pickup(before, growth),
                }
            }
        }
    }

    /// [`step`](Self::step), but returning an error instead of extrapolating.
    ///
    /// # Errors
    ///
    /// Whatever
    /// [`OxidationKinetics::thickness_checked`] and
    /// [`HydrogenPickupModel::pickup_checked`] report: an unphysical input, a
    /// temperature outside the correlation's fitted range, an out-of-range
    /// pickup fraction, or a degenerate cladding geometry.
    /// [`Constant`](Self::Constant) never errors.
    pub fn step_checked(&self, state: &CorrosionState) -> Result<CorrosionStep> {
        match self {
            Self::Constant => Ok(CorrosionStep::unchanged(state.oxide_thickness)),
            Self::ZircaloyOuter { kinetics, hydrogen } => {
                let before = state.oxide_thickness;
                let thickness = kinetics.thickness_checked(
                    before,
                    state.interface_temperature,
                    state.fast_flux,
                    state.time_step,
                )?;
                let growth = (thickness - before.max(0.0)).max(0.0);
                let pickup = hydrogen.pickup_checked(before.max(0.0), growth)?;
                Ok(CorrosionStep {
                    oxide_thickness: thickness,
                    oxide_growth: growth,
                    metal_loss: Self::metal_loss(growth),
                    hydrogen_pickup: pickup,
                })
            }
        }
    }

    /// Metal wall thickness \[m\] consumed to grow `oxide_growth` \[m\] of
    /// oxide.
    ///
    /// `oxide_growth / 1.56` — upstream's `updateDMetalThickness`, verbatim.
    /// The Pilling–Bedworth ratio being greater than one means the oxide is
    /// always thicker than the metal it replaced, so **the wall thins more
    /// slowly than the oxide grows**, and the rod's outer radius actually
    /// increases: a 60 µm oxide has eaten 38.5 µm of wall and added 21.5 µm to
    /// the outside.
    ///
    /// This is a pure geometric conversion with no temperature or material
    /// dependence, and it is deliberately an associated function rather than a
    /// method: every corrosion model in upstream uses the same ratio.
    ///
    /// Negative growth is floored at zero — oxide does not un-grow.
    ///
    /// ```
    /// use outram_park_fork_offbeat::corrosion::CorrosionModel;
    ///
    /// let consumed = CorrosionModel::metal_loss(6.0e-5);
    /// assert!((consumed - 3.846e-5).abs() < 1.0e-8);
    /// assert!(consumed < 6.0e-5, "the oxide is thicker than the metal it ate");
    /// ```
    #[must_use]
    pub fn metal_loss(oxide_growth: f64) -> f64 {
        oxide_growth.max(0.0) / PILLING_BEDWORTH_ZIRCONIUM
    }

    /// The oxidation kinetics this model uses, if it grows oxide at all.
    ///
    /// `None` for [`Constant`](Self::Constant). Provided so a caller can query
    /// the kinetics directly — for a growth rate, or to check a validity range
    /// — without destructuring the enum.
    #[must_use]
    pub fn kinetics(&self) -> Option<OxidationKinetics> {
        match self {
            Self::Constant => None,
            Self::ZircaloyOuter { kinetics, .. } => Some(*kinetics),
        }
    }

    /// The hydrogen-pickup model this corrosion model carries.
    ///
    /// [`HydrogenPickupModel::None`] for [`Constant`](Self::Constant).
    #[must_use]
    pub fn hydrogen_pickup(&self) -> HydrogenPickupModel {
        match self {
            Self::Constant => HydrogenPickupModel::None,
            Self::ZircaloyOuter { hydrogen, .. } => *hydrogen,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corrosion::hydrogen::hydrogen_liberated;
    use crate::corrosion::PickupScaling;
    use crate::error::OffbeatError;
    use crate::materials::MaterialState;

    const DAY: f64 = 86_400.0;
    const R_INNER: f64 = 4.18e-3;
    const R_OUTER: f64 = 4.75e-3;

    fn full_model() -> CorrosionModel {
        CorrosionModel::zircaloy_outer_default()
            .with_hydrogen_pickup(HydrogenPickupModel::zircaloy_4(R_INNER, R_OUTER))
    }

    /// The `Constant` model is inert, by definition — it is what an OFFBEAT
    /// case selects to freeze the oxide profile it read from a file.
    #[test]
    fn the_constant_model_changes_nothing() {
        let model = CorrosionModel::Constant;
        let mut state = CorrosionState::fresh(600.0, DAY);
        state.oxide_thickness = 4.0e-5;
        state.fast_flux = 7.0e17;

        let step = model.step(&state);
        assert_eq!(step.oxide_thickness, 4.0e-5);
        assert_eq!(step.oxide_growth, 0.0);
        assert_eq!(step.metal_loss, 0.0);
        assert_eq!(step.hydrogen_pickup, 0.0);
        assert_eq!(model.step_checked(&state).unwrap(), step);

        // ...and it stays inert even at an absurd temperature, where the
        // checked path of a real model would refuse.
        state.interface_temperature = 5000.0;
        assert!(model.step_checked(&state).is_ok());
        assert_eq!(model.kinetics(), None);
        assert_eq!(model.hydrogen_pickup(), HydrogenPickupModel::None);

        // Adding hydrogen pickup to it is a no-op, because it grows no oxide.
        let with = model.with_hydrogen_pickup(HydrogenPickupModel::zircaloy_4(R_INNER, R_OUTER));
        assert_eq!(with, CorrosionModel::Constant);
    }

    /// **Reference-checked against upstream's own `updateDMetalThickness`**,
    /// which is the exact statement `DS_metal = DS_oxide / 1.56`.
    ///
    /// # Methodology
    ///
    /// - Inputs: oxide growths from 1 nm to 200 µm.
    /// - Reference: `growth / 1.56`, from the Pilling–Bedworth ratio, plus the
    ///   physical requirement that the metal consumed is *less* than the oxide
    ///   grown.
    /// - Tolerance: exact equality to `growth / 1.56`.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// Exact for all inputs. For 60 µm of oxide, `38.462` µm of wall is
    /// consumed and the outer radius grows by `21.538` µm — the ratio of
    /// consumed to grown is `0.641026` = 1/1.56 throughout.
    #[test]
    fn metal_loss_is_the_pilling_bedworth_conversion() {
        for growth in [1.0e-9, 1.0e-7, 1.0e-6, 1.0e-5, 6.0e-5, 2.0e-4] {
            let loss = CorrosionModel::metal_loss(growth);
            assert_eq!(loss, growth / 1.56);
            assert!(loss < growth, "the oxide must be thicker than the metal");
            assert!((loss / growth - 0.641_025_641_025_641).abs() < 1.0e-15);
        }
        assert!((CorrosionModel::metal_loss(6.0e-5) - 3.846_153_8e-5).abs() < 1.0e-11);

        // Negative growth is floored, never turned into metal gain.
        assert_eq!(CorrosionModel::metal_loss(-1.0e-5), 0.0);
        assert_eq!(CorrosionModel::metal_loss(0.0), 0.0);
    }

    /// Self-consistency check, not validation: everything in a
    /// [`CorrosionStep`] must be derived from the same oxide increment, so the
    /// four numbers cannot disagree with one another.
    #[test]
    fn a_step_is_internally_consistent() {
        let model = full_model();
        let mut state = CorrosionState::fresh(610.0, DAY);
        state.fast_flux = 7.0e17;

        for _ in 0..2000 {
            let step = model.step(&state);
            assert!(step.oxide_growth >= 0.0);
            assert!(
                (step.oxide_thickness - (state.oxide_thickness + step.oxide_growth)).abs()
                    < 1.0e-18
            );
            assert_eq!(step.metal_loss, step.oxide_growth / 1.56);
            assert!(step.metal_loss <= step.oxide_growth);
            assert!(step.hydrogen_pickup >= 0.0);
            assert!(
                step.hydrogen_pickup
                    <= hydrogen_liberated(step.oxide_growth, R_INNER, R_OUTER) * (1.0 + 1.0e-12),
                "picked up more hydrogen than the reaction released"
            );
            state = state.advanced(&step);
        }
        assert!(state.oxide_thickness > 0.0);
    }

    /// Self-consistency check, not validation: an integrated life history must
    /// conserve the hydrogen mass balance in aggregate, not merely step by
    /// step. The total hydrogen accumulated into a
    /// [`MaterialState`](crate::materials::MaterialState) must equal the pickup
    /// fraction times the hydrogen liberated by the **total** oxide grown.
    ///
    /// # Methodology
    ///
    /// - Inputs: 1500 daily steps at a fixed 610 K interface with
    ///   `φ = 7e17` n/(m²·s), Zircaloy-4 pickup (`f = 0.15`) on a 17×17 rod.
    /// - Reference: `f · hydrogen_liberated(total oxide grown)`, computed once
    ///   at the end from the total, and therefore independent of the path.
    /// - Tolerance: 1e-12 relative.
    ///
    /// # Results (measured 2026-07-29, this port)
    ///
    /// After 1500 days: oxide `49.605` µm, wall consumed `31.798` µm, hydrogen
    /// `393.40` wt-ppm against a liberated total of `2622.6` wt-ppm — exactly
    /// 15%, agreeing with the path-independent reference to better than 1e-12
    /// relative.
    ///
    /// # Interpretation
    ///
    /// Both the accumulation into `MaterialState` and the per-step pickup are
    /// linear and lossless, so a caller may take any timestep and get the same
    /// hydrogen. The absolute numbers are the model's own at a *fixed*
    /// interface temperature; a real rod's interface heats up as the oxide
    /// insulates, which this steady-temperature history does not capture.
    #[test]
    fn an_integrated_history_conserves_the_hydrogen_mass_balance() {
        let model = full_model();
        let mut state = CorrosionState::fresh(610.0, DAY);
        state.fast_flux = 7.0e17;
        let mut cladding = MaterialState::fresh(610.0);
        let mut total_metal_loss = 0.0;

        for _ in 0..1500 {
            let step = model.step(&state);
            step.apply_to(&mut cladding);
            total_metal_loss += step.metal_loss;
            state = state.advanced(&step);
        }

        let total_oxide = state.oxide_thickness;
        let reference = 0.15 * hydrogen_liberated(total_oxide, R_INNER, R_OUTER);
        assert!(
            (cladding.hydrogen_content / reference - 1.0).abs() < 1.0e-12,
            "accumulated {} wt-ppm vs path-independent {} wt-ppm",
            cladding.hydrogen_content,
            reference
        );
        assert!((total_metal_loss - total_oxide / 1.56).abs() < 1.0e-15);

        // The recorded values.
        assert!(
            (total_oxide * 1.0e6 - 49.605).abs() < 1.0e-2,
            "recorded oxide drifted: {} um",
            total_oxide * 1.0e6
        );
        assert!(
            (total_metal_loss * 1.0e6 - 31.798).abs() < 1.0e-2,
            "recorded metal loss drifted: {} um",
            total_metal_loss * 1.0e6
        );
        assert!(
            (cladding.hydrogen_content - 393.40).abs() < 0.05,
            "recorded hydrogen drifted: {} wt-ppm",
            cladding.hydrogen_content
        );
        assert!(
            (hydrogen_liberated(total_oxide, R_INNER, R_OUTER) - 2622.6).abs() < 0.1,
            "recorded liberated hydrogen drifted"
        );
    }

    /// The checked path propagates the errors of the parts it is built from.
    #[test]
    fn step_checked_reports_what_its_parts_report() {
        let model = full_model();

        // A temperature above the combined model's stated range.
        let mut hot = CorrosionState::fresh(3000.0, DAY);
        hot.oxide_thickness = 1.0e-5;
        assert!(matches!(
            model.step_checked(&hot),
            Err(OffbeatError::OutOfRange { .. })
        ));
        assert!(model.step(&hot).oxide_thickness.is_finite());

        // The broken Cathcart-Pawel window.
        let broken = CorrosionState::fresh(1850.0, 1.0);
        assert!(matches!(
            model.step_checked(&broken),
            Err(OffbeatError::Unphysical { .. })
        ));

        // A degenerate cladding geometry, caught by the hydrogen submodel.
        let bad_geometry = CorrosionModel::zircaloy_outer_default().with_hydrogen_pickup(
            HydrogenPickupModel::OxidePickupFraction {
                pickup_fraction: 0.15,
                inner_radius: 5.0e-3,
                outer_radius: 4.0e-3,
                scaling: PickupScaling::Uniform,
            },
        );
        assert!(matches!(
            bad_geometry.step_checked(&CorrosionState::fresh(600.0, DAY)),
            Err(OffbeatError::Unphysical { .. })
        ));

        // A well-posed case goes through and agrees with the unchecked path.
        let good = CorrosionState::fresh(600.0, DAY);
        assert_eq!(model.step_checked(&good).unwrap(), model.step(&good));
    }

    /// The accessors report the parts the model was built from.
    #[test]
    fn accessors_expose_the_configured_parts() {
        let model = full_model();
        assert_eq!(
            model.kinetics(),
            Some(OxidationKinetics::EpriKwuCeCathcartPawel)
        );
        assert_eq!(
            model.hydrogen_pickup(),
            HydrogenPickupModel::zircaloy_4(R_INNER, R_OUTER)
        );

        // Replacing the hydrogen model keeps the kinetics.
        let stripped = model.with_hydrogen_pickup(HydrogenPickupModel::None);
        assert_eq!(stripped.kinetics(), model.kinetics());
        assert_eq!(stripped.hydrogen_pickup(), HydrogenPickupModel::None);
        assert_eq!(
            stripped
                .step(&CorrosionState::fresh(600.0, DAY))
                .hydrogen_pickup,
            0.0
        );

        // A single-branch kinetics model can be built directly.
        let low_only = CorrosionModel::ZircaloyOuter {
            kinetics: OxidationKinetics::EpriKwuCe,
            hydrogen: HydrogenPickupModel::None,
        };
        assert_eq!(low_only.kinetics(), Some(OxidationKinetics::EpriKwuCe));
    }
}
