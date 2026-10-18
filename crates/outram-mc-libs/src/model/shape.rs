//! **Beginner geometry: shapes filled with materials** (gh #938, #926,
//! epic #925).
//!
//! A beginner says "the critical sphere is this shape and this composition"
//! and never meets a cell, a region, a universe or the root (maintainer
//! decision on #925, 2026-10-17). Nesting is explicit:
//! [`Shape::containing`], one call per inner shape, and the outer shape
//! fills only what its children leave. Outside the outermost shape is vacuum
//! automatically, or reflective when the outermost shape says so
//! ([`Shape::boundary`], decided on #943 for tutorial 4).
//! [`super::Model::assemble`] lowers it all to today's CSG
//! [`Geometry`](crate::geometry::geometry::Geometry).
//!
//! ~~Three solids today~~ **Four solids (tutorial 7, 2026-10-18)**: [`Sphere`],
//! [`Cuboid`] (a box; [`Cube`] makes one
//! with equal sides), [`Cylinder`] (axis along z: a finite cylinder with
//! a height, or a **rod** without one that runs through whatever holds it,
//! a pin in a lattice tile) and [`HexPrism`](super::HexPrism) (the tile of a
//! hexagonal lattice). **Repeated structures** (#926) are a [`Lattice`] of
//! tiles, each tile an ordinary shape (a cube of matrix containing a TRISO
//! particle, say), placed in a container with [`Shape::containing_lattice`].
//! A TRISO particle itself needs nothing new: it is concentric spheres, one
//! `.containing` per statement (maintainer, #926, 2026-10-17).

use uom::si::f64::{Length, Ratio};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;

use super::dh_region::DhSpec;
use super::hex::{distance_to_polygon, polygons_overlap, prism_holds_all, HexPrism};
use super::lattice::Lattice;
use super::material::Material;
use super::model::{ModelError, Temperature};

/// **A labelled group of shapes**, made by the model it belongs to
/// ([`super::Model::component`]): the core, the reflector, all the control
/// rods together. A small `Copy` handle; **its identity is the object, not
/// the label**, so two components with the same label are still two, and a
/// component from another model is refused by
/// [`super::Model::assemble`]. The label is for output only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Component {
    pub(crate) model: u64,
    pub(crate) index: usize,
}

/// **What a neutron meets at the outside of the outermost shape** (set with
/// [`Shape::boundary`]). Vacuum is the automatic default and never needs
/// saying (decided on #943, tutorial 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    /// Neutrons that leave are lost (leakage).
    Vacuum,
    /// Neutrons are mirrored back in: no leakage. A reflective cube around
    /// one TRISO particle is an infinite regular array of them, so its k is
    /// a k-infinity.
    Reflective,
}

/// **A sphere**: a radius and, optionally, a centre (the origin otherwise).
/// Fill it with [`Sphere::filled_with`] to get a [`Shape`].
///
/// ```
/// use outram_mc_libs::model::prelude::*;
/// use uom::si::{f64::Length, length::centimeter};
/// let s = Sphere::radius(Length::new::<centimeter>(8.7407));
/// assert!((s.radius_cm() - 8.7407).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    centre_cm: [f64; 3],
    radius_cm: f64,
}

impl Sphere {
    /// A sphere of `radius` centred at the origin. The radius must be
    /// positive and finite; [`super::Model::assemble`] refuses one that is
    /// not.
    pub fn radius(radius: Length) -> Self {
        Self {
            centre_cm: [0.0; 3],
            radius_cm: radius.get::<centimeter>(),
        }
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
        Shape::new(Solid::Sphere(self), material)
    }

    /// Radius \[cm\].
    pub fn radius_cm(&self) -> f64 {
        self.radius_cm
    }

    /// Centre \[cm\].
    pub fn centre_cm(&self) -> [f64; 3] {
        self.centre_cm
    }

    pub(crate) fn distance_cm(&self, other: &Sphere) -> f64 {
        let d: [f64; 3] = std::array::from_fn(|i| self.centre_cm[i] - other.centre_cm[i]);
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
    }

    /// Whether `point` (cm) is inside.
    pub(crate) fn holds(&self, p: [f64; 3]) -> bool {
        let d: [f64; 3] = std::array::from_fn(|i| p[i] - self.centre_cm[i]);
        d[0] * d[0] + d[1] * d[1] + d[2] * d[2] < self.radius_cm * self.radius_cm
    }

    /// Axis-aligned bounding box `(lower, upper)` \[cm\].
    pub(crate) fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        (
            std::array::from_fn(|i| self.centre_cm[i] - self.radius_cm),
            std::array::from_fn(|i| self.centre_cm[i] + self.radius_cm),
        )
    }
}

/// **A box** with faces normal to x, y and z: three side lengths and,
/// optionally, a centre (the origin otherwise). [`Cube::side`] makes one with
/// equal sides.
///
/// ```
/// use outram_mc_libs::model::prelude::*;
/// use uom::si::{f64::Length, length::centimeter};
/// let cm = Length::new::<centimeter>;
/// let b = Cuboid::sides(cm(1.0), cm(2.0), cm(3.0));
/// assert_eq!(b.sides_cm(), [1.0, 2.0, 3.0]);
/// assert!((b.volume_cm3() - 6.0).abs() < 1e-12);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cuboid {
    centre_cm: [f64; 3],
    sides_cm: [f64; 3],
}

impl Cuboid {
    /// A box of sides `x`, `y`, `z` centred at the origin. Each side must be
    /// positive and finite; [`super::Model::assemble`] refuses one that is
    /// not.
    pub fn sides(x: Length, y: Length, z: Length) -> Self {
        Self {
            centre_cm: [0.0; 3],
            sides_cm: [
                x.get::<centimeter>(),
                y.get::<centimeter>(),
                z.get::<centimeter>(),
            ],
        }
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
        Shape::new(Solid::Cuboid(self), material)
    }

    /// Side lengths \[cm\].
    pub fn sides_cm(&self) -> [f64; 3] {
        self.sides_cm
    }

    /// Centre \[cm\].
    pub fn centre_cm(&self) -> [f64; 3] {
        self.centre_cm
    }

    /// Volume \[cm³\].
    pub fn volume_cm3(&self) -> f64 {
        self.sides_cm[0] * self.sides_cm[1] * self.sides_cm[2]
    }

    /// `(lower, upper)` corners \[cm\].
    pub(crate) fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        (
            std::array::from_fn(|i| self.centre_cm[i] - 0.5 * self.sides_cm[i]),
            std::array::from_fn(|i| self.centre_cm[i] + 0.5 * self.sides_cm[i]),
        )
    }
}

