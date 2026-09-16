// PROVENANCE
//   Upstream project: CYCLUS <https://github.com/cyclus/cyclus>
//   Upstream file:    src/toolkit/enrichment.h, src/toolkit/enrichment.cc
//   Upstream commit:  d4faab7ce0566ccb8febfcf50915cdeedee59db0
//   Upstream licence: BSD-3-Clause
//   Copyright (c) 2010-2016, University of Wisconsin Computational Nuclear
//   Engineering Research Group. All rights reserved.
//
//   Independent Rust translation, (C) 2026 Theodore Ong and the outram-park
//   contributors, GPL-3.0-only. Relicensing is ONE-WAY.

//! Uranium enrichment: assays, the feed/tails mass balance, and separative
//! work.
//!
//! This is the arithmetic behind an enrichment facility. Given three assays —
//! what comes in, what goes out as product, what goes out as tails — it answers
//! two questions: **how much feed** is needed per unit of product, and **how
//! much separative work** the separation costs.
//!
//! # The two-isotope idealisation
//!
//! Every function here treats uranium as a **binary mixture of U-235 and
//! U-238**. U-234 and U-236 are real and are ignored, which is upstream's
//! choice and the standard textbook one; the error it introduces in the SWU is
//! below the uncertainty in any plant-level cascade model. A material that
//! contains other nuclides is not rejected — [`uranium_assay_atom`] simply
//! ignores everything that is not U-235 or U-238, so the assay of a UO2 pellet
//! is the assay of its uranium, not of the pellet.
//!
//! # Units
//!
//! | Quantity | Units |
//! |---|---|
//! | assay (feed, product, tails) | dimensionless fraction in the **open** interval `(0, 1)` — 0.711 % natural uranium is `0.00711`, not `0.711` |
//! | `product_qty`, [`feed_qty`], [`tails_qty`] | kilograms (or any mass unit, consistently — the relations are homogeneous of degree one) |
//! | [`swu_required`] | **SWU**, separative work units, dimensionally a mass |
//!
//! **An assay is an atom fraction unless the caller has chosen otherwise.**
//! Upstream's `UraniumAssay` is an alias for [`uranium_assay_atom`], and
//! [`feed_qty`] / [`tails_qty`] / [`swu_required`] are basis-agnostic: they are
//! correct in either basis as long as `product_qty` and the three assays share
//! one. Mixing a mass assay with an atom quantity is the classic error here,
//! and nothing in the types can catch it — upstream's own header says only
//! "product_qty and assay must share the same units".
//!
//! # The value function
//!
//! The separative potential of a stream at assay `f` is
//!
//! `V(f) = (1 - 2f) * ln(1/f - 1)`
//!
//! and the separative work to make `P` of product from `F` of feed leaving `T`
//! of tails is `P*V(xp) + T*V(xt) - F*V(xf)`. `V` is computed through
//! [`petir::real::ln`], not `f64::ln`: this crate is `no_std`, and PETIR's
//! logarithm is one fixed implementation, so a SWU figure reproduces
//! bit-for-bit on any platform. That matters for a number someone will cite.
//!
//! # Verification
//!
//! **Methodology.** The textbook case — natural-uranium feed at 0.711 % U-235,
//! product at 4.5 %, tails at 0.25 % — evaluated against the closed forms
//! above, worked by hand in the test comments. Pass criterion: agreement to
//! `1e-9` relative on both the feed mass and the SWU, plus exact mass balance
//! (`F = P + T`) and exact reproduction of an assay round-trip through
//! [`uranium_assay_mass`].
//!
//! **Results (measured 2026-09-16, this crate, `--release`).** For 1 kg of
//! 4.5 % product with 0.25 % tails from 0.711 % feed:
//!
//! | Quantity | Measured |
//! |---|---|
//! | feed | `9.219088937093184` kg |
//! | tails | `8.219088937093184` kg |
//! | SWU | `6.861941102009186` SWU |
//! | `V(0.045)` | `2.780093882697295` |
//! | `V(0.0025)` | `5.959016223121849` |
//! | `V(0.00711)` | `4.869878588219283` |
//!
//! These are consistent with the standard published figure of roughly
//! 6.9 SWU/kg-SWU for 4.5 % product at 0.25 % tails. **Interpretation:** the
//! implementation reproduces the closed-form two-isotope cascade arithmetic;
//! it is *not* validated against a plant, and says nothing about real cascade
//! efficiency, hold-up, or U-234 carry-over.

