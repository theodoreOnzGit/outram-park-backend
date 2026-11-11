// SPDX-License-Identifier: AGPL-3.0-only
//! Multi-contact half-space model of Brodu et al. (Phys. Rev. E, 2016).
//!
//! Translated from LIGGGHTS-PUBLIC commit `3d5c00f2` (GPL-2.0-or-later),
//! contributing author Arno Mayrhofer, copyright 2016- CFDEMresearch GmbH,
//! Linz:
//!
//! * `src/surface_model_multicontact.h`, `SurfaceModel<SURFACE_MULTICONTACT>`:
//!   [`surfaces_intersect_history`], [`surfaces_close_history`],
//!   [`end_surfaces_intersect_history`] and the layout constants;
//! * `src/fix_multicontact_halfspace.cpp`, `FixMultiContactHalfSpace::pre_force`
//!   and `HistoryData::{compute_surfPos, get_fn, save_contact_property_atom}`:
//!   [`HalfSpaceFix`] and [`McData`];
//! * the radius expansion read in `pair_gran_base.h` (pairs) and
//!   `fix_wall_gran.cpp` (primitive and mesh walls):
//!   [`McData::pair_expansion`], [`McData::wall_expansion`].
//!
//! Each contact is a half-space loaded by the normal forces of the particle's
//! other contacts; the displacement `delta_ij` the other contacts cause at the
//! contact point expands the particle's effective radius for that contact
//! only. The neighbour list must look out to a contact-distance factor of
//! `2.0` ([`CONTACT_DISTANCE_FACTOR`], upstream
//! `neighbor->register_contact_dist_factor(2.0)`).
//!
//! The module is pure data: positions, tags and the per-contact history in,
//! the per-contact radius expansions out. [`MulticontactState`] holds the
//! per-contact history between steps; `GranularSystem::with_multicontact`
//! hooks it into the pair and primitive-wall loops (mesh walls are not hooked).
//!
//! Research, education and V&V only (`RESPONSIBLE_USE.md`).

use std::f64::consts::PI;

/// Neighbour contact-distance factor the multicontact surface model registers
/// (upstream `neighbor->register_contact_dist_factor(2.0)`, "assume it's at
/// most one radius away").
pub const CONTACT_DISTANCE_FACTOR: f64 = 2.0;

/// Pair history slot of `radij` (radius of `i` plus expansion at contact `ij`).
pub const PAIR_RADIJ: usize = 0;
/// Pair history slot of `radji`.
pub const PAIR_RADJI: usize = 1;
/// Pair history slot of the normal force `fn`.
pub const PAIR_FN: usize = 2;
/// Wall history slots `surfPos_x/y/z` are 0..3.
pub const WALL_SURFPOS: usize = 0;
/// Wall history slot of the normal force `fn`.
pub const WALL_FN: usize = 3;

/// Default of the `geometric_prefactor` keyword (upstream constructor).
pub const DEFAULT_GEOMETRIC_PREFACTOR: f64 = 1.125;

