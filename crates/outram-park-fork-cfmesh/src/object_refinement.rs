// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// Ported from cfMesh (Creative Fields, Ltd.; Franjo Juretic), GPL-3.0:
//   meshLibrary/utilities/octrees/meshOctree/refinementControls/
//     objectRefinement/{objectRefinement,boxRefinement,sphereRefinement,
//       coneRefinement,hollowConeRefinement,lineRefinement}.{C,H}
//       (calculateAdditionalRefLevels, intersectsObject)
//     patchRefinement/patchRefinement.{C,H} (as
//       crate::mesh_octree_creator::PatchRefinement)
//   meshLibrary/utilities/octrees/meshOctree/meshOctreeCreator/
//     meshOctreeCreatorAdjustOctreeToSurface.C (refineBoxesContainedInObjects)
// Source: the cfMesh v1.1 line as carried by github.com/DAFoam/cfmesh, commit
// e944f6ba695fcefce73ad306074f6b3c0ea432ae.
// Copyright (C) Creative Fields, Ltd. Licence: GPL-3.0-or-later (cfMesh files
// say "either version 3 of the License, or (at your option) any later
// version"), conveyed here under GPL-3.0-only.
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

//! cfMesh's **object refinement** (gh:#1265): refine every octree leaf that
//! meets a box, sphere, cone, hollow cone or line to a requested cell size
//! or number of extra levels, with an optional band thickness.
//!
//! Upstream's `objectRefinement` is an abstract class with five derived
//! types chosen at run time; here [`ObjectShape`] is an enum (the crate's
//! design rule) and [`ObjectRefinement`] carries the common fields.
//!
//! What each shape tests is upstream's choice, kept as is:
//! - **box**: the leaf's box **overlaps** the object box;
//! - **sphere, cone, hollow cone**: the leaf's **centre** is inside;
//! - **line**: the segment crosses a face of the leaf (inflated by
//!   `SMALL x` the leaf size) or starts inside it.
//!
//! Patch refinement (`patchCellSize`, `localRefinement`) is in
//! [`crate::mesh_octree_creator`], where upstream applies it
//! (`setRootCubeSizeAndRefParameters`); the tests are here.

use crate::math::Vec3;
use crate::mesh_octree::{mag_sqr, BoundBox, CubeType, MeshOctree, SMALL, VSMALL};

/// The geometry of a refinement object.
#[derive(Debug, Clone, PartialEq)]
pub enum ObjectShape {
    /// `boxRefinement`: centre and edge lengths.
    Box {
        /// `centre`.
        centre: Vec3,
        /// `lengthX`, `lengthY`, `lengthZ`.
        lengths: Vec3,
    },
    /// `sphereRefinement`.
    Sphere {
        /// `centre`.
        centre: Vec3,
        /// `radius`.
        radius: f64,
    },
    /// `coneRefinement`: a truncated cone from `p0` (radius `r0`) to `p1`
    /// (radius `r1`).
    Cone {
        /// `p0`.
        p0: Vec3,
        /// `radius0`.
        r0: f64,
        /// `p1`.
        p1: Vec3,
        /// `radius1`.
        r1: f64,
    },
    /// `hollowConeRefinement`: between an inner and an outer cone.
    HollowCone {
        /// `p0`.
        p0: Vec3,
        /// `radius0_Outer`.
        r0_outer: f64,
        /// `radius0_Inner`.
        r0_inner: f64,
        /// `p1`.
        p1: Vec3,
        /// `radius1_Outer`.
        r1_outer: f64,
        /// `radius1_Inner`.
        r1_inner: f64,
    },
    /// `lineRefinement`: the segment `p0`–`p1`.
    Line {
        /// `p0`.
        p0: Vec3,
        /// `p1`.
        p1: Vec3,
    },
}

/// `objectRefinement`: a shape with its refinement request.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectRefinement {
    /// `name_`.
    pub name: String,
    /// `cellSize` (negative when not given, as upstream's `-1`).
    pub cell_size: f64,
    /// `additionalRefinementLevels`.
    pub additional_ref_levels: u8,
    /// `refinementThickness`.
    pub refinement_thickness: f64,
    /// The geometry.
    pub shape: ObjectShape,
}

impl ObjectRefinement {
    /// An object refined to `cell_size`.
    pub fn with_cell_size(name: &str, shape: ObjectShape, cell_size: f64) -> Self {
        ObjectRefinement { name: name.into(), cell_size, additional_ref_levels: 0, refinement_thickness: 0.0, shape }
    }

