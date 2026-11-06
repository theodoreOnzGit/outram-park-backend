//! Visual process object wrappers.
//!
//! ~~**MOVED 2026-11-02 (gh:#1112):** the widget library now lives in
//! `bugis::components` (feature `widgets`), the workspace's central GUI
//! crate; everything there is re-exported here so no call site changes.~~
//!
//! **CORRECTED 2026-11-05** (maintainer: "bugis should be standalone"; then
//! "widget art can live in bugis, the combination of logic and art can live
//! inside the digital twin engine"): the widget library is split.
//!
//! - **The art** stays in [`bugis::components`] (feature `widgets`) and is
//!   re-exported here: every widget that draws from plain numbers (the HTR-10
//!   and FHR vessels and schematics, the pebble packing, legends, bends,
//!   routes, the distillation column, instrumentation, control-rod drive) and
//!   the drawing half of every physics-backed widget (`PipeArt`,
//!   `CondenserArt`, `CoolingTowerArt`, `HeatExchangerArt`, `PumpArt`,
//!   `SteamGeneratorArt`, `TurbineArt`, `ValveArt`, `ReactorVesselArt`).
//! - **Logic + art** lives here: the widgets that hold a `tampines`,
//!   `tampines-steam-tables`, DWSIM or `nee_soon` component
//!   ([`PipeVisual`], [`PipeComponent`], [`ValveVisual`],
//!   [`ReactorVesselVisual`], [`CondenserVisual`], [`CoolingTowerVisual`],
//!   [`HeatExchangerVisual`], [`PumpVisual`], [`SteamGeneratorVisual`],
//!   [`TurbineVisual`]) and the physics halves of the archetype and
//!   heat-exchanger enums ([`ReactorArchetypeKinetics`],
//!   [`HeatExchangerKindArrangement`]). Each reads its physics and draws
//!   through the art type of the same stem.
//!
//! The reason is the release graph: `cargo publish` resolves optional and dev
//! dependencies, and the physics crates dev-depend on bugis for their GUI
//! examples' theme, so bugis's optional physics dependencies were dependency
//! cycles. Every `outram_park_digital_twin_engine::components::…` path that
//! existed before the split still resolves.
//!
//! [`excursion`], the fuel-temperature excursion overlay, never left: it reads
//! the cited Gao & Shi (2002) limit and the generic 1600 degC landmark from
//! [`crate::htr10::design`], which is physics data and this crate's sanctioned
//! exception. The kovan review records for the old paths stay in this folder.

pub use bugis::components::*;

pub mod condenser;
pub mod cooling_tower;
pub mod excursion;
pub mod heat_exchanger;
pub mod pipe;
mod pipe_component;
pub mod pump;
pub mod reactor_archetype;
pub mod reactor_vessel;
pub mod steam_generator;
pub mod turbine;
pub mod valve;

pub use condenser::{CondenserVisual, CondenserVisualState};
pub use cooling_tower::{CoolingTowerScalars, CoolingTowerVisual, CoolingTowerVisualState};
pub use excursion::{ExcursionOverlay, ExcursionStage, ExcursionTrigger};
pub use heat_exchanger::{HeatExchangerKindArrangement, HeatExchangerVisual, HeatExchangerVisualState};
pub use pipe::{PipeVisual, PipeVisualState};
pub use pipe_component::PipeComponent;
pub use pump::PumpVisual;
pub use reactor_archetype::ReactorArchetypeKinetics;
pub use reactor_vessel::ReactorVesselVisual;
pub use steam_generator::{SteamGeneratorPhysicsState, SteamGeneratorVisual, SteamGeneratorVisualState};
pub use turbine::{TurbineVisual, TurbineVisualState};
pub use valve::ValveVisual;

#[cfg(test)]
mod physics_feeds_the_art {
    use super::*;
    use egui::{Pos2, Vec2};

    /// The integration the distillation-column art exists for: a headless
    /// [`CrudePlant`](outram_park_fork_dwsim_libs::petroleum::crude_plant::CrudePlant)
    /// snapshot must feed it directly, with no arithmetic in between. If this
    /// stops compiling, the widget and the plant have drifted apart.
    ///
    /// Moved 2026-11-05 from `bugis::components::distillation_column` (bugis
    /// depends on no physics crate).
    #[test]
    fn a_crude_plant_snapshot_drives_the_widget() {
        use outram_park_fork_dwsim_libs::petroleum::crude_distillation::{
            BlackOilCrude, CrudeColumnConfig,
        };
        use outram_park_fork_dwsim_libs::petroleum::crude_plant::CrudePlant;

        let plant = CrudePlant::new(
            &BlackOilCrude::light_sweet(),
            &CrudeColumnConfig::atmospheric_default(),
            8,
        )
        .expect("the reference crude column must build");
        let snap = plant.snapshot().expect("snapshot");

        let draws: Vec<(usize, &str)> = snap
            .cuts
            .iter()
            .map(|(stage, _, cut)| (*stage, cut.label()))
            .collect();
        let w = DistillationColumnVisual::from_scalars(
            &snap.stage_temperature_k,
            &snap.lightest_liquid_fraction,
            Pos2::new(0.0, 0.0),
            Vec2::new(120.0, 400.0),
        )
        .with_side_draws(&draws);

        assert_eq!(w.stage_temperature_k.len(), snap.n_stages);
        assert_eq!(w.side_draws.len(), 5, "overhead + 3 draws + residue");
    }
}
