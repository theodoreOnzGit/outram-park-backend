// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
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

//! `fix massflow/mesh`: count the particles (and their mass) that cross a mesh
//! (gh:#1416). Translated from `fix_massflow_mesh.cpp` of LIGGGHTS-PUBLIC
//! `3d5c00f2` (`FixMassflowMesh::post_integrate`, `compute_vector`,
//! `setRefPoint`).
//!
//! The per-particle counter has upstream's meaning: `-1` never seen near the
//! mesh, `0` last seen on the far side of the normal, `1` last seen on the
//! normal side, `2` counted already (`count once`). A particle is counted
//! when it was on the far side (`0`) and its centre is now on the normal side
//! over a facet's face (`barySign == 7`).
//!
//! **Scope, stated plainly.** Translated: `mesh`, `vec_side`,
//! `count once|multiple`, `point_at_outlet` / `inside_out`, the four rates and
//! totals of `compute_vector` including the `reset_t_count_` / `delta_t_`
//! bookkeeping, and the per-crossing log (`file`/`screen` output as a
//! returned `Vec`), `delete_atoms yes`, `sum_property`, a moving counting mesh
//! (`with_moving_wall`) and `fix massflow/mesh/sieve`. **Not translated:**
//! ~~the multisphere volume weight (this crate has no `fix multisphere`; gh:#1491)~~
//! **CORRECTED 2026-11-13** (gh:#1415): `fix multisphere` is ported, and the
//! volume weight is applied ([`MassFlowMesh::set_volume_weights`], cross-coded
//! by `in.ms_massflow` in `tests/multisphere_cross_code.rs`);
//! `sieveMultiSphereCanPass` is still not translated. Also not translated:
//! restart, and a *deforming* counting mesh (no deformation exists upstream).
//! `point_at_outlet` / `inside_out` are cross-coded since 2026-11-12
//! (`tests/massflow_options_cross_code.rs`, `in.massflow_outlet`). Like upstream, only the
//! particles in the mesh's neighbour list (`createOtherNeighList`) are
//! visited, and that list is built at neighbour-list rebuilds
//! ([`MassFlowMesh::rebuild_lists`], called by the owning system at set-up
//! and on its rebuild schedule): a particle outside the list keeps a stale
//! counter, which changes what is counted when it later enters the list
//! (found 2026-11-11, when a bounding-sphere shortcut miscounted one
//! crossing in the cross-code deck).

use crate::insert::RanPark;
use crate::mesh_wall::MeshWall;
use crate::particle::{Particle, Vec3};
use crate::tri_mesh::arr;
use crate::DemError;
use std::collections::HashMap;
use std::sync::Arc;

/// `PIOVERFOUR` of `fix_massflow_mesh_sieve.h` (a truncated literal, kept).
const PI_OVER_FOUR: f64 = 0.78539816339;

/// Options of `fix massflow/mesh/sieve` (`sieveSize`, `sieveSpacing`,
/// `sieveStiffness`, `sieveDamping`; `sieveMultiSphereCanPass` is moot without
/// multispheres).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SieveConfig {
    /// `sieveSize` `[m]` (aperture), `0..=1e5`.
    pub size: f64,
    /// `sieveSpacing` `[m]`, `0..=1e5`.
    pub spacing: f64,
    /// `sieveStiffness` `[N/m]`, `0..=1e10`.
    pub stiffness: f64,
    /// `sieveDamping` `[kg/s]`.
    pub damping: f64,
}

/// State of the sieve half of `fix massflow/mesh/sieve`.
#[derive(Debug, Clone, PartialEq)]
struct Sieve {
    cfg: SieveConfig,
    random: RanPark,
    /// Per-particle `massflowSieve_<id>` property (default `-1`).
    apply_force: HashMap<u64, f64>,
}

/// Options of `fix massflow/mesh`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassFlowConfig {
    /// `vec_side`: a vector on the side the flow goes to (fixes the sign of
    /// the face normal). Ignored when `point_at_outlet` is set.
    pub vec_side: Vec3,
    /// `count once` (`true`) or `count multiple` (`false`).
    pub once: bool,
    /// `point_at_outlet x y z` (needs `once`).
    pub point_at_outlet: Option<Vec3>,
    /// `inside_out` (needs `point_at_outlet`).
    pub inside_out: bool,
    /// Neighbour skin `[m]` of the run (`neighbor <skin>`); the mesh
    /// neighbour list uses `0.5 * skin` for a static mesh.
    pub skin: f64,
}

