// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/composition.h, src/composition.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Nuclide compositions in both atom and mass bases.
//!
//! A [`Composition`] is an immutable pair of [`CompMap`]s describing the same
//! material two ways — per atom and per unit mass. Neither is normalised: a
//! composition describes *ratios*, and the absolute scale lives on the
//! [`Material`](crate::material::Material) that carries it.
//!
//! # Where the atomic masses come from
//!
//! Upstream converts between the two bases lazily, calling
//! `pyne::atomic_mass(nuc)` against a table compiled into the binary. This
//! crate ships **no nuclear data** — the workspace rule is that all of it lives
//! in `njoy-outram-park-fork` — so the conversion takes an explicit
//! [`AtomicMasses`] argument instead, and the caller says where the masses come
//! from. [`AtomicMasses::MassNumber`] is the zero-data option, good to a few
//! tenths of a percent, and is what the tests here use.
//!
//! # Three divergences from upstream, all deliberate
//!
//! 1. **Both bases are computed eagerly**, at construction. Upstream computes
//!    the second basis on first access and caches it by mutating the object
//!    through a non-`const` accessor. Doing that here would need interior
//!    mutability, which in `no_std` means a lock or a `Cell`, for no benefit:
//!    the conversion is one multiply per nuclide and every composition in a
//!    running simulation gets asked for both bases sooner or later.
//!
//! 2. **There is no `id()` and no global counter.** Upstream's `next_id_` is a
//!    mutable process global, which `no_std` has no good way to provide and
//!    which makes two runs of the same input disagree on ids if anything is
//!    ever constructed in a different order. Identity is handled by
//!    [`Context`](crate::context::Context), which interns compositions and
//!    hands out a [`CompId`]; a composition that has never been recorded
//!    simply has no id.
//!
//! 3. **There is no cached decay chain.** Upstream threads a shared
//!    `decay_line_` map through every composition descended from a common
//!    ancestor, memoising decay results. That is a `shared_ptr` graph of
//!    exactly the kind this workspace's rules exclude, and the memo is a cache,
//!    not semantics. [`decay`](crate::decay) recomputes; a caller who wants the
//!    memo can hold one.

use alloc::collections::BTreeMap;

use crate::comp_math::{self, CompMap};
use crate::error::{CyclusError, Result};
use crate::nuclide::Nuc;

/// An identifier for a composition that has been recorded by a
/// [`Context`](crate::context::Context).
///
/// Corresponds to upstream's `Composition::id()` / the `QualId` column in the
/// output database. Two compositions built separately from the same
/// [`CompMap`] get different ids, exactly as upstream documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CompId(pub u64);

/// A source of atomic masses, in unified atomic mass units (u).
///
/// An enum rather than a trait, per the workspace's no-trait-objects rule, and
/// the closed set is genuine: either you have a table of measured masses or
/// you are approximating by mass number. There is no third case.
#[derive(Debug, Clone, PartialEq)]
pub enum AtomicMasses {
    /// Approximate each nuclide's atomic mass by its mass number `A`.
    ///
    /// Carries no data, which is why it is the default in tests and examples.
    /// The error is the mass defect — under 1 % everywhere and under 0.1 % for
    /// the actinides that dominate a fuel cycle — so it is fine for a
    /// structural test and **not** fine for a mass balance anyone will quote.
    /// Use [`AtomicMasses::Table`] with evaluated masses for that.
    ///
    /// A natural-element id (`A == 0`) has no mass number to use, so
    /// [`AtomicMasses::mass_of`] reports
    /// [`CyclusError::InvalidNuclide`] for one.
    MassNumber,

    /// Explicit atomic masses in u, keyed by nuclide.
    ///
    /// Populate this from `njoy-outram-park-fork`, or from any evaluated
    /// nuclear data library. A nuclide absent from the table is an error, not
    /// a silent zero.
    Table(BTreeMap<Nuc, f64>),
}

