// SPDX-License-Identifier: GPL-3.0

//! The core radionuclide inventory: outram-mc's [`RadionuclideInventory`],
//! read in TRISO-ATOPS's terms.
//!
//! # One inventory type (GitHub #951)
//!
//! ~~This crate's own `CoreInventory` / `NuclideInventory`~~ **REPLACED
//! 2026-10-17 (maintainer: "Change sembawang to use RadionuclideInventory
//! instead"):** the inventory is `outram_mc_libs::inventory::RadionuclideInventory`,
//! the type a depletion run produces, re-exported here. What changed and what
//! did not:
//!
//! | `CoreInventory` held | Now |
//! |---|---|
//! | `name: String` per nuclide, a TRISO-ATOPS spelling | an [`Isotope`] (Z, A, metastable state). [`prescribed`] parses names through upstream's own normaliser, so `cs137` is still accepted; [`triso_atops_name`] spells it back (`Ag-110m`) |
//! | `per_ring: Vec<Radioactivity>` | one [`Amount`] per region, whose activity is the same `Radioactivity` value, untouched |
//! | `n_radial` | [`Regions::RadialRings`]; [`Regions::WholeCore`] reads as one ring |
//! | `n_axial` | **dropped**: it duplicated the temperature transient's `n_axial`, which is what the axial split has always used. The axial split itself is still upstream's `distribute_inventory_axially`, in [`axial_curies`] |
//!
//! So every number this crate produced before the change is reproduced bit
//! for bit (`tests/inventory_handoff_fingerprints.rs`). A non-canonical
//! spelling now comes back canonical in `AccidentRelease::screened_out`
//! (`cs137` is reported as `Cs-137`).
//!
//! # A prescribed inventory is still an INPUT, and this crate does not compute it
//!
//! Nothing here derives an inventory from power history, burnup or fission
//! yields. A caller prescribes one ([`prescribed`], [`unit()`]) or hands over
//! one a depletion code produced (`RadionuclideInventory::from_burnup_step`
//! in outram-mc, then [`restrict_to_triso_atops`]). The reason the obvious
//! shortcut is wrong is worth keeping, because it does not announce itself:
//!
//! **Do not build one from `fission-yields-data`.** That crate exposes
//! **independent** fission yields — the yield *directly* from fission, before
//! any beta decay. Most of the nuclides that dominate a source term do not
//! arrive that way. Cs-137's independent yield is roughly **two orders of
//! magnitude** below its cumulative yield, because Cs-137 is reached down the
//! A = 137 isobaric chain (Xe-137 to Cs-137) rather than produced directly.
//! Using independent yields naively gives a silently ~100x low inventory for
//! most consequence-dominant species — low, so it looks reassuring, and with
//! no error anywhere. A depletion run that walks the decay chain does not
//! have this problem; that is the outram-mc path.
//!
//! # Radial and axial distribution
//!
//! An inventory is given **per radial ring**, innermost first, and spread over
//! the axial nodes by `boon-lay`'s `distribute_inventory_axially`, which is
//! upstream's own distribution. This crate does not invent a shape. An
//! inventory over **named** regions (components, depletion zones) is refused
//! until the caller states the ring mapping with
//! `RadionuclideInventory::with_regions`.

use boon_lay::triso_atops_fork::accident::distribute_inventory_axially;
use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::find_nuclide;
use boon_lay::triso_atops_fork::run_selection::normalise_nuclide_name;
use uom::si::f64::Radioactivity;

pub use outram_mc_libs::inventory::{
    zai, Amount, InventoryError, InventoryPoint, InventorySource, NuclideEntry,
    RadionuclideInventory, Regions,
};
pub use outram_mc_libs::model::Isotope;

use crate::error::{Error, Result};
use crate::units;

/// The TRISO-ATOPS spelling of `isotope`: `Cs-137`, `Ag-110m` (first isomer
/// `m`). A second or higher isomer is spelled `m2`, ...; no TRISO-ATOPS
/// nuclide is one and upstream's name parser does not accept that spelling,
/// so such a nuclide is refused by the release as unknown (reported, never
/// dropped).
#[must_use]
pub fn triso_atops_name(isotope: Isotope) -> String {
    let gnds = isotope.name();
    let symbol: String = gnds.chars().take_while(char::is_ascii_alphabetic).collect();
    match isotope.metastable_state() {
        0 => format!("{symbol}-{}", isotope.a()),
        1 => format!("{symbol}-{}m", isotope.a()),
        m => format!("{symbol}-{}m{m}", isotope.a()),
    }
}

/// Parse a nuclide name the way upstream's `nuclide_import_accident` does
/// (`cs137`, `CS-137` and `Cs-137` are one nuclide), into an [`Isotope`].
///
/// # Errors
/// [`Error::UnknownNuclide`] when the name does not parse or names no nuclide.
pub fn isotope_from_triso_atops_name(name: &str) -> Result<Isotope> {
    let unknown = || Error::UnknownNuclide(name.to_string());
    let canonical = normalise_nuclide_name(name).map_err(|_| unknown())?;
    let (symbol, rest) = canonical.split_once('-').ok_or_else(unknown)?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    // Upstream's normaliser knows only a ground state and a first isomer.
    let state = match &rest[digits.len()..] {
        "" => 0,
        "m" => 1,
        _ => return Err(unknown()),
    };
    let gnds = if state == 0 {
        format!("{symbol}{digits}")
    } else {
        format!("{symbol}{digits}_m{state}")
    };
    Isotope::from_name(&gnds).map_err(|_| unknown())
}

