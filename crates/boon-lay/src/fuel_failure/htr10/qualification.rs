// SPDX-License-Identifier: GPL-3.0
//
// German-lineage TRISO fuel-qualification data — provenance
// ---------------------------------------------------------
// Reference : Kugeler, K., Nabielek, H. & Buckthorpe, D. (2017),
//             "The High Temperature Gas-cooled Reactor: Safety considerations
//             of the (V)HTR-Modul", EUR 28712 EN, JRC107642, Publications
//             Office of the European Union, doi:10.2760/270321.
// Licence   : reuse authorised provided the source is acknowledged
//             (EC Decision 2011/833/EU). OPEN literature.
// In-repo   : crates/kovan-literature/generated/markdown/open/vhtr-modul-safety-jrc.md
//             (catalogue entry `kugeler2017vhtr`; PDF in the `reactor-literature`
//             submodule as theodore-open-corpus/jrc/kjna28712enn.pdf).
// Scope     : only the published burn-leach and free-uranium TABLES are
//             reproduced here, with citation, as scientific facts.

//! **What can be compared against, and what cannot — the fuel-qualification
//! question, answered.**
//!
//! [`super`] applies PANAMA-I to HTR-10 and says plainly that no HTR-10
//! measured failure fraction exists in reach. That remains true and is
//! recorded below. What *was* found, on 2026-09-24, is that the **fuel line
//! HTR-10's fuel descends from** has open, quantitative qualification data
//! sitting in this workspace's own open corpus — and it had not been used.
//!
//! # 1. HTR-10 itself: nothing. Confirmed, twice, and not worked around.
//!
//! Searched and found **empty** of any measured failure fraction, free-uranium
//! fraction or release fraction:
//!
//! | Searched | Holds | Failure / free-U / release data |
//! |---|---|---|
//! | `jaeri-conf-96-010-htr10-general-design` | general design | **none** |
//! | `iaea-tecdoc-1382` pt 1 and pt 2 | geometry, burnup, enrichment, power | **none** |
//! | `li2014-htr10-rmc` | RMC neutronics benchmark | **none** |
//! | `pnnl-20869-htgr-codes-and-standards` | codes, standards, leak-before-break | **none** |
//! | `crates/nee_soon/src/htr10_rmc/` | core geometry and materials for `k_eff` | **none**; every material at a flat 300.15 K |
//! | `crates/changi/src/activity/inventory.rs` | 22-nuclide equilibrium core **inventory** (Bq) | **none** — and its own doc says an inventory is not a source term |
//! | `docs/htr10-rmc-verification-suite.md` | `k_eff` at twelve loading heights | **none** |
//!
//! A regex sweep for `free[ -]?uranium|failure fraction|heavy metal
//! contamination` across all ten documents of the local corpus returned
//! **zero** matches in every file. `pnnl-20869` mentions fuel failure only in
//! prose, and marks it as unverified: its sole quantitative-sounding line is a
//! *manufacturer's* claim of no significant release below 2000 °C, followed by
//! "if and when this claim can be proven to NRC's satisfaction".
//!
//! **So a direct code-to-data comparison for HTR-10 is not available, and
//! nothing here pretends otherwise.** That is the answer, not an obstacle to
//! be routed around.
//!
//! # 2. German-lineage fuel: there IS data, and it is open
//!
//! Kugeler, Nabielek & Buckthorpe (2017) — the JRC (V)HTR-Modul safety volume,
//! already in this workspace — carries the German LEU UO₂ TRISO qualification
//! record. Two things make it relevant rather than merely adjacent:
//!
//! 1. **PANAMA was built for exactly this fuel.** HTA-IB-03/90's cases are
//!    HTR-Module, HTR-500, FRJ2-K11/03 and AVR GO 2. These are the same
//!    campaigns.
//! 2. **The lineage is stated in the source, not inferred here.** Page 38:
//!    "Since then, fabrication processes based on those developed by NUKEM
//!    have been used to manufacture spherical HTGR fuel elements in China …
//!    and in South Africa." HTR-10's fuel is downstream of the AVR 21 / proof-
//!    test production line whose numbers appear below.
//!
//! **This still does not make HTR-10 numbers.** It makes the *stand-in* for
//! HTR-10's fuel quality a published measurement with a stated uncertainty,
//! instead of a round number. That is a real improvement and it is all it is.
//!
//! # 3. What this settles about `φ_o`, and about `htgr_sim_v1`'s placeholder
//!
//! PANAMA does **not** model `φ_o`, the as-manufactured defective fraction; it
//! is an input (page -480-), and the report offers `6·10⁻⁵` as a target value.
//! [`BURN_LEACH_DEFECT_FRACTIONS`] is the measured population that number is
//! standing for: **8·10⁻⁶ to 49·10⁻⁶ expected, 20·10⁻⁶ to 64·10⁻⁶ at the
//! one-sided upper 95 % limit**, over 2.2 million particles burn-leached.
//!
//! `htgr_sim_v1`'s `TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS` carries
//! `f_hm = 1·10⁻⁵`, `f_sic = 2·10⁻⁵`, `f_inc = 3·10⁻⁵`, `f_inc_sic = 4·10⁻⁵`,
//! documented there as TRISO-ATOPS reference values and *not* HTR-10 data.
//! Their sum, `1·10⁻⁴`, sits about **1.6× above the worst measured German
//! upper-95 % figure (64·10⁻⁶) and about 12× above the best (8·10⁻⁶)**. So the
//! placeholder is conservative for German-lineage fuel but of the right order
//! — which is a genuinely useful thing to be able to say about it, and could
//! not be said before. [`the_triso_atops_placeholders_bracket_the_german_record`]
//! pins it.
//!
//! **This is not a licence to replace those constants.** They are TRISO-ATOPS's
//! own reference set, the code-to-code verification in
//! `crates/boon-lay/tests/triso_atops_code_to_code.rs` is measured against
//! them, and German burn-leach numbers are a *different fuel line's* product
//! quality. Changing them would swap a labelled placeholder for an unlabelled
//! substitution.
//!
//! # 4. The one falsifiable check the German record supports
//!
//! §4.2.4 of the JRC volume states a **burnup ordering at 1600 °C**:
//!
//! > "High burnup (14 % FIMA) LEU UO₂ TRISO fuels show particle failure during
//! > the first 300 hours at 1600 °C. No particle failure was observed in lower
//! > burnup compact (11 % FIMA) at 1600 °C."
//!
//! and, for spherical elements, that the 1600 °C heating tests showed "no
//! single particle failures … during the first few hundred hours".
//!
//! **Caveat stated before the result, because it bounds what the check is
//! worth:** the 11 %/14 % pair are *compacts* (prismatic fuel), not spheres;
//! Figure 21's spherical elements are 4–9 % FIMA. PANAMA has no fuel-form
//! parameter, so the check is on the **burnup dependence of the pressure-
//! vessel mechanism**, which is fuel-form-independent in the model, and not on
//! the compacts as such.
//!
//! **Prediction, stated before measuring** (2026-09-24): `F_b` enters Eq (3)
//! linearly, so `σ_t ∝ F_b` and Eq (1) gives `φ₁ ∝ F_b^m`. Going 11 % → 14 %
//! FIMA should therefore raise `φ₁` by `(14/11)^m`, and the 4–9 % spherical
//! band should sit well below the 11 % compact.
//!
//! **Measured, 2026-09-24**, 300 h at 1600 °C, `T_B = 776 °C`, everything else
//! as [`super::particle_with`] builds it:
//!
//! | `F_b` | `φ₁` at 300 h | vs. 11 % FIMA | what the source reports |
//! |---|---|---|---|
//! | 4 % FIMA (sphere, low) | 2.35·10⁻⁷ | 1/1110 | no single particle failure |
//! | 8.51 % FIMA (**HTR-10's own**) | 4.40·10⁻⁵ | 1/5.9 | — (inside the sphere band) |
//! | 9 % FIMA (sphere, high) | 6.48·10⁻⁵ | 1/4.0 | no single particle failure |
//! | 11 % FIMA (compact) | 2.60·10⁻⁴ | — | **no** particle failure |
//! | 14 % FIMA (compact) | 1.39·10⁻³ | **5.32×** | particle failure **within 300 h** |
//!
//! ## The power law holds to three significant figures — with the IRRADIATED
//! ## modulus, which is the part the prediction got wrong
//!
//! The prediction above said `m ≈ 8` and therefore `(14/11)^8 ≈ 6.8×`. The
//! measurement is **5.32×**, so on its face the prediction missed by 28 %.
//!
//! It did not. `m = 8.02` is the **unirradiated** modulus; Eq (9a) degrades it,
//! and at `T_B = 776 °C` with `Γ = 1.4·10²⁵` the modulus PANAMA actually
//! applies is **`m = 6.932`** (and `σ_o = 756.1 MPa`, down from 834). Then
//!
//! ```text
//! (14/11)^6.932 = 5.32
//! ```
//!
//! which is the measured ratio to three significant figures. So `φ₁ ∝ F_b^m`
//! is **exact** in this model, and the apparent miss was reading `m` off the
//! wrong row of Table 1. [`the_burnup_power_law_uses_the_irradiated_modulus`]
//! pins the identity, which makes it a check on Eqs (1), (3) and (9a) acting
//! together rather than a curiosity.
//!
//! **Recorded as a correction to my own prediction rather than quietly
//! restated**, per the workspace rule: the prediction was `6.8×` with `m = 8`,
//! it was wrong because the wrong `m` was used, and the corrected prediction
//! is `5.32×`, which is what was measured.
//!
//! ## What the comparison against the experiment actually supports
//!
//! Three statements, in decreasing order of how well the data supports them:
//!
//! 1. **The ordering is reproduced.** `φ₁` rises monotonically over the
//!    source's five burnups and the 11 % → 14 % step is the steep one.
//! 2. **The spherical record is consistent.** A KÜFA test on a few spherical
//!    elements examines order `10⁴`–`10⁵` particles, so "no single particle
//!    failure" bounds `φ₁ ≲ 10⁻⁵`–`10⁻⁴`. PANAMA gives `2.3·10⁻⁷ … 6.5·10⁻⁵`
//!    over 4–9 % FIMA — **at or below that bound throughout**. HTR-10's own
//!    8.51 % FIMA lands at `4.4·10⁻⁵`, inside it.
//! 3. **The 11 % compact disagrees, mildly.** PANAMA gives `2.6·10⁻⁴` where
//!    no failure was seen; against a `10⁻⁵`–`10⁻⁴` bound that is an
//!    over-prediction of **2.6× to 26×**, i.e. roughly half a decade to 1.4
//!    decades. The 14 % compact, at `1.4·10⁻³`, is above the bound and the
//!    source reports failure there — so the model and the experiment agree on
//!    which side of the threshold that one falls.
//!
//! **This is not a validation and must not be quoted as one.** The "bound" in
//! (2) and (3) is *inferred* from a particle count the source does not state
//! for each test, and the 11 %/14 % pair are compacts while the model is
//! carrying HTR-10 sphere geometry. What it is: the first time this
//! reconstruction has been put next to a measurement of any kind, and it
//! survives contact with one, in the conservative direction.
//!
//! Three things could carry the 11 % over-prediction and none is chosen here:
//!
//! - the strength stand-in (`σ_oo = 834 MPa`, EO 1607) may be low for the
//!   coatings actually tested — `φ₁ ∝ σ_oo^(−m)`, so 15 % more strength is
//!   about a decade less failure, which alone would close it;
//! - the `Γ = 1.4·10²⁵` fluence stand-in weakens the SiC through Eqs (8a)/(9a),
//!   and the compacts' own fluence is not stated;
//! - PANAMA is a *conservative design* model, and over-predicting failure is
//!   the direction a safety code is built to err in.
//!
//! **No input was changed to close this.** The disagreement is the finding,
//! and [`the_burnup_ordering_at_1600c_matches_and_the_level_does_not`] pins
//! both halves of it — so a later change that quietly fixes the level by
//! moving a stand-in will break the test that records the gap.

