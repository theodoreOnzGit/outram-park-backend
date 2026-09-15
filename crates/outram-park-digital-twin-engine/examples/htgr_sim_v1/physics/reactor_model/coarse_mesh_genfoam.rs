//! Coarse-mesh GeN-Foam pebble-bed core -- **placeholder**, falls back to
//! [`super::one_node`].
//!
//! ## What this tier is for
//!
//! [`super::one_node`] and the scaffolded
//! [`super::axial_seven_node`] are both 1-D lumps: at most an axial profile,
//! never a radial one, and never a coupled multi-region neutronics solve --
//! point kinetics stays a single reactivity/power pair regardless of tier.
//! This is meant to be the tier above both of those: a genuinely
//! multi-dimensional, coarse-mesh core, reusing this workspace's own
//! deterministic-neutronics + thermal-hydraulics port rather than writing a
//! new one.
//!
//! ## Scaffolded design (not implemented)
//!
//! The real target is `outram_foam_appbuilder_lib::genfoam`, specifically its
//! `neutronics`, `thermal_hydraulics` and `multi_region` submodules (confirmed
//! present in that crate's `genfoam/mod.rs` as of 2026-08-16; ~32k lines /
//! ~262 tests per the workspace root `CLAUDE.md`, itself flagged there as an
//! AI-assisted draft with no human V&V). A real implementation of this tier
//! would:
//!
//! - build a coarse multi-region mesh over the HTR-10 bed (order-10 cells,
//!   not the 27,000-pebble discrete geometry -- "coarse" is the point: coarse
//!   enough to resolve axial *and* radial structure cheaply, coarser than a
//!   pebble-resolved DEM/CFD case would need);
//! - couple `genfoam::neutronics` (multi-region diffusion) to
//!   `genfoam::thermal_hydraulics` on that mesh, replacing point kinetics'
//!   single reactivity/power pair with a spatial power distribution;
//! - **but step the reactivity/power feedback with
//!   [`teh_o_prke::nordheim_fuchs::NordheimFuchsExactTimestepper`]
//!   (confirmed real at `crates/teh-o-prke/src/nordheim_fuchs.rs`) instead of
//!   GeN-Foam's own point-kinetics coupling** -- this is the specific design
//!   choice the maintainer asked for this tier to scaffold. Nordheim-Fuchs's
//!   closed-form exact integration is what keeps a stiff prompt-excursion
//!   feedback term non-stiff (see `kinetics.rs`'s own doc comment on why this
//!   plant's point-kinetics layer already relies on that exactness); reusing
//!   it here would mean the coarse mesh's *spatial* power shape comes from
//!   GeN-Foam while the *time* integration of the reactivity feedback still
//!   comes from the exact solver, rather than GeN-Foam's own (presumably
//!   less exact) transient coupling. `nee_soon::NeeSoon::new_prompt_excursion_model`
//!   is the existing precedent for composing `NordheimFuchsExactTimestepper`
//!   into a plant model, and is the pattern to follow here.
//!
//! ## Why this is a placeholder today, and a dependency note
//!
//! **`outram-foam-appbuilder-lib` is now a dev-dependency of
//! `outram-park-digital-twin-engine`** (wired in 2026-08-17, scoped to
//! examples via `[dev-dependencies]` -- the crate's own CLAUDE.md keeps new
//! physics out of `src/`, and this tier belongs to `htgr_sim_v1` specifically,
//! same as every other example-only physics dependency in that Cargo.toml
//! section). That makes real `genfoam` types importable from this file; it
//! does **not** by itself make the coupling below real. This module still
//! only *cites* `genfoam::{neutronics, thermal_hydraulics, multi_region}` and
//! `NordheimFuchsExactTimestepper` in prose, and [`CoarseMeshGenFoamCore`]
//! still falls back to [`super::one_node`]'s math unmodified -- adding the
//! dependency was scoped separately from implementing the coupling, which
//! remains a distinct, sizeable piece of work.
//!
//! Wiring in real GeN-Foam physics ahead of that work would also contradict
//! the workspace's own "search before building" rule the other direction:
//! `genfoam` is itself flagged as an untested draft, so composing it into
//! this simulator needs the same V&V scrutiny `outram-park-digital-twin-engine`'s
//! own `CLAUDE.md` records for this plant's other physics, not a quiet
//! coupling. The Android/Termux portability rule is also not yet addressed
//! here: `outram-foam-appbuilder-lib` has not been audited against that rule
//! from this crate's side, so a real implementation of this tier should check
//! it before this dev-dependency becomes load-bearing rather than merely
//! available.
//!
//! Falling back to [`super::one_node`]'s [`PebbleBedPorousMediaNode`] (the
//! more physically complete of the two real tiers, and the plant's own
//! default since 2026-08-17) keeps
//! [`ReactorModelKind::CoarseMeshGenFoam`](super::ReactorModelKind::CoarseMeshGenFoam)
//! selectable and exercised by the enum's own tests now, without shipping an
//! unvalidated coupling ahead of it being real.