/// **A cylinder with its axis along z**: a radius and, optionally, a centre
/// (the origin otherwise) and a height.
///
/// - **With a height** ([`Cylinder::height`]) it is a finite solid, centred
///   on its centre: a reactor core, a tank.
/// - **Without one it is a rod**: it runs the whole height of whatever holds
///   it. That is what a fuel pin is in a lattice tile, where the lattice
///   (and the container the lattice fills) decide how tall it is. A rod is
///   only allowed inside a lattice tile; anywhere else
///   [`super::Model::assemble`] refuses it and asks for a height.
///
/// ```
/// use outram_mc_libs::model::prelude::*;
/// use uom::si::{f64::Length, length::centimeter};
/// let cm = Length::new::<centimeter>;
/// let pin = Cylinder::radius(cm(0.5149));
/// assert!(pin.is_rod());
/// let core = Cylinder::radius(cm(76.2)).height(cm(163.3));
/// assert_eq!(core.height_cm(), Some(163.3));
/// assert!((core.volume_cm3().unwrap() - std::f64::consts::PI * 76.2 * 76.2 * 163.3).abs() < 1e-6);
/// ```
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cylinder {
    centre_cm: [f64; 3],
    radius_cm: f64,
    height_cm: Option<f64>,
}

impl Cylinder {
    /// A rod of `radius` on the z axis (give it a height with
    /// [`Cylinder::height`] unless it sits in a lattice tile). The radius
    /// must be positive and finite; [`super::Model::assemble`] refuses one
    /// that is not.
    pub fn radius(radius: Length) -> Self {
        Self {
            centre_cm: [0.0; 3],
            radius_cm: radius.get::<centimeter>(),
            height_cm: None,
        }
    }

    /// Give it a height (its z extent, centred on its centre): now a finite
    /// cylinder. Must be positive and finite.
    pub fn height(mut self, height: Length) -> Self {
        self.height_cm = Some(height.get::<centimeter>());
        self
    }

    /// Move the centre to `(x, y, z)`. The z of a rod means nothing (it has
    /// no ends); give it 0.
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
        Shape::new(Solid::Cylinder(self), material)
    }

    /// Radius \[cm\].
    pub fn radius_cm(&self) -> f64 {
        self.radius_cm
    }

    /// Height \[cm\], `None` for a rod.
    pub fn height_cm(&self) -> Option<f64> {
        self.height_cm
    }

    /// Centre \[cm\].
    pub fn centre_cm(&self) -> [f64; 3] {
        self.centre_cm
    }

    /// Whether it has no height: it runs through whatever holds it.
    pub fn is_rod(&self) -> bool {
        self.height_cm.is_none()
    }

    /// Volume \[cm³\], `None` for a rod (it has no height of its own).
    pub fn volume_cm3(&self) -> Option<f64> {
        self.height_cm
            .map(|h| std::f64::consts::PI * self.radius_cm * self.radius_cm * h)
    }

    /// The z range \[cm\], infinite for a rod.
    pub(crate) fn z_range(&self) -> (f64, f64) {
        match self.height_cm {
            Some(h) => (self.centre_cm[2] - 0.5 * h, self.centre_cm[2] + 0.5 * h),
            None => (f64::NEG_INFINITY, f64::INFINITY),
        }
    }

    /// Distance in the x-y plane from the axis to the point `p`.
    fn radial_cm(&self, p: [f64; 3]) -> f64 {
        (p[0] - self.centre_cm[0]).hypot(p[1] - self.centre_cm[1])
    }

    /// Whether `p` is inside.
    pub(crate) fn holds(&self, p: [f64; 3]) -> bool {
        let (lo, hi) = self.z_range();
        self.radial_cm(p) < self.radius_cm && p[2] > lo && p[2] < hi
    }

    /// Bounding box `(lower, upper)` \[cm\] (infinite in z for a rod).
    pub(crate) fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        let (lo, hi) = self.z_range();
        (
            [
                self.centre_cm[0] - self.radius_cm,
                self.centre_cm[1] - self.radius_cm,
                lo,
            ],
            [
                self.centre_cm[0] + self.radius_cm,
                self.centre_cm[1] + self.radius_cm,
                hi,
            ],
        )
    }
}

/// **A cube**: makes a [`Cuboid`] with three equal sides, from a length or
/// from a packing fraction (both decided for the regular unit cell on #943,
/// tutorial 4).
pub struct Cube;

impl Cube {
    /// A cube of side `side`, centred at the origin.
    ///
    /// ```
    /// use outram_mc_libs::model::prelude::*;
    /// use uom::si::{f64::Length, length::centimeter};
    /// let c = Cube::side(Length::new::<centimeter>(0.2));
    /// assert_eq!(c.sides_cm(), [0.2; 3]);
    /// ```
    pub fn side(side: Length) -> Cuboid {
        Cuboid::sides(side, side, side)
    }

    /// **The cube a particle fills to `packing_fraction`**: side
    /// `a = (V / pf)^(1/3)`, where `V` is the volume of `particle`'s
    /// outermost solid (for a TRISO particle, the OPyC sphere). A reflective
    /// cube of this side around one particle is an infinite regular (simple
    /// cubic) array at that packing fraction.
    ///
    /// A fraction above what fits (`pi/6 = 0.5236` for a sphere in a cube)
    /// gives a cube the particle pokes out of, which
    /// [`super::Model::assemble`] refuses with a picture; a fraction that is
    /// not positive gives an infinite or NaN side, refused the same way.
    ///
    /// ```
    /// use outram_mc_libs::model::prelude::*;
    /// use uom::si::{f64::{Length, Ratio}, length::centimeter, ratio::ratio};
    /// let p = Sphere::radius(Length::new::<centimeter>(1.0))
    ///     .filled_with(Material::from_atom_densities([(C12, atoms_per_barn_cm(0.08))]));
    /// let c = Cube::with_packing_fraction(&p, Ratio::new::<ratio>(std::f64::consts::PI / 6.0));
    /// assert!((c.sides_cm()[0] - 2.0).abs() < 1e-12);
    /// ```
    pub fn with_packing_fraction(particle: &Shape, packing_fraction: Ratio) -> Cuboid {
        let pf = packing_fraction.get::<ratio>();
        let a = (particle.solid.volume_cm3() / pf).cbrt();
        let l = Length::new::<centimeter>(a);
        Cuboid::sides(l, l, l)
    }

