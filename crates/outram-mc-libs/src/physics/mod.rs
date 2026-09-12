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
//! - [`reactor_physics::run_keff_reactor_physics`] — k-eigenvalue **plus**
//!   auto-captured 3-group six-factor decomposition (η, f, p, ε, P_FNL, P_TNL)
//!   and lethargy-normalised flux spectrum, from one combined tally + explicit
//!   leakage accounting. Built on [`transport_csg::run_keff_csg_reactor_physics`].
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
//! # Oracles (independent solutions the drivers are measured against)
//!
//! - [`slowing_down`] — the epithermal slowing-down equation solved
//!   **deterministically**, both for an infinite homogeneous medium
//!   ([`slowing_down::solve_on_grid`], exact) and for a concentric-sphere cell
//!   ([`slowing_down::solve_deterministic_multiregion`], exact but for the
//!   flat-flux-per-shell discretisation). Together they are the reference the
//!   self-shielded resonance absorption is judged against, in energy and in
//!   space.
//! - [`collision_probability`] — the geometric half of that: exact first-flight
//!   collision probabilities for concentric spheres by impact-parameter track
//!   quadrature, with a white-boundary closure.
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
pub mod reactor_physics;
pub mod slowing_down;
pub mod collision_probability;