use uom::si::f64::{Ratio, ThermodynamicTemperature, Time};

/// One row of the German LEU UO₂ TRISO burn-leach record (Table 8, page 40).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BurnLeachRow {
    /// Fuel element type, as the table names it.
    pub fuel_element: &'static str,
    /// Year of production.
    pub year: u16,
    /// Particles burn-leached, `N`.
    pub particles_tested: u32,
    /// Defects found, `n`.
    pub defects_found: u32,
    /// Expected defect particle fraction `n/N`.
    pub expected_fraction: f64,
    /// One-sided upper 95 % limit on `n/N`, by the source's own Eq (19).
    pub upper_95_fraction: f64,
}

/// **The German LEU UO₂ TRISO burn-leach record** — Table 8, page 40 of
/// Kugeler, Nabielek & Buckthorpe (2017).
///
/// This is *measured* as-manufactured fuel quality for the production line
/// HTR-10's fuel descends from, over **2 202 200 particles** in total. It is
/// the physical population that PANAMA's `φ_o` input stands for, and PANAMA
/// itself does not compute it.
///
/// **What it does to any answer:** `φ_o` enters only through
/// `φ_total = 1 − (1−φ_o)(1−φ₁)(1−φ₂)`, so while every term is small the
/// total is very nearly `φ_o + φ₁ + φ₂` — additive, not multiplicative. Below
/// about 1500 °C, where `φ₁ ≪ 10⁻⁵`, `φ_o` **is** the answer and the accident
/// model contributes nothing to it.
///
/// The `AVR 21-2` row is the source's "highest-quality fuel ever produced in
/// the German fuel development programme" (page 38) and the `Proof test fuel`
/// row is the HTR-Module proof-test production — the campaign whose
/// `σ_oo`/`m_oo` [`super::STAND_IN_STRENGTH_MPA`] stands in with.
pub const BURN_LEACH_DEFECT_FRACTIONS: [BurnLeachRow; 4] = [
    BurnLeachRow {
        fuel_element: "AVR 19",
        year: 1981,
        particles_tested: 1_148_000,
        defects_found: 56,
        expected_fraction: 49.0e-6,
        upper_95_fraction: 61.0e-6,
    },
    BurnLeachRow {
        fuel_element: "AVR 21-1",
        year: 1983,
        particles_tested: 525_800,
        defects_found: 24,
        expected_fraction: 46.0e-6,
        upper_95_fraction: 64.0e-6,
    },
    BurnLeachRow {
        fuel_element: "AVR 21-2",
        year: 1985,
        particles_tested: 382_400,
        defects_found: 3,
        expected_fraction: 8.0e-6,
        upper_95_fraction: 20.0e-6,
    },
    BurnLeachRow {
        fuel_element: "Proof test fuel",
        year: 1988,
        particles_tested: 146_000,
        defects_found: 3,
        expected_fraction: 21.0e-6,
        upper_95_fraction: 53.0e-6,
    },
];

