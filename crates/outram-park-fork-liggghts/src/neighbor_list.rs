// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// PORTED FROM UPSTREAM — provenance (see the crate NOTICE):
//   Upstream project : LIGGGHTS-PUBLIC (DCS Computing GmbH / JKU Linz)
//   Upstream files   : src/neighbor.cpp (`setup_bins`, `bin_atoms`,
//                      `coord2bin`, `bin_distance`, `check_distance`),
//                      src/neigh_stencil.cpp (`stencil_half_bin_3d_no_newton`),
//                      src/neigh_gran.cpp (`granular_bin_no_newton`),
//                      src/atom.cpp (`Atom::setup_sort_bins`, `Atom::sort`),
//                      src/verlet.cpp (`setup`, `run`: when each is called)
//   Upstream commit  : 3d5c00f20519e6bb6eb6756f51f1ad36564e649d (2024-06-07)
//   Upstream licence : "GNU Public License, version 2 or later" -> used here
//                      under the "or later" option as GPL-3.0. The LAMMPS
//                      originals are Copyright (2003) Sandia Corporation
//                      (GPL); the LIGGGHTS changes are Copyright 2012-
//                      DCS Computing GmbH, Linz and 2009-2012 JKU Linz.
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

//! # LIGGGHTS neighbour-list order (`neighbor bin`, `newton off`, `atom_modify sort`)
//!
//! **Why this module exists (gh:#1406 prio 2, gh:#1418).** A particle's net
//! contact force is a floating-point *sum* over its contacts, and addition is
//! not associative, so two codes that evaluate every contact bit-identically
//! still differ by 1–3 ulp per step unless they add the contacts **in the same
//! order**. LIGGGHTS' order is fixed by three things, all translated here:
//!
//! 1. **The local atom order**, which `Atom::sort` permutes into spatial-bin
//!    order at the start of every `run` and every `atom_modify sort` interval
//!    (default 1000 steps): bins of side `0.5 * cutneighmax` over the box, in
//!    `iz*nbiny*nbinx + iy*nbinx + ix` order, atoms ascending within a bin
//!    ([`atom_sort_permutation`]).
//! 2. **The half neighbour list**, built by `granular_bin_no_newton`: for each
//!    local atom `i` ascending, every atom `j > i` found by walking the bin
//!    stencil in `k, j, i` order and each bin's linked list in ascending
//!    local index ([`half_bin_no_newton`]).
//! 3. **The pair loop** (`pair_gran_base.h`), which visits `i` ascending then
//!    the list of `i` in order, adding the contact force to `f[i]` and
//!    subtracting it from `f[j]`. [`crate::granular_system::GranularSystem`]
//!    already accumulates in the order of a pair list, so feeding it this list
//!    reproduces upstream's sums.
//!
//! The list is **stale between rebuilds**, exactly as upstream's: it is rebuilt
//! only when an atom has moved more than half the skin since the last build
//! (`Neighbor::check_distance`, `neigh_modify delay 0 every 1 check yes`) or at
//! set-up, so a contact's position in its particle's sum is kept for as long as
//! upstream keeps it.
//!
//! ## What is, and is not, translated
//!
//! Translated: the `newton off`, orthogonal-box, `neighbor <skin> bin`,
//! single-processor path with no ghost atoms (every deck under
//! `reference-data/liggghts/` uses `newton off`, `boundary f f f`).
//! **Not** translated: `newton on` (`granular_bin_newton`), periodic or
//! shrink-wrapped boxes (ghost atoms, box-change `boxcheck`), triclinic
//! boxes, `neighbor multi`, `neigh_modify exclude`, and the rebuild triggers of
//! fixes that request one (`next_reneighbor`) other than a moving mesh's. A
//! system that needs those keeps the default [`NeighborOrder::CellGrid`].
//!
//! ## Why a NAMED option and not the default
//!
//! The order depends on inputs the library cannot know: the **box**
//! (`bboxlo/bboxhi`, which sets the bin boundaries and so who is a neighbour of
//! whom in the list order) and the **skin**. Defaulting them would silently
//! invent a deck. The default stays the deterministic cell-grid order — bit
//! reproducible run to run, what every other cross-code row was measured with,
//! and independent of the box. The cost of the option is one `O(N)` binning and
//! stencil walk per rebuild plus a rebuild every `skin/2` of motion, measured
//! in `tests/pair_gran_local_cross_code.rs` output and in
//! `verification_and_validation/neighbor_order.md`.

use crate::particle::Vec3;
use crate::DemError;
use std::collections::HashMap;

