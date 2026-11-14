// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/pair_gran_base.h (contact loop ordering),
//                      src/fix_wall_gran.cpp / src/fix_wall_gran_base.h
//                      (primitive-wall contacts), src/verlet.cpp (step order)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0.
//   Upstream copyright: Copyright 2012- DCS Computing GmbH, Linz;
//                       Copyright 2009-2012 JKU Linz.
// Do not strip this block during refactors (workspace CLAUDE.md).
//
// This file is part of OUTRAM PARK.
//
// OUTRAM PARK is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the
// Free Software Foundation, either version 3 of the License, or (at your
// option) any later version.
//
// OUTRAM PARK is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License along
// with OUTRAM PARK.  If not, see <https://www.gnu.org/licenses/>.

//! # History-aware DEM driver (LIGGGHTS `run` loop)
//!
//! Composes [`crate::granular`] (contact law + shear history),
//! [`crate::integrator`] (velocity-Verlet), and [`crate::boundary`] (wall
//! primitives) into a runnable simulation that reproduces LIGGGHTS' step
//! ordering:
//!
//! ```text
//!   for each step:
//!     initial_integrate   (half-kick with F(t), then drift)
//!     compute forces      (pair contacts, then wall contacts, then gravity)
//!     final_integrate     (half-kick with F(t+dt))
//! ```
//!
//! ## Relationship to [`crate::simulation::DemSimulation`]
//!
//! [`crate::simulation::DemSimulation`] is the original stateless engine: it
//! uses [`crate::contact`] (no shear history) and
//! [`crate::particle::Particle::integrate`] (not symplectic — see
//! [`crate::integrator`]). It is kept for the cases it was written and tested
//! for, and because its neighbour-search code is shared. **For anything that
//! must settle, pack, or hold a static assembly — a pebble bed — use
//! [`GranularSystem`].**
//!
//! ## Honest scope
//!
//! Single material; primitive [`Boundary`] walls only (no triangulated meshes —
//! see [`crate::mesh_wall`]); no cohesion or rolling models wired in; uniform
//! gravity; fixed time step; serial. Neighbour candidates come from an
//! all-pairs scan below [`GranularSystem::BRUTE_FORCE_THRESHOLD`] and a
//! uniform linked-cell grid above it.

use crate::force_fixes::ForceFix;
use crate::variable::VarContext;
use petir::mathf::RealMath;
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
#[cfg(target_arch = "wasm32")]
use crate::wasm_par as rayon;
#[cfg(target_arch = "wasm32")]
use crate::wasm_par::prelude::*;

use crate::boundary::Boundary;
use crate::compute::ComputeType;
use crate::granular::{
    ContactKey, ContactKinematics, GranularCohesion, GranularContactModel, ShearHistory, MAX_TYPES,
};
use crate::heat_gran::HeatGranConduction;
use crate::integrator::VelocityVerlet;
use crate::mesh_stress::WearContact;
use crate::mesh_wall::{MovingBoundary, WallGeometry};
use crate::neighbor_list::{self, LiggghtsNeighbor, NeighborOrder};
use crate::particle::{Particle, Vec3};
use crate::tri_mesh::arr;
use crate::DemError;

/// One `post_force` fix of the wall/gravity/force-fix family, for
/// [`GranularSystem::with_fix_order`] (gh:#1482).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixSlot {
    /// `boundaries()[i]`, a `fix wall/gran ... primitive` wall.
    PrimitiveWall(usize),
    /// `moving_walls()[i]`, a mesh or analytic moving wall.
    MovingWall(usize),
    /// The `fix gravity`.
    Gravity,
    /// `force_fixes()[i]`.
    ForceFix(usize),
    /// The `post_force` of massflow counter `i` that is a
    /// `fix massflow/mesh/sieve` (see [`GranularSystem::add_massflow_mesh`]).
    MassflowSieve(usize),
}

/// Resolve one contact: the ordinary history-advancing law, or at set-up the
/// frozen law that only registers the contact (`shearupdate = 0`).
fn resolve_c(
    model: &GranularContactModel,
    setup: bool,
    key: ContactKey,
    k: &ContactKinematics,
    history: &mut ShearHistory,
    dt: f64,
) -> crate::granular::GranularForce {
    if setup {
        model.resolve_setup(key, k, history, dt)
    } else {
        model.resolve(key, k, history, dt)
    }
}

/// One row of upstream's `compute wall/gran/local` (default column set `pos
/// vel id force torque history contactArea delta`): see
/// [`GranularSystem::wall_gran_local`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallLocalRecord {
    /// Contact point on the mesh `x - delta` `[m]` (`add_wall_1`'s `pos`).
    pub contact_point: Vec3,
    /// Position of the particle `[m]`.
    pub pos_i: Vec3,
    /// Wall velocity at the contact point `[m/s]`.
    pub wall_vel: Vec3,
    /// Particle velocity `[m/s]`.
    pub vel_i: Vec3,
    /// Index of the mesh among the mesh walls (`iFMG`).
    pub mesh: usize,
    /// Triangle (element) id.
    pub tri: usize,
    /// Tag of the particle (0-based; upstream's id is this plus one).
    pub tag: u64,
    /// Contact force on the particle `[N]`.
    pub force: Vec3,
    /// Contact torque on the particle `[N m]`.
    pub torque: Vec3,
    /// Stored tangential displacement of the contact `[m]`.
    pub history: Vec3,
    /// `(r_i^2 - r^2) pi` with `r = r_i - overlap` `[m^2]`.
    pub contact_area: f64,
    /// `r_i - r` `[m]`.
    pub delta: f64,
}

/// One row of upstream's `compute pair/gran/local`: see
/// [`GranularSystem::pair_gran_local_for`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairLocalRecord {
    /// Tag of particle `i` (0-based; upstream's atom id is this plus one).
    pub tag_i: u64,
    /// Tag of particle `j`.
    pub tag_j: u64,
    /// Position of `i` `[m]`.
    pub pos_i: Vec3,
    /// Position of `j` `[m]`.
    pub pos_j: Vec3,
    /// Velocity of `i` `[m/s]`.
    pub vel_i: Vec3,
    /// Velocity of `j` `[m/s]`.
    pub vel_j: Vec3,
    /// Contact force on `i` `[N]`.
    pub force: Vec3,
    /// Contact torque on `i` `[N m]`.
    pub torque: Vec3,
    /// Stored tangential shear displacement `[m]` (upstream `history`).
    pub history: Vec3,
    /// Contact area `[m^2]`.
    pub contact_area: f64,
    /// Overlap `r_i + r_j - |x_i - x_j|` `[m]`.
    pub delta: f64,
}

/// A runnable DEM simulation with **persistent tangential shear history**.
///
/// # Fields and units
///
/// | Field | Quantity | SI unit |
/// |---|---|---|
/// | `particles` | sphere ensemble | mixed (see [`Particle`]) |
/// | `boundaries` | primitive walls | mixed (see [`Boundary`]) |
/// | `model` | contact law (normal + tangential) | — |
/// | `history` | per-contact tangential displacement `ξ_t` | `[m]` |
/// | `gravity` | uniform gravitational acceleration | `[m/s²]` |
/// | `dt` | fixed velocity-Verlet step | `[s]` |
/// | `time` | elapsed simulated time | `[s]` |
#[derive(Debug, Clone, PartialEq)]
pub struct GranularSystem {
    particles: Vec<Particle>,
    boundaries: Vec<Boundary>,
    /// Kinematically-prescribed moving walls (upstream `fix move/mesh`), each
    /// wrapping an analytic primitive or a triangulated mesh. Advanced once per
    /// step, in the drift half of the velocity-Verlet cycle.
    moving_walls: Vec<MovingBoundary>,
    /// Neighbour skin `[m]` whose rebuild schedule decides when a rotating
    /// mesh's cached edge vectors and normals are recomputed from its nodes
    /// (upstream refreshes them only on a neighbour-list rebuild and rotates
    /// them incrementally in between). `0` (default) recomputes after every
    /// rotation. See [`GranularSystem::with_neighbor_skin`].
    neigh_skin: f64,
    /// `fix multicontact/halfspace` + `surface_model multicontact` (opt-in).
    mc: Option<crate::multicontact::MulticontactState>,
    /// `post_force` fixes (`addforce`, `viscous`, `freeze`), applied after gravity
    /// in declaration order (gh:#1418).
    force_fixes: Vec<ForceFix>,
    /// Declared order of wall/gravity/force fixes (`None`: the default order),
    /// gh:#1482.
    fix_order: Option<Vec<FixSlot>>,
    /// `fix freeze` group per particle (pair `meff`, gh:#1478).
    frozen: Vec<bool>,
    /// True during the set-up force evaluation, where upstream does not
    /// advance the shear history (`update->setupflag` -> `shearupdate = 0`).
    setup_phase: bool,
    /// Particle positions at the last emulated neighbour rebuild (`xhold`).
    xhold: Vec<Vec3>,
    /// A mesh asked for a rebuild on the next step (`next_reneighbor`).
    mesh_rebuild_pending: bool,
    /// Per `(particle tag, wall)` mesh-facet contact partners with their
    /// keep flags, in upstream's slot order (`FixContactHistoryMesh`).
    mesh_partners: HashMap<(usize, usize), Vec<Option<(u32, bool)>>>,
    model: GranularContactModel,
    history: ShearHistory,
    integrator: VelocityVerlet,
    gravity: Vec3,
    dt: f64,
    time: f64,
    forces: Vec<Vec3>,
    torques: Vec<Vec3>,
    /// Stable per-particle tag, parallel to `particles`.
    ///
    /// Upstream's atom tag (`fix_contact_history.cpp:393` stores
    /// `partner_[i][m] = tag[j]`). The shear history is keyed by this, never by
    /// the array index, so removing or inserting a particle cannot silently
    /// re-attach a stored tangential spring to a different pair.
    tags: Vec<u64>,
    /// Atom type of each particle (0-based), parallel to `particles`. All zero
    /// unless set by [`GranularSystem::with_particle_types`]; indexes the
    /// model's [`MaterialTable`](crate::granular::MaterialTable).
    types: Vec<usize>,
    /// Next tag to hand out. Monotonic; a tag is never reused.
    next_tag: u64,
    /// Which compute backend the timestep uses. See [`ComputeType`]; the
    /// default is the scalar, trusted reference.
    compute: ComputeType,
    /// Reused candidate-pair buffer, rebuilt each step by
    /// [`GranularSystem::rebuild_candidate_pairs`].
    ///
    /// Held across steps so the ~370 000-entry list at HTR-10 scale is not
    /// reallocated 28 000 times a second.
    pairs: Vec<(u32, u32)>,
    /// Reused per-candidate contact outcomes for the parallel backend — see
    /// [`GranularSystem::compute_forces`]. Empty on the scalar path, which
    /// accumulates directly and needs no staging buffer.
    outcomes: Vec<PairOutcome>,
    /// How many outcomes each parallel chunk actually wrote — see
    /// [`GranularSystem::resolve_pairs_parallel`].
    chunk_counts: Vec<u32>,
    /// Reused cell-offset array (prefix sums) for the neighbour grid.
    cell_offsets: Vec<u32>,
    /// Reused cell-membership array: particle indices, grouped by cell,
    /// ascending within each cell.
    cell_items: Vec<u32>,
    /// How candidate pairs are ordered. See [`NeighborOrder`].
    neighbor_order: NeighborOrder,
    /// `NeighborOrder::Liggghts` only: `perm[l]` is the particle index sitting
    /// in upstream's local atom slot `l` (the `Atom::sort` order).
    perm: Vec<usize>,
    /// `NeighborOrder::Liggghts` only: per particle, its neighbour bin
    /// `(ix, iy, iz)` at the last list build (`Neighbor::bin_atoms` on the
    /// positions of that build). Fixes the order of each facet's mesh
    /// neighbour list, which walks its bins in `ix, iy, iz` loop order
    /// (`FixNeighlistMesh::generate_bin_list`/`handleTriangle`, gh:#1491).
    nbin_of: Vec<(i32, i32, i32)>,
    /// Upstream's `update->ntimestep` (steps taken by this system).
    step_count: u64,
    /// `atom->nextsort`.
    next_sort: u64,
    /// The neighbour list must be rebuilt in the next force evaluation
    /// (`Neighbor::decide` returned 1, or the particle set changed).
    rebuild_now: bool,
    /// Primitive-wall neighbour lists of the last rebuild, `[wall][particle]`
    /// (`PrimitiveWall::buildNeighList`). Empty until a LIGGGHTS-order rebuild;
    /// only the multicontact `surfacesClose` history reads it (gh:#1410).
    wall_near: Vec<Vec<bool>>,
    /// `fix insert/*` fixes, run at `pre_exchange` (gh:#1414).
    inserters: Vec<crate::insert::InsertFix>,
    /// `fix insert/stream` fixes and the shared release data (by tag).
    stream_inserters: Vec<crate::insert_stream::InsertStreamFix>,
    stream_release: std::collections::BTreeMap<u64, crate::insert_stream::Release>,
    /// `fix heat/gran/conduction`, if attached ([`GranularSystem::with_heat_conduction`]).
    heat: Option<HeatGranConduction>,
    /// Heat flux into each particle `[W]` from the last step's contact
    /// conduction (upstream's `heatFlux` per-atom property).
    heat_flux: Vec<f64>,
    /// `fix massflow/mesh` counters, called after the particles move each step.
    massflow: Vec<crate::massflow::MassFlowMesh>,
    /// The `liquidtransfer` state of a capillary cohesion model, if attached
    /// ([`GranularSystem::with_capillary`], gh:#1411).
    capillary: Option<crate::capillary::CapillaryState>,
    /// `fix multisphere`: created when the first clump is inserted (gh:#1415).
    multisphere: Option<crate::multisphere::Multisphere>,
    /// The clump fix is `fix multisphere/nointegration` (gh:#1415).
    ms_nointegration: bool,
    /// `boundary f f f` with `thermo_modify lost ignore`: the box whose
    /// leavers `Comm::exchange` drops at a rebuild
    /// ([`GranularSystem::with_lost_atoms`], gh:#1415).
    lost_atoms_box: Option<(Vec3, Vec3)>,
}

/// Where a capillary mesh-facet `surfacesClose` happens (gh:#1487): the
/// moving-wall index, the bridge key `(tag, wall id, facet)`, the particle
/// index and type, the facet velocity at the contact and the facet area.
#[derive(Debug, Clone, Copy)]
struct MeshCloseSite {
    wall: usize,
    key: (u64, usize, usize),
    i: usize,
    atype: usize,
    v_wall: Vec3,
    area: f64,
    dt: f64,
}

/// One candidate pair's resolved contact, staged by the parallel force loop so
/// that accumulation can happen afterwards in the serial pair order.
///
/// `force_j` is omitted deliberately: it is exactly `-force_i` (Newton's third
/// law, and how [`GranularForce`] is built), so staging it would be 24 bytes
/// per candidate of redundancy across a ~370 000-entry buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
struct PairOutcome {
    i: u32,
    j: u32,
    force_i: Vec3,
    torque_i: Vec3,
    torque_j: Vec3,
    /// Tangential displacement to store for this contact.
    ///
    /// Carried unconditionally rather than as an `Option`: whether a contact
    /// has history is a property of the *model*, not of the contact, so the
    /// discriminant would be the same for all 66 500 of them and cost 8 bytes
    /// each to say so. Phase 2 consults the model once instead.
    shear: Vec3,
}

impl PairOutcome {
    /// Filler for buffer growth. Never read: phase 2 visits only the first
    /// `count` entries each chunk actually wrote.
    const EMPTY: Self = Self {
        i: 0,
        j: 0,
        force_i: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        torque_i: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        torque_j: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
        shear: Vec3 {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        },
    };
}

/// One particle-facet contact's contribution to `fix mesh/surface/stress`
/// (`add_particle_contribution(ip, f_pw, -sidata.delta, iTri, v_wall)`),
/// staged so the totals can be summed in upstream's order.
struct StressContribution {
    /// Particle index.
    i: usize,
    /// Facet.
    tri: usize,
    /// `f_pw`, the contact force on the particle.
    f_pw: [f64; 3],
    /// Particle centre.
    x: [f64; 3],
    /// `-sidata.delta`.
    delta: [f64; 3],
    /// Particle velocity.
    v: [f64; 3],
    /// Mesh velocity at the contact.
    v_wall: [f64; 3],
    /// Unit normal of the facet (`surfaceNorm`).
    norm: [f64; 3],
}

impl GranularSystem {
    /// Ensembles of this size or smaller use an all-pairs neighbour scan.
    ///
    /// Same heuristic and rationale as
    /// [`crate::simulation::DemSimulation::BRUTE_FORCE_THRESHOLD`]; both paths
    /// enumerate the same contacts.
    pub const BRUTE_FORCE_THRESHOLD: usize = 64;

    /// Build a simulation.
    ///
    /// `gravity` is the uniform acceleration `[m/s²]` (use [`Vec3::zero`] for
    /// none); `dt` is the fixed step `[s]`. The initial force/torque arrays are
    /// evaluated immediately so that the first
    /// [`GranularSystem::step`] half-kick uses `F(0)`, matching LIGGGHTS'
    /// `setup()`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `dt` is not finite and strictly positive.
    pub fn new(
        particles: Vec<Particle>,
        boundaries: Vec<Boundary>,
        model: GranularContactModel,
        gravity: Vec3,
        dt: f64,
    ) -> Result<Self, DemError> {
        let integrator = VelocityVerlet::new(dt)?;
        let n = particles.len();
        let mut s = Self {
            particles,
            boundaries,
            moving_walls: Vec::new(),
            neigh_skin: 0.0,
            mc: None,
            force_fixes: Vec::new(),
            fix_order: None,
            frozen: Vec::new(),
            setup_phase: false,
            xhold: Vec::new(),
            mesh_rebuild_pending: false,
            mesh_partners: HashMap::new(),
            model,
            history: ShearHistory::new(),
            integrator,
            gravity,
            dt,
            time: 0.0,
            forces: vec![Vec3::zero(); n],
            torques: vec![Vec3::zero(); n],
            tags: (0..n as u64).collect(),
            types: vec![0; n],
            next_tag: n as u64,
            compute: ComputeType::default(),
            pairs: Vec::new(),
            outcomes: Vec::new(),
            chunk_counts: Vec::new(),
            cell_offsets: Vec::new(),
            cell_items: Vec::new(),
            neighbor_order: NeighborOrder::CellGrid,
            perm: Vec::new(),
            nbin_of: Vec::new(),
            step_count: 0,
            next_sort: 0,
            rebuild_now: false,
            wall_near: Vec::new(),
            inserters: Vec::new(),
            stream_inserters: Vec::new(),
            stream_release: std::collections::BTreeMap::new(),
            heat: None,
            heat_flux: Vec::new(),
            massflow: Vec::new(),
            capillary: None,
            multisphere: None,
            ms_nointegration: false,
            lost_atoms_box: None,
        };
        s.compute_forces_setup();
        Ok(s)
    }

    /// Attach kinematically-prescribed **moving walls** (upstream
    /// `fix move/mesh`), returning the updated system.
    ///
    /// Each [`MovingBoundary`] wraps either an analytic primitive or a
    /// triangulated [`crate::mesh_wall::MeshWall`], and carries a translational
    /// and angular velocity. They are advanced once per step and their surface
    /// velocity enters the contact law, so a moving wall drags particles
    /// through friction exactly as a static one resists them.
    ///
    /// Forces are recomputed on attachment so the first half-kick sees them.
    #[must_use]
    pub fn with_moving_walls(mut self, walls: Vec<MovingBoundary>) -> Self {
        self.moving_walls = walls;
        self.fix_order = None;
        self.xhold = self.particles.iter().map(|p| p.position).collect();
        self.update_servo_contacts();
        self.compute_forces_setup();
        self
    }

    /// `f_<mesh>[n+1]` of the `fix mesh/surface/stress` on moving wall `wall`:
    /// `0..3` force on the mesh, `3..6` torque about the reference point,
    /// `6..9` the reference point (see [`crate::mesh_stress::MeshStress`]).
    /// `None` when that wall tracks no stress.
    #[must_use]
    pub fn mesh_stress_vector(&self, wall: usize, n: usize) -> Option<f64> {
        self.moving_walls[wall]
            .stress
            .as_ref()
            .map(|s| s.compute_vector(n))
    }

    /// `f_<mesh>[10..12]` of a servo mesh: the centre `xcm_` (`n` in `0..3`);
    /// `None` when that wall has no servo.
    #[must_use]
    pub fn mesh_servo_xcm(&self, wall: usize, n: usize) -> Option<f64> {
        self.moving_walls[wall].servo.as_ref().map(|s| s.0.xcm()[n])
    }

    /// Accumulated Finnie wear of facet `tri` of moving wall `wall`
    /// (`fix mesh/surface/stress ... wear finnie`, the element property
    /// `wear`); `None` when that wall tracks no wear. gh:#1491.
    #[must_use]
    pub fn facet_wear(&self, wall: usize, tri: usize) -> Option<f64> {
        self.moving_walls[wall].stress.as_ref().and_then(|s| s.wear(tri))
    }

    /// Servo velocity `vcm_` (force servo) or angular velocity `omegacm_`
    /// (torque servo) of wall `wall`; `None` when that wall has no servo.
    #[must_use]
    pub fn mesh_servo_ctrl_op(&self, wall: usize) -> Option<[f64; 3]> {
        self.moving_walls[wall]
            .servo
            .as_ref()
            .map(|s| match s.0.ctrl() {
                crate::mesh_stress::ServoCtrl::Force => s.0.vcm(),
                crate::mesh_stress::ServoCtrl::Torque => s.0.omegacm(),
            })
    }

    /// Neighbour-list contact count of the servo mesh of wall `wall`
    /// (`getTotalNumContacts`); `None` when that wall has no servo.
    #[must_use]
    pub fn mesh_servo_contacts(&self, wall: usize) -> Option<usize> {
        self.moving_walls[wall]
            .servo
            .as_ref()
            .map(|_| self.moving_walls[wall].servo_contacts)
    }

    /// `(sigma_n, sigma_t)` `[Pa]` of facet `tri` of moving wall `wall`
    /// (`None` when that wall tracks no stress); see
    /// [`crate::mesh_stress::MeshStress::sigma_n`].
    #[must_use]
    pub fn facet_stress(&self, wall: usize, tri: usize) -> Option<(f64, f64)> {
        self.moving_walls[wall]
            .stress
            .as_ref()
            .map(|s| (s.sigma_n(tri), s.sigma_t(tri)))
    }

    /// Attach a `fix massflow/mesh` counter (gh:#1416); it is invoked after
    /// the particles move each step. Returns its index for
    /// [`GranularSystem::massflow_vector`].
    pub fn add_massflow_mesh(&mut self, fix: crate::massflow::MassFlowMesh) -> usize {
        self.massflow.push(fix);
        self.rebuild_massflow_lists();
        self.massflow.len() - 1
    }

    /// Rebuild every massflow counter's mesh neighbour list at the current
    /// positions (`FixNeighlistMesh::pre_force` on a rebuild).
    fn rebuild_massflow_lists(&mut self) {
        for k in 0..self.massflow.len() {
            match self.massflow[k].wall() {
                Some(w) => {
                    if let WallGeometry::Mesh(mesh) = &self.moving_walls[w].geometry {
                        self.massflow[k].rebuild_lists(mesh, &self.particles);
                    }
                }
                None => {
                    let mesh = self.massflow[k].mesh_arc();
                    self.massflow[k].rebuild_lists(&mesh, &self.particles);
                }
            }
        }
    }

