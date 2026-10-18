//! **A hopper: a cylinder that narrows through a cone into an outlet tube**
//! (tutorial 8 on #943, epic #925).
//!
//! The container of a pebble bed that is not a box: HTR-10's core cavity is
//! a cylinder (radius 90 cm), a cone below it (36.946 cm tall, narrowing to
//! the 25 cm discharge tube) and the tube (to the bottom of the model). It is
//! a solid of revolution about the z axis. Its **floor** is the plane where
//! the cylinder ends and the cone begins (the top of the cone): a pebble bed
//! rests on it.
//!
//! The one exact test a bed needs is [`Hopper::contains_ball`]: **a sphere
//! lies inside a solid of revolution exactly when its meridional disc lies
//! inside the profile**, i.e. its centre is inside the profile polygon and
//! no farther than its radius from the nearest profile edge. That is
//! `nee_soon::htr10_rmc::bed::SekerBed::containment` (gh:#472), expression
//! for expression, so the set of balls a bed keeps is that crate's.

use uom::si::f64::Length;
use uom::si::length::centimeter;

use super::material::Material;
use super::shape::{Shape, Solid};

/// **Which wall of a hopper a ball would have crossed.**
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wall {
    /// The top of the loaded bed.
    Top,
    /// The cylinder's side wall.
    SideWall,
    /// The cone.
    Cone,
    /// The outlet tube's wall.
    TubeWall,
    /// The bottom of the outlet tube.
    TubeBottom,
}

/// **A hopper**: a cylinder of radius `R` from its floor up `height`, a cone
/// below the floor narrowing to the outlet radius, and an outlet tube below
/// that. Build it with [`Hopper::new`] and the builders, fill it with
/// [`Hopper::filled_with`].
///
/// ```
/// use outram_mc_libs::model::prelude::*;
/// use uom::si::{f64::Length, length::centimeter};
/// let cm = Length::new::<centimeter>;
/// // HTR-10's core cavity: cylinder 90 cm in radius and 221.818 cm tall above
/// // the floor, a 36.946 cm cone down to a 25 cm tube 221.236 cm deep.
/// let cavity = Hopper::new(cm(90.0), cm(221.818))
///     .conical_floor(cm(36.946), cm(25.0))
///     .outlet_tube(cm(221.236));
/// assert!((cavity.bottom_z_cm() + 258.182).abs() < 1e-9);
/// assert!(cavity.volume_cm3() > 5.6e6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hopper {
    radius_cm: f64,
    height_cm: f64,
    cone_height_cm: f64,
    outlet_radius_cm: f64,
    tube_depth_cm: f64,
    floor_z_cm: f64,
}

impl Hopper {
    /// A plain cylinder of `radius` and `height`, its floor at `z = 0`, the
    /// axis on the z axis. Add the cone and the tube with
    /// [`Self::conical_floor`] and [`Self::outlet_tube`]. Sizes must be
    /// positive and finite; [`super::Model::assemble`] refuses others.
    pub fn new(radius: Length, height: Length) -> Self {
        Self {
            radius_cm: radius.get::<centimeter>(),
            height_cm: height.get::<centimeter>(),
            cone_height_cm: 0.0,
            outlet_radius_cm: 0.0,
            tube_depth_cm: 0.0,
            floor_z_cm: 0.0,
        }
    }

    /// Narrow the floor through a cone `height` tall, from the cylinder's
    /// radius down to `outlet_radius` (smaller than the cylinder's).
    pub fn conical_floor(mut self, height: Length, outlet_radius: Length) -> Self {
        self.cone_height_cm = height.get::<centimeter>();
        self.outlet_radius_cm = outlet_radius.get::<centimeter>();
        self
    }

    /// Continue the outlet below the cone as a tube `depth` long. Needs a
    /// cone first.
    pub fn outlet_tube(mut self, depth: Length) -> Self {
        self.tube_depth_cm = depth.get::<centimeter>();
        self
    }

    /// Put the floor (the top of the cone) at height `z`.
    pub fn floor_at(mut self, z: Length) -> Self {
        self.floor_z_cm = z.get::<centimeter>();
        self
    }

    /// Fill it with `material`.
    pub fn filled_with(self, material: Material) -> Shape {
        Shape::new(Solid::Hopper(self), material)
    }

    /// Cylinder radius \[cm\].
    pub fn radius_cm(&self) -> f64 {
        self.radius_cm
    }

    /// Height of the cylinder above the floor \[cm\].
    pub fn height_cm(&self) -> f64 {
        self.height_cm
    }

    /// Height of the cone \[cm\] (zero when there is none).
    pub fn cone_height_cm(&self) -> f64 {
        self.cone_height_cm
    }

