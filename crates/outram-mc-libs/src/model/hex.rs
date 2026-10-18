//! **A hexagonal prism, and the plane geometry that goes with it**
//! (tutorial 7 on #943, epic #925).
//!
//! The tile of a hexagonal lattice, and the container that holds one. Its
//! faces are six planes and two z-planes; the hexagon is named by which way
//! its faces point ([`HexFaces`]), because the lower level's `HexOrientation`
//! names (`Y`, `X`) were once documented backwards (corrected 2026-09-25 in
//! `outram_blender::csg::lattice::hex`).
//!
//! The helpers at the bottom are the 2-D convex-polygon tests the solid
//! relations ([`super::Solid::encloses`], [`super::Solid::overlaps`]) use for
//! a prism against a sphere, a box, a cylinder or another prism. They are
//! exact (no sampling).

use outram_blender::csg::lattice::HexOrientation;
use uom::si::f64::Length;
use uom::si::length::centimeter;

use super::material::Material;
use super::shape::{Shape, Solid};

/// **Which way the faces of a hexagon point.** Named after the axis the
/// faces are *perpendicular* to, which is OpenMC's own wording
/// (`openmc/lattice.py`, `HexLattice.orientation`: "each lattice element has
/// two faces that are perpendicular to the y-axis" for the `y`
/// orientation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexFaces {
    /// Two faces perpendicular to y (flat top and bottom, vertices pointing
    /// along +-x). Neighbour centres at `(0, +-pitch)` and
    /// `(+-sqrt(3)/2, +-1/2) pitch`. OpenMC's default, and Şeker & Çolak's
    /// cell.
    NormalToY,
    /// Two faces perpendicular to x. Neighbour centres at `(+-pitch, 0)`.
    NormalToX,
}

impl HexFaces {
    /// The lower level's name for the same orientation.
    pub(crate) fn orientation(self) -> HexOrientation {
        match self {
            HexFaces::NormalToY => HexOrientation::Y,
            HexFaces::NormalToX => HexOrientation::X,
        }
    }
}

/// `sqrt(3) / 2`.
const S: f64 = 0.866_025_403_784_438_6;

/// **A hexagonal prism** (z axis along its length): a flat-to-flat pitch, a
/// height, which way its faces point and, optionally, a centre (the origin
/// otherwise). The tile of [`super::Lattice::hex`] and the container that
/// holds it.
///
/// ```
/// use outram_mc_libs::model::prelude::*;
/// use uom::si::{f64::Length, length::centimeter};
/// let cm = Length::new::<centimeter>;
/// let tile = HexPrism::flat_to_flat(cm(16.392), cm(9.798));
/// assert!((tile.apothem_cm() - 8.196).abs() < 1e-12);
/// assert!((tile.volume_cm3() - 3.0_f64.sqrt() / 2.0 * 16.392 * 16.392 * 9.798).abs() < 1e-9);
/// assert_eq!(tile.faces_kind(), HexFaces::NormalToY);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HexPrism {
    centre_cm: [f64; 3],
    pitch_cm: f64,
    height_cm: f64,
    faces: HexFaces,
}

impl HexPrism {
    /// A prism of flat-to-flat distance `pitch` and length `height`, faces
    /// perpendicular to y ([`HexFaces::NormalToY`]), centred at the origin.
    /// Both must be positive and finite; [`super::Model::assemble`] refuses
    /// anything else.
    pub fn flat_to_flat(pitch: Length, height: Length) -> Self {
        Self {
            centre_cm: [0.0; 3],
            pitch_cm: pitch.get::<centimeter>(),
            height_cm: height.get::<centimeter>(),
            faces: HexFaces::NormalToY,
        }
    }

    /// Turn the faces: [`HexFaces::NormalToY`] or [`HexFaces::NormalToX`].
    pub fn faces(mut self, faces: HexFaces) -> Self {
        self.faces = faces;
        self
    }