    /// **The tile of a simple-cubic lattice that places exactly `count`
    /// whole `particle`s in `zone`** (a sphere), with one tile centre at
    /// `offset` pitches from the zone's centre and whole-particle rejection
    /// (a particle is kept when it lies wholly inside). Use the cube with
    /// [`super::Lattice::covering`] and the same `offset`.
    ///
    /// The pitch is searched over +/-3 % of the continuum estimate by
    /// [`crate::pebble_beds::sphere_packing::cubic_pitch_for_count`] (the
    /// search the HTR-10 core's lattice uses, `nee_soon`'s `TrisoBuild`);
    /// this function is that search, not a copy of it.
    ///
    /// # Errors
    /// [`ModelError::BadValue`] when the particle is not a sphere centred
    /// at its origin, when it does not fit in the zone, or when **no pitch
    /// in the search gives exactly `count`**: the count moves in symmetry
    /// shells, and the message names the nearest count reached so the
    /// reader can pick another offset (a generic one such as `[0.13, 0.37,
    /// 0.71]` breaks the symmetry; gh:#430).
    ///
    /// ```
    /// use outram_mc_libs::model::prelude::*;
    /// use uom::si::{f64::Length, length::centimeter};
    /// let cm = Length::new::<centimeter>;
    /// let fuel = Material::from_atom_densities([(U235, atoms_per_barn_cm(0.04))]);
    /// let particle = Sphere::radius(cm(0.0455)).filled_with(fuel);
    /// let tile = Cube::for_whole_particles_in_sphere(
    ///     &particle, Sphere::radius(cm(2.5)), 8335, [0.13, 0.37, 0.71])?;
    /// assert!((tile.sides_cm()[0] - 0.1951235).abs() < 1e-6);
    /// # Ok::<(), ModelError>(())
    /// ```
    pub fn for_whole_particles_in_sphere(
        particle: &Shape,
        zone: Sphere,
        count: usize,
        offset: [f64; 3],
    ) -> Result<Cuboid, ModelError> {
        let r = particle.packed_radius_cm()?;
        if !(r < zone.radius_cm) {
            return Err(ModelError::BadValue(format!(
                "a particle of radius {r} cm does not fit in a zone of radius {} cm",
                zone.radius_cm
            )));
        }
        let (pitch, got) = crate::pebble_beds::sphere_packing::cubic_pitch_for_count(
            r,
            zone.radius_cm,
            count,
            offset,
        );
        if got != count {
            return Err(ModelError::BadValue(format!(
                "no pitch places exactly {count} whole particles at offset {offset:?}; the \
                 nearest is {got} (pitch {pitch:.7} cm). The count moves in symmetry shells: \
                 try a generic offset such as [0.13, 0.37, 0.71]"
            )));
        }
        Ok(Cube::side(Length::new::<centimeter>(pitch)))
    }
}

/// **The solid a [`Shape`] occupies**: a [`Sphere`], a [`Cuboid`], a
/// [`Cylinder`] or a [`HexPrism`]. Enum dispatch (workspace rule: no trait objects).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Solid {
    /// A sphere.
    Sphere(Sphere),
    /// A box.
    Cuboid(Cuboid),
    /// A cylinder along z, finite or a rod.
    Cylinder(Cylinder),
    /// A hexagonal prism along z: the tile of a hexagonal lattice.
    HexPrism(HexPrism),
}

impl Solid {
    /// Volume \[cm³\], exact (infinite for a rod: see
    /// [`Solid::volume_within`]).
    pub fn volume_cm3(&self) -> f64 {
        match self {
            Solid::Sphere(s) => 4.0 / 3.0 * std::f64::consts::PI * s.radius_cm.powi(3),
            Solid::Cuboid(b) => b.volume_cm3(),
            Solid::Cylinder(c) => c.volume_cm3().unwrap_or(f64::INFINITY),
            Solid::HexPrism(h) => h.volume_cm3(),
        }
    }

    /// Volume \[cm³\] when a rod is as tall as `height` \[cm\] (the height
    /// a one-layer lattice is extruded through); the exact volume otherwise.
    pub(crate) fn volume_within(&self, height: Option<f64>) -> f64 {
        match (self, height) {
            (Solid::Cylinder(c), Some(h)) if c.is_rod() => {
                std::f64::consts::PI * c.radius_cm * c.radius_cm * h
            }
            _ => self.volume_cm3(),
        }
    }

    /// Whether this is a [`Cylinder`] without a height.
    pub(crate) fn is_rod(&self) -> bool {
        matches!(self, Solid::Cylinder(c) if c.is_rod())
    }

    /// Centre \[cm\].
    pub fn centre_cm(&self) -> [f64; 3] {
        match self {
            Solid::Sphere(s) => s.centre_cm,
            Solid::Cuboid(b) => b.centre_cm,
            Solid::Cylinder(c) => c.centre_cm,
            Solid::HexPrism(h) => h.centre_cm(),
        }
    }