    /// An object refined by `levels` extra levels.
    pub fn with_levels(name: &str, shape: ObjectShape, levels: u8) -> Self {
        ObjectRefinement { name: name.into(), cell_size: -1.0, additional_ref_levels: levels, refinement_thickness: 0.0, shape }
    }

    /// `calculateAdditionalRefLevels(globalCellSize)`: the number of halvings
    /// of the global size that are still `>= cellSize` (to `1 + SMALL`); a
    /// no-op when the levels are given directly or no size is.
    pub fn calculate_additional_ref_levels(&mut self, global_cell_size: f64) {
        if self.cell_size < 0.0 || self.additional_ref_levels != 0 {
            return;
        }
        let mut s = global_cell_size;
        while self.cell_size <= s * (1.0 + SMALL) {
            self.additional_ref_levels += 1;
            s /= 2.0;
        }
    }

    /// `intersectsObject(bb)` (module docs).
    pub fn intersects_object(&self, bb: &BoundBox) -> bool {
        let c = bb.max.add(bb.min).scale(0.5);
        match &self.shape {
            ObjectShape::Box { centre, lengths } => {
                let v = lengths.scale(0.5);
                BoundBox::new(centre.sub(v), centre.add(v)).overlaps(bb)
            }
            ObjectShape::Sphere { centre, radius } => mag_sqr(c.sub(*centre)) < radius * radius,
            ObjectShape::Cone { p0, r0, p1, r1 } => {
                let v = p1.sub(*p0);
                let d = mag_sqr(v);
                if d < VSMALL {
                    return false;
                }
                let t = c.sub(*p0).dot(v) / d;
                if !(0.0..=1.0).contains(&t) {
                    return false;
                }
                let r = r0 + (r1 - r0) * t;
                p0.add(v.scale(t)).sub(c).length() < r
            }
            ObjectShape::HollowCone { p0, r0_outer, r0_inner, p1, r1_outer, r1_inner } => {
                let v = p1.sub(*p0);
                let d = mag_sqr(v);
                if d < VSMALL {
                    return false;
                }
                let t = c.sub(*p0).dot(v) / d;
                if !(0.0..=1.0).contains(&t) {
                    return false;
                }
                let ro = r0_outer + (r1_outer - r0_outer) * t;
                let ri = r0_inner + (r1_inner - r0_inner) * t;
                let m = p0.add(v.scale(t)).sub(c).length();
                m < ro && m > ri
            }
            ObjectShape::Line { p0, p1 } => line_intersects_box(*p0, *p1, bb),
        }
    }
}

/// `lineRefinement::intersectsObject`, transcribed (upstream's per-axis
/// early exits and open-interval face tests included).
fn line_intersects_box(p0: Vec3, p1: Vec3, bb: &BoundBox) -> bool {
    let l = bb.max.x - bb.min.x;
    let g = Vec3::new(SMALL, SMALL, SMALL).scale(l);
    let (mn, mx) = (bb.min.sub(g), bb.max.add(g));
    let v = p1.sub(p0);
    let comp = crate::mesh_octree::comp;
    for dir in 0..3 {
        if comp(v, dir).abs() <= SMALL {
            continue;
        }
        let (a, b) = (comp(p0, dir), comp(p1, dir));
        if (a < comp(mn, dir) && b < comp(mn, dir)) || (a > comp(mx, dir) && b > comp(mx, dir)) {
            return false;
        }
        let (o1, o2) = ((dir + 1) % 3, (dir + 2) % 3);
        let (o1, o2) = (o1.min(o2), o1.max(o2));
        for (plane, sign) in [(mn, -1.0), (mx, 1.0)] {
            // n & (plane - p0) / (n & v), with n = sign * e_dir.
            let t = (sign * (comp(plane, dir) - a)) / (sign * comp(v, dir));
            let i = p0.add(v.scale(t));
            if t > -SMALL
                && t < 1.0 + SMALL
                && comp(i, o1) > comp(mn, o1)
                && comp(i, o1) < comp(mx, o1)
                && comp(i, o2) > comp(mn, o2)
                && comp(i, o2) < comp(mx, o2)
            {
                return true;
            }
        }
    }
    p0.x > mn.x && p0.x < mx.x && p0.y > mn.y && p0.y < mx.y && p0.z > mn.z && p0.z < mx.z
}