fn mag(v: &[f64; 3]) -> f64 {
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}
fn dot(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn sub(a: &[f64; 3], b: &[f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn scale(v: &[f64; 3], s: f64) -> [f64; 3] {
    [s * v[0], s * v[1], s * v[2]]
}
/// `v1 + f * v2` (upstream `vectorAddMultiple3D`).
fn add_mult(v1: &[f64; 3], f: f64, v2: &[f64; 3]) -> [f64; 3] {
    [v1[0] + f * v2[0], v1[1] + f * v2[1], v1[2] + f * v2[2]]
}

/// What `SurfaceModel<SURFACE_MULTICONTACT>::surfacesIntersect` writes into the
/// contact history when `update_history` (`computeflag && shearupdate`) is set.
///
/// Pair: `[radi, radj]` (slots 0, 1). Wall: `-delta` (slots 0..3).
#[must_use]
pub fn surfaces_intersect_history(is_wall: bool, delta: [f64; 3], radi: f64, radj: f64) -> Vec<f64> {
    if is_wall {
        vec![-delta[0], -delta[1], -delta[2]]
    } else {
        vec![radi, radj]
    }
}

/// `endSurfacesIntersect`: the normal force `|F_i . e_n|` stored after the
/// force is known (slot 3 for a wall, 2 for a pair).
#[must_use]
pub fn end_surfaces_intersect_history(i_forces: [f64; 3], en: [f64; 3]) -> f64 {
    dot(&i_forces, &en).abs()
}

/// `surfacesClose`: the history of a contact that is inside the neighbour
/// skin but not overlapping. Wall (slots 0..4):
/// `delta * radi * rinv` with `rinv = -1/|delta|` (0 if `|delta| <= 1e-15`),
/// then `fn = 0`. Pair (slots 0..3): `[radi, radj, 0]`.
#[must_use]
pub fn surfaces_close_history(is_wall: bool, delta: [f64; 3], radi: f64, radj: f64) -> Vec<f64> {
    if is_wall {
        let len = mag(&delta);
        let rinv = if len > 1e-15 { -1.0 / len } else { 0.0 };
        vec![
            delta[0] * radi * rinv,
            delta[1] * radi * rinv,
            delta[2] * radi * rinv,
            0.0,
        ]
    } else {
        vec![radi, radj, 0.0]
    }
}

/// One entry of a `FixContactPropertyAtom(Wall)`: the partner (a particle tag
/// for the pair store, a triangle id for a mesh, `1` for a primitive wall), the
/// contact surface position relative to the owning particle, and the value in
/// slot 3, first the normal force and after step 2 the half-space
/// displacement `delta`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct McEntry {
    /// Partner id (see type docs).
    pub partner: usize,
    /// Contact surface position relative to the particle `[m]`.
    pub surf_pos: [f64; 3],
    /// Slot 3: `f.n` after step 1, `delta` after step 2.
    pub value: f64,
}

/// A particle-particle contact as seen by the fix: upstream's neighbour-list
/// entry `jj` of particle `i`, with its three-slot `delta` history.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PairContact {
    /// Index of partner `j`.
    pub j: usize,
    /// `[radij, radji, fn]` from the contact history.
    pub history: [f64; 3],
}

/// A wall contact (primitive or mesh facet) of particle `i`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallContact {
    /// Facet id for a mesh, `1` for a primitive wall (upstream `idTri`).
    pub id: usize,
    /// `[surfPos_x, surfPos_y, surfPos_z, fn]` from the contact history.
    pub history: [f64; 4],
}

/// The per-contact data `pre_force` produces (upstream's
/// `multicontactData_` property atoms): one store for pairs and one per wall.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct McData {
    /// `pair[i]`: entries in insertion order.
    pub pair: Vec<Vec<McEntry>>,
    /// `walls[w][i]`: entries of wall `w` for particle `i`.
    pub walls: Vec<Vec<Vec<McEntry>>>,
}

impl McData {
    /// Radius expansion of particle `i` at its contact with the particle of
    /// tag `tag_j` (`pair_gran_base.h`: `radi += dataI[3]` when
    /// `has_partner(i, tag[j]) != -1`), else `0`.
    #[must_use]
    pub fn pair_expansion(&self, i: usize, tag_j: usize) -> f64 {
        self.pair[i]
            .iter()
            .find(|e| e.partner == tag_j)
            .map_or(0.0, |e| e.value)
    }

    /// Radius expansion of particle `i` at facet/primitive `id` of wall `w`
    /// (`fix_wall_gran.cpp`: `radi += deltaData[3]` when `haveContact`).
    #[must_use]
    pub fn wall_expansion(&self, w: usize, i: usize, id: usize) -> f64 {
        self.walls[w][i]
            .iter()
            .find(|e| e.partner == id)
            .map_or(0.0, |e| e.value)
    }
}

/// Per-particle inputs of [`HalfSpaceFix::pre_force`].
#[derive(Debug, Clone, Copy)]
pub struct ParticleView<'a> {
    /// Positions `[m]`.
    pub x: &'a [[f64; 3]],
    /// Stable tags.
    pub tag: &'a [usize],
    /// Atom types, 0-based.
    pub typ: &'a [usize],
    /// Radii `[m]`.
    pub radius: &'a [f64],
    /// Fix group membership.
    pub in_group: &'a [bool],
}