    /// Axis-aligned bounding box `(lower, upper)` \[cm\] (infinite in z for
    /// a rod).
    pub(crate) fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        match self {
            Solid::Sphere(s) => s.bounds(),
            Solid::Cuboid(b) => b.bounds(),
            Solid::Cylinder(c) => c.bounds(),
            Solid::HexPrism(h) => h.bounds(),
        }
    }

    /// Whether `p` (cm) is inside.
    pub(crate) fn holds(&self, p: [f64; 3]) -> bool {
        match self {
            Solid::Sphere(s) => s.holds(p),
            Solid::Cuboid(b) => {
                let (l, u) = b.bounds();
                (0..3).all(|i| p[i] > l[i] && p[i] < u[i])
            }
            Solid::Cylinder(c) => c.holds(p),
            Solid::HexPrism(h) => h.holds(p),
        }
    }

    /// Its size, for error messages and tolerances \[cm\].
    pub(crate) fn scale_cm(&self) -> f64 {
        match self {
            Solid::Sphere(s) => s.radius_cm,
            Solid::Cuboid(b) => b.sides_cm.iter().fold(0.0_f64, |m, x| m.max(*x)),
            Solid::Cylinder(c) => c.radius_cm.max(c.height_cm.unwrap_or(0.0)),
            Solid::HexPrism(h) => h.pitch_cm().max(h.height_cm()),
        }
    }

    /// Positive and finite dimensions.
    pub(crate) fn is_valid(&self) -> bool {
        let ok = |x: f64| x > 0.0 && x.is_finite();
        match self {
            Solid::Sphere(s) => ok(s.radius_cm) && s.centre_cm.iter().all(|c| c.is_finite()),
            Solid::Cuboid(b) => {
                b.sides_cm.iter().all(|x| ok(*x)) && b.centre_cm.iter().all(|c| c.is_finite())
            }
            Solid::Cylinder(c) => {
                ok(c.radius_cm)
                    && c.height_cm.is_none_or(ok)
                    && c.centre_cm.iter().all(|x| x.is_finite())
            }
            Solid::HexPrism(h) => h.is_valid(),
        }
    }

    /// In words, for messages.
    pub(crate) fn describe(&self) -> String {
        match self {
            Solid::Sphere(s) => format!("a sphere of radius {:.6} cm", s.radius_cm),
            Solid::Cuboid(b) => format!(
                "a box of sides {:.6} x {:.6} x {:.6} cm",
                b.sides_cm[0], b.sides_cm[1], b.sides_cm[2]
            ),
            Solid::Cylinder(c) => match c.height_cm {
                Some(h) => format!(
                    "a cylinder of radius {:.6} cm and height {h:.6} cm",
                    c.radius_cm
                ),
                None => format!("a rod of radius {:.6} cm", c.radius_cm),
            },
            Solid::HexPrism(h) => h.describe(),
        }
    }

    /// This solid shifted by `d` \[cm\].
    pub(crate) fn translated(&self, d: [f64; 3]) -> Solid {
        let shift = |c: [f64; 3]| -> [f64; 3] { std::array::from_fn(|i| c[i] + d[i]) };
        match *self {
            Solid::Sphere(mut s) => {
                s.centre_cm = shift(s.centre_cm);
                Solid::Sphere(s)
            }
            Solid::Cuboid(mut b) => {
                b.centre_cm = shift(b.centre_cm);
                Solid::Cuboid(b)
            }
            Solid::Cylinder(mut c) => {
                c.centre_cm = shift(c.centre_cm);
                Solid::Cylinder(c)
            }
            Solid::HexPrism(h) => Solid::HexPrism(h.translated(d)),
        }
    }

    /// The mirror image in the plane `z = 0`.
    pub(crate) fn mirrored_z(&self) -> Solid {
        let c = self.centre_cm();
        self.translated([0.0, 0.0, -2.0 * c[2]])
    }

    /// Whether `other` is this solid to within `tol` \[cm\]: the same kind,
    /// size and place.
    pub(crate) fn congruent(&self, other: &Solid, tol: f64) -> bool {
        let near = |a: f64, b: f64| (a - b).abs() <= tol;
        let same_centre = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| near(a[i], b[i]));
        match (self, other) {
            (Solid::Sphere(a), Solid::Sphere(b)) => {
                same_centre(a.centre_cm, b.centre_cm) && near(a.radius_cm, b.radius_cm)
            }
            (Solid::Cuboid(a), Solid::Cuboid(b)) => {
                same_centre(a.centre_cm, b.centre_cm)
                    && (0..3).all(|i| near(a.sides_cm[i], b.sides_cm[i]))
            }
            (Solid::Cylinder(a), Solid::Cylinder(b)) => {
                same_centre(a.centre_cm, b.centre_cm)
                    && near(a.radius_cm, b.radius_cm)
                    && match (a.height_cm, b.height_cm) {
                        (Some(x), Some(y)) => near(x, y),
                        (None, None) => true,
                        _ => false,
                    }
            }
            (Solid::HexPrism(a), Solid::HexPrism(b)) => {
                same_centre(a.centre_cm(), b.centre_cm())
                    && near(a.pitch_cm(), b.pitch_cm())
                    && near(a.height_cm(), b.height_cm())
                    && a.faces_kind() == b.faces_kind()
            }
            _ => false,
        }
    }

    /// Whether `inner` lies wholly inside `self` (touching allowed, to a
    /// relative `1e-12`). Exact for every pair of solids here. A rod has no
    /// ends of its own, so it is checked in x and y only (and in z against
    /// nothing): [`super::Model::assemble`] separately insists a rod sits in
    /// a lattice tile, whose lattice gives it its height. Nothing finite
    /// holds a rod in z: a sphere does not enclose one.
    pub(crate) fn encloses(&self, inner: &Solid) -> bool {
        let tol = 1.0e-12 * self.scale_cm();
        match (self, inner) {
            (Solid::HexPrism(o), _) => {
                let (lo, hi) = o.z_range();
                match inner {
                    Solid::Sphere(i) => {
                        o.signed_lateral(i.centre_cm) + i.radius_cm <= tol
                            && i.centre_cm[2] - i.radius_cm >= lo - tol
                            && i.centre_cm[2] + i.radius_cm <= hi + tol
                    }
                    Solid::Cuboid(i) => {
                        let (l, u) = i.bounds();
                        let corners: Vec<[f64; 3]> = (0..8)
                            .map(|k| {
                                std::array::from_fn(|d| if (k >> d) & 1 == 0 { l[d] } else { u[d] })
                            })
                            .collect();
                        prism_holds_all(o, &corners, tol)
                    }
                    Solid::Cylinder(i) => {
                        let (zl, zh) = i.z_range();
                        o.signed_lateral(i.centre_cm) + i.radius_cm <= tol
                            && (i.is_rod() || (zl >= lo - tol && zh <= hi + tol))
                    }
                    Solid::HexPrism(i) => prism_holds_all(o, &i.vertices(), tol),
                }
            }
            (Solid::Sphere(o), Solid::HexPrism(i)) => i.vertices().iter().all(|v| {
                (0..3)
                    .map(|k| (v[k] - o.centre_cm[k]).powi(2))
                    .sum::<f64>()
                    .sqrt()
                    <= o.radius_cm + tol
            }),
            (Solid::Cylinder(o), Solid::HexPrism(i)) => {
                let (lo, hi) = o.z_range();
                i.vertices().iter().all(|v| {
                    o.radial_cm(*v) <= o.radius_cm + tol && v[2] >= lo - tol && v[2] <= hi + tol
                })
            }
            (Solid::Sphere(o), Solid::Sphere(i)) => {
                o.distance_cm(i) + i.radius_cm <= o.radius_cm + tol
            }
            (Solid::Sphere(o), Solid::Cuboid(i)) => {
                // The farthest corner from the sphere's centre.
                let (l, u) = i.bounds();
                let d2: f64 = (0..3)
                    .map(|k| {
                        let a = (l[k] - o.centre_cm[k]).abs();
                        let b = (u[k] - o.centre_cm[k]).abs();
                        a.max(b).powi(2)
                    })
                    .sum();
                d2.sqrt() <= o.radius_cm + tol
            }
            (Solid::Sphere(o), Solid::Cylinder(i)) => {
                // The farthest point of a finite cylinder is on the rim of
                // its farther end.
                let (lo, hi) = i.z_range();
                if !(lo.is_finite() && hi.is_finite()) {
                    return false;
                }
                let rim = (i.centre_cm[0] - o.centre_cm[0]).hypot(i.centre_cm[1] - o.centre_cm[1])
                    + i.radius_cm;
                let dz = (lo - o.centre_cm[2]).abs().max((hi - o.centre_cm[2]).abs());
                rim.hypot(dz) <= o.radius_cm + tol
            }
            (Solid::Cuboid(o), Solid::Cylinder(i)) => {
                let (lo, uo) = o.bounds();
                let (li, ui) = i.bounds();
                let z_ok = i.is_rod() || (li[2] >= lo[2] - tol && ui[2] <= uo[2] + tol);
                z_ok && (0..2).all(|k| li[k] >= lo[k] - tol && ui[k] <= uo[k] + tol)
            }
            (Solid::Cuboid(o), _) => {
                let (lo, uo) = o.bounds();
                let (li, ui) = inner.bounds();
                (0..3).all(|k| li[k] >= lo[k] - tol && ui[k] <= uo[k] + tol)
            }
            (Solid::Cylinder(o), Solid::Sphere(i)) => {
                let (lo, hi) = o.z_range();
                o.radial_cm(i.centre_cm) + i.radius_cm <= o.radius_cm + tol
                    && i.centre_cm[2] - i.radius_cm >= lo - tol
                    && i.centre_cm[2] + i.radius_cm <= hi + tol
            }
            (Solid::Cylinder(o), Solid::Cuboid(i)) => {
                let (lo, hi) = o.z_range();
                let (l, u) = i.bounds();
                let corners_in = [(l[0], l[1]), (l[0], u[1]), (u[0], l[1]), (u[0], u[1])]
                    .iter()
                    .all(|&(x, y)| {
                        (x - o.centre_cm[0]).hypot(y - o.centre_cm[1]) <= o.radius_cm + tol
                    });
                corners_in && l[2] >= lo - tol && u[2] <= hi + tol
            }
            (Solid::Cylinder(o), Solid::Cylinder(i)) => {
                let (lo, hi) = o.z_range();
                let (li, ui) = i.z_range();
                let d = (i.centre_cm[0] - o.centre_cm[0]).hypot(i.centre_cm[1] - o.centre_cm[1]);
                let radial = d + i.radius_cm <= o.radius_cm + tol;
                // A rod in a rod, or any rod in a cylinder (the holder gives
                // it its height): x-y only. A finite one must fit in z too.
                radial && (i.is_rod() || (li >= lo - tol && ui <= hi + tol))
            }
        }
    }

    /// Whether `self` and `other` share volume (touching is not
    /// overlapping, to a relative `1e-12`). Exact for every pair.
    pub(crate) fn overlaps(&self, other: &Solid) -> bool {
        let tol = 1.0e-12 * self.scale_cm().max(other.scale_cm());
        match (self, other) {
            (Solid::HexPrism(h), o) | (o, Solid::HexPrism(h)) => hex_overlaps(h, o, tol),
            (Solid::Sphere(a), Solid::Sphere(b)) => {
                a.distance_cm(b) < a.radius_cm + b.radius_cm - tol
            }
            (Solid::Sphere(s), Solid::Cuboid(b)) | (Solid::Cuboid(b), Solid::Sphere(s)) => {
                // Distance from the centre to the nearest point of the box.
                let (l, u) = b.bounds();
                let d2: f64 = (0..3)
                    .map(|k| {
                        let c = s.centre_cm[k];
                        (l[k] - c).max(0.0).max(c - u[k]).powi(2)
                    })
                    .sum();
                d2.sqrt() < s.radius_cm - tol
            }
            (Solid::Cuboid(a), Solid::Cuboid(b)) => {
                let (la, ua) = a.bounds();
                let (lb, ub) = b.bounds();
                (0..3).all(|k| la[k] < ub[k] - tol && lb[k] < ua[k] - tol)
            }
            (Solid::Sphere(s), Solid::Cylinder(c)) | (Solid::Cylinder(c), Solid::Sphere(s)) => {
                // The nearest point of a cylinder to the centre: the centre
                // pulled radially onto the disc and clamped in z.
                let (lo, hi) = c.z_range();
                let dz = (lo - s.centre_cm[2]).max(0.0).max(s.centre_cm[2] - hi);
                let dr = (c.radial_cm(s.centre_cm) - c.radius_cm).max(0.0);
                dr.hypot(dz) < s.radius_cm - tol
            }
            (Solid::Cuboid(b), Solid::Cylinder(c)) | (Solid::Cylinder(c), Solid::Cuboid(b)) => {
                let (l, u) = b.bounds();
                let (lo, hi) = c.z_range();
                let dx = (l[0] - c.centre_cm[0]).max(0.0).max(c.centre_cm[0] - u[0]);
                let dy = (l[1] - c.centre_cm[1]).max(0.0).max(c.centre_cm[1] - u[1]);
                dx.hypot(dy) < c.radius_cm - tol && l[2] < hi - tol && lo < u[2] - tol
            }
            (Solid::Cylinder(a), Solid::Cylinder(b)) => {
                let (la, ha) = a.z_range();
                let (lb, hb) = b.z_range();
                let d = (a.centre_cm[0] - b.centre_cm[0]).hypot(a.centre_cm[1] - b.centre_cm[1]);
                d < a.radius_cm + b.radius_cm - tol && la < hb - tol && lb < ha - tol
            }
        }
    }
}

