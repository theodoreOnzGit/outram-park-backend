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
//
// Ported from LIGGGHTS-PUBLIC (GPLv2+), commit 3d5c00f2:
//   src/fix_insert_stream.cpp (FixInsertStream), src/fix_insert.cpp
//   (FixInsert::pre_exchange, distribute_ninsert_this),
//   src/tri_mesh_I.h (generateRandomOwnedGhost, isInElement),
//   src/surface_mesh_I.h (randomOwnedGhostElement, searchElementByAreaAcc).
// Copyright 2012- DCS Computing GmbH, Linz; 2009-2015 JKU Linz.

//! # `fix insert/stream` (gh:#1414)
//!
//! A translation of LIGGGHTS-PUBLIC's `fix insert/stream` for spheres on a
//! planar triangulated insertion face: random points on the face are
//! *extruded* against the face normal by a random length, the particles are
//! created there, **moved on rails with the insertion velocity's face-normal
//! component** until they reach the face (`end_of_step`), and are released
//! there with the full insertion velocity.
//!
//! The mesh side of it (`TriMesh::generateRandomOwnedGhost`: an area-weighted
//! facet from the mesh's own `RanPark(179424799)` stream, then barycentric
//! coordinates by rejection) is translated here from the facets of a
//! [`MeshWall`]. The Monte-Carlo insertion fraction (`calc_ins_fraction`,
//! `ntry_mc` draws from *both* the mesh stream and the fix stream) is
//! translated too, because it advances both streams before the first
//! insertion position is drawn.
//!
//! `all_in yes` is translated (`with_all_in`, from `TriMeshPlanar::
//! generateRandomOwnedGhostWithin` and `buildEdgeLists`, 2026-11-11).
//! `fix insert/stream/predefined` does not exist in LIGGGHTS-PUBLIC 3d5c00f2
//! (only a 2-line stub header `fix_insert_stream_predefined.h` that
//! `pair_gran_base.h` references; no `.cpp`, no registration in `style_fix.h`),
//! so there is nothing to translate or compare.
//!
//! Single process, orthogonal fixed box. Not translated: `parallel yes`,
//! `duration_time`, ~~multi-sphere templates~~ (**CORRECTED 2026-11-14,
//! gh:#1415:** clumps are streamed, with a constant velocity, the identity
//! orientation and no sphere/clump mix; V&V 4.16.3), restart,
//! a moving or scaling face (upstream rejects
//! those too). `is_nearby`'s second filter (`isInBoundingBox`) is implied for a
//! planar face by its first two tests and is not repeated.

use crate::insert::{
    InsertVelocity, InsertedParticle, OverlapGrid, ParticleDistributionDiscrete, Rate, RateTotal,
    RanPark, SMALL,
};
use crate::mesh_wall::MeshWall;
use crate::particle::{Particle, Vec3};
use crate::tri_mesh::{arr, vec3, Nodes, PlanarEdgeLists, TriMeshState};
use crate::DemError;

const TINY: f64 = 1e-14;
/// `MAXTRY_WITHIN` (`tri_mesh_planar.h`).
const MAXTRY_WITHIN: usize = 500;
const SMALL_TRIMESH: f64 = 1.0e-10;
/// `RanPark(lmp,"179424799")` in `MultiNodeMesh`'s constructor.
const MESH_SEED: i32 = 179_424_799;

type V3 = [f64; 3];

fn dot(a: &V3, b: &V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn sub(a: &V3, b: &V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: &V3, b: &V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: &V3, s: f64) -> V3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
fn mag(a: &V3) -> f64 {
    (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt()
}

/// How the insertion interval is given.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StreamExtent {
    /// `extrude_length L`: `insert_every = int((L + 1e-14) / (dt |v_normal|))`.
    ExtrudeLength(f64),
    /// `insert_every N [duration D]`: `L = duration dt |v_normal|`.
    InsertEvery {
        /// Steps between insertions.
        every: u64,
        /// Steps one insertion streams for (default `every`; `<= every`).
        duration: Option<u64>,
    },
}

/// Per-particle release data (`release_fix_insert_stream`, 14 values).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Release {
    /// Position the rail starts at (`release_data[0..3]`).
    pub x_ins: Vec3,
    /// Insertion step (`[3]`).
    pub ins_step: u64,
    /// Step at which the particle is released (`[4]`).
    pub release_step: u64,
    /// Rail velocity (`[5..8]`).
    pub v_integrate: Vec3,
    /// Velocity given at release (`[8..11]`).
    pub v_release: Vec3,
    /// Angular velocity given at release (`[11..14]`).
    pub omega_release: Vec3,
}

