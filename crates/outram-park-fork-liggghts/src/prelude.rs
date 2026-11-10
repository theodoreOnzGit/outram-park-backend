// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK. See `src/lib.rs` for the full licence
// notice. It only re-exports items defined elsewhere in this crate; it ports
// no upstream source.

//! Convenience re-exports: the intended entry point for this crate (gh #64).
//!
//! ```
//! use outram_park_fork_liggghts::prelude::*;
//! ```
//!
//! This brings into scope every type on the critical path to running a DEM
//! simulation, so a caller does not need to know which module an item lives
//! in:
//! - particles;
//! - analytic and mesh walls;
//! - the contact laws;
//! - the two engines: [`GranularSystem`] (the full LIGGGHTS-style granular
//!   model with shear history, rolling resistance and moving walls) and
//!   [`DemSimulation`] (the simpler Hooke/Hertz engine);
//! - the compute backend;
//! - thermal and radiative exchange;
//! - bonds;
//! - the time-step estimate;
//! - the radial distribution function;
//! - the HTR-10 fill driver;
//! - the crate error.
//!
//! **A public type reachable only by its full module path is not considered
//! exposed.** If you add one on the critical path, add it here in the same
//! change.
//!
//! What it leaves out, deliberately:
//! - **`rolling::RollingModel`.** It shares its name with
//!   [`granular::RollingModel`](crate::granular::RollingModel), the rolling
//!   resistance [`GranularContactModel::with_rolling`] takes, which is the one
//!   exported here. The older `rolling` module stays reachable by its path.
//! - **Free functions** of the `thermal`, `thermal_radiation` and `timestep`
//!   modules. Call them through their module (for example
//!   `outram_park_fork_liggghts::timestep::estimate`). Their model types
//!   ([`ThermalModel`], [`RadiationModel`], [`TimestepEstimate`]) are here.
//! - **The `gpu` and `gnn_bridge` modules.** These are platform- or
//!   feature-gated (`gpu` is off on Android; `gnn_bridge` needs the `gnn`
//!   feature), and a prelude that changes with the target would break the
//!   "one import" promise.
//!
//! The completeness test is executable. The crate's examples import from it,
//! and the example below runs a small settle through it alone:
//!
//! ```
//! use outram_park_fork_liggghts::prelude::*;
//! use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
//! use uom::si::length::meter;
//! use uom::si::mass::kilogram;
//! use uom::si::thermodynamic_temperature::kelvin;
//!
//! // Two graphite-like pebbles stacked above a floor, inside a cylinder.
//! let r = 0.03;
//! let particles: Vec<Particle> = [0.05, 0.12]
//!     .iter()
//!     .map(|z| {
//!         Particle::new(
//!             Vec3::new(0.0, 0.0, *z),
//!             Vec3::zero(),
//!             Vec3::zero(),
//!             Mass::new::<kilogram>(0.2),
//!             Length::new::<meter>(r),
//!             ThermodynamicTemperature::new::<kelvin>(300.0),
//!         )
//!     })
//!     .collect::<Result<_, DemError>>()?;
//! let boundaries = vec![
//!     Boundary::cylinder(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0), 0.2)?,
//!     Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0))?,
//! ];
//! let material = GranularMaterial::new(5.0e8, 0.2, 0.5, 0.4)?;
//! let model = GranularContactModel::hertz_history(material)
//!     .with_rolling(RollingModel::cdt(0.1)?);
//! let mut sys = GranularSystem::new(particles, boundaries, model, Vec3::new(0.0, 0.0, -9.81), 1.0e-5)?
//!     .with_compute(ComputeType::CpuSingleThread);
//! sys.run(2000);
//!
//! // Nothing falls through the floor.
//! assert!(sys.particles().iter().all(|p| p.position.z > 0.0));
//! # Ok::<(), DemError>(())
//! ```

// The crate error.
pub use crate::DemError;

// Particles and the 3-vector every API takes.
pub use crate::particle::{Particle, Vec3};

// Walls: analytic primitives, triangulated meshes, and moving walls.
pub use crate::boundary::{Boundary, Contact};
pub use crate::mesh_wall::{MeshWall, MovingBoundary, Triangle, WallGeometry};

// The full granular contact model used by `GranularSystem`.
pub use crate::granular::{
    ContactKinematics, GranularContactModel, GranularForce, GranularMaterial, GranularNormalModel,
    RollingModel, TangentialModel,
};

// The simpler contact laws used by `DemSimulation`.
pub use crate::contact::{ContactForce, ContactLaw, ContactModel, HertzContact, HookeContact};

// The two engines, their integrator and the compute backend.
pub use crate::compute::{ComputeType, ThreadCount};
pub use crate::granular_system::GranularSystem;
pub use crate::integrator::VelocityVerlet;
pub use crate::simulation::DemSimulation;

// Choosing a stable time step.
pub use crate::timestep::TimestepEstimate;

// Bonded particles, cohesion and rolling torque.
pub use crate::bonded::{Bond, BondForce, BondModel};
pub use crate::rolling::{CohesionModel, RollingTorque};

// Heat transfer between particles and walls.
pub use crate::thermal::ThermalModel;
pub use crate::thermal_radiation::RadiationModel;

// Fluid coupling interface (the CFD-DEM seam).
pub use crate::coupling::{
    CouplingExchange, CouplingScheme, DemCouplingResponse, FluidCouplingSource, LocalFluidState,
};

// Bed structure: the radial distribution function.
pub use crate::rdf::{radial_distribution, Rdf, RdfDomain, RdfSettings};

// The HTR-10 fresh-core fill driver.
pub use crate::htr10_fill::{FillProgress, Htr10Fill, Htr10FillSettings};
