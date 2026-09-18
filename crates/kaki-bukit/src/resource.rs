// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/resource.h, src/resource.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! The traded-resource enum.
//!
//! Upstream's `Resource` is an abstract base class with two concrete
//! subclasses, [`Material`] and [`Product`], and everything in the exchange
//! layer holds a `Resource::Ptr`. This workspace forbids trait objects, so the
//! polymorphism becomes an enum and dispatch becomes a `match`.
//!
//! That substitution is not a compromise here — it is a better fit. The set of
//! resource types is closed and has been closed since Cyclus 1.0; upstream's
//! own `ResourceType` is a string compared against two constants. An enum
//! makes the exhaustiveness a compile error instead of a runtime string
//! comparison, and lets a third resource type be added without any call site
//! silently ignoring it.

use alloc::string::String;

use crate::error::{CyclusError, Result};
use crate::material::Material;
use crate::product::Product;

/// A traded resource: either a [`Material`] or a [`Product`].
#[derive(Debug, Clone, PartialEq)]
pub enum Resource {
    /// A mass of nuclides. Upstream `ResourceType == "Material"`.
    Material(Material),
    /// A non-nuclide resource with a quality string. Upstream
    /// `ResourceType == "Product"`.
    Product(Product),
}

/// Which concrete resource a [`Resource`] holds. Upstream `ResourceType`,
/// which is a `std::string`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceType {
    /// [`Resource::Material`].
    Material,
    /// [`Resource::Product`].
    Product,
}

impl ResourceType {
    /// The upstream `ResourceType` string, for output compatible with
    /// Cyclus's own tables.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Material => "Material",
            Self::Product => "Product",
        }
    }
}

impl Resource {
    /// Which concrete resource this is. Upstream `Resource::type()`.
    #[must_use]
    pub fn resource_type(&self) -> ResourceType {
        match self {
            Self::Material(_) => ResourceType::Material,
            Self::Product(_) => ResourceType::Product,
        }
    }

    /// The quantity — kilograms for a material, quality-defined units for a
    /// product. Upstream `Resource::quantity()`.
    #[must_use]
    pub fn quantity(&self) -> f64 {
        match self {
            Self::Material(m) => m.quantity(),
            Self::Product(p) => p.quantity(),
        }
    }

    /// The units of [`quantity`](Resource::quantity). Upstream
    /// `Resource::units()`.
    #[must_use]
    pub fn units(&self) -> &str {
        match self {
            Self::Material(m) => m.units(),
            Self::Product(p) => p.units(),
        }
    }

    /// The per-unit economic value. Upstream `Resource::unit_value()`.
    #[must_use]
    pub fn unit_value(&self) -> f64 {
        match self {
            Self::Material(m) => m.unit_value(),
            Self::Product(p) => p.unit_value(),
        }
    }