/// `fix multicontact/halfspace`.
#[derive(Debug, Clone, PartialEq)]
pub struct HalfSpaceFix {
    /// `geometric_prefactor` keyword (default 1.125); must be `> 0`.
    pub geometric_prefactor: f64,
    /// Young's modulus per atom type (0-based) `[Pa]`.
    pub youngs: Vec<f64>,
    /// Poisson's ratio per atom type (0-based) `[-]`.
    pub poisson: Vec<f64>,
}

impl HalfSpaceFix {
    /// Fix with the default prefactor.
    #[must_use]
    pub fn new(youngs: Vec<f64>, poisson: Vec<f64>) -> Self {
        Self {
            geometric_prefactor: DEFAULT_GEOMETRIC_PREFACTOR,
            youngs,
            poisson,
        }
    }

    /// `HistoryData::compute_surfPos` for a pair contact: `(surfPos_ij,
    /// surfPos_ji)` from the two centres, the stored `[radij, radji, fn]` and
    /// the force threshold `f_eps`.
    #[must_use]
    pub fn pair_surf_pos(
        xi: [f64; 3],
        xj: [f64; 3],
        hist: [f64; 3],
        f_eps: f64,
    ) -> ([f64; 3], [f64; 3]) {
        let xij = sub(&xj, &xi);
        let len = mag(&xij);
        if hist[PAIR_FN] > f_eps {
            let tmp = scale(&xij, (hist[0] - hist[1]) * 0.5 / len);
            (add_mult(&tmp, 0.5, &xij), add_mult(&tmp, -0.5, &xij))
        } else {
            (scale(&xij, hist[0] / len), scale(&xij, -hist[1] / len))
        }
    }