impl AtomicMasses {
    /// Builds a table from `(nuclide, mass in u)` pairs.
    #[must_use]
    pub fn from_pairs(pairs: &[(Nuc, f64)]) -> Self {
        Self::Table(pairs.iter().copied().collect())
    }

    /// The atomic mass of `nuc`, in u.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::InvalidNuclide`] if `nuc` has no mass number and this
    ///   is [`AtomicMasses::MassNumber`].
    /// - [`CyclusError::Key`] if `nuc` is absent from a
    ///   [`AtomicMasses::Table`].
    pub fn mass_of(&self, nuc: Nuc) -> Result<f64> {
        match self {
            Self::MassNumber => {
                let a = nuc.a();
                if a <= 0 {
                    Err(CyclusError::InvalidNuclide(nuc.raw()))
                } else {
                    Ok(f64::from(a))
                }
            }
            Self::Table(t) => t
                .get(&nuc)
                .copied()
                .ok_or(CyclusError::Key("nuclide absent from the atomic mass table")),
        }
    }
}

/// An immutable nuclide composition, held in both atom and mass bases.
///
/// Built through [`Composition::from_atom`], [`Composition::from_mass`] or
/// [`Composition::from_nuclide`], each of which validates that every key is a
/// real nuclide and that no quantity is negative — the same two checks
/// upstream's factory functions make.
///
/// Neither basis is normalised. To read fractions rather than raw ratios, use
/// [`Composition::atom_frac`] and [`Composition::mass_frac`], or
/// [`MatQuery`](crate::toolkit::mat_query::MatQuery) for quantities on a
/// specific material.
#[derive(Debug, Clone, PartialEq)]
pub struct Composition {
    atom: CompMap,
    mass: CompMap,
}

impl Composition {
    /// Creates a composition whose components are in atom ratios.
    ///
    /// `v` need not be normalised to any particular value.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::InvalidNuclide`] if any key is not a nuclide.
    /// - [`CyclusError::Value`] if any quantity is negative.
    /// - Whatever [`AtomicMasses::mass_of`] reports, for the mass-basis
    ///   conversion.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaki_bukit::composition::{AtomicMasses, Composition};
    /// use kaki_bukit::comp_math::CompMap;
    /// use kaki_bukit::nuclide::nuc;
    ///
    /// // Uranium dioxide, as in the upstream header's own example.
    /// let mut v = CompMap::new();
    /// v.insert(nuc::U235, 2.4);
    /// v.insert(nuc::O16, 4.8);
    /// let c = Composition::from_atom(v, &AtomicMasses::MassNumber)?;
    ///
    /// // Twice as many oxygen atoms as uranium atoms.
    /// assert!((c.atom_frac(nuc::O16) - 2.0 / 3.0).abs() < 1e-12);
    /// # Ok::<(), kaki_bukit::error::CyclusError>(())
    /// ```
    pub fn from_atom(v: CompMap, masses: &AtomicMasses) -> Result<Self> {
        Self::validate(&v)?;
        let mut mass = CompMap::new();
        for (&nuc, &qty) in &v {
            mass.insert(nuc, qty * masses.mass_of(nuc)?);
        }
        Ok(Self { atom: v, mass })
    }

    /// Creates a composition whose components are in mass ratios.
    ///
    /// `v` need not be normalised to any particular value.
    ///
    /// # Errors
    ///
    /// As [`Composition::from_atom`].
    pub fn from_mass(v: CompMap, masses: &AtomicMasses) -> Result<Self> {
        Self::validate(&v)?;
        let mut atom = CompMap::new();
        for (&nuc, &qty) in &v {
            atom.insert(nuc, qty / masses.mass_of(nuc)?);
        }
        Ok(Self { atom, mass: v })
    }