/// `meshOctreeCreator::refineBoxesContainedInObjects`: refine every
/// non-`OUTSIDE` leaf meeting an object below that object's level
/// (`global_ref_level` plus its extra levels, from `max_cell_size`), with
/// layers where a thickness is set, until nothing changes; then re-mark
/// inside/outside.
pub fn refine_boxes_contained_in_objects(
    o: &mut MeshOctree,
    objects: &[ObjectRefinement],
    max_cell_size: f64,
    global_ref_level: u8,
    hex_refinement: bool,
) {
    let mut objs = objects.to_vec();
    let mut ref_levels = vec![global_ref_level; objs.len()];
    for (i, ob) in objs.iter_mut().enumerate() {
        ob.calculate_additional_ref_levels(max_cell_size);
        ref_levels[i] += ob.additional_ref_levels;
    }
    loop {
        let n = o.number_of_leaves();
        let mut refine = vec![0i32; n];
        let mut thick = vec![0.0f64; n];
        let mut use_n_layers = false;
        let mut changed = false;
        for l in 0..n {
            let oc = o.return_leaf(l);
            if oc.cube_type & CubeType::OUTSIDE != 0 {
                continue;
            }
            let bb = oc.coords.cube_box(&o.root_box);
            let mut want = false;
            for (i, ob) in objs.iter().enumerate() {
                if ob.intersects_object(&bb) {
                    if oc.coords.level < ref_levels[i] {
                        want = true;
                    }
                    if ob.refinement_thickness > VSMALL {
                        thick[l] = thick[l].max(ob.refinement_thickness);
                        use_n_layers = true;
                    }
                }
            }
            if want {
                refine[l] = 1;
                changed = true;
            }
        }
        if use_n_layers && changed {
            o.refine_selected_boxes_and_additional_layers(&refine, &thick);
        } else if changed {
            o.refine_selected_boxes(&mut refine, hex_refinement);
        }
        if !changed {
            break;
        }
    }
    crate::mesh_octree_creator::mesh_octree_inside_outside(o);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh_octree::TriSurf;
    use crate::mesh_octree_creator::tests::create;
    use crate::mesh_octree_creator::{LocalRefinement, MeshDict, PatchRefinement};
    use std::sync::Arc;

    /// The cube `[0, 8]^3`, one region per side (`-Z, +Z, -Y, +Y, +X, -X`).
    fn cube8() -> Arc<TriSurf> {
        let (p, t) = crate::shapes::box_surface(Vec3::ZERO, Vec3::new(8.0, 8.0, 8.0));
        Arc::new(TriSurf::new(p, t, Some((0..12).map(|i| i / 2).collect())))
    }

    fn max_jump(o: &MeshOctree) -> i32 {
        let mut j = 0;
        for l in 0..o.number_of_leaves() {
            let cc = o.return_leaf(l).coords;
            for m in o.find_all_leaf_neighbours(&cc).into_iter().flatten() {
                j = j.max((o.return_leaf(m).coords.level as i32 - cc.level as i32).abs());
            }
        }
        j
    }

    /// V&V (gh:#1265): box, sphere, cone, hollow-cone and line sources on a
    /// cube domain, checked analytically.
    ///
    /// # Methodology
    ///
    /// Domain: the cube `[0, 8]^3` with `maxCellSize` 1 (root side 16,
    /// global level 4), no boundary refinement. One object at a time, each
    /// asking for cell size 0.25 (2 extra levels, level 6), placed well
    /// inside the cube. For every leaf:
    /// 1. if the object meets the leaf (the shape's own test), the leaf is
    ///    at level >= 6;
    /// 2. every leaf at level 6 has a **parent** the object meets (a
    ///    level-6 leaf can only come from refining a selected level-5 cube;
    ///    the regularity rule only refines cubes coarser than a selected
    ///    one, so it creates at most level-5 leaves);
    /// 3. the octree stays 1-irregular.
    ///
    /// **Prediction (written before the run):** all three hold for all five
    /// shapes; no level above 6 appears. `calculateAdditionalRefLevels(1)`
    /// for 0.25 gives 3 halvings counted (1, 0.5, 0.25 are all >= 0.25),
    /// i.e. **3** extra levels, not 2. Re-reading the loop: it increments
    /// while `cellSize <= s`, starting from `s = 1`: 0.25 <= 1, 0.25 <= 0.5,
    /// 0.25 <= 0.25, so 3; the target is level 7 (cell 1/8 of the root
    /// side 16 / 2^7 = 0.125). Prediction corrected **before** running:
    /// target level 7, cell size 0.125, half the requested 0.25. That is
    /// upstream's arithmetic and is recorded as such.
    ///
    /// # Results (2026-11-05)
    ///
    /// Target level 7 for all five shapes; the three properties hold.
    ///
    /// **A miss on the first run, kept as a finding:** the hollow cone was
    /// first a shell between radii 0.8 and 1.5 about `y = z = 4`. The
    /// level-4 leaf centres sit at half-integers, so their distances from
    /// that axis are 0.71 and 1.58: none falls in the shell, no leaf is
    /// selected, and the object refines **nothing**. That is upstream's
    /// centre test working as written: a sphere, cone or shell thinner than
    /// the background spacing can be missed entirely.
    /// [`hollow_cone_thinner_than_the_background_spacing_is_missed`] pins
    /// it. The shell here was widened to radii 0.3–1.7, which the centres
    /// sample.
    #[test]
    fn object_sources_refine_exactly_the_leaves_they_meet() {
        let c = Vec3::new(4.0, 4.0, 4.0);
        let shapes = [
            ObjectShape::Box { centre: c, lengths: Vec3::new(2.0, 1.0, 1.5) },
            ObjectShape::Sphere { centre: c, radius: 1.3 },
            ObjectShape::Cone { p0: Vec3::new(4.0, 4.0, 2.5), r0: 1.0, p1: Vec3::new(4.0, 4.0, 5.5), r1: 0.3 },
            ObjectShape::HollowCone {
                p0: Vec3::new(2.5, 4.0, 4.0),
                r0_outer: 1.7,
                r0_inner: 0.3,
                p1: Vec3::new(5.5, 4.0, 4.0),
                r1_outer: 1.7,
                r1_inner: 0.3,
            },
            ObjectShape::Line { p0: Vec3::new(2.2, 3.1, 2.7), p1: Vec3::new(5.9, 4.6, 5.3) },
        ];
        let mut ob = ObjectRefinement::with_cell_size("x", shapes[0].clone(), 0.25);
        ob.calculate_additional_ref_levels(1.0);
        assert_eq!(ob.additional_ref_levels, 3, "upstream's halving count");
        for shape in shapes {
            let obj = ObjectRefinement::with_cell_size("obj", shape.clone(), 0.25);
            let dict = MeshDict { object_refinements: vec![obj.clone()], ..MeshDict::with_max_cell_size(1.0) };
            let (o, cr) = create(cube8(), dict);
            assert_eq!(cr.global_ref_level, 4);
            let target = 7u8;
            let mut n_target = 0;
            for l in 0..o.number_of_leaves() {
                let lf = o.return_leaf(l);
                if lf.cube_type & CubeType::OUTSIDE != 0 {
                    continue;
                }
                let bb = lf.coords.cube_box(&o.root_box);
                if obj.intersects_object(&bb) {
                    assert!(lf.coords.level >= target, "{shape:?}: leaf {l} meets the object at level {}", lf.coords.level);
                }
                assert!(lf.coords.level <= target);
                if lf.coords.level == target {
                    n_target += 1;
                    let parent = lf.coords.reduce_level_by(1).cube_box(&o.root_box);
                    assert!(obj.intersects_object(&parent), "{shape:?}: level-7 leaf {l} without a selected parent");
                }
            }
            assert!(n_target > 0, "{shape:?}");
            assert!(max_jump(&o) <= 1, "{shape:?}");
            println!("{:?}: {} leaves, {n_target} at level 7", std::mem::discriminant(&shape), o.number_of_leaves());
        }
    }

    /// V&V (gh:#1265): upstream's centre test misses a shell that lies
    /// between the background cell centres (see the finding in
    /// [`object_sources_refine_exactly_the_leaves_they_meet`]): the shell
    /// 0.8–1.5 about the axis `y = z = 4` on the 1 m lattice of `[0, 8]^3`
    /// refines no leaf (the octree equals the one built without it).
    #[test]
    fn hollow_cone_thinner_than_the_background_spacing_is_missed() {
        let shell = ObjectShape::HollowCone {
            p0: Vec3::new(2.5, 4.0, 4.0),
            r0_outer: 1.5,
            r0_inner: 0.8,
            p1: Vec3::new(5.5, 4.0, 4.0),
            r1_outer: 1.5,
            r1_inner: 0.8,
        };
        let with = MeshDict {
            object_refinements: vec![ObjectRefinement::with_cell_size("shell", shell, 0.25)],
            ..MeshDict::with_max_cell_size(1.0)
        };
        let (a, _) = create(cube8(), with);
        let (b, _) = create(cube8(), MeshDict::with_max_cell_size(1.0));
        let levels = |o: &MeshOctree| (0..o.number_of_leaves()).map(|l| o.return_leaf(l).coords).collect::<Vec<_>>();
        assert_eq!(levels(&a), levels(&b));
    }

    /// V&V (gh:#1265): `patchCellSize` and `localRefinement` on the cube
    /// `[0, 8]^3` (`maxCellSize` 1, `boundaryCellSize` 0.5, so the boundary
    /// level is 5): region 0 (`-Z`) asks for cell size 0.25 by
    /// `patchCellSize` (level 6), region 4 (`+X`) for 2 extra levels by
    /// `localRefinement` (level 6). **Prediction:** every leaf holding a
    /// triangle of region 0 or 4 is at level 6, every other triangle leaf at
    /// level 5, and a `refinementThickness` of 1.0 on region 4 refines at
    /// least one non-triangle leaf whose centre is within 1.0 of `x = 8` to
    /// level 6. The `levels_to` rule for `patchCellSize` counts halvings
    /// until `max / 2^n <= cs`: 1 -> 0.5 -> 0.25 is 2, level 4 + 2 = 6.
    ///
    /// **Result (2026-11-05): the "every other triangle leaf at level 5"
    /// part missed.** Upstream's `refineBoxesNearDataBoxes` refines every
    /// coarser non-`OUTSIDE` 26-neighbour of a leaf holding triangles,
    /// including other triangle leaves, so `-Y` / `-X` leaves next to the
    /// level-6 `-Z` leaves are lifted to level 6 as well. The assertion now
    /// is: region 0 and 4 leaves at 6, every other triangle leaf at 5 or 6,
    /// with the number lifted to 6 pinned ([`PATCH_PINS`]). A first
    /// replacement assertion ("lifted only if its parent touches a region 0
    /// or 4 leaf") also failed: the lifting chains. `refineBoundary` splits
    /// a level-5 corner leaf holding `-Z` triangles; its upper children hold
    /// only `-X` / `-Y` triangles but are level-6 leaves with triangles, and
    /// `refineBoxesNearDataBoxes` lifts *their* coarser neighbours in turn
    /// (seen at the leaf `(6; 15, 15, 18)`, box `z` 0.5–0.75).
    const PATCH_PINS: (usize, usize) = (2776, 1092);

    #[test]
    fn patch_and_local_refinement_reach_their_levels() {
        let dict = MeshDict {
            boundary_cell_size: Some(0.5),
            patch_cell_size: vec![PatchRefinement { region: 0, cell_size: 0.25 }],
            local_refinement: vec![LocalRefinement { region: 4, additional_levels: Some(2), cell_size: None, refinement_thickness: 0.0 }],
            ..MeshDict::with_max_cell_size(1.0)
        };
        let (o, _) = create(cube8(), dict.clone());
        let (mut n_lifted, mut n_other) = (0, 0);
        for l in 0..o.number_of_leaves() {
            let regs = o.find_boundary_patches_for_leaf(l);
            if regs.is_empty() {
                continue;
            }
            let lv = o.return_leaf(l).coords.level;
            if regs.contains(&0) || regs.contains(&4) {
                assert_eq!(lv, 6, "leaf {l} regions {regs:?}");
            } else {
                assert!(lv == 5 || lv == 6, "leaf {l} regions {regs:?} level {lv}");
                n_other += 1;
                if lv == 6 {
                    n_lifted += 1;
                }
            }
        }
        println!("other-region triangle leaves {n_other}, lifted to level 6: {n_lifted}");
        assert_eq!((n_other, n_lifted), PATCH_PINS);
        let mut d2 = dict;
        d2.local_refinement[0].refinement_thickness = 1.0;
        let (o2, _) = create(cube8(), d2);
        let thick = (0..o2.number_of_leaves()).any(|l| {
            let lf = o2.return_leaf(l);
            let c = lf.coords.centre(&o2.root_box);
            !lf.has_contained_elements() && lf.coords.level == 6 && (8.0 - c.x).abs() < 1.0 && c.z > 2.0
        });
        assert!(thick, "refinementThickness grows a band inside the +X side");
        assert!(max_jump(&o2) <= 1);
    }
}