    /// Radius of the outlet \[cm\] (zero when there is no cone).
    pub fn outlet_radius_cm(&self) -> f64 {
        self.outlet_radius_cm
    }

    /// Depth of the outlet tube \[cm\].
    pub fn tube_depth_cm(&self) -> f64 {
        self.tube_depth_cm
    }

    /// The floor, the top of the cone \[cm\].
    pub fn floor_z_cm(&self) -> f64 {
        self.floor_z_cm
    }

    /// The top of the cylinder \[cm\].
    pub fn top_z_cm(&self) -> f64 {
        self.floor_z_cm + self.height_cm
    }

    /// The bottom of the cone \[cm\] (the floor when there is no cone).
    pub fn cone_bottom_z_cm(&self) -> f64 {
        self.floor_z_cm - self.cone_height_cm
    }

    /// The lowest point \[cm\]: the bottom of the tube, or of the cone, or
    /// the floor.
    pub fn bottom_z_cm(&self) -> f64 {
        self.cone_bottom_z_cm() - self.tube_depth_cm
    }

    /// Whether there is a cone.
    pub fn has_cone(&self) -> bool {
        self.cone_height_cm > 0.0
    }

    /// Exact volume \[cm³\]: the cylinder, the frustum (`pi h (R² + R r +
    /// r²) / 3`) and the tube.
    pub fn volume_cm3(&self) -> f64 {
        let pi = std::f64::consts::PI;
        let (r, ro) = (self.radius_cm, self.outlet_radius_cm);
        let mut v = pi * r * r * self.height_cm;
        if self.has_cone() {
            v += pi * self.cone_height_cm * (r * r + r * ro + ro * ro) / 3.0;
            v += pi * ro * ro * self.tube_depth_cm;
        }
        v
    }

    /// The cone's slope, `(R - r_outlet) / height` (zero without a cone).
    pub(crate) fn slope(&self) -> f64 {
        if self.has_cone() {
            (self.radius_cm - self.outlet_radius_cm) / self.cone_height_cm
        } else {
            0.0
        }
    }

    /// Radius of the hopper at height `z` \[cm\] (zero outside it).
    pub(crate) fn radius_at(&self, z: f64) -> f64 {
        if z > self.top_z_cm() || z < self.bottom_z_cm() {
            0.0
        } else if z >= self.floor_z_cm {
            self.radius_cm
        } else if z >= self.cone_bottom_z_cm() {
            self.outlet_radius_cm + self.slope() * (z - self.cone_bottom_z_cm())
        } else {
            self.outlet_radius_cm
        }
    }

    /// Whether `p` is strictly inside.
    pub(crate) fn holds(&self, p: [f64; 3]) -> bool {
        p[2] > self.bottom_z_cm()
            && p[2] < self.top_z_cm()
            && p[0].hypot(p[1]) < self.radius_at(p[2])
    }