/// One logged crossing (`file` / `screen` line of upstream).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Crossing {
    /// Particle tag.
    pub tag: u64,
    /// `update->dt * ntimestep` at the crossing `[s]`.
    pub time: f64,
    /// Particle diameter `[m]`.
    pub diameter: f64,
    /// Position `[m]`.
    pub position: Vec3,
    /// Velocity `[m/s]`.
    pub velocity: Vec3,
}

/// `fix massflow/mesh` state.
#[derive(Debug, Clone, PartialEq)]
pub struct MassFlowMesh {
    mesh: Arc<MeshWall>,
    cfg: MassFlowConfig,
    /// The moving wall (`GranularSystem` moving-wall index) whose mesh is
    /// counted, if the counted mesh is moved by `fix move/mesh` or a servo.
    wall: Option<usize>,
    /// `delete_atoms yes`.
    delete_atoms: bool,
    /// Tags marked for deletion, in marking order (`atom_tags_delete_`).
    pending_delete: Vec<u64>,
    mass_deleted: f64,
    nparticles_deleted: f64,
    /// `sum_property`: the per-particle scalar summed over counted particles.
    property: Option<HashMap<u64, f64>>,
    property_sum: f64,
    sieve: Option<Sieve>,
    nvec: [f64; 3],
    pref: [f64; 3],
    counter: HashMap<u64, f64>,
    /// `volumeweight_ms` per particle index, present when a `fix multisphere`
    /// exists (gh:#1415, #1491 item 4).
    volume_weight: Option<Vec<f64>>,
    mass: f64,
    nparticles: f64,
    mass_last: f64,
    nparticles_last: f64,
    t_count: f64,
    delta_t: f64,
    reset_t_count: bool,
    steps: u64,
    log: Vec<Crossing>,
    /// Per-facet neighbour list: particle indices, as built at the last
    /// neighbour-list rebuild (`FixNeighlistMesh::pre_force`).
    lists: Vec<Vec<usize>>,
}

fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

impl MassFlowMesh {
    /// `FixMassflowMesh` constructor, `setRefPoint` and the planarity check of
    /// `setup`.
    ///
    /// # Errors
    /// `vec_side` zero or in the mesh plane (`|n . s| < 1e-6`) without
    /// `point_at_outlet`; a non-planar mesh without `point_at_outlet`;
    /// `point_at_outlet` with `count multiple`; `inside_out` without
    /// `point_at_outlet`.
    pub fn new(mesh: MeshWall, cfg: MassFlowConfig) -> Result<Self, DemError> {
        let bad = |m: &str| Err(DemError::InvalidInput(format!("fix massflow/mesh: {m}")));
        let side = arr(cfg.vec_side);
        let have_outlet = cfg.point_at_outlet.is_some();
        if cfg.inside_out && !have_outlet {
            return bad("'inside_out' has no meaning without 'point_at_outlet'");
        }
        if !cfg.once && have_outlet {
            return bad("setting 'point_at_outlet' requires 'count once'");
        }
        if (side[0] * side[0] + side[1] * side[1] + side[2] * side[2]).sqrt() == 0.0 && !have_outlet
        {
            return bad("expecting keyword 'vec_side'");
        }
        let t0 = mesh.triangles[0];
        let pref = arr(t0.a);
        let mut nvec = mesh.state.surface_norm(0);
        let d = dot(&nvec, &side);
        if d.abs() < 1e-6 && !have_outlet {
            return bad("'vec_side' is in or too close to the mesh plane");
        } else if d < 0.0 {
            for c in &mut nvec {
                *c *= -1.0;
            }
        }
        if !have_outlet {
            // isPlanar: every facet coplanar with facet 0
            let n0 = mesh.state.surface_norm(0);
            for (i, t) in mesh.triangles.iter().enumerate() {
                let n = mesh.state.surface_norm(i);
                let off = [t.a.x - pref[0], t.a.y - pref[1], t.a.z - pref[2]];
                if dot(&n, &n0).abs() < 1.0 - 1e-9 || dot(&off, &n0).abs() > 1e-9 {
                    return bad(
                        "requires a planar face mass flow measurement or using 'point_at_outlet'",
                    );
                }
            }
        }
        Ok(Self {
            mesh: Arc::new(mesh),
            cfg,
            wall: None,
            delete_atoms: false,
            pending_delete: Vec::new(),
            mass_deleted: 0.0,
            nparticles_deleted: 0.0,
            property: None,
            property_sum: 0.0,
            sieve: None,
            nvec,
            pref,
            counter: HashMap::new(),
            volume_weight: None,
            mass: 0.0,
            nparticles: 0.0,
            mass_last: 0.0,
            nparticles_last: 0.0,
            t_count: 0.0,
            delta_t: 0.0,
            reset_t_count: true,
            steps: 0,
            log: Vec::new(),
            lists: Vec::new(),
        })
    }