/// The result of one insertion step: the particles, in insertion order, and
/// their release data (same order).
#[derive(Debug, Clone, PartialEq)]
pub struct StreamInsertion {
    /// The new particles.
    pub particles: Vec<InsertedParticle>,
    /// Release data, one per particle.
    pub release: Vec<Release>,
}

/// A `fix insert/stream` on a planar triangle mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct InsertStreamFix {
    nodes: Vec<Nodes>,
    state: TriMeshState,
    all_in: Option<PlanarEdgeLists>,
    mesh_random: RanPark,
    area_acc: Vec<f64>,
    area_total: f64,
    distribution: ParticleDistributionDiscrete,
    random: RanPark,
    box_lo: Vec3,
    box_hi: Vec3,
    dt: f64,
    total: Option<RateTotal>,
    rate: Rate,
    velocity: InsertVelocity,
    extent: StreamExtent,
    overlap_check: bool,
    maxattempt: i64,
    ntry_mc: usize,
    exact_number: bool,
    // derived at set-up
    normalvec: V3,
    v_normal: V3,
    p_ref: V3,
    extrude_length: f64,
    extrude_length_min: f64,
    extrude_length_max: f64,
    insert_every: u64,
    ninsert_exists: bool,
    ninsert: i64,
    ninsert_per: f64,
    ins_fraction: f64,
    do_ins_fraction_calc: bool,
    // run state
    next_reneighbor: Option<u64>,
    most_recent_ins_step: Option<u64>,
    ninserted: i64,
    massinserted: f64,
}

impl InsertStreamFix {
    /// `fix ID all insert/stream seed S distributiontemplate D <nparticles|mass>
    /// <particlerate|massrate> vel ... insertion_face F <extrude_length|insert_every>`.
    ///
    /// `face` is the mesh the insertion face is read from (its facets, in file
    /// order, are upstream's `node_`). `dt` is the time step `[s]`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] for the errors upstream raises: an invalid
    /// seed or equal seeds, a non-planar face, a velocity that is too low or
    /// not into the face, both or neither of `insert_every`/`extrude_length`,
    /// an extrusion that is too small, no total or no rate; and, not ported
    /// (gh:#1415), a distribution holding ~~multisphere or~~ superquadric
    /// templates. **CORRECTED 2026-11-14 (gh:#1415, fourth pass):** multisphere
    /// templates are streamed (`FixMultisphere`'s per-body `start_step` and
    /// `v_integrate`; see [`crate::granular_system::GranularSystem::add_stream_insertion`]).
    /// With clumps it also refuses what upstream refuses, a velocity that is
    /// not `constant` ("Currently only fix insert/stream with multisphere
    /// particles only supports constant velocity"), and, not ported, a
    /// distribution mixing spheres and clumps.
    pub fn new(
        seed: i32,
        distribution: ParticleDistributionDiscrete,
        face: &MeshWall,
        extent: StreamExtent,
        total: Option<RateTotal>,
        rate: Rate,
        velocity: InsertVelocity,
        dt: f64,
        box_lo: Vec3,
        box_hi: Vec3,
    ) -> Result<Self, DemError> {
        if distribution.is_superquadric() {
            return Err(DemError::InvalidInput(
                "fix insert/stream with superquadric templates is not ported (gh:#1415)".into(),
            ));
        }
        if distribution.has_clump() {
            if distribution.has_sphere() {
                return Err(DemError::InvalidInput(
                    "fix insert/stream with a distribution mixing spheres and multisphere clumps is not ported (gh:#1415)"
                        .into(),
                ));
            }
            if !matches!(velocity, InsertVelocity::Constant(_)) {
                return Err(DemError::InvalidInput(
                    "Currently only fix insert/stream with multisphere particles only supports constant velocity"
                        .into(),
                ));
            }
        }
        let random = RanPark::checked(seed)?;
        let nodes: Vec<Nodes> = face
            .triangles
            .iter()
            .map(|t| [arr(t.a), arr(t.b), arr(t.c)])
            .collect();
        let state: &TriMeshState = &face.state;
        for a in 0..nodes.len() {
            for b in 0..nodes.len() {
                if a != b
                    && state.node_neighbours(a).contains(&(b as u32))
                    && !state.are_coplanar_node_neighs(a, b)
                {
                    return Err(DemError::InvalidInput(
                        "command requires a planar face for insertion".into(),
                    ));
                }
            }
        }
        let mut area_acc = Vec::with_capacity(nodes.len());
        let mut area_total = 0.0;
        for i in 0..nodes.len() {
            let mut a = state.area(i);
            if i > 0 {
                a += area_acc[i - 1];
            }
            area_acc.push(a);
            area_total += state.area(i);
        }
        let normal0 = state.surface_norm(0);
        let mut fix = Self {
            nodes,
            state: state.clone(),
            all_in: None,
            mesh_random: RanPark::new(MESH_SEED)?,
            area_acc,
            area_total,
            distribution,
            random,
            box_lo,
            box_hi,
            dt,
            total,
            rate,
            velocity,
            extent,
            overlap_check: true,
            maxattempt: 50,
            ntry_mc: 100_000,
            exact_number: true,
            normalvec: normal0,
            v_normal: [0.0; 3],
            p_ref: [0.0; 3],
            extrude_length: 0.0,
            extrude_length_min: 0.0,
            extrude_length_max: 0.0,
            insert_every: 0,
            ninsert_exists: total.is_some(),
            ninsert: 0,
            ninsert_per: 0.0,
            ins_fraction: 0.0,
            do_ins_fraction_calc: true,
            next_reneighbor: Some(1),
            most_recent_ins_step: None,
            ninserted: 0,
            massinserted: 0.0,
        };
        fix.setup()?;
        Ok(fix)
    }