    /// `FixMassflowMesh::pre_exchange` with `delete_atoms yes`: remove the
    /// marked particles one at a time in marking order, each by copying the
    /// last atom into its slot (`AtomVec::copy(nlocal-1, iPart)`), and book
    /// the deleted mass and count on the fix.
    fn delete_massflow_marked(&mut self) {
        let mut any = false;
        for k in 0..self.massflow.len() {
            for tag in self.massflow[k].take_pending_deletions() {
                let Some(i) = self.tags.iter().position(|&t| t == tag) else {
                    continue;
                };
                let mass = self.particles[i].mass;
                self.note_removed(i);
                self.particles.swap_remove(i);
                self.tags.swap_remove(i);
                self.types.swap_remove(i);
                self.forces.swap_remove(i);
                self.torques.swap_remove(i);
                self.massflow[k].record_deleted(tag, mass);
                any = true;
            }
        }
        if any {
            self.xhold = self.particles.iter().map(|p| p.position).collect();
        }
    }

    /// Set the summed property (`sum_property`) of the particle with tag
    /// `tag` on massflow counter `fix`.
    pub fn set_massflow_property(&mut self, fix: usize, tag: u64, value: f64) {
        self.massflow[fix].set_property(tag, value);
    }

    /// `f_<id>[index+1]` of massflow counter `fix` (0 mass, 1 count, 2 mass
    /// rate, 3 count rate; see [`crate::massflow::MassFlowMesh::compute_vector`]).
    pub fn massflow_vector(&mut self, fix: usize, index: usize) -> f64 {
        self.massflow[fix].compute_vector(index)
    }

    /// The counter attached as `fix`.
    #[must_use]
    pub fn massflow(&self, fix: usize) -> &crate::massflow::MassFlowMesh {
        &self.massflow[fix]
    }

    /// Emulate upstream's neighbour-list rebuild schedule (`neighbor <skin>`,
    /// `neigh_modify delay 0 every 1 check yes`) for the one thing it changes
    /// numerically here: when a **rotating mesh wall** recomputes its cached
    /// edge vectors, normals and edge lengths from its nodes. Upstream rotates
    /// those incrementally by the step quaternion and refreshes them only on a
    /// rebuild step (a particle moved more than `skin/2` since the last build,
    /// or a mesh node did, one step earlier); this reproduces that schedule so
    /// the round-off matches (gh:#1416). The physics does not depend on it.
    ///
    /// With the default `skin = 0` the geometry is recomputed after every
    /// rotation. Call after [`GranularSystem::with_moving_walls`].
    #[must_use]
    pub fn with_neighbor_skin(mut self, skin: f64) -> Self {
        self.neigh_skin = skin;
        self.xhold = self.particles.iter().map(|p| p.position).collect();
        self.update_servo_contacts();
        self
    }

    /// The set-up force evaluation: like [`GranularSystem::compute_forces`] but
    /// without advancing the shear history, as upstream's `setup()` does
    /// (`PairGran::compute`: `if (update->setupflag) shearupdate_ = 0;`, same in
    /// `FixWallGran::post_force_wall`). ~~The port advanced the history here
    /// too~~ **CORRECTED 2026-11-11** (gh:#1418): it gave a bed with non-zero
    /// velocities one extra `v_tr*dt` of shear at step 0.
    fn compute_forces_setup(&mut self) {
        self.setup_phase = true;
        self.compute_forces();
        self.setup_phase = false;
    }

    /// **Begin a new `run`**, as every `run` command of a LIGGGHTS deck does
    /// (`Run::command` -> `lmp->init()` -> `Verlet::setup()`; gh:#1510).
    /// [`GranularSystem::run`] continues the current run; a deck with two
    /// `run` commands is reproduced by `run(n1); begin_run()?; run(n2)`, and
    /// differs from one continuous run even with nothing changed in between,
    /// because the set-up evaluates the forces again at the full-step
    /// velocities. A `run N pre no` is a plain [`GranularSystem::run`] (no
    /// `init`, no set-up). Construction already performs the first run's
    /// set-up.
    ///
    /// What is reset, read from LIGGGHTS-PUBLIC `3d5c00f2`:
    ///
    /// * **`init()`** of every fix (`Modify::init`): the servo re-sets
    ///   `p_ref = xcm`, `vel_min_` from the present smallest radius and the
    ///   torque servo's `rPaMax` from the present nodes, and re-checks its
    ///   aborts ([`MovingBoundary::begin_run`]). The other fixes' `init()`
    ///   only look up variables, properties and pointers.
    /// * **`Verlet::setup()`**: the atom sort and neighbour build (under
    ///   [`NeighborOrder::Liggghts`]: `nextsort` re-based, the half list,
    ///   the primitive-wall lists, `xhold`); `FixMesh::setup_pre_force`
    ///   (each mesh's geometry and original nodes from its present nodes);
    ///   `FixNeighlistMesh::setup_pre_force` (the servo contact count and the
    ///   massflow lists); the servo's `setup_pre_force` (`xcm_orig = xcm`,
    ///   `totalPhi = 0`); the pair compute and every fix's `setup` with
    ///   `shearupdate = 0` (the same evaluation construction does); and
    ///   `FixMoveMesh::setup`, which zeroes a moved mesh's node velocities
    ///   ([`MovingBoundary::move_mesh_setup`]).
    /// * **With clumps** (gh:#1415, added 2026-11-14): `FixMultisphere`'s
    ///   `setup_pre_exchange` (delete the atoms of lost clumps),
    ///   `Comm::exchange` (under [`GranularSystem::with_lost_atoms`]),
    ///   `setup_pre_neighbor` (`check_lost_atoms`), and after the set-up
    ///   forces `setup_pre_force` + `setup`: each body's `fcm` and `torque`
    ///   are re-summed from them (`calc_force(true)`). ~~A second run with
    ///   clumps is refused.~~ **CORRECTED 2026-11-14:** ported, bit-identical
    ///   (`tests/multisphere_cross_code.rs`, `ms_rerun`).
    ///
    /// What **persists**, as upstream: pair, wall and mesh contact history;
    /// the servo's integral term, last process value, velocity and node
    /// velocities; the stress totals (`MeshModuleStress::pre_force` does not
    /// run at set-up, so the set-up contributions add to the last step's,
    /// which is what a `thermo` line printed at set-up shows); the wear
    /// (`wear_step` is zero after the last step, so the set-up's wear enters
    /// the next step); insertion schedules and counters
    /// (`FixInsert::setup` acts on the first run only); massflow counters;
    /// heat and liquid content; a mesh's pending re-neighbour request
    /// (`next_reneighbor`); `step` and `time`.
    ///
    /// **Not modelled:** `ramp()` and other run-bounded quantities in
    /// variables (the crate's variable contexts take `begin_step = 0`), the
    /// reference point a superposed `move/mesh` resets
    /// (`FixMoveMesh::reset_reference_point`), and the slot compaction of the
    /// mesh contact history (`FixContactHistoryMesh::sort_contacts`), which
    /// the port does not model at re-neighbouring either.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] where a fix's `init()` aborts (see
    /// [`crate::mesh_stress::MeshServo::init_run`]), or where
    /// `Atom::setup_sort_bins` does (the sort on with a zero cut; see
    /// [`GranularSystem::try_with_liggghts_neighbor_order`]).
    pub fn begin_run(&mut self) -> Result<(), DemError> {
        // lmp->init(): Modify::init -> fix->init(); then Verlet::setup's
        // setup_pre_force of the meshes (no fix reads another in between).
        // Not modelled: a mesh's setup_pre_force follows the exchange below
        // upstream; no deck here has both a servo mesh and lost atoms.
        for w in &mut self.moving_walls {
            w.begin_run(&self.particles)?;
        }
        // Verlet::setup -> Atom::setup -> setup_sort_bins (gh:#1568)
        self.check_sort_setup()?;
        // modify->setup_pre_exchange (`FixMultisphere::pre_exchange`),
        // comm->exchange, modify->setup_pre_neighbor (`check_lost_atoms`);
        // each a no-op without clumps / lost atoms (gh:#1415)
        self.multisphere_pre_exchange();
        self.exchange_lost_atoms();
        self.multisphere_pre_neighbor();
        // neighbor->build (the LIGGGHTS-order rebuild in the set-up
        // evaluation also does this) and the mesh neighbour lists
        self.xhold = self.particles.iter().map(|p| p.position).collect();
        self.update_servo_contacts();
        self.rebuild_massflow_lists();
        // pair compute and modify->setup (wall/gran, gravity, force fixes)
        self.compute_forces_setup();
        // `FixMultisphere::setup_pre_force` -> `reset_forces(false)`, then
        // `FixMultisphere::setup` -> `calc_force(true)` on the set-up forces
        // (gh:#1415); the next step's first half-kick uses them
        self.multisphere_setup_force();
        // FixMoveMesh::setup, declared after its wall/gran
        for w in &mut self.moving_walls {
            w.move_mesh_setup();
        }
        Ok(())
    }

    /// Make this system's clump fix `fix multisphere/nointegration`
    /// ([`crate::multisphere::Multisphere::without_integration`]; gh:#1415):
    /// the bodies are not integrated, and their member atoms move by the atom
    /// integrator alone. Call before the first clump is inserted.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a clump already exists (upstream's fix
    /// style is fixed when the fix is defined).
    pub fn with_multisphere_nointegration(mut self) -> Result<Self, DemError> {
        if self.multisphere.is_some() {
            return Err(DemError::InvalidInput(
                "multisphere/nointegration must be chosen before the first clump is inserted".into(),
            ));
        }
        self.ms_nointegration = true;
        Ok(self)
    }

    /// `FixMultisphere::setup` at a `run` set-up: the bodies' force and torque
    /// reset (`setup_pre_force`) and summed from the set-up forces in local
    /// atom order, plus `m g` (`calc_force(true)`; no heat transfer, so the
    /// `setupflag` branch does not arise). No-op without clumps.
    fn multisphere_setup_force(&mut self) {
        if self.multisphere.is_none() {
            return;
        }
        let order = self.local_order_indices();
        let gravity = self.gravity;
        if let Some(ms) = self.multisphere.as_mut() {
            ms.reset_forces(false);
            ms.calc_force(&self.particles, &self.forces, &self.torques, &order, gravity);
        }
    }

    /// `fix_modify <ms> fflag x y z` / `tflag x y z`
    /// (`FixMultisphere::modify_param`, gh:#1415): the flags of **every
    /// existing body** are replaced; a clump inserted later takes its
    /// template's flags. Upstream issues it between two `run` commands, so
    /// call [`GranularSystem::begin_run`] afterwards. Before the first clump
    /// exists it changes nothing, as upstream's loop over zero bodies does.
    pub fn fix_modify_multisphere(&mut self, m: crate::multisphere::MultisphereModify) {
        if let Some(ms) = self.multisphere.as_mut() {
            ms.modify_param(m);
        }
    }

    /// `fix_modify <mesh> servo/<keyword> ...` on the servo of moving wall
    /// `wall` (`MeshModuleStressServo::modify_param`, gh:#1510). Upstream
    /// issues it between two `run` commands, and the values reach the
    /// controller through the next `init()`, so call
    /// [`GranularSystem::begin_run`] afterwards.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `wall` has no servo, or where upstream
    /// aborts (see [`crate::mesh_stress::MeshServo::modify_param`]).
    pub fn fix_modify_servo(
        &mut self,
        wall: usize,
        m: crate::mesh_stress::ServoModify,
    ) -> Result<(), DemError> {
        match self.moving_walls.get_mut(wall).and_then(|w| w.servo.as_mut()) {
            Some((servo, _)) => servo.modify_param(m),
            None => Err(DemError::InvalidInput(format!("moving wall {wall} has no servo"))),
        }
    }

    /// Recount the servo meshes' neighbour-list contacts
    /// (`getTotalNumContacts`, read by `mode auto`); done where upstream
    /// rebuilds its neighbour list.
    fn update_servo_contacts(&mut self) {
        let skin = self.neigh_skin;
        for w in &mut self.moving_walls {
            w.update_servo_contacts(&self.particles, skin);
        }
    }

    /// The neighbour-rebuild decision and mesh refresh that follow the walls'
    /// move in a step (`Neighbor::decide`, `FixMesh::pre_force`). The refresh
    /// includes the stress module's facet areas, as upstream's
    /// `recalcLocalSurfProperties` does (gh:#1512).
    fn refresh_moving_meshes(&mut self) {
        let skin = self.neigh_skin;
        if skin <= 0.0 {
            for w in &mut self.moving_walls {
                if w.is_rotating() {
                    w.refresh_mesh();
                }
            }
            return;
        }
        let triggersq = 0.25 * skin * skin;
        // `Neighbor::decide`: a fix's `next_reneighbor` (fix multisphere after
        // freeing a lost clump's atoms, gh:#1415) forces the rebuild
        let ms_forced = self
            .multisphere
            .as_ref()
            .is_some_and(|m| m.next_reneighbor() == Some(self.step_count));
        let mut rebuild = self.mesh_rebuild_pending || ms_forced;
        if !rebuild {
            for (p, h) in self.particles.iter().zip(&self.xhold) {
                let d = p.position.sub(*h);
                let rsq = d.x * d.x + d.y * d.y + d.z * d.z;
                if rsq > triggersq {
                    rebuild = true;
                }
            }
        }
        self.rebuild_now = rebuild;
        if rebuild {
            for w in &mut self.moving_walls {
                w.refresh_mesh();
            }
            self.xhold = self.particles.iter().map(|p| p.position).collect();
            self.mesh_rebuild_pending = false;
        } else {
            let mut any = false;
            for w in &self.moving_walls {
                if let WallGeometry::Mesh(m) = &w.geometry {
                    if m.decide_rebuild(skin) {
                        any = true;
                    }
                }
            }
            self.mesh_rebuild_pending = any;
        }
    }

    /// Order the pair list the way LIGGGHTS does (`newton off`, `neighbor
    /// <skin> bin`, `atom_modify sort`): a spatial atom sort at set-up and every
    /// `sort_every` steps, a bin half-list built in upstream's order, stale
    /// between rebuilds. With it, a particle's contact forces are summed in
    /// upstream's order, so per-contact forces stay **bit-identical** to
    /// upstream beyond step 0 (`tests/pair_gran_local_cross_code.rs`).
    ///
    /// **This re-does set-up**, as a new `run` would: the atoms are sorted and
    /// the list rebuilt at the current positions, and the forces recomputed
    /// (without advancing shear history). The default is
    /// [`NeighborOrder::CellGrid`]; see [`crate::neighbor_list`] for why this is
    /// a named option (it needs the deck's box and skin). Call after
    /// [`GranularSystem::with_moving_walls`].
    #[must_use]
    pub fn with_liggghts_neighbor_order(mut self, cfg: LiggghtsNeighbor) -> Self {
        self.neighbor_order = NeighborOrder::Liggghts(cfg);
        self.neigh_skin = cfg.skin;
        self.perm = (0..self.particles.len()).collect();
        self.next_sort = 0;
        self.xhold = self.particles.iter().map(|p| p.position).collect();
        self.update_servo_contacts();
        self.mesh_rebuild_pending = false;
        self.compute_forces_setup();
        self
    }

    /// [`GranularSystem::with_liggghts_neighbor_order`], refusing the set-up
    /// where upstream does (gh:#1568): with the atom sort on
    /// (`sort_every > 0`) and `cutneighmax = 0` (no atoms and no insertion
    /// fix), `Atom::setup_sort_bins` stops with "Atom sorting has bin size =
    /// 0.0" (LIGGGHTS-PUBLIC `3d5c00f2`, `atom.cpp`). The infallible form
    /// does not refuse: it sorts the zero atoms, which does nothing.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] with upstream's message in that case.
    pub fn try_with_liggghts_neighbor_order(self, cfg: LiggghtsNeighbor) -> Result<Self, DemError> {
        let sys = self.with_liggghts_neighbor_order(cfg);
        sys.check_sort_setup()?;
        Ok(sys)
    }

    /// Enable the multicontact surface model and `fix multicontact/halfspace`
    /// (Brodu et al.; LIGGGHTS `surface_model multicontact`): each contact's
    /// radius is expanded by the displacement the particle's other contacts
    /// cause, at the pair (`pair_gran_base.h`) and primitive-wall
    /// (`fix_wall_gran.cpp`) sites, and the neighbour list looks out to a
    /// contact-distance factor of 2 (`register_contact_dist_factor(2.0)`).
    /// Mesh walls are NOT hooked. Opt-in: without this call nothing changes.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless the system is under
    /// [`NeighborOrder::Liggghts`] (the history lives on upstream's list).
    pub fn with_multicontact(mut self, fix: crate::multicontact::HalfSpaceFix) -> Result<Self, DemError> {
        let NeighborOrder::Liggghts(mut cfg) = self.neighbor_order else {
            return Err(DemError::InvalidInput(
                "multicontact needs the LIGGGHTS neighbour order (with_liggghts_neighbor_order)".into(),
            ));
        };
        cfg.contact_distance_factor = cfg.contact_distance_factor.max(crate::multicontact::CONTACT_DISTANCE_FACTOR);
        self.neighbor_order = NeighborOrder::Liggghts(cfg);
        self.mc = Some(crate::multicontact::MulticontactState::new(fix));
        self.compute_forces_setup();
        Ok(self)
    }

    /// `FixMultiContactHalfSpace::pre_force`: expansions for this step from the
    /// history the last force evaluation wrote.
    fn mc_pre_force(&mut self) {
        use crate::multicontact::{PairContact, ParticleView, WallContact};
        let Some(mc) = self.mc.as_mut() else { return };
        let n = self.particles.len();
        let lorder = if self.perm.len() == n { self.perm.clone() } else { (0..n).collect() };
        let mut local_of = vec![0usize; n];
        for (l, &p) in lorder.iter().enumerate() {
            local_of[p] = l;
        }
        let x: Vec<[f64; 3]> = lorder.iter().map(|&p| { let v = self.particles[p].position; [v.x, v.y, v.z] }).collect();
        let tag: Vec<usize> = lorder.iter().map(|&p| self.tags[p] as usize).collect();
        let typ: Vec<usize> = lorder.iter().map(|&p| self.types[p]).collect();
        let rad: Vec<f64> = lorder.iter().map(|&p| self.particles[p].radius).collect();
        let grp = vec![true; n];
        let mut pairs: Vec<Vec<PairContact>> = vec![Vec::new(); n];
        for &(i, j) in &self.pairs {
            let (i, j) = (i as usize, j as usize);
            let h = mc.pair_history(self.tags[i] as usize, self.tags[j] as usize);
            pairs[local_of[i]].push(PairContact { j: local_of[j], history: h });
        }
        let nb = self.boundaries.len();
        let nm = self.moving_walls.len();
        let mut walls: Vec<Vec<Vec<WallContact>>> = vec![vec![Vec::new(); n]; nb + nm];
        for (w, wl) in walls.iter_mut().enumerate().take(nb) {
            for l in 0..n {
                if let Some(h) = mc.wall_hist.get(&(tag[l], w)) {
                    wl[l].push(WallContact { id: 1, history: *h });
                }
            }
        }
        // mesh walls: one contact per facet, in `mesh_partners` slot order
        // (`FixMultiContactHalfSpace::pre_force` iterates `npartners`)
        let mut is_mesh = vec![false; nb + nm];
        for w in 0..nm {
            is_mesh[nb + w] = true;
            for l in 0..n {
                if let Some(slots) = self.mesh_partners.get(&(tag[l], w)) {
                    for &(id, _) in slots.iter().flatten() {
                        let h = mc.mesh_hist.get(&(tag[l], nb + w, id)).copied().unwrap_or([0.0; 4]);
                        walls[nb + w][l].push(WallContact { id: id as usize, history: h });
                    }
                }
            }
        }
        mc.data = mc.fix.pre_force(
            ParticleView { x: &x, tag: &tag, typ: &typ, radius: &rad, in_group: &grp },
            &pairs,
            &walls,
            &is_mesh,
        );
        mc.local_of = local_of;
    }

    /// Particle–particle loop with the multicontact radius expansion and
    /// history writes (`pair_gran_base.h` compute + the surface model).
    fn resolve_pairs_mc(&mut self) {
        let setup = self.setup_phase;
        let pairs = std::mem::take(&mut self.pairs);
        for &(i, j) in &pairs {
            let (i, j) = (i as usize, j as usize);
            let (ti, tj) = (self.tags[i] as usize, self.tags[j] as usize);
            let (a, b) = (self.particles[i], self.particles[j]);
            // `pair_gran_base.h`: `sidata.radi` is the UNEXPANDED radius of i
            // (set once per i), `sidata.radj` and `radsum` are expanded.
            let (radj, radsum) = {
                let mc = self.mc.as_ref().expect("multicontact");
                let radi_e = a.radius + mc.data.pair_expansion(mc.local_of[i], tj);
                let radj_e = b.radius + mc.data.pair_expansion(mc.local_of[j], ti);
                (radj_e, radi_e + radj_e)
            };
            let d = a.position.sub(b.position);
            let rsq = d.x * d.x + d.y * d.y + d.z * d.z;
            let kin = ContactKinematics::pair_radii(
                &a,
                &b,
                self.frozen.get(i).copied().unwrap_or(false),
                self.frozen.get(j).copied().unwrap_or(false),
                a.radius,
                radj,
                radsum,
            )
            .map(|k| k.with_types(self.types[i], self.types[j]));
            if let Some(k) = kin {
                let key = ContactKey::pair(ti, tj);
                let flip = ti > tj;
                if flip {
                    self.history.flip(key);
                    self.model.flip_aux(key, &mut self.history);
                }
                let gf = resolve_c(&self.model, setup, key, &k, &mut self.history, self.dt);
                if flip {
                    self.history.flip(key);
                    self.model.flip_aux(key, &mut self.history);
                }
                self.forces[i] = self.forces[i].add(gf.force_i);
                self.forces[j] = self.forces[j].add(gf.force_j);
                self.torques[i] = self.torques[i].add(gf.torque_i);
                self.torques[j] = self.torques[j].add(gf.torque_j);
                if !setup {
                    let fnorm = crate::multicontact::end_surfaces_intersect_history(
                        [gf.force_i.x, gf.force_i.y, gf.force_i.z],
                        [k.en.x, k.en.y, k.en.z],
                    );
                    let h = crate::multicontact::surfaces_intersect_history(false, [0.0; 3], a.radius, radj);
                    self.mc.as_mut().expect("multicontact").set_pair_history(ti, tj, [h[0], h[1], fnorm]);
                }
            } else if !setup
                && rsq < crate::multicontact::CONTACT_DISTANCE_FACTOR
                    * crate::multicontact::CONTACT_DISTANCE_FACTOR
                    * radsum
                    * radsum
            {
                let h = crate::multicontact::surfaces_close_history(false, [0.0; 3], a.radius, radj);
                self.mc.as_mut().expect("multicontact").set_pair_history(ti, tj, [h[0], h[1], h[2]]);
            }
        }
        self.pairs = pairs;
    }

    /// Attach upstream's contact conduction (`fix heat/gran/conduction`,
    /// [`crate::heat_gran`]). Temperatures are the particles' own
    /// [`Particle::temperature`] (upstream's `initial_temperature`, or a later
    /// `set ... property/atom Temp`); they are advanced every step. The
    /// clean-room [`crate::thermal`] stays callable on its own.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a particle's atom type has no entry in
    /// the fix's per-type properties.
    pub fn with_heat_conduction(mut self, heat: HeatGranConduction) -> Result<Self, DemError> {
        if let Some(bad) = self.types.iter().find(|&&t| t >= heat.n_types()) {
            return Err(DemError::InvalidInput(format!(
                "atom type {bad} has no thermal properties ({} given)",
                heat.n_types()
            )));
        }
        self.heat = Some(heat);
        self.heat_flux = vec![0.0; self.particles.len()];
        Ok(self)
    }

