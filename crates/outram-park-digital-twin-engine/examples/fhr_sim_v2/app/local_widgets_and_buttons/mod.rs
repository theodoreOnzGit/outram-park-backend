//! Widgets still local to this simulator.
//!
//! Everything that proved reusable has moved into the engine's shared
//! component library (bead `op-wqk.8`, step 2):
//!
//! - the FHR reactor vessel is now
//!   [`outram_park_digital_twin_engine::components::FhrReactorVesselVisual`];
//! - the temperature-coloured panel buttons are now
//!   [`outram_park_digital_twin_engine::components::temperature_button`];
//! - the colour maps were always available as
//!   [`outram_park_digital_twin_engine::color_maps`] — the local copies were
//!   duplicates of functions the library already had.
//!
//! Deleted outright rather than migrated, because nothing referenced them:
//! the standalone `reactor_art` drawing routine (superseded by the vessel
//! widget), an empty `PumpWidget`, an unused `VisualSimulationObject` trait,
//! and three `put`/`place` layout helpers that were thin wrappers over
//! `egui::Ui::put` and `Rect::from_center_size`.
//!
//! What remains is what has **not** been migrated yet. Prefer an engine widget
//! over adding anything new here.

/// Temperature- and steam-quality-coloured schematic pipe runs.
///
/// Not yet migrated. The engine's `PipeVisual` is nodalised and colours per
/// finite-volume cell; these draw a single flat-coloured connector between two
/// schematic points and return the end coordinate so runs can be chained.
/// Reconciling the two is outstanding work.
pub mod pipes;

/// Simple rotor drawn at a caller-supplied shaft angle.
///
/// Not yet migrated. The engine's `TurbineVisual` derives rotation from a
/// physics model — a generator's torque balance — and cannot currently be
/// driven from a bare angle. Adding a scalar-fed variant to it would let this
/// module go away.
pub mod turbine_widget;