    /// `FixMultiContactHalfSpace::pre_force`, single process.
    ///
    /// `pairs[i]` is particle `i`'s neighbour-list contacts and `walls[w][i]`
    /// its contacts with wall `w`, both in upstream order. Step 1 stores each
    /// contact's surface position and `f.n` (pair contacts are added to both
    /// particles' stores, `j` getting the `ji` position); step 2 replaces the
    /// `f.n` by the displacement `delta` caused by the particle's other
    /// contacts. Primitive-wall contacts with `|surfPos|^2 <= 1e-14` are not
    /// stored, as upstream (`wall_is_mesh[w] == false`).
    ///
    /// # Panics
    /// If a type index is out of range of the modulus tables or the slices
    /// differ in length.
    #[must_use]
    pub fn pre_force(
        &self,
        p: ParticleView<'_>,
        pairs: &[Vec<PairContact>],
        walls: &[Vec<Vec<WallContact>>],
        wall_is_mesh: &[bool],
    ) -> McData {
        let n = p.x.len();
        let mut data = McData {
            pair: vec![Vec::new(); n],
            walls: walls.iter().map(|_| vec![Vec::new(); n]).collect(),
        };

        // STEP 1: surface position and f.n of every contact
        for i in 0..n {
            if !p.in_group[i] {
                continue;
            }
            let yi = self.youngs[p.typ[i]];
            let f_eps = yi * p.radius[i] * p.radius[i] * 1e-14;
            for jc in &pairs[i] {
                let (s_ij, s_ji) = Self::pair_surf_pos(p.x[i], p.x[jc.j], jc.history, f_eps);
                let fnorm = jc.history[PAIR_FN];
                data.pair[i].push(McEntry { partner: p.tag[jc.j], surf_pos: s_ij, value: fnorm });
                data.pair[jc.j].push(McEntry { partner: p.tag[i], surf_pos: s_ji, value: fnorm });
            }
            for (w, wall) in walls.iter().enumerate() {
                for wc in &wall[i] {
                    let s = [wc.history[0], wc.history[1], wc.history[2]];
                    if wall_is_mesh[w] || dot(&s, &s) > 1e-14 {
                        data.walls[w][i].push(McEntry {
                            partner: wc.id,
                            surf_pos: s,
                            value: wc.history[WALL_FN],
                        });
                    }
                }
            }
        }

        // STEP 2: delta_ij from the contacts ik (k != j)
        for i in 0..n {
            if !p.in_group[i] {
                continue;
            }
            let yi = self.youngs[p.typ[i]];
            let nui = self.poisson[p.typ[i]];
            let c1 = -(1.0 + nui) / (2.0 * PI * yi) * self.geometric_prefactor;
            let c2 = 3.0 - 4.0 * nui;
            let c3 = 1.0 - 2.0 * nui;
            let f_eps = yi * p.radius[i] * p.radius[i] * 1e-14;

            // flat list in upstream order: pair store, then each wall
            let mut sp: Vec<McEntry> = data.pair[i].clone();
            for wl in &data.walls {
                sp.extend_from_slice(&wl[i]);
            }
            let inv: Vec<f64> = sp
                .iter()
                .map(|e| 1.0 / mag(&e.surf_pos).max(1e-6 * p.radius[i]))
                .collect();
            let m = sp.len();
            let mut delta = vec![0.0_f64; m];

            // upstream: `*surfPos_ij != surfPos.back()`, so the last entry is never an `ij`
            for a in 0..m.saturating_sub(1) {
                for b in (a + 1)..m {
                    let f_ij = sp[a].value;
                    let f_ik = sp[b].value;
                    if f_ij > f_eps || f_ik > f_eps {
                        let ukc = sub(&sp[a].surf_pos, &sp[b].surf_pos);
                        let inv_dkc = 1.0 / mag(&ukc);
                        if inv_dkc < 1e10 {
                            let nk_dot_nc = dot(&sp[a].surf_pos, &sp[b].surf_pos) * inv[a] * inv[b];
                            let ukc_dot_nc = -dot(&ukc, &sp[a].surf_pos) * inv[a] * inv_dkc;
                            let ukc_dot_nk = -dot(&ukc, &sp[b].surf_pos) * inv[b] * inv_dkc;
                            let careful = c2 * nk_dot_nc + ukc_dot_nk * ukc_dot_nc;
                            if f_ik > f_eps {
                                let mut c3term = 0.0;
                                if c3.abs() > 1e-6 {
                                    let divisor = 1.0 + ukc_dot_nk;
                                    if divisor > 1e-6 {
                                        c3term = -c3 * (nk_dot_nc + ukc_dot_nc) / divisor;
                                    }
                                }
                                delta[a] += c1 * f_ik * (careful + c3term) * inv_dkc;
                            }
                            if f_ij > f_eps {
                                let mut c3term = 0.0;
                                if c3.abs() > 1e-6 {
                                    let divisor = 1.0 - ukc_dot_nc;
                                    if divisor > 1e-6 {
                                        c3term = -c3 * (nk_dot_nc - ukc_dot_nk) / divisor;
                                    }
                                }
                                delta[b] += c1 * f_ij * (careful + c3term) * inv_dkc;
                            }
                        }
                    }
                }
            }
            // write back after the loops (upstream: avoids a race condition)
            let mut it = delta.into_iter();
            for e in &mut data.pair[i] {
                e.value = it.next().unwrap_or(0.0);
            }
            for wl in &mut data.walls {
                for e in &mut wl[i] {
                    e.value = it.next().unwrap_or(0.0);
                }
            }
        }
        data
    }
}

/// Per-system multicontact state: the fix, this step's [`McData`] and the
/// contact history the surface model writes (`[radij, radji, fn]` per pair,
/// `[surfPos, fn]` per primitive-wall contact), kept between steps.
#[derive(Debug, Clone, PartialEq)]
pub struct MulticontactState {
    /// The `fix multicontact/halfspace`.
    pub fix: HalfSpaceFix,
    /// Expansions computed by the last `pre_force`.
    pub data: McData,
    /// Pair history keyed by `(lower tag, higher tag)`, stored as
    /// `[rad of lower tag, rad of higher tag, fn]`.
    pub pair_hist: std::collections::HashMap<(usize, usize), [f64; 3]>,
    /// Primitive-wall history keyed by `(tag, wall)`.
    pub wall_hist: std::collections::HashMap<(usize, usize), [f64; 4]>,
    /// Local index of each particle (inverse of the atom-sort permutation).
    pub local_of: Vec<usize>,
}