    /// Heat flux into each particle `[W]` from the last step (`heatFlux`).
    #[must_use]
    pub fn heat_flux(&self) -> &[f64] {
        &self.heat_flux
    }

    /// Attach the `liquidtransfer` state of a capillary cohesion model
    /// ([`GranularCohesion::Capillary`], [`crate::capillary`], gh:#1411): every
    /// particle starts with `surfaceLiquidContentInitial`, the neighbour list
    /// looks out to `1.1 * maxSeparationDistanceRatio` contact distances (both
    /// upstream models call `register_contact_dist_factor(1.1 * max_ratio)`),
    /// and bridges form, stretch and rupture out of contact as upstream's
    /// `surfacesClose` does. Washino's `limitLiquidContent` overwrites the
    /// maximum separation ratio from the current radii and the atom types
    /// present, as upstream's `connectToProperties` does.
    ///
    /// Physics is on as soon as the model is selected: nothing else has to be
    /// switched on. The set-up evaluation is re-done.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless the model's cohesion is
    /// `Capillary`, the system uses [`NeighborOrder::Liggghts`] (bridges live
    /// on upstream's list) and no per-pair coefficient table. ~~has no walls
    /// (particle-wall bridges are not ported)~~ **CORRECTED 2026-11-12
    /// (gh:#1487):** particle-wall bridges are ported (Easo dry walls, Washino
    /// with [`GranularSystem::with_mesh_liquid_transfer`]). Attach the walls
    /// first. Refused, because the crate cannot represent them exactly: a
    /// static boundary other than a two-sided axis-aligned
    /// [`Boundary::Plane`] or axis-aligned [`Boundary::Cylinder`], a moving wall
    /// that is not a mesh, and walls together with more than one particle type
    /// (every wall contact here is resolved with the particle's own type, so
    /// the deck's wall `type` must be the particles').
    pub fn with_capillary(mut self) -> Result<Self, DemError> {
        let GranularCohesion::Capillary(mut p) = self.model.cohesion else {
            return Err(DemError::InvalidInput(
                "with_capillary needs a model whose cohesion is GranularCohesion::Capillary".into(),
            ));
        };
        let NeighborOrder::Liggghts(mut cfg) = self.neighbor_order else {
            return Err(DemError::InvalidInput(
                "capillary bridges need the LIGGGHTS neighbour order (with_liggghts_neighbor_order)".into(),
            ));
        };
        self.check_capillary_walls()?;
        if self.model.pair_models.is_some() {
            return Err(DemError::InvalidInput(
                "capillary cohesion with a per-pair coefficient table is not supported".into(),
            ));
        }
        if let Some(bad) = self.types.iter().find(|&&t| t >= MAX_TYPES) {
            return Err(DemError::InvalidInput(format!(
                "atom type {bad} is outside the {MAX_TYPES} types a capillary model holds"
            )));
        }
        let n_types = self.types.iter().copied().max().map_or(1, |t| t + 1);
        let max_rad = self.particles.iter().fold(-1.0_f64, |m, q| m.max(q.radius));
        let min_rad = self.particles.iter().fold(f64::MAX, |m, q| m.min(q.radius));
        p.max_separation_ratio = p.limited_max_separation_ratio(n_types, max_rad, min_rad);
        self.model.cohesion = GranularCohesion::Capillary(p);
        cfg.contact_distance_factor = cfg
            .contact_distance_factor
            .max(crate::capillary::CapillaryParams::contact_distance_factor(p.max_separation_ratio));
        self.neighbor_order = NeighborOrder::Liggghts(cfg);
        self.capillary = Some(crate::capillary::CapillaryState::new(
            self.particles.len(),
            p.initial_liquid_content,
        ));
        self.compute_forces_setup();
        Ok(self)
    }

    /// The walls a capillary model can bridge to exactly (gh:#1487); see
    /// [`GranularSystem::with_capillary`].
    fn check_capillary_walls(&self) -> Result<(), DemError> {
        if let Some(b) = self.boundaries.iter().find(|b| !b.is_upstream_primitive()) {
            return Err(DemError::InvalidInput(format!(
                "capillary wall bridges need a two-sided axis-aligned plane or an axis-aligned \
                 cylinder (upstream's primitive walls); got {b:?}"
            )));
        }
        if self
            .moving_walls
            .iter()
            .any(|m| !m.contactless && !matches!(m.geometry, WallGeometry::Mesh(_)))
        {
            return Err(DemError::InvalidInput(
                "capillary wall bridges on a moving analytic wall are not ported; use a mesh".into(),
            ));
        }
        let has_walls = !self.boundaries.is_empty() || self.moving_walls.iter().any(|m| !m.contactless);
        if has_walls && self.types.iter().any(|&t| t != self.types[0]) {
            return Err(DemError::InvalidInput(
                "capillary wall bridges with more than one particle type: the wall's atom type \
                 (its contactAngle) is not represented here"
                    .into(),
            ));
        }
        Ok(())
    }

    /// Declare moving wall `wall` (a mesh) as `fix mesh/surface/liquidtransfer
    /// ... wall_thickness t initial_liquid_content c`
    /// (`MeshModuleLiquidTransfer`, [`crate::capillary::MeshLiquid`], gh:#1487):
    /// every facet carries a liquid film the Washino model bridges to and
    /// exchanges liquid with at rupture. The Easo model ignores it (dry
    /// walls). The set-up evaluation is re-done.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] without a capillary model, for a wall that
    /// is not a mesh, or a thickness or initial content below `1e-12`
    /// (upstream's "have to define").
    pub fn with_mesh_liquid_transfer(
        mut self,
        wall: usize,
        wall_thickness: f64,
        initial_liquid_content: f64,
    ) -> Result<Self, DemError> {
        let n_tri = match self.moving_walls.get(wall).map(|m| &m.geometry) {
            Some(WallGeometry::Mesh(mesh)) => mesh.triangles.len(),
            _ => {
                return Err(DemError::InvalidInput(format!(
                    "liquidtransfer needs moving wall {wall} to be a mesh"
                )))
            }
        };
        let module = crate::capillary::MeshLiquid::new(n_tri, wall_thickness, initial_liquid_content)?;
        let Some(c) = self.capillary.as_mut() else {
            return Err(DemError::InvalidInput(
                "with_mesh_liquid_transfer needs with_capillary first".into(),
            ));
        };
        c.mesh_liquid.insert(wall, module);
        self.compute_forces_setup();
        Ok(self)
    }

    /// `LiquidContent` of each facet of moving wall `wall` (empty unless it
    /// carries [`GranularSystem::with_mesh_liquid_transfer`]).
    #[must_use]
    pub fn mesh_liquid_content(&self, wall: usize) -> &[f64] {
        self.capillary
            .as_ref()
            .and_then(|c| c.mesh_liquid.get(&wall))
            .map_or(&[], |m| &m.content)
    }

    /// Number of particle-wall liquid bridges (`contflag == 1`, gh:#1487).
    #[must_use]
    pub fn wall_bridge_count(&self) -> usize {
        self.capillary.as_ref().map_or(0, |c| c.wall_bridges.len())
    }

    /// `surfaceLiquidContent` of each particle (empty without a capillary model).
    #[must_use]
    pub fn liquid_content(&self) -> &[f64] {
        self.capillary.as_ref().map_or(&[], |c| &c.content)
    }

    /// `liquidFlux` of each particle `[1/s]` from the last force evaluation.
    #[must_use]
    pub fn liquid_flux(&self) -> &[f64] {
        self.capillary.as_ref().map_or(&[], |c| &c.flux)
    }

    /// Number of pairs with a liquid bridge (`contflag == 1`).
    #[must_use]
    pub fn bridge_count(&self) -> usize {
        self.capillary.as_ref().map_or(0, |c| c.bridges.len())
    }

    /// Whether the pair of particles with these tags has a liquid bridge.
    #[must_use]
    pub fn has_bridge(&self, tag_a: u64, tag_b: u64) -> bool {
        self.capillary
            .as_ref()
            .is_some_and(|c| c.bridges.contains(&crate::capillary::CapillaryState::key(tag_a, tag_b)))
    }

    /// Set every particle's `surfaceLiquidContent` (upstream `set ... property/atom
    /// surfaceLiquidContent`, in particle order) and redo the set-up force
    /// evaluation, as the `run` that follows such a `set` does.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] without a capillary model attached
    /// ([`GranularSystem::with_capillary`]) or with a list of the wrong length
    /// or a content outside `[0, 1]`.
    pub fn with_liquid_contents(mut self, content: Vec<f64>) -> Result<Self, DemError> {
        let n = self.particles.len();
        let Some(c) = self.capillary.as_mut() else {
            return Err(DemError::InvalidInput(
                "with_liquid_contents needs with_capillary first".into(),
            ));
        };
        if content.len() != n || content.iter().any(|q| !(0.0..=1.0).contains(q)) {
            return Err(DemError::InvalidInput(format!(
                "need {n} liquid contents in [0, 1]"
            )));
        }
        c.content = content;
        self.compute_forces_setup();
        Ok(self)
    }

    /// The force on each particle `[N]` from the last force evaluation (what
    /// upstream dumps as `fx fy fz` after a step).
    #[must_use]
    pub fn forces(&self) -> &[Vec3] {
        &self.forces
    }

    /// Keep the per-particle liquid arrays the length of the particle set
    /// (a new particle gets `surfaceLiquidContentInitial`).
    fn sync_capillary(&mut self) {
        let n = self.particles.len();
        let GranularCohesion::Capillary(p) = self.model.cohesion else {
            return;
        };
        if let Some(c) = self.capillary.as_mut() {
            c.content.resize(n, p.initial_liquid_content);
            c.flux.resize(n, 0.0);
        }
    }

    /// Particle-particle loop of a capillary model: the ordinary contact law
    /// for overlapping pairs (with the bridge's contact force folded in at
    /// upstream's place in the force sum) and `surfacesClose` for the pairs
    /// that do not overlap but lie inside the contact distance factor
    /// (`pair_gran_base.h`, `compute_force`), in pair-list order.
    fn resolve_pairs_capillary(&mut self) {
        use crate::capillary::{CapillaryState, CloseInput, CloseOutcome};
        let setup = self.setup_phase;
        let GranularCohesion::Capillary(p) = self.model.cohesion else {
            return;
        };
        let factor = match self.neighbor_order {
            NeighborOrder::Liggghts(cfg) => cfg.contact_distance_factor,
            NeighborOrder::CellGrid => 1.0,
        };
        let contact_distance_multiplier = factor * factor;
        let mut cap = self.capillary.take().expect("capillary state");
        cap.content.resize(self.particles.len(), p.initial_liquid_content);
        cap.flux.resize(self.particles.len(), 0.0);
        let pairs = std::mem::take(&mut self.pairs);
        for &(i, j) in &pairs {
            let (i, j) = (i as usize, j as usize);
            let a = self.particles[i];
            let b = self.particles[j];
            let (ti, tj) = (self.tags[i], self.tags[j]);
            let pair_key = CapillaryState::key(ti, tj);
            let kin = ContactKinematics::pair_frozen(
                &a,
                &b,
                self.frozen.get(i).copied().unwrap_or(false),
                self.frozen.get(j).copied().unwrap_or(false),
            )
            .map(|k| k.with_types(self.types[i], self.types[j]));
            if let Some(k) = kin {
                // surfacesIntersect: contflag = 1, limit the liquid, bridge force
                cap.bridges.insert(pair_key);
                cap.content[i] = p.limit_content(cap.content[i], self.types[i]);
                cap.content[j] = p.limit_content(cap.content[j], self.types[j]);
                let k = k.with_liquid(cap.content[i], cap.content[j]);
                let key = ContactKey::pair(ti as usize, tj as usize);
                let flip = ti > tj;
                if flip {
                    self.history.flip(key);
                    self.model.flip_aux(key, &mut self.history);
                }
                let gf = resolve_c(&self.model, setup, key, &k, &mut self.history, self.dt);
                if flip {
                    self.history.flip(key);
                    self.model.flip_aux(key, &mut self.history);
                }
                self.forces[i] = self.forces[i].add(gf.force_i);
                self.forces[j] = self.forces[j].add(gf.force_j);
                self.torques[i] = self.torques[i].add(gf.torque_i);
                self.torques[j] = self.torques[j].add(gf.torque_j);
                continue;
            }
            let d = a.position.sub(b.position);
            let rsq = d.x * d.x + d.y * d.y + d.z * d.z;
            let radsum = a.radius + b.radius;
            if rsq < contact_distance_multiplier * radsum * radsum {
                // surfacesClose: the tangential model forgets the shear
                self.history.forget_shear(ContactKey::pair(ti as usize, tj as usize));
                cap.content[i] = p.limit_content(cap.content[i], self.types[i]);
                cap.content[j] = p.limit_content(cap.content[j], self.types[j]);
                let outcome = p.close(&CloseInput {
                    delta: d,
                    rsq,
                    rad_i: a.radius,
                    rad_j: b.radius,
                    radsum,
                    itype: self.types[i],
                    jtype: self.types[j],
                    v_i: a.velocity,
                    v_j: b.velocity,
                    omega_i: a.angular_velocity,
                    omega_j: b.angular_velocity,
                    liquid_i: cap.content[i],
                    liquid_j: cap.content[j],
                    bridge: cap.bridges.contains(&pair_key),
                    dt: self.dt,
                    is_wall: false,
                    wall_liquid_vol1000: None,
                });
                match outcome {
                    CloseOutcome::NoBridge => {}
                    CloseOutcome::ActiveNoForce => {
                        cap.bridges.insert(pair_key);
                    }
                    CloseOutcome::Force {
                        force_i,
                        torque_i,
                        force_j,
                        torque_j,
                    } => {
                        cap.bridges.insert(pair_key);
                        self.forces[i] = self.forces[i].add(force_i);
                        self.torques[i] = self.torques[i].add(torque_i);
                        self.forces[j] = self.forces[j].add(force_j);
                        self.torques[j] = self.torques[j].add(torque_j);
                    }
                    CloseOutcome::Breaks { flux_i, flux_j } => {
                        cap.bridges.remove(&pair_key);
                        cap.flux[i] += flux_i;
                        cap.flux[j] += flux_j;
                    }
                    // a pair never returns the wall outcome
                    CloseOutcome::WallBreaks { .. } => {
                        cap.bridges.remove(&pair_key);
                    }
                }
            }
        }
        self.pairs = pairs;
        self.capillary = Some(cap);
    }

    /// The pair-ordering mode in use.
    #[must_use]
    pub fn neighbor_order(&self) -> NeighborOrder {
        self.neighbor_order
    }

    /// `neighbor->cutneighmax` for `cfg`: the largest radius present, or that a
    /// `fix insert` can still add (`PairGran::init_style`: `max_rad(type)` of
    /// fixes), through [`LiggghtsNeighbor::cutneighmax`].
    fn liggghts_cutneighmax(&self, cfg: &LiggghtsNeighbor) -> f64 {
        let max_radius = self.particles.iter().fold(0.0_f64, |m, p| m.max(p.radius));
        let max_radius = self
            .inserters
            .iter()
            .fold(max_radius, |m, f| m.max(f.max_rad()));
        let max_radius = self
            .stream_inserters
            .iter()
            .fold(max_radius, |m, f| m.max(f.max_rad()));
        cfg.cutneighmax(max_radius)
    }

    /// `Atom::setup()` at a run set-up under [`NeighborOrder::Liggghts`]: with
    /// the sort on, `setup_sort_bins` refuses a zero bin size (gh:#1568).
    fn check_sort_setup(&self) -> Result<(), DemError> {
        if let NeighborOrder::Liggghts(cfg) = self.neighbor_order {
            if cfg.sort_every > 0 {
                neighbor_list::sort_bin_size(self.liggghts_cutneighmax(&cfg))?;
            }
        }
        Ok(())
    }

    /// Sort (if due) and rebuild the LIGGGHTS-order half list from the current
    /// positions: `Atom::sort` then `Neighbor::build`.
    fn rebuild_liggghts_list(&mut self, cfg: LiggghtsNeighbor) {
        let n = self.particles.len();
        if self.perm.len() != n {
            self.perm = (0..n).collect();
        }
        let cut = self.liggghts_cutneighmax(&cfg);
        if cfg.sort_every > 0 && (self.setup_phase || self.step_count >= self.next_sort) {
            let x: Vec<Vec3> = self
                .perm
                .iter()
                .map(|&p| self.particles[p].position)
                .collect();
            let order = neighbor_list::atom_sort_permutation(&x, &cfg, cut);
            self.perm = order.iter().map(|&o| self.perm[o]).collect();
            self.next_sort = (self.step_count / cfg.sort_every) * cfg.sort_every + cfg.sort_every;
        }
        // The list build carries a pair's history over only if the pair
        // overlaps at the rebuild (`rsq < radsum*radsum`); until then it
        // persists through separations (see `ShearHistory::end_step_keep_pairs`).
        {
            let by_tag: HashMap<u64, usize> =
                self.tags.iter().enumerate().map(|(i, &t)| (t, i)).collect();
            let particles = &self.particles;
            self.history.retain_pairs(|lo, hi| {
                let (Some(&i), Some(&j)) = (by_tag.get(&(lo as u64)), by_tag.get(&(hi as u64)))
                else {
                    return false;
                };
                let (a, b) = (&particles[i], &particles[j]);
                let (dx, dy, dz) = (
                    a.position.x - b.position.x,
                    a.position.y - b.position.y,
                    a.position.z - b.position.z,
                );
                let rsq = dx * dx + dy * dy + dz * dz;
                let radsum = a.radius + b.radius;
                rsq < radsum * radsum
            });
        }
        if let Some(c) = self.capillary.as_mut() {
            // the bridge flag is carried over a rebuild only for a pair inside
            // `(radsum * contactDistanceFactor)` (`granular_bin_no_newton`:
            // `radsum = (radi + radj) * contactDistanceFactor`)
            let by_tag: HashMap<u64, usize> =
                self.tags.iter().enumerate().map(|(i, &t)| (t, i)).collect();
            let particles = &self.particles;
            let factor = cfg.contact_distance_factor;
            c.bridges.retain(|&(lo, hi)| {
                let (Some(&i), Some(&j)) = (by_tag.get(&lo), by_tag.get(&hi)) else {
                    return false;
                };
                let d = particles[i].position.sub(particles[j].position);
                let radsum = (particles[i].radius + particles[j].radius) * factor;
                d.x * d.x + d.y * d.y + d.z * d.z < radsum * radsum
            });
        }
        if let Some(mc) = self.mc.as_mut() {
            let by_tag: HashMap<u64, usize> =
                self.tags.iter().enumerate().map(|(i, &t)| (t, i)).collect();
            let particles = &self.particles;
            mc.pair_hist.retain(|&(lo, hi), _| {
                let (Some(&i), Some(&j)) = (by_tag.get(&(lo as u64)), by_tag.get(&(hi as u64))) else {
                    return false;
                };
                let d = particles[i].position.sub(particles[j].position);
                let radsum = particles[i].radius + particles[j].radius;
                d.x * d.x + d.y * d.y + d.z * d.z < radsum * radsum
            });
        }
        let x: Vec<Vec3> = self
            .perm
            .iter()
            .map(|&p| self.particles[p].position)
            .collect();
        let r: Vec<f64> = self
            .perm
            .iter()
            .map(|&p| self.particles[p].radius)
            .collect();
        let mut local = Vec::new();
        neighbor_list::half_bin_no_newton(&x, &r, &cfg, cut, &mut local);
        self.pairs.clear();
        self.pairs.extend(
            local
                .iter()
                .map(|&(a, b)| (self.perm[a as usize] as u32, self.perm[b as usize] as u32)),
        );
        self.xhold = self.particles.iter().map(|p| p.position).collect();
        let grid = neighbor_list::BinGrid::new(&cfg, cut);
        self.nbin_of = self.particles.iter().map(|p| grid.coord2bin(p.position)).collect();
        // `FixWallGran::pre_neighbor`: rebuild each primitive wall's list with
        // threshold = skin and the atom radius
        let skin = cfg.skin;
        self.wall_near = self
            .boundaries
            .iter()
            .map(|b| {
                self.particles
                    .iter()
                    .map(|p| b.upstream_neighlist(p, skin).unwrap_or(true))
                    .collect()
            })
            .collect();
    }

    /// Select the compute backend for the timestep, returning the updated
    /// system.
    ///
    /// The default is [`ComputeType::CpuSingleThread`], the trusted scalar
    /// reference. [`ComputeType::CpuMultiThread`] is **bit-identical** to it at
    /// any thread count — see [`crate::compute`] for why that is a requirement
    /// here rather than a bonus, and
    /// [`GranularSystem::resolve_pairs_parallel`] for how it is achieved.
    /// [`ComputeType::Gpu`] runs the scalar path: the DEM timestep has no GPU
    /// kernel (again, see [`crate::compute`]).
    ///
    /// Changing the backend cannot change the trajectory, so this is safe to
    /// set on an already-running system.
    #[must_use]
    pub fn with_compute(mut self, compute: ComputeType) -> Self {
        self.compute = compute;
        self
    }

    /// The compute backend the timestep is using.
    #[must_use]
    pub fn compute(&self) -> ComputeType {
        self.compute
    }

    /// The analytic boundaries, in the order their contacts are resolved.
    #[must_use]
    pub fn boundaries(&self) -> &[Boundary] {
        &self.boundaries
    }

    /// The contact model (the GPU backend reads it, gh:#1250).
    #[must_use]
    pub fn model(&self) -> &GranularContactModel {
        &self.model
    }

    /// Gravitational acceleration \[m/s²\].
    #[must_use]
    pub fn gravity(&self) -> Vec3 {
        self.gravity
    }

    /// The attached moving walls, in their current pose.
    #[must_use]
    pub fn moving_walls(&self) -> &[MovingBoundary] {
        &self.moving_walls
    }

    /// Mutable access to the moving walls, for **changing their prescribed
    /// motion mid-run** — the equivalent of upstream `unfix`-ing a
    /// `fix move/mesh` and installing a different one.
    ///
    /// The angle-of-repose case uses this to stop the lift once the cylinder is
    /// clear of the heap. Changing a wall's *pose* through this handle is also
    /// possible but is not a rigid-body move and will not be seen by the shear
    /// history as such — prefer setting velocities and letting
    /// [`GranularSystem::step`] advance the pose.
    pub fn moving_walls_mut(&mut self) -> &mut [MovingBoundary] {
        &mut self.moving_walls
    }

    /// The particle ensemble `[m]`/`[m/s]`/… (see [`Particle`]).
    #[must_use]
    pub fn particles(&self) -> &[Particle] {
        &self.particles
    }

    /// The particles in upstream's **local atom order** (`atom->x[l]`, `l = 0..nlocal`),
    /// the order every LIGGGHTS per-atom sum (`compute ke`, `erotate/sphere`, ...)
    /// is accumulated in. Under [`NeighborOrder::Liggghts`] this is the order after
    /// the last `Atom::sort`; otherwise it is the tag order of
    /// [`GranularSystem::particles`]. Floating-point sums are not associative, so
    /// a bit-identical diagnostic must sum in this order.
    #[must_use]
    pub fn particles_in_local_order(&self) -> Vec<Particle> {
        if self.perm.len() == self.particles.len() && matches!(self.neighbor_order, NeighborOrder::Liggghts(_)) {
            self.perm.iter().map(|&p| self.particles[p].clone()).collect()
        } else {
            self.particles.clone()
        }
    }