    /// `overlapcheck yes|no` (default yes).
    #[must_use]
    pub fn with_overlap_check(mut self, on: bool) -> Self {
        self.overlap_check = on;
        self
    }

    /// `maxattempt N` (default 50).
    #[must_use]
    pub fn with_maxattempt(mut self, n: usize) -> Self {
        self.maxattempt = n as i64;
        self
    }

    /// `start N` (default 1).
    #[must_use]
    pub fn with_start(mut self, step: u64) -> Self {
        self.next_reneighbor = Some(step);
        self
    }

    /// `random_distribute exact|uncorrelated` (default exact).
    #[must_use]
    pub fn with_exact_number(mut self, exact: bool) -> Self {
        self.exact_number = exact;
        self
    }

    /// `ntry_mc N` (default 100000, upstream requires >= 1000).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `n < 1000`.
    pub fn with_ntry_mc(mut self, n: usize) -> Result<Self, DemError> {
        if n < 1000 {
            return Err(DemError::InvalidInput("ntry_mc must be > 1000".into()));
        }
        self.ntry_mc = n;
        Ok(self)
    }

    fn v_insert(&self) -> V3 {
        match self.velocity {
            InsertVelocity::Constant(v) => arr(v),
            InsertVelocity::Uniform { v, .. } | InsertVelocity::Gaussian { v, .. } => arr(v),
        }
    }

    /// `calc_insertion_properties` (the pieces that apply to a mesh face).
    fn setup(&mut self) -> Result<(), DemError> {
        let err = |m: &str| Err(DemError::InvalidInput(m.into()));
        let v_insert = self.v_insert();
        let mut d = dot(&v_insert, &self.normalvec);
        if d < 0.0 {
            self.normalvec = scale(&self.normalvec, -1.0);
        }
        d = dot(&v_insert, &self.normalvec);
        self.v_normal = scale(&self.normalvec, d);
        if mag(&self.v_normal) < 1.0e-3 {
            return err("insertion velocity projected on face normal is < 1e-3");
        }
        self.p_ref = self.nodes[0][0];
        if mag(&v_insert) < 1e-5 {
            return err("insertion velocity too low");
        }
        let vn = mag(&self.v_normal);
        let max_r_bound = self.distribution.max_r_bound();
        match self.extent {
            StreamExtent::ExtrudeLength(l) => {
                if l <= 0.0 {
                    return err("must define either 'insert_every' or 'extrude_length'");
                }
                self.extrude_length = l;
                if l < 3.0 * max_r_bound && self.overlap_check {
                    return err("'extrude_length' is too small");
                }
                let every = ((l + TINY) / (self.dt * vn)) as i64;
                if every == 0 {
                    return err("insertion velocity too high or extrude_length too low");
                }
                self.insert_every = every as u64;
            }
            StreamExtent::InsertEvery { every, duration } => {
                if every < 1 {
                    return err("'insert_every' must be > 0");
                }
                let duration = duration.unwrap_or(every);
                if duration > every {
                    return err("'duration' > 'insert_every' not allowed");
                }
                self.insert_every = every;
                self.extrude_length = (duration as f64) * self.dt * vn;
                if self.extrude_length < 2.0 * max_r_bound {
                    return err("'insert_every' or 'vel' is too small, or (bounding) radius of inserted particles too large");
                }
            }
        }
        let mass_expect = self.distribution.mass_expect();
        if let Some(t) = self.total {
            self.ninsert = match t {
                RateTotal::Particles(n) => n as i64,
                RateTotal::Mass(m) => {
                    if m <= 0.0 {
                        return err("must define either 'nparticles' or 'mass'");
                    }
                    ((m + TINY) / mass_expect) as i64
                }
            };
        }
        let nflowrate = match self.rate {
            Rate::Particles(n) => n,
            Rate::Mass(m) => m / mass_expect,
        };
        if nflowrate == 0.0 {
            return err("must define either 'massrate' or 'particlerate'");
        }
        self.ninsert_per = nflowrate * ((self.insert_every as f64) * self.dt);
        self.extrude_length_min = 0.0;
        self.extrude_length_max = self.extrude_length;
        Ok(())
    }