    /// `delete_atoms yes`: a counted particle is marked and removed at the
    /// next neighbour-list rebuild (`pre_exchange`), after which
    /// `compute_vector(4)`/`(5)` hold the deleted mass and count.
    ///
    /// # Errors
    /// Upstream's check: `delete_atoms yes` requires `count once`.
    pub fn with_delete_atoms(mut self) -> Result<Self, DemError> {
        if !self.cfg.once {
            return Err(DemError::InvalidInput(
                "fix massflow/mesh: using 'delete_atoms yes' requires 'count once'".into(),
            ));
        }
        self.delete_atoms = true;
        Ok(self)
    }

    /// `sum_property <id>`: sum a per-particle scalar over the counted
    /// particles (`compute_vector(6)`). The values are set with
    /// [`MassFlowMesh::set_property`]; a particle without one counts `0`.
    #[must_use]
    pub fn with_sum_property(mut self) -> Self {
        self.property = Some(HashMap::new());
        self
    }

    /// Set the summed property of the particle with tag `tag`.
    pub fn set_property(&mut self, tag: u64, value: f64) {
        if let Some(p) = &mut self.property {
            p.insert(tag, value);
        }
    }

    /// The counted mesh is moved: it is the geometry of moving wall `wall` of
    /// the owning [`crate::granular_system::GranularSystem`] (a mesh that
    /// `fix move/mesh` or a servo moves). The reference point is then
    /// refreshed every step (`setRefPoint` when `isMoving()`) and the mesh
    /// neighbour list uses the full skin as threshold.
    #[must_use]
    pub fn with_moving_wall(mut self, wall: usize) -> Self {
        self.wall = Some(wall);
        self
    }

    /// Turn this counter into `fix massflow/mesh/sieve`.
    ///
    /// # Errors
    /// Upstream's range checks on `sieveSize`, `sieveSpacing`,
    /// `sieveStiffness`.
    pub fn with_sieve(mut self, cfg: SieveConfig) -> Result<Self, DemError> {
        let bad = |m: &str| Err(DemError::InvalidInput(format!("fix massflow/mesh/sieve: {m}")));
        if cfg.size < 0.0 || cfg.size > 1e5 {
            return bad("'sieveSize' is not set, larger than 1e5, or less than zero.");
        }
        if cfg.spacing < 0.0 || cfg.spacing > 1e5 {
            return bad("'sieveSpacing' is not set, larger than 1e5, or less than zero.");
        }
        if cfg.stiffness < 0.0 || cfg.stiffness > 1e10 {
            return bad("'sieveStiffness' is not set, larger than 1e10, or less than zero.");
        }
        self.sieve = Some(Sieve {
            cfg,
            // `new RanPark(lmp,"15485863")`
            random: RanPark::new(15_485_863).expect("positive seed"),
            apply_force: HashMap::new(),
        });
        Ok(self)
    }

    /// The moving wall index, if the counted mesh is moved.
    #[must_use]
    pub fn wall(&self) -> Option<usize> {
        self.wall
    }

    /// The counter's own copy of the mesh (used when no moving wall is set).
    #[must_use]
    pub fn mesh_arc(&self) -> Arc<MeshWall> {
        Arc::clone(&self.mesh)
    }

    /// Whether this counter is a `massflow/mesh/sieve`.
    #[must_use]
    pub fn is_sieve(&self) -> bool {
        self.sieve.is_some()
    }