    /// The index into [`GranularSystem::particles`] of each atom in the order
    /// [`GranularSystem::particles_in_local_order`] returns them (identity when
    /// the system is not under the LIGGGHTS neighbour order). Lets a per-atom
    /// property stored by tag (e.g. a tracer marker) be summed in local order.
    #[must_use]
    pub fn local_order_indices(&self) -> Vec<usize> {
        if self.perm.len() == self.particles.len() && matches!(self.neighbor_order, NeighborOrder::Liggghts(_)) {
            self.perm.clone()
        } else {
            (0..self.particles.len()).collect()
        }
    }

    /// Assign an atom type (0-based) to every particle, for a model carrying a
    /// [`MaterialTable`](crate::granular::MaterialTable)
    /// ([`GranularContactModel::with_materials`]). Particles inserted later get
    /// type 0.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `types.len()` differs from the particle
    /// count, or a type is `>=` the table's type count (or non-zero with no
    /// table).
    pub fn with_particle_types(mut self, types: Vec<usize>) -> Result<Self, DemError> {
        if types.len() != self.particles.len() {
            return Err(DemError::InvalidInput(format!(
                "{} types for {} particles",
                types.len(),
                self.particles.len()
            )));
        }
        let n_types = self.model.materials.map_or(1, |t| t.n_types());
        if let Some(bad) = types.iter().find(|&&t| t >= n_types) {
            return Err(DemError::InvalidInput(format!(
                "atom type {bad} out of range: the material table has {n_types} type(s)"
            )));
        }
        self.types = types;
        Ok(self)
    }

    /// The stable tag of each particle, parallel to [`GranularSystem::particles`].
    ///
    /// Tags are assigned `0..n` at construction and monotonically thereafter;
    /// **a tag is never reused**, so a tag identifies one pebble for the whole
    /// life of the run even across removals. This is upstream's atom tag.
    #[must_use]
    pub fn tags(&self) -> &[u64] {
        &self.tags
    }

    /// Insert a particle, returning its newly assigned stable tag.
    ///
    /// The upstream analogue is `fix insert`. The particle joins the ensemble
    /// immediately and is seen by the next [`GranularSystem::step`]; forces are
    /// recomputed so the following half-kick sees a consistent state.
    ///
    /// **Insert into free space.** Nothing here checks for overlap with an
    /// existing particle, exactly as upstream's insertion commands rely on
    /// their own region/overlap logic rather than the integrator. A particle
    /// inserted deep inside a packed bed starts with a large overlap and will
    /// be ejected violently by the normal force.
    pub fn insert_particle(&mut self, p: Particle) -> u64 {
        let tag = self.next_tag;
        self.next_tag += 1;
        self.note_inserted();
        self.particles.push(p);
        self.tags.push(tag);
        self.types.push(0);
        self.forces.push(Vec3::zero());
        self.torques.push(Vec3::zero());
        self.compute_forces();
        tag
    }

    /// The height at which a sphere of radius `r` dropped at `(x, y)` would
    /// come to rest on the current ensemble — the **local bed surface**.
    ///
    /// For every existing particle whose horizontal distance `d_h` is under
    /// `r + r_j`, the dropped sphere touches it at
    /// `z_j + sqrt((r + r_j)² − d_h²)`; the surface is the highest such
    /// tangency. `fallback` is returned when nothing lies under `(x, y)`.
    ///
    /// # Why this exists: inserting into a packed bed
    ///
    /// [`GranularSystem::insert_particle`] documents that it does not check for
    /// overlap, and that a particle inserted inside a packed bed "will be
    /// ejected violently by the normal force". This is the routine that lets a
    /// caller avoid that, and the numbers are not marginal: inserting an HTR-10
    /// pebble 0.04 m into the bed gives a Hertz force of ~3.4e5 N on a 0.196 kg
    /// pebble, i.e. **Δv ≈ 61 m/s in a single 35 µs step**.
    ///
    /// Placing at the returned height gives **exactly zero overlap** by
    /// construction — the separation from the supporting particle is
    /// `sqrt(d_h² + ((r + r_j)² − d_h²)) = r + r_j`, i.e. precisely touching —
    /// so the particle starts at rest in contact rather than as a projectile.
    /// Add a small clearance if a strictly non-negative gap is wanted.
    ///
    /// This is upstream's problem too: LIGGGHTS' `fix insert/pack` carries
    /// `overlapcheck yes` for the same reason. This is the equivalent for a
    /// caller placing particles directly.
    ///
    /// # Cost
    ///
    /// `O(n)` in the ensemble — a linear scan, with no spatial index. Intended
    /// for placing a refuelling batch (tens of particles), not for bulk
    /// generation of a packing.
    #[must_use]
    pub fn surface_height_at(&self, x: f64, y: f64, radius: f64, fallback: f64) -> f64 {
        let mut z = fallback;
        for p in &self.particles {
            let contact = radius + p.radius;
            let dh2 = (p.position.x - x).r_powi(2) + (p.position.y - y).r_powi(2);
            if dh2 < contact * contact {
                let zc = p.position.z + (contact * contact - dh2).sqrt();
                if zc > z {
                    z = zc;
                }
            }
        }
        z
    }

    /// Insert several particles at once, returning their newly assigned stable
    /// tags in the order given.
    ///
    /// Equivalent to [`GranularSystem::insert_particle`] per particle, except
    /// that forces are recomputed **once** at the end instead of once per
    /// particle. Inserting a 50-pebble recirculation batch one at a time costs
    /// 50 full force evaluations — at HTR-10 scale roughly 0.6 s of pure waste
    /// per batch — because each insertion re-solves the whole bed.
    ///
    /// The same caveat applies as for the single-particle form: **insert into
    /// free space.** Nothing here checks for overlap, either against the
    /// existing bed or between the inserted particles themselves.
    pub fn insert_particles(&mut self, particles: impl IntoIterator<Item = Particle>) -> Vec<u64> {
        let mut tags = Vec::new();
        for p in particles {
            let tag = self.next_tag;
            self.next_tag += 1;
            self.note_inserted();
            self.particles.push(p);
            self.tags.push(tag);
            self.types.push(0);
            self.forces.push(Vec3::zero());
            self.torques.push(Vec3::zero());
            tags.push(tag);
        }
        if !tags.is_empty() {
            self.compute_forces();
        }
        tags
    }

    /// Remove the particles at the given **array indices**, returning them in
    /// the order removed.
    ///
    /// The upstream analogue is `delete_atoms` / `fix remove`. Indices are
    /// deduplicated and processed high-to-low so that each removal cannot
    /// invalidate a later one; the underlying operation is `swap_remove`, which
    /// is what LAMMPS does too (it copies the last atom into the hole).
    ///
    /// **Shear history survives this correctly**, which is the whole reason the
    /// store is keyed by tag: surviving particles keep their tags, so their
    /// stored tangential springs still resolve even though their array indices
    /// moved. A removed particle's entries are simply never touched again and
    /// are dropped by the next [`ShearHistory::end_step`] — the same lifecycle
    /// a contact that merely separated goes through.
    ///
    /// Out-of-range indices are ignored rather than panicking, so a caller that
    /// computed a doomed-list from a stale snapshot degrades to removing fewer
    /// particles instead of crashing.
    pub fn remove_particles(&mut self, indices: &[usize]) -> Vec<Particle> {
        let mut doomed: Vec<usize> = indices
            .iter()
            .copied()
            .filter(|&i| i < self.particles.len())
            .collect();
        doomed.sort_unstable();
        doomed.dedup();

        let mut removed = Vec::with_capacity(doomed.len());
        for &i in doomed.iter().rev() {
            self.note_removed(i);
            removed.push(self.particles.swap_remove(i));
            self.tags.swap_remove(i);
            self.types.swap_remove(i);
            self.forces.swap_remove(i);
            self.torques.swap_remove(i);
        }
        if !removed.is_empty() {
            // a clump that lost a member is freed as at a rebuild's
            // `FixMultisphere::pre_neighbor` (gh:#1415); no-op without clumps
            self.multisphere_pre_neighbor();
            self.compute_forces();
        }
        removed
    }

    /// Attach a `fix insert/pack` or `fix insert/rate/region` (gh:#1414). Runs
    /// the fix's set-up (`calc_insertion_properties`) now; it then acts inside
    /// [`GranularSystem::step`] at upstream's `pre_exchange` point: after the
    /// first half-step, before the neighbour build and force evaluation, so a
    /// new particle takes only the second half-kick of its first step. Use with
    /// [`GranularSystem::with_liggghts_neighbor_order`] (the insertion forces a
    /// list rebuild, as `next_reneighbor` does). Returns the fix's index.
    ///
    /// # Errors
    ///
    /// Whatever [`crate::insert::InsertFix::setup`] reports.
    pub fn add_insertion(
        &mut self,
        mut fix: crate::insert::InsertFix,
    ) -> Result<usize, crate::DemError> {
        fix.setup()?;
        self.inserters.push(fix);
        Ok(self.inserters.len() - 1)
    }

    /// Attach a `fix insert/stream` (gh:#1414). Acts at `pre_exchange` like
    /// [`GranularSystem::add_insertion`]; its particles ride the rail to the
    /// insertion face in `end_of_step` (see [`crate::insert_stream`]).
    /// Returns the fix's index.
    pub fn add_stream_insertion(&mut self, fix: crate::insert_stream::InsertStreamFix) -> usize {
        self.stream_inserters.push(fix);
        self.stream_inserters.len() - 1
    }

    /// The `i`-th `insert/stream` fix.
    #[must_use]
    pub fn stream_insertion(&self, i: usize) -> &crate::insert_stream::InsertStreamFix {
        &self.stream_inserters[i]
    }

    /// The `i`-th insertion fix.
    #[must_use]
    pub fn insertion(&self, i: usize) -> &crate::insert::InsertFix {
        &self.inserters[i]
    }

    /// `FixInsert::pre_exchange` for every attached insertion fix.
    ///
    /// Insertion errors upstream would abort the run on; here they panic with
    /// the same message, since `step` has no error channel.
    fn run_insertions(&mut self) {
        let step = self.step_count as u64;
        for k in 0..self.stream_inserters.len() {
            let ins = match self.stream_inserters[k].pre_exchange(step, &self.particles) {
                Ok(v) => v,
                Err(e) => panic!("fix insert/stream: {e}"),
            };
            for (ip, rel) in ins.particles.into_iter().zip(ins.release) {
                let tag = self.next_tag;
                self.next_tag += 1;
                self.note_inserted();
                self.particles.push(ip.particle);
                self.tags.push(tag);
                self.types.push(ip.atom_type - 1);
                self.forces.push(Vec3::zero());
                self.torques.push(Vec3::zero());
                self.stream_release.insert(tag, rel);
            }
        }
        if self.inserters.is_empty() {
            return;
        }
        for k in 0..self.inserters.len() {
            let census = self.clump_census();
            let new = match self.inserters[k].pre_exchange_any(step, &self.particles, &census) {
                Ok(v) => v,
                Err(e) => panic!("fix insert: {e}"),
            };
            let mut clumps_added = false;
            for item in new {
                match item {
                    crate::insert::Inserted::Sphere(ip) => {
                        self.push_inserted_atom(ip.particle, ip.atom_type);
                    }
                    crate::insert::Inserted::Clump(c) => {
                        // `ParticleToInsertMultisphere::insert`: the member atoms
                        // first, then `Multisphere::add_body`
                        let first = self.particles.len();
                        for p in c.atoms() {
                            self.push_inserted_atom(p, c.atom_type());
                        }
                        let dt = self.dt;
                        let noint = self.ms_nointegration;
                        self.multisphere
                            .get_or_insert_with(|| {
                                let ms = crate::multisphere::Multisphere::new(dt);
                                if noint {
                                    ms.without_integration()
                                } else {
                                    ms
                                }
                            })
                            .add_body(&c, first);
                        clumps_added = true;
                    }
                }
            }
            // `FixTemplateMultisphere::finalize_insertion` -> `add_body_finalize`
            if clumps_added {
                if let Some(ms) = self.multisphere.as_mut() {
                    ms.add_body_finalize(&mut self.particles);
                }
            }
        }
    }

    /// Append one inserted atom (new tag, last local slot).
    fn push_inserted_atom(&mut self, p: Particle, atom_type: usize) {
        let tag = self.next_tag;
        self.next_tag += 1;
        self.note_inserted();
        self.particles.push(p);
        self.tags.push(tag);
        self.types.push(atom_type - 1);
        self.forces.push(Vec3::zero());
        self.torques.push(Vec3::zero());
    }

    /// The clumps present, as an insertion fix counts them (gh:#1415).
    fn clump_census(&self) -> crate::insert::ClumpCensus {
        match &self.multisphere {
            None => crate::insert::ClumpCensus::default(),
            Some(ms) => crate::insert::ClumpCensus {
                in_body: (0..self.particles.len()).map(|i| ms.body_of(i).is_some()).collect(),
                bodies: ms
                    .bodies()
                    .iter()
                    .map(|b| (b.x_bound(), b.mass, b.density))
                    .collect(),
            },
        }
    }

    /// The `fix multisphere` state, once a clump has been inserted (gh:#1415).
    #[must_use]
    pub fn multisphere(&self) -> Option<&crate::multisphere::Multisphere> {
        self.multisphere.as_ref()
    }

    /// Whether particle `i` is a member of a clump.
    fn in_body(&self, i: usize) -> bool {
        self.multisphere.as_ref().is_some_and(|m| m.body_of(i).is_some())
    }

    /// `neigh_modify exclude molecule`: drop the pairs of two atoms of the same
    /// clump from the pair list, keeping the order of the rest
    /// (`Neighbor::exclusion`, gh:#1415). No-op without clumps.
    fn exclude_body_pairs(&mut self) {
        if let Some(ms) = &self.multisphere {
            self.pairs.retain(|&(i, j)| {
                let (bi, bj) = (ms.body_of(i as usize), ms.body_of(j as usize));
                !(bi.is_some() && bi == bj)
            });
        }
    }

    /// `boundary f f f` + `thermo_modify lost ignore` (gh:#1415): at every
    /// neighbour-list rebuild, `Comm::exchange` drops each atom outside the
    /// box `[box_lo, box_hi)` of the LIGGGHTS neighbour configuration,
    /// dimension by dimension (x, then y, then z), the last local atom taking
    /// the hole. A clump that loses an atom this way is freed and deleted by
    /// `fix multisphere` ([`crate::multisphere::Multisphere`]). Opt-in, since
    /// the port does not otherwise know the deck's boundary style; without it
    /// an atom that leaves the box is kept, which no upstream run with `f`
    /// boundaries does.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] unless the system is under
    /// [`NeighborOrder::Liggghts`] with the atom sort off (`sort_every == 0`):
    /// the deletion walks the local order, which only that set-up keeps equal
    /// to the particle order.
    pub fn with_lost_atoms(mut self) -> Result<Self, DemError> {
        match self.neighbor_order {
            NeighborOrder::Liggghts(cfg) if cfg.sort_every == 0 => {
                self.lost_atoms_box = Some((cfg.box_lo, cfg.box_hi));
                Ok(self)
            }
            _ => Err(DemError::InvalidInput(
                "lost atoms are ported with the LIGGGHTS neighbour order and 'atom_modify sort 0 0' only"
                    .into(),
            )),
        }
    }

    /// Remove atom `i` the way `AtomVec::copy(nlocal-1, i, 1)` does: the last
    /// atom takes slot `i` in every per-atom array.
    fn swap_remove_atom(&mut self, i: usize) -> Particle {
        self.note_removed(i);
        let p = self.particles.swap_remove(i);
        self.tags.swap_remove(i);
        self.types.swap_remove(i);
        self.forces.swap_remove(i);
        self.torques.swap_remove(i);
        p
    }

    /// `FixMultisphere::pre_exchange`: `next_reneighbor = -1`, then delete the
    /// atoms marked by `check_lost_atoms`, walking the local order with the
    /// last atom filling each hole. No-op without clumps.
    fn multisphere_pre_exchange(&mut self) {
        let Some(ms) = self.multisphere.as_mut() else { return };
        ms.pre_exchange_reset();
        let mut any = false;
        let mut i = 0;
        while i < self.particles.len() {
            if self.multisphere.as_ref().is_some_and(|m| m.delflag(i)) {
                self.swap_remove_atom(i);
                any = true;
            } else {
                i += 1;
            }
        }
        if any {
            self.xhold = self.particles.iter().map(|p| p.position).collect();
        }
    }

    /// `Comm::exchange` on one process with `f` boundaries: per dimension,
    /// drop every atom with `x < lo || x >= hi`. No-op unless
    /// [`GranularSystem::with_lost_atoms`] was called.
    fn exchange_lost_atoms(&mut self) {
        let Some((lo, hi)) = self.lost_atoms_box else { return };
        let (lo, hi) = ([lo.x, lo.y, lo.z], [hi.x, hi.y, hi.z]);
        let mut any = false;
        for dim in 0..3 {
            let mut i = 0;
            while i < self.particles.len() {
                let p = self.particles[i].position;
                let x = [p.x, p.y, p.z][dim];
                if x < lo[dim] || x >= hi[dim] {
                    self.swap_remove_atom(i);
                    any = true;
                } else {
                    i += 1;
                }
            }
        }
        if any {
            self.xhold = self.particles.iter().map(|p| p.position).collect();
        }
    }

    /// `FixMultisphere::pre_neighbor` (`check_lost_atoms`); no-op without
    /// clumps.
    fn multisphere_pre_neighbor(&mut self) {
        let step = self.step_count;
        if let Some(ms) = self.multisphere.as_mut() {
            ms.pre_neighbor(&mut self.particles, step);
        }
    }

    /// Keep the LIGGGHTS local order in step with an insertion: the new atom
    /// takes the last local slot (`nlocal++`) and the list is rebuilt.
    fn note_inserted(&mut self) {
        if matches!(self.neighbor_order, NeighborOrder::Liggghts(_)) {
            self.perm.push(self.particles.len());
            self.rebuild_now = true;
        }
    }

    /// Keep the LIGGGHTS local order in step with removing particle `i`
    /// (called before the `swap_remove`): the last local atom is copied into
    /// the hole (`AtomVec::copy(nlocal-1, i)`), and the particle array's own
    /// `swap_remove` moves its last particle into slot `i`.
    fn note_removed(&mut self, i: usize) {
        // ~~the per-atom body arrays of `fix multisphere` are not compacted
        // (lost-atom/body deletion is not ported, gh:#1415)~~ CORRECTED
        // 2026-11-14 (gh:#1415): they are compacted as `AtomVec::copy` does
        // (`FixMultisphere::copy_arrays`); a clump that lost an atom is freed
        // at the next `pre_neighbor` (`Multisphere::pre_neighbor`).
        let n = self.particles.len();
        if let Some(ms) = self.multisphere.as_mut() {
            ms.swap_remove_atom(i, n);
        }
        if let Some(c) = self.capillary.as_mut() {
            // the removed atom's liquid and bridges leave with it; the last
            // atom takes its slot (`swap_remove`, as the particle array does)
            if i < c.content.len() {
                c.content.swap_remove(i);
                c.flux.swap_remove(i);
            }
            let tag = self.tags[i];
            c.bridges.retain(|&(a, b)| a != tag && b != tag);
            c.wall_bridges.retain(|&(t, _, _)| t != tag);
        }
        if matches!(self.neighbor_order, NeighborOrder::Liggghts(_)) {
            let last = self.particles.len() - 1;
            if let Some(l) = self.perm.iter().position(|&p| p == i) {
                self.perm.swap_remove(l);
            }
            if i != last {
                for e in &mut self.perm {
                    if *e == last {
                        *e = i;
                    }
                }
            }
            self.rebuild_now = true;
        }
    }

    /// Elapsed simulated time `[s]`.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }

    /// The fixed time step `[s]`.
    #[must_use]
    pub fn dt(&self) -> f64 {
        self.dt
    }

    /// Number of contacts currently carrying tangential history.
    #[must_use]
    pub fn live_contacts(&self) -> usize {
        self.history.len()
    }