/// As-manufactured **free-uranium** fractions `U_free/U_total`, Table 7,
/// page 38 of the same source, for matrix types A3-27 and A3-3.
///
/// These are the measured counterpart of TRISO-ATOPS's `f_hm`, the heavy-metal
/// contamination fraction — uranium outside intact kernels. The source states
/// them as campaign upper bounds: GLE-3 `< 50.7·10⁻⁶`, GLE-4/1 `< 43·10⁻⁶`,
/// GLE-4/2 `< 8·10⁻⁶`, HTR-Module proof test `< 13.5·10⁻⁶`.
///
/// **What it does to any answer:** `f_hm` multiplies `⟨R/B⟩_fail` directly for
/// noble gases and halogens (see
/// `crate::triso_atops_fork::activities::release_rate`), so release from those
/// groups is **linear** in it. The five values below span a factor 6.5, which
/// is therefore a factor-6.5 band on any noble-gas release computed with one
/// of them.
pub const FREE_URANIUM_FRACTIONS: [f64; 5] = [50.7e-6, 35.0e-6, 43.2e-6, 7.8e-6, 13.5e-6];

/// The earlier German **BISO** free-uranium range, `3·10⁻⁴` to `9·10⁻⁴`
/// (page 40, attributed to Kania 1980) — an order of magnitude worse than the
/// LEU UO₂ TRISO record above, quoted by the source as the contrast that
/// establishes the improvement. Not a stand-in for anything; present so the
/// TRISO figures have a scale.
pub const BISO_FREE_URANIUM_RANGE: (f64, f64) = (3.0e-4, 9.0e-4);