/// How [`GranularSystem`](crate::granular_system::GranularSystem) orders the
/// particle pairs it sums contact forces over.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum NeighborOrder {
    /// The default: a linked-cell grid of cell side `2 r_max`, cells in
    /// ascending linear index, members ascending, rebuilt every step. Bit
    /// reproducible and box-independent; **not** LIGGGHTS' order, so a
    /// particle with three or more contacts can differ from upstream by 1–3 ulp
    /// per step.
    #[default]
    CellGrid,
    /// LIGGGHTS-PUBLIC's order: spatial atom sort, `bin` half neighbour list
    /// with `newton off`, stale between rebuilds. See the module docs.
    Liggghts(LiggghtsNeighbor),
}

/// The deck inputs that fix LIGGGHTS' neighbour order: `read_data`'s box,
/// `neighbor <skin> bin`, `atom_modify sort`, and the contact distance factor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LiggghtsNeighbor {
    /// Lower corner of the simulation box `[m]` (`domain->boxlo`).
    pub box_lo: Vec3,
    /// Upper corner of the simulation box `[m]` (`domain->boxhi`).
    pub box_hi: Vec3,
    /// `neighbor <skin> bin` `[m]`.
    pub skin: f64,
    /// `atom_modify sort <every> <binsize>`: re-sort the local atoms every this
    /// many steps, at a rebuild step. LIGGGHTS' default is 1000; `0` turns the
    /// sort off (`atom_modify sort 0 0`), leaving the data-file order.
    pub sort_every: u64,
    /// `neighbor->contactDistanceFactor` (default 1).
    pub contact_distance_factor: f64,
}

impl LiggghtsNeighbor {
    /// LIGGGHTS' defaults for a given box and skin: `sort_every = 1000`,
    /// `contact_distance_factor = 1`.
    ///
    /// # Errors
    ///
    /// [`DemError::InvalidInput`] if the skin is not finite and positive (a
    /// zero skin makes every step a rebuild step, which is a different list
    /// life than any deck here uses and is not what this option reproduces) or
    /// the box has a non-positive extent.
    pub fn new(box_lo: Vec3, box_hi: Vec3, skin: f64) -> Result<Self, DemError> {
        if !(skin.is_finite() && skin > 0.0) {
            return Err(DemError::InvalidInput(format!(
                "neighbour skin must be finite and positive, got {skin}"
            )));
        }
        let ext = box_hi.sub(box_lo);
        if !(ext.x > 0.0 && ext.y > 0.0 && ext.z > 0.0) {
            return Err(DemError::InvalidInput(
                "neighbour box must have a positive extent in x, y and z".into(),
            ));
        }
        Ok(Self {
            box_lo,
            box_hi,
            skin,
            sort_every: 1000,
            contact_distance_factor: 1.0,
        })
    }

    /// `cutneighmax`: the largest force cutoff plus skin
    /// (`Neighbor::init`: `cut = cutoff + skin`; for `pair gran` the cutoff
    /// of a type pair is the sum of the two types' largest radii times the
    /// contact distance factor, `PairGran::init_one`). `max_radius` is the
    /// largest radius present.
    #[must_use]
    pub fn cutneighmax(&self, max_radius: f64) -> f64 {
        let cutoff = (max_radius + max_radius) * self.contact_distance_factor;
        if cutoff > 0.0 {
            cutoff + self.skin
        } else {
            0.0
        }
    }
}

/// `static_cast<int>(double)`: truncation toward zero.
#[inline]
fn c_int(v: f64) -> i32 {
    v as i32
}