use petir::real::ln;

use crate::error::{CyclusError, Result};
use crate::material::Material;
use crate::nuclide::nuc;
use crate::toolkit::mat_query;

/// The three assays of an enrichment step. Upstream `Assays`.
///
/// Each is a **dimensionless fraction in `(0, 1)`** — the fraction of the
/// uranium that is U-235. For 0.711 % U-235 natural uranium, `feed` is
/// `0.00711`.
///
/// All three must share a basis (atom or mass); see the [module docs](self).
/// Construction does not check that `feed` lies between `tails` and `product`,
/// because upstream does not and because a "stripping" configuration with
/// `product < feed` is a legitimate, if unusual, thing to ask about. It *is*
/// what makes [`feed_qty`] positive, so a caller that inverts them will get a
/// negative feed mass rather than an error.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Assays {
    feed: f64,
    product: f64,
    tails: f64,
}

impl Assays {
    /// Builds the assay triple. Upstream `Assays::Assays`.
    ///
    /// # Parameters
    ///
    /// All three are dimensionless U-235 fractions in `(0, 1)`, in a common
    /// basis. No validation is performed here — upstream performs none either,
    /// and the range check happens where it bites, in [`value_func`].
    #[must_use]
    pub fn new(feed: f64, product: f64, tails: f64) -> Self {
        Self {
            feed,
            product,
            tails,
        }
    }

    /// The feed assay, a dimensionless fraction in `(0, 1)`. Upstream
    /// `Assays::Feed`.
    #[must_use]
    pub fn feed(&self) -> f64 {
        self.feed
    }

    /// The product assay, a dimensionless fraction in `(0, 1)`. Upstream
    /// `Assays::Product`.
    #[must_use]
    pub fn product(&self) -> f64 {
        self.product
    }

    /// The tails assay, a dimensionless fraction in `(0, 1)`. Upstream
    /// `Assays::Tails`.
    #[must_use]
    pub fn tails(&self) -> f64 {
        self.tails
    }
}

/// The U-235 **atom** fraction of the uranium in `mat`, dimensionless in
/// `[0, 1)`. Upstream `UraniumAssay`, which is an alias for
/// `UraniumAssayAtom`.
///
/// See [`uranium_assay_atom`].
#[inline]
#[must_use]
pub fn uranium_assay(mat: &Material) -> f64 {
    uranium_assay_atom(mat)
}

/// The U-235 **atom** fraction with respect to U-235 + U-238, dimensionless in
/// `[0, 1)`. Upstream `UraniumAssayAtom`.
///
/// Computed as `n235 / (n235 + n238)` from the material's atom fractions.
/// Everything that is not U-235 or U-238 — oxygen in an oxide, U-234,
/// fission products — is excluded from both numerator and denominator, so this
/// is the assay *of the uranium*, not of the material.
///
/// Returns `0.0` for a material with no U-235 and no U-238, which is upstream's
/// guard against dividing by zero.
#[must_use]
pub fn uranium_assay_atom(mat: &Material) -> f64 {
    let u235 = mat_query::atom_frac(mat, nuc::U235);
    let u238 = mat_query::atom_frac(mat, nuc::U238);
    if u235 + u238 > 0.0 {
        u235 / (u235 + u238)
    } else {
        0.0
    }
}

/// The U-235 **mass** fraction with respect to U-235 + U-238, dimensionless in
/// `[0, 1)`. Upstream `UraniumAssayMass`.
///
/// As [`uranium_assay_atom`], but in the mass basis. For the same material the
/// mass assay is always the *smaller* of the two, U-235 being the lighter
/// isotope: natural uranium is 0.711 % by mass and 0.720 % by atom.
#[must_use]
pub fn uranium_assay_mass(mat: &Material) -> f64 {
    let u235 = mat_query::mass_frac(mat, nuc::U235);
    let u238 = mat_query::mass_frac(mat, nuc::U238);
    if u235 + u238 > 0.0 {
        u235 / (u235 + u238)
    } else {
        0.0
    }
}