    /// Move the centre to `(x, y, z)`.
    pub fn centred_at(mut self, x: Length, y: Length, z: Length) -> Self {
        self.centre_cm = [
            x.get::<centimeter>(),
            y.get::<centimeter>(),
            z.get::<centimeter>(),
        ];
        self
    }

    /// Fill it with `material`.
    pub fn filled_with(self, material: Material) -> Shape {
        Shape::new(Solid::HexPrism(self), material)
    }

    /// Flat-to-flat distance \[cm\] (the lattice pitch).
    pub fn pitch_cm(&self) -> f64 {
        self.pitch_cm
    }

    /// Centre to face \[cm\].
    pub fn apothem_cm(&self) -> f64 {
        0.5 * self.pitch_cm
    }

    /// Length along z \[cm\].
    pub fn height_cm(&self) -> f64 {
        self.height_cm
    }

    /// Which way the faces point.
    pub fn faces_kind(&self) -> HexFaces {
        self.faces
    }

    /// Centre \[cm\].
    pub fn centre_cm(&self) -> [f64; 3] {
        self.centre_cm
    }

    /// Volume \[cm³\]: `2 sqrt(3) apothem² height`.
    pub fn volume_cm3(&self) -> f64 {
        let a = self.apothem_cm();
        2.0 * 3.0_f64.sqrt() * a * a * self.height_cm
    }

    /// The six outward unit normals of the side faces (x, y).
    pub(crate) fn normals(&self) -> [[f64; 2]; 6] {
        match self.faces {
            HexFaces::NormalToY => [
                [0.0, 1.0],
                [S, 0.5],
                [-S, 0.5],
                [0.0, -1.0],
                [-S, -0.5],
                [S, -0.5],
            ],
            HexFaces::NormalToX => [
                [1.0, 0.0],
                [0.5, S],
                [-0.5, S],
                [-1.0, 0.0],
                [-0.5, -S],
                [0.5, -S],
            ],
        }
    }

    /// The six centre-to-centre vectors to the neighbouring tiles of a
    /// lattice of this pitch.
    pub(crate) fn neighbour_vectors(&self) -> [[f64; 2]; 6] {
        self.normals()
            .map(|n| [n[0] * self.pitch_cm, n[1] * self.pitch_cm])
    }

    /// The plane of side face `k`: `n . x = d` with unit normal `n`.
    pub(crate) fn face_plane(&self, k: usize) -> ([f64; 2], f64) {
        let n = self.normals()[k];
        (
            n,
            n[0] * self.centre_cm[0] + n[1] * self.centre_cm[1] + self.apothem_cm(),
        )
    }

    /// The six corners of the hexagon (x, y), counter-clockwise.
    pub(crate) fn polygon(&self) -> [[f64; 2]; 6] {
        let a = self.apothem_cm();
        let r = a / S;
        let [cx, cy, _] = self.centre_cm;
        let v: [[f64; 2]; 6] = match self.faces {
            HexFaces::NormalToY => [
                [r, 0.0],
                [0.5 * r, a],
                [-0.5 * r, a],
                [-r, 0.0],
                [-0.5 * r, -a],
                [0.5 * r, -a],
            ],
            HexFaces::NormalToX => [
                [a, 0.5 * r],
                [0.0, r],
                [-a, 0.5 * r],
                [-a, -0.5 * r],
                [0.0, -r],
                [a, -0.5 * r],
            ],
        };
        v.map(|p| [p[0] + cx, p[1] + cy])
    }

    /// The z range \[cm\].
    pub(crate) fn z_range(&self) -> (f64, f64) {
        (
            self.centre_cm[2] - 0.5 * self.height_cm,
            self.centre_cm[2] + 0.5 * self.height_cm,
        )
    }

    /// The twelve corners (x, y, z).
    pub(crate) fn vertices(&self) -> Vec<[f64; 3]> {
        let (lo, hi) = self.z_range();
        let mut v = Vec::with_capacity(12);
        for z in [lo, hi] {
            v.extend(self.polygon().iter().map(|p| [p[0], p[1], z]));
        }
        v
    }