    /// Creates a composition of one pure nuclide. Upstream
    /// `Composition::CreateFromNuclide`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::InvalidNuclide`] if `nuc` is not a nuclide.
    pub fn from_nuclide(nuc: Nuc, masses: &AtomicMasses) -> Result<Self> {
        if !nuc.is_nuclide() {
            return Err(CyclusError::InvalidNuclide(nuc.raw()));
        }
        let mut v = CompMap::new();
        v.insert(nuc, 1.0);
        Self::from_atom(v, masses)
    }

    /// The two checks upstream's factories make, in upstream's order.
    fn validate(v: &CompMap) -> Result<()> {
        if let Some(bad) = comp_math::first_invalid_nuc(v) {
            return Err(CyclusError::InvalidNuclide(bad.raw()));
        }
        if !comp_math::all_positive(v) {
            return Err(CyclusError::Value("negative quantity in composition"));
        }
        Ok(())
    }

    /// The unnormalised atom-basis composition. Upstream `Composition::atom()`.
    #[must_use]
    pub fn atom(&self) -> &CompMap {
        &self.atom
    }

    /// The unnormalised mass-basis composition. Upstream `Composition::mass()`.
    #[must_use]
    pub fn mass(&self) -> &CompMap {
        &self.mass
    }

    /// The atom fraction of `nuc`: its atom quantity over the total.
    ///
    /// Returns `0.0` for a nuclide that is absent, and for an empty
    /// composition. Upstream's `MatQuery::atom_frac` behaves the same way.
    #[must_use]
    pub fn atom_frac(&self, nuc: Nuc) -> f64 {
        frac(&self.atom, nuc)
    }

    /// The mass fraction of `nuc`: its mass quantity over the total.
    ///
    /// Returns `0.0` for an absent nuclide or an empty composition.
    #[must_use]
    pub fn mass_frac(&self, nuc: Nuc) -> f64 {
        frac(&self.mass, nuc)
    }

    /// The atom-basis composition normalised to sum to 1.
    #[must_use]
    pub fn normalized_atom(&self) -> CompMap {
        let mut v = self.atom.clone();
        comp_math::normalize(&mut v, 1.0);
        v
    }

    /// The mass-basis composition normalised to sum to 1.
    ///
    /// This is the form upstream records to the `Compositions` output table.
    #[must_use]
    pub fn normalized_mass(&self) -> CompMap {
        let mut v = self.mass.clone();
        comp_math::normalize(&mut v, 1.0);
        v
    }

    /// `true` if the composition holds no nuclides.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.atom.is_empty()
    }

    /// The number of nuclides tracked.
    #[must_use]
    pub fn len(&self) -> usize {
        self.atom.len()
    }

    /// `true` if this and `other` agree in the mass basis to within a relative
    /// `threshold`.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `threshold` is negative.
    pub fn almost_eq(&self, other: &Self, threshold: f64) -> Result<bool> {
        comp_math::almost_eq(&self.mass, &other.mass, threshold)
    }
}