    /// `true` if the quantity is at or below the resource epsilon.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Material(m) => m.is_empty(),
            Self::Product(p) => p.is_empty(),
        }
    }

    /// Splits `qty` off this resource. Upstream `Resource::ExtractRes`.
    ///
    /// For a material this keeps the composition, as
    /// [`Material::extract_qty`] does.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `qty` is negative or exceeds the available
    /// quantity.
    pub fn extract_qty(&mut self, qty: f64) -> Result<Self> {
        match self {
            Self::Material(m) => Ok(Self::Material(m.extract_qty(qty)?)),
            Self::Product(p) => Ok(Self::Product(p.extract_qty(qty)?)),
        }
    }

    /// Borrows the inner material, or `None` if this is a product.
    #[must_use]
    pub fn as_material(&self) -> Option<&Material> {
        match self {
            Self::Material(m) => Some(m),
            Self::Product(_) => None,
        }
    }

    /// Mutably borrows the inner material, or `None` if this is a product.
    #[must_use]
    pub fn as_material_mut(&mut self) -> Option<&mut Material> {
        match self {
            Self::Material(m) => Some(m),
            Self::Product(_) => None,
        }
    }

    /// Borrows the inner product, or `None` if this is a material.
    #[must_use]
    pub fn as_product(&self) -> Option<&Product> {
        match self {
            Self::Product(p) => Some(p),
            Self::Material(_) => None,
        }
    }

    /// Consumes this resource and returns the inner material.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if this is a product. A fuel-cycle agent that
    /// asks for a material and is handed electricity has a configuration bug,
    /// and this is where it surfaces.
    pub fn into_material(self) -> Result<Material> {
        match self {
            Self::Material(m) => Ok(m),
            Self::Product(_) => Err(CyclusError::Value("expected a Material, found a Product")),
        }
    }

    /// Consumes this resource and returns the inner product.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if this is a material.
    pub fn into_product(self) -> Result<Product> {
        match self {
            Self::Product(p) => Ok(p),
            Self::Material(_) => Err(CyclusError::Value("expected a Product, found a Material")),
        }
    }

    /// The quality identifier used to decide whether two resources may be
    /// combined.
    ///
    /// For a product this is its quality string. For a material upstream uses
    /// the composition's integer id; here compositions have no global id (see
    /// [`composition`](crate::composition)), so a material reports `None` and
    /// callers compare compositions directly. Returning `None` rather than a
    /// fabricated id keeps the difference visible instead of inventing an
    /// identity that would not be stable across runs.
    #[must_use]
    pub fn quality_tag(&self) -> Option<String> {
        match self {
            Self::Material(_) => None,
            Self::Product(p) => Some(alloc::string::ToString::to_string(p.quality())),
        }
    }
}

impl From<Material> for Resource {
    fn from(m: Material) -> Self {
        Self::Material(m)
    }
}

impl From<Product> for Resource {
    fn from(p: Product) -> Self {
        Self::Product(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comp_math::CompMap;
    use crate::composition::{AtomicMasses, Composition};
    use crate::nuclide::nuc;

    fn material(qty: f64) -> Material {
        let map: CompMap = [(nuc::U235, 1.0)].into_iter().collect();
        let c = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
        Material::new(qty, c).unwrap()
    }

    #[test]
    fn dispatches_quantity_and_units_by_variant() {
        let m = Resource::from(material(10.0));
        assert_eq!(m.quantity(), 10.0);
        assert_eq!(m.units(), "kg");
        assert_eq!(m.resource_type(), ResourceType::Material);

        let p = Resource::from(Product::new(5.0, "MWh").unwrap());
        assert_eq!(p.quantity(), 5.0);
        assert_eq!(p.units(), "MWh");
        assert_eq!(p.resource_type(), ResourceType::Product);
    }

    #[test]
    fn resource_type_strings_match_upstream() {
        assert_eq!(ResourceType::Material.as_str(), "Material");
        assert_eq!(ResourceType::Product.as_str(), "Product");
    }

    #[test]
    fn extract_qty_works_through_the_enum() {
        let mut r = Resource::from(material(10.0));
        let got = r.extract_qty(4.0).unwrap();
        assert_eq!(r.quantity(), 6.0);
        assert_eq!(got.quantity(), 4.0);
        assert_eq!(got.resource_type(), ResourceType::Material);
    }

    #[test]
    fn asking_a_product_for_a_material_is_an_error() {
        let p = Resource::from(Product::new(1.0, "MWh").unwrap());
        assert!(p.as_material().is_none());
        assert_eq!(
            p.into_material().unwrap_err(),
            CyclusError::Value("expected a Material, found a Product")
        );
    }

    #[test]
    fn quality_tag_distinguishes_products_and_defers_for_materials() {
        let p = Resource::from(Product::new(1.0, "SWU").unwrap());
        assert_eq!(p.quality_tag().as_deref(), Some("SWU"));
        let m = Resource::from(material(1.0));
        assert!(m.quality_tag().is_none());
    }
}