    /// Bounding box `(lower, upper)` \[cm\].
    pub(crate) fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        let poly = self.polygon();
        let (lo, hi) = self.z_range();
        let min = |k: usize| poly.iter().map(|p| p[k]).fold(f64::INFINITY, f64::min);
        let max = |k: usize| poly.iter().map(|p| p[k]).fold(f64::NEG_INFINITY, f64::max);
        ([min(0), min(1), lo], [max(0), max(1), hi])
    }

    /// Whether `p` is strictly inside.
    pub(crate) fn holds(&self, p: [f64; 3]) -> bool {
        let (lo, hi) = self.z_range();
        p[2] > lo && p[2] < hi && self.signed_lateral(p) < 0.0
    }

    /// The largest `n . (p - c) - apothem` over the six faces: negative
    /// inside, zero on a face, positive outside (the hexagon's own "max
    /// plane" form; exact inside, a lower bound on the distance outside).
    pub(crate) fn signed_lateral(&self, p: [f64; 3]) -> f64 {
        let a = self.apothem_cm();
        self.normals()
            .iter()
            .map(|n| n[0] * (p[0] - self.centre_cm[0]) + n[1] * (p[1] - self.centre_cm[1]) - a)
            .fold(f64::NEG_INFINITY, f64::max)
    }

    /// Positive and finite size.
    pub(crate) fn is_valid(&self) -> bool {
        let ok = |x: f64| x > 0.0 && x.is_finite();
        ok(self.pitch_cm) && ok(self.height_cm) && self.centre_cm.iter().all(|c| c.is_finite())
    }

    /// In words, for messages.
    pub(crate) fn describe(&self) -> String {
        format!(
            "a hexagonal prism of pitch {:.6} cm and height {:.6} cm",
            self.pitch_cm, self.height_cm
        )
    }

    /// Shifted by `d`.
    pub(crate) fn translated(mut self, d: [f64; 3]) -> Self {
        for (c, x) in self.centre_cm.iter_mut().zip(d) {
            *c += x;
        }
        self
    }
}

// ── 2-D convex polygon helpers ───────────────────────────────────────────

/// Distance from `p` to the convex polygon `poly` (counter-clockwise): zero
/// inside, else the distance to the nearest edge.
pub(crate) fn distance_to_polygon(p: [f64; 2], poly: &[[f64; 2]]) -> f64 {
    let n = poly.len();
    let mut inside = true;
    let mut best = f64::INFINITY;
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + 1) % n]);
        let e = [b[0] - a[0], b[1] - a[1]];
        let w = [p[0] - a[0], p[1] - a[1]];
        if e[0] * w[1] - e[1] * w[0] < 0.0 {
            inside = false;
        }
        let t = ((w[0] * e[0] + w[1] * e[1]) / (e[0] * e[0] + e[1] * e[1])).clamp(0.0, 1.0);
        let d = (p[0] - (a[0] + t * e[0])).hypot(p[1] - (a[1] + t * e[1]));
        best = best.min(d);
    }
    if inside {
        0.0
    } else {
        best
    }
}

/// Whether two convex polygons share area (touching is not sharing, to
/// `tol`): the separating-axis test on every edge normal of both.
pub(crate) fn polygons_overlap(a: &[[f64; 2]], b: &[[f64; 2]], tol: f64) -> bool {
    for poly in [a, b] {
        let n = poly.len();
        for i in 0..n {
            let (p, q) = (poly[i], poly[(i + 1) % n]);
            let axis = [q[1] - p[1], p[0] - q[0]];
            let len = axis[0].hypot(axis[1]);
            let u = [axis[0] / len, axis[1] / len];
            let project = |s: &[[f64; 2]]| {
                s.iter()
                    .map(|v| v[0] * u[0] + v[1] * u[1])
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), x| {
                        (lo.min(x), hi.max(x))
                    })
            };
            let (alo, ahi) = project(a);
            let (blo, bhi) = project(b);
            if ahi <= blo + tol || bhi <= alo + tol {
                return false;
            }
        }
    }
    true
}

