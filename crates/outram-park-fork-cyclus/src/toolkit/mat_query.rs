// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/toolkit/mat_query.h, src/toolkit/mat_query.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Reading physical quantities out of a [`Material`].
//!
//! A [`Material`] carries a quantity in kilograms and a *ratio* — its
//! [`Composition`] is not normalised and is not a mass. Turning that pair into
//! "how many kilograms of U-235 are in this material" is a two-step operation
//! that is easy to get subtly wrong (normalise the right basis, then scale by
//! the quantity), and upstream's `MatQuery` exists to do it once. So does this.
//!
//! # Units, stated plainly
//!
//! | Function | Returns | Units |
//! |---|---|---|
//! | [`qty`] | total mass | kilograms |
//! | [`mass`] | mass of one nuclide | kilograms |
//! | [`moles`] | amount of substance of one nuclide | moles |
//! | [`mass_frac`], [`mass_frac_of`] | mass fraction | dimensionless, in `[0, 1]` |
//! | [`atom_frac`], [`atom_frac_of`] | atom (mole) fraction | dimensionless, in `[0, 1]` |
//! | [`all_mass`] | mass of every nuclide | kilograms |
//! | [`all_atoms`] | moles of every nuclide | moles |
//! | [`amount`] | extractable mass of a given composition | kilograms |
//!
//! # Free functions and [`MatQuery`] both
//!
//! Upstream's `MatQuery` holds a `Material::Ptr` — a `boost::shared_ptr`. This
//! workspace forbids lifetime parameters on structs, so a borrowing wrapper is
//! not available, and every query here is *also* a free function taking
//! `&Material`. [`MatQuery`] is the owning form, for a caller who wants the
//! upstream shape; it clones the material on construction and every method
//! delegates to the free function of the same name. Nothing is lost: these are
//! all read-only queries.
//!
//! Prefer the free functions. They allocate nothing.
//!
//! # A note on upstream's API surface
//!
//! The brief for this port named `qty_bumped`, `AllMass` and `AllAtoms`. **No
//! such members exist at commit `d4faab7c`** — a `grep` over the whole
//! upstream tree finds none of the three. [`all_mass`] and [`all_atoms`] here
//! are therefore *not* translations; they are this port's own convenience over
//! [`Material::nuclide_masses`], named after what was asked for, and are
//! marked as such below. There is no `qty_bumped` equivalent because there is
//! nothing upstream to translate.
//!
//! Upstream's `std::string`-keyed overloads (`mass("U235")` and friends) are
//! also absent: they call `pyne::nucname::id`, a name parser that belongs with
//! the nuclear data, not here. Construct a [`Nuc`] instead.

use alloc::vec::Vec;

use crate::comp_math::{self, CompMap};
use crate::composition::{AtomicMasses, Composition};
use crate::error::Result;
use crate::limits::EPS_RSRC;
use crate::material::Material;
use crate::nuclide::Nuc;

/// The total mass of `m`, in kilograms. Upstream `MatQuery::qty`.
///
/// A thin alias for [`Material::quantity`], kept so a reader can follow
/// upstream's `mat_query.cc` line for line.
#[inline]
#[must_use]
pub fn qty(m: &Material) -> f64 {
    m.quantity()
}

/// The mass of nuclide `nuc` in `m`, in kilograms. Upstream `MatQuery::mass`.
///
/// Computed as `mass_frac(nuc) * qty`, exactly as upstream does. Returns `0.0`
/// for a nuclide the material does not contain, and for an empty material.
///
/// Valid range of the result: `0.0` to `m.quantity()`.
#[inline]
#[must_use]
pub fn mass(m: &Material, nuc: Nuc) -> f64 {
    mass_frac(m, nuc) * qty(m)
}