    /// `setRefPoint`: reference point and normal from facet 0 of the current
    /// mesh. Aborts as upstream does when `vec_side` lies in the mesh plane.
    ///
    /// # Panics
    /// `vec_side` in or too close to the mesh plane without `point_at_outlet`.
    fn set_ref_point(&mut self, mesh: &MeshWall) {
        let t0 = mesh.triangles[0];
        self.pref = arr(t0.a);
        self.nvec = mesh.state.surface_norm(0);
        let d = dot(&self.nvec, &arr(self.cfg.vec_side));
        assert!(
            d.abs() >= 1e-6 || self.cfg.point_at_outlet.is_some(),
            "fix massflow/mesh: need to change 'vec_side', it is currently in or to close to the mesh plane"
        );
        if d < 0.0 {
            for c in &mut self.nvec {
                *c *= -1.0;
            }
        }
    }

    /// Take the tags marked for deletion (`atom_tags_delete_`), in marking
    /// order, for the owning system to remove at its next `pre_exchange`.
    pub fn take_pending_deletions(&mut self) -> Vec<u64> {
        std::mem::take(&mut self.pending_delete)
    }

    /// Record one deleted particle of mass `mass` (`pre_exchange`).
    pub fn record_deleted(&mut self, tag: u64, mass: f64) {
        self.mass_deleted += mass;
        self.nparticles_deleted += 1.0;
        self.counter.remove(&tag);
        if let Some(s) = &mut self.sieve {
            s.apply_force.remove(&tag);
        }
    }

    /// `FixMassflowMeshSieve::post_force`: the sieve's repulsive force on
    /// particles overlapping a facet of `mesh`, added to `forces` in
    /// upstream's loop order (facets, then the facet's neighbour list). A
    /// first contact throws a coin (`RanPark::uniform`) against the pass
    /// probability; a particle that passes feels no force from that facet.
    pub fn sieve_post_force(
        &mut self,
        mesh: &MeshWall,
        particles: &[Particle],
        tags: &[u64],
        forces: &mut [Vec3],
    ) {
        let Some(sieve) = &mut self.sieve else {
            return;
        };
        let nodes = mesh.nodes();
        for i_tri in 0..nodes.len() {
            let i_tri_double = i_tri as f64;
            for &ip in &self.lists[i_tri] {
                let p = &particles[ip];
                let tag = tags[ip];
                if *self.counter.get(&tag).unwrap_or(&-1.0) == 2.0 {
                    continue;
                }
                let x = arr(p.position);
                let (deltan, delta, _bary, _sign) = mesh
                    .state
                    .resolve_tri_sphere_contact_bary(&nodes[i_tri], i_tri, p.radius, &x, true);
                let af = sieve.apply_force.entry(tag).or_insert(-1.0);
                if deltan <= 0.0 {
                    if *af < 0.0 {
                        let rand_number = sieve.random.uniform();
                        let d_pass = sieve.cfg.size - 2.0 * p.radius;
                        let prob = if d_pass > 0.0 {
                            d_pass * d_pass * PI_OVER_FOUR / (sieve.cfg.spacing * sieve.cfg.spacing)
                        } else {
                            0.0
                        };
                        if rand_number < prob {
                            *af = 0.0;
                            continue;
                        }
                        *af = 1.0 + i_tri_double;
                    } else if *af == 0.0 {
                        continue;
                    }
                    let inv_radius = 1.0 / p.radius;
                    let n = [
                        delta[0] * inv_radius,
                        delta[1] * inv_radius,
                        delta[2] * inv_radius,
                    ];
                    let v = arr(p.velocity);
                    let normal_vel = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
                    let fn_damping = sieve.cfg.damping * normal_vel;
                    let fn_contact = -sieve.cfg.stiffness * deltan;
                    let mut fnn = fn_damping + fn_contact;
                    if fnn < 0.0 {
                        fnn = 0.0;
                    }
                    let f = &mut forces[ip];
                    f.x -= fnn * n[0];
                    f.y -= fnn * n[1];
                    f.z -= fnn * n[2];
                } else if *af == 1.0 + i_tri_double {
                    *af = -1.0;
                }
            }
        }
    }