    /// Bounding box `(lower, upper)` \[cm\].
    pub(crate) fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        let r = self.radius_cm;
        ([-r, -r, self.bottom_z_cm()], [r, r, self.top_z_cm()])
    }

    /// Positive, finite sizes; an outlet narrower than the cylinder; a tube
    /// only below a cone.
    pub(crate) fn is_valid(&self) -> bool {
        let ok = |x: f64| x > 0.0 && x.is_finite();
        let fin = |x: f64| x >= 0.0 && x.is_finite();
        ok(self.radius_cm)
            && ok(self.height_cm)
            && fin(self.cone_height_cm)
            && fin(self.tube_depth_cm)
            && self.floor_z_cm.is_finite()
            && if self.has_cone() {
                ok(self.outlet_radius_cm) && self.outlet_radius_cm < self.radius_cm
            } else {
                self.outlet_radius_cm == 0.0 && self.tube_depth_cm == 0.0
            }
    }

    /// In words, for messages.
    pub(crate) fn describe(&self) -> String {
        format!(
            "a hopper of radius {:.6} cm, {:.6} cm tall above its floor, cone {:.6} cm to \
             radius {:.6} cm, tube {:.6} cm",
            self.radius_cm,
            self.height_cm,
            self.cone_height_cm,
            self.outlet_radius_cm,
            self.tube_depth_cm
        )
    }

    /// Shifted by `d`. The axis stays on the z axis, so only z moves.
    pub(crate) fn translated(mut self, d: [f64; 3]) -> Self {
        self.floor_z_cm += d[2];
        self
    }

    /// **The meridional profile** `(rho, z)` as a closed polygon, from the
    /// axis at `top` round to the axis at the bottom: the top, the side
    /// wall, the cone, the tube wall, the tube bottom, and the axis closing
    /// it. `top` is the top of the *loaded bed* (at most the hopper's own).
    /// The last edge is the axis, which is not a wall.
    pub(crate) fn profile(&self, top: f64) -> Vec<[f64; 2]> {
        let mut p = vec![
            [0.0, top],
            [self.radius_cm, top],
            [self.radius_cm, self.floor_z_cm],
        ];
        if self.has_cone() {
            p.push([self.outlet_radius_cm, self.cone_bottom_z_cm()]);
            if self.tube_depth_cm > 0.0 {
                p.push([self.outlet_radius_cm, self.bottom_z_cm()]);
            }
            p.push([0.0, self.bottom_z_cm()]);
        } else {
            p.push([0.0, self.floor_z_cm]);
        }
        p
    }

    /// The wall each edge of [`Self::profile`] is, in order (the closing axis
    /// edge is not one).
    pub(crate) fn walls(&self) -> Vec<Wall> {
        let mut w = vec![Wall::Top, Wall::SideWall];
        if self.has_cone() {
            w.push(Wall::Cone);
            if self.tube_depth_cm > 0.0 {
                w.push(Wall::TubeWall);
                w.push(Wall::TubeBottom);
            } else {
                w.push(Wall::TubeBottom);
            }
        } else {
            w.push(Wall::TubeBottom);
        }
        w
    }

    /// **Whether a ball of radius `r` centred at `c` lies wholly inside the
    /// hopper filled up to `top`, and whether its centre does, and which
    /// wall is nearest.** Ported from
    /// `nee_soon::htr10_rmc::bed::SekerBed::containment` (gh:#472): even-odd
    /// test of `(rho, z)` against the profile; the ball is whole when the
    /// nearest wall is at least `r - 1e-9` away (a ball touching a wall is
    /// whole).
    pub(crate) fn contains_ball(&self, c: [f64; 3], r: f64, top: f64) -> BallFit {
        let (rho, z) = (c[0].hypot(c[1]), c[2]);
        let p = self.profile(top);
        let walls = self.walls();
        let mut inside = false;
        let mut dmin = f64::INFINITY;
        let mut nearest = Wall::SideWall;
        for i in 0..p.len() {
            let (u, v) = (p[i], p[(i + 1) % p.len()]);
            if (u[1] > z) != (v[1] > z) && rho < u[0] + (z - u[1]) / (v[1] - u[1]) * (v[0] - u[0]) {
                inside = !inside;
            }
            // The closing segment is the axis, which is not a boundary.
            if i + 1 < p.len() {
                let d = segment_distance(rho, z, u, v);
                if d < dmin {
                    dmin = d;
                    nearest = walls[i];
                }
            }
        }
        BallFit {
            whole: inside && dmin >= r - 1e-9,
            centre_inside: inside,
            nearest,
        }
    }
}

/// How a ball sits against a hopper: see [`Hopper::contains_ball`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BallFit {
    pub(crate) whole: bool,
    pub(crate) centre_inside: bool,
    pub(crate) nearest: Wall,
}

