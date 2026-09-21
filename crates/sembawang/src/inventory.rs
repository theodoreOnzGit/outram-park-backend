// SPDX-License-Identifier: GPL-3.0

//! The core radionuclide inventory, prescribed.
//!
//! # This is an INPUT, and this crate does not compute it
//!
//! Nothing here derives an inventory from power history, burnup or fission
//! yields. It is supplied by the caller, and the reason is worth stating
//! because the obvious shortcut is wrong in a way that does not announce
//! itself:
//!
//! **Do not build one from `fission-yields-data`.** That crate exposes
//! **independent** fission yields — the yield *directly* from fission, before
//! any beta decay. Most of the nuclides that dominate a source term do not
//! arrive that way. Cs-137's independent yield is roughly **two orders of
//! magnitude** below its cumulative yield, because Cs-137 is reached down the
//! A = 137 isobaric chain (Xe-137 to Cs-137) rather than produced directly.
//! Using independent yields naively gives a silently ~100x low inventory for
//! most consequence-dominant species — low, so it looks reassuring, and with
//! no error anywhere.
//!
//! Getting a cumulative yield right needs a decay-chain walk over the
//! evaluation. That is real work and is deliberately out of scope here.
//!
//! # Radial and axial distribution
//!
//! An inventory is given **per radial ring**, and spread over the axial nodes
//! by `boon-lay`'s `distribute_inventory_axially`, which is upstream's own
//! distribution. This crate does not invent a shape.

use boon_lay::triso_atops_fork::accident::distribute_inventory_axially;
use uom::si::f64::Radioactivity;

use crate::units;

/// One nuclide's core inventory, before the accident starts.
#[derive(Debug, Clone, PartialEq)]
pub struct NuclideInventory {
    /// TRISO-ATOPS nuclide name, e.g. `"Cs-137"`. Must be one of the 84 in
    /// `boon_lay::triso_atops_fork::nuclide_model::nuclide_database`.
    pub name: String,
    /// Activity in each radial ring, innermost first. Length sets `n_radial`.
    pub per_ring: Vec<Radioactivity>,
}

impl NuclideInventory {
    /// Every ring carrying the same activity.
    #[must_use]
    pub fn uniform(name: &str, per_ring: Radioactivity, n_radial: usize) -> Self {
        Self {
            name: name.to_string(),
            per_ring: vec![per_ring; n_radial],
        }
    }

    /// This nuclide's total across every ring.
    #[must_use]
    pub fn total(&self) -> Radioactivity {
        let bq: f64 = self
            .per_ring
            .iter()
            .map(|a| a.get::<uom::si::radioactivity::becquerel>())
            .sum();
        Radioactivity::new::<uom::si::radioactivity::becquerel>(bq)
    }

    /// This nuclide's inventory in one ring, spread over the axial nodes.
    ///
    /// Delegates to upstream's `distribute_inventory_axially`; the returned
    /// values are in **curies**, which is the unit TRISO-ATOPS works in
    /// internally.
    ///
    /// # Panics
    /// Panics if `ring` is out of range.
    #[must_use]
    pub fn axial_curies(&self, ring: usize, n_axial: usize) -> Vec<f64> {
        distribute_inventory_axially(units::in_curies(self.per_ring[ring]), n_axial)
    }
}

/// The whole core's inventory: every nuclide, resolved by radial ring.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreInventory {
    /// One entry per nuclide.
    pub nuclides: Vec<NuclideInventory>,
    /// Number of radial rings. Every nuclide must supply this many.
    pub n_radial: usize,
    /// Number of axial nodes each ring is split into.
    pub n_axial: usize,
}

impl CoreInventory {
    /// Build and validate.
    ///
    /// # Panics
    /// Panics if the nuclide list is empty, if any nuclide's ring count
    /// disagrees with `n_radial`, if any activity is negative, or if either
    /// node count is zero.
    #[must_use]
    pub fn new(nuclides: Vec<NuclideInventory>, n_radial: usize, n_axial: usize) -> Self {
        assert!(!nuclides.is_empty(), "a core inventory needs at least one nuclide");
        assert!(n_radial > 0 && n_axial > 0, "need at least one node in each direction");
        for n in &nuclides {
            assert_eq!(
                n.per_ring.len(),
                n_radial,
                "nuclide {} supplies {} rings for n_radial = {n_radial}",
                n.name,
                n.per_ring.len()
            );
            for (r, a) in n.per_ring.iter().enumerate() {
                assert!(
                    a.get::<uom::si::radioactivity::becquerel>() >= 0.0,
                    "nuclide {} has a negative inventory in ring {r}",
                    n.name
                );
            }
        }
        Self {
            nuclides,
            n_radial,
            n_axial,
        }
    }

    /// **A shakedown inventory: exactly one curie of each nuclide, per ring.**
    ///
    /// Not a physical inventory and not intended to be one. It exists so the
    /// chain can be run and its *shape* checked — the arithmetic is linear in
    /// the inventory, so a run with this and a run with a real one differ only
    /// by a per-nuclide scale factor. Anything reported from it is a
    /// demonstration, never a result.
    ///
    /// Use it to answer "does the plumbing work", never "how much comes out".
    #[must_use]
    pub fn unit(names: &[&str], n_radial: usize, n_axial: usize) -> Self {
        Self::new(
            names
                .iter()
                .map(|n| NuclideInventory::uniform(n, units::from_curies(1.0), n_radial))
                .collect(),
            n_radial,
            n_axial,
        )
    }

    /// The nuclide names, in order, for handing to
    /// `select_nuclides_accident`.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.nuclides.iter().map(|n| n.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unit_inventory_is_one_curie_per_ring_per_nuclide() {
        let inv = CoreInventory::unit(&["Kr-88", "I-131"], 3, 4);
        assert_eq!(inv.nuclides.len(), 2);
        for n in &inv.nuclides {
            assert_eq!(n.per_ring.len(), 3);
            // Three rings of one curie each.
            assert!((units::in_curies(n.total()) - 3.0).abs() < 1e-9);
        }
    }

    #[test]
    fn the_axial_distribution_conserves_the_ring_total() {
        let inv = CoreInventory::unit(&["Cs-137"], 2, 5);
        let axial = inv.nuclides[0].axial_curies(0, 5);
        assert_eq!(axial.len(), 5);
        let sum: f64 = axial.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-9,
            "the axial split must conserve the ring's 1 Ci; got {sum}"
        );
    }

    #[test]
    #[should_panic(expected = "rings for n_radial")]
    fn a_nuclide_with_the_wrong_ring_count_is_rejected() {
        let _ = CoreInventory::new(
            vec![NuclideInventory::uniform("Kr-88", units::from_curies(1.0), 2)],
            3,
            4,
        );
    }

    #[test]
    fn names_come_back_in_order_for_the_selection_call() {
        let inv = CoreInventory::unit(&["Kr-88", "I-131", "Cs-137"], 1, 1);
        assert_eq!(inv.names(), vec!["Kr-88", "I-131", "Cs-137"]);
    }
}
