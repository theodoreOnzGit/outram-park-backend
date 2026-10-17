//! **Beginner geometry: shapes filled with materials** (gh #938, epic #925).
//!
//! A beginner says "the critical sphere is this shape and this composition"
//! and never meets a cell, a region, a universe or the root (maintainer
//! decision on #925, 2026-10-17). Nesting is explicit:
//! [`Shape::containing`], one call per inner shape, and the outer shape
//! fills only what its children leave. Outside the outermost shape is vacuum
//! automatically. [`super::Model::assemble`] lowers it all to today's CSG
//! [`Geometry`](crate::geometry::geometry::Geometry).
//!
//! Only spheres today; more primitives and repeated structures (lattices,
//! TRISO) follow on #926.

use uom::si::f64::Length;
use uom::si::length::centimeter;

use super::material::Material;

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
        Shape {
            sphere: self,
            material,
            component: None,
            children: Vec::new(),
        }
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

/// **A shape filled with a material**, possibly containing other shapes and
/// belonging to a [`Component`].
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub(crate) sphere: Sphere,
    pub(crate) material: Material,
    pub(crate) component: Option<Component>,
    pub(crate) children: Vec<Shape>,
}

impl Shape {
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
    pub fn containing(mut self, inner: Shape) -> Self {
        self.children.push(inner);
        self
    }

    /// The sphere.
    pub fn sphere(&self) -> &Sphere {
        &self.sphere
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::material::{atoms_per_barn_cm, Material};

    /// Geometry helpers the overlap checks rely on.
    #[test]
    fn sphere_geometry_helpers() {
        let cm = Length::new::<centimeter>;
        let a = Sphere::radius(cm(2.0));
        let b = Sphere::radius(cm(1.0)).centred_at(cm(3.0), cm(4.0), cm(0.0));
        assert_eq!(b.centre_cm(), [3.0, 4.0, 0.0]);
        assert!((a.distance_cm(&b) - 5.0).abs() < 1e-12);
        assert!(a.holds([1.0, 1.0, 1.0]) && !a.holds([2.0, 0.0, 0.0]));
        assert_eq!(b.bounds(), ([2.0, 3.0, -1.0], [4.0, 5.0, 1.0]));
        let m = Material::from_atom_densities([(
            crate::model::isotopes::U235,
            atoms_per_barn_cm(0.04),
        )]);
        let s = a.filled_with(m.clone()).containing(b.filled_with(m));
        assert_eq!(s.contained().len(), 1);
        assert_eq!(s.sphere().radius_cm(), 2.0);
        assert!(s.component().is_none());
        assert_eq!(s.material().name(), "material");
    }
}