    /// Largest radius this fix can insert `[m]`.
    #[must_use]
    pub fn max_rad(&self) -> f64 {
        self.distribution.max_r_bound()
    }

    /// The step of the next insertion, if any.
    #[must_use]
    pub fn next_insertion_step(&self) -> Option<u64> {
        self.next_reneighbor
    }

    /// Particles inserted so far.
    #[must_use]
    pub fn ninserted(&self) -> usize {
        self.ninserted as usize
    }

    /// Mass inserted so far `[kg]`.
    #[must_use]
    pub fn massinserted(&self) -> f64 {
        self.massinserted
    }

    /// Steps between insertions (`insert_every`).
    #[must_use]
    pub fn insert_every(&self) -> u64 {
        self.insert_every
    }

    /// Extrusion length `[m]`.
    #[must_use]
    pub fn extrude_length(&self) -> f64 {
        self.extrude_length
    }

    /// Unit face normal, flipped to point along the insertion velocity.
    #[must_use]
    pub fn normal(&self) -> Vec3 {
        vec3(self.normalvec)
    }

    /// `searchElementByAreaAcc`.
    fn search_area_acc(&self, area: f64, lo: usize, hi: usize) -> usize {
        let acc = &self.area_acc;
        if (lo < 1 || area > acc[lo - 1]) && area <= acc[lo] {
            return lo;
        }
        if (hi < 1 || area > acc[hi - 1]) && area <= acc[hi] {
            return hi;
        }
        let mid = (lo + hi) / 2;
        if area > acc[mid] {
            self.search_area_acc(area, mid, hi)
        } else {
            self.search_area_acc(area, lo, mid)
        }
    }

    /// `TriMesh::generateRandomOwnedGhost`.
    fn generate_random_on_face(&mut self) -> Result<(V3, usize), DemError> {
        let r = self.mesh_random.uniform() * self.area_total;
        let chosen = self.search_area_acc(r, 0, self.nodes.len() - 1);
        if chosen >= self.nodes.len() {
            return Err(DemError::InvalidInput("TriMesh::generate_random error".into()));
        }
        let (mut u, mut v);
        loop {
            u = self.mesh_random.uniform();
            v = self.mesh_random.uniform();
            if u + v <= 1.0 {
                break;
            }
        }
        let (b0, b1, b2) = (1.0 - u - v, v, u);
        let n = &self.nodes[chosen];
        Ok((
            [
                b0 * n[0][0] + b1 * n[1][0] + b2 * n[2][0],
                b0 * n[0][1] + b1 * n[1][1] + b2 * n[2][1],
                b0 * n[0][2] + b1 * n[1][2] + b2 * n[2][2],
            ],
            chosen,
        ))
    }

    /// `TriMeshPlanar::generateRandomOwnedGhostWithin(pos, delta)`: redraw
    /// (at most `MAXTRY_WITHIN` = 500 times) until the point is at least
    /// `delta` from every stored nearest active edge of its facet. Like
    /// upstream, after 500 tries the last point is used (upstream warns).
    fn generate_random_within(&mut self, delta: f64) -> Result<V3, DemError> {
        let mut ntry = 0;
        loop {
            ntry += 1;
            let (pos, i_srf) = self.generate_random_on_face()?;
            let mut far_enough = true;
            let lists = self.all_in.as_ref().expect("all_in lists");
            if lists.min_dist[i_srf] < delta {
                let mut i = 0;
                while i < 6 && lists.edge_facet[i_srf][i] >= 0 && far_enough {
                    let mut min_dist = 2.0 * delta;
                    let j = lists.edge_facet[i_srf][i] as usize;
                    let edge = lists.edge_index[i_srf][i] as usize;
                    let dist = self.state.edge_point_dist(&self.nodes, j, edge, &pos);
                    if dist < min_dist {
                        min_dist = dist;
                    }
                    if min_dist < delta {
                        far_enough = false;
                    }
                    i += 1;
                }
            }
            if far_enough || ntry >= MAXTRY_WITHIN {
                return Ok(pos);
            }
        }
    }