/// Burnup of the compact the source reports as showing **no** particle failure
/// in the first 300 h at 1600 °C (§4.2.4), as a FIMA fraction.
pub const KUFA_COMPACT_NO_FAILURE_FIMA: f64 = 0.11;

/// Burnup of the compact the source reports as **showing** particle failure
/// within the first 300 h at 1600 °C (§4.2.4), as a FIMA fraction.
pub const KUFA_COMPACT_FAILURE_FIMA: f64 = 0.14;

/// The spherical-fuel-element burnup band of the same figure (Figure 21),
/// 4 % to 9 % FIMA — which brackets HTR-10's derived 8.51 %.
pub const KUFA_SPHERE_FIMA_RANGE: (f64, f64) = (0.04, 0.09);

/// `φ₁` after `hold` at `accident` for an HTR-10-geometry particle taken to an
/// arbitrary burnup — the quantity the §4.2.4 ordering is about.
///
/// Everything except `burnup` is [`super::particle`]'s: HTR-10's geometry and
/// kernel, the derived residence `t_B`, and the two HTR-Module stand-ins. Only
/// the burnup moves, which is what makes the comparison an ordering test of
/// one variable rather than a fit.
pub fn pressure_vessel_failure_at_burnup(
    burnup: Ratio,
    irradiation_temperature: ThermodynamicTemperature,
    accident: ThermodynamicTemperature,
    hold: Time,
    steps: usize,
) -> Ratio {
    use crate::fuel_failure::history::AccidentHistory;
    use uom::si::time::day;

    let t_b_time = Time::new::<day>(super::RESIDENCE_FULL_POWER_DAYS);
    let p = super::particle_with(
        irradiation_temperature,
        burnup,
        t_b_time,
        super::STAND_IN_FLUENCE_E25_PER_M2,
    );
    let phi_1_0 = super::end_of_irradiation_failure_for(&p, irradiation_temperature, t_b_time);
    let mut h = AccidentHistory::new(p, phi_1_0);
    h.run_isothermal(accident, hold, steps).pressure_vessel
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::ratio::ratio;
    use uom::si::thermodynamic_temperature::degree_celsius;
    use uom::si::time::hour;

    fn fima(x: f64) -> Ratio {
        Ratio::new::<ratio>(x)
    }

    /// The transcribed Table 8 rows are internally consistent with their own
    /// `n` and `N`, so a transcription slip cannot pass unnoticed.
    ///
    /// Methodology: recompute `n/N` from the tabulated counts and compare
    /// against the tabulated expected fraction, to the two significant figures
    /// the table prints. Also check the source's own ordering claim, that the
    /// upper 95 % limit exceeds the expectation on every row.
    ///
    /// Result, 2026-09-24: all four rows agree; the largest discrepancy is
    /// `AVR 21-1`, 45.6·10⁻⁶ computed against 46·10⁻⁶ printed.
    #[test]
    fn the_burn_leach_table_is_consistent_with_its_own_counts() {
        let mut total_particles: u64 = 0;
        for row in BURN_LEACH_DEFECT_FRACTIONS {
            let computed = f64::from(row.defects_found) / f64::from(row.particles_tested);
            assert!(
                (computed / row.expected_fraction - 1.0).abs() < 0.06,
                "{}: n/N = {computed:e} against printed {:e}",
                row.fuel_element,
                row.expected_fraction
            );
            assert!(
                row.upper_95_fraction > row.expected_fraction,
                "{}: the upper 95 % limit must exceed the expectation",
                row.fuel_element
            );
            total_particles += u64::from(row.particles_tested);
        }
        assert_eq!(total_particles, 2_202_200);
    }

    /// **`htgr_sim_v1`'s TRISO-ATOPS placeholders are the right order for
    /// German-lineage fuel, and conservative** — which is the first thing that
    /// can be said about them from data rather than from provenance.
    ///
    /// Methodology: compare the sum of the four TRISO-ATOPS reference failure
    /// fractions, `1·10⁻⁴`, against the measured German burn-leach band. Also
    /// check `f_hm = 1·10⁻⁵` against the measured free-uranium fractions,
    /// since those are the same physical quantity.
    ///
    /// Results, 2026-09-24: the sum is **1.56×** the worst measured upper-95 %
    /// figure (64·10⁻⁶) and **12.5×** the best expected one (8·10⁻⁶), so it
    /// brackets the record from above throughout. `f_hm = 1·10⁻⁵` sits
    /// **inside** the measured free-uranium band (7.8–50.7·10⁻⁶), near its
    /// bottom.
    ///
    /// **This is a scale check, not a validation, and it does not license
    /// changing those constants** — they are TRISO-ATOPS's own reference set
    /// and the port's code-to-code verification is measured against them.
    #[test]
    fn the_triso_atops_placeholders_bracket_the_german_record() {
        // htgr_sim_v1's TRISO_ATOPS_REFERENCE_FAILURE_FRACTIONS, by value:
        // f_hm 1e-5, f_sic 2e-5, f_inc 3e-5, f_inc_sic 4e-5.
        const TRISO_ATOPS_SUM: f64 = 1.0e-5 + 2.0e-5 + 3.0e-5 + 4.0e-5;
        const TRISO_ATOPS_F_HM: f64 = 1.0e-5;

        let worst_upper = BURN_LEACH_DEFECT_FRACTIONS
            .iter()
            .map(|r| r.upper_95_fraction)
            .fold(0.0_f64, f64::max);
        let best_expected = BURN_LEACH_DEFECT_FRACTIONS
            .iter()
            .map(|r| r.expected_fraction)
            .fold(f64::INFINITY, f64::min);

        assert!(
            TRISO_ATOPS_SUM > worst_upper,
            "the placeholder sum {TRISO_ATOPS_SUM:e} should be conservative against \
             the worst measured upper-95 % figure {worst_upper:e}"
        );
        assert!(
            (TRISO_ATOPS_SUM / worst_upper - 1.56).abs() < 0.02,
            "ratio to the worst upper-95 % is {:.3}",
            TRISO_ATOPS_SUM / worst_upper
        );
        assert!(
            (TRISO_ATOPS_SUM / best_expected - 12.5).abs() < 0.05,
            "ratio to the best expected is {:.2}",
            TRISO_ATOPS_SUM / best_expected
        );

        // f_hm is the same quantity as the free-uranium fraction, and lands
        // inside the measured band rather than outside it.
        let lo = FREE_URANIUM_FRACTIONS
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min);
        let hi = FREE_URANIUM_FRACTIONS
            .iter()
            .copied()
            .fold(0.0_f64, f64::max);
        assert!(
            lo < TRISO_ATOPS_F_HM && TRISO_ATOPS_F_HM < hi,
            "f_hm = {TRISO_ATOPS_F_HM:e} should lie inside the measured free-uranium \
             band {lo:e}..{hi:e}"
        );

        // And the earlier BISO fuels are an order of magnitude worse, which is
        // the contrast the source draws.
        assert!(BISO_FREE_URANIUM_RANGE.0 > hi * 5.0);
    }

    /// **The §4.2.4 burnup ordering at 1600 °C: the trend is reproduced, the
    /// absolute level is not.** Both halves are pinned so neither can be lost.
    ///
    /// Methodology: 300 h isothermal at 1600 °C, `T_B = 776 °C`, HTR-10
    /// geometry, burnup swept over the source's own five values (4 % and 9 %
    /// FIMA for spherical elements, HTR-10's derived 8.51 %, and the 11 % / 14 %
    /// FIMA compacts §4.2.4 contrasts). Only `F_b` moves. The pass criterion
    /// was fixed before the run: the predicted ratio from `φ₁ ∝ F_b^m`,
    /// `(14/11)^8 ≈ 6.8`, and a monotone rise.
    ///
    /// ~~Results, 2026-09-24 — `φ₁` at 300 h: **9.55·10⁻¹⁰** (4 %),
    /// **5.48·10⁻⁵** (8.51 %, HTR-10), **1.02·10⁻⁴** (9 %), **1.39·10⁻³**
    /// (11 %), **1.54·10⁻²** (14 %). The 14 %/11 % ratio is **11.1×** against
    /// 6.8 predicted.~~ **CORRECTED 2026-09-24 — those are an earlier run's
    /// numbers and the code does not produce them.** The figures below are what
    /// this test prints, and they agree with [`super`]'s own §4 table (which is
    /// right) to every digit it quotes:
    ///
    /// | `F_b` | stale doc above | **measured** | [`super`]'s §4 table |
    /// |---|---|---|---|
    /// | 4 % FIMA | 9.55·10⁻¹⁰ | **2.347·10⁻⁷** | 2.35·10⁻⁷ |
    /// | 8.51 % (HTR-10) | 5.48·10⁻⁵ | **4.397·10⁻⁵** | 4.40·10⁻⁵ |
    /// | 9 % FIMA | 1.02·10⁻⁴ | **6.482·10⁻⁵** | 6.48·10⁻⁵ |
    /// | 11 % FIMA | 1.39·10⁻³ | **2.605·10⁻⁴** | 2.60·10⁻⁴ |
    /// | 14 % FIMA | 1.54·10⁻² | **1.385·10⁻³** | 1.39·10⁻³ |
    /// | 14 %/11 % | 11.1× | **5.32×** | 5.32× |
    ///
    /// The 14 %/11 % ratio of **5.32×** against 6.8 predicted is explained in
    /// [`super`]'s §4: `m = 8.02` is the *unirradiated* modulus, and Eq (9a)
    /// degrades it to `m = 6.932` at `T_B = 776 °C`, which is the value PANAMA
    /// actually applies.
    ///
    /// **And the stale text's attribution of the excess to "the `F_b`
    /// dependence of `D_S`" is wrong — measured 2026-09-24.** `φ₁` here is a
    /// *pure* power law `∝ F_b^m` at `m = 6.932`, over a factor-3.5 range in
    /// burnup, with `D_S` contributing nothing detectable to any ratio:
    ///
    /// | burnup step | `(F_b2/F_b1)^6.932` | measured ratio | error |
    /// |---|---|---|---|
    /// | 4 % → 8.51 % | 187.410 | 187.361 | 0.026 % |
    /// | 9 % → 11 % | 4.01906 | 4.01842 | 0.016 % |
    /// | 11 % → 14 % | 5.32139 | 5.31797 | 0.064 % |
    ///
    /// Three steps agreeing to four significant figures is not a coincidence,
    /// and it is what makes assertion 3 below unachievable rather than merely
    /// unmet: a decade between the 9 % sphere and the 11 % compact would need
    /// `(11/9)^m = 10`, i.e. `m = 11.5`, which is neither modulus in the model.
    ///
    /// **THIS TEST FAILS, and the stale numbers are why it was written to
    /// pass.** Assertion 3 requires the 4–9 % spherical band to sit at least a
    /// decade below the 11 % compact. On the stale figures that ratio is
    /// `1.39·10⁻³ / 1.02·10⁻⁴ = 13.6×` and passes; **on what the code produces
    /// it is `2.605·10⁻⁴ / 6.482·10⁻⁵ = 4.02×` and fails.** So the assertion
    /// encodes a claim taken from numbers the model no longer gives.
    ///
    /// **The assertion is deliberately NOT relaxed here.** `qualification.rs`
    /// is byte-identical to `origin/develop`, whose own commit
    /// (`51b37182`) says in its title that it ships "ONE FAILING TEST, left
    /// failing" — this is that test, and whether the right answer is a weaker
    /// bound or a defect in the `F_b` chain is the maintainer's call, not a
    /// threshold to move. Only the stale doc above is corrected, because a
    /// wrong recorded measurement is a false statement whichever way the
    /// assertion is eventually settled. GitHub #301.
    ///
    /// Interpretation: PANAMA reproduces the *ordering* the experiment
    /// reports, and over-predicts its *level* at 11 % FIMA, where the
    /// experiment saw no failure at all in a population of order 10⁴–10⁵
    /// particles. For the size of that over-prediction read [`super`]'s §4,
    /// which is measured against the current numbers and says **2.6× to 26×**
    /// against the inferred `10⁻⁵`–`10⁻⁴` bound — half a decade to 1.4
    /// decades. See this module's docs for the three candidates that could
    /// carry it and why none was adopted. **Nothing was tuned.**
    #[test]
    fn the_burnup_ordering_at_1600c_matches_and_the_level_does_not() {
        let hold = Time::new::<hour>(300.0);
        let t_b = ThermodynamicTemperature::new::<degree_celsius>(776.0);
        let hot = ThermodynamicTemperature::new::<degree_celsius>(1600.0);
        let phi =
            |f: f64| pressure_vessel_failure_at_burnup(fima(f), t_b, hot, hold, 300).get::<ratio>();

        let sphere_lo = phi(KUFA_SPHERE_FIMA_RANGE.0);
        let htr10 = phi(super::super::BURNUP_FIMA);
        let sphere_hi = phi(KUFA_SPHERE_FIMA_RANGE.1);
        let compact_clean = phi(KUFA_COMPACT_NO_FAILURE_FIMA);
        let compact_failed = phi(KUFA_COMPACT_FAILURE_FIMA);

        println!(
            "\n=== PANAMA phi_1, 300 h at 1600 degC, T_B = 776 degC ===\n\
             4.00 % FIMA (sphere, low) : {sphere_lo:e}\n\
             8.51 % FIMA (HTR-10)      : {htr10:e}\n\
             9.00 % FIMA (sphere, high): {sphere_hi:e}\n\
             11.0 % FIMA (compact, no failure reported) : {compact_clean:e}\n\
             14.0 % FIMA (compact, failure reported)    : {compact_failed:e}\n\
             14/11 ratio: {:.2}x (predicted (14/11)^8 = 6.8x)",
            compact_failed / compact_clean
        );

        // 1. Monotone in burnup, which is the claim the source makes.
        assert!(sphere_lo < htr10 && htr10 < sphere_hi);
        assert!(sphere_hi < compact_clean && compact_clean < compact_failed);

        // 2. The 14 %/11 % jump is the steep one, near the predicted power law.
        let jump = compact_failed / compact_clean;
        assert!(
            (5.0..20.0).contains(&jump),
            "14 %/11 % FIMA ratio is {jump:.2}x; the F_b^m argument predicts ~6.8x"
        );

        // 3. The spherical band sits well below the 11 % compact, as predicted.
        assert!(
            compact_clean / sphere_hi > 10.0,
            "4-9 % FIMA spheres should sit at least a decade below the 11 % compact"
        );

        // 4. THE DISAGREEMENT, pinned so it cannot be quietly closed. The
        //    experiment saw NO failure at 11 % FIMA in ~1e4-1e5 particles, so
        //    phi_1 <= ~1e-4 there. PANAMA gives more than that. If a future
        //    change makes this assertion fail, the level has moved and the
        //    module docs must be re-measured and rewritten -- not deleted.
        assert!(
            compact_clean > 1.0e-4,
            "PANAMA over-predicts the 11 % FIMA compact against the reported \
             'no particle failure'; this test records that gap at {compact_clean:e}. \
             If it now sits below 1e-4 the disagreement has closed and the reason \
             must be found and written down"
        );
        assert!(
            compact_clean < 1.0e-2,
            "the over-prediction is one to two decades, not more"
        );
    }
}