/// The permutation `Atom::sort` applies: element `n` is the old local index of
/// the atom that becomes local atom `n`. `x` are the local atoms' positions in
/// their current local order. Returns the identity when there is a single sort
/// bin (`if (nbins == 1) return;`).
///
/// Translated from `Atom::setup_sort_bins` + `Atom::sort` (the in-place
/// reordering by `copy()` realises exactly this permutation): bins of side
/// `binsize = 0.5 * cutneighmax` over the box, `nbin = int(extent / binsize)`
/// (at least 1), `bininv = nbin / extent`, atoms binned by
/// `int((x - lo) * bininv)` clamped to `[0, nbin - 1]`, ordered by
/// `iz*nbiny*nbinx + iy*nbinx + ix` and, inside a bin, by ascending old index.
#[must_use]
pub fn atom_sort_permutation(x: &[Vec3], cfg: &LiggghtsNeighbor, cutneighmax: f64) -> Vec<usize> {
    let n = x.len();
    let binsize = 0.5 * cutneighmax;
    let bininv = 1.0 / binsize;
    let ext = cfg.box_hi.sub(cfg.box_lo);
    let nb = |e: f64| c_int(e * bininv).max(1);
    let (nbinx, nbiny, nbinz) = (nb(ext.x), nb(ext.y), nb(ext.z));
    if (nbinx as i64) * (nbiny as i64) * (nbinz as i64) == 1 {
        return (0..n).collect();
    }
    let (bininvx, bininvy, bininvz) = (
        f64::from(nbinx) / ext.x,
        f64::from(nbiny) / ext.y,
        f64::from(nbinz) / ext.z,
    );
    let mut keyed: Vec<(i64, usize)> = x
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let ix = c_int((p.x - cfg.box_lo.x) * bininvx).max(0).min(nbinx - 1);
            let iy = c_int((p.y - cfg.box_lo.y) * bininvy).max(0).min(nbiny - 1);
            let iz = c_int((p.z - cfg.box_lo.z) * bininvz).max(0).min(nbinz - 1);
            let ibin = i64::from(iz) * i64::from(nbiny) * i64::from(nbinx)
                + i64::from(iy) * i64::from(nbinx)
                + i64::from(ix);
            (ibin, i)
        })
        .collect();
    // (bin, old index) ascending == the forward linked list `binhead` walks.
    keyed.sort_unstable();
    keyed.into_iter().map(|(_, i)| i).collect()
}

/// Whether the half list must be rebuilt: some local atom has moved more than
/// half the skin since `xhold` was stored (`Neighbor::check_distance`, fixed
/// box: `deltasq = triggersq = 0.25*skin*skin`).
#[must_use]
pub fn needs_rebuild(x: &[Vec3], xhold: &[Vec3], skin: f64) -> bool {
    let triggersq = 0.25 * skin * skin;
    let mut flag = false;
    for (p, h) in x.iter().zip(xhold) {
        let (dx, dy, dz) = (p.x - h.x, p.y - h.y, p.z - h.z);
        let rsq = dx * dx + dy * dy + dz * dz;
        if rsq > triggersq {
            flag = true;
        }
    }
    flag
}

/// The neighbour bins of `Neighbor::setup_bins` (orthogonal box, one
/// processor) and the `coord2bin` that maps a position onto them.
///
/// `binsize_optimal = 0.5 * cutneighmax` (the box's x extent when the cutoff
/// is zero), `nbin = int(extent / binsize_optimal)` (at least 1),
/// `binsize = extent / nbin`, `bininv = 1 / binsize`. Bin coordinates are
/// returned without the `mbin*lo` offsets, which only shift every index by
/// the same amount and so never change an order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BinGrid {
    /// `bboxlo`.
    pub lo: Vec3,
    /// `bboxhi`.
    pub hi: Vec3,
    /// `nbinx, nbiny, nbinz`.
    pub nbin: (i32, i32, i32),
    /// `binsizex, binsizey, binsizez` `[m]`.
    pub binsize: Vec3,
    /// `bininvx, bininvy, bininvz` `[1/m]`.
    pub bininv: Vec3,
}

impl BinGrid {
    /// `Neighbor::setup_bins` for the box of `cfg` and this `cutneighmax`.
    #[must_use]
    pub fn new(cfg: &LiggghtsNeighbor, cutneighmax: f64) -> Self {
        let (lo, hi) = (cfg.box_lo, cfg.box_hi);
        let bbox = hi.sub(lo);
        let binsize_optimal = if cutneighmax == 0.0 {
            bbox.x
        } else {
            0.5 * cutneighmax
        };
        let binsizeinv = 1.0 / binsize_optimal;
        let nb = |e: f64| c_int(e * binsizeinv).max(1);
        let (nbinx, nbiny, nbinz) = (nb(bbox.x), nb(bbox.y), nb(bbox.z));
        let binsize = Vec3::new(
            bbox.x / f64::from(nbinx),
            bbox.y / f64::from(nbiny),
            bbox.z / f64::from(nbinz),
        );
        let bininv = Vec3::new(1.0 / binsize.x, 1.0 / binsize.y, 1.0 / binsize.z);
        Self {
            lo,
            hi,
            nbin: (nbinx, nbiny, nbinz),
            binsize,
            bininv,
        }
    }

    /// `Neighbor::coord2bin(x, ix, iy, iz)` without the `mbin*lo` offsets.
    #[must_use]
    pub fn coord2bin(&self, p: Vec3) -> (i32, i32, i32) {
        let one = |v: f64, lo: f64, hi: f64, bininv: f64, nbin: i32| -> i32 {
            if v >= hi {
                c_int((v - hi) * bininv) + nbin
            } else if v >= lo {
                c_int((v - lo) * bininv).min(nbin - 1)
            } else {
                c_int((v - lo) * bininv) - 1
            }
        };
        (
            one(p.x, self.lo.x, self.hi.x, self.bininv.x, self.nbin.0),
            one(p.y, self.lo.y, self.hi.y, self.bininv.y, self.nbin.1),
            one(p.z, self.lo.z, self.hi.z, self.bininv.z, self.nbin.2),
        )
    }
}