    /// `all_in yes|no` (default no): insert only particles lying wholly inside
    /// the insertion face. Needs a planar face (always true here) and, as
    /// upstream, `extrude_length >= 3 * max_r_bound`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if `extrude_length` is too small.
    pub fn with_all_in(mut self, on: bool) -> Result<Self, DemError> {
        if on {
            if self.extrude_length < 3.0 * self.distribution.max_r_bound() {
                return Err(DemError::InvalidInput("'extrude_length' is too small".into()));
            }
            self.all_in = Some(self.state.build_planar_edge_lists(&self.nodes));
        } else {
            self.all_in = None;
        }
        Ok(self)
    }

    /// `generate_random(pos, rad)`.
    fn generate_random(&mut self, rad: f64) -> Result<V3, DemError> {
        let pos = if self.all_in.is_some() {
            self.generate_random_within(rad)?
        } else {
            self.generate_random_on_face()?.0
        };
        let r = if self.overlap_check {
            -1.0 * (self.random.uniform() * self.extrude_length_max + rad + self.extrude_length_min)
        } else {
            -1.0 * (self.random.uniform() * (self.extrude_length_max - 2.0 * rad)
                + rad
                + self.extrude_length_min)
        };
        Ok(add(&pos, &scale(&self.normalvec, r)))
    }

    /// `generate_random_global`.
    fn generate_random_global(&mut self) -> Result<V3, DemError> {
        let pos = self.generate_random_on_face()?.0;
        let r = -1.0 * (self.random.uniform() * self.extrude_length);
        Ok(add(&pos, &scale(&self.normalvec, r)))
    }

    fn in_subdomain(&self, p: &V3) -> bool {
        p[0] >= self.box_lo.x - SMALL
            && p[0] < self.box_hi.x + SMALL
            && p[1] >= self.box_lo.y - SMALL
            && p[1] < self.box_hi.y + SMALL
            && p[2] >= self.box_lo.z - SMALL
            && p[2] < self.box_hi.z + SMALL
    }

    fn dist_subbox_borders(&self, p: &V3) -> f64 {
        let dlo = (self.box_lo.x - p[0])
            .abs()
            .min((self.box_lo.y - p[1]).abs())
            .min((self.box_lo.z - p[2]).abs());
        let dhi = (self.box_hi.x - p[0])
            .abs()
            .min((self.box_hi.y - p[1]).abs())
            .min((self.box_hi.z - p[2]).abs());
        if dlo < dhi {
            dlo
        } else {
            dhi
        }
    }

    /// `calc_ins_fraction`: `ntry_mc` global draws (mesh stream and fix stream).
    fn calc_ins_fraction(&mut self) -> Result<(), DemError> {
        self.do_ins_fraction_calc = false;
        let mut n_in = 0_usize;
        for _ in 0..self.ntry_mc {
            let pos = self.generate_random_global()?;
            if self.in_subdomain(&pos) {
                n_in += 1;
            }
        }
        self.ins_fraction = n_in as f64 / self.ntry_mc as f64;
        if self.ins_fraction < 0.9 || self.ins_fraction > 1.1 {
            return Err(DemError::InvalidInput(
                "insertion volume could not be distributed properly in parallel. Bad decomposition or \
                 insertion face extrusion is too small or outside domain"
                    .into(),
            ));
        }
        Ok(())
    }

    /// `TriMesh::isInElement`.
    fn is_in_element(&self, pos: &V3, i: usize) -> bool {
        let n = &self.nodes[i];
        let v0 = sub(&n[2], &n[0]);
        let v1 = sub(&n[1], &n[0]);
        let v2 = sub(pos, &n[0]);
        let dot00 = dot(&v0, &v0);
        let dot01 = dot(&v0, &v1);
        let dot02 = dot(&v0, &v2);
        let dot11 = dot(&v1, &v1);
        let dot12 = dot(&v1, &v2);
        let inv = 1.0 / (dot00 * dot11 - dot01 * dot01);
        let u = (dot11 * dot02 - dot01 * dot12) * inv;
        let v = (dot00 * dot12 - dot01 * dot02) * inv;
        u > -SMALL_TRIMESH && v > -SMALL_TRIMESH && u + v < 1.0 + SMALL_TRIMESH
    }

    /// `FixInsertStream::is_nearby`.
    fn is_nearby(&self, x: &V3) -> bool {
        let maxrad = self.distribution.max_r_bound();
        let pos_rel = sub(x, &self.p_ref);
        let dist_normal = dot(&pos_rel, &self.normalvec);
        if dist_normal > maxrad {
            return false;
        }
        if dist_normal < -(self.extrude_length + 3.0 * maxrad) {
            return false;
        }
        let t = scale(&self.normalvec, dist_normal);
        let projected = add(x, &t);
        (0..self.nodes.len()).any(|i| self.is_in_element(&projected, i))
    }

