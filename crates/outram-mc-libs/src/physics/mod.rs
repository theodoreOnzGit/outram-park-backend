//! Neutron-transport physics: the collision kernels and the top-level drivers
//! that iterate them over a geometry.
//!
//! # Transport drivers (what runs a whole calculation)
//!
//! - [`keff::run_keff`] — k-eigenvalue power iteration for a homogeneous bare
//!   sphere (the reference criticality driver; `CPU`/`GPU` backends).
//! - [`transport_csg::run_keff_csg`] — k-eigenvalue power iteration over
//!   **general CSG geometry** (surfaces/cells/universes/lattices), with track-
//!   length tallies. Generalises `keff`.
//! - [`fixed_source::run_fixed_source`] — **fixed-source** transport: an
//!   external neutron source (point/box) driving a sub-critical or
//!   non-multiplying system, scoring track-length tallies. No `k_eff` / power
//!   iteration; the second canonical MC mode (shielding / detector response).
//!   Reuses [`transport_csg::transport_history`] for the per-history physics.
//! - [`physics_mg`] — multigroup transport (group-averaged cross sections;
//!   pending / partial).
//!
//! # Collision-level kernels (the per-collision physics the drivers call)
//!
//! - [`scatter`] — elastic / inelastic scattering, centre-of-mass kinematics.
//! - [`fission`] — ν̄ sampling and fission-site banking.
//! - [`compute`] — [`compute::ComputeType`] backend selector shared by the
//!   drivers (single-thread / multi-thread / GPU).
//! - [`search`] — reactivity search wrapping the k-eigenvalue driver (root-find
//!   a geometry/material parameter for a target `k_eff`).
//!
//! [`transport`] is a stub retained for the generic history-based loop notes;
//! the live per-history loop is in [`transport_csg`].

pub mod compute;
pub mod transport;
pub mod transport_csg;
pub mod fixed_source;
pub mod scatter;
pub mod fission;
pub mod keff;
pub mod search;
pub mod physics_mg;