/// **Whether a hexagonal prism and another solid share volume** (touching is
/// not sharing, to `tol`). Every solid here is an extrusion along z, so the
/// x-y shapes are tested as convex polygons or discs and the z extents as
/// intervals; a sphere is the one solid that couples the two.
fn hex_overlaps(h: &HexPrism, other: &Solid, tol: f64) -> bool {
    let (hlo, hhi) = h.z_range();
    let poly = h.polygon();
    let gap = |lo: f64, hi: f64| (hlo - hi).max(lo - hhi).max(0.0);
    match other {
        Solid::Sphere(s) => {
            let dz = gap(s.centre_cm[2], s.centre_cm[2]);
            distance_to_polygon([s.centre_cm[0], s.centre_cm[1]], &poly).hypot(dz)
                < s.radius_cm - tol
        }
        Solid::Cuboid(b) => {
            let (l, u) = b.bounds();
            let rect = [[l[0], l[1]], [u[0], l[1]], [u[0], u[1]], [l[0], u[1]]];
            polygons_overlap(&poly, &rect, tol) && l[2] < hhi - tol && hlo < u[2] - tol
        }
        Solid::Cylinder(c) => {
            let (lo, hi) = c.z_range();
            distance_to_polygon([c.centre_cm[0], c.centre_cm[1]], &poly) < c.radius_cm - tol
                && lo < hhi - tol
                && hlo < hi - tol
        }
        Solid::HexPrism(g) => {
            let (glo, ghi) = g.z_range();
            polygons_overlap(&poly, &g.polygon(), tol) && glo < hhi - tol && hlo < ghi - tol
        }
    }
}