    /// `FixInsert::generate_random_velocity`.
    fn generate_random_velocity(&mut self, v_insert: &V3) -> V3 {
        match self.velocity {
            InsertVelocity::Constant(_) => *v_insert,
            InsertVelocity::Uniform { fluct, .. } => {
                let x = v_insert[0] + fluct.x * 2.0 * (self.random.uniform() - 0.50);
                let y = v_insert[1] + fluct.y * 2.0 * (self.random.uniform() - 0.50);
                let z = v_insert[2] + fluct.z * 2.0 * (self.random.uniform() - 0.50);
                [x, y, z]
            }
            InsertVelocity::Gaussian { fluct, .. } => {
                let x = v_insert[0] + fluct.x * self.random.gaussian();
                let y = v_insert[1] + fluct.y * self.random.gaussian();
                let z = v_insert[2] + fluct.z * self.random.gaussian();
                [x, y, z]
            }
        }
    }

    fn schedule_next(&mut self, zero_case: bool) {
        if self.insert_every != 0 && (!self.ninsert_exists || self.ninserted < self.ninsert) {
            self.next_reneighbor = self.next_reneighbor.map(|n| n + self.insert_every);
        } else {
            let _ = zero_case;
            self.next_reneighbor = None;
        }
    }

    /// `FixInsert::pre_exchange` + `FixInsertStream::finalize_insertion`: on the
    /// scheduled step generate the particles (and their release data) in
    /// insertion order. `atoms` is the current ensemble (overlap test).
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] where upstream stops with an error.
    pub fn pre_exchange(
        &mut self,
        step: u64,
        atoms: &[Particle],
    ) -> Result<StreamInsertion, DemError> {
        let empty = StreamInsertion {
            particles: Vec::new(),
            release: Vec::new(),
        };
        if self.distribution.has_clump() {
            return Err(DemError::InvalidInput(
                "fix insert/stream with clumps: insert through GranularSystem::add_stream_insertion".into(),
            ));
        }
        let Some(n_local) = self.begin_insertion(step)? else {
            return Ok(empty);
        };
        let list = self.distribution.randomize_list(n_local, self.exact_number);
        if list.is_empty() {
            self.schedule_after_empty_list();
            return Ok(empty);
        }
        self.insert_spheres(step, atoms, &list)
    }

    /// The front of `FixInsert::pre_exchange`: on the scheduled step,
    /// `calc_ninsert_this` and `distribute_ninsert_this` (one process).
    /// `None` when this is not an insertion step.
    fn begin_insertion(&mut self, step: u64) -> Result<Option<usize>, DemError> {
        if self.next_reneighbor != Some(step) || self.most_recent_ins_step == Some(step) {
            return Ok(None);
        }
        self.most_recent_ins_step = Some(step);
        // calc_ninsert_this
        let mut ninsert_this = (self.ninsert_per + self.random.uniform()) as i64;
        if self.ninsert_exists && self.ninserted + ninsert_this > self.ninsert {
            ninsert_this = self.ninsert - self.ninserted;
        }
        if self.ninsert_exists && self.ninserted + ninsert_this >= self.ninsert {
            ninsert_this = (self.ninsert - self.ninserted).max(0);
            self.exact_number = true;
        }
        // distribute_ninsert_this (one process)
        if self.do_ins_fraction_calc {
            self.calc_ins_fraction()?;
        }
        let fraction_local = self.ins_fraction;
        let ninsert_this_local: i64 = if !self.exact_number {
            (fraction_local * ninsert_this as f64 + self.random.uniform()) as i64
        } else {
            if fraction_local < 0.02 {
                return Err(DemError::InvalidInput(
                    "Internal error distributing particles to processes".into(),
                ));
            }
            ((fraction_local / fraction_local) * ninsert_this as f64) as i64
        };
        Ok(Some(ninsert_this_local.max(0) as usize))
    }

    /// Whether this fix streams multisphere clumps (gh:#1415).
    pub(crate) fn has_clumps(&self) -> bool {
        self.distribution.has_clump()
    }

    /// `FixInsert::pre_exchange` for clumps (gh:#1415, fourth pass):
    /// `FixInsertStream::x_v_omega` places each list entry at
    /// `generate_random(pos, r_bound_ins)` with
    /// `check_near_set_x_v_omega(pos, v_normal, omega = 0, quat_insert)` (or
    /// `set_x_v_omega` without the overlap check), `quat_insert` being the
    /// identity (`orientation` is not ported for this fix). The caller
    /// inserts the clumps in order (atoms, then `add_body`), runs
    /// `add_body_finalize`, and then builds each new atom's release data with
    /// [`InsertStreamFix::clump_release`] (`FixInsertStream::finalize_insertion`).
    pub(crate) fn pre_exchange_clumps(
        &mut self,
        step: u64,
        atoms: &[Particle],
    ) -> Result<Vec<crate::multisphere::ClumpToInsert>, DemError> {
        let Some(n_local) = self.begin_insertion(step)? else {
            return Ok(Vec::new());
        };
        let mut list: Vec<crate::multisphere::ClumpToInsert> = Vec::with_capacity(n_local);
        for p in self.distribution.randomize_list_any(n_local, self.exact_number) {
            match p {
                crate::insert::Pti::Clump(c) => list.push(c),
                crate::insert::Pti::Sphere(_) | crate::insert::Pti::Superquadric(_) => {
                    return Err(DemError::InvalidInput(
                        "fix insert/stream: a clump distribution produced a non-clump entry".into(),
                    ))
                }
            }
        }
        if list.is_empty() {
            self.schedule_after_empty_list();
            return Ok(Vec::new());
        }
        let v = vec3(self.v_normal);
        let omega = Vec3::zero();
        let quat = [1.0, 0.0, 0.0, 0.0];
        let n_local = list.len();
        let maxtry = n_local as i64 * self.maxattempt;
        let mut ntry: i64 = 0;
        let mut placed: Vec<crate::multisphere::ClumpToInsert> = Vec::with_capacity(n_local);
        if !self.overlap_check {
            for _ in 0..n_local {
                let rad = list[placed.len()].r_bound;
                let pos = loop {
                    let pos = self.generate_random(rad)?;
                    ntry += 1;
                    if !(ntry < maxtry && !self.in_subdomain(&pos)) {
                        break pos;
                    }
                };
                if ntry < maxtry {
                    let mut c = list[placed.len()].clone();
                    c.set_x_v_omega(vec3(pos), v, omega, quat);
                    placed.push(c);
                }
            }
        } else {
            let mut grid = OverlapGrid::new(
                2.0 * self
                    .distribution
                    .max_r_bound()
                    .max(atoms.iter().fold(0.0, |m, a| a.radius.max(m))),
            );
            for a in atoms {
                if self.is_nearby(&arr(a.position)) {
                    grid.insert(a.position, a.radius);
                }
            }
            while ntry < maxtry && placed.len() < n_local {
                let mut c = list[placed.len()].clone();
                let rad = c.r_bound;
                let mut nins = 0;
                while nins == 0 && ntry < maxtry {
                    let pos = loop {
                        let pos = self.generate_random(rad)?;
                        ntry += 1;
                        if !(ntry < maxtry
                            && (!self.in_subdomain(&pos) || self.dist_subbox_borders(&pos) < rad))
                        {
                            break pos;
                        }
                    };
                    if ntry < maxtry {
                        if let Some(spheres) =
                            c.check_near_set_x_v_omega(vec3(pos), v, omega, quat, |p, r| grid.has_overlap(p, r))
                        {
                            for (p, r) in spheres {
                                grid.insert(p, r);
                            }
                            nins = 1;
                        }
                    }
                }
                if nins > 0 {
                    placed.push(c);
                }
            }
        }
        let mut mass_inserted_this = 0.0;
        for c in &placed {
            mass_inserted_this += c.mass;
        }
        self.ninserted += placed.len() as i64;
        self.massinserted += mass_inserted_this;
        self.schedule_next(false);
        Ok(placed)
    }

    /// `FixInsertStream::finalize_insertion` for one new member atom at `x`
    /// whose body was given `n_steps` by `Multisphere::calc_n_steps`: the
    /// release data (`x`, `step`, `step + n_steps`, `v_normal`,
    /// `generate_random_velocity(v_insert)`, `omega_insert` = 0).
    pub(crate) fn clump_release(&mut self, x: Vec3, step: u64, n_steps: i64) -> Release {
        let v_insert = self.v_insert();
        let v_to_insert = self.generate_random_velocity(&v_insert);
        Release {
            x_ins: x,
            ins_step: step,
            release_step: (step as i64 + n_steps) as u64,
            v_integrate: vec3(self.v_normal),
            v_release: vec3(v_to_insert),
            omega_release: Vec3::zero(),
        }
    }

    /// `p_ref`, the face normal and `v_normal` (`calc_n_steps` arguments).
    pub(crate) fn stream_geometry(&self) -> (V3, V3, V3) {
        (self.p_ref, self.normalvec, self.v_normal)
    }

    /// The schedule of `FixInsert::pre_exchange` when the list to insert is
    /// empty.
    fn schedule_after_empty_list(&mut self) {
        if self.insert_every != 0 && (!self.ninsert_exists || self.ninserted < self.ninsert) {
            self.next_reneighbor = self.next_reneighbor.map(|n| n + self.insert_every);
        } else if self.insert_every == 0 {
            self.next_reneighbor = None;
        }
    }

    /// `FixInsert::pre_exchange` for spheres, after
    /// [`InsertStreamFix::begin_insertion`]: place `list` and build the
    /// particles and their release data.
    fn insert_spheres(
        &mut self,
        step: u64,
        atoms: &[Particle],
        list: &[crate::insert::ParticleToInsert],
    ) -> Result<StreamInsertion, DemError> {
        // load xnear, x_v_omega
        let mut grid = OverlapGrid::new(
            2.0 * self
                .distribution
                .max_r_bound()
                .max(atoms.iter().fold(0.0, |m, a| a.radius.max(m))),
        );
        if self.overlap_check {
            for a in atoms {
                if self.is_nearby(&arr(a.position)) {
                    grid.insert(a.position, a.radius);
                }
            }
        }
        let n_local = list.len();
        let maxtry = n_local as i64 * self.maxattempt;
        let mut ntry: i64 = 0;
        let mut placed: Vec<V3> = Vec::with_capacity(n_local);
        if !self.overlap_check {
            for _ in 0..n_local {
                let rad = list[placed.len()].radius;
                let pos = loop {
                    let pos = self.generate_random(rad)?;
                    ntry += 1;
                    if !(ntry < maxtry && !self.in_subdomain(&pos)) {
                        break pos;
                    }
                };
                if ntry < maxtry {
                    placed.push(pos);
                }
            }
        } else {
            while ntry < maxtry && placed.len() < n_local {
                let rad = list[placed.len()].radius;
                let mut nins = 0;
                while nins == 0 && ntry < maxtry {
                    let pos = loop {
                        let pos = self.generate_random(rad)?;
                        ntry += 1;
                        if !(ntry < maxtry
                            && (!self.in_subdomain(&pos) || self.dist_subbox_borders(&pos) < rad))
                        {
                            break pos;
                        }
                    };
                    if ntry < maxtry && !grid.has_overlap(vec3(pos), rad) {
                        grid.insert(vec3(pos), rad);
                        placed.push(pos);
                        nins = 1;
                    }
                }
            }
        }
        // finalize_insertion
        let v_insert = self.v_insert();
        let mut out = StreamInsertion {
            particles: Vec::with_capacity(placed.len()),
            release: Vec::with_capacity(placed.len()),
        };
        let mut mass_inserted_this = 0.0;
        for (pti, pos) in list.iter().zip(&placed) {
            mass_inserted_this += pti.mass;
            out.particles.push(InsertedParticle {
                particle: Particle {
                    position: vec3(*pos),
                    velocity: vec3(self.v_normal),
                    angular_velocity: Vec3::zero(),
                    mass: pti.mass,
                    radius: pti.radius,
                    temperature: 300.0,
                },
                atom_type: pti.atom_type,
                density: pti.density,
            });
        }
        let dt = self.dt;
        for pos in &placed {
            let pos_rel = sub(&self.p_ref, pos);
            let dist_normal = dot(&pos_rel, &self.normalvec);
            let n_steps = ((dist_normal + TINY) / (mag(&self.v_normal) * dt)) as i64;
            let v_to_insert = self.generate_random_velocity(&v_insert);
            out.release.push(Release {
                x_ins: vec3(*pos),
                ins_step: step,
                release_step: (step as i64 + n_steps) as u64,
                v_integrate: vec3(self.v_normal),
                v_release: vec3(v_to_insert),
                omega_release: Vec3::zero(),
            });
        }
        self.ninserted += placed.len() as i64;
        self.massinserted += mass_inserted_this;
        self.schedule_next(false);
        Ok(out)
    }
}

/// `FixInsertStream::end_of_step` for one particle: returns `(position,
/// velocity, omega)` to write and whether the entry is finished.
///
/// While `step < release_step` the particle rides the rail with `v_integrate`
/// and zero spin; at `release_step` it takes its release velocity and spin;
/// after that upstream skips it (`step > r_step`). Force and torque are zeroed
/// by the caller in the first two cases.
#[must_use]
pub fn end_of_step_state(rel: &Release, step: u64, dt: f64) -> Option<(Vec3, Vec3, Vec3, bool)> {
    if step > rel.release_step {
        return None;
    }
    let time_elapsed = (step - rel.ins_step) as f64 * dt;
    let dist = rel.v_integrate.scale(time_elapsed);
    let x = Vec3::new(
        rel.x_ins.x + dist.x,
        rel.x_ins.y + dist.y,
        rel.x_ins.z + dist.z,
    );
    if step == rel.release_step {
        Some((x, rel.v_release, rel.omega_release, true))
    } else {
        Some((x, rel.v_integrate, Vec3::zero(), false))
    }
}
