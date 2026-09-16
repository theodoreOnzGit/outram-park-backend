// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/product.h, src/product.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Product: a traded resource that is not made of nuclides.
//!
//! Electricity, separative work units, money, cooling water. A [`Product`] has
//! a quantity and a **quality** — an opaque string that plays the role a
//! composition plays for [`Material`](crate::material::Material): two products
//! may only be combined if their qualities match.

use alloc::string::{String, ToString};

use crate::error::{CyclusError, Result};
use crate::limits::EPS_RSRC;

/// A quantity of some non-nuclide resource.
///
/// Units are whatever the quality implies — upstream's `Product::units()`
/// returns the quality string itself, and that convention is kept here.
#[derive(Debug, Clone, PartialEq)]
pub struct Product {
    qty: f64,
    quality: String,
    unit_value: f64,
}

impl Product {
    /// Creates `qty` units of a product of the given `quality`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `qty` is negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use outram_park_fork_cyclus::product::Product;
    ///
    /// let p = Product::new(1500.0, "MWh")?;
    /// assert_eq!(p.quantity(), 1500.0);
    /// assert_eq!(p.quality(), "MWh");
    /// # Ok::<(), outram_park_fork_cyclus::error::CyclusError>(())
    /// ```
    pub fn new(qty: f64, quality: &str) -> Result<Self> {
        if qty < 0.0 {
            return Err(CyclusError::Value("product quantity cannot be negative"));
        }
        Ok(Self {
            qty,
            quality: quality.to_string(),
            unit_value: 1.0,
        })
    }

    /// The quantity, in whatever units [`quality`](Product::quality) implies.
    #[must_use]
    pub fn quantity(&self) -> f64 {
        self.qty
    }

    /// The quality string. Upstream `Product::quality()`.
    #[must_use]
    pub fn quality(&self) -> &str {
        &self.quality
    }

    /// The units, which upstream defines to be the quality string.
    #[must_use]
    pub fn units(&self) -> &str {
        &self.quality
    }

    /// The per-unit economic value.
    #[must_use]
    pub fn unit_value(&self) -> f64 {
        self.unit_value
    }

    /// Sets the per-unit economic value.
    pub fn set_unit_value(&mut self, v: f64) {
        self.unit_value = v;
    }

    /// Splits `qty` units off this product. Upstream `Product::ExtractQty`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `qty` is negative or exceeds the available
    /// quantity.
    pub fn extract_qty(&mut self, qty: f64) -> Result<Self> {
        if qty < 0.0 {
            return Err(CyclusError::Value("cannot extract a negative quantity"));
        }
        if self.qty < qty {
            return Err(CyclusError::Value("extraction causes negative quantity"));
        }
        self.qty -= qty;
        Ok(Self {
            qty,
            quality: self.quality.clone(),
            unit_value: self.unit_value,
        })
    }

    /// Merges `other` into this product. Upstream `Product::Absorb`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if the qualities differ. Upstream raises the
    /// same error: there is no meaningful blend of 1 MWh and 1 SWU.
    pub fn absorb(&mut self, other: Self) -> Result<()> {
        if self.quality != other.quality {
            return Err(CyclusError::Value(
                "cannot absorb a product of a different quality",
            ));
        }
        let tot = self.qty + other.qty;
        if tot > 0.0 {
            self.unit_value = (self.qty * self.unit_value + other.qty * other.unit_value) / tot;
        }
        self.qty = tot;
        Ok(())
    }

    /// `true` if the quantity is at or below the resource epsilon.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.qty <= EPS_RSRC
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_conserves_quantity_and_quality() {
        let mut p = Product::new(100.0, "MWh").unwrap();
        let got = p.extract_qty(40.0).unwrap();
        assert_eq!(p.quantity(), 60.0);
        assert_eq!(got.quantity(), 40.0);
        assert_eq!(got.quality(), "MWh");
    }

    #[test]
    fn extract_rejects_more_than_is_present() {
        let mut p = Product::new(1.0, "SWU").unwrap();
        assert_eq!(
            p.extract_qty(2.0),
            Err(CyclusError::Value("extraction causes negative quantity"))
        );
        assert_eq!(p.quantity(), 1.0);
    }

    #[test]
    fn absorb_requires_matching_quality() {
        let mut a = Product::new(1.0, "MWh").unwrap();
        let b = Product::new(1.0, "SWU").unwrap();
        assert_eq!(
            a.absorb(b),
            Err(CyclusError::Value(
                "cannot absorb a product of a different quality"
            ))
        );
    }

    #[test]
    fn absorb_sums_matching_qualities_and_weights_unit_value() {
        let mut a = Product::new(10.0, "MWh").unwrap();
        a.set_unit_value(100.0);
        let mut b = Product::new(30.0, "MWh").unwrap();
        b.set_unit_value(20.0);
        a.absorb(b).unwrap();
        assert_eq!(a.quantity(), 40.0);
        assert!((a.unit_value() - 40.0).abs() < 1e-12);
    }

    #[test]
    fn units_is_the_quality_string() {
        let p = Product::new(1.0, "SWU").unwrap();
        assert_eq!(p.units(), "SWU");
    }

    #[test]
    fn new_rejects_a_negative_quantity() {
        assert_eq!(
            Product::new(-1.0, "MWh").unwrap_err(),
            CyclusError::Value("product quantity cannot be negative")
        );
    }
}