/// **A shape filled with a material**, possibly containing other shapes or
/// a lattice, belonging to a [`Component`], and (outermost only) with a
/// [`Boundary`].
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub(crate) solid: Solid,
    pub(crate) material: Material,
    pub(crate) component: Option<Component>,
    pub(crate) children: Vec<Shape>,
    pub(crate) lattice: Option<Lattice>,
    pub(crate) boundary: Option<Boundary>,
    /// Set when this shape is a [`super::DhRegion`]: its particles, their
    /// packing and its tracking.
    pub(crate) dh: Option<DhSpec>,
    /// This shape's own temperature (gh #945), for it and everything it
    /// contains that sets none of its own.
    pub(crate) temperature: Option<Temperature>,
}

impl Shape {
    pub(crate) fn new(solid: Solid, material: Material) -> Self {
        Self {
            solid,
            material,
            component: None,
            children: Vec::new(),
            lattice: None,
            boundary: None,
            dh: None,
            temperature: None,
        }
    }

    /// A copy of this shape, and everything inside it, shifted by `d`
    /// \[cm\]. A lattice inside keeps its own frame; a packed particle is
    /// placed by its region's centre, so it moves with it.
    pub(crate) fn translated(&self, d: [f64; 3]) -> Shape {
        let mut s = self.clone();
        s.solid = s.solid.translated(d);
        s.children = s.children.iter().map(|c| c.translated(d)).collect();
        s
    }

    /// A copy mirrored in the plane `z = 0` (centres only; every shape here
    /// is its own mirror image about its own centre).
    pub(crate) fn mirrored_z(&self) -> Shape {
        let mut s = self.clone();
        s.solid = s.solid.mirrored_z();
        s.children = s.children.iter().map(Shape::mirrored_z).collect();
        s
    }

    /// Whether `other` is this shape to within `tol` \[cm\]: congruent
    /// solids, equal materials, components, temperatures and contents, in
    /// the same order.
    pub(crate) fn congruent(&self, other: &Shape, tol: f64) -> bool {
        self.solid.congruent(&other.solid, tol)
            && self.material == other.material
            && self.component == other.component
            && self.temperature == other.temperature
            && self.boundary == other.boundary
            && self.dh == other.dh
            && self.lattice == other.lattice
            && self.children.len() == other.children.len()
            && self
                .children
                .iter()
                .zip(&other.children)
                .all(|(a, b)| a.congruent(b, tol))
    }

    /// **Give this shape its own temperature** (gh #945, stage 1: one fixed
    /// temperature per region). It applies to this shape and to every shape,
    /// lattice tile and packed particle inside it that sets none of its own,
    /// as a fill cell's temperature does in OpenMC (`openmc/cell.py`,
    /// `Cell.temperature` setter). Unlike OpenMC's setter, an inner shape's
    /// own temperature always wins, whichever was written first.
    ///
    /// Which temperature a shape takes, first match wins: this; its
    /// component's ([`super::Model::set_component_temperature`]); its
    /// container's; the model's ([`super::Model::set_temperature`]). The
    /// nuclides of the shape's material are processed at it, exactly (no
    /// interpolation between tabulated temperatures; that is stage 2).
    pub fn at_temperature(mut self, temperature: Temperature) -> Self {
        self.temperature = Some(temperature);
        self
    }

    /// This shape's own temperature, if one was set.
    pub fn own_temperature(&self) -> Option<Temperature> {
        self.temperature
    }

    /// The radius of a particle to be packed: its solid must be a sphere
    /// centred at its own origin (the packing places it).
    pub(crate) fn packed_radius_cm(&self) -> Result<f64, ModelError> {
        match self.solid {
            Solid::Sphere(s) if s.centre_cm == [0.0; 3] && self.solid.is_valid() => Ok(s.radius_cm),
            _ => Err(ModelError::BadValue(format!(
                "a packed particle must be a sphere centred at its own origin (the packing \
                 places it), not {}",
                self.solid.describe()
            ))),
        }
    }

    /// Put this shape in `component` (for per-component output; #940).
    pub fn in_component(mut self, component: Component) -> Self {
        self.component = Some(component);
        self
    }

    /// **Put `inner` inside this shape.** This shape then fills only the
    /// space inside it and outside every contained shape. `inner` must lie
    /// wholly inside, and contained shapes must not overlap each other;
    /// [`super::Model::assemble`] refuses either, with a picture. Write one
    /// `.containing` per statement, inside out (#943).
    ///
    /// `inner` may also be a [`super::DhRegion`] (a pebble's fuel zone).
    pub fn containing(mut self, inner: impl Into<Shape>) -> Self {
        self.children.push(inner.into());
        self
    }

    /// **Fill this shape with a [`Lattice`]** of tiles (#926). The lattice is
    /// centred on this shape; this shape's material fills whatever of it the
    /// lattice does not cover (OpenMC's lattice `outer`). The container
    /// holds nothing else, and is either a [`Cuboid`] the lattice fits
    /// inside, or a [`Sphere`] the lattice covers: then **whole-particle
    /// rejection** applies (a tile whose contents would poke out of the
    /// sphere is left out and the sphere's material fills its place;
    /// nothing is cut), and the tiles' own material must be the sphere's.
    /// [`super::Lattice::covering`] sizes the grid for a sphere.
    ///
    /// **A [`Cylinder`] container (tutorial 6)** is filled the other way
    /// round: the grid must *cover* the cylinder, and the cylinder cuts the
    /// tiles at its wall (OpenMC's lattice fill cell bounded by a cylinder
    /// and two planes; LCT-008's core is a 7 x 7 lattice cut by one).
    ///
    /// **A tile may itself be filled with a lattice** (a core of assemblies
    /// of pins).
    ///
    /// **A one-layer lattice (`n_z = 1`) is extruded**: the CSG lattice
    /// ignores z for it, so its tiles run through the container's whole
    /// height and sit at `z = 0`. It therefore needs a container centred at
    /// `z = 0` with tiles at least as tall (a box: exactly as tall); a grid
    /// that is to stop short in z needs two or more layers.
    /// ~~(Until 2026-10-18 a shorter one-layer grid was accepted and filled
    /// the whole box, against the sentence above about the material that
    /// fills what the lattice does not cover.)~~
    /// [`super::Model::assemble`] refuses anything else.
    pub fn containing_lattice(mut self, lattice: Lattice) -> Self {
        self.lattice = Some(lattice);
        self
    }