/// The amount of nuclide `nuc` in `m`, in **moles**. Upstream
/// `MatQuery::moles`.
///
/// # The unit conversion, spelled out
///
/// Upstream computes `mass(nuc) / (pyne::atomic_mass(nuc) * units::g)`, where
/// `units::g` is `1e-3` — the mass of one gram expressed in the kilogram base
/// unit. An atomic mass in unified atomic mass units (u) is numerically equal
/// to a molar mass in grams per mole, so:
///
/// `moles = mass_kg / (A_u [g/mol] * 1e-3 [kg/g]) = 1000 * mass_kg / A_u`
///
/// # Parameters
///
/// - `nuc` — the nuclide to count.
/// - `masses` — the source of `A_u`. Upstream reaches a PyNE global here;
///   this crate has no nuclear data of its own (workspace rule: that lives in
///   `njoy-outram-park-fork`), so the table is an explicit argument.
///   [`AtomicMasses::MassNumber`] approximates `A_u` by the mass number, which
///   is under 0.1 % wrong for the actinides and is *not* good enough for a
///   mass balance anyone will quote.
///
/// # Errors
///
/// Whatever [`AtomicMasses::mass_of`] reports — [`CyclusError::Key`] for a
/// nuclide missing from a [`AtomicMasses::Table`], or
/// [`CyclusError::InvalidNuclide`] for a natural-element id under
/// [`AtomicMasses::MassNumber`].
///
/// [`CyclusError::Key`]: crate::error::CyclusError::Key
/// [`CyclusError::InvalidNuclide`]: crate::error::CyclusError::InvalidNuclide
pub fn moles(m: &Material, nuc: Nuc, masses: &AtomicMasses) -> Result<f64> {
    let a = masses.mass_of(nuc)?;
    Ok(mass(m, nuc) / (a * 1e-3))
}

/// The mass fraction of `nuc` in `m`: dimensionless, in `[0, 1]`. Upstream
/// `MatQuery::mass_frac(Nuc)`.
///
/// Upstream copies the mass-basis composition, normalises it, and indexes it.
/// [`Composition::mass_frac`] is that operation, so this delegates. Returns
/// `0.0` for an absent nuclide or an empty composition.
#[inline]
#[must_use]
pub fn mass_frac(m: &Material, nuc: Nuc) -> f64 {
    m.comp().mass_frac(nuc)
}

/// The combined mass fraction of a set of nuclides: dimensionless, in
/// `[0, 1]`. Upstream `MatQuery::mass_frac(std::set<Nuc>)`.
///
/// Upstream sums `mass(nuc)` over the set and divides by `qty()`. That is
/// reproduced exactly, including the consequence that a **repeated nuclide in
/// `nucs` is counted twice** — upstream takes a `std::set`, which cannot
/// repeat, and a slice can. Pass distinct nuclides.
///
/// Returns `0.0` for an empty material (upstream would divide by zero and
/// return NaN; returning zero is a deliberate hardening, since every caller
/// treats the result as a fraction).
#[must_use]
pub fn mass_frac_of(m: &Material, nucs: &[Nuc]) -> f64 {
    let total = qty(m);
    if total == 0.0 {
        return 0.0;
    }
    let mut m_tot = 0.0;
    for &n in nucs {
        m_tot += mass(m, n);
    }
    m_tot / total
}

/// The atom (mole) fraction of `nuc` in `m`: dimensionless, in `[0, 1]`.
/// Upstream `MatQuery::atom_frac(Nuc)`.
///
/// Returns `0.0` for an absent nuclide or an empty composition.
#[inline]
#[must_use]
pub fn atom_frac(m: &Material, nuc: Nuc) -> f64 {
    m.comp().atom_frac(nuc)
}

/// The combined atom fraction of a set of nuclides: dimensionless, in
/// `[0, 1]`. Upstream `MatQuery::atom_frac(std::set<Nuc>)`.
///
/// Note that upstream implements this differently from its mass-fraction
/// counterpart — it normalises the atom map once and sums the *fractions*,
/// rather than summing absolute amounts and dividing. The two agree; the
/// normalise-once form is reproduced here. Nuclides absent from the
/// composition contribute nothing.
#[must_use]
pub fn atom_frac_of(m: &Material, nucs: &[Nuc]) -> f64 {
    let v = m.comp().normalized_atom();
    let mut frac_tot = 0.0;
    for &n in nucs {
        if let Some(x) = v.get(&n) {
            frac_tot += x;
        }
    }
    frac_tot
}

/// The mass of **every** nuclide in `m`, in kilograms.
///
/// **Not an upstream translation.** There is no `AllMass` at commit
/// `d4faab7c`; this is an alias for [`Material::nuclide_masses`], provided
/// under the name the port brief asked for. Keys are nuclides, values are
/// kilograms, and the values sum to `m.quantity()`.
#[inline]
#[must_use]
pub fn all_mass(m: &Material) -> CompMap {
    m.nuclide_masses()
}

/// The amount of **every** nuclide in `m`, in moles.
///
/// **Not an upstream translation** — see [`all_mass`]. Keys are nuclides,
/// values are moles, computed per [`moles`].
///
/// # Errors
///
/// Whatever [`AtomicMasses::mass_of`] reports for the first nuclide it cannot
/// price. A material containing a nuclide absent from `masses` fails as a
/// whole rather than silently dropping it.
pub fn all_atoms(m: &Material, masses: &AtomicMasses) -> Result<CompMap> {
    let mut out = CompMap::new();
    for (&nuc, &kg) in &all_mass(m) {
        let a = masses.mass_of(nuc)?;
        out.insert(nuc, kg / (a * 1e-3));
    }
    Ok(out)
}