impl MulticontactState {
    /// Fresh state with empty history.
    #[must_use]
    pub fn new(fix: HalfSpaceFix) -> Self {
        Self {
            fix,
            data: McData::default(),
            pair_hist: std::collections::HashMap::new(),
            wall_hist: std::collections::HashMap::new(),
            local_of: Vec::new(),
        }
    }

    /// History `[radij, radji, fn]` of the pair as listed `(tag_i, tag_j)`.
    #[must_use]
    pub fn pair_history(&self, tag_i: usize, tag_j: usize) -> [f64; 3] {
        let key = (tag_i.min(tag_j), tag_i.max(tag_j));
        let h = self.pair_hist.get(&key).copied().unwrap_or([0.0; 3]);
        if tag_i <= tag_j { h } else { [h[1], h[0], h[2]] }
    }

    /// Store `[radij, radji, fn]` of the pair as listed `(tag_i, tag_j)`.
    pub fn set_pair_history(&mut self, tag_i: usize, tag_j: usize, h: [f64; 3]) {
        let key = (tag_i.min(tag_j), tag_i.max(tag_j));
        let v = if tag_i <= tag_j { h } else { [h[1], h[0], h[2]] };
        self.pair_hist.insert(key, v);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view<'a>(x: &'a [[f64; 3]], tag: &'a [usize], typ: &'a [usize], r: &'a [f64], g: &'a [bool]) -> ParticleView<'a> {
        ParticleView { x, tag, typ, radius: r, in_group: g }
    }

    #[test]
    fn history_writers_match_upstream_layout() {
        assert_eq!(surfaces_intersect_history(false, [1.0, 2.0, 3.0], 0.5, 0.6), vec![0.5, 0.6]);
        assert_eq!(surfaces_intersect_history(true, [1.0, 2.0, 3.0], 0.5, 0.0), vec![-1.0, -2.0, -3.0]);
        assert_eq!(surfaces_close_history(false, [0.0; 3], 0.5, 0.6), vec![0.5, 0.6, 0.0]);
        // wall, |delta| = 2: delta * radi * (-1/2)
        assert_eq!(surfaces_close_history(true, [0.0, 0.0, 2.0], 0.5, 0.0)[2], -0.5);
        // zero-length delta: rinv = 0
        assert_eq!(surfaces_close_history(true, [0.0; 3], 0.5, 0.0)[2], 0.0);
        assert_eq!(end_surfaces_intersect_history([0.0, 0.0, -3.0], [0.0, 0.0, 1.0]), 3.0);
        assert_eq!(CONTACT_DISTANCE_FACTOR, 2.0);
    }

    #[test]
    fn surf_pos_direct_and_no_contact() {
        let xi = [0.0; 3];
        let xj = [1.0, 0.0, 0.0];
        let (a, b) = HalfSpaceFix::pair_surf_pos(xi, xj, [0.55, 0.55, 1.0], 1e-12);
        assert_eq!(a, [0.5, 0.0, 0.0]);
        assert_eq!(b, [-0.5, 0.0, 0.0]);
        let (a, b) = HalfSpaceFix::pair_surf_pos(xi, xj, [0.4, 0.3, 0.0], 1e-12);
        assert_eq!(a, [0.4, 0.0, 0.0]);
        assert_eq!(b, [-0.3, 0.0, 0.0]);
    }

    /// A lone contact has delta 0; two equal opposed loaded contacts displace
    /// each other equally; forces below f_eps contribute nothing.
    #[test]
    fn two_contacts_symmetric_and_threshold() {
        let fix = HalfSpaceFix::new(vec![1e7], vec![0.3]);
        let x = [[-1.0, 0.0, 0.0], [0.0; 3], [1.0, 0.0, 0.0]];
        let (tag, typ, rad, grp) = ([1, 2, 3], [0, 0, 0], [0.5; 3], [true; 3]);
        let h = |f| [0.5, 0.5, f];
        let mk = |f| {
            vec![
                vec![PairContact { j: 1, history: h(f) }],
                vec![PairContact { j: 2, history: h(f) }],
                vec![],
            ]
        };
        let d = fix.pre_force(view(&x, &tag, &typ, &rad, &grp), &mk(10.0), &[], &[]);
        assert_eq!(d.pair[0][0].value, 0.0);
        assert_eq!(d.pair[2][0].value, 0.0);
        assert_eq!(d.pair[1].len(), 2);
        let (a, b) = (d.pair[1][0].value, d.pair[1][1].value);
        assert!(a != 0.0 && (a - b).abs() <= 1e-12 * a.abs(), "{a} {b}");
        assert_eq!(d.pair_expansion(1, 1), a);
        assert_eq!(d.pair_expansion(1, 99), 0.0);
        let d0 = fix.pre_force(view(&x, &tag, &typ, &rad, &grp), &mk(0.0), &[], &[]);
        assert!(d0.pair[1].iter().all(|e| e.value == 0.0));
    }

    /// A primitive-wall contact with a negligible surface position is dropped;
    /// a mesh one is kept; the group mask excludes a particle.
    #[test]
    fn wall_store_and_group_mask() {
        let fix = HalfSpaceFix::new(vec![1e7], vec![0.3]);
        let x = [[0.0; 3], [5.0, 0.0, 0.0]];
        let walls = vec![vec![
            vec![WallContact { id: 1, history: [0.0, 0.0, 0.0, 1.0] }],
            vec![WallContact { id: 1, history: [0.0, 0.0, -0.5, 1.0] }],
        ]];
        let run = |mesh: bool, grp: [bool; 2]| {
            fix.pre_force(view(&x, &[1, 2], &[0, 0], &[0.5, 0.5], &grp), &[vec![], vec![]], &walls, &[mesh])
        };
        let d = run(false, [true, true]);
        assert!(d.walls[0][0].is_empty());
        assert_eq!(d.walls[0][1].len(), 1);
        assert_eq!(d.wall_expansion(0, 1, 1), 0.0);
        assert_eq!(run(true, [true, true]).walls[0][0].len(), 1);
        assert!(run(false, [true, false]).walls[0][1].is_empty());
    }

    /// Pair plus wall contact on one grain with orthogonal normals: delta
    /// against the C1/C2/C3 closed form evaluated by hand.
    #[test]
    fn pair_plus_wall_closed_form() {
        let (y, nu) = (1e7, 0.3);
        let fix = HalfSpaceFix::new(vec![y], vec![nu]);
        let x = [[0.0; 3], [1.0, 0.0, 0.0]];
        let f = 100.0;
        let pairs = vec![vec![PairContact { j: 1, history: [0.5, 0.5, f] }], vec![]];
        let walls = vec![vec![vec![WallContact { id: 1, history: [0.0, 0.0, -0.5, f] }], vec![]]];
        let d = fix.pre_force(view(&x, &[1, 2], &[0, 0], &[0.5; 2], &[true; 2]), &pairs, &walls, &[false]);
        // particle 0: a = pair (0.5,0,0), b = wall (0,0,-0.5); nk.nc = 0
        let c1 = -(1.0 + nu) / (2.0 * PI * y) * DEFAULT_GEOMETRIC_PREFACTOR;
        let c3 = 1.0 - 2.0 * nu;
        let inv_dkc = 1.0 / 0.5_f64.sqrt(); // ukc = (0.5, 0, 0.5)
        let ukc_dot_nc = -(0.5 * 0.5) * 2.0 * inv_dkc;
        let ukc_dot_nk = (0.5 * 0.5) * 2.0 * inv_dkc;
        let careful = ukc_dot_nk * ukc_dot_nc;
        let t_ik = -c3 * ukc_dot_nc / (1.0 + ukc_dot_nk);
        let t_ij = c3 * ukc_dot_nk / (1.0 - ukc_dot_nc);
        let want_a = c1 * f * (careful + t_ik) * inv_dkc;
        let want_b = c1 * f * (careful + t_ij) * inv_dkc;
        assert!((d.pair[0][0].value - want_a).abs() <= 1e-12 * want_a.abs());
        assert!((d.walls[0][0][0].value - want_b).abs() <= 1e-12 * want_b.abs());
        assert_eq!(d.pair[1][0].value, 0.0);
    }
}