/// Build `granular_bin_no_newton`'s half list for the local atoms `x` with
/// radii `radius` (both in local order), appending `(i, j)` **local-index**
/// pairs with `i < j` to `pairs` in upstream's order: `i` ascending, then the
/// bins of `i`'s stencil in `k, j, i` order, then each bin in ascending local
/// index. A pair is stored when `rsq <= ((r_i + r_j) cdf + skin)^2`.
///
/// Translated from `Neighbor::setup_bins` (bin geometry and stencil extent),
/// `stencil_half_bin_3d_no_newton`, `Neighbor::bin_atoms`, `coord2bin`
/// (without the `mbin*lo` offsets, which only shift indices) and
/// `Neighbor::granular_bin_no_newton`.
pub fn half_bin_no_newton(
    x: &[Vec3],
    radius: &[f64],
    cfg: &LiggghtsNeighbor,
    cutneighmax: f64,
    pairs: &mut Vec<(u32, u32)>,
) {
    pairs.clear();
    let n = x.len();

    // --- setup_bins ---
    let grid = BinGrid::new(cfg, cutneighmax);
    let (binsizex, binsizey, binsizez) = (grid.binsize.x, grid.binsize.y, grid.binsize.z);
    let (bininvx, bininvy, bininvz) = (grid.bininv.x, grid.bininv.y, grid.bininv.z);
    let stencil_extent = |bininv: f64, binsize: f64| -> i32 {
        let mut s = c_int(cutneighmax * bininv);
        if f64::from(s) * binsize < cutneighmax {
            s += 1;
        }
        s
    };
    let (sx, sy, sz) = (
        stencil_extent(bininvx, binsizex),
        stencil_extent(bininvy, binsizey),
        stencil_extent(bininvz, binsizez),
    );

    // --- stencil_half_bin_3d_no_newton ---
    let cutneighmaxsq = cutneighmax * cutneighmax;
    let bin_distance = |i: i32, j: i32, k: i32| -> f64 {
        let d = |a: i32, binsize: f64| -> f64 {
            if a > 0 {
                f64::from(a - 1) * binsize
            } else if a == 0 {
                0.0
            } else {
                f64::from(a + 1) * binsize
            }
        };
        let (delx, dely, delz) = (d(i, binsizex), d(j, binsizey), d(k, binsizez));
        delx * delx + dely * dely + delz * delz
    };
    let mut stencil: Vec<(i32, i32, i32)> = Vec::new();
    for k in -sz..=sz {
        for j in -sy..=sy {
            for i in -sx..=sx {
                if bin_distance(i, j, k) < cutneighmaxsq {
                    stencil.push((i, j, k));
                }
            }
        }
    }

    // --- bin_atoms: `bins` linked list in forward (ascending) order ---
    let mut bins: HashMap<(i32, i32, i32), Vec<u32>> = HashMap::new();
    let mut ibins: Vec<(i32, i32, i32)> = Vec::with_capacity(n);
    for (i, p) in x.iter().enumerate() {
        let b = grid.coord2bin(*p);
        ibins.push(b);
        bins.entry(b).or_default().push(i as u32);
    }

    // --- granular_bin_no_newton ---
    let skin = cfg.skin;
    for i in 0..n {
        let (xtmp, ytmp, ztmp) = (x[i].x, x[i].y, x[i].z);
        let radi = radius[i];
        let (bx, by, bz) = ibins[i];
        for &(dx, dy, dz) in &stencil {
            let Some(members) = bins.get(&(bx + dx, by + dy, bz + dz)) else {
                continue;
            };
            for &j in members {
                let j = j as usize;
                if j <= i {
                    continue;
                }
                let delx = xtmp - x[j].x;
                let dely = ytmp - x[j].y;
                let delz = ztmp - x[j].z;
                let rsq = delx * delx + dely * dely + delz * delz;
                let radsum = (radi + radius[j]) * cfg.contact_distance_factor;
                let cutsq = (radsum + skin) * (radsum + skin);
                if rsq <= cutsq {
                    pairs.push((i as u32, j as u32));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> LiggghtsNeighbor {
        LiggghtsNeighbor::new(Vec3::new(-1.0, -1.0, 0.0), Vec3::new(1.0, 1.0, 1.0), 0.003).unwrap()
    }

    /// The list holds exactly the pairs inside `radsum + skin`, once each, with
    /// `i < j`, and every `i` block is ascending in `i` — checked against a
    /// brute-force enumeration (the set, not the order).
    #[test]
    fn half_list_is_the_brute_force_pair_set() {
        let c = cfg();
        let r = 0.005;
        let mut x = Vec::new();
        for a in 0..7 {
            for b in 0..7 {
                for k in 0..3 {
                    // slightly jittered lattice, spacing just above a diameter
                    let j = ((a * 7 + b * 3 + k) % 5) as f64 * 1e-4;
                    x.push(Vec3::new(
                        0.0101 * a as f64 + j,
                        0.0102 * b as f64 - j,
                        0.0103 * k as f64 + 0.1,
                    ));
                }
            }
        }
        let radius = vec![r; x.len()];
        let cut = c.cutneighmax(r);
        let mut pairs = Vec::new();
        half_bin_no_newton(&x, &radius, &c, cut, &mut pairs);
        let mut want = Vec::new();
        for i in 0..x.len() {
            for j in i + 1..x.len() {
                let d = x[i].sub(x[j]);
                let rsq = d.x * d.x + d.y * d.y + d.z * d.z;
                let rs = (r + r) * 1.0 + c.skin;
                if rsq <= rs * rs {
                    want.push((i as u32, j as u32));
                }
            }
        }
        assert!(!want.is_empty());
        let mut got = pairs.clone();
        got.sort_unstable();
        assert_eq!(got, want);
        let firsts: Vec<u32> = pairs.iter().map(|p| p.0).collect();
        assert!(firsts.windows(2).all(|w| w[0] <= w[1]), "i must ascend");
    }

    /// The sort permutation is a bijection and orders atoms by bin.
    #[test]
    fn sort_is_a_bin_ordered_permutation() {
        let c = cfg();
        let x: Vec<Vec3> = (0..50)
            .map(|i| {
                Vec3::new(
                    ((i * 37) % 200) as f64 / 100.0 - 1.0,
                    ((i * 53) % 200) as f64 / 100.0 - 1.0,
                    ((i * 11) % 100) as f64 / 100.0,
                )
            })
            .collect();
        let perm = atom_sort_permutation(&x, &c, c.cutneighmax(0.005));
        let mut seen = perm.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..50).collect::<Vec<_>>());
        assert_ne!(
            perm,
            (0..50).collect::<Vec<_>>(),
            "scattered input must reorder"
        );
    }

    /// A single-bin box leaves the order alone (`if (nbins == 1) return;`).
    #[test]
    fn single_bin_box_is_identity() {
        let c = LiggghtsNeighbor::new(Vec3::zero(), Vec3::new(0.01, 0.01, 0.01), 0.003).unwrap();
        let x = vec![Vec3::new(0.001, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0)];
        assert_eq!(
            atom_sort_permutation(&x, &c, c.cutneighmax(0.005)),
            vec![0, 1]
        );
    }

    /// `coord2bin`: owned positions land in `[0, nbin)`, the upper face in the
    /// last bin, and points outside the box below 0 or at/after `nbin`.
    #[test]
    fn coord2bin_matches_setup_bins() {
        let c = cfg();
        let cut = c.cutneighmax(0.005);
        let g = BinGrid::new(&c, cut);
        // binsize_optimal = 0.5 * 0.013 = 0.0065: int(2/0.0065) = 307 in x
        assert_eq!(g.nbin, (307, 307, 153));
        assert_eq!(g.coord2bin(Vec3::new(-1.0, -1.0, 0.0)), (0, 0, 0));
        assert_eq!(g.coord2bin(Vec3::new(1.0 - 1e-15, 0.0, 0.5)).0, 306);
        assert_eq!(g.coord2bin(Vec3::new(-1.001, 1.0, 1.0)), (-1, 307, 153));
    }

    #[test]
    fn rebuild_trigger_is_half_skin() {
        let h = vec![Vec3::zero()];
        assert!(!needs_rebuild(&[Vec3::new(0.0014, 0.0, 0.0)], &h, 0.003));
        assert!(needs_rebuild(&[Vec3::new(0.0016, 0.0, 0.0)], &h, 0.003));
    }

    #[test]
    fn invalid_inputs_are_rejected() {
        assert!(LiggghtsNeighbor::new(Vec3::zero(), Vec3::new(1.0, 1.0, 1.0), 0.0).is_err());
        assert!(LiggghtsNeighbor::new(Vec3::zero(), Vec3::new(1.0, 0.0, 1.0), 0.003).is_err());
    }
}