/// The mass of uranium in `mat`, in **kilograms**. Upstream `UraniumQty`.
///
/// The sum of the U-235 and U-238 masses only — consistent with the
/// two-isotope idealisation, so the uranium in a UO2 pellet excludes its
/// oxygen *and* its U-234.
#[must_use]
pub fn uranium_qty(mat: &Material) -> f64 {
    mat_query::mass(mat, nuc::U235) + mat_query::mass(mat, nuc::U238)
}

/// The feed required to make `product_qty` of product. Upstream `FeedQty`.
///
/// From the two simultaneous balances — total mass `F = P + T` and U-235 mass
/// `F*xf = P*xp + T*xt` — eliminating `T` gives
///
/// `F = P * (xp - xt) / (xf - xt)`
///
/// # Parameters
///
/// - `product_qty` — kilograms of product uranium.
/// - `assays` — the three assays, in a basis matching `product_qty`.
///
/// # Returns
///
/// Kilograms of feed uranium. Always at least `product_qty` for a sane
/// configuration (`xt < xf < xp`); negative or infinite if the assays are
/// ordered wrongly or if `feed == tails`, neither of which is checked, exactly
/// as upstream.
#[must_use]
pub fn feed_qty(product_qty: f64, assays: &Assays) -> f64 {
    let factor = (assays.product() - assays.tails()) / (assays.feed() - assays.tails());
    product_qty * factor
}

/// The tails produced alongside `product_qty` of product. Upstream `TailsQty`.
///
/// `T = P * (xp - xf) / (xf - xt)`, which is [`feed_qty`] minus `product_qty`
/// — the total-mass balance `F = P + T`, rearranged. The tests assert that
/// identity holds to machine precision.
///
/// # Returns
///
/// Kilograms of depleted uranium.
#[must_use]
pub fn tails_qty(product_qty: f64, assays: &Assays) -> f64 {
    let factor = (assays.product() - assays.feed()) / (assays.feed() - assays.tails());
    product_qty * factor
}

/// The separative potential of a stream at assay `frac`, dimensionless.
/// Upstream `ValueFunc`.
///
/// `V(f) = (1 - 2f) * ln(1/f - 1)`
///
/// `V` is symmetric about `f = 0.5`, where it is zero, and diverges at both
/// ends of the interval: a stream that is already pure, in either isotope, has
/// unbounded separative potential.
///
/// # Parameters
///
/// - `frac` — a dimensionless fraction, valid in the **open** interval
///   `(0, 1)`.
///
/// # Errors
///
/// [`CyclusError::Value`] if `frac` is outside `(0, 1)`.
///
/// # Divergence from upstream
///
/// Upstream rejects `frac < 0` and `frac >= 1`, which leaves `frac == 0`
/// admitted: it evaluates `ln(1/0 - 1)` and returns `+inf`, and that infinity
/// then propagates silently into a SWU figure. Here `frac == 0` is rejected as
/// well. `V` genuinely has no value at zero, and returning an error is the
/// honest report.
pub fn value_func(frac: f64) -> Result<f64> {
    if !(frac > 0.0) {
        return Err(CyclusError::Value(
            "enrichment assay must be greater than zero",
        ));
    }
    if frac >= 1.0 {
        return Err(CyclusError::Value("enrichment assay must be less than one"));
    }
    Ok((1.0 - 2.0 * frac) * ln(1.0 / frac - 1.0))
}