    /// Set the per-particle multisphere volume weights (`volumeweight_ms`,
    /// indexed like the particles; 1 for a free sphere), as the owning
    /// [`GranularSystem`](crate::granular_system::GranularSystem) does each
    /// step once it holds clumps (gh:#1415, #1491 item 4). A counted atom then
    /// adds `w * rmass` to the mass and `w` to the count, so a clump counts its
    /// mass once in total.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] as `FixMassflowMesh::post_create` with a
    /// `fix multisphere`: `delete_atoms` is refused and `count once` required.
    pub fn set_volume_weights(&mut self, w: Vec<f64>) -> Result<(), DemError> {
        if self.delete_atoms {
            return Err(DemError::InvalidInput(
                "fix massflow/mesh: can not use 'delete_atoms' with fix multisphere/*".into(),
            ));
        }
        if !self.cfg.once {
            return Err(DemError::InvalidInput(
                "fix massflow/mesh: must use 'count once' with fix multisphere/*".into(),
            ));
        }
        self.volume_weight = Some(w);
        Ok(())
    }

    /// `FixMassflowMesh::post_integrate`: one invocation per step, after the
    /// particles have moved. `tags[i]` is particle `i`'s tag, `dt` the
    /// timestep.
    ///
    /// `mesh` is the counted mesh in its current pose: the counter's own
    /// ([`MassFlowMesh::mesh_arc`]) or, with [`MassFlowMesh::with_moving_wall`],
    /// the moving wall's.
    pub fn post_integrate(
        &mut self,
        mesh: &MeshWall,
        particles: &[Particle],
        tags: &[u64],
        dt: f64,
    ) {
        if self.wall.is_some() {
            self.set_ref_point(mesh);
        }
        self.steps += 1;
        self.t_count += dt;
        if !self.reset_t_count {
            self.nparticles_last = self.nparticles;
            self.mass_last = self.mass;
            self.reset_t_count = true;
        }
        let nodes = mesh.nodes();
        let (mut mass_this, mut n_this, mut prop_this) = (0.0, 0.0, 0.0);
        for i_tri in 0..nodes.len() {
            for &ip in &self.lists[i_tri] {
                let p = &particles[ip];
                let tag = tags[ip];
                let c = *self.counter.get(&tag).unwrap_or(&-1.0);
                if c == 2.0 {
                    continue;
                }
                let x = arr(p.position);
                let (deltan, delta, _bary, bary_sign) = mesh
                    .state
                    .resolve_tri_sphere_contact_bary(&nodes[i_tri], i_tri, p.radius, &x, true);
                let d = if let Some(out) = self.cfg.point_at_outlet {
                    if deltan < p.radius {
                        let nv = [x[0] - out.x, x[1] - out.y, x[2] - out.z];
                        let d = dot(&delta, &nv);
                        if self.cfg.inside_out {
                            -d
                        } else {
                            d
                        }
                    } else {
                        continue;
                    }
                } else {
                    let dl = [
                        x[0] - self.pref[0],
                        x[1] - self.pref[1],
                        x[2] - self.pref[2],
                    ];
                    dot(&dl, &self.nvec)
                };
                if c == -1.0 {
                    self.counter.insert(tag, if d <= 0.0 { 0.0 } else { 1.0 });
                    continue;
                }
                if d > 0.0 && bary_sign == 7 {
                    if c == 0.0 {
                        // `(fix_volumeweight_ms_ ? vector_atom[iPart] : 1.)`:
                        // a clump's member atoms each carry the clump mass and
                        // their share of its volume
                        let w = self
                            .volume_weight
                            .as_ref()
                            .map_or(1.0, |v| v.get(ip).copied().unwrap_or(1.0));
                        mass_this += w * p.mass;
                        n_this += w;
                        if let Some(prop) = &self.property {
                            prop_this += prop.get(&tag).copied().unwrap_or(0.0);
                        }
                        if self.delete_atoms {
                            self.pending_delete.push(tag);
                        }
                        self.log.push(Crossing {
                            tag,
                            time: dt * self.steps as f64,
                            diameter: 2.0 * p.radius,
                            position: p.position,
                            velocity: p.velocity,
                        });
                    }
                    if self.delete_atoms {
                        // marked: reset the counter so other fixes do not trip
                        if c == 0.0 {
                            self.counter.insert(tag, -1.0);
                        }
                    } else {
                        self.counter
                            .insert(tag, if self.cfg.once { 2.0 } else { 1.0 });
                    }
                } else if d <= 0.0 {
                    self.counter.insert(tag, 0.0);
                }
            }
        }
        self.mass += mass_this;
        self.nparticles += n_this;
        self.property_sum += prop_this;
    }

