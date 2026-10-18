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
//! Two solids today: [`Sphere`] and [`Cuboid`] (a box; [`Cube`] makes one
//! with equal sides). **Repeated structures** (#926) are a [`Lattice`] of
//! tiles, each tile an ordinary shape (a cube of matrix containing a TRISO
//! particle, say), placed in a container with [`Shape::containing_lattice`].
//! A TRISO particle itself needs nothing new: it is concentric spheres, one
//! `.containing` per statement (maintainer, #926, 2026-10-17).

use uom::si::f64::{Length, Ratio};
use uom::si::length::centimeter;
use uom::si::ratio::ratio;

use super::dh_region::DhSpec;
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

/// **The solid a [`Shape`] occupies**: a [`Sphere`] or a [`Cuboid`]. Enum
/// dispatch (workspace rule: no trait objects).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Solid {
    /// A sphere.
    Sphere(Sphere),
    /// A box.
    Cuboid(Cuboid),
}

impl Solid {
    /// Volume \[cm³\], exact.
    pub fn volume_cm3(&self) -> f64 {
        match self {
            Solid::Sphere(s) => 4.0 / 3.0 * std::f64::consts::PI * s.radius_cm.powi(3),
            Solid::Cuboid(b) => b.volume_cm3(),
        }
    }

    /// Centre \[cm\].
    pub fn centre_cm(&self) -> [f64; 3] {
        match self {
            Solid::Sphere(s) => s.centre_cm,
            Solid::Cuboid(b) => b.centre_cm,
        }
    }

    /// Axis-aligned bounding box `(lower, upper)` \[cm\].
    pub(crate) fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        match self {
            Solid::Sphere(s) => s.bounds(),
            Solid::Cuboid(b) => b.bounds(),
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
        }
    }

    /// Its size, for error messages and tolerances \[cm\].
    pub(crate) fn scale_cm(&self) -> f64 {
        match self {
            Solid::Sphere(s) => s.radius_cm,
            Solid::Cuboid(b) => b.sides_cm.iter().fold(0.0_f64, |m, x| m.max(*x)),
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
        }
    }

    /// Whether `inner` lies wholly inside `self` (touching allowed, to a
    /// relative `1e-12`). Exact for every pair of solids here.
    pub(crate) fn encloses(&self, inner: &Solid) -> bool {
        let tol = 1.0e-12 * self.scale_cm();
        match (self, inner) {
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
            (Solid::Cuboid(o), _) => {
                let (lo, uo) = o.bounds();
                let (li, ui) = inner.bounds();
                (0..3).all(|k| li[k] >= lo[k] - tol && ui[k] <= uo[k] + tol)
            }
        }
    }

    /// Whether `self` and `other` share volume (touching is not
    /// overlapping, to a relative `1e-12`). Exact for every pair.
    pub(crate) fn overlaps(&self, other: &Solid) -> bool {
        let tol = 1.0e-12 * self.scale_cm().max(other.scale_cm());
        match (self, other) {
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
    fn new(solid: Solid, material: Material) -> Self {
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
            Solid::Sphere(s) if s.centre_cm == [0.0; 3] && self.solid.is_valid() => {
                Ok(s.radius_cm)
            }
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
}
