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
//! returned `Vec`). **Not translated:** `delete_atoms yes`, `sum_property`,
//! the multisphere volume weight, restart, and a *moving or deforming*
//! counting mesh (the reference point is set once). Like upstream, only the
//! particles in the mesh's neighbour list (`createOtherNeighList`) are
//! visited, and that list is built at neighbour-list rebuilds
//! ([`MassFlowMesh::rebuild_lists`], called by the owning system at set-up
//! and on its rebuild schedule): a particle outside the list keeps a stale
//! counter, which changes what is counted when it later enters the list
//! (found 2026-11-11, when a bounding-sphere shortcut miscounted one
//! crossing in the cross-code deck).

use crate::mesh_wall::MeshWall;
use crate::particle::{Particle, Vec3};
use crate::tri_mesh::arr;
use crate::DemError;
use std::collections::HashMap;

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
    mesh: MeshWall,
    cfg: MassFlowConfig,
    nvec: [f64; 3],
    pref: [f64; 3],
    counter: HashMap<u64, f64>,
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
            mesh,
            cfg,
            nvec,
            pref,
            counter: HashMap::new(),
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

    /// `FixMassflowMesh::post_integrate`: one invocation per step, after the
    /// particles have moved. `tags[i]` is particle `i`'s tag, `dt` the
    /// timestep.
    pub fn post_integrate(&mut self, particles: &[Particle], tags: &[u64], dt: f64) {
        self.steps += 1;
        self.t_count += dt;
        if !self.reset_t_count {
            self.nparticles_last = self.nparticles;
            self.mass_last = self.mass;
            self.reset_t_count = true;
        }
        let nodes = self.mesh.nodes();
        let (mut mass_this, mut n_this) = (0.0, 0.0);
        for i_tri in 0..nodes.len() {
            for &ip in &self.lists[i_tri] {
                let p = &particles[ip];
                let tag = tags[ip];
                let c = *self.counter.get(&tag).unwrap_or(&-1.0);
                if c == 2.0 {
                    continue;
                }
                let x = arr(p.position);
                let (deltan, delta, _bary, bary_sign) = self
                    .mesh
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
                        mass_this += 1.0 * p.mass;
                        n_this += 1.0;
                        self.log.push(Crossing {
                            tag,
                            time: dt * self.steps as f64,
                            diameter: 2.0 * p.radius,
                            position: p.position,
                            velocity: p.velocity,
                        });
                    }
                    self.counter
                        .insert(tag, if self.cfg.once { 2.0 } else { 1.0 });
                } else if d <= 0.0 {
                    self.counter.insert(tag, 0.0);
                }
            }
        }
        self.mass += mass_this;
        self.nparticles += n_this;
    }

    /// Rebuild the per-facet neighbour lists from the current positions
    /// (`FixNeighlistMesh::pre_force` on a neighbour-list rebuild step): a
    /// static mesh uses threshold `0.5 * skin` and `r * contactDistanceFactor`
    /// (`= r`) as the sphere radius. Particles are listed in index order
    /// (upstream: bin order; the sums involved are order-independent for equal
    /// masses and differ by round-off otherwise).
    pub fn rebuild_lists(&mut self, particles: &[Particle]) {
        let nodes = self.mesh.nodes();
        let threshold = 0.5 * self.cfg.skin;
        self.lists = (0..nodes.len())
            .map(|i| {
                particles
                    .iter()
                    .enumerate()
                    .filter(|(_, p)| {
                        self.mesh.state.neighbuild(
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
            _ => 0.0,
        }
    }

    /// Crossings logged so far (the upstream `file`/`screen` lines).
    #[must_use]
    pub fn crossings(&self) -> &[Crossing] {
        &self.log
    }
}