    /// Rebuild the per-facet neighbour lists from the current positions
    /// (`FixNeighlistMesh::pre_force` on a neighbour-list rebuild step): a
    /// static mesh uses threshold `0.5 * skin` and `r * contactDistanceFactor`
    /// (`= r`) as the sphere radius. Particles are listed in index order
    /// (upstream: bin order; the sums involved are order-independent for equal
    /// masses and differ by round-off otherwise).
    pub fn rebuild_lists(&mut self, mesh: &MeshWall, particles: &[Particle]) {
        let nodes = mesh.nodes();
        // a moving mesh uses the full skin (`changingMesh`)
        let threshold = if self.wall.is_some() {
            self.cfg.skin
        } else {
            0.5 * self.cfg.skin
        };
        self.lists = (0..nodes.len())
            .map(|i| {
                particles
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        mesh.state.neighbuild(
                            &nodes[i],
                            i,
                            p.radius,
                            &arr(p.position),
                            threshold,
                        )
                    })
                    .map(|(k, _)| k)
                    .collect()
            })
            .collect();
    }

    /// `FixMassflowMesh::compute_vector(index)`: 0 mass, 1 particle count,
    /// 2 mass rate, 3 particle rate (over the time since the previous
    /// evaluation), 4/5 deleted mass/count (always 0 here).
    pub fn compute_vector(&mut self, index: usize) -> f64 {
        if self.reset_t_count {
            self.delta_t = self.t_count;
            self.t_count = 0.0;
            self.reset_t_count = false;
        }
        match index {
            0 => self.mass,
            1 => self.nparticles,
            2 => {
                if self.delta_t == 0.0 {
                    0.0
                } else {
                    (self.mass - self.mass_last) / self.delta_t
                }
            }
            3 => {
                if self.delta_t == 0.0 {
                    0.0
                } else {
                    (self.nparticles - self.nparticles_last) / self.delta_t
                }
            }
            4 => self.mass_deleted,
            5 => self.nparticles_deleted,
            6 if self.property.is_some() => self.property_sum,
            _ => 0.0,
        }
    }

    /// Crossings logged so far (the upstream `file`/`screen` lines).
    #[must_use]
    pub fn crossings(&self) -> &[Crossing] {
        &self.log
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::f64::{Length, Mass, ThermodynamicTemperature};
    use uom::si::{length::meter, mass::kilogram, thermodynamic_temperature::kelvin};

    const PLATE: &str = "solid p\nfacet normal 0 0 1\nouter loop\nvertex -1 -1 0\nvertex 1 -1 0\nvertex 1 1 0\nendloop\nendfacet\nfacet normal 0 0 1\nouter loop\nvertex -1 -1 0\nvertex 1 1 0\nvertex -1 1 0\nendloop\nendfacet\nendsolid p\n";

    fn ball(z: f64, vz: f64) -> Particle {
        Particle::new(
            Vec3::new(0.2, 0.1, z),
            Vec3::new(0.0, 0.0, vz),
            Vec3::zero(),
            Mass::new::<kilogram>(1.0e-3),
            Length::new::<meter>(0.005),
            ThermodynamicTemperature::new::<kelvin>(300.0),
        )
        .expect("valid ball")
    }

    fn fix(once: bool) -> MassFlowMesh {
        MassFlowMesh::new(
            MeshWall::from_ascii_stl(PLATE).expect("plate"),
            MassFlowConfig {
                vec_side: Vec3::new(0.0, 0.0, -1.0),
                once,
                point_at_outlet: None,
                inside_out: false,
                skin: 0.003,
            },
        )
        .expect("valid fix")
    }

    /// **Methodology.** A sphere (r = 5 mm) 4 mm above the plate overlaps it
    /// by 1 mm; with `sieveSize = 0` the pass probability is zero, so the
    /// first contact is blocked and the sieve force is `Fn = -k deltan = 10 N`
    /// along the contact normal `(0, 0, -0.8)` (delta/r), i.e. `f_z = +8 N`
    /// (`f -= Fn n`). A second call keeps the same facet's force; moving the
    /// sphere away releases it (`applyForce = -1`). **Result:** exact to 1e-12.
    #[test]
    fn sieve_force_blocks_and_releases() {
        let mut f = fix(true)
            .with_sieve(SieveConfig {
                size: 0.0,
                spacing: 0.015,
                stiffness: 1.0e4,
                damping: 0.0,
            })
            .expect("valid sieve");
        let mesh = f.mesh_arc();
        let mut ps = vec![ball(0.004, 0.0)];
        f.rebuild_lists(&mesh, &ps);
        let mut forces = vec![Vec3::zero()];
        f.sieve_post_force(&mesh, &ps, &[0], &mut forces);
        assert!((forces[0].z - 8.0).abs() < 1e-12, "{:?}", forces[0]);
        let mut again = vec![Vec3::zero()];
        f.sieve_post_force(&mesh, &ps, &[0], &mut again);
        assert!((again[0].z - 8.0).abs() < 1e-12);
        // out of contact: the force is released and nothing is added
        ps[0].position.z = 0.02;
        f.rebuild_lists(&mesh, &ps);
        let mut none = vec![Vec3::zero()];
        f.sieve_post_force(&mesh, &ps, &[0], &mut none);
        assert_eq!(none[0], Vec3::zero());
        assert!(f.is_sieve());
    }

    /// **Methodology.** A particle that crosses from the `vec_side` side's
    /// opposite side is counted once, marked for deletion (`take_pending_
    /// deletions`), and `record_deleted` books its mass and count on
    /// `compute_vector(4)`/`(5)` and drops its counter; `sum_property` adds
    /// the property of the counted particle (`compute_vector(6)`).
    /// **Result:** exact.
    #[test]
    fn delete_marks_books_and_sums_property() {
        let mut f = fix(true)
            .with_delete_atoms()
            .expect("count once")
            .with_sum_property();
        f.set_property(7, 2.5);
        let mesh = f.mesh_arc();
        // above the plate (dot = -z < 0): counter 0, then below: counted
        let mut ps = vec![ball(0.004, -1.0)];
        f.rebuild_lists(&mesh, &ps);
        f.post_integrate(&mesh, &ps, &[7], 1.0e-5);
        ps[0].position.z = -0.001;
        f.rebuild_lists(&mesh, &ps);
        f.post_integrate(&mesh, &ps, &[7], 1.0e-5);
        assert_eq!(f.compute_vector(1), 1.0);
        assert_eq!(f.compute_vector(6), 2.5);
        assert_eq!(f.take_pending_deletions(), vec![7]);
        assert!(f.take_pending_deletions().is_empty());
        f.record_deleted(7, 1.0e-3);
        assert_eq!(f.compute_vector(4), 1.0e-3);
        assert_eq!(f.compute_vector(5), 1.0);
    }

    /// **Methodology.** `with_moving_wall` records the wall index, and the
    /// reference point follows the mesh: after the plate is translated by
    /// 0.1 m, a particle above the new plane is on the far side and one below
    /// it on the near side, as `setRefPoint` re-derives `pref`.
    #[test]
    fn moving_wall_refreshes_reference_point() {
        let mut f = fix(false).with_moving_wall(0);
        assert_eq!(f.wall(), Some(0));
        let mut moved = MeshWall::from_ascii_stl(PLATE).expect("plate");
        let mut nodes = moved.nodes();
        for nd in &mut nodes {
            for n in nd.iter_mut() {
                n[2] += 0.1;
            }
        }
        moved.set_nodes(&nodes);
        moved.rebuild_bounds();
        let ps = vec![ball(0.098, 0.0)];
        f.rebuild_lists(&moved, &ps);
        f.post_integrate(&moved, &ps, &[0], 1.0e-5);
        // pref.z is now 0.1: the particle (z = 0.098) has dot = +0.002 > 0, on the
        // nvec side (counter 1); it has not crossed
        assert_eq!(f.compute_vector(1), 0.0);
        assert!((f.pref[2] - 0.1).abs() < 1e-15);
    }
}