/// `true` if `m` and `other` have the same mass-basis composition to within a
/// relative `threshold`. Upstream `MatQuery::AlmostEq`.
///
/// Both compositions are normalised before comparison, so this compares
/// *composition*, not quantity: 1 kg and 1000 kg of the same enrichment are
/// almost-equal. Upstream's default `threshold` is
/// [`EPS_RSRC`](crate::limits::EPS_RSRC); use [`almost_eq_default`] for it.
///
/// # Errors
///
/// [`CyclusError::Value`](crate::error::CyclusError::Value) if `threshold` is
/// negative.
pub fn almost_eq(m: &Material, other: &Material, threshold: f64) -> Result<bool> {
    let n1 = m.comp().normalized_mass();
    let n2 = other.comp().normalized_mass();
    comp_math::almost_eq(&n1, &n2, threshold)
}

/// [`almost_eq`] at upstream's default tolerance,
/// [`EPS_RSRC`](crate::limits::EPS_RSRC).
///
/// # Errors
///
/// Cannot fail in practice — [`EPS_RSRC`] is positive — but the signature is
/// kept fallible so the two forms stay interchangeable.
pub fn almost_eq_default(m: &Material, other: &Material) -> Result<bool> {
    almost_eq(m, other, EPS_RSRC)
}

/// The maximum mass, in kilograms, of composition `c` that can be extracted
/// from `m`. Upstream `MatQuery::Amount`.
///
/// This is the limiting-reagent calculation behind every separation and
/// fabrication step: given a recipe `c` and a feedstock `m`, how much product
/// does the scarcest ingredient allow?
///
/// # Method (upstream's, reproduced)
///
/// 1. Normalise both mass-basis compositions to sum to 1.
/// 2. For each nuclide in `c`, form the ratio `m_frac / c_frac`. If `m` lacks
///    a nuclide that `c` needs in nonzero amount, the answer is `0.0`.
/// 3. The smallest such ratio is the limiting one. Multiply it by
///    `m.quantity()` and sum the scaled recipe.
///
/// Because step 1 normalises `c` to sum to 1, step 3's sum is just
/// `min_ratio * m.quantity()` — upstream's explicit re-sum is arithmetically
/// redundant, and is kept here only so the two files read alike.
///
/// # Returns
///
/// Kilograms, in `[0, m.quantity()]`. Returns `0.0` for an empty `c` — there
/// is no limiting ratio to take, and upstream's `CY_LARGE_DOUBLE` sentinel
/// would otherwise escape as a nonsense answer. That guard is this port's, not
/// upstream's.
#[must_use]
pub fn amount(m: &Material, c: &Composition) -> f64 {
    let have = m.comp().normalized_mass();
    let want = c.normalized_mass();

    if want.is_empty() {
        return 0.0;
    }

    let mut min_ratio = crate::limits::CY_LARGE_DOUBLE;
    for (&nuc, &qty_other) in &want {
        if qty_other <= 0.0 {
            continue;
        }
        let Some(&q) = have.get(&nuc) else {
            return 0.0;
        };
        let ratio = q / qty_other;
        if ratio < min_ratio {
            min_ratio = ratio;
        }
    }

    if min_ratio >= crate::limits::CY_LARGE_DOUBLE {
        return 0.0;
    }

    let mult = min_ratio * qty(m);
    let mut scaled = want;
    comp_math::normalize(&mut scaled, mult);
    comp_math::sum(&scaled)
}

/// An owning inspector over a [`Material`]. Upstream `MatQuery`.
///
/// Every method is the free function of the same name, applied to the held
/// material. See the [module docs](self) for why this owns rather than
/// borrows, and for the units of each query.
///
/// # Example
///
/// ```
/// use outram_park_fork_cyclus::comp_math::CompMap;
/// use outram_park_fork_cyclus::composition::{AtomicMasses, Composition};
/// use outram_park_fork_cyclus::material::Material;
/// use outram_park_fork_cyclus::nuclide::nuc;
/// use outram_park_fork_cyclus::toolkit::mat_query::MatQuery;
///
/// // 10 kg of 5 % (by mass) enriched uranium.
/// let map: CompMap = [(nuc::U235, 0.05), (nuc::U238, 0.95)].into_iter().collect();
/// let comp = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
/// let mq = MatQuery::new(Material::new(10.0, comp).unwrap());
///
/// assert!((mq.mass(nuc::U235) - 0.5).abs() < 1e-12);      // kilograms
/// assert!((mq.mass_frac(nuc::U235) - 0.05).abs() < 1e-12); // dimensionless
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MatQuery {
    m: Material,
}

