//! Walk-on-Spheres / Green's-function first-passage diffusion.
//!
//! This module replaces the single-Gaussian (central-limit-theorem) diffusion
//! step with an **exact, timestep-free** random walk that respects the TRISO
//! layer interfaces. The motivation — why the Gaussian step overshoots the
//! buffer layer — is written up in `docs/buffer_clt_failure_analysis.md`.
//!
//! ## What belongs here
//!
//! - [`sphere_fpt`] — first-passage statistics for 3-D Brownian motion started
//!   at the centre of a sphere: the mean exit time and (added in the CPU
//!   engine) the exit-time sampler and the uniform exit direction.
//! - [`walk_on_spheres`] — the [`walk_on_spheres::WoSWalker`] itself and the
//!   geometry helper [`walk_on_spheres::nearest_interface_distance`] that sizes
//!   each interface-free hop from the concentric-sphere `TrisoCell` geometry.
//! - [`interface`] — the transmission/reflection rule applied when a walker
//!   reaches a layer interface (continuity of concentration and flux, with an
//!   optional partition coefficient).
//! - [`depletion`] — decay and (neutron-field) transmutation as competing
//!   clocks alongside each hop, so the walker changes nuclide as it diffuses;
//!   the ensemble of identities over time is the depleted inventory.
//!
//! ## What does not belong here
//!
//! Diffusion-coefficient correlations (they live in
//! `temperature_dependent_collisions`), the concentric-sphere geometry itself
//! (it lives in `single_particle_simulator::constructive_solid_geometry`), and
//! decay/transmutation bookkeeping (that couples in from
//! `nuclide_reaction_and_decay_data` and the transmutation simulator).
//!
//! The pre-existing Gaussian-step code under `single_particle_simulator` is
//! left in place; this engine is additive and is the intended replacement for
//! the diffusion core.

pub mod depletion;
pub mod interface;
pub mod sphere_fpt;
pub mod walk_on_spheres;
