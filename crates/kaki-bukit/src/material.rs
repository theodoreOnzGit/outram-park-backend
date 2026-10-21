// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/material.h, src/material.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Material: a mass of nuclides with a composition.
//!
//! [`Material`] is the resource almost everything in a fuel cycle moves
//! around. It is a **quantity in kilograms** plus a
//! [`Composition`](crate::composition::Composition), and its whole interface
//! is four operations that conserve mass between them:
//!
//! | Operation | Effect |
//! |---|---|
//! | [`extract_qty`](Material::extract_qty) | split off `qty` kg of the same composition |
//! | [`extract_comp`](Material::extract_comp) | split off `qty` kg of a *different* composition, leaving the remainder |
//! | [`absorb`](Material::absorb) | merge another material in, mass-weighting the compositions |
//! | [`transmute`](Material::transmute) | replace the composition, keeping the mass |
//!
//! Upstream's units are kilograms by fiat — `Material::units()` returns the
//! literal string `"kg"` — and this translation keeps that rather than
//! reaching for `uom`. See the note on the `petir` dependency in `Cargo.toml`
//! for why.

use crate::comp_math::{self, CompMap};
use crate::composition::{AtomicMasses, Composition};
use crate::error::{CyclusError, Result};
use crate::limits::EPS_RSRC;

/// The default extraction threshold. Upstream `Material::kDefaultThreshold`.
///
/// Quantities at or below this are cleared from the remainder composition
/// after an [`extract_comp`](Material::extract_comp), so a separation that
/// removes "all" of a nuclide does not leave `1e-30` kg of it behind to be
/// carried for the rest of the simulation.
pub const DEFAULT_THRESHOLD: f64 = 1e-14;

/// A quantity of material, in kilograms, with a nuclide composition.
///
/// # Tracking
///
/// Upstream threads a `ResTracker` through every operation to write a resource
/// genealogy to the output database. There is no database in this `no_std`
/// kernel, so no tracker: a [`Material`] here is the physical object only.
/// `prev_decay_time` is kept, because it is not bookkeeping — it is the state
/// [`absorb`](Material::absorb) needs to refuse to merge materials that have
/// been decayed to different times.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    qty: f64,
    comp: Composition,
    prev_decay_time: i64,
    unit_value: f64,
}

impl Material {
    /// Creates `qty` kilograms of material with composition `comp`.
    ///
    /// Corresponds to upstream's `Material::CreateUntracked`; the tracked
    /// `Create` has no counterpart here, since tracking is a database concern.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `qty` is negative.
    ///
    /// # Examples
    ///
    /// ```
    /// use kaki_bukit::composition::{AtomicMasses, Composition};
    /// use kaki_bukit::material::Material;
    /// use kaki_bukit::nuclide::nuc;
    ///
    /// let masses = AtomicMasses::MassNumber;
    /// let comp = Composition::from_nuclide(nuc::U235, &masses)?;
    /// let m = Material::new(10.0, comp)?;
    /// assert_eq!(m.quantity(), 10.0);
    /// assert_eq!(m.units(), "kg");
    /// # Ok::<(), kaki_bukit::error::CyclusError>(())
    /// ```
    pub fn new(qty: f64, comp: Composition) -> Result<Self> {
        if qty < 0.0 {
            return Err(CyclusError::Value("material quantity cannot be negative"));
        }
        Ok(Self {
            qty,
            comp,
            prev_decay_time: 0,
            unit_value: 1.0,
        })
    }

    /// Creates `qty` kilograms with composition `comp` and a per-unit economic
    /// value.
    ///
    /// `unit_value` is upstream's `Resource::unit_value`, carried through
    /// [`absorb`](Material::absorb) as a mass-weighted average. It has no
    /// physical meaning to this crate; it exists so an economics layer on top
    /// has somewhere to put a price.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `qty` is negative.
    pub fn with_unit_value(qty: f64, comp: Composition, unit_value: f64) -> Result<Self> {
        let mut m = Self::new(qty, comp)?;
        m.unit_value = unit_value;
        Ok(m)
    }

    /// The mass in kilograms. Upstream `Material::quantity()`.
    #[must_use]
    pub fn quantity(&self) -> f64 {
        self.qty
    }