    /// **The boundary condition on this shape's outer surface.** Only for an
    /// outermost shape, and only when it is the model's one outermost shape;
    /// [`super::Model::assemble`] refuses it anywhere else. Vacuum is the
    /// default and never needs saying (#943).
    pub fn boundary(mut self, boundary: Boundary) -> Self {
        self.boundary = Some(boundary);
        self
    }

    /// The solid.
    pub fn solid(&self) -> &Solid {
        &self.solid
    }

    /// The material.
    pub fn material(&self) -> &Material {
        &self.material
    }

    /// The component, if one was given.
    pub fn component(&self) -> Option<Component> {
        self.component
    }

    /// The shapes directly inside.
    pub fn contained(&self) -> &[Shape] {
        &self.children
    }

    /// The lattice this shape is filled with, if any.
    pub fn contained_lattice(&self) -> Option<&Lattice> {
        self.lattice.as_ref()
    }

    /// The boundary set on this shape, if any.
    pub fn boundary_condition(&self) -> Option<Boundary> {
        self.boundary
    }

    /// Visit this shape and every shape inside it (contained shapes and
    /// lattice tiles), depth first, children before parents.
    pub(crate) fn visit(&self, f: &mut impl FnMut(&Shape)) {
        for c in &self.children {
            c.visit(f);
        }
        if let Some(l) = &self.lattice {
            for t in l.tiles() {
                t.visit(f);
            }
        }
        if let Some(p) = self.dh.as_ref().and_then(DhSpec::particle) {
            p.visit(f);
        }
        f(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::material::{atoms_per_barn_cm, Material};

    fn cm(x: f64) -> Length {
        Length::new::<centimeter>(x)
    }

    fn m() -> Material {
        Material::from_atom_densities([(crate::model::isotopes::U235, atoms_per_barn_cm(0.04))])
    }

    /// Geometry helpers the overlap checks rely on.
    #[test]
    fn sphere_geometry_helpers() {
        let a = Sphere::radius(cm(2.0));
        let b = Sphere::radius(cm(1.0)).centred_at(cm(3.0), cm(4.0), cm(0.0));
        assert_eq!(b.centre_cm(), [3.0, 4.0, 0.0]);
        assert!((a.distance_cm(&b) - 5.0).abs() < 1e-12);
        assert!(a.holds([1.0, 1.0, 1.0]) && !a.holds([2.0, 0.0, 0.0]));
        assert_eq!(b.bounds(), ([2.0, 3.0, -1.0], [4.0, 5.0, 1.0]));
        let s = a.filled_with(m()).containing(b.filled_with(m()));
        assert_eq!(s.contained().len(), 1);
        assert_eq!(s.solid(), &Solid::Sphere(a));
        assert!(s.component().is_none() && s.boundary_condition().is_none());
        assert!(s.contained_lattice().is_none());
        assert_eq!(s.material().name(), "material");
    }

    /// Box helpers, the cube constructors and every containment / overlap
    /// pair, each with one case each way.
    #[test]
    fn cuboid_cube_and_solid_relations() {
        let b = Cuboid::sides(cm(2.0), cm(4.0), cm(6.0)).centred_at(cm(1.0), cm(0.0), cm(0.0));
        assert_eq!(b.centre_cm(), [1.0, 0.0, 0.0]);
        assert_eq!(b.bounds(), ([0.0, -2.0, -3.0], [2.0, 2.0, 3.0]));
        let sb = Solid::Cuboid(b);
        assert!((sb.volume_cm3() - 48.0).abs() < 1e-12);
        assert!(sb.holds([1.0, 1.0, 1.0]) && !sb.holds([2.5, 0.0, 0.0]));
        assert_eq!(sb.scale_cm(), 6.0);
        assert!(sb.is_valid() && sb.describe().contains("box"));
        assert_eq!(sb.centre_cm(), [1.0, 0.0, 0.0]);

        let unit = Solid::Sphere(Sphere::radius(cm(1.0)));
        assert!(!Solid::Sphere(Sphere::radius(cm(-1.0))).is_valid());
        assert!(!Solid::Cuboid(Cube::side(cm(f64::INFINITY))).is_valid());
        // Sphere in sphere, box in sphere, sphere in box, box in box.
        assert!(Solid::Sphere(Sphere::radius(cm(2.0))).encloses(&unit));
        assert!(!unit.encloses(&Solid::Sphere(Sphere::radius(cm(1.5)))));
        assert!(unit.encloses(&Solid::Cuboid(Cube::side(cm(1.1)))));
        assert!(!unit.encloses(&Solid::Cuboid(Cube::side(cm(1.2)))));
        assert!(
            Solid::Cuboid(Cube::side(cm(2.0))).encloses(&unit),
            "touching faces"
        );
        assert!(!Solid::Cuboid(Cube::side(cm(1.9))).encloses(&unit));
        assert!(Solid::Cuboid(Cube::side(cm(2.0))).encloses(&Solid::Cuboid(Cube::side(cm(2.0)))));
        // Overlaps: touching is not overlapping.
        let at =
            |x: f64| Solid::Sphere(Sphere::radius(cm(1.0)).centred_at(cm(x), cm(0.0), cm(0.0)));
        assert!(!unit.overlaps(&at(2.0)) && unit.overlaps(&at(1.9)));
        let box_at =
            |x: f64| Solid::Cuboid(Cube::side(cm(2.0)).centred_at(cm(x), cm(0.0), cm(0.0)));
        assert!(!unit.overlaps(&box_at(2.0)) && box_at(1.9).overlaps(&unit));
        assert!(!box_at(0.0).overlaps(&box_at(2.0)) && box_at(0.0).overlaps(&box_at(1.9)));

        // The cube a particle fills to a packing fraction.
        let p = Sphere::radius(cm(0.0455)).filled_with(m());
        let pf = 0.050_248;
        let c = Cube::with_packing_fraction(&p, Ratio::new::<ratio>(pf));
        let got = p.solid().volume_cm3() / c.volume_cm3();
        assert!((got / pf - 1.0).abs() < 1e-14, "{got}");
        let s = c
            .filled_with(m())
            .containing(p)
            .boundary(Boundary::Reflective);
        assert_eq!(s.boundary_condition(), Some(Boundary::Reflective));
        let mut n = 0;
        s.visit(&mut |_| n += 1);
        assert_eq!(n, 2);
    }

    /// **Cylinders (tutorial 6)**: a rod and a finite cylinder against every
    /// other solid, one case each way for `encloses` and `overlaps`, and the
    /// helpers the lowering relies on.
    #[test]
    fn cylinder_rods_and_finite_cylinders_relate_exactly() {
        let rod = Cylinder::radius(cm(1.0));
        let core = Cylinder::radius(cm(5.0)).height(cm(10.0));
        assert!(rod.is_rod() && !core.is_rod());
        assert_eq!((rod.height_cm(), core.height_cm()), (None, Some(10.0)));
        assert!(rod.volume_cm3().is_none());
        assert!((core.volume_cm3().unwrap() - std::f64::consts::PI * 250.0).abs() < 1e-9);
        assert_eq!(core.centre_cm(), [0.0; 3]);
        let moved = core.centred_at(cm(1.0), cm(2.0), cm(3.0));
        assert_eq!(moved.centre_cm(), [1.0, 2.0, 3.0]);
        assert_eq!(moved.bounds(), ([-4.0, -3.0, -2.0], [6.0, 7.0, 8.0]));
        assert_eq!(rod.bounds().0[2], f64::NEG_INFINITY);
        assert!(core.holds([3.0, 3.0, 4.9]) && !core.holds([3.0, 4.0, 0.0]));
        assert!(!core.holds([0.0, 0.0, 5.0]) && rod.holds([0.5, 0.0, 1.0e9]));

        let (srod, score) = (Solid::Cylinder(rod), Solid::Cylinder(core));
        assert!(srod.is_rod() && !score.is_rod());
        assert!(srod.volume_cm3().is_infinite());
        assert!((srod.volume_within(Some(10.0)) - std::f64::consts::PI * 10.0).abs() < 1e-9);
        assert_eq!(score.volume_within(Some(1.0)), score.volume_cm3());
        assert!(srod.is_valid() && score.is_valid());
        assert!(!Solid::Cylinder(Cylinder::radius(cm(-1.0))).is_valid());
        assert!(!Solid::Cylinder(Cylinder::radius(cm(1.0)).height(cm(0.0))).is_valid());
        assert!(srod.describe().contains("rod") && score.describe().contains("height"));
        assert_eq!(score.scale_cm(), 10.0);
        assert_eq!(srod.centre_cm(), [0.0; 3]);

        // A rod fits in a box or a cylinder by its x-y extent only; nothing
        // finite holds it in z against a sphere.
        let bx = Solid::Cuboid(Cuboid::sides(cm(2.0), cm(2.0), cm(2.0)));
        assert!(bx.encloses(&srod) && score.encloses(&srod));
        assert!(!Solid::Cuboid(Cuboid::sides(cm(1.9), cm(2.0), cm(2.0))).encloses(&srod));
        assert!(!Solid::Sphere(Sphere::radius(cm(100.0))).encloses(&srod));
        // A finite cylinder: in a sphere by its rim, in a box and a cylinder
        // by x-y and z.
        let small = Solid::Cylinder(Cylinder::radius(cm(1.0)).height(cm(2.0)));
        let rim = 2.0_f64.sqrt();
        assert!(Solid::Sphere(Sphere::radius(cm(rim + 1e-9))).encloses(&small));
        assert!(!Solid::Sphere(Sphere::radius(cm(rim - 1e-3))).encloses(&small));
        assert!(bx.encloses(&small));
        assert!(!Solid::Cuboid(Cuboid::sides(cm(2.0), cm(2.0), cm(1.9))).encloses(&small));
        assert!(score.encloses(&small));
        assert!(!score.encloses(&Solid::Cylinder(Cylinder::radius(cm(1.0)).height(cm(11.0)))));
        assert!(
            !score.encloses(&Solid::Cylinder(Cylinder::radius(cm(1.0)).centred_at(
                cm(4.5),
                cm(0.0),
                cm(0.0)
            )))
        );
        // A sphere or a box in a cylinder.
        assert!(score.encloses(&Solid::Sphere(Sphere::radius(cm(4.9)))));
        assert!(!score.encloses(&Solid::Sphere(Sphere::radius(cm(5.1)))));
        assert!(score.encloses(&Solid::Cuboid(Cuboid::sides(cm(6.0), cm(6.0), cm(10.0)))));
        assert!(!score.encloses(&Solid::Cuboid(Cuboid::sides(cm(8.0), cm(8.0), cm(2.0)))));

        // Overlaps, touching is not overlapping.
        let at =
            |x: f64| Solid::Cylinder(Cylinder::radius(cm(1.0)).centred_at(cm(x), cm(0.0), cm(0.0)));
        assert!(srod.overlaps(&at(1.9)) && !srod.overlaps(&at(2.0)));
        let high = |z: f64| {
            Solid::Cylinder(Cylinder::radius(cm(1.0)).height(cm(2.0)).centred_at(
                cm(0.0),
                cm(0.0),
                cm(z),
            ))
        };
        assert!(high(0.0).overlaps(&high(1.9)) && !high(0.0).overlaps(&high(2.0)));
        assert!(
            srod.overlaps(&high(1.0e6)),
            "a rod meets a cylinder at any height"
        );
        let ball = |x: f64, z: f64| {
            Solid::Sphere(Sphere::radius(cm(1.0)).centred_at(cm(x), cm(0.0), cm(z)))
        };
        assert!(small.overlaps(&ball(1.9, 0.0)) && !small.overlaps(&ball(2.0, 0.0)));
        assert!(ball(0.0, 1.9).overlaps(&small) && !ball(0.0, 2.1).overlaps(&small));
        let slab = |x: f64| {
            Solid::Cuboid(Cuboid::sides(cm(2.0), cm(2.0), cm(2.0)).centred_at(
                cm(x),
                cm(0.0),
                cm(0.0),
            ))
        };
        assert!(small.overlaps(&slab(1.9)) && !small.overlaps(&slab(2.0)));
        assert!(slab(0.0).overlaps(&srod));
        // A cylinder cannot be a whole particle of a sphere unless finite and
        // inside: the packing's test.
        let zone = Sphere::radius(cm(3.0));
        assert!(super::super::lower_packed::whole_in_sphere(
            &zone, &small, [0.0; 3]
        ));
        assert!(!super::super::lower_packed::whole_in_sphere(
            &zone,
            &small,
            [2.0, 0.0, 0.0]
        ));
        assert!(!super::super::lower_packed::whole_in_sphere(
            &zone, &srod, [0.0; 3]
        ));
        // Fill it and put it in a shape.
        let pin = rod
            .filled_with(m())
            .containing(Cylinder::radius(cm(0.5)).filled_with(m()));
        assert_eq!(pin.contained().len(), 1);
        let mut n = 0;
        pin.visit(&mut |_| n += 1);
        assert_eq!(n, 2);
    }
}