/// Whether every one of `points` is inside `prism` (touching allowed, to
/// `tol`).
pub(crate) fn prism_holds_all(prism: &HexPrism, points: &[[f64; 3]], tol: f64) -> bool {
    let (lo, hi) = prism.z_range();
    points
        .iter()
        .all(|p| prism.signed_lateral(*p) <= tol && p[2] >= lo - tol && p[2] <= hi + tol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::shape::Solid;

    fn cm(x: f64) -> Length {
        Length::new::<centimeter>(x)
    }

    /// The prism's own numbers: area, corners, faces and neighbour vectors
    /// are those of a regular hexagon, for both ways of turning it.
    #[test]
    fn a_hex_prism_is_a_regular_hexagon() {
        for faces in [HexFaces::NormalToY, HexFaces::NormalToX] {
            let p = HexPrism::flat_to_flat(cm(2.0), cm(3.0)).faces(faces);
            assert_eq!(p.faces_kind(), faces);
            assert_eq!(p.faces_kind().orientation().eq(&faces.orientation()), true);
            assert!((p.apothem_cm() - 1.0).abs() < 1e-15);
            assert!((p.volume_cm3() - 2.0 * 3.0_f64.sqrt() * 3.0).abs() < 1e-12);
            let poly = p.polygon();
            let r = 2.0 / 3.0_f64.sqrt();
            for v in poly {
                assert!(
                    (v[0].hypot(v[1]) - r).abs() < 1e-12,
                    "corner at the circumradius"
                );
            }
            // Every corner lies on two faces; every face is at the apothem.
            for k in 0..6 {
                let (n, d) = p.face_plane(k);
                assert!((n[0].hypot(n[1]) - 1.0).abs() < 1e-15);
                assert!((d - 1.0).abs() < 1e-15);
                let on: usize = poly
                    .iter()
                    .filter(|v| (n[0] * v[0] + n[1] * v[1] - d).abs() < 1e-12)
                    .count();
                assert_eq!(on, 2);
                let nb = p.neighbour_vectors()[k];
                assert!((nb[0].hypot(nb[1]) - 2.0).abs() < 1e-12);
            }
            assert!(p.holds([0.0, 0.0, 0.0]) && !p.holds([0.0, 0.0, 1.5]));
            assert!(!p.holds([1.0, 0.0, 0.0]) || faces == HexFaces::NormalToY);
            assert!(p.is_valid());
            assert!(!HexPrism::flat_to_flat(cm(-1.0), cm(1.0)).is_valid());
            assert!(p.describe().contains("hexagonal prism"));
        }
        // Y: faces perpendicular to y, so (0, 1) is a face normal and a
        // neighbour sits straight above.
        let y = HexPrism::flat_to_flat(cm(2.0), cm(1.0));
        assert!(y.normals().contains(&[0.0, 1.0]));
        let moved = y.centred_at(cm(1.0), cm(2.0), cm(3.0));
        assert_eq!(moved.centre_cm(), [1.0, 2.0, 3.0]);
        assert_eq!(
            moved.translated([1.0, 0.0, 0.0]).centre_cm(),
            [2.0, 2.0, 3.0]
        );
        let (lo, hi) = moved.bounds();
        assert!((hi[2] - lo[2] - 1.0).abs() < 1e-15 && hi[0] > lo[0] && hi[1] > lo[1]);
        assert_eq!(moved.vertices().len(), 12);
    }

    /// Polygon distance and overlap, one case each way.
    #[test]
    fn polygon_distance_and_overlap_are_exact() {
        let hex = HexPrism::flat_to_flat(cm(2.0), cm(1.0)).polygon();
        assert_eq!(distance_to_polygon([0.0, 0.0], &hex), 0.0);
        // Straight out from a face: 0.5 beyond the apothem 1.
        assert!((distance_to_polygon([0.0, 1.5], &hex) - 0.5).abs() < 1e-12);
        // Out from a corner (the circumradius is 1.1547 along x).
        let r = 2.0 / 3.0_f64.sqrt();
        assert!((distance_to_polygon([r + 0.3, 0.0], &hex) - 0.3).abs() < 1e-12);
        // Two hexagons one pitch apart touch; closer they overlap.
        let shifted = |dx: f64, dy: f64| {
            HexPrism::flat_to_flat(cm(2.0), cm(1.0))
                .centred_at(cm(dx), cm(dy), cm(0.0))
                .polygon()
        };
        assert!(!polygons_overlap(&hex, &shifted(0.0, 2.0), 1e-12));
        assert!(polygons_overlap(&hex, &shifted(0.0, 1.9), 1e-12));
        assert!(polygons_overlap(&hex, &shifted(0.3, 0.0), 1e-12));
        let p = HexPrism::flat_to_flat(cm(2.0), cm(1.0));
        assert!(prism_holds_all(
            &p,
            &[[0.0, 0.0, 0.0], [0.0, 1.0, 0.5]],
            1e-12
        ));
        assert!(!prism_holds_all(&p, &[[0.0, 1.1, 0.0]], 1e-12));
        assert!(!prism_holds_all(&p, &[[0.0, 0.0, 0.6]], 1e-12));
    }
    /// A prism against every other solid, in both directions, one case each
    /// way for `encloses` and `overlaps`.
    #[test]
    fn a_hex_prism_relates_exactly_to_every_solid() {
        use crate::model::shape::{Cuboid, Cylinder, Sphere};
        let hex = |p: f64, h: f64| Solid::HexPrism(HexPrism::flat_to_flat(cm(p), cm(h)));
        let big = hex(4.0, 2.0); // apothem 2, circumradius 2.3094, z in +-1
                                 // Sphere inside: touches the faces at r = 1 on x-y? apothem 2.
        let ball = |r: f64, x: f64, z: f64| {
            Solid::Sphere(Sphere::radius(cm(r)).centred_at(cm(x), cm(0.0), cm(z)))
        };
        assert!(big.encloses(&ball(1.0, 0.0, 0.0)));
        assert!(
            !big.encloses(&ball(1.1, 0.0, 0.0)),
            "pokes through the ends"
        );
        assert!(big.encloses(&ball(0.9, 0.0, 0.0)));
        assert!(!big.encloses(&ball(0.9, 1.5, 0.0)), "through a face");
        // Box inside (corners checked): 2 x 2 x 2 fits, 4.2 wide does not.
        assert!(big.encloses(&Solid::Cuboid(Cuboid::sides(cm(2.0), cm(2.0), cm(2.0)))));
        assert!(!big.encloses(&Solid::Cuboid(Cuboid::sides(cm(4.2), cm(2.0), cm(2.0)))));
        // Cylinders: a rod of radius 1.9 fits (any height), 2.1 does not; a
        // finite one must also fit in z.
        let cyl = |r: f64, h: Option<f64>| {
            let c = Cylinder::radius(cm(r));
            Solid::Cylinder(h.map_or(c, |h| c.height(cm(h))))
        };
        assert!(big.encloses(&cyl(1.9, None)) && !big.encloses(&cyl(2.1, None)));
        assert!(big.encloses(&cyl(1.9, Some(2.0))) && !big.encloses(&cyl(1.9, Some(2.2))));
        // A prism inside a prism, and the other way round.
        assert!(big.encloses(&hex(3.0, 1.0)) && !hex(3.0, 1.0).encloses(&big));
        // A prism inside a sphere, a box, a cylinder.
        let sphere = |r: f64| Solid::Sphere(Sphere::radius(cm(r)));
        let r_circ: f64 = 2.0 / 0.866_025_403_784_438_6; // apothem 2
        let far = (r_circ * r_circ + 1.0).sqrt(); // farthest vertex
        assert!(sphere(far + 1e-9).encloses(&big) && !sphere(far - 1e-3).encloses(&big));
        assert!(Solid::Cuboid(Cuboid::sides(cm(4.7), cm(4.0), cm(2.0))).encloses(&big));
        assert!(!Solid::Cuboid(Cuboid::sides(cm(4.5), cm(4.0), cm(2.0))).encloses(&big));
        assert!(cyl(2.4, Some(2.0)).encloses(&big) && !cyl(2.3, Some(2.0)).encloses(&big));
        assert!(!cyl(2.4, Some(1.9)).encloses(&big));
        // Overlap: touching is not overlapping.
        // Sphere: the nearest point of the prism is 0.5 away along a face.
        assert!(big.overlaps(&ball(1.0, 2.9, 0.0)), "0.59 from a corner");
        assert!(ball(1.0, 2.9, 0.0).overlaps(&big));
        assert!(!big.overlaps(&ball(1.0, 3.4, 0.0)));
        assert!(!big.overlaps(&ball(1.0, 0.0, 2.1)) && big.overlaps(&ball(1.0, 0.0, 1.9)));
        // Boxes and cylinders beside a face.
        let slab = |x: f64| {
            Solid::Cuboid(Cuboid::sides(cm(1.0), cm(1.0), cm(1.0)).centred_at(
                cm(x),
                cm(0.0),
                cm(0.0),
            ))
        };
        assert!(big.overlaps(&slab(2.4)) && !big.overlaps(&slab(2.9)));
        assert!(!slab(2.9).overlaps(&big));
        let rod =
            |x: f64| Solid::Cylinder(Cylinder::radius(cm(0.5)).centred_at(cm(x), cm(0.0), cm(0.0)));
        assert!(big.overlaps(&rod(2.4)) && !big.overlaps(&rod(2.9)));
        assert!(!big.overlaps(&cyl(0.5, Some(1.0)).translated([0.0, 0.0, 1.6])));
        assert!(big.overlaps(&cyl(0.5, Some(1.0)).translated([0.0, 0.0, 1.4])));
        // Prism against prism: one pitch apart along a face normal touches.
        let shifted = |dy: f64| {
            Solid::HexPrism(HexPrism::flat_to_flat(cm(4.0), cm(2.0)).centred_at(
                cm(0.0),
                cm(dy),
                cm(0.0),
            ))
        };
        assert!(!big.overlaps(&shifted(4.0)) && big.overlaps(&shifted(3.9)));
        assert!(!big.overlaps(&hex(4.0, 2.0).translated([0.0, 0.0, 2.0])));
        // Solid helpers: shifted, mirrored, congruent.
        let off = ball(1.0, 0.5, 0.25);
        assert_eq!(off.mirrored_z().centre_cm(), [0.5, 0.0, -0.25]);
        assert!(off.congruent(&ball(1.0, 0.5, 0.25 + 1e-12), 1e-9));
        assert!(!off.congruent(&ball(1.1, 0.5, 0.25), 1e-9));
        assert!(!off.congruent(&sphere(1.0), 1e-9));
        assert!(big.congruent(&hex(4.0, 2.0), 1e-12) && !big.congruent(&hex(4.0, 2.1), 1e-12));
        assert!(slab(0.0).congruent(&slab(0.0), 0.0) && !slab(0.0).congruent(&slab(0.1), 0.0));
        assert!(
            rod(0.0).congruent(&rod(0.0), 0.0) && !rod(0.0).congruent(&cyl(0.5, Some(1.0)), 0.0)
        );
        assert!(cyl(0.5, Some(1.0)).congruent(&cyl(0.5, Some(1.0)), 0.0));
        assert!(!cyl(0.5, Some(1.0)).congruent(&cyl(0.5, Some(1.1)), 0.0));
        // A prism as a particle of a sphere (the packing's keep rule).
        let zone = Sphere::radius(cm(3.0));
        assert!(crate::model::lower_packed::whole_in_sphere(
            &zone,
            &hex(1.0, 1.0),
            [0.0; 3]
        ));
        assert!(!crate::model::lower_packed::whole_in_sphere(
            &zone,
            &hex(1.0, 1.0),
            [2.9, 0.0, 0.0]
        ));
    }
}