/// Distance from `(px, pz)` to the segment `a`-`b` in the meridional plane
/// (`nee_soon::htr10_rmc::bed::segment_distance`).
fn segment_distance(px: f64, pz: f64, a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dz * dz;
    let t = if l2 > 0.0 {
        (((px - a[0]) * dx + (pz - a[1]) * dz) / l2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (px - a[0] - t * dx).hypot(pz - a[1] - t * dz)
}

/// Distance from a point `(rho, z)` to the (filled) profile: zero inside.
/// For two solids' overlap tests (a sphere against a hopper).
pub(crate) fn distance_to_profile(h: &Hopper, rho: f64, z: f64) -> f64 {
    if h.contains_ball([rho, 0.0, z], 0.0, h.top_z_cm())
        .centre_inside
    {
        return 0.0;
    }
    let p = h.profile(h.top_z_cm());
    (0..p.len() - 1)
        .map(|i| segment_distance(rho, z, p[i], p[i + 1]))
        .fold(f64::INFINITY, f64::min)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::shape::Solid;

    fn cm(x: f64) -> Length {
        Length::new::<centimeter>(x)
    }

    fn htr10() -> Hopper {
        Hopper::new(cm(90.0), cm(221.818))
            .conical_floor(cm(36.946), cm(25.0))
            .outlet_tube(cm(221.236))
    }

    /// The hopper's own numbers: levels, radius at each height, volume, and
    /// the profile and its walls.
    #[test]
    fn a_hopper_is_a_cylinder_a_cone_and_a_tube() {
        let h = htr10().floor_at(cm(10.0));
        assert!((h.top_z_cm() - 231.818).abs() < 1e-9);
        assert!((h.floor_z_cm() - 10.0).abs() < 1e-12);
        assert!((h.cone_bottom_z_cm() - (10.0 - 36.946)).abs() < 1e-12);
        assert!((h.bottom_z_cm() - (10.0 - 36.946 - 221.236)).abs() < 1e-12);
        assert!((h.radius_at(100.0) - 90.0).abs() < 1e-9);
        assert!((h.radius_at(10.0 - 36.946) - 25.0).abs() < 1e-9);
        assert!(
            (h.radius_at(10.0 - 0.5 * 36.946) - 57.5).abs() < 1e-9,
            "mid-cone"
        );
        assert!((h.radius_at(-100.0) - 25.0).abs() < 1e-9);
        assert_eq!(h.radius_at(1000.0), 0.0);
        let pi = std::f64::consts::PI;
        let frustum = pi * 36.946 / 3.0 * (8100.0 + 2250.0 + 625.0);
        let want = pi * 8100.0 * 221.818 + frustum + pi * 625.0 * 221.236;
        assert!((h.volume_cm3() / want - 1.0).abs() < 1e-14);
        assert!(h.holds([0.0, 0.0, 0.0]) && h.holds([89.0, 0.0, 100.0]));
        assert!(!h.holds([89.0, 0.0, -10.0]), "outside the cone");
        assert!(h.holds([24.0, 0.0, -100.0]) && !h.holds([26.0, 0.0, -100.0]));
        let (lo, hi) = h.bounds();
        assert!(
            (lo[2] - h.bottom_z_cm()).abs() < 1e-12
                && (hi[2] - h.top_z_cm()).abs() < 1e-12
                && (hi[0] - 90.0).abs() < 1e-9
        );
        assert_eq!(h.profile(50.0).len(), 6);
        assert_eq!(
            h.walls().len(),
            5,
            "top, wall, cone, tube wall, tube bottom"
        );
        assert!(h.is_valid() && h.describe().contains("hopper"));
        assert!((h.translated([5.0, 7.0, 1.0]).floor_z_cm() - 11.0).abs() < 1e-12);
        // A cylinder only, and a cone without a tube.
        let plain = Hopper::new(cm(2.0), cm(3.0));
        assert!(plain.is_valid() && !plain.has_cone());
        assert!(plain.bottom_z_cm().abs() < 1e-12);
        assert!((plain.volume_cm3() - pi * 12.0).abs() < 1e-12);
        assert_eq!(plain.profile(3.0).len(), 4);
        assert_eq!(
            plain.walls(),
            vec![Wall::Top, Wall::SideWall, Wall::TubeBottom]
        );
        let coned = Hopper::new(cm(2.0), cm(3.0)).conical_floor(cm(1.0), cm(1.0));
        assert!(coned.is_valid());
        assert_eq!(coned.walls().len(), 4);
        assert_eq!(coned.profile(3.0).len(), 5);
        // Refused.
        assert!(!Hopper::new(cm(-1.0), cm(3.0)).is_valid());
        assert!(!Hopper::new(cm(1.0), cm(3.0))
            .conical_floor(cm(1.0), cm(2.0))
            .is_valid());
        assert!(!Hopper::new(cm(1.0), cm(3.0))
            .outlet_tube(cm(1.0))
            .is_valid());
    }

    /// The ball test: whole against each wall, and the reason it fails.
    #[test]
    fn a_ball_is_whole_only_if_it_clears_every_wall() {
        let h = htr10();
        let top = 123.576;
        let fit = |x: f64, z: f64| h.contains_ball([x, 0.0, z], 3.0, top);
        // Deep inside.
        assert!(fit(0.0, 60.0).whole);
        // Touching the side wall is whole (tolerance), a hair past is not.
        assert!(fit(87.0, 60.0).whole);
        let past = fit(87.0 + 1e-6, 60.0);
        assert!(!past.whole && past.centre_inside && past.nearest == Wall::SideWall);
        // The top plane.
        assert!(fit(0.0, top - 3.0).whole);
        let high = fit(0.0, top - 2.9);
        assert!(!high.whole && high.nearest == Wall::Top);
        // A centre outside is not whole and not "inside".
        let out = fit(95.0, 60.0);
        assert!(!out.whole && !out.centre_inside);
        // The cone: a ball low on the floor circle near the wall crosses it.
        let c = fit(86.0, -1.0);
        assert!(!c.whole && c.centre_inside && c.nearest == Wall::Cone);
        // The tube: a ball of radius 3 wholly inside radius 25 has centre
        // radius <= 22; at 23 it crosses the tube wall.
        assert!(h.contains_ball([22.0, 0.0, -100.0], 3.0, top).whole);
        let t = h.contains_ball([23.0, 0.0, -100.0], 3.0, top);
        assert!(!t.whole && t.nearest == Wall::TubeWall);
        // The bottom of the tube.
        let b = h.contains_ball([0.0, 0.0, h.bottom_z_cm() + 2.0], 3.0, top);
        assert!(!b.whole && b.nearest == Wall::TubeBottom);
        // Off-axis azimuth does not matter.
        let y = h.contains_ball([0.0, 87.0 + 1e-6, 60.0], 3.0, top);
        assert!(!y.whole && y.nearest == Wall::SideWall);
        // A distance to the profile: zero inside, the gap outside.
        assert!(distance_to_profile(&h, 10.0, 0.0) == 0.0);
        assert!((distance_to_profile(&h, 95.0, 100.0) - 5.0).abs() < 1e-9);
    }

    /// A hopper solid against the other solids: what holds what.
    #[test]
    fn a_hopper_relates_to_the_other_solids() {
        use crate::model::shape::{Cuboid, Cylinder, Sphere};
        use crate::model::hex::HexPrism;
        let h = Hopper::new(cm(10.0), cm(20.0))
            .conical_floor(cm(5.0), cm(4.0))
            .outlet_tube(cm(10.0));
        let s = Solid::Hopper(h);
        let ball = |x: f64, z: f64| {
            Solid::Sphere(Sphere::radius(cm(1.0)).centred_at(cm(x), cm(0.0), cm(z)))
        };
        assert!(s.encloses(&ball(0.0, 10.0)) && !s.encloses(&ball(9.5, 10.0)));
        assert!(!s.encloses(&ball(9.5, -0.5)), "cuts the cone");
        assert!(s.encloses(&Solid::Cylinder(
            Cylinder::radius(cm(3.0))
                .height(cm(5.0))
                .centred_at(cm(0.0), cm(0.0), cm(10.0))
        )));
        assert!(s.encloses(&Solid::Hopper(
            Hopper::new(cm(5.0), cm(5.0)).floor_at(cm(2.0))
        )));
        let cyl = Solid::Cylinder(Cylinder::radius(cm(11.0)).height(cm(60.0)).centred_at(
            cm(0.0),
            cm(0.0),
            cm(5.0),
        ));
        assert!(cyl.encloses(&s));
        assert!(!Solid::Cylinder(Cylinder::radius(cm(9.0)).height(cm(60.0))).encloses(&s));
        let big = Solid::Sphere(Sphere::radius(cm(100.0)));
        assert!(big.encloses(&s) && !Solid::Sphere(Sphere::radius(cm(5.0))).encloses(&s));
        assert!(
            Solid::Cuboid(Cuboid::sides(cm(30.0), cm(30.0), cm(100.0)).centred_at(
                cm(0.0),
                cm(0.0),
                cm(0.0)
            ))
            .encloses(&s)
        );
        assert!(Solid::HexPrism(HexPrism::flat_to_flat(cm(60.0), cm(100.0))).encloses(&s));
        assert!(s.overlaps(&ball(9.5, 10.0)) && !s.overlaps(&ball(13.0, 10.0)));
        assert!(s.overlaps(&Solid::Cuboid(Cuboid::sides(cm(2.0), cm(2.0), cm(2.0)))));
        assert!(!s.overlaps(&Solid::Cuboid(
            Cuboid::sides(cm(2.0), cm(2.0), cm(2.0)).centred_at(cm(50.0), cm(0.0), cm(0.0))
        )));
        assert!(s.congruent(&Solid::Hopper(h), 1e-12) && !s.congruent(&ball(0.0, 0.0), 1e-12));
        assert!(s.centre_cm().iter().all(|v| v.abs() < 1e-12));
        assert!(s.is_valid() && s.describe().contains("hopper") && s.scale_cm() > 10.0);
        assert!((s.volume_cm3() - h.volume_cm3()).abs() < 1e-9);
        assert!(s.holds([0.0, 0.0, 1.0]) && !s.holds([0.0, 0.0, 100.0]));
        let moved = s.translated([5.0, 5.0, 3.0]);
        assert!((moved.centre_cm()[2] - 3.0).abs() < 1e-12);
        let (l, u) = s.bounds();
        assert!(l[2] < 0.0 && u[2] == 20.0);
        assert!(!Solid::Sphere(Sphere::radius(cm(1.0))).encloses(&Solid::Hopper(h)));
    }
}
