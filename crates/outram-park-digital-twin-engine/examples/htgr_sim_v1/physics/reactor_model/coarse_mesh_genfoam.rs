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
//! **`outram-foam-appbuilder-lib` is not yet a dependency of
//! `outram-park-digital-twin-engine`** (checked 2026-08-16: no such line in
//! this crate's `Cargo.toml`). Adding it, and everything that implies for
//! build time and the Android/Termux portability rule (`genfoam` has not been
//! audited against that rule from this crate's side), is a separate decision
//! from wiring the *selection* of this tier into the plant, which is what
//! this change delivers. The imports above are therefore cited in prose only
//! -- not as real `use` statements -- until that dependency is deliberately
//! added.
//!
//! Wiring in real GeN-Foam physics ahead of that groundwork would also
//! contradict the workspace's own "search before building" rule the other
//! direction: `genfoam` is itself flagged as an untested draft, so composing
//! it into this simulator needs the same V&V scrutiny `outram-park-digital-twin-engine`'s
//! own `CLAUDE.md` records for this plant's other physics, not a quiet import.
//!
//! Falling back to [`super::one_node`] keeps
//! [`ReactorModelKind::CoarseMeshGenFoam`](super::ReactorModelKind::CoarseMeshGenFoam)
//! selectable and exercised by the enum's own tests now, without pulling in
//! an unaudited dependency or an unvalidated coupling ahead of either being
//! ready.

use super::one_node::PebbleBedCore;
use uom::si::f64::{MassRate, Power, ThermodynamicTemperature, Time};

/// Placeholder coarse-mesh GeN-Foam core. Currently a thin wrapper around
/// [`PebbleBedCore`] -- every method below delegates to it unmodified, so
/// selecting this tier changes nothing about the plant's numbers yet. See the
/// module doc comment for the design a real implementation would replace this
/// with.
#[derive(Clone, Copy, Debug)]
pub struct CoarseMeshGenFoamCore {
    /// The one-node fallback this placeholder currently *is*. Not a GeN-Foam
    /// mesh region set coupled to a `NordheimFuchsExactTimestepper` -- that is
    /// exactly what implementing this tier for real would replace this field
    /// with.
    fallback: PebbleBedCore,
}

impl CoarseMeshGenFoamCore {
    /// Construct at the same cold-start seed [`PebbleBedCore::new`] uses.
    pub fn new() -> Self {
        Self {
            fallback: PebbleBedCore::new(),
        }
    }

    /// Delegates to [`PebbleBedCore::step`] -- see the module doc comment for
    /// the GeN-Foam + Nordheim-Fuchs coupling a real implementation would do
    /// instead.
    pub fn step(
        &mut self,
        dt: Time,
        fission_power: Power,
        helium_inlet_temperature: ThermodynamicTemperature,
        helium_mass_flow: MassRate,
    ) -> Power {
        self.fallback
            .step(dt, fission_power, helium_inlet_temperature, helium_mass_flow)
    }

    /// Delegates to [`PebbleBedCore::temperature`]. A real implementation
    /// would need to decide what single number this reports from a
    /// multi-region mesh -- most likely a volume-weighted average, to stay
    /// comparable with the other tiers' bed-average convention.
    pub fn temperature(&self) -> ThermodynamicTemperature {
        self.fallback.temperature()
    }

    /// Delegates to [`PebbleBedCore::helium_outlet_temperature`] -- in a real
    /// implementation this becomes the mesh's outlet-boundary helium
    /// temperature.
    pub fn helium_outlet_temperature(&self) -> ThermodynamicTemperature {
        self.fallback.helium_outlet_temperature()
    }
}

impl Default for CoarseMeshGenFoamCore {
    fn default() -> Self {
        Self::new()
    }
}