/// **A prescribed inventory**: activity per radial ring, innermost first, by
/// nuclide name, in the given order (a parent must precede its daughter for
/// the normal-operation solve to see its pools). `reference` says where the
/// numbers come from.
///
/// # Errors
/// [`Error::UnknownNuclide`] for a name that does not parse, and
/// [`Error::Inventory`] for an empty list, a ring count that disagrees with
/// `n_radial`, a negative or non-finite activity, or a nuclide given twice.
pub fn prescribed<S: AsRef<str>>(
    entries: Vec<(S, Vec<Radioactivity>)>,
    n_radial: usize,
    reference: &str,
) -> Result<RadionuclideInventory> {
    let activities = entries
        .into_iter()
        .map(|(name, per_ring)| Ok((isotope_from_triso_atops_name(name.as_ref())?, per_ring)))
        .collect::<Result<Vec<_>>>()?;
    Ok(RadionuclideInventory::from_activities(
        Regions::RadialRings(n_radial),
        activities,
        InventoryPoint::unspecified(),
        InventorySource::Prescribed {
            reference: reference.to_string(),
        },
    )?)
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
///
/// # Errors
/// As [`prescribed`].
pub fn unit(names: &[&str], n_radial: usize) -> Result<RadionuclideInventory> {
    prescribed(
        names
            .iter()
            .map(|n| (*n, vec![units::from_curies(1.0); n_radial]))
            .collect(),
        n_radial,
        "shakedown: exactly 1 Ci of each nuclide per ring (CoreInventory::unit before gh:#951)",
    )
}

/// The number of radial rings `inventory` is resolved into.
///
/// # Errors
/// [`Error::InventoryNotInRings`] for [`Regions::Named`]: which ring a
/// component is in is the caller's statement to make
/// (`RadionuclideInventory::with_regions`), not this crate's guess.
pub fn n_rings(inventory: &RadionuclideInventory) -> Result<usize> {
    match inventory.regions() {
        Regions::WholeCore => Ok(1),
        Regions::RadialRings(n) => Ok(*n),
        Regions::Named(names) => Err(Error::InventoryNotInRings(names.clone())),
    }
}

/// The TRISO-ATOPS names of every nuclide, in order, for handing to
/// `select_nuclides_accident`.
#[must_use]
pub fn names(inventory: &RadionuclideInventory) -> Vec<String> {
    inventory
        .nuclides()
        .iter()
        .map(|e| triso_atops_name(e.isotope()))
        .collect()
}

/// One nuclide's inventory in one ring, spread over `n_axial` nodes.
///
/// Delegates to upstream's `distribute_inventory_axially`; the returned
/// values are in **curies**, which is the unit TRISO-ATOPS works in
/// internally.
///
/// # Errors
/// [`Error::InventoryRingOutOfRange`] if `ring` is past the last region.
pub fn axial_curies(entry: &NuclideEntry, ring: usize, n_axial: usize) -> Result<Vec<f64>> {
    let activity = entry
        .activity_in(ring)
        .ok_or(Error::InventoryRingOutOfRange {
            ring,
            rings: entry.per_region().len(),
        })?;
    Ok(distribute_inventory_axially(
        units::in_curies(activity),
        n_axial,
    ))
}

/// Keep the nuclides TRISO-ATOPS models (its 84-entry table), in order, and
/// **report** the rest rather than dropping them silently: the path for an
/// inventory from depletion, which carries every chain nuclide, stable ones
/// included.
///
/// # Errors
/// [`Error::Inventory`] if no nuclide is left.
pub fn restrict_to_triso_atops(
    inventory: &RadionuclideInventory,
) -> Result<(RadionuclideInventory, Vec<Isotope>)> {
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for e in inventory.nuclides() {
        if find_nuclide(&triso_atops_name(e.isotope())).is_some() {
            kept.push((e.isotope(), e.per_region().to_vec()));
        } else {
            dropped.push(e.isotope());
        }
    }
    let restricted = RadionuclideInventory::new(
        inventory.regions().clone(),
        kept,
        *inventory.point(),
        inventory.source().clone(),
    )?;
    Ok((restricted, dropped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use boon_lay::triso_atops_fork::nuclide_model::nuclide_database::supported_nuclides;

    #[test]
    fn the_unit_inventory_is_one_curie_per_ring_per_nuclide() {
        let inv = unit(&["Kr-88", "I-131"], 3).unwrap();
        assert_eq!(inv.nuclides().len(), 2);
        assert_eq!(n_rings(&inv).unwrap(), 3);
        for n in inv.nuclides() {
            assert_eq!(n.per_region().len(), 3);
            // Three rings of one curie each.
            assert!((units::in_curies(n.total_activity()) - 3.0).abs() < 1e-9);
        }
    }

    #[test]
    fn the_axial_distribution_conserves_the_ring_total() {
        let inv = unit(&["Cs-137"], 2).unwrap();
        let axial = axial_curies(&inv.nuclides()[0], 0, 5).unwrap();
        assert_eq!(axial.len(), 5);
        let sum: f64 = axial.iter().sum();
        assert!(
            (sum - 1.0).abs() < 1e-9,
            "the axial split must conserve the ring's 1 Ci; got {sum}"
        );
        assert_eq!(
            axial_curies(&inv.nuclides()[0], 2, 5),
            Err(Error::InventoryRingOutOfRange { ring: 2, rings: 2 })
        );
    }

    #[test]
    fn a_nuclide_with_the_wrong_ring_count_is_rejected() {
        let r = prescribed(vec![("Kr-88", vec![units::from_curies(1.0); 2])], 3, "t");
        assert!(matches!(
            r,
            Err(Error::Inventory(InventoryError::RegionCountMismatch {
                expected: 3,
                got: 2,
                ..
            }))
        ));
        assert!(matches!(
            prescribed::<&str>(vec![], 1, "t"),
            Err(Error::Inventory(InventoryError::Empty))
        ));
    }

    #[test]
    fn names_come_back_in_order_for_the_selection_call() {
        let inv = unit(&["Kr-88", "I-131", "Cs-137"], 1).unwrap();
        assert_eq!(names(&inv), vec!["Kr-88", "I-131", "Cs-137"]);
    }

    /// Every one of TRISO-ATOPS's 84 names, metastable ones included, goes to
    /// an [`Isotope`] and back to the same spelling; non-canonical spellings
    /// land on the canonical one; names that are not nuclides are errors.
    #[test]
    fn every_triso_atops_name_round_trips_through_an_isotope() {
        let all = supported_nuclides();
        assert_eq!(all.len(), 84);
        for n in &all {
            let iso = isotope_from_triso_atops_name(n.name).unwrap();
            assert_eq!(triso_atops_name(iso), n.name);
            assert_eq!(u32::from(iso.z()), u32::from(n.z), "{}", n.name);
        }
        let ag = isotope_from_triso_atops_name("ag110m").unwrap();
        assert_eq!((ag.z(), ag.a(), ag.metastable_state()), (47, 110, 1));
        assert_eq!(zai(ag), 471_101);
        assert_eq!(
            triso_atops_name(Isotope::metastable(52, 127, 2).unwrap()),
            "Te-127m2"
        );
        for bad in ["???", "137", "Xx-999", "Cs-137x", "Te-127m2"] {
            assert!(isotope_from_triso_atops_name(bad).is_err(), "{bad}");
        }
    }

    /// Named regions are refused until the caller maps them to rings; the
    /// whole core reads as one ring.
    #[test]
    fn only_ring_or_whole_core_inventories_have_a_ring_count() {
        let inv = unit(&["Cs-137"], 2).unwrap();
        let named = inv
            .clone()
            .with_regions(Regions::Named(vec!["a".into(), "b".into()]))
            .unwrap();
        assert_eq!(
            n_rings(&named),
            Err(Error::InventoryNotInRings(vec!["a".into(), "b".into()]))
        );
        let whole = unit(&["Cs-137"], 1)
            .unwrap()
            .with_regions(Regions::WholeCore)
            .unwrap();
        assert_eq!(n_rings(&whole), Ok(1));
    }

    /// Restriction keeps order and amounts and lists, not drops, the rest.
    #[test]
    fn restriction_to_triso_atops_reports_what_it_leaves_out() {
        let u235 = Isotope::new(92, 235).unwrap();
        let inv = RadionuclideInventory::from_activities(
            Regions::RadialRings(1),
            vec![
                (u235, vec![units::from_curies(0.0)]),
                (
                    isotope_from_triso_atops_name("I-131").unwrap(),
                    vec![units::from_curies(2.0)],
                ),
                (
                    isotope_from_triso_atops_name("Cs-137").unwrap(),
                    vec![units::from_curies(3.0)],
                ),
            ],
            InventoryPoint::unspecified(),
            InventorySource::Prescribed {
                reference: "t".into(),
            },
        )
        .unwrap();
        let (kept, dropped) = restrict_to_triso_atops(&inv).unwrap();
        assert_eq!(names(&kept), vec!["I-131", "Cs-137"]);
        assert_eq!(
            kept.nuclides()[1].per_region(),
            inv.nuclides()[2].per_region()
        );
        assert_eq!(kept.source(), inv.source());
        assert_eq!(dropped, vec![u235]);
        let only_u = RadionuclideInventory::from_activities(
            Regions::WholeCore,
            vec![(u235, vec![units::from_curies(0.0)])],
            InventoryPoint::unspecified(),
            InventorySource::Prescribed {
                reference: "t".into(),
            },
        )
        .unwrap();
        assert_eq!(
            restrict_to_triso_atops(&only_u),
            Err(Error::Inventory(InventoryError::Empty))
        );
    }
}