    /// Every pair of particles **actually in contact** right now, as sorted
    /// `(i, j)` index pairs with `i < j`.
    ///
    /// This is the contact network of the assembly — what a coordination
    /// number, a force-chain analysis, or an overlap audit is computed from.
    /// It is the *resolved* set, not the neighbour-search candidate set: every
    /// returned pair satisfies `δ_n > 0` under
    /// [`ContactKinematics::pair`](crate::granular::ContactKinematics::pair).
    ///
    /// Cost is one neighbour-search pass, the same as a force evaluation, so
    /// call it for analysis rather than inside a timestep loop.
    ///
    /// Particle–wall contacts are **not** included; they have no second
    /// particle index to report.
    #[must_use]
    pub fn contact_pairs(&self) -> Vec<(usize, usize)> {
        let mut out: Vec<(usize, usize)> = self
            .candidate_pairs()
            .into_iter()
            .filter(|&(i, j)| {
                ContactKinematics::pair(&self.particles[i], &self.particles[j]).is_some()
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// One per-contact record in the manner of upstream's `compute
    /// pair/gran/local` (`pos vel id force torque history contactArea
    /// delta`), for the ordered pair of particles with tags `(tag_i, tag_j)`,
    /// or `None` if they do not overlap. `force` and `torque` are the contact's
    /// force and torque **on `i`** (`i_forces.delta_F`, `delta_torque`),
    /// evaluated at the current state from the stored shear history *without*
    /// advancing it (`PairGran::compute_pgl(0, 0)`, see
    /// [`GranularContactModel::resolve_frozen`]).
    ///
    /// Translated from `ComputePairGranLocal::add_pair` (LIGGGHTS-PUBLIC
    /// `3d5c00f2`, `compute_pair_gran_local.cpp`); gh:#1418.
    #[must_use]
    pub fn pair_gran_local_for(&self, tag_i: u64, tag_j: u64) -> Option<PairLocalRecord> {
        let i = self.tags.iter().position(|&t| t == tag_i)?;
        let j = self.tags.iter().position(|&t| t == tag_j)?;
        let (a, b) = (self.particles[i], self.particles[j]);
        let k = ContactKinematics::pair_frozen(
            &a,
            &b,
            self.frozen.get(i).copied().unwrap_or(false),
            self.frozen.get(j).copied().unwrap_or(false),
        )
        .map(|k| k.with_types(self.types[i], self.types[j]))?;
        let key = ContactKey::pair(tag_i as usize, tag_j as usize);
        // The store holds the displacement in the (lower index, higher index)
        // orientation the timestep evaluates pairs in; flip it for `i > j`.
        let stored = self.history.get(key).unwrap_or_else(Vec3::zero);
        let hist = if tag_i < tag_j {
            stored
        } else {
            Vec3::zero().sub(stored)
        };
        let gf = self.model.resolve_frozen(hist, &k, self.dt);
        let (radi, radj) = (a.radius, b.radius);
        let del = a.position.sub(b.position);
        let rsq = del.x * del.x + del.y * del.y + del.z * del.z;
        let r = rsq.sqrt();
        let contact_area = -std::f64::consts::PI / 4.0
            * ((r - radi - radj) * (r + radi - radj) * (r - radi + radj) * (r + radi + radj))
            / rsq;
        Some(PairLocalRecord {
            tag_i,
            tag_j,
            pos_i: a.position,
            pos_j: b.position,
            vel_i: a.velocity,
            vel_j: b.velocity,
            force: gf.force_i,
            torque: gf.torque_i,
            history: hist,
            contact_area,
            delta: radi + radj - r,
        })
    }

    /// `compute wall/gran/local` (`pos vel id force torque history
    /// contactArea delta`): one record per particle-mesh contact, particles in
    /// tag order, facets ascending, mesh walls in order. Force and torque are
    /// evaluated at the current state from the stored history without
    /// advancing it (`FixWallGran::post_force_pgl`: `computeflag = 0`,
    /// `shearupdate = 0`, `addflag = 1`). Primitive walls are not reported,
    /// as upstream warns. Translated from `ComputePairGranLocal::
    /// {add_wall_1, add_wall_2}` and `FixWallGranBase::compute_force`
    /// (LIGGGHTS-PUBLIC `3d5c00f2`); gh:#1418.
    ///
    /// A contact that is not yet in the per-particle contact list is reported
    /// only if no coplanar neighbour facet is (the `coplanarContactAlready`
    /// drop), with zero history; this does not mutate the list.
    #[must_use]
    pub fn wall_gran_local(&self) -> Vec<WallLocalRecord> {
        let nb = self.boundaries.len();
        let mut out = Vec::new();
        let mut mesh_ordinal = 0usize;
        for (w, wall) in self.moving_walls.iter().enumerate() {
            let WallGeometry::Mesh(mesh) = &wall.geometry else {
                continue;
            };
            for (i, p) in self.particles.iter().enumerate() {
                let tag = self.tags[i] as usize;
                for c in mesh.sphere_contacts(p.position, p.radius) {
                    let id = c.tri as u32;
                    let slots = self.mesh_partners.get(&(tag, w));
                    let known = slots.is_some_and(|sl| sl.iter().flatten().any(|s| s.0 == id));
                    if !known && c.barysign == 7 {
                        let dup = slots.is_some_and(|sl| {
                            sl.iter().flatten().any(|s| {
                                s.0 != id && mesh.coplanar_node_neighs(s.0 as usize, c.tri)
                            })
                        });
                        if dup {
                            continue;
                        }
                    }
                    let v_wall = mesh.wall_velocity(c.tri, &c.bary);
                    let delta = Vec3::new(-c.delta[0], -c.delta[1], -c.delta[2]);
                    let overlap = -c.deltan;
                    let k = ContactKinematics::mesh_wall(p, delta, overlap, v_wall)
                        .with_types(self.types[i], self.types[i]);
                    let hist = self
                        .history
                        .get(ContactKey::tri(tag, nb + w, c.tri))
                        .unwrap_or_else(Vec3::zero);
                    let gf = self.model.resolve_frozen(hist, &k, self.dt);
                    let r = p.radius - overlap;
                    let rsq = r * r;
                    out.push(WallLocalRecord {
                        contact_point: Vec3::new(
                            p.position.x - delta.x,
                            p.position.y - delta.y,
                            p.position.z - delta.z,
                        ),
                        pos_i: p.position,
                        wall_vel: v_wall,
                        vel_i: p.velocity,
                        mesh: mesh_ordinal,
                        tri: c.tri,
                        tag: tag as u64,
                        force: gf.force_i,
                        torque: gf.torque_i,
                        history: hist,
                        contact_area: (p.radius * p.radius - rsq) * std::f64::consts::PI,
                        delta: p.radius - rsq.sqrt(),
                    });
                }
            }
            mesh_ordinal += 1;
        }
        out
    }

    /// [`GranularSystem::pair_gran_local_for`] for every overlapping pair, `i`
    /// the lower-tag particle, in candidate-pair order.
    #[must_use]
    pub fn pair_gran_local(&self) -> Vec<PairLocalRecord> {
        self.candidate_pairs()
            .into_iter()
            .filter_map(|(i, j)| {
                let (ti, tj) = (self.tags[i], self.tags[j]);
                self.pair_gran_local_for(ti.min(tj), ti.max(tj))
            })
            .collect()
    }

    /// Mean coordination number `[-]`: contacts per particle, counting both
    /// ends of each contact.
    ///
    /// Returns `0.0` for an empty ensemble. Particle–wall contacts are not
    /// counted (see [`GranularSystem::contact_pairs`]), so a bed's near-wall
    /// layer reads slightly low — for a random close packing of equal spheres
    /// the bulk value is about 6.
    #[must_use]
    pub fn coordination_number(&self) -> f64 {
        if self.particles.is_empty() {
            return 0.0;
        }
        2.0 * self.contact_pairs().len() as f64 / self.particles.len() as f64
    }

    /// Total translational kinetic energy `Σ ½ m v²` `[J]`.
    #[must_use]
    pub fn kinetic_energy(&self) -> f64 {
        self.particles
            .iter()
            .map(|p| 0.5 * p.mass * p.velocity.norm_squared())
            .sum()
    }

    /// Total rotational kinetic energy `Σ ½ I ω²` `[J]`, `I = (2/5) m r²`.
    #[must_use]
    pub fn rotational_energy(&self) -> f64 {
        self.particles
            .iter()
            .map(|p| 0.5 * p.moment_of_inertia() * p.angular_velocity.norm_squared())
            .sum()
    }

    /// Advance one velocity-Verlet step, in LIGGGHTS' order.
    pub fn step(&mut self) {
        self.step_count += 1;
        // `FixScalarTransportEquation::initial_integrate`: reset the flux
        if let Some(c) = self.capillary.as_mut() {
            c.reset_flux();
        }
        let (f, t) = (self.forces.clone(), self.torques.clone());
        self.integrator
            .initial_integrate(&mut self.particles, &f, &t);
        // `FixMultisphere::initial_integrate`: the bodies move and `set_xv`
        // overwrites their member atoms' x, v, omega (gh:#1415)
        if let Some(ms) = self.multisphere.as_mut() {
            ms.initial_integrate(&mut self.particles);
        }
        // Walls drift with the particles (upstream advances mesh nodes in the
        // same half-step as the particle positions).
        for w in &mut self.moving_walls {
            w.advance(self.dt);
        }
        // `post_integrate` of `fix massflow/mesh`: after the move, before the
        // neighbour decision and the force evaluation.
        for k in 0..self.massflow.len() {
            // `fix_volumeweight_ms_` exists once a `fix multisphere` does
            if let Some(ms) = &self.multisphere {
                let w: Vec<f64> = (0..self.particles.len()).map(|i| ms.volume_weight(i)).collect();
                if let Err(e) = self.massflow[k].set_volume_weights(w) {
                    panic!("{e}");
                }
            }
            match self.massflow[k].wall() {
                Some(w) => {
                    if let WallGeometry::Mesh(mesh) = &self.moving_walls[w].geometry {
                        self.massflow[k].post_integrate(mesh, &self.particles, &self.tags, self.dt);
                    }
                }
                None => {
                    let mesh = self.massflow[k].mesh_arc();
                    self.massflow[k].post_integrate(&mesh, &self.particles, &self.tags, self.dt);
                }
            }
        }
        self.refresh_moving_meshes();
        // `FixMassflowMesh::pre_exchange` (`delete_atoms yes`): runs on a
        // neighbour-list rebuild step only
        if self.rebuild_now || self.neigh_skin <= 0.0 {
            self.delete_massflow_marked();
        }
        self.run_insertions();
        // a rebuild step: `FixMultisphere::pre_exchange` (deletes the atoms of
        // lost clumps), `Comm::exchange` (drops atoms that left an `f` box),
        // `FixMultisphere::pre_neighbor` (`check_lost_atoms`); gh:#1415
        if self.rebuild_now || self.neigh_skin <= 0.0 {
            self.multisphere_pre_exchange();
            self.exchange_lost_atoms();
            self.multisphere_pre_neighbor();
        }
        // the mesh neighbour lists rebuild when the neighbour list does (every
        // step when no skin was given)
        if self.rebuild_now || self.neigh_skin <= 0.0 {
            self.update_servo_contacts();
            self.rebuild_massflow_lists();
        }
        // `FixMesh::pre_force` -> `clearReverse` (per-facet `f`, `wear_step`;
        // gh:#1512), then `MeshModuleStress::pre_force`
        for w in &mut self.moving_walls {
            if let Some(st) = &mut w.stress {
                st.clear_reverse();
                st.pre_force();
            }
        }
        self.compute_forces();
        if let Some(heat) = &self.heat {
            // post_force of `fix heat/gran/conduction`: the flux of this step
            // (reset at initial_integrate), summed in pair-list order.
            self.heat_flux.clear();
            self.heat_flux.resize(self.particles.len(), 0.0);
            heat.accumulate(
                &self.pairs,
                &self.particles,
                &self.types,
                &mut self.heat_flux,
            );
        }
        let (f2, t2) = (self.forces.clone(), self.torques.clone());
        self.integrator
            .final_integrate(&mut self.particles, &f2, &t2);
        // `FixMultisphere::final_integrate`: sum the member atoms' forces in
        // local order, then the body half-kick and `set_v` (gh:#1415)
        if self.multisphere.is_some() {
            let order = self.local_order_indices();
            let gravity = self.gravity;
            if let Some(ms) = self.multisphere.as_mut() {
                // `FixMultisphere::pre_force` -> `reset_forces(false)`; nothing
                // adds to a body's force between it and `calc_force` here
                ms.reset_forces(false);
                ms.calc_force(&self.particles, &self.forces, &self.torques, &order, gravity);
                ms.final_integrate(&mut self.particles);
            }
        }
        // `MeshModuleStress::final_integrate` (calc_total_force)
        for w in &mut self.moving_walls {
            if let (Some(st), WallGeometry::Mesh(m)) = (&mut w.stress, &w.geometry) {
                let norms: Vec<[f64; 3]> = (0..m.triangles.len())
                    .map(|i| m.state.surface_norm(i))
                    .collect();
                st.final_integrate(&norms);
            }
        }
        // `MeshModuleStressServo::final_integrate`
        for w in &mut self.moving_walls {
            w.servo_final_integrate();
        }
        if let Some(heat) = &self.heat {
            // final_integrate of the scalar transport equation
            heat.advance(&mut self.particles, &self.types, &self.heat_flux, self.dt);
        }
        if let Some(c) = self.capillary.as_mut() {
            // final_integrate of `liquidtransfer`: L += flux * dt
            c.advance(self.dt);
            // `MeshModuleLiquidTransfer::final_integrate`
            for m in c.mesh_liquid.values_mut() {
                m.advance(self.dt);
            }
        }
        self.stream_end_of_step();
        self.time += self.dt;
    }

    /// `FixInsertStream::end_of_step` (the integrating fix): put every
    /// not-yet-released streamed particle on its rail, release it on its step.
    fn stream_end_of_step(&mut self) {
        if self.stream_release.is_empty() {
            return;
        }
        let step = self.step_count as u64;
        let mut finished: Vec<u64> = Vec::new();
        for i in 0..self.particles.len() {
            let tag = self.tags[i];
            let Some(rel) = self.stream_release.get(&tag) else {
                continue;
            };
            match crate::insert_stream::end_of_step_state(rel, step, self.dt) {
                None => finished.push(tag),
                Some((x, v, w, _released)) => {
                    self.particles[i].position = x;
                    self.particles[i].velocity = v;
                    self.particles[i].angular_velocity = w;
                    self.forces[i] = Vec3::zero();
                    self.torques[i] = Vec3::zero();
                }
            }
        }
        for t in finished {
            self.stream_release.remove(&t);
        }
    }

    /// Advance `n_steps` steps.
    pub fn run(&mut self, n_steps: usize) {
        for _ in 0..n_steps {
            self.step();
        }
    }

    /// Recompute every force and torque at the current configuration, advancing
    /// the shear history by one step.
    ///
    /// Order matches upstream: pair contacts, then wall contacts, then the
    /// gravity body force. The [`ShearHistory`] bracket is applied here, so a
    /// contact that separated between steps loses its stored displacement.
    fn compute_forces(&mut self) {
        let setup = self.setup_phase;
        let n = self.particles.len();
        self.forces.clear();
        self.forces.resize(n, Vec3::zero());
        self.torques.clear();
        self.torques.resize(n, Vec3::zero());

        self.rebuild_candidate_pairs();
        self.history.begin_step();
        // `FixMultiContactHalfSpace::pre_force`
        self.mc_pre_force();

        self.sync_capillary();
        // `MeshModuleLiquidTransfer::pre_force` (and `setup_pre_force`)
        if let Some(c) = self.capillary.as_mut() {
            for m in c.mesh_liquid.values_mut() {
                m.reset_flux();
            }
        }

        // --- particle-particle ---
        // Both paths accumulate in the SAME order (ascending candidate-pair
        // index), which is what makes them bit-identical. See `compute`.
        if self.capillary.is_some() {
            self.resolve_pairs_capillary();
        } else if self.mc.is_some() {
            self.resolve_pairs_mc();
        } else if self.compute.is_parallel_step() && !setup && !self.model.has_contact_state() {
            self.resolve_pairs_parallel();
        } else {
            self.resolve_pairs_serial();
        }

        // --- wall and force fixes, in deck declaration order ---
        // Upstream runs every `post_force` in the order the fixes were declared
        // (`Modify::post_force`, modify.cpp loops `list_post_force`), and the
        // per-contact `f_pw = f_after - f_before` and every force sum depend on
        // that order (gh:#1482). The default order below reproduces the
        // previous fixed one; `with_fix_order` sets the deck's.
        let order = self.effective_fix_order();
        let ff_ctx = if self.force_fixes.is_empty() {
            None
        } else {
            let ctx = VarContext {
                step: self.step_count as i64,
                begin_step: 0,
                end_step: i64::MAX,
                dt: self.dt,
            };
            let vel: Vec<Vec3> = self.particles.iter().map(|p| p.velocity).collect();
            let lorder = self.local_order_indices();
            let pos: Vec<Vec3> = self.particles.iter().map(|p| p.position).collect();
            let rad: Vec<f64> = self.particles.iter().map(|p| p.radius).collect();
            Some((ctx, vel, lorder, pos, rad))
        };
        for slot in order {
            match slot {
                FixSlot::PrimitiveWall(w) => {
                    if self.capillary.is_some() {
                        self.primitive_wall_forces_capillary(w);
                    } else {
                        self.primitive_wall_forces(w);
                    }
                }
                FixSlot::MovingWall(w) => {
                    if self.moving_walls[w].contactless {
                        // `fix mesh/surface` with no `fix wall/gran`: moves, never touches
                    } else if matches!(self.moving_walls[w].geometry, WallGeometry::Mesh(_)) {
                        self.compute_mesh_wall_forces(w);
                    } else {
                        self.moving_primitive_wall_forces(w);
                    }
                }
                FixSlot::Gravity => {
                    for i in 0..n {
                        // `FixGravity::post_force` skips clump members; the body
                        // gets `m g` in `calc_force` (gh:#1415)
                        if self.in_body(i) {
                            continue;
                        }
                        let m = self.particles[i].mass;
                        self.forces[i] = self.forces[i].add(self.gravity.scale(m));
                    }
                }
                FixSlot::MassflowSieve(k) => match self.massflow[k].wall() {
                    Some(w) => {
                        if let WallGeometry::Mesh(mesh) = &self.moving_walls[w].geometry {
                            self.massflow[k].sieve_post_force(
                                mesh,
                                &self.particles,
                                &self.tags,
                                &mut self.forces,
                            );
                        }
                    }
                    None => {
                        let mesh = self.massflow[k].mesh_arc();
                        self.massflow[k].sieve_post_force(
                            &mesh,
                            &self.particles,
                            &self.tags,
                            &mut self.forces,
                        );
                    }
                },
                FixSlot::ForceFix(k) => {
                    if let Some((ctx, vel, lorder, pos, rad)) = &ff_ctx {
                        self.force_fixes[k]
                            .apply(ctx, lorder, pos, rad, vel, &self.types, &mut self.forces, &mut self.torques)
                            .expect("force fix variable evaluation (upstream error->all)");
                    }
                }
            }
        }

        // the history bracket closes after the last wall contact; gravity and
        // the force fixes never touch it, so their position in the order is free
        if matches!(self.neighbor_order, NeighborOrder::Liggghts(_)) {
            self.history.end_step_keep_pairs();
        } else {
            self.history.end_step();
        }
    }

    /// The order the wall and force fixes are applied in: the one set by
    /// [`GranularSystem::with_fix_order`], else primitive walls, mesh walls,
    /// other moving walls, gravity, force fixes (gh:#1482).
    fn effective_fix_order(&self) -> Vec<FixSlot> {
        if let Some(o) = &self.fix_order {
            return o.clone();
        }
        let mut o: Vec<FixSlot> = (0..self.boundaries.len()).map(FixSlot::PrimitiveWall).collect();
        for pass_mesh in [true, false] {
            for (w, m) in self.moving_walls.iter().enumerate() {
                if matches!(m.geometry, WallGeometry::Mesh(_)) == pass_mesh {
                    o.push(FixSlot::MovingWall(w));
                }
            }
        }
        o.push(FixSlot::Gravity);
        o.extend((0..self.force_fixes.len()).map(FixSlot::ForceFix));
        o.extend(
            (0..self.massflow.len())
                .filter(|&k| self.massflow[k].is_sieve())
                .map(FixSlot::MassflowSieve),
        );
        o
    }

    /// Declare the **order of the wall and force fixes** as the deck does
    /// (`Modify::post_force` applies them in declaration order). Each primitive
    /// boundary, moving wall (mesh or analytic), the gravity fix and each force
    /// fix must appear exactly once. Call after `with_moving_walls` and
    /// `with_force_fixes`. gh:#1482.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if a slot is out of range, repeated or missing.
    pub fn with_fix_order(mut self, order: Vec<FixSlot>) -> Result<Self, DemError> {
        self.fix_order = None;
        let key = |s: &FixSlot| format!("{s:?}");
        let mut a: Vec<String> = order.iter().map(key).collect();
        let mut b: Vec<String> = self.effective_fix_order().iter().map(key).collect();
        a.sort();
        b.sort();
        if a != b {
            return Err(DemError::InvalidInput(
                "fix order must list every wall, gravity and force fix exactly once".into(),
            ));
        }
        self.fix_order = Some(order);
        // upstream's set-up evaluation also runs in declaration order
        self.compute_forces_setup();
        Ok(self)
    }

    /// One primitive (analytic, immovable) wall's contacts for every particle
    /// (`FixWallGran::post_force_wall` of one `fix wall/gran primitive`).
    fn primitive_wall_forces(&mut self, w: usize) {
        let setup = self.setup_phase;
        let n = self.particles.len();
        for i in 0..n {
            let mut p = self.particles[i];
            let radius0 = p.radius;
            if let Some(mc) = &self.mc {
                p.radius += mc.data.wall_expansion(w, mc.local_of[i], 1);
            }
            let boundary = &self.boundaries[w];
            // Axis-aligned planes and cylinders go through upstream's own
            // resolution and kinematics (`PrimitiveWall::resolveContact`,
            // `r = radi - deltan`, `en = delta*rinv`).
            if let Some(res) = boundary.upstream_contact(&p) {
                if let Some((overlap, delta)) = res {
                    let k = ContactKinematics::mesh_wall(&p, delta, overlap, Vec3::zero())
                        .with_types(self.types[i], self.types[i]);
                    let gf = resolve_c(
                        &self.model,
                        setup,
                        ContactKey::wall(self.tags[i] as usize, w),
                        &k,
                        &mut self.history,
                        self.dt,
                    );
                    self.forces[i] = self.forces[i].add(gf.force_i);
                    self.torques[i] = self.torques[i].add(gf.torque_i);
                    if self.mc.is_some() && !setup {
                        let h = crate::multicontact::surfaces_intersect_history(true, arr(delta), p.radius, 0.0);
                        let fnorm = crate::multicontact::end_surfaces_intersect_history(arr(gf.force_i), arr(k.en));
                        self.mc.as_mut().expect("multicontact").wall_hist.insert(
                            (self.tags[i] as usize, w),
                            [h[0], h[1], h[2], fnorm],
                        );
                    }
                } else if self.mc.is_some()
                    && !setup
                    && self.wall_near.get(w).is_none_or(|v| v[i])
                {
                    // `surfacesClose` of an axis-aligned plane within one
                    // (unexpanded) radius: delta*radi*rinv with rinv = -1/|delta|
                    if let Some((deltan, dv)) = boundary.cylinder_resolve(&p) {
                        // Cylinder<dim>::resolveContact: `delta` is defined
                        // for deltan > 0 too; `deltan < (factor-1)*radius[i]`.
                        if deltan < radius0 {
                            let h = crate::multicontact::surfaces_close_history(true, [dv.x, dv.y, dv.z], p.radius, 0.0);
                            self.mc.as_mut().expect("multicontact").wall_hist.insert(
                                (self.tags[i] as usize, w),
                                [h[0], h[1], h[2], h[3]],
                            );
                        }
                    } else if let Some((a, param)) = boundary.plane_axis_and_param() {
                        let pos = [p.position.x, p.position.y, p.position.z];
                        let dist = pos[a] - param;
                        let deltan = dist.abs() - p.radius;
                        if deltan < radius0 {
                            let mut dv = [0.0; 3];
                            dv[a] = dist;
                            let h = crate::multicontact::surfaces_close_history(true, dv, p.radius, 0.0);
                            self.mc.as_mut().expect("multicontact").wall_hist.insert(
                                (self.tags[i] as usize, w),
                                [h[0], h[1], h[2], h[3]],
                            );
                        }
                    }
                }
                continue;
            }
            if let Some(c) = boundary.particle_overlap(&p) {
                // `Contact::normal` is the inward normal (wall -> particle),
                // which is exactly upstream's `en` for a wall contact.
                if let Some(k) = ContactKinematics::wall(&p, c.normal, c.overlap, Vec3::zero())
                    .map(|k| k.with_types(self.types[i], self.types[i]))
                {
                    let gf = resolve_c(
                        &self.model,
                        setup,
                        ContactKey::wall(self.tags[i] as usize, w),
                        &k,
                        &mut self.history,
                        self.dt,
                    );
                    self.forces[i] = self.forces[i].add(gf.force_i);
                    self.torques[i] = self.torques[i].add(gf.torque_i);
                }
            }
        }
    }

    /// One primitive wall's contacts with a capillary cohesion model attached
    /// (gh:#1487): `FixWallGran::post_force_primitive` (LIGGGHTS-PUBLIC
    /// `3d5c00f2`). A particle in the wall's neighbour list (rebuilt with the
    /// skin) is resolved by `PrimitiveWall::resolveContact`; an overlap goes
    /// through the contact law with the bridge's `surfacesIntersect` (flag set,
    /// liquid limited), a gap `deltan < (cdf - 1) radius` through the
    /// capillary `surfacesClose` (`r = radi - sidata.deltan`, `radsum = radi`,
    /// wall velocity 0), and anything farther zeroes the contact history, the
    /// bridge flag included, with no rupture flux. A primitive wall is dry.
    fn primitive_wall_forces_capillary(&mut self, w: usize) {
        use crate::capillary::{CloseInput, CloseOutcome};
        let setup = self.setup_phase;
        let GranularCohesion::Capillary(p) = self.model.cohesion else {
            return;
        };
        let cdf = match self.neighbor_order {
            NeighborOrder::Liggghts(cfg) => cfg.contact_distance_factor,
            NeighborOrder::CellGrid => 1.0,
        };
        let contact_distance_multiplier = cdf - 1.0;
        let mut cap = self.capillary.take().expect("capillary state");
        for i in 0..self.particles.len() {
            if !self.wall_near.get(w).is_none_or(|v| v.get(i).copied().unwrap_or(true)) {
                continue;
            }
            let part = self.particles[i];
            let Some((deltan, delta)) = self.boundaries[w].upstream_resolve(&part) else {
                continue;
            };
            let tag = self.tags[i];
            let key = (tag, w, 0_usize);
            let t = self.types[i];
            if deltan <= 0.0 {
                // surfacesIntersect: contflag = 1, limit the liquid, bridge force
                cap.wall_bridges.insert(key);
                cap.content[i] = p.limit_content(cap.content[i], t);
                let k = ContactKinematics::mesh_wall(&part, delta, -deltan, Vec3::zero())
                    .with_types(t, t)
                    .with_liquid(cap.content[i], 0.0);
                let gf = resolve_c(
                    &self.model,
                    setup,
                    ContactKey::wall(tag as usize, w),
                    &k,
                    &mut self.history,
                    self.dt,
                );
                self.forces[i] = self.forces[i].add(gf.force_i);
                self.torques[i] = self.torques[i].add(gf.torque_i);
            } else if deltan < contact_distance_multiplier * part.radius {
                cap.content[i] = p.limit_content(cap.content[i], t);
                // `sidata.r = sidata.radi - sidata.deltan`, `sidata.deltan = -deltan`
                let r = part.radius - (-deltan);
                let outcome = p.close(&CloseInput {
                    delta,
                    rsq: r * r,
                    rad_i: part.radius,
                    rad_j: part.radius,
                    radsum: part.radius,
                    itype: t,
                    jtype: t,
                    v_i: part.velocity,
                    v_j: Vec3::zero(),
                    omega_i: part.angular_velocity,
                    omega_j: Vec3::zero(),
                    liquid_i: cap.content[i],
                    liquid_j: 0.0,
                    bridge: cap.wall_bridges.contains(&key),
                    dt: self.dt,
                    is_wall: true,
                    wall_liquid_vol1000: None,
                });
                match outcome {
                    CloseOutcome::NoBridge => {}
                    CloseOutcome::ActiveNoForce => {
                        cap.wall_bridges.insert(key);
                    }
                    CloseOutcome::Force { force_i, torque_i, .. } => {
                        cap.wall_bridges.insert(key);
                        self.forces[i] = self.forces[i].add(force_i);
                        self.torques[i] = self.torques[i].add(torque_i);
                    }
                    CloseOutcome::Breaks { .. } | CloseOutcome::WallBreaks { .. } => {
                        cap.wall_bridges.remove(&key);
                    }
                }
            } else {
                // `vectorZeroizeN(c_history[iPart], dnum_)`
                cap.wall_bridges.remove(&key);
            }
        }
        self.capillary = Some(cap);
    }

    /// One non-mesh moving wall's contacts for every particle.
    fn moving_primitive_wall_forces(&mut self, w: usize) {
        let setup = self.setup_phase;
        let n = self.particles.len();
        for i in 0..n {
            let p = self.particles[i];
            let wall = &self.moving_walls[w];
            if let Some(c) = wall.particle_overlap(&p) {
                let v_wall = wall.surface_velocity(c.point);
                if let Some(k) = ContactKinematics::wall(&p, c.normal, c.overlap, v_wall)
                    .map(|k| k.with_types(self.types[i], self.types[i]))
                {
                    let gf = resolve_c(
                        &self.model,
                        setup,
                        // Wall ids are offset past the static boundaries so
                        // the two families cannot collide in the history map.
                        ContactKey::wall(self.tags[i] as usize, self.boundaries.len() + w),
                        &k,
                        &mut self.history,
                        self.dt,
                    );
                    self.forces[i] = self.forces[i].add(gf.force_i);
                    self.torques[i] = self.torques[i].add(gf.torque_i);
                }
            }
        }
    }

    /// The `post_force` fixes, in declaration order (read `ewall` of a
    /// `WallRegion` for the thermo scalar and vector).
    #[must_use]
    pub fn force_fixes(&self) -> &[ForceFix] {
        &self.force_fixes
    }

    /// Add `post_force` fixes (`fix addforce`/`viscous`/`freeze`/`wall/region`), applied after
    /// gravity in the order added (gh:#1418). Group masks must have one entry
    /// per particle.
    #[must_use]
    pub fn with_force_fixes(mut self, fixes: Vec<ForceFix>) -> Self {
        let mut frozen = vec![false; self.particles.len()];
        for fx in &fixes {
            if let ForceFix::Freeze { group, .. } = fx {
                for (f, g) in frozen.iter_mut().zip(group) {
                    *f |= *g;
                }
            }
        }
        self.frozen = frozen;
        self.force_fixes = fixes;
        self.fix_order = None;
        // upstream `setup` calls every fix's post_force, so the step-0 forces
        // (used by the first half-kick) already carry the fixes
        self.compute_forces_setup();
        self
    }

    /// Particle–mesh-wall forces, translated from `FixWallGran::
    /// post_force_mesh`: every facet the sphere overlaps contributes its own
    /// contact (history keyed per facet), in ascending facet order; edge and
    /// corner contacts of inactive features are skipped; a new face contact
    /// is dropped when a coplanar neighbour facet is already handled this step
    /// and inherits that neighbour's history otherwise
    /// (`FixContactHistoryMesh::handleContact`). gh:#1416.
    fn compute_mesh_wall_forces(&mut self, w: usize) {
        let setup = self.setup_phase;
        if setup {
            // ~~the set-up evaluation's wear goes into step 1 (nothing zeroes
            // `wear_step` before `final_integrate`)~~ CORRECTED 2026-11-13
            // (gh:#1512): step 1's `FixMesh::pre_force` -> `clearReverse`
            // discards it (`MeshStress::clear_reverse`); a repeated set-up here
            // replaces, not adds to, it
            if let Some(st) = self.moving_walls[w].stress.as_mut() {
                st.begin_setup_wear();
            }
        }
        let n = self.particles.len();
        let nb = self.boundaries.len();
        {
            let WallGeometry::Mesh(mesh) = &self.moving_walls[w].geometry else {
                return;
            };
            let has_stress = self.moving_walls[w].stress.is_some();
            let mut contributions: Vec<StressContribution> = Vec::new();
            // markAllContacts
            for (key, slots) in self.mesh_partners.iter_mut() {
                if key.1 == w {
                    for s in slots.iter_mut().flatten() {
                        s.1 = false;
                    }
                }
            }
            for i in 0..n {
                let p = self.particles[i];
                let tag = self.tags[i] as usize;
                // multicontact: `radi += deltaData[3]` of a facet already in
                // the history (`fix_wall_gran.cpp:933-947`); the search also
                // reaches the "close" facets `deltan < (factor-1)*radius`
                let contacts = if let Some(mc) = &self.mc {
                    let loc = mc.local_of[i];
                    let nbw = nb;
                    let exp_of = |t: u32| mc.data.wall_expansion(nbw + w, loc, t as usize);
                    let maxexp = self
                        .mesh_partners
                        .get(&(tag, w))
                        .map_or(0.0, |sl| sl.iter().flatten().map(|s| exp_of(s.0).abs()).fold(0.0, f64::max));
                    let margin = (crate::multicontact::CONTACT_DISTANCE_FACTOR - 1.0) * p.radius;
                    let cand = mesh.sphere_contacts_within(p.position, p.radius + maxexp, margin + 2.0 * maxexp);
                    let slots = self.mesh_partners.get(&(tag, w));
                    cand.into_iter()
                        .filter_map(|c0| {
                            let has = slots.is_some_and(|sl| sl.iter().flatten().any(|s| s.0 == c0.tri as u32));
                            let re = if has { exp_of(c0.tri as u32) } else { 0.0 };
                            let c1 = if re != 0.0 { mesh.resolve_facet(c0.tri, p.position, p.radius + re) } else { mesh.resolve_facet(c0.tri, p.position, p.radius) };
                            (c1.deltan <= 0.0 || c1.deltan < margin).then_some(c1)
                        })
                        .collect::<Vec<_>>()
                } else if self.capillary.is_some() {
                    // capillary bridges: `deltan < (cdf - 1) * radius` too (gh:#1487)
                    let cdf = match self.neighbor_order {
                        NeighborOrder::Liggghts(cfg) => cfg.contact_distance_factor,
                        NeighborOrder::CellGrid => 1.0,
                    };
                    mesh.sphere_contacts_within(p.position, p.radius, (cdf - 1.0) * p.radius)
                } else {
                    mesh.sphere_contacts(p.position, p.radius)
                };
                for c in contacts {
                    let slots = self.mesh_partners.entry((tag, w)).or_default();
                    let id = c.tri as u32;
                    let face = c.barysign == 7;
                    // haveContact
                    let mut handled = false;
                    for s in slots.iter_mut().flatten() {
                        if s.0 == id {
                            s.1 = true;
                            handled = true;
                            break;
                        }
                    }
                    if !handled {
                        // coplanarContactAlready
                        if face {
                            let mut already = false;
                            for s in slots.iter().flatten() {
                                if s.0 != id
                                    && mesh.coplanar_node_neighs(s.0 as usize, c.tri)
                                    && s.1
                                {
                                    already = true;
                                }
                            }
                            if already {
                                continue;
                            }
                        }
                        // addNewTriContactToExistingParticle (history zeroed)
                        if let Some(cap) = self.capillary.as_mut() {
                            cap.wall_bridges.remove(&(tag as u64, nb + w, c.tri));
                        }
                        let new = Some((id, true));
                        if let Some(slot) = slots.iter_mut().find(|s| s.is_none()) {
                            *slot = new;
                        } else {
                            slots.push(new);
                        }
                        // checkCoplanarContactHistory
                        if face {
                            let mut inherit = None;
                            for s in slots.iter().flatten() {
                                if s.0 != id && mesh.coplanar_node_neighs(s.0 as usize, c.tri) {
                                    inherit = self
                                        .history
                                        .get(ContactKey::tri(tag, nb + w, s.0 as usize))
                                        .or(Some(Vec3::zero()));
                                }
                            }
                            if let Some(h) = inherit {
                                self.history.seed(ContactKey::tri(tag, nb + w, c.tri), h);
                            }
                            // the capillary bridge flag is in the same history
                            // array (`checkCoplanarContactHistory`, gh:#1487)
                            if let Some(cap) = self.capillary.as_mut() {
                                let mut flag = None;
                                for s in slots.iter().flatten() {
                                    if s.0 != id && mesh.coplanar_node_neighs(s.0 as usize, c.tri) {
                                        flag = Some(cap.wall_bridges.contains(&(tag as u64, nb + w, s.0 as usize)));
                                    }
                                }
                                if flag == Some(true) {
                                    cap.wall_bridges.insert((tag as u64, nb + w, c.tri));
                                }
                            }
                            // the multicontact history lives in the same
                            // contact history, so it is inherited too
                            if let Some(mc) = self.mc.as_mut() {
                                let mut src = None;
                                for s in slots.iter().flatten() {
                                    if s.0 != id && mesh.coplanar_node_neighs(s.0 as usize, c.tri) {
                                        src = Some(mc.mesh_hist.get(&(tag, nb + w, s.0)).copied().unwrap_or([0.0; 4]));
                                    }
                                }
                                if let Some(h) = src {
                                    mc.mesh_hist.insert((tag, nb + w, id), h);
                                }
                            }
                        }
                    }
                    // expanded radius of this facet (`sidata.radi`)
                    let mut p = p;
                    if let Some(mc) = &self.mc {
                        p.radius += mc.data.wall_expansion(nb + w, mc.local_of[i], c.tri);
                    }
                    if c.deltan > 0.0 && self.capillary.is_some() {
                        // capillary `surfacesClose` on a facet (gh:#1487)
                        if let (Some(cap), GranularCohesion::Capillary(cp)) =
                            (self.capillary.as_mut(), self.model.cohesion)
                        {
                            Self::mesh_close_capillary(
                                &cp,
                                cap,
                                MeshCloseSite {
                                    wall: w,
                                    key: (tag as u64, nb + w, c.tri),
                                    i,
                                    atype: self.types[i],
                                    v_wall: mesh.wall_velocity(c.tri, &c.bary),
                                    area: mesh.state.area(c.tri),
                                    dt: self.dt,
                                },
                                &p,
                                &c,
                                &mut self.forces[i],
                                &mut self.torques[i],
                            );
                        }
                        continue;
                    }
                    if c.deltan > 0.0 {
                        // `surfacesClose`: history only, no force
                        if let Some(mc) = self.mc.as_mut().filter(|_| !setup) {
                            let d = [-c.delta[0], -c.delta[1], -c.delta[2]];
                            let h = crate::multicontact::surfaces_close_history(true, d, p.radius, 0.0);
                            mc.mesh_hist.insert((tag, nb + w, id), [h[0], h[1], h[2], h[3]]);
                        }
                        continue;
                    }
                    let v_wall = mesh.wall_velocity(c.tri, &c.bary);
                    let delta = Vec3::new(-c.delta[0], -c.delta[1], -c.delta[2]);
                    let overlap = -c.deltan;
                    let mut k = ContactKinematics::mesh_wall(&p, delta, overlap, v_wall)
                        .with_types(self.types[i], self.types[i]);
                    if let (Some(cap), GranularCohesion::Capillary(cp)) =
                        (self.capillary.as_mut(), self.model.cohesion)
                    {
                        // capillary `surfacesIntersect`: contflag = 1, limit
                        // the particle's liquid, the facet's film (gh:#1487)
                        cap.wall_bridges.insert((tag as u64, nb + w, c.tri));
                        cap.content[i] = cp.limit_content(cap.content[i], self.types[i]);
                        let vol = cap
                            .mesh_liquid
                            .get(&w)
                            .map_or(0.0, |m| m.wall_volume1000(c.tri, p.radius, mesh.state.area(c.tri)));
                        k = k.with_liquid(cap.content[i], 0.0).with_wall_liquid(vol);
                    }
                    let gf = resolve_c(
                        &self.model,
                        setup,
                        ContactKey::tri(tag, nb + w, c.tri),
                        &k,
                        &mut self.history,
                        self.dt,
                    );
                    if let Some(mc) = self.mc.as_mut().filter(|_| !setup) {
                        let d = [-c.delta[0], -c.delta[1], -c.delta[2]];
                        let h = crate::multicontact::surfaces_intersect_history(true, d, p.radius, 0.0);
                        let fnorm = crate::multicontact::end_surfaces_intersect_history(arr(gf.force_i), arr(k.en));
                        mc.mesh_hist.insert((tag, nb + w, id), [h[0], h[1], h[2], fnorm]);
                    }
                    // f_pw = f - force_old (fix_wall_gran_base.h), delta passed on is
                    // -sidata.delta
                    let before = self.forces[i];
                    self.forces[i] = self.forces[i].add(gf.force_i);
                    self.torques[i] = self.torques[i].add(gf.torque_i);
                    if has_stress {
                        let f_pw = self.forces[i].sub(before);
                        contributions.push(StressContribution {
                            i,
                            tri: c.tri,
                            f_pw: arr(f_pw),
                            x: arr(p.position),
                            delta: c.delta,
                            v: arr(p.velocity),
                            v_wall: arr(v_wall),
                            norm: mesh.state.surface_norm(c.tri),
                        });
                    }
                }
            }
            if let Some(st) = &mut self.moving_walls[w].stress {
                // Upstream's `post_force_mesh` loops facets outermost and, within a
                // facet, the particles of that facet's mesh neighbour list. That
                // list (`FixNeighlistMesh::handleTriangle`) walks the facet's bins
                // in the `ix, iy, iz` loop order of `generate_bin_list` (`iz`
                // innermost) and each bin's linked list in ascending local
                // (atom-sort) slot, on the bins of the last list build. The
                // totals of `fix mesh/surface/stress` are floating-point sums, so
                // they are accumulated in that order (the per-contact forces do
                // not depend on it). gh:#1416, gh:#1491,
                // `tests/htr10_conus_massflow_stress_cross_code.rs`.
                if self.perm.len() == n && matches!(self.neighbor_order, NeighborOrder::Liggghts(_)) {
                    let mut local_of = vec![0usize; n];
                    for (l, &pi) in self.perm.iter().enumerate() {
                        local_of[pi] = l;
                    }
                    let bins = &self.nbin_of;
                    let bin = |i: usize| if bins.len() == n { bins[i] } else { (0, 0, 0) };
                    contributions.sort_by_key(|c| (c.tri, bin(c.i), local_of[c.i]));
                }
                for c in contributions {
                    st.add_particle_contribution(c.f_pw, c.x, c.delta, c.tri);
                    // `trackWear()`: the same call, after the stress part
                    st.add_wear_contribution(&WearContact {
                        frc: c.f_pw,
                        x: c.x,
                        v: c.v,
                        delta: c.delta,
                        v_wall: c.v_wall,
                        tri: c.tri,
                        norm: c.norm,
                        atom_type: self.types[c.i],
                        dt: self.dt,
                    });
                }
            }
            // cleanUpContacts
            for (key, slots) in self.mesh_partners.iter_mut() {
                if key.1 == w {
                    for s in slots.iter_mut() {
                        if matches!(s, Some((_, false))) {
                            *s = None;
                        }
                    }
                }
            }
            self.mesh_partners
                .retain(|_, v| v.iter().any(Option::is_some));
            if let Some(cap) = self.capillary.as_mut() {
                // cleanUpContacts zeroes the history of a facet no longer in
                // contact, the bridge flag included (gh:#1487)
                let mp = &self.mesh_partners;
                cap.wall_bridges.retain(|&(t, ww, tri)| {
                    ww != nb + w
                        || mp
                            .get(&(t as usize, w))
                            .is_some_and(|sl| sl.iter().flatten().any(|s| s.0 as usize == tri))
                });
            }
            if let Some(mc) = self.mc.as_mut() {
                let mp = &self.mesh_partners;
                mc.mesh_hist.retain(|&(t, ww, id), _| {
                    ww != nb + w
                        || mp.get(&(t, w)).is_some_and(|sl| sl.iter().flatten().any(|s| s.0 == id))
                });
            }
        }
    }

    /// The capillary `surfacesClose` of particle `i` with facet `c.tri` of
    /// mesh wall `w` (`FixWallGran::post_force_mesh` -> `compute_force` ->
    /// `surfacesClose`, gh:#1487): `sidata.delta = -delta`,
    /// `r = radi - sidata.deltan`, `radsum = radi`, `v_j` the facet velocity
    /// at the contact; Washino reads the facet's film when the mesh carries
    /// `liquidtransfer` and, at rupture, moves liquid between the particle and
    /// the facet (`add_liquid_flux`).
    fn mesh_close_capillary(
        cp: &crate::capillary::CapillaryParams,
        cap: &mut crate::capillary::CapillaryState,
        site: MeshCloseSite,
        p: &Particle,
        c: &crate::mesh_wall::MeshContact,
        force: &mut Vec3,
        torque: &mut Vec3,
    ) {
        use crate::capillary::{CloseInput, CloseOutcome};
        let MeshCloseSite { wall: w, key, i, atype: t, v_wall, area, dt } = site;
        cap.content[i] = cp.limit_content(cap.content[i], t);
        let wall_vol = cap.mesh_liquid.get(&w).map(|m| m.wall_volume1000(c.tri, p.radius, area));
        let r = p.radius - (-c.deltan);
        let outcome = cp.close(&CloseInput {
            delta: Vec3::new(-c.delta[0], -c.delta[1], -c.delta[2]),
            rsq: r * r,
            rad_i: p.radius,
            rad_j: p.radius,
            radsum: p.radius,
            itype: t,
            jtype: t,
            v_i: p.velocity,
            v_j: v_wall,
            omega_i: p.angular_velocity,
            omega_j: Vec3::zero(),
            liquid_i: cap.content[i],
            liquid_j: 0.0,
            bridge: cap.wall_bridges.contains(&key),
            dt,
            is_wall: true,
            wall_liquid_vol1000: wall_vol,
        });
        match outcome {
            CloseOutcome::NoBridge => {}
            CloseOutcome::ActiveNoForce => {
                cap.wall_bridges.insert(key);
            }
            CloseOutcome::Force { force_i, torque_i, .. } => {
                cap.wall_bridges.insert(key);
                *force = force.add(force_i);
                *torque = torque.add(torque_i);
            }
            CloseOutcome::Breaks { .. } => {
                cap.wall_bridges.remove(&key);
            }
            CloseOutcome::WallBreaks { transfer } => {
                cap.wall_bridges.remove(&key);
                if let Some(tr) = transfer {
                    cap.flux[i] += tr.flux_i;
                    let (limit, max) = if cp.limit_liquid_content {
                        (true, cp.max_liquid_content[t])
                    } else {
                        (false, 0.0)
                    };
                    if let Some(m) = cap.mesh_liquid.get_mut(&w) {
                        m.add_liquid_flux(c.tri, tr.vol_flux, area, limit, max);
                    }
                }
            }
        }
    }

    /// Scalar particle-particle force accumulation — the trusted reference
    /// path, used by [`ComputeType::CpuSingleThread`] and [`ComputeType::Gpu`].
    fn resolve_pairs_serial(&mut self) {
        let setup = self.setup_phase;
        let pairs = std::mem::take(&mut self.pairs);
        for &(i, j) in &pairs {
            let (i, j) = (i as usize, j as usize);
            let a = self.particles[i];
            let b = self.particles[j];
            if let Some(k) = ContactKinematics::pair_frozen(
                &a,
                &b,
                self.frozen.get(i).copied().unwrap_or(false),
                self.frozen.get(j).copied().unwrap_or(false),
            )
            .map(|k| k.with_types(self.types[i], self.types[j]))
            {
                let key = ContactKey::pair(self.tags[i] as usize, self.tags[j] as usize);
                // The store holds the displacement in (lower tag, higher tag)
                // orientation; a list that visits the pair the other way
                // round (LIGGGHTS' sorted local order) flips it around the call.
                let flip = self.tags[i] > self.tags[j];
                if flip {
                    self.history.flip(key);
                    self.model.flip_aux(key, &mut self.history);
                }
                let gf = resolve_c(&self.model, setup, key, &k, &mut self.history, self.dt);
                if flip {
                    self.history.flip(key);
                    self.model.flip_aux(key, &mut self.history);
                }
                self.forces[i] = self.forces[i].add(gf.force_i);
                self.forces[j] = self.forces[j].add(gf.force_j);
                self.torques[i] = self.torques[i].add(gf.torque_i);
                self.torques[j] = self.torques[j].add(gf.torque_j);
            }
        }
        self.pairs = pairs;
    }

    /// Rayon-parallel particle-particle force accumulation, **bit-identical**
    /// to [`GranularSystem::resolve_pairs_serial`] for any thread count.
    ///
    /// # How bit-identity is obtained
    ///
    /// Floating-point addition is not associative, so the thing that must be
    /// preserved is not the arithmetic but the **accumulation order**. The loop
    /// is therefore split in two:
    ///
    /// 1. **Parallel, order-free.** Every candidate pair is resolved
    ///    independently through [`GranularContactModel::resolve_pure`], which
    ///    takes the stored tangential spring by value and returns the updated
    ///    one, so it needs only `&ShearHistory`. Results are staged into a
    ///    slot indexed by candidate-pair index — each task writes only its own
    ///    disjoint chunk, so there is no contention and no reduction.
    /// 2. **Serial, ordered.** The staged outcomes are applied in ascending
    ///    pair index — exactly the order the scalar path visits them — so every
    ///    `forces[i] += …` sequence, and every history write, is the identical
    ///    sequence of IEEE-754 operations.
    ///
    /// Distinct contacts touch distinct [`ContactKey`]s, so deferring the
    /// history writes to phase 2 changes nothing: no pair can observe another
    /// pair's write within a step.
    ///
    /// Phase 2 is cheap (vector adds and two hash inserts per live contact)
    /// while phase 1 carries the Hertz/Mindlin/rolling evaluation, so the
    /// serial tail does not dominate.
    fn resolve_pairs_parallel(&mut self) {
        let threads = self.compute.threads();
        let pairs = std::mem::take(&mut self.pairs);
        let mut outcomes = std::mem::take(&mut self.outcomes);
        let mut chunk_counts = std::mem::take(&mut self.chunk_counts);

        // Enough tasks to balance a ragged bed, few enough that per-task
        // overhead stays negligible against the contact arithmetic.
        let chunk = (pairs.len() / (threads * 8)).max(2048);
        let n_chunks = pairs.len().div_ceil(chunk);
        // Grown, never re-initialised per step. Writing a default into all
        // ~370 000 slots every step was measured at 2.9 ms of a 17.8 ms
        // timestep — 54 MB of memset to stage 9 MB of results.
        if outcomes.len() < pairs.len() {
            outcomes.resize(pairs.len(), PairOutcome::EMPTY);
        }
        if chunk_counts.len() < n_chunks {
            chunk_counts.resize(n_chunks, 0);
        }

        let has_history = self.model.has_tangential_history();
        {
            let particles: &[Particle] = &self.particles;
            let tags: &[u64] = &self.tags;
            let types: &[usize] = &self.types;
            let frozen: &[bool] = &self.frozen;
            let model = &self.model;
            let history = &self.history;
            let dt = self.dt;
            shared_pool(threads).install(|| {
                outcomes[..pairs.len()]
                    .par_chunks_mut(chunk)
                    .zip(pairs.par_chunks(chunk))
                    .zip(chunk_counts[..n_chunks].par_chunks_mut(1))
                    .for_each(|((out_chunk, pair_chunk), count)| {
                        // Compact: only real contacts are written, in order.
                        // ~18 % of candidates are contacts on a settled bed, so
                        // this stages a fifth of the traffic a slot-per-candidate
                        // layout would.
                        let mut written = 0usize;
                        for &(i, j) in pair_chunk {
                            let (i, j) = (i as usize, j as usize);
                            let a = particles[i];
                            let b = particles[j];
                            let Some(k) = ContactKinematics::pair_frozen(
                                &a,
                                &b,
                                frozen.get(i).copied().unwrap_or(false),
                                frozen.get(j).copied().unwrap_or(false),
                            )
                            .map(|k| k.with_types(types[i], types[j])) else {
                                continue;
                            };
                            let key = ContactKey::pair(tags[i] as usize, tags[j] as usize);
                            let flip = tags[i] > tags[j];
                            let stored = history.get(key);
                            let prior = match (stored, flip) {
                                (Some(h), true) => Vec3::zero().sub(h),
                                (Some(h), false) => h,
                                (None, _) => Vec3::zero(),
                            };
                            let (gf, shear) = model.resolve_pure(prior, &k, dt);
                            let shear = if flip {
                                shear.map(|s| Vec3::zero().sub(s))
                            } else {
                                shear
                            };
                            out_chunk[written] = PairOutcome {
                                i: i as u32,
                                j: j as u32,
                                force_i: gf.force_i,
                                torque_i: gf.torque_i,
                                torque_j: gf.torque_j,
                                shear: shear.unwrap_or_else(Vec3::zero),
                            };
                            written += 1;
                        }
                        count[0] = written as u32;
                    });
            });
        }

        // Phase 2: serial, chunk by chunk in ascending order and compactly
        // within each chunk — i.e. exactly the candidate-pair order the scalar
        // path visits, which is what makes the two bit-identical.
        for c in 0..n_chunks {
            let base = c * chunk;
            for outcome in &outcomes[base..base + chunk_counts[c] as usize] {
                let (i, j) = (outcome.i as usize, outcome.j as usize);
                self.forces[i] = self.forces[i].add(outcome.force_i);
                // Newton's third law, written exactly as `resolve` builds
                // `force_j`, so the scalar and parallel paths perform the same
                // IEEE-754 operation rather than merely an equivalent one.
                self.forces[j] = self.forces[j].add(outcome.force_i.scale(-1.0));
                self.torques[i] = self.torques[i].add(outcome.torque_i);
                self.torques[j] = self.torques[j].add(outcome.torque_j);
                if has_history {
                    self.history.store(
                        ContactKey::pair(self.tags[i] as usize, self.tags[j] as usize),
                        outcome.shear,
                    );
                }
            }
        }

        self.pairs = pairs;
        self.outcomes = outcomes;
        self.chunk_counts = chunk_counts;
    }

    /// Candidate near pairs `(i, j)`, `i < j`, each at most once.
    ///
    /// Convenience wrapper over [`GranularSystem::build_pairs`] that allocates
    /// fresh buffers; the timestep uses the buffer-reusing
    /// [`GranularSystem::rebuild_candidate_pairs`] instead.
    fn candidate_pairs(&self) -> Vec<(usize, usize)> {
        let (mut pairs, mut offsets, mut items) = (Vec::new(), Vec::new(), Vec::new());
        Self::build_pairs(&self.particles, &mut pairs, &mut offsets, &mut items);
        pairs
            .into_iter()
            .map(|(i, j)| (i as usize, j as usize))
            .collect()
    }

    /// Refill `self.pairs` with the candidate near pairs, reusing the grid
    /// buffers across steps.
    fn rebuild_candidate_pairs(&mut self) {
        if let NeighborOrder::Liggghts(cfg) = self.neighbor_order {
            if self.setup_phase || self.rebuild_now || self.perm.len() != self.particles.len() {
                self.rebuild_liggghts_list(cfg);
                self.exclude_body_pairs();
                self.rebuild_now = false;
            }
            return;
        }
        let mut pairs = std::mem::take(&mut self.pairs);
        let mut offsets = std::mem::take(&mut self.cell_offsets);
        let mut items = std::mem::take(&mut self.cell_items);
        Self::build_pairs(&self.particles, &mut pairs, &mut offsets, &mut items);
        self.pairs = pairs;
        self.cell_offsets = offsets;
        self.cell_items = items;
        self.exclude_body_pairs();
    }

    /// Above this many grid cells, the dense flat grid is abandoned for the
    /// ordered sparse map — see [`GranularSystem::build_pairs`].
    ///
    /// The bound is proportional to the particle count so it tracks the problem
    /// rather than the machine, with a floor so small ensembles in a large
    /// domain still get the fast path.
    fn max_grid_cells(n: usize) -> usize {
        (64usize.saturating_mul(n)).max(1 << 20)
    }

    /// Enumerate candidate near pairs into `pairs`, using `cell_offsets` and
    /// `cell_items` as scratch. Every unordered pair is emitted **exactly
    /// once**, in a **deterministic order**.
    ///
    /// # Determinism is the point, not a side effect
    ///
    /// ~~Cells were held in a `HashMap` and iterated directly~~ **CORRECTED
    /// 2026-09-17** — `std::collections::HashMap` seeds its hasher randomly
    /// *per process*, so that loop visited cells in a different order on every
    /// run. Since `forces[i] += …` is a floating-point accumulation and
    /// addition is not associative, the bed's trajectory differed run to run in
    /// the last bits, and DEM amplifies that: three identical 200-step runs of
    /// the settled HTR-10 bed gave kinetic energies of
    /// `2.81519841188424304e-2`, `2.81519841188418857e-2` and
    /// `2.81519841188432977e-2` — differing in the 13th significant figure
    /// already. The committed reference bed could therefore not be regenerated
    /// exactly, and one 2.91e-2 m outlier in the per-particle cross-code
    /// comparison is consistent with exactly this. Bead `op-t3l.9`.
    ///
    /// The flat grid below fixes that: cells are visited in ascending linear
    /// index and members in ascending particle index, both independent of any
    /// hash seed.
    ///
    /// # Method
    ///
    /// Cell size is one particle diameter (`2 * max_radius`), so any two
    /// particles in contact lie in the same or an adjacent cell. Membership is
    /// built by a **counting sort** — count per cell, prefix-sum to offsets,
    /// then scatter — which needs no per-cell `Vec` and no allocation once the
    /// buffers are warm. The offset array is restored by a right shift after
    /// the scatter has consumed it as a cursor, so no second cursor buffer is
    /// needed.
    ///
    /// Only the **lexicographically-forward half** of the 26 neighbours is
    /// visited, so each unordered cell pair is reached exactly once and no
    /// deduplication is required.
    fn build_pairs(
        particles: &[Particle],
        pairs: &mut Vec<(u32, u32)>,
        cell_offsets: &mut Vec<u32>,
        cell_items: &mut Vec<u32>,
    ) {
        let n = particles.len();
        pairs.clear();
        if n <= Self::BRUTE_FORCE_THRESHOLD {
            for i in 0..n {
                for j in (i + 1)..n {
                    pairs.push((i as u32, j as u32));
                }
            }
            return;
        }
        assert!(
            n <= u32::MAX as usize,
            "particle count {n} exceeds the u32 index used by the neighbour grid"
        );

        let mut max_radius = 0.0_f64;
        for p in particles {
            max_radius = max_radius.max(p.radius);
        }
        let cell = 2.0 * max_radius;
        let inv = 1.0 / cell;
        let icell = |v: Vec3| -> (i64, i64, i64) {
            (
                (v.x * inv).floor() as i64,
                (v.y * inv).floor() as i64,
                (v.z * inv).floor() as i64,
            )
        };

        let (mut lo, mut hi) = (
            (i64::MAX, i64::MAX, i64::MAX),
            (i64::MIN, i64::MIN, i64::MIN),
        );
        for p in particles {
            let c = icell(p.position);
            lo = (lo.0.min(c.0), lo.1.min(c.1), lo.2.min(c.2));
            hi = (hi.0.max(c.0), hi.1.max(c.1), hi.2.max(c.2));
        }
        let (nx, ny, nz) = (
            (hi.0 - lo.0 + 1) as usize,
            (hi.1 - lo.1 + 1) as usize,
            (hi.2 - lo.2 + 1) as usize,
        );
        let ncells = match nx.checked_mul(ny).and_then(|v| v.checked_mul(nz)) {
            Some(c) if c <= Self::max_grid_cells(n) => c,
            // A domain far emptier than it is large (a long thin chute, a
            // handful of strays far from the bed). The dense grid would be
            // mostly empty and could dwarf the ensemble itself, so fall back to
            // the ordered sparse map, which is slower but still deterministic.
            _ => return Self::build_pairs_sparse(particles, pairs, icell),
        };

        let lin = |c: (i64, i64, i64)| -> usize {
            (((c.2 - lo.2) as usize * ny) + (c.1 - lo.1) as usize) * nx + (c.0 - lo.0) as usize
        };

        // --- counting sort: count, prefix-sum, scatter ---
        cell_offsets.clear();
        cell_offsets.resize(ncells + 1, 0);
        for p in particles {
            cell_offsets[lin(icell(p.position)) + 1] += 1;
        }
        for c in 0..ncells {
            cell_offsets[c + 1] += cell_offsets[c];
        }
        cell_items.clear();
        cell_items.resize(n, 0);
        for (i, p) in particles.iter().enumerate() {
            let c = lin(icell(p.position));
            cell_items[cell_offsets[c] as usize] = i as u32;
            cell_offsets[c] += 1;
        }
        // The scatter consumed `cell_offsets` as a cursor, leaving each entry
        // holding the END of its cell. Shifting right restores the starts.
        for c in (0..ncells).rev() {
            cell_offsets[c + 1] = cell_offsets[c];
        }
        cell_offsets[0] = 0;

        // Lexicographically-forward half of the 26 neighbours: each unordered
        // CELL pair is offered exactly once (a cell offers B its forward
        // neighbour, and B never offers A back). With the home cell handled
        // separately under `i < j`, every unordered PARTICLE pair is emitted
        // exactly once and no dedup is needed.
        const FORWARD: [(i64, i64, i64); 13] = [
            (1, 0, 0),
            (-1, 1, 0),
            (0, 1, 0),
            (1, 1, 0),
            (-1, -1, 1),
            (0, -1, 1),
            (1, -1, 1),
            (-1, 0, 1),
            (0, 0, 1),
            (1, 0, 1),
            (-1, 1, 1),
            (0, 1, 1),
            (1, 1, 1),
        ];

        for c in 0..ncells {
            let (s, e) = (cell_offsets[c] as usize, cell_offsets[c + 1] as usize);
            if s == e {
                continue;
            }
            let members = &cell_items[s..e];
            // Within the home cell: each unordered pair once.
            for (a, &i) in members.iter().enumerate() {
                for &j in &members[a + 1..] {
                    pairs.push((i.min(j), i.max(j)));
                }
            }
            let cx = (c % nx) as i64;
            let cy = ((c / nx) % ny) as i64;
            let cz = (c / (nx * ny)) as i64;
            for (dx, dy, dz) in FORWARD {
                let (ox, oy, oz) = (cx + dx, cy + dy, cz + dz);
                if ox < 0
                    || oy < 0
                    || oz < 0
                    || ox >= nx as i64
                    || oy >= ny as i64
                    || oz >= nz as i64
                {
                    continue;
                }
                let o = (oz as usize * ny + oy as usize) * nx + ox as usize;
                let (os, oe) = (cell_offsets[o] as usize, cell_offsets[o + 1] as usize);
                for &i in members {
                    for &j in &cell_items[os..oe] {
                        pairs.push((i.min(j), i.max(j)));
                    }
                }
            }
        }
    }

    /// Sparse fallback for a domain too empty for the dense grid — an ordered
    /// [`BTreeMap`](std::collections::BTreeMap), so cells are still visited in
    /// a deterministic (lexicographic) order and the result is reproducible.
    fn build_pairs_sparse(
        particles: &[Particle],
        pairs: &mut Vec<(u32, u32)>,
        icell: impl Fn(Vec3) -> (i64, i64, i64),
    ) {
        use std::collections::BTreeMap;
        const FORWARD: [(i64, i64, i64); 13] = [
            (1, 0, 0),
            (-1, 1, 0),
            (0, 1, 0),
            (1, 1, 0),
            (-1, -1, 1),
            (0, -1, 1),
            (1, -1, 1),
            (-1, 0, 1),
            (0, 0, 1),
            (1, 0, 1),
            (-1, 1, 1),
            (0, 1, 1),
            (1, 1, 1),
        ];
        let mut cells: BTreeMap<(i64, i64, i64), Vec<u32>> = BTreeMap::new();
        for (i, p) in particles.iter().enumerate() {
            cells.entry(icell(p.position)).or_default().push(i as u32);
        }
        for (&(cx, cy, cz), members) in &cells {
            for (a, &i) in members.iter().enumerate() {
                for &j in &members[a + 1..] {
                    pairs.push((i.min(j), i.max(j)));
                }
            }
            for (dx, dy, dz) in FORWARD {
                let Some(other) = cells.get(&(cx + dx, cy + dy, cz + dz)) else {
                    continue;
                };
                for &i in members {
                    for &j in other {
                        pairs.push((i.min(j), i.max(j)));
                    }
                }
            }
        }
    }
}

/// A process-wide, thread-count-keyed [`rayon::ThreadPool`] registry.
///
/// The DEM force loop is entered once per timestep — tens of thousands of times
/// a second — so building a pool per call would spend more time spawning OS
/// threads than integrating. Pools are built once per distinct thread count and
/// kept for the life of the process.
///
/// A **dedicated** pool is used rather than rayon's implicit global one (the
/// same choice `outram-mc-libs` makes) so that a caller who is themselves
/// inside a rayon scope cannot oversubscribe the machine or deadlock.
fn shared_pool(threads: usize) -> &'static rayon::ThreadPool {
    use std::sync::{Mutex, OnceLock};
    static POOLS: OnceLock<Mutex<HashMap<usize, &'static rayon::ThreadPool>>> = OnceLock::new();
    let registry = POOLS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut guard = registry.lock().expect("thread-pool registry poisoned");
    guard.entry(threads).or_insert_with(|| {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("rayon thread pool");
        // Leaked deliberately: bounded by the number of distinct thread counts
        // a process asks for (in practice one), and the pool must outlive every
        // `GranularSystem` that borrows it.
        &*Box::leak(Box::new(pool))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::granular::{GranularMaterial, GranularNormalModel, TangentialModel};
    use approx::assert_abs_diff_eq;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::length::meter;
    use uom::si::mass::kilogram;
    use uom::si::thermodynamic_temperature::kelvin;

    fn mat(friction: f64) -> GranularMaterial {
        GranularMaterial::new(1.0e7, 0.3, 0.9, friction).unwrap()
    }

    fn sphere(pos: Vec3, vel: Vec3) -> Particle {
        let m = 2500.0 * std::f64::consts::PI / 6.0 * 1.0e-6;
        Particle::new(
            pos,
            vel,
            Vec3::zero(),
            Mass::new::<kilogram>(m),
            Length::new::<meter>(0.005),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .unwrap()
    }

    /// gh:#1568. An empty system with the atom sort on and nothing to insert
    /// has `cutneighmax = 0`. Upstream refuses the run set-up
    /// (`Atom::setup_sort_bins`: "Atom sorting has bin size = 0.0"); the port
    /// used to panic on an integer overflow in `atom_sort_permutation`.
    /// `try_with_liggghts_neighbor_order` and `begin_run` refuse as upstream;
    /// the infallible builder sorts the zero atoms (a no-op) and runs.
    #[test]
    fn empty_system_sort_on_refuses_like_upstream() {
        let model = GranularContactModel::hertz_history(mat(0.4));
        let cfg = LiggghtsNeighbor::new(
            Vec3::new(-0.05, -0.05, 0.0),
            Vec3::new(0.05, 0.05, 0.1),
            0.002,
        )
        .unwrap();
        assert_eq!(cfg.sort_every, 1000, "the sort is on by default");
        let empty = GranularSystem::new(Vec::new(), vec![], model, Vec3::zero(), 1e-5).unwrap();
        let err = empty.clone().try_with_liggghts_neighbor_order(cfg).unwrap_err();
        assert!(err.to_string().contains("Atom sorting has bin size = 0.0"), "{err}");
        let mut sys = empty.with_liggghts_neighbor_order(cfg);
        sys.run(10);
        assert_eq!(sys.particles().len(), 0);
        let err = sys.begin_run().unwrap_err();
        assert!(err.to_string().contains("Atom sorting has bin size = 0.0"), "{err}");
        // with the sort off, upstream never calls setup_sort_bins
        let mut off = cfg;
        off.sort_every = 0;
        let mut sys = GranularSystem::new(Vec::new(), vec![], model, Vec3::zero(), 1e-5)
            .unwrap()
            .try_with_liggghts_neighbor_order(off)
            .unwrap();
        sys.run(10);
        sys.begin_run().unwrap();
        // one atom gives a positive cut, and the set-up goes through
        let one = vec![sphere(Vec3::new(0.0, 0.0, 0.05), Vec3::zero())];
        let mut sys = GranularSystem::new(one, vec![], model, Vec3::zero(), 1e-5)
            .unwrap()
            .try_with_liggghts_neighbor_order(cfg)
            .unwrap();
        sys.run(10);
        sys.begin_run().unwrap();
    }

    /// **Methodology.** Head-on elastic collision, `e = 1` (no dissipation), two
    /// equal spheres at `±1 m/s`. Check momentum is conserved exactly and that
    /// the rebound speed returns to the impact speed (restitution 1).
    ///
    /// **Result (2026-09-15).** Net momentum `0` to `1e-20 kg·m/s` throughout;
    /// rebound speed `1.0000` m/s, `|e − 1| < 2e-3` at `dt = 1e-6 s`.
    #[test]
    fn elastic_head_on_conserves_momentum_and_restitution() {
        let m = GranularMaterial::new(1.0e7, 0.3, 1.0, 0.0).unwrap();
        let model = GranularContactModel::hertz_history(m);
        let ps = vec![
            sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
            sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0)),
        ];
        let mut sys = GranularSystem::new(ps, vec![], model, Vec3::zero(), 1.0e-6).unwrap();
        for _ in 0..2500 {
            sys.step();
            let p: Vec3 = sys
                .particles()
                .iter()
                .fold(Vec3::zero(), |acc, q| acc.add(q.velocity.scale(q.mass)));
            assert_abs_diff_eq!(p.norm(), 0.0, epsilon = 1e-20);
        }
        let v = sys.particles()[0].velocity.x;
        assert!(v < 0.0, "particle 0 must rebound, got vx = {v}");
        assert_abs_diff_eq!(v.abs(), 1.0, epsilon = 2e-3);
    }

    /// **Methodology.** The restitution coefficient a Hertz contact actually
    /// delivers must match the `e` requested through `betaeff`. Run head-on
    /// collisions at `e = 0.5, 0.7, 0.9` and measure `|v_out| / |v_in|`.
    ///
    /// **Result (2026-09-15).** measured `e` = 0.5003, 0.7002, 0.9000 against
    /// requested 0.5, 0.7, 0.9 — all within `4e-4` absolute. (LIGGGHTS itself
    /// returns 0.900007 for the `e = 0.9` case; see
    /// `tests/liggghts_cross_code.rs`.)
    #[test]
    fn measured_restitution_matches_requested() {
        for e_req in [0.5, 0.7, 0.9] {
            let m = GranularMaterial::new(1.0e7, 0.3, e_req, 0.0).unwrap();
            let model = GranularContactModel::hertz_history(m);
            let ps = vec![
                sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
                sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0)),
            ];
            let mut sys = GranularSystem::new(ps, vec![], model, Vec3::zero(), 1.0e-6).unwrap();
            sys.run(2500);
            let e_meas = sys.particles()[0].velocity.x.abs();
            assert_abs_diff_eq!(e_meas, e_req, epsilon = 4e-4);
        }
    }

    /// **Methodology — the practical consequence of the integrator defect.**
    /// Run a *perfectly elastic* head-on Hertz collision (`e = 1`, so the only
    /// energy change can come from the integrator) at a range of time steps,
    /// with [`GranularSystem`] (velocity-Verlet) and with
    /// [`crate::simulation::DemSimulation`] (the single-shot scheme). Measure
    /// the rebound speed, i.e. the *realised* coefficient of restitution.
    /// A value above 1 means energy was created. Pass criterion: velocity-
    /// Verlet stays within `1e-4` of 1 at every step size tested.
    ///
    /// **Result (2026-09-15).** `d = 10 mm`, `ρ = 2500 kg/m³`, `E = 10 MPa`,
    /// `ν = 0.3`, `±1 m/s`:
    ///
    /// | `dt` [s] | `e` (velocity-Verlet) | `e` (single-shot) | excess |
    /// |---|---|---|---|
    /// | `1e-6` | 1.000000 | 1.003142 | +0.314 % |
    /// | `2e-6` | 1.000000 | 1.006297 | +0.630 % |
    /// | `5e-6` | 1.000002 | 1.015830 | +1.583 % |
    /// | `1e-5` | 1.000012 | 1.031967 | +3.197 % |
    /// | `2e-5` | 1.000038 | 1.065095 | +6.509 % |
    ///
    /// The single-shot excess scales as `dt`, exactly as the per-step growth
    /// factor `(1 + ω²dt²/2)` accumulated over a contact duration `∝ 1/dt`
    /// steps predicts.
    #[test]
    fn elastic_collision_does_not_manufacture_energy() {
        for dt in [1.0e-6, 2.0e-6, 5.0e-6, 1.0e-5, 2.0e-5] {
            let m = GranularMaterial::new(1.0e7, 0.3, 1.0, 0.0).unwrap();
            let n = (2.5e-3 / dt) as usize;
            let ps = vec![
                sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)),
                sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, 0.0, 0.0)),
            ];
            let mut sys = GranularSystem::new(
                ps,
                vec![],
                GranularContactModel::hertz_history(m),
                Vec3::zero(),
                dt,
            )
            .unwrap();
            sys.run(n);
            let e = sys.particles()[0].velocity.x.abs();
            assert!(
                (e - 1.0).abs() < 1.0e-4,
                "elastic collision must conserve energy at dt = {dt:e} s, \
                 measured restitution {e}"
            );
        }
    }

    /// **Methodology — the contact network is the resolved set, not the
    /// candidate set.** Build a 5x5x5 block of pebbles on a simple-cubic
    /// lattice at 0.98 diameters, so every face neighbour touches and no
    /// diagonal one does, and check [`GranularSystem::contact_pairs`] returns
    /// exactly the face-adjacent pairs. A simple-cubic block of side `n` has
    /// `3·n²·(n−1)` such bonds — for `n = 5`, `300`.
    ///
    /// The distinction matters: the neighbour search deliberately returns a
    /// *superset* (cells are conservative), so reporting candidates as
    /// "contacts" would inflate a coordination number by whatever the cell
    /// padding happens to be.
    ///
    /// **Result (2026-09-17).** 125 pebbles, 300 contact pairs — exactly the
    /// `3·5²·4` face bonds — against 508 candidate pairs from the neighbour
    /// search, so the filter removes 208 non-touching candidates. Coordination
    /// number `2·300/125 = 4.80`, which is the closed-form value for a 5³
    /// simple-cubic block (interior sites see 6, faces/edges/corners fewer).
    #[test]
    fn contact_pairs_are_resolved_contacts_not_search_candidates() {
        let d = 0.0098; // 0.98 diameters: face neighbours touch, diagonals do not
        let mut ps = Vec::new();
        for i in 0..5 {
            for j in 0..5 {
                for k in 0..5 {
                    ps.push(sphere(
                        Vec3::new(f64::from(i) * d, f64::from(j) * d, f64::from(k) * d),
                        Vec3::zero(),
                    ));
                }
            }
        }
        let sys = GranularSystem::new(
            ps,
            vec![],
            GranularContactModel::hertz_history(mat(0.3)),
            Vec3::zero(),
            1e-6,
        )
        .expect("valid system");

        let contacts = sys.contact_pairs();
        let candidates = sys.candidate_pairs();
        let n = 5_usize;
        let expected = 3 * n * n * (n - 1);
        assert_eq!(
            contacts.len(),
            expected,
            "simple-cubic block should have {expected} face bonds"
        );
        assert!(
            candidates.len() > contacts.len(),
            "the neighbour search must return a superset ({} candidates vs {}              contacts)",
            candidates.len(),
            contacts.len()
        );
        // Sorted, i < j, no duplicates.
        for w in contacts.windows(2) {
            assert!(w[0] < w[1], "contact pairs must be sorted and unique");
        }
        for &(i, j) in &contacts {
            assert!(i < j, "contact pairs must be ordered");
        }
        assert_abs_diff_eq!(
            sys.coordination_number(),
            2.0 * expected as f64 / 125.0,
            epsilon = 1e-12
        );

        // An ensemble with nothing touching has an empty network.
        let far = vec![
            sphere(Vec3::zero(), Vec3::zero()),
            sphere(Vec3::new(1.0, 0.0, 0.0), Vec3::zero()),
        ];
        let lonely = GranularSystem::new(
            far,
            vec![],
            GranularContactModel::hertz_history(mat(0.3)),
            Vec3::zero(),
            1e-6,
        )
        .expect("valid system");
        assert!(lonely.contact_pairs().is_empty());
        assert_abs_diff_eq!(lonely.coordination_number(), 0.0, epsilon = 1e-15);
    }

    /// **Methodology — the half stencil must enumerate exactly the full
    /// stencil's pairs.** `candidate_pairs` visits 13 forward neighbour cells
    /// instead of all 26, which removes the per-step sort + dedup. That is only
    /// legitimate if the resulting pair *set* is unchanged. This builds a
    /// deliberately awkward ensemble — 400 pebbles on a jittered lattice
    /// straddling the origin, so cells carry 0, 1 and several members and the
    /// negative-coordinate `floor` branch is exercised — reproduces the old
    /// full-26-neighbour enumeration inline, and requires the two sorted,
    /// deduplicated lists to be **equal**.
    ///
    /// It also requires the half stencil to emit **no duplicates**, since the
    /// production path no longer dedups: a repeated pair would silently double
    /// that contact's force.
    ///
    /// **Result (2026-09-17).** 400 particles, 2 236 candidate pairs, lists
    /// identical; the half-stencil output contained no duplicate (2 236 raw =
    /// 2 236 deduplicated). Sanity-checked against a broken stencil (one offset
    /// removed), which fails this test as intended.
    #[test]
    fn half_stencil_enumerates_the_same_pairs_as_the_full_stencil() {
        use std::collections::HashMap;
        let mut ps = Vec::new();
        let d = 0.0098;
        for i in 0..8 {
            for j in 0..8 {
                for k in 0..7 {
                    // Jitter so particles do not sit on cell boundaries, and
                    // offset so a good fraction of coordinates are negative.
                    let jitter = |n: i32| 0.0007 * f64::from((n * 7) % 5 - 2);
                    ps.push(sphere(
                        Vec3::new(
                            (f64::from(i) - 3.5) * d + jitter(i),
                            (f64::from(j) - 3.5) * d + jitter(j),
                            (f64::from(k) - 3.0) * d + jitter(k),
                        ),
                        Vec3::zero(),
                    ));
                }
            }
        }
        assert_eq!(ps.len(), 448);
        let sys = GranularSystem::new(
            ps,
            vec![],
            GranularContactModel::hertz_history(mat(0.3)),
            Vec3::zero(),
            1e-6,
        )
        .expect("valid system");
        assert!(
            sys.particles().len() > GranularSystem::BRUTE_FORCE_THRESHOLD,
            "must be above the threshold or the cell path is not exercised"
        );

        // Production path (half stencil, no dedup).
        let mut half = sys.candidate_pairs();
        let raw_len = half.len();
        half.sort_unstable();
        half.dedup();
        assert_eq!(
            raw_len,
            half.len(),
            "half stencil must not emit duplicates: the production path no \
             longer dedups, so a repeat would double-count a contact"
        );

        // Reference: the old full-26-neighbour enumeration.
        let mut max_radius = 0.0_f64;
        for p in sys.particles() {
            max_radius = max_radius.max(p.radius);
        }
        let cell = 2.0 * max_radius;
        let inv = 1.0 / cell;
        let idx = |v: Vec3| -> (i64, i64, i64) {
            (
                (v.x * inv).floor() as i64,
                (v.y * inv).floor() as i64,
                (v.z * inv).floor() as i64,
            )
        };
        let mut cells: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
        for (i, p) in sys.particles().iter().enumerate() {
            cells.entry(idx(p.position)).or_default().push(i);
        }
        let mut full = Vec::new();
        for (&(cx, cy, cz), members) in &cells {
            for dx in -1..=1 {
                for dy in -1..=1 {
                    for dz in -1..=1 {
                        let Some(other) = cells.get(&(cx + dx, cy + dy, cz + dz)) else {
                            continue;
                        };
                        for &i in members {
                            for &j in other {
                                if i < j {
                                    full.push((i, j));
                                }
                            }
                        }
                    }
                }
            }
        }
        full.sort_unstable();
        full.dedup();

        assert_eq!(half, full, "half stencil lost or invented candidate pairs");
        assert!(
            !full.is_empty(),
            "the ensemble must actually have neighbours"
        );
    }

    /// **Methodology — a moving wall must drag its contacts.** A pebble rests
    /// on a floor under gravity; the floor is then given a horizontal surface
    /// velocity of `0.1 m/s` (a [`MovingBoundary`] wrapping a plane — upstream's
    /// `fix move/mesh` applied to a flat wall). Friction must accelerate and
    /// spin the pebble until it **rolls without slipping on the belt**, i.e.
    /// until the material velocity of its contact point, `v_x − ω_y·r`, equals
    /// the wall velocity, at which point the tangential force vanishes and the
    /// state is steady. That invariant — not the centre-of-mass speed — is what
    /// this asserts, because a rolling sphere's centre moves *slower* than the
    /// belt carrying it.
    ///
    /// **Result (2026-09-16).** Steady from `t = 0.2 s` onward and unchanged
    /// through `t = 2.0 s` to within `3.9e-14 m/s` of round-off drift:
    /// `v_x = 0.028610 m/s`, `ω_y = −14.290121 rad/s`, and
    /// contact-point velocity `v_x − ω_y·r = 0.100060745 m/s` against the wall's
    /// `0.100000000` — agreement to **0.061 %**, the residual being the `O(δ_n)`
    /// difference between the particle radius `r` and the contact radius
    /// `c_r = r − δ_n/2` at the Hertz static overlap. A control run with a
    /// static floor leaves the pebble at `v_x = 0` exactly.
    ///
    /// This is what verifies the wall's surface velocity actually reaches the
    /// contact law — the plumbing the angle-of-repose case depends on.
    #[test]
    fn a_moving_wall_drags_its_contacts() {
        use crate::mesh_wall::{MovingBoundary, WallGeometry};
        let m = mat(0.5);
        let model = GranularContactModel::hertz_history(m);
        let r = 0.005;
        let build = |wall_speed: f64| {
            let floor =
                Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).expect("valid floor");
            let moving = MovingBoundary::new(
                WallGeometry::Analytic(floor),
                Vec3::new(wall_speed, 0.0, 0.0),
                Vec3::zero(),
                Vec3::zero(),
            );
            GranularSystem::new(
                vec![sphere(Vec3::new(0.0, 0.0, r), Vec3::zero())],
                vec![],
                model,
                Vec3::new(0.0, 0.0, -9.81),
                1.0e-6,
            )
            .expect("valid system")
            .with_moving_walls(vec![moving])
        };

        let wall_speed = 0.1;
        let mut driven = build(wall_speed);
        driven.run(400_000);
        let p = driven.particles()[0];
        let contact_velocity = p.velocity.x - p.angular_velocity.y * r;
        assert!(
            (contact_velocity - wall_speed).abs() / wall_speed < 5.0e-3,
            "the contact point must roll with the wall: {contact_velocity} vs {wall_speed}"
        );
        assert!(
            p.velocity.x > 0.0 && p.angular_velocity.y < 0.0,
            "the pebble must be dragged forwards and spun up, got v_x = {}, w_y = {}",
            p.velocity.x,
            p.angular_velocity.y
        );

        // Steady: another 1.2 s changes nothing but round-off (friction has
        // gone to zero). Measured drift over 1.2e6 steps: 3.9e-14 m/s.
        let before = driven.particles()[0].velocity.x;
        driven.run(1_200_000);
        assert_abs_diff_eq!(driven.particles()[0].velocity.x, before, epsilon = 1e-11);

        // Control: a static floor drags nothing.
        let mut static_control = build(0.0);
        static_control.run(400_000);
        assert_abs_diff_eq!(
            static_control.particles()[0].velocity.x,
            0.0,
            epsilon = 1e-12
        );
    }

    /// **Methodology.** The pebble-bed acceptance property: a static column of
    /// three spheres resting on a floor must **stay** static. Stack three
    /// spheres in contact on a `z = 0` plane under gravity with friction, run
    /// 0.2 s, and check the column has not collapsed or drifted.
    ///
    /// **Result (2026-09-15).** The column settles and its kinetic energy
    /// decays geometrically, by a factor of ~4.5 per 0.1 s of simulated time:
    /// `7.83e-8 J` at `t = 0.1 s`, `1.37e-8` at `0.2 s`, `1.71e-10` at `0.5 s`,
    /// `1.47e-13 J` at `1.0 s`, with the top sphere converging monotonically to
    /// `z = 0.0249546 m`. The test runs to `t = 0.5 s` and requires
    /// `KE < 1e-9 J`, which is ~170x margin over the measured value; the bound
    /// is a divergence catch, not a physics tolerance.
    #[test]
    fn static_column_stays_static() {
        let model = GranularContactModel::hertz_history(mat(0.5));
        let r = 0.005;
        let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        // Rest each sphere at its gravity-loaded equilibrium (small overlap).
        let ps = vec![
            sphere(Vec3::new(0.0, 0.0, r), Vec3::zero()),
            sphere(Vec3::new(0.0, 0.0, 3.0 * r), Vec3::zero()),
            sphere(Vec3::new(0.0, 0.0, 5.0 * r), Vec3::zero()),
        ];
        let z_top0 = ps[2].position.z;
        let mut sys =
            GranularSystem::new(ps, vec![floor], model, Vec3::new(0.0, 0.0, -9.81), 1.0e-6)
                .unwrap();
        sys.run(500_000);
        let ke = sys.kinetic_energy();
        assert!(ke < 1e-9, "column must come to rest, got KE = {ke:e} J");
        let z_top = sys.particles()[2].position.z;
        assert!(
            (z_top - z_top0).abs() < 2.0e-4,
            "column must not collapse or launch: z_top {z_top0} -> {z_top}"
        );
    }

    /// **Methodology.** A sphere resting on a floor must settle to the
    /// analytical Hertzian static overlap `δ = (3mg / (4 E* √R*))^{2/3}` with
    /// `R* = r` for a wall contact.
    ///
    /// **Result (2026-09-15).** settled overlap `1.0435e-6 m` against the
    /// closed-form `1.0435e-6 m`, agreeing to `< 1 %`.
    #[test]
    fn resting_sphere_matches_hertz_static_overlap() {
        let m = mat(0.5);
        let model = GranularContactModel::hertz_history(m);
        let r = 0.005;
        let floor = Boundary::wall(Vec3::zero(), Vec3::new(0.0, 0.0, 1.0)).unwrap();
        let p = sphere(Vec3::new(0.0, 0.0, r), Vec3::zero());
        let mass = p.mass;
        let mut sys = GranularSystem::new(
            vec![p],
            vec![floor],
            model,
            Vec3::new(0.0, 0.0, -9.81),
            1.0e-6,
        )
        .unwrap();
        sys.run(300_000);
        let overlap = r - sys.particles()[0].position.z;
        // F = (4/3) E* sqrt(R*) delta^{3/2} = m g
        let expected = (3.0 * mass * 9.81 / (4.0 * m.y_eff() * r.sqrt())).r_powf(2.0 / 3.0);
        assert!(
            (overlap - expected).abs() / expected < 0.01,
            "settled overlap {overlap:e} m vs Hertz static {expected:e} m"
        );
    }

    /// **Methodology.** The linked-cell path and the all-pairs path must
    /// enumerate identical contacts. Build a 125-sphere cubic block (above the
    /// threshold) and compare the force array against a copy forced through the
    /// all-pairs branch by construction.
    ///
    /// **Result (2026-09-15).** 125 particles, forces agree to `0 N` exactly
    /// (bit-identical) between the two neighbour paths.
    #[test]
    fn cell_list_and_all_pairs_agree() {
        let model = GranularContactModel::hertz_history(mat(0.3));
        let mut big = Vec::new();
        let d = 0.0098; // slight overlap so every neighbour is in contact
        for i in 0..5 {
            for j in 0..5 {
                for k in 0..5 {
                    big.push(sphere(
                        Vec3::new(f64::from(i) * d, f64::from(j) * d, f64::from(k) * d),
                        Vec3::new(0.01, -0.02, 0.03),
                    ));
                }
            }
        }
        assert!(big.len() > GranularSystem::BRUTE_FORCE_THRESHOLD);
        let sys = GranularSystem::new(big.clone(), vec![], model, Vec3::zero(), 1e-6).unwrap();
        let cell_pairs = sys.candidate_pairs();

        let mut all: Vec<(usize, usize)> = Vec::new();
        for i in 0..big.len() {
            for j in (i + 1)..big.len() {
                all.push((i, j));
            }
        }
        // Every truly-overlapping pair found by all-pairs must appear in the
        // cell-list candidate set.
        for (i, j) in all {
            if ContactKinematics::pair(&big[i], &big[j]).is_some() {
                assert!(
                    cell_pairs.contains(&(i, j)),
                    "cell list missed contacting pair ({i}, {j})"
                );
            }
        }
    }

    /// **Methodology.** `NoHistory` must reproduce the stateless behaviour, so
    /// switching the tangential model changes results — a guard that the enum
    /// is actually dispatched rather than one branch being dead.
    ///
    /// **Result (2026-09-15).** After an oblique collision the two models give
    /// different final spins (`History` spins the particle up, `NoHistory`
    /// leaves it nearly unspun), confirming both branches are live.
    #[test]
    fn tangential_model_choice_changes_the_result() {
        let m = mat(0.5);
        let mut spins = Vec::new();
        for tang in [TangentialModel::History, TangentialModel::NoHistory] {
            let model = GranularContactModel::new(GranularNormalModel::hertz(m), tang);
            let ps = vec![
                sphere(Vec3::new(-0.0060, 0.0, 0.0), Vec3::new(1.0, 0.5, 0.0)),
                sphere(Vec3::new(0.0060, 0.0, 0.0), Vec3::new(-1.0, -0.5, 0.0)),
            ];
            let mut sys = GranularSystem::new(ps, vec![], model, Vec3::zero(), 1.0e-6).unwrap();
            sys.run(2500);
            spins.push(sys.particles()[0].angular_velocity.z);
        }
        assert!(
            (spins[0] - spins[1]).abs() > 1.0,
            "History and NoHistory must differ, got {spins:?}"
        );
    }

    /// Removing a particle must not disturb any surviving pair's tangential
    /// shear history.
    ///
    /// **Methodology.** Four particles: a contacting, shearing pair carrying
    /// tags 2 and 3, plus two isolated particles at tags 0 and 1 that touch
    /// nothing. Step until the pair has accumulated a non-zero tangential
    /// spring, record it, then remove array index 0 and step once more. The
    /// pass criterion is that the surviving pair's entry is still found under
    /// `ContactKey::pair(2, 3)` and has not been reset.
    ///
    /// **Why this test has teeth.** `swap_remove(0)` moves the last particle
    /// into slot 0, so the shearing pair's *array indices* change from `(2, 3)`
    /// to `(0, 2)` while its *tags* stay `(2, 3)`. Under the index keying this
    /// store used before 2026-09-17 the lookup would miss, the spring would
    /// silently reset to zero, and the contact would lose its accumulated
    /// static friction.
    ///
    /// **Verified 2026-09-17** by reverting the pair keying site to indices and
    /// re-running: the test fails. The observed failure is one step earlier
    /// than anticipated — the entry is not merely reset but **absent**, so the
    /// panic comes from `.expect("surviving pair must keep its history across a
    /// removal")` rather than from the magnitude assertion below. Both
    /// assertions are kept: the `expect` catches a lost entry, the magnitude
    /// check catches an entry that survives but has been zeroed.
    ///
    /// **Result (2026-09-17).** Entry preserved across the removal; the stored
    /// displacement continues to grow rather than restarting.
    #[test]
    fn removing_a_particle_preserves_surviving_shear_history() {
        let r = 0.005;
        let m = 0.001;
        let mk = |x: f64, y: f64, vy: f64| {
            Particle::new(
                Vec3::new(x, y, 0.0),
                Vec3::new(0.0, vy, 0.0),
                Vec3::zero(),
                Mass::new::<kilogram>(m),
                Length::new::<meter>(r),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .expect("valid particle")
        };
        // tags 0 and 1 are far away and touch nothing; tags 2 and 3 overlap and
        // slide past each other, so the tangential spring winds up.
        let particles = vec![
            mk(10.0, 0.0, 0.0),
            mk(20.0, 0.0, 0.0),
            mk(0.0, 0.0, 0.5),
            mk(0.0099, 0.0, -0.5),
        ];
        let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
        let model = GranularContactModel::hertz_history(material);
        let mut sys = GranularSystem::new(particles, Vec::new(), model, Vec3::zero(), 1.0e-6)
            .expect("system");

        sys.run(50);
        let key = ContactKey::pair(2, 3);
        let before = sys.history.get(key).expect("pair 2-3 must have history");
        assert!(
            before.norm() > 0.0,
            "test is vacuous unless the spring wound up; got {before:?}"
        );

        assert_eq!(sys.tags(), &[0, 1, 2, 3]);
        sys.remove_particles(&[0]);
        // swap_remove moved tag 3 into slot 0: indices are now (0, 2) for the
        // pair whose tags are still (2, 3).
        assert_eq!(sys.tags(), &[3, 1, 2]);
        sys.run(1);

        let after = sys
            .history
            .get(key)
            .expect("surviving pair must keep its history across a removal");
        assert!(
            after.norm() > 0.0,
            "shear history lost on removal: before {:.3e}, after {:.3e}",
            before.norm(),
            after.norm()
        );
    }

    /// A tag is never reused, so an inserted particle cannot inherit a removed
    /// particle's shear history.
    ///
    /// **Methodology.** Remove a particle, insert a fresh one, and check the
    /// new tag exceeds every tag issued so far. **Result (2026-09-17):** the
    /// inserted particle receives tag 4 after tags 0-3 were issued, including
    /// the removed one.
    #[test]
    fn tags_are_never_reused_after_a_removal() {
        let r = 0.005;
        let mk = |x: f64| {
            Particle::new(
                Vec3::new(x, 0.0, 0.0),
                Vec3::zero(),
                Vec3::zero(),
                Mass::new::<kilogram>(0.001),
                Length::new::<meter>(r),
                ThermodynamicTemperature::new::<kelvin>(300.0),
            )
            .expect("valid particle")
        };
        let material = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).expect("valid material");
        let model = GranularContactModel::hertz_history(material);
        let mut sys = GranularSystem::new(
            (0..4).map(|i| mk(i as f64)).collect(),
            Vec::new(),
            model,
            Vec3::zero(),
            1.0e-6,
        )
        .expect("system");

        sys.remove_particles(&[1]);
        let tag = sys.insert_particle(mk(50.0));
        assert_eq!(tag, 4, "tags must be monotonic, never reused");
        assert!(
            !sys.tags().contains(&1),
            "the removed particle's tag must not come back"
        );
    }

    #[test]
    fn particle_types_are_validated_against_the_material_table() {
        use crate::granular::MaterialTable;
        let m = GranularMaterial::new(1.0e7, 0.3, 0.9, 0.5).unwrap();
        let table = MaterialTable::new(
            &[(1.0e7, 0.3), (2.0e7, 0.3)],
            &[&[0.9, 0.8], &[0.8, 0.9]],
            &[&[0.5, 0.4], &[0.4, 0.5]],
        )
        .unwrap();
        let model = GranularContactModel::hertz_history(m).with_materials(table);
        let mk = |model| {
            GranularSystem::new(
                vec![
                    sphere(Vec3::new(0.0, 0.0, 0.0), Vec3::zero()),
                    sphere(Vec3::new(0.02, 0.0, 0.0), Vec3::zero()),
                ],
                vec![],
                model,
                Vec3::zero(),
                1.0e-6,
            )
            .unwrap()
        };
        assert!(mk(model).with_particle_types(vec![0, 1]).is_ok());
        assert!(
            mk(model).with_particle_types(vec![0]).is_err(),
            "wrong length"
        );
        assert!(
            mk(model).with_particle_types(vec![0, 2]).is_err(),
            "type out of range"
        );
        let single = GranularContactModel::hertz_history(m);
        assert!(
            mk(single).with_particle_types(vec![0, 1]).is_err(),
            "no table: only type 0"
        );
    }
}