use super::one_node::PebbleBedPorousMediaNode;
use uom::si::f64::{MassRate, Power, ThermodynamicTemperature, Time};
use uom::si::power::watt;

/// Placeholder coarse-mesh GeN-Foam core. Currently a thin wrapper around
/// [`PebbleBedPorousMediaNode`] -- every method below delegates to it
/// unmodified, so selecting this tier changes nothing about the plant's
/// numbers yet. See the module doc comment for the design a real
/// implementation would replace this with.
#[derive(Clone, Copy, Debug)]
pub struct CoarseMeshGenFoamCore {
    /// The one-node fallback this placeholder currently *is*. Not a GeN-Foam
    /// mesh region set coupled to a `NordheimFuchsExactTimestepper` -- that is
    /// exactly what implementing this tier for real would replace this field
    /// with.
    fallback: PebbleBedPorousMediaNode,
}

impl CoarseMeshGenFoamCore {
    /// Construct at the same cold-start seed [`PebbleBedPorousMediaNode::new`] uses.
    pub fn new() -> Self {
        Self {
            fallback: PebbleBedPorousMediaNode::new(),
        }
    }

    /// Delegates to [`PebbleBedPorousMediaNode::step`] -- see the module doc
    /// comment for the GeN-Foam + Nordheim-Fuchs coupling a real
    /// implementation would do instead.
    ///
    /// `fission_power` here is the caller-summed reactor thermal power (see
    /// [`super::ReactorModel::step`]'s doc comment); it is passed through as
    /// [`PebbleBedPorousMediaNode::step`]'s `fission_power` argument with a
    /// zero `decay_heat_power`, the same fold-in [`super::ReactorModel::step`]
    /// performs for its own `OneNodePorousMedia` arm.
    pub fn step(
        &mut self,
        dt: Time,
        fission_power: Power,
        helium_inlet_temperature: ThermodynamicTemperature,
        helium_mass_flow: MassRate,
    ) -> Power {
        self.fallback.step(
            dt,
            fission_power,
            Power::new::<watt>(0.0),
            helium_inlet_temperature,
            helium_mass_flow,
        )
    }

    /// Delegates to [`PebbleBedPorousMediaNode::pebble_temperature`]. A real
    /// implementation would need to decide what single number this reports
    /// from a multi-region mesh -- most likely a volume-weighted average, to
    /// stay comparable with the other tiers' bed-average convention.
    pub fn temperature(&self) -> ThermodynamicTemperature {
        self.fallback.pebble_temperature()
    }

    /// Delegates to [`PebbleBedPorousMediaNode::helium_temperature`] -- in a
    /// real implementation this becomes the mesh's outlet-boundary helium
    /// temperature.
    pub fn helium_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.fallback.helium_temperature()
    }
}

impl Default for CoarseMeshGenFoamCore {
    fn default() -> Self {
        Self::new()
    }
}