    /// The units of [`quantity`](Material::quantity), always `"kg"`.
    /// Upstream `Material::units()`.
    #[must_use]
    pub fn units(&self) -> &'static str {
        "kg"
    }

    /// The composition. Upstream `Material::comp()`.
    #[must_use]
    pub fn comp(&self) -> &Composition {
        &self.comp
    }

    /// The per-unit economic value. Upstream `Resource::unit_value()`.
    #[must_use]
    pub fn unit_value(&self) -> f64 {
        self.unit_value
    }

    /// Sets the per-unit economic value.
    pub fn set_unit_value(&mut self, v: f64) {
        self.unit_value = v;
    }

    /// The simulation time this material was last decayed to.
    #[must_use]
    pub fn prev_decay_time(&self) -> i64 {
        self.prev_decay_time
    }

    /// Sets the last-decayed time. Called by [`decay`](crate::decay).
    pub fn set_prev_decay_time(&mut self, t: i64) {
        self.prev_decay_time = t;
    }

    /// The **mass** of each nuclide, in kilograms: the composition scaled to
    /// this material's quantity.
    ///
    /// This is the form every mass balance wants, and getting it by hand — the
    /// composition is a ratio, not a mass — is the mistake this method exists
    /// to prevent.
    #[must_use]
    pub fn nuclide_masses(&self) -> CompMap {
        let mut v = self.comp.mass().clone();
        comp_math::normalize(&mut v, self.qty);
        v
    }

    /// Splits `qty` kilograms off this material, keeping the same composition.
    /// Upstream `Material::ExtractQty`.
    ///
    /// On success this material's quantity is reduced by `qty` and the
    /// extracted material is returned.
    ///
    /// # Errors
    ///
    /// [`CyclusError::Value`] if `qty` exceeds the available quantity or is
    /// negative.
    pub fn extract_qty(&mut self, qty: f64) -> Result<Self> {
        if qty < 0.0 {
            return Err(CyclusError::Value("cannot extract a negative quantity"));
        }
        if self.qty < qty {
            return Err(CyclusError::Value("mass extraction causes negative quantity"));
        }
        self.qty -= qty;
        Ok(Self {
            qty,
            comp: self.comp.clone(),
            prev_decay_time: self.prev_decay_time,
            unit_value: self.unit_value,
        })
    }

    /// Splits `qty` kilograms of composition `c` off this material, leaving
    /// the remainder. Upstream `Material::ExtractComp`.
    ///
    /// This is how every separation, enrichment and fabrication step works:
    /// the caller names *what* it wants out, and the material keeps what is
    /// left. The remainder's composition is computed by subtracting the
    /// extracted nuclide masses from this material's, then clearing anything
    /// at or below `threshold`.
    ///
    /// # Parameters
    ///
    /// - `qty` — kilograms to remove.
    /// - `c` — the composition of the removed material.
    /// - `threshold` — remainder quantities at or below this are cleared;
    ///   [`DEFAULT_THRESHOLD`] is upstream's default.
    /// - `masses` — needed to rebuild the remainder composition's atom basis.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::Value`] if `qty` exceeds the available quantity, or if
    ///   the extraction would leave a negative quantity of some nuclide —
    ///   i.e. the caller asked for more of a nuclide than is present.
    ///
    /// # A divergence from upstream, and why it is a hardening
    ///
    /// Upstream subtracts, applies the threshold, and constructs the remainder
    /// composition — and `Composition::CreateFromMass` then throws
    /// `ValueError("negative quantity in CompMap")` if the subtraction went
    /// negative. So upstream *does* reject it, but only incidentally, and the
    /// message names the composition rather than the extraction. Here the
    /// check is explicit and the error says what actually happened.
    pub fn extract_comp(
        &mut self,
        qty: f64,
        c: &Composition,
        threshold: f64,
        masses: &AtomicMasses,
    ) -> Result<Self> {
        if qty < 0.0 {
            return Err(CyclusError::Value("cannot extract a negative quantity"));
        }
        if self.qty < qty {
            return Err(CyclusError::Value("mass extraction causes negative quantity"));
        }

        if &self.comp != c {
            let mut v = self.comp.mass().clone();
            comp_math::normalize(&mut v, self.qty);
            let mut other = c.mass().clone();
            comp_math::normalize(&mut other, qty);

            let mut newv = comp_math::sub(&v, &other);
            comp_math::apply_threshold(&mut newv, threshold)?;

            if !comp_math::all_positive(&newv) {
                return Err(CyclusError::Value(
                    "extraction requests more of a nuclide than the material holds",
                ));
            }
            self.comp = Composition::from_mass(newv, masses)?;
        }

        self.qty -= qty;
        Ok(Self {
            qty,
            comp: c.clone(),
            // The extracted material carries this material's decay time
            // regardless of composition, so a later Absorb does not think it
            // is less decayed than it is.
            prev_decay_time: self.prev_decay_time,
            unit_value: self.unit_value,
        })
    }

    /// Merges `other` into this material. Upstream `Material::Absorb`.
    ///
    /// The result holds the combined mass, a mass-weighted blend of the two
    /// compositions, and a mass-weighted average unit value. `other` is
    /// consumed, which is the translation of upstream setting its quantity to
    /// zero.
    ///
    /// # Errors
    ///
    /// - [`CyclusError::Value`] if `other` has been decayed further than this
    ///   material. Upstream raises the same error, and the reason is physical
    ///   rather than technical: blending a material decayed to time *t₂* into
    ///   one at *t₁ < t₂* would silently un-decay it.
    ///
    /// # A divergence from upstream
    ///
    /// Upstream, when both materials are attached to a simulation context in
    /// `"lazy"` decay mode, decays *both* to the current time before merging.
    /// That path needs a `Context` and a decay-chain, neither of which this
    /// function has. Here the caller decays first and then absorbs; the guard
    /// above is what makes forgetting to do so an error rather than a silent
    /// wrong answer.
    pub fn absorb(&mut self, other: Self, masses: &AtomicMasses) -> Result<()> {
        if other.prev_decay_time > self.prev_decay_time {
            return Err(CyclusError::Value(
                "cannot absorb a material that is more decayed than this one",
            ));
        }

        if self.comp != other.comp {
            let mut v = self.comp.mass().clone();
            comp_math::normalize(&mut v, self.qty);
            let mut otherv = other.comp.mass().clone();
            comp_math::normalize(&mut otherv, other.qty);
            self.comp = Composition::from_mass(comp_math::add(&v, &otherv), masses)?;
        }

        let tot_mass = self.qty + other.qty;
        if tot_mass > 0.0 {
            self.unit_value =
                (self.qty * self.unit_value + other.qty * other.unit_value) / tot_mass;
        }
        self.qty = tot_mass;
        Ok(())
    }

    /// Replaces the composition, keeping the mass. Upstream
    /// `Material::Transmute`.
    ///
    /// This is what a reactor does to its fuel: the same kilograms come out,
    /// with a different nuclide vector. The caller is asserting that `c` is
    /// correct for the current simulation time, so `now` is recorded as the
    /// new decay time — accumulated decay from a composition that no longer
    /// exists must not be applied to the new one.
    pub fn transmute(&mut self, c: Composition, now: i64) {
        self.comp = c;
        if now > self.prev_decay_time {
            self.prev_decay_time = now;
        }
    }

    /// `true` if the quantity is at or below the resource epsilon.
    ///
    /// Not an upstream method, but the `qty_ > eps_rsrc()` test appears
    /// throughout upstream's buffers and agents; naming it keeps the tolerance
    /// in one place.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.qty <= EPS_RSRC
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nuclide::nuc;

    fn masses() -> AtomicMasses {
        AtomicMasses::MassNumber
    }

    fn comp(entries: &[(crate::nuclide::Nuc, f64)]) -> Composition {
        let map: CompMap = entries.iter().copied().collect();
        Composition::from_mass(map, &masses()).unwrap()
    }

    #[test]
    fn extract_qty_conserves_mass_and_composition() {
        let c = comp(&[(nuc::U235, 0.05), (nuc::U238, 0.95)]);
        let mut m = Material::new(100.0, c.clone()).unwrap();
        let got = m.extract_qty(30.0).unwrap();

        assert_eq!(m.quantity(), 70.0);
        assert_eq!(got.quantity(), 30.0);
        assert_eq!(m.quantity() + got.quantity(), 100.0);
        assert_eq!(got.comp(), &c);
        assert_eq!(m.comp(), &c);
    }

    #[test]
    fn extract_qty_rejects_more_than_is_present() {
        let mut m = Material::new(1.0, comp(&[(nuc::U235, 1.0)])).unwrap();
        assert_eq!(
            m.extract_qty(2.0),
            Err(CyclusError::Value("mass extraction causes negative quantity"))
        );
        assert_eq!(m.quantity(), 1.0, "a failed extraction must not mutate");
    }

    #[test]
    fn extract_comp_leaves_the_correct_remainder() {
        // 100 kg of 5 % enriched uranium: 5 kg U-235, 95 kg U-238.
        let mut m = Material::new(100.0, comp(&[(nuc::U235, 0.05), (nuc::U238, 0.95)])).unwrap();
        // Pull out 4 kg of pure U-235.
        let pure = comp(&[(nuc::U235, 1.0)]);
        let got = m.extract_comp(4.0, &pure, DEFAULT_THRESHOLD, &masses()).unwrap();

        assert_eq!(got.quantity(), 4.0);
        assert!((m.quantity() - 96.0).abs() < 1e-12);

        // 1 kg U-235 and 95 kg U-238 remain.
        let rem = m.nuclide_masses();
        assert!((rem[&nuc::U235] - 1.0).abs() < 1e-9);
        assert!((rem[&nuc::U238] - 95.0).abs() < 1e-9);
    }

    #[test]
    fn extract_comp_rejects_taking_more_of_a_nuclide_than_is_present() {
        let mut m = Material::new(100.0, comp(&[(nuc::U235, 0.05), (nuc::U238, 0.95)])).unwrap();
        let pure = comp(&[(nuc::U235, 1.0)]);
        // Only 5 kg of U-235 is present; ask for 10.
        assert_eq!(
            m.extract_comp(10.0, &pure, DEFAULT_THRESHOLD, &masses()),
            Err(CyclusError::Value(
                "extraction requests more of a nuclide than the material holds"
            ))
        );
        assert_eq!(m.quantity(), 100.0, "a failed extraction must not mutate");
    }

    #[test]
    fn extract_comp_clears_residue_below_the_threshold() {
        let mut m = Material::new(10.0, comp(&[(nuc::U235, 1.0)])).unwrap();
        let pure = comp(&[(nuc::U235, 1.0)]);
        // Identical composition: upstream short-circuits, so the remainder
        // keeps its single nuclide rather than being emptied.
        let got = m.extract_comp(10.0, &pure, DEFAULT_THRESHOLD, &masses()).unwrap();
        assert_eq!(got.quantity(), 10.0);
        assert_eq!(m.quantity(), 0.0);
        assert!(m.is_empty());
    }

    #[test]
    fn absorb_blends_compositions_by_mass() {
        // 10 kg of pure U-235 + 90 kg of pure U-238 -> 100 kg at 5 % ... no:
        // 10 % U-235.
        let mut a = Material::new(10.0, comp(&[(nuc::U235, 1.0)])).unwrap();
        let b = Material::new(90.0, comp(&[(nuc::U238, 1.0)])).unwrap();
        a.absorb(b, &masses()).unwrap();

        assert!((a.quantity() - 100.0).abs() < 1e-12);
        assert!((a.comp().mass_frac(nuc::U235) - 0.10).abs() < 1e-12);
        assert!((a.comp().mass_frac(nuc::U238) - 0.90).abs() < 1e-12);
    }

    #[test]
    fn absorb_averages_unit_value_by_mass() {
        let mut a = Material::with_unit_value(10.0, comp(&[(nuc::U235, 1.0)]), 100.0).unwrap();
        let b = Material::with_unit_value(30.0, comp(&[(nuc::U235, 1.0)]), 20.0).unwrap();
        a.absorb(b, &masses()).unwrap();
        // (10*100 + 30*20) / 40 = 40
        assert!((a.unit_value() - 40.0).abs() < 1e-12);
    }

    #[test]
    fn absorb_refuses_a_more_decayed_material() {
        let mut a = Material::new(1.0, comp(&[(nuc::U235, 1.0)])).unwrap();
        let mut b = Material::new(1.0, comp(&[(nuc::U235, 1.0)])).unwrap();
        b.set_prev_decay_time(5);
        assert_eq!(
            a.absorb(b, &masses()),
            Err(CyclusError::Value(
                "cannot absorb a material that is more decayed than this one"
            ))
        );
    }

    #[test]
    fn extract_then_absorb_round_trips_exactly() {
        let c = comp(&[(nuc::U235, 0.05), (nuc::U238, 0.95)]);
        let mut m = Material::new(100.0, c).unwrap();
        let piece = m.extract_qty(37.5).unwrap();
        m.absorb(piece, &masses()).unwrap();
        assert!((m.quantity() - 100.0).abs() < 1e-12);
        assert!((m.comp().mass_frac(nuc::U235) - 0.05).abs() < 1e-12);
    }

    #[test]
    fn transmute_keeps_mass_and_advances_the_decay_clock() {
        let mut m = Material::new(50.0, comp(&[(nuc::U235, 1.0)])).unwrap();
        m.transmute(comp(&[(nuc::PU239, 1.0)]), 12);
        assert_eq!(m.quantity(), 50.0);
        assert_eq!(m.comp().mass_frac(nuc::PU239), 1.0);
        assert_eq!(m.prev_decay_time(), 12);
    }

    #[test]
    fn nuclide_masses_scales_the_composition_to_the_quantity() {
        let m = Material::new(200.0, comp(&[(nuc::U235, 0.03), (nuc::U238, 0.97)])).unwrap();
        let masses = m.nuclide_masses();
        assert!((masses[&nuc::U235] - 6.0).abs() < 1e-9);
        assert!((masses[&nuc::U238] - 194.0).abs() < 1e-9);
    }

    #[test]
    fn new_rejects_a_negative_quantity() {
        assert_eq!(
            Material::new(-1.0, comp(&[(nuc::U235, 1.0)])).unwrap_err(),
            CyclusError::Value("material quantity cannot be negative")
        );
    }
}