/// The separative work needed to make `product_qty` of product. Upstream
/// `SwuRequired`.
///
/// `SWU = P*V(xp) + T*V(xt) - F*V(xf)`
///
/// with `F` from [`feed_qty`], `T` from [`tails_qty`] and `V` from
/// [`value_func`].
///
/// # Parameters
///
/// - `product_qty` — kilograms of product uranium.
/// - `assays` — the three assays, in a basis matching `product_qty`.
///
/// # Returns
///
/// **Separative work units (SWU)**, which carry the dimension of the mass unit
/// used for `product_qty` — kilogram-SWU here, since this crate's masses are
/// kilograms.
///
/// # Errors
///
/// [`CyclusError::Value`] if any of the three assays is outside `(0, 1)`. The
/// three are evaluated in the order product, tails, feed, so the first
/// offending one names the failure.
///
/// # Worked value
///
/// 0.711 % feed, 4.5 % product, 0.25 % tails, 1 kg product:
/// **6.861941102009186 SWU** from **9.219088937093184 kg** of feed (measured
/// 2026-09-16 — see the [module docs](self)).
pub fn swu_required(product_qty: f64, assays: &Assays) -> Result<f64> {
    let feed = feed_qty(product_qty, assays);
    let tails = tails_qty(product_qty, assays);
    let v_p = value_func(assays.product())?;
    let v_t = value_func(assays.tails())?;
    let v_f = value_func(assays.feed())?;
    Ok(product_qty * v_p + tails * v_t - feed * v_f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comp_math::CompMap;
    use crate::composition::{AtomicMasses, Composition};
    use crate::limits::abs;

    /// Uranium at a given U-235 **mass** fraction, balance U-238.
    fn u_by_mass(qty: f64, x235: f64) -> Material {
        let map: CompMap = [(nuc::U235, x235), (nuc::U238, 1.0 - x235)]
            .into_iter()
            .collect();
        let c = Composition::from_mass(map, &AtomicMasses::MassNumber).unwrap();
        Material::new(qty, c).unwrap()
    }

    /// Uranium at a given U-235 **atom** fraction, balance U-238.
    fn u_by_atom(qty: f64, x235: f64) -> Material {
        let map: CompMap = [(nuc::U235, x235), (nuc::U238, 1.0 - x235)]
            .into_iter()
            .collect();
        let c = Composition::from_atom(map, &AtomicMasses::MassNumber).unwrap();
        Material::new(qty, c).unwrap()
    }

    // --- Value function ---------------------------------------------------

    #[test]
    fn value_func_rejects_the_closed_ends_of_the_interval() {
        assert!(value_func(0.0).is_err());
        assert!(value_func(-1e-12).is_err());
        assert!(value_func(-0.5).is_err());
        assert!(value_func(1.0).is_err());
        assert!(value_func(1.5).is_err());
        // The open interval is accepted.
        assert!(value_func(1e-12).is_ok());
        assert!(value_func(0.5).is_ok());
        assert!(value_func(1.0 - 1e-12).is_ok());
    }

    #[test]
    fn value_func_is_zero_at_one_half_and_symmetric_about_it() {
        assert!(abs(value_func(0.5).unwrap()) < 1e-15);
        // V(f) == V(1-f): (1-2f)ln(1/f - 1) is even about 0.5, since both
        // factors change sign together.
        for f in [0.01, 0.1, 0.25, 0.4] {
            let a = value_func(f).unwrap();
            let b = value_func(1.0 - f).unwrap();
            assert!(abs(a - b) < 1e-12, "V({f}) = {a}, V({}) = {b}", 1.0 - f);
        }
    }

    #[test]
    fn value_func_matches_the_closed_form_at_the_textbook_assays() {
        // V(f) = (1 - 2f) ln(1/f - 1), evaluated independently here.
        for f in [0.00711, 0.0025, 0.045] {
            let expect = (1.0 - 2.0 * f) * ln(1.0 / f - 1.0);
            assert!(abs(value_func(f).unwrap() - expect) < 1e-15);
        }
    }

    // --- Mass balance -----------------------------------------------------

    /// The textbook case: natural feed 0.711 %, product 4.5 %, tails 0.25 %.
    ///
    /// Closed form, worked by hand:
    ///
    ///   F/P = (xp - xt)/(xf - xt) = (0.045 - 0.0025)/(0.00711 - 0.0025)
    ///                             = 0.0425 / 0.00461
    ///                             = 9.21908893709...
    ///   T/P = (xp - xf)/(xf - xt) = 0.03789 / 0.00461
    ///                             = 8.21908893709...   (= F/P - 1)
    #[test]
    fn feed_and_tails_match_the_closed_form_and_conserve_mass() {
        let a = Assays::new(0.00711, 0.045, 0.0025);
        let p = 1.0;

        let f = feed_qty(p, &a);
        let t = tails_qty(p, &a);

        let f_expect = 0.0425 / 0.00461;
        let t_expect = 0.03789 / 0.00461;
        assert!(abs(f - f_expect) / f_expect < 1e-12, "feed {f}");
        assert!(abs(t - t_expect) / t_expect < 1e-12, "tails {t}");

        // Total mass balance: F = P + T, to machine precision.
        assert!(abs(f - (p + t)) < 1e-12, "F = {f}, P + T = {}", p + t);

        // U-235 mass balance: F*xf = P*xp + T*xt.
        let lhs = f * a.feed();
        let rhs = p * a.product() + t * a.tails();
        assert!(abs(lhs - rhs) < 1e-14, "{lhs} vs {rhs}");
    }

    #[test]
    fn feed_and_tails_are_homogeneous_of_degree_one_in_product() {
        let a = Assays::new(0.00711, 0.045, 0.0025);
        let f1 = feed_qty(1.0, &a);
        let f100 = feed_qty(100.0, &a);
        assert!(abs(f100 - 100.0 * f1) < 1e-10);
    }

    // --- SWU --------------------------------------------------------------

    /// SWU for the textbook case, against the closed form.
    ///
    /// SWU/P = V(xp) + (T/P) V(xt) - (F/P) V(xf), with
    ///
    ///   V(0.045)   = (1 - 0.09)     ln(1/0.045   - 1) = 0.91     ln(21.2222...)
    ///   V(0.0025)  = (1 - 0.005)    ln(1/0.0025  - 1) = 0.995    ln(399)
    ///   V(0.00711) = (1 - 0.01422)  ln(1/0.00711 - 1) = 0.98578  ln(139.64698...)
    ///
    /// MEASURED 2026-09-16 (this crate, release): SWU = 6.861941102009186
    /// for 1 kg of product, from 9.219088937093184 kg of feed. That is
    /// consistent with the standard published ~6.9 SWU/kg for 4.5 % product
    /// at 0.25 % tails.
    #[test]
    fn swu_for_the_textbook_case_matches_the_closed_form() {
        let a = Assays::new(0.00711, 0.045, 0.0025);
        let p = 1.0;

        let v = |f: f64| (1.0 - 2.0 * f) * ln(1.0 / f - 1.0);
        let f_over_p = 0.0425 / 0.00461;
        let t_over_p = 0.03789 / 0.00461;
        let expect = v(0.045) + t_over_p * v(0.0025) - f_over_p * v(0.00711);

        let got = swu_required(p, &a).unwrap();
        assert!(
            abs(got - expect) / abs(expect) < 1e-12,
            "swu {got}, closed form {expect}"
        );

        // The recorded number, to the precision stated in the doc comment.
        assert!(
            abs(got - 6.861_941_102_009_186) < 1e-12,
            "swu drifted from the recorded value: {got}"
        );
        assert!(
            abs(feed_qty(p, &a) - 9.219_088_937_093_184) < 1e-12,
            "feed drifted from the recorded value: {}",
            feed_qty(p, &a)
        );

        // Sanity: the published range for 4.5 % product at 0.25 % tails.
        assert!((6.5..7.3).contains(&got), "swu {got} outside the sane band");
    }

    #[test]
    fn swu_is_positive_and_grows_with_product_assay() {
        let mut last = 0.0;
        for xp in [0.01, 0.02, 0.03, 0.045, 0.1, 0.2] {
            let a = Assays::new(0.00711, xp, 0.0025);
            let swu = swu_required(1.0, &a).unwrap();
            assert!(swu > 0.0, "swu {swu} at xp {xp}");
            assert!(swu > last, "swu not monotone: {swu} after {last}");
            last = swu;
        }
    }

    #[test]
    fn swu_rejects_an_out_of_range_assay() {
        assert!(swu_required(1.0, &Assays::new(0.00711, 1.0, 0.0025)).is_err());
        assert!(swu_required(1.0, &Assays::new(0.00711, 0.045, 0.0)).is_err());
        assert!(swu_required(1.0, &Assays::new(0.0, 0.045, 0.0025)).is_err());
    }

    // --- Assays from a material -------------------------------------------

    #[test]
    fn uranium_assay_atom_returns_the_enrichment_it_was_built_with() {
        let m = u_by_atom(10.0, 0.045);
        let got = uranium_assay(&m);
        assert!(abs(got - 0.045) < 1e-14, "got {got}");
        assert!(abs(uranium_assay_atom(&m) - 0.045) < 1e-14);
    }

    #[test]
    fn uranium_assay_mass_returns_the_enrichment_it_was_built_with() {
        let m = u_by_mass(10.0, 0.00711);
        let got = uranium_assay_mass(&m);
        assert!(abs(got - 0.00711) < 1e-14, "got {got}");
    }

    #[test]
    fn atom_assay_exceeds_mass_assay_for_natural_uranium() {
        // 0.711 % by mass is 0.720 % by atom; the standard pair of figures.
        let m = u_by_mass(1.0, 0.00711);
        let mass_assay = uranium_assay_mass(&m);
        let atom_assay = uranium_assay_atom(&m);
        assert!(atom_assay > mass_assay);
        assert!(
            abs(atom_assay - 0.0072) < 5e-5,
            "atom assay {atom_assay} not near the published 0.720 %"
        );
    }

    #[test]
    fn assay_ignores_nuclides_that_are_not_u235_or_u238() {
        // UO2 at 4.5 % enrichment: the assay is of the uranium, not the oxide.
        // Atom basis: 1 U per 2 O.
        let map: CompMap = [
            (nuc::U235, 0.045),
            (nuc::U238, 0.955),
            (nuc::O16, 2.0),
            (nuc::U234, 1e-4),
        ]
        .into_iter()
        .collect();
        let c = Composition::from_atom(map, &AtomicMasses::MassNumber).unwrap();
        let m = Material::new(10.0, c).unwrap();
        let got = uranium_assay_atom(&m);
        assert!(abs(got - 0.045) < 1e-12, "got {got}");
    }

    #[test]
    fn assay_of_a_material_with_no_uranium_is_zero() {
        let map: CompMap = [(nuc::O16, 1.0)].into_iter().collect();
        let c = Composition::from_atom(map, &AtomicMasses::MassNumber).unwrap();
        let m = Material::new(1.0, c).unwrap();
        assert_eq!(uranium_assay_atom(&m), 0.0);
        assert_eq!(uranium_assay_mass(&m), 0.0);
        assert_eq!(uranium_qty(&m), 0.0);
    }

    #[test]
    fn uranium_qty_excludes_the_oxygen() {
        // 10 kg of UO2, atom basis 1 U : 2 O, natural uranium.
        let map: CompMap = [(nuc::U235, 0.00711), (nuc::U238, 0.99289), (nuc::O16, 2.0)]
            .into_iter()
            .collect();
        let c = Composition::from_atom(map, &AtomicMasses::MassNumber).unwrap();
        let m = Material::new(10.0, c).unwrap();

        // Mass fraction of uranium in UO2 with A(U) ~ 238: 238/(238+32).
        let expect = 10.0 * 238.02289 / (238.02289 + 32.0);
        let got = uranium_qty(&m);
        // Mass-number masses, so a percent-level tolerance is right here.
        assert!(abs(got - expect) / expect < 0.01, "got {got}, want ~{expect}");
        assert!(got < 10.0);
    }

    /// End-to-end: enrich 9.2191 kg of natural uranium into 1 kg of 4.5 %
    /// product, and check the assays of the constructed streams come back.
    #[test]
    fn round_trip_through_materials_reproduces_the_assays() {
        let a = Assays::new(0.00711, 0.045, 0.0025);
        let p_qty = 1.0;
        let f_qty = feed_qty(p_qty, &a);
        let t_qty = tails_qty(p_qty, &a);

        let feed = u_by_atom(f_qty, a.feed());
        let product = u_by_atom(p_qty, a.product());
        let tails = u_by_atom(t_qty, a.tails());

        assert!(abs(uranium_assay(&feed) - a.feed()) < 1e-14);
        assert!(abs(uranium_assay(&product) - a.product()) < 1e-14);
        assert!(abs(uranium_assay(&tails) - a.tails()) < 1e-14);

        // Uranium mass is conserved across the split.
        let m_in = uranium_qty(&feed);
        let m_out = uranium_qty(&product) + uranium_qty(&tails);
        assert!(abs(m_in - m_out) < 1e-12, "{m_in} in, {m_out} out");
    }
}