impl MatQuery {
    /// Wraps `m` for inspection. Upstream `MatQuery::MatQuery(Material::Ptr)`.
    #[must_use]
    pub fn new(m: Material) -> Self {
        Self { m }
    }

    /// Borrows the wrapped material.
    #[must_use]
    pub fn material(&self) -> &Material {
        &self.m
    }

    /// Consumes the query and returns the wrapped material.
    #[must_use]
    pub fn into_material(self) -> Material {
        self.m
    }

    /// Total mass, in kilograms. See [`qty`].
    #[must_use]
    pub fn qty(&self) -> f64 {
        qty(&self.m)
    }

    /// Mass of `nuc`, in kilograms. See [`mass`].
    #[must_use]
    pub fn mass(&self, nuc: Nuc) -> f64 {
        mass(&self.m, nuc)
    }

    /// Amount of `nuc`, in moles. See [`moles`].
    ///
    /// # Errors
    ///
    /// As [`moles`].
    pub fn moles(&self, nuc: Nuc, masses: &AtomicMasses) -> Result<f64> {
        moles(&self.m, nuc, masses)
    }

    /// Mass fraction of `nuc`, dimensionless. See [`mass_frac`].
    #[must_use]
    pub fn mass_frac(&self, nuc: Nuc) -> f64 {
        mass_frac(&self.m, nuc)
    }

    /// Combined mass fraction of `nucs`, dimensionless. See [`mass_frac_of`].
    #[must_use]
    pub fn mass_frac_of(&self, nucs: &[Nuc]) -> f64 {
        mass_frac_of(&self.m, nucs)
    }

    /// Atom fraction of `nuc`, dimensionless. See [`atom_frac`].
    #[must_use]
    pub fn atom_frac(&self, nuc: Nuc) -> f64 {
        atom_frac(&self.m, nuc)
    }

    /// Combined atom fraction of `nucs`, dimensionless. See [`atom_frac_of`].
    #[must_use]
    pub fn atom_frac_of(&self, nucs: &[Nuc]) -> f64 {
        atom_frac_of(&self.m, nucs)
    }

    /// Mass of every nuclide, in kilograms. See [`all_mass`].
    #[must_use]
    pub fn all_mass(&self) -> CompMap {
        all_mass(&self.m)
    }

    /// Moles of every nuclide. See [`all_atoms`].
    ///
    /// # Errors
    ///
    /// As [`all_atoms`].
    pub fn all_atoms(&self, masses: &AtomicMasses) -> Result<CompMap> {
        all_atoms(&self.m, masses)
    }

    /// Composition equality to within `threshold`. See [`almost_eq`].
    ///
    /// # Errors
    ///
    /// As [`almost_eq`].
    pub fn almost_eq(&self, other: &Material, threshold: f64) -> Result<bool> {
        almost_eq(&self.m, other, threshold)
    }

    /// Extractable mass of composition `c`, in kilograms. See [`amount`].
    #[must_use]
    pub fn amount(&self, c: &Composition) -> f64 {
        amount(&self.m, c)
    }

    /// The nuclides present, in ascending id order.
    ///
    /// **Not an upstream member.** A convenience for iterating a material's
    /// contents without reaching through to its composition.
    #[must_use]
    pub fn nuclides(&self) -> Vec<Nuc> {
        self.m.comp().mass().keys().copied().collect()
    }
}