/// `v[nuc] / sum(v)`, or `0.0` when the total is zero.
fn frac(v: &CompMap, nuc: Nuc) -> f64 {
    let total = comp_math::sum(v);
    if total == 0.0 {
        return 0.0;
    }
    v.get(&nuc).copied().unwrap_or(0.0) / total
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nuclide::nuc;

    fn comp(entries: &[(Nuc, f64)]) -> CompMap {
        entries.iter().copied().collect()
    }

    #[test]
    fn uo2_from_atom_has_two_oxygens_per_uranium() {
        let c = Composition::from_atom(
            comp(&[(nuc::U235, 2.4), (nuc::O16, 4.8)]),
            &AtomicMasses::MassNumber,
        )
        .unwrap();
        assert!((c.atom_frac(nuc::O16) - 2.0 / 3.0).abs() < 1e-12);
        assert!((c.atom_frac(nuc::U235) - 1.0 / 3.0).abs() < 1e-12);
    }

    #[test]
    fn mass_basis_follows_from_atom_basis_by_atomic_mass() {
        let c = Composition::from_atom(
            comp(&[(nuc::U235, 1.0), (nuc::O16, 2.0)]),
            &AtomicMasses::MassNumber,
        )
        .unwrap();
        // 1 atom of A=235 and 2 atoms of A=16 -> 235 and 32 mass units.
        assert!((c.mass()[&nuc::U235] - 235.0).abs() < 1e-12);
        assert!((c.mass()[&nuc::O16] - 32.0).abs() < 1e-12);
        assert!((c.mass_frac(nuc::U235) - 235.0 / 267.0).abs() < 1e-12);
    }

    #[test]
    fn from_mass_and_from_atom_round_trip() {
        let masses = AtomicMasses::MassNumber;
        let a = Composition::from_atom(comp(&[(nuc::U235, 1.0), (nuc::O16, 2.0)]), &masses).unwrap();
        let b = Composition::from_mass(a.mass().clone(), &masses).unwrap();
        assert!(a.almost_eq(&b, 1e-12).unwrap());
        for (nuc, qty) in a.atom() {
            assert!((b.atom()[nuc] - qty).abs() < 1e-12);
        }
    }

    #[test]
    fn rejects_an_invalid_nuclide() {
        let e = Composition::from_atom(comp(&[(nuc::U_NATURAL, 1.0)]), &AtomicMasses::MassNumber);
        assert_eq!(e, Err(CyclusError::InvalidNuclide(nuc::U_NATURAL.raw())));
    }

    #[test]
    fn rejects_a_negative_quantity() {
        let e = Composition::from_atom(comp(&[(nuc::U235, -1.0)]), &AtomicMasses::MassNumber);
        assert_eq!(e, Err(CyclusError::Value("negative quantity in composition")));
    }

    #[test]
    fn from_nuclide_is_a_pure_single_nuclide() {
        let c = Composition::from_nuclide(nuc::U235, &AtomicMasses::MassNumber).unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c.atom_frac(nuc::U235), 1.0);
        assert_eq!(c.mass_frac(nuc::U235), 1.0);
    }

    #[test]
    fn fractions_of_an_absent_nuclide_are_zero() {
        let c = Composition::from_atom(comp(&[(nuc::U235, 1.0)]), &AtomicMasses::MassNumber).unwrap();
        assert_eq!(c.atom_frac(nuc::PU239), 0.0);
        assert_eq!(c.mass_frac(nuc::PU239), 0.0);
    }

    #[test]
    fn normalized_views_sum_to_one() {
        let c = Composition::from_atom(
            comp(&[(nuc::U235, 3.0), (nuc::U238, 7.0)]),
            &AtomicMasses::MassNumber,
        )
        .unwrap();
        assert!((comp_math::sum(&c.normalized_atom()) - 1.0).abs() < 1e-12);
        assert!((comp_math::sum(&c.normalized_mass()) - 1.0).abs() < 1e-12);
    }

    #[test]
    fn explicit_table_overrides_the_mass_number_approximation() {
        // A real U-235 atomic mass, not the integer 235.
        let masses = AtomicMasses::from_pairs(&[(nuc::U235, 235.043_929_9)]);
        let c = Composition::from_atom(comp(&[(nuc::U235, 1.0)]), &masses).unwrap();
        assert!((c.mass()[&nuc::U235] - 235.043_929_9).abs() < 1e-9);
    }

    #[test]
    fn a_nuclide_missing_from_the_table_is_an_error_not_a_zero() {
        let masses = AtomicMasses::from_pairs(&[(nuc::U235, 235.0)]);
        let e = Composition::from_atom(comp(&[(nuc::U238, 1.0)]), &masses);
        assert_eq!(
            e,
            Err(CyclusError::Key("nuclide absent from the atomic mass table"))
        );
    }

    #[test]
    fn mass_number_approximation_rejects_an_element_id() {
        // Guards the A == 0 case that would otherwise divide by zero.
        assert_eq!(
            AtomicMasses::MassNumber.mass_of(nuc::U_NATURAL),
            Err(CyclusError::InvalidNuclide(nuc::U_NATURAL.raw()))
        );
    }
}