impl From<Material> for MatQuery {
    fn from(m: Material) -> Self {
        Self::new(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::abs;
    use crate::nuclide::nuc;

    /// 10 kg of uranium enriched to 5 % U-235 **by mass**.
    fn ueu(qty: f64, u235_mass_frac: f64) -> Material {
        let map: CompMap = [
            (nuc::U235, u235_mass_frac),
            (nuc::U238, 1.0 - u235_mass_frac),
        ]
        .into_iter()
        .collect();
        let c = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
        Material::new(qty, c).unwrap()
    }

    #[test]
    fn mass_scales_the_fraction_by_the_quantity() {
        let m = ueu(10.0, 0.05);
        assert!(abs(qty(&m) - 10.0) < 1e-12);
        assert!(abs(mass(&m, nuc::U235) - 0.5) < 1e-12);
        assert!(abs(mass(&m, nuc::U238) - 9.5) < 1e-12);
        // A nuclide the material does not contain.
        assert_eq!(mass(&m, nuc::PU239), 0.0);
    }

    #[test]
    fn mass_and_atom_fractions_differ_as_the_mass_numbers_do() {
        let m = ueu(10.0, 0.05);
        assert!(abs(mass_frac(&m, nuc::U235) - 0.05) < 1e-12);

        // Atom fraction: (0.05/235) / (0.05/235 + 0.95/238).
        let n235 = 0.05 / 235.0;
        let n238 = 0.95 / 238.0;
        let expect = n235 / (n235 + n238);
        assert!(abs(atom_frac(&m, nuc::U235) - expect) < 1e-12);
        // The atom fraction is the larger of the two, U-235 being the lighter.
        assert!(atom_frac(&m, nuc::U235) > mass_frac(&m, nuc::U235));
    }

    #[test]
    fn moles_converts_kilograms_through_the_molar_mass() {
        let m = ueu(10.0, 0.05);
        // 0.5 kg of U-235 at 235 g/mol -> 500 g / 235 g/mol = 2.1276... mol.
        let expect = 500.0 / 235.0;
        let got = moles(&m, nuc::U235, &AtomicMasses::MassNumber).unwrap();
        assert!(abs(got - expect) < 1e-12, "got {got}, want {expect}");
    }

    #[test]
    fn set_fractions_sum_the_members() {
        let m = ueu(10.0, 0.05);
        let both = [nuc::U235, nuc::U238];
        assert!(abs(mass_frac_of(&m, &both) - 1.0) < 1e-12);
        assert!(abs(atom_frac_of(&m, &both) - 1.0) < 1e-12);
        // An absent nuclide contributes nothing.
        assert!(abs(atom_frac_of(&m, &[nuc::PU239]) - 0.0) < 1e-12);
    }

    #[test]
    fn all_mass_sums_to_the_quantity() {
        let m = ueu(10.0, 0.05);
        let masses = all_mass(&m);
        assert!(abs(comp_math::sum(&masses) - 10.0) < 1e-12);
        assert!(abs(masses[&nuc::U235] - 0.5) < 1e-12);

        let atoms = all_atoms(&m, &AtomicMasses::MassNumber).unwrap();
        assert!(abs(atoms[&nuc::U235] - 500.0 / 235.0) < 1e-12);
        assert!(abs(atoms[&nuc::U238] - 9500.0 / 238.0) < 1e-12);
    }

    #[test]
    fn almost_eq_compares_composition_not_quantity() {
        let a = ueu(1.0, 0.05);
        let b = ueu(1000.0, 0.05);
        assert!(almost_eq_default(&a, &b).unwrap());

        let c = ueu(1.0, 0.04);
        assert!(!almost_eq_default(&a, &c).unwrap());
    }

    #[test]
    fn amount_is_limited_by_the_scarcest_ingredient() {
        // Feed: 10 kg, half U-235 and half U-238 by mass.
        let feed = ueu(10.0, 0.5);

        // Recipe wanting 10 % U-235 / 90 % U-238. U-238 is the limiter:
        // available mass fraction 0.5, wanted 0.9 -> ratio 0.5555...;
        // U-235 ratio is 0.5/0.1 = 5. So 0.5555... * 10 kg = 5.5555... kg.
        let map: CompMap = [(nuc::U235, 0.1), (nuc::U238, 0.9)].into_iter().collect();
        let recipe = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
        let got = amount(&feed, &recipe);
        let expect = (0.5 / 0.9) * 10.0;
        assert!(abs(got - expect) < 1e-12, "got {got}, want {expect}");
        // Never more than the feedstock itself.
        assert!(got <= feed.quantity() + 1e-12);
    }

    #[test]
    fn amount_is_zero_when_a_needed_nuclide_is_absent() {
        let feed = ueu(10.0, 0.5);
        let map: CompMap = [(nuc::PU239, 1.0)].into_iter().collect();
        let recipe = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
        assert_eq!(amount(&feed, &recipe), 0.0);
    }

    #[test]
    fn amount_of_its_own_composition_is_the_whole_material() {
        let feed = ueu(10.0, 0.05);
        let got = amount(&feed, feed.comp());
        assert!(abs(got - 10.0) < 1e-9, "got {got}");
    }

    #[test]
    fn wrapper_agrees_with_the_free_functions() {
        let m = ueu(7.5, 0.03);
        let mq = MatQuery::new(m.clone());
        assert_eq!(mq.qty(), qty(&m));
        assert_eq!(mq.mass(nuc::U235), mass(&m, nuc::U235));
        assert_eq!(mq.atom_frac(nuc::U238), atom_frac(&m, nuc::U238));
        assert_eq!(mq.nuclides(), alloc::vec![nuc::U235, nuc::U238]);
    }
}
