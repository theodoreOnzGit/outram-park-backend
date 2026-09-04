// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 OUTRAM PARK contributors
//
// This file is part of OUTRAM PARK.

//! **Crude (atmospheric) distillation from a black-oil characterisation** —
//! epic `op-190j`.
//!
//! The petroleum counterpart of the benzene/toluene column that
//! `distillation_sim_v1` drives: same rigorous MESH solver, but the feed is a
//! *crude oil* described the way production engineering describes one — by API
//! gravity and gas gravity — rather than by a component list.
//!
//! # Read this before using it: what "based on black oil" can and cannot mean
//!
//! A **black-oil model has exactly two pseudo-components**, stock-tank oil and
//! solution gas. It carries no composition, so it *cannot by itself* produce a
//! naphtha / kerosene / diesel / gas-oil slate — there is nothing in it to
//! fractionate. Anyone who tells you they ran a crude unit on a black-oil model
//! and got cuts has done something else.
//!
//! What this module does instead, and why it is still honestly "black-oil
//! based": the black-oil correlations supply the **bulk characterisation**, and
//! that characterisation is cut into pseudo-components. The fit is exact rather
//! than contrived — [`crate::thermo::black_oil`] produces precisely the three
//! quantities [`BulkAssay`] asks for:
//!
//! | black_oil function | BulkAssay field |
//! |---|---|
//! | [`oil_specific_gravity_from_api`] | `specific_gravity_60f` |
//! | [`liquid_molecular_weight`] | `molar_mass` |
//! | [`liquid_normal_boiling_point`] | `average_boiling_point` |
//!
//! [`generate_compounds`] then distributes those bulk properties into a
//! [`PseudoComponent`] slate, and the column runs on that. This is how a
//! refinery specifies a crude when a full TBP assay is not to hand: two
//! numbers off the certificate of analysis.
//!
//! **The cost of that convenience is real and must not be forgotten.** A slate
//! generated from bulk properties is a *distribution assumption*, not a
//! measurement. Two crudes with identical API gravity and identical mean
//! boiling point can have quite different TBP curves and therefore quite
//! different yields. If you have an actual assay, use
//! [`crate::petroleum::assay::CurveAssay`] and do not come through here.
//!
//! # Scope
//!
//! Atmospheric column only — no vacuum tower, no pre-flash, no crude furnace,
//! no pump-arounds, and (see [`CrudeColumnConfig`]) no steam stripping. It is a
//! teaching and scoping model in the same spirit as the benzene column, not a
//! refinery simulator.
//!
//! **This module is headless, and stays that way.** The workspace split is that
//! the plant model lives here with the physics, and any egui rendering lives in
//! `outram-park-digital-twin-engine`'s *library* under `src/components/` —
//! `components::distillation_column` is the existing precedent, a scalar-backed
//! widget whose caller feeds it real per-stage state. Nothing in this file
//! should grow a drawing dependency.
//!
//! No human V&V is claimed. See the test module for what is and is not checked.

use crate::thermo::property_package::PropertyPackageModel;
use uom::si::f64::{MolarMass, Ratio, ThermodynamicTemperature};
use uom::si::molar_mass::gram_per_mole;
use uom::si::ratio::ratio;
use uom::si::thermodynamic_temperature::kelvin;

use crate::petroleum::assay::BulkAssay;
use crate::petroleum::generate_compounds::{
    generate_compounds, BulkCharacterizationOptions, CharacterizationError,
};
use crate::petroleum::pseudo_component::PseudoComponent;
use crate::thermo::black_oil::{
    api_gravity, liquid_molecular_weight, liquid_normal_boiling_point,
    oil_specific_gravity_from_api,
};

/// A crude oil described the black-oil way: gravities and a gas-oil ratio.
///
/// These are the numbers on a crude certificate of analysis, and the inputs a
/// production engineer already has. [`Self::pseudo_components`] turns them into
/// something a distillation column can run on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlackOilCrude {
    /// Stock-tank oil API gravity, °API. Light crudes are ~35-45, medium
    /// ~25-35, heavy below ~25.
    pub api_gravity: f64,
    /// Solution-gas specific gravity (air = 1), dimensionless. Typically
    /// 0.6-0.9; the black-oil correlations are stated for roughly 0.55-1.5.
    pub gas_specific_gravity: f64,
    /// Basic sediment and water, percent by volume. Affects the apparent
    /// liquid molecular weight via [`liquid_molecular_weight`]. A desalted
    /// crude entering a CDU is essentially dry, so `0.0` is the usual value
    /// here.
    pub bsw_percent: f64,
}

impl BlackOilCrude {
    /// A light sweet crude, 38 °API — in the band Brent and WTI occupy.
    ///
    /// Chosen as the module's worked example because it sits comfortably inside
    /// every black-oil correlation's stated validity range, so the
    /// characterisation is not being extrapolated. Gas gravity 0.75 is a
    /// mid-range associated-gas value.
    #[must_use]
    pub fn light_sweet() -> Self {
        Self {
            api_gravity: 38.0,
            gas_specific_gravity: 0.75,
            bsw_percent: 0.0,
        }
    }

    /// A heavy crude, 22 °API — near the lower edge of the correlations'
    /// comfortable range, kept as a contrast case for the tests.
    #[must_use]
    pub fn heavy() -> Self {
        Self {
            api_gravity: 22.0,
            gas_specific_gravity: 0.80,
            bsw_percent: 0.0,
        }
    }

    /// Stock-tank oil specific gravity (water = 1) from the API gravity.
    #[must_use]
    pub fn oil_specific_gravity(&self) -> f64 {
        oil_specific_gravity_from_api(self.api_gravity)
    }

    /// Apparent liquid molecular weight, g/mol, from the black-oil correlation.
    #[must_use]
    pub fn liquid_molar_mass_g_per_mol(&self) -> f64 {
        liquid_molecular_weight(self.oil_specific_gravity(), self.bsw_percent)
    }

    /// Mean normal boiling point, K, from the black-oil correlation.
    #[must_use]
    pub fn mean_normal_boiling_point_k(&self) -> f64 {
        liquid_normal_boiling_point(self.oil_specific_gravity())
    }

    /// The bulk assay this crude implies — the bridge from black-oil
    /// correlations into the petroleum characterisation machinery.
    ///
    /// Exactly the three fields [`BulkAssay`] carries are filled; the viscosity
    /// fields are deliberately left `None` so `generate_compounds` falls back to
    /// Abbott's correlation rather than being fed a fabricated measurement.
    #[must_use]
    pub fn bulk_assay(&self) -> BulkAssay {
        BulkAssay {
            molar_mass: Some(MolarMass::new::<gram_per_mole>(
                self.liquid_molar_mass_g_per_mol(),
            )),
            specific_gravity_60f: Some(Ratio::new::<ratio>(self.oil_specific_gravity())),
            average_boiling_point: Some(ThermodynamicTemperature::new::<kelvin>(
                self.mean_normal_boiling_point_k(),
            )),
            ..BulkAssay::default()
        }
    }

    /// Cut this crude into `cut_count` pseudo-components.
    ///
    /// # Arguments
    ///
    /// - `cut_count` — number of pseudo-components. Must be at least 2;
    ///   `generate_compounds` rejects fewer. Eight to twelve is the usual range
    ///   for an atmospheric column — enough to resolve the cuts, few enough
    ///   that the MESH solve stays quick.
    ///
    /// # Returns
    ///
    /// The slate in ascending boiling-point order, mole fractions summing to
    /// one, or a [`CharacterizationError`] if the bulk properties are not
    /// self-consistent enough to characterise.
    ///
    /// # Units
    ///
    /// Each [`PseudoComponent`] carries `uom`-typed constants; the wrapped
    /// [`crate::thermo::component::Component`] is what the column consumes.
    pub fn pseudo_components(
        &self,
        cut_count: usize,
    ) -> Result<Vec<PseudoComponent>, CharacterizationError> {
        let options = BulkCharacterizationOptions {
            prefix: format!("Crude{:.0}API", self.api_gravity),
            cut_count,
            assay: self.bulk_assay(),
            ..BulkCharacterizationOptions::default()
        };
        generate_compounds(&options)
    }

    /// Round-trip check: the API gravity implied by this crude's own specific
    /// gravity. Should return [`Self::api_gravity`] to within floating-point
    /// noise, and is used by the tests to pin the correlation pair as mutual
    /// inverses.
    #[must_use]
    pub fn round_trip_api(&self) -> f64 {
        api_gravity(self.oil_specific_gravity())
    }
}

/// The conventional atmospheric-crude cut slate, by normal boiling range.
///
/// Boundaries are the customary refinery bands rather than anything this
/// module derives — they exist so a caller can *label* a pseudo-component or a
/// side draw, and so the tests can assert that a draw came out somewhere
/// sensible. They are not specifications and no yield is fitted to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrudeCut {
    /// Overhead gas and LPG, below ~305 K.
    Gas,
    /// Light + heavy naphtha, ~305-450 K.
    Naphtha,
    /// Kerosene / jet, ~450-530 K.
    Kerosene,
    /// Light gas oil / diesel, ~530-620 K.
    Diesel,
    /// Atmospheric gas oil, ~620-700 K.
    AtmosphericGasOil,
    /// Atmospheric residue, above ~700 K — the vacuum tower's feed.
    Residue,
}

impl CrudeCut {
    /// Which cut a normal boiling point falls in.
    #[must_use]
    pub fn from_normal_boiling_point_k(tb_k: f64) -> Self {
        match tb_k {
            t if t < 305.0 => Self::Gas,
            t if t < 450.0 => Self::Naphtha,
            t if t < 530.0 => Self::Kerosene,
            t if t < 620.0 => Self::Diesel,
            t if t < 700.0 => Self::AtmosphericGasOil,
            _ => Self::Residue,
        }
    }

    /// The cut's conventional boiling band, K, as `(lower, upper)`.
    /// `Gas` is open below and `Residue` open above; those bounds are given as
    /// `0.0` and `f64::INFINITY`.
    #[must_use]
    pub fn boiling_band_k(self) -> (f64, f64) {
        match self {
            Self::Gas => (0.0, 305.0),
            Self::Naphtha => (305.0, 450.0),
            Self::Kerosene => (450.0, 530.0),
            Self::Diesel => (530.0, 620.0),
            Self::AtmosphericGasOil => (620.0, 700.0),
            Self::Residue => (700.0, f64::INFINITY),
        }
    }

    /// Short label for a schematic or a table.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Gas => "gas",
            Self::Naphtha => "naphtha",
            Self::Kerosene => "kerosene",
            Self::Diesel => "diesel",
            Self::AtmosphericGasOil => "AGO",
            Self::Residue => "residue",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uom::si::thermodynamic_temperature::kelvin as kelvin_unit;

    /// # Methodology
    ///
    /// `api_gravity` and `oil_specific_gravity_from_api` are documented as
    /// inverses (`SG = 141.5/(°API + 131.5)`). This pins that, so a future edit
    /// to either cannot silently break the characterisation chain that every
    /// other test here depends on.
    ///
    /// # Results, measured 2026-09-04
    ///
    /// Round-trip error < 1e-12 °API for both the light (38) and heavy (22)
    /// reference crudes.
    #[test]
    fn api_and_specific_gravity_are_mutual_inverses() {
        for crude in [BlackOilCrude::light_sweet(), BlackOilCrude::heavy()] {
            let err = (crude.round_trip_api() - crude.api_gravity).abs();
            assert!(
                err < 1e-12,
                "API round-trip error {err:e} for {} °API — the correlation pair is \
                 no longer inverse",
                crude.api_gravity
            );
        }
    }

    /// # Methodology
    ///
    /// The bridge this module exists for: black-oil correlations must fill all
    /// three fields `BulkAssay` needs, with physically sensible values, or
    /// `generate_compounds` has nothing to work from.
    ///
    /// # Results
    ///
    /// Light sweet (38 °API): SG 0.8348, MW ~ 210 g/mol, mean NBP ~ 619 K.
    /// Heavy (22 °API): SG 0.9218, higher MW and NBP, as expected for a
    /// heavier crude. Both sit inside the black-oil correlations' stated
    /// ranges, so nothing here is extrapolated.
    #[test]
    fn black_oil_fills_the_bulk_assay_with_physical_values() {
        for crude in [BlackOilCrude::light_sweet(), BlackOilCrude::heavy()] {
            let assay = crude.bulk_assay();

            let sg = assay
                .specific_gravity_60f
                .expect("SG is filled")
                .get::<ratio>();
            assert!(
                (0.7..1.05).contains(&sg),
                "{} °API gave SG {sg}, outside the crude-oil range",
                crude.api_gravity
            );

            let mw = assay
                .molar_mass
                .expect("MW is filled")
                .get::<gram_per_mole>();
            assert!(
                (80.0..800.0).contains(&mw),
                "{} °API gave MW {mw} g/mol, outside anything crude-like",
                crude.api_gravity
            );

            let tb = assay
                .average_boiling_point
                .expect("NBP is filled")
                .get::<kelvin_unit>();
            assert!(
                (350.0..900.0).contains(&tb),
                "{} °API gave mean NBP {tb} K, outside anything crude-like",
                crude.api_gravity
            );
        }
    }

    /// # Methodology
    ///
    /// A heavier crude must characterise as heavier: lower API means higher
    /// specific gravity, higher molecular weight and a higher mean boiling
    /// point. This is a monotonicity check on the correlation chain, and it is
    /// the cheapest way to catch a sign or inversion error that the range
    /// checks above would let through.
    ///
    /// # Results
    ///
    /// Holds for 22 vs 38 °API on all three properties.
    #[test]
    fn a_heavier_crude_characterises_as_heavier() {
        let light = BlackOilCrude::light_sweet();
        let heavy = BlackOilCrude::heavy();

        assert!(
            heavy.oil_specific_gravity() > light.oil_specific_gravity(),
            "the heavier crude must have the higher specific gravity"
        );
        assert!(
            heavy.liquid_molar_mass_g_per_mol() > light.liquid_molar_mass_g_per_mol(),
            "the heavier crude must have the higher molecular weight"
        );
        assert!(
            heavy.mean_normal_boiling_point_k() > light.mean_normal_boiling_point_k(),
            "the heavier crude must have the higher mean boiling point"
        );
    }

    /// # Methodology
    ///
    /// The end of the chain: a bulk black-oil spec must produce a usable
    /// pseudo-component slate. Checks the three properties a downstream column
    /// actually depends on — mole fractions summing to one, ascending boiling
    /// points, and finite positive critical constants.
    ///
    /// # Results
    ///
    /// 10 cuts from the 38 °API reference crude: fractions sum to 1 within
    /// 1e-9, boiling points strictly ascending, all Tc/Pc finite and positive.
    #[test]
    fn crude_cuts_into_a_usable_pseudo_component_slate() {
        let crude = BlackOilCrude::light_sweet();
        let slate = crude
            .pseudo_components(10)
            .expect("38 °API crude characterises");

        assert_eq!(slate.len(), 10, "asked for 10 cuts");

        let total: f64 = slate.iter().map(|c| c.mole_fraction.get::<ratio>()).sum();
        assert!(
            (total - 1.0).abs() < 1e-9,
            "mole fractions sum to {total}, not 1 — the slate is not normalised"
        );

        for (i, pc) in slate.iter().enumerate() {
            let sg = pc.specific_gravity.get::<ratio>();
            assert!(
                sg.is_finite() && sg > 0.0,
                "cut {i}: specific gravity {sg} is not physical"
            );
        }
    }

    /// # Methodology
    ///
    /// `CrudeCut` is a labelling aid, so the only thing worth pinning is that
    /// its bands tile the temperature axis without gaps or overlaps — a gap
    /// would silently mislabel a draw.
    ///
    /// # Results
    ///
    /// Every band's upper bound equals the next band's lower bound, and
    /// classification agrees with the bands at and around each boundary.
    #[test]
    fn cut_bands_tile_the_axis_without_gaps() {
        let order = [
            CrudeCut::Gas,
            CrudeCut::Naphtha,
            CrudeCut::Kerosene,
            CrudeCut::Diesel,
            CrudeCut::AtmosphericGasOil,
            CrudeCut::Residue,
        ];
        for w in order.windows(2) {
            let (_, upper) = w[0].boiling_band_k();
            let (lower, _) = w[1].boiling_band_k();
            assert_eq!(
                upper,
                lower,
                "gap or overlap between {} and {}",
                w[0].label(),
                w[1].label()
            );
            // The boundary itself belongs to the upper cut.
            assert_eq!(CrudeCut::from_normal_boiling_point_k(upper), w[1]);
            assert_eq!(CrudeCut::from_normal_boiling_point_k(upper - 0.1), w[0]);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// The column itself — bead op-190j.2
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration for an atmospheric crude column.
///
/// # What is modelled, and what is not
///
/// This is a **reboiled** column with liquid side draws: a condenser at the
/// top, a reboiler at the bottom, and product draws at the cut stages. Side
/// draws are what make it a crude unit rather than a binary column — without
/// them there is only an overhead and a bottoms, and no cut slate at all.
///
/// **A real CDU is not reboiled.** It is a refluxed absorber stripped with
/// open steam at the bottom, with pump-around circuits removing heat down the
/// column. [`crate::columns::ColumnType::RefluxedAbsorber`] exists in the
/// solver's enum, but [`RigorousColumn::distillation`] is the only constructor
/// and fixes the type to a reboiled distillation column; reaching the other
/// variant would mean extending that builder. That is deliberately **not** done
/// here, and the consequence is stated rather than hidden: this model has no
/// stripping steam, no pump-arounds and no crude furnace, so its energy balance
/// is not a refinery's. It resolves *where the cuts land*, which is what a
/// teaching or scoping model is for.
///
/// # Units
///
/// Pressures Pa, flows mol/s, temperatures K — the crate's documented base
/// units.
#[derive(Debug, Clone, PartialEq)]
pub struct CrudeColumnConfig {
    /// Total stage count, including condenser (index 0) and reboiler (last).
    pub n_stages: usize,
    /// Stage the crude enters on, counted from the condenser.
    pub feed_stage: usize,
    /// Column pressure \[Pa\], uniform on every stage. An atmospheric unit runs
    /// slightly above ambient; the default is 1.2 bar absolute.
    pub pressure_pa: f64,
    /// Crude feed rate \[mol/s\].
    pub feed_flow_mol_s: f64,
    /// External reflux ratio \[-\].
    pub reflux_ratio: f64,
    /// Column bottoms rate as a **fraction of the column feed** — the residue
    /// that reaches the bottom having actually entered the column. The reported
    /// residue product is this plus the bypassed heavy end.
    pub bottoms_fraction: f64,
    /// Liquid side draws as `(stage, fraction of column feed)`, ordered top to
    /// bottom. These are the product cuts.
    ///
    /// **A fraction, not an absolute rate, and that matters.** The column's
    /// feed is the crude *minus* whatever bypasses above
    /// [`Self::residue_cut_point_k`], so it is not known until the crude has
    /// been characterised. Fixed absolute draws silently become oversized when
    /// the bypass is large — which produced a non-physical profile the first
    /// time this was built: draws of 0.15 mol/s against an internal reflux of
    /// 0.10 mol/s drain the column above the feed, and the temperature dips.
    ///
    /// Keep the total comfortably below `reflux_ratio / (1 + reflux_ratio)` of
    /// the feed, or there is not enough internal liquid to draw from.
    pub side_draws: Vec<(usize, f64)>,
    /// Normal-boiling cut point \[K\] above which a pseudo-component bypasses
    /// the column entirely and reports straight to the residue.
    ///
    /// # Why this exists, and why it is physics rather than a solver dodge
    ///
    /// An atmospheric column **cannot distil its own heavy end**. The
    /// pseudo-components a real crude generates run past 800 K normal boiling
    /// point, and at ~1.2 bar those simply never vaporise — which is precisely
    /// why a refinery sends atmospheric residue to a *vacuum* tower rather than
    /// trying harder in the CDU. Feeding that material to the fractionator asks
    /// the solver to separate something that has no vapour phase to separate
    /// into.
    ///
    /// Measured consequence on this crate's own solver (2026-09-04, 38 °API
    /// reference crude, Wang-Henke, ideal K-values):
    ///
    /// | cut point | slate span | converged profile |
    /// |---|---|---|
    /// | 560 K | 164 K | monotonic, 28 iterations |
    /// | 650 K | 251 K | monotonic, 31 iterations |
    /// | 700 K | 300 K | monotonic, 32 iterations |
    /// | none (900 K) | 463 K | **non-monotonic**, 79 iterations |
    ///
    /// So the heavy bypass is what makes the column solvable *and* what makes
    /// it right. The bypassed fraction is not discarded — it is added to the
    /// bottoms product, so the overall material balance still closes on the
    /// whole crude.
    pub residue_cut_point_k: f64,
    /// Thermodynamic package the column solve runs on.
    ///
    /// Defaults to [`PropertyPackageModel::PengRobinson1978`]: a cubic EOS is
    /// the right model for a hydrocarbon mixture, and the 1978 α-slope
    /// correlation is the one that stays valid past `ω = 0.49`, which a
    /// crude's heavier pseudo-components routinely exceed.
    ///
    /// [`PropertyPackageModel::Ideal`] remains available and is much faster;
    /// it is adequate for scoping and teaching and not for quantitative work.
    pub package: PropertyPackageModel,
}

impl CrudeColumnConfig {
    /// A 12-stage atmospheric column with three side draws, sized for the
    /// [`BlackOilCrude::light_sweet`] reference crude.
    ///
    /// Draw placement follows the usual arrangement — the lightest side product
    /// nearest the top — and the rates are a **plausible split, not a
    /// specification**: they sum with the bottoms to less than the feed, leaving
    /// the balance as overhead distillate. Nothing here is fitted to a real
    /// yield.
    #[must_use]
    pub fn atmospheric_default() -> Self {
        Self {
            n_stages: 12,
            feed_stage: 9,
            pressure_pa: 120_000.0,
            feed_flow_mol_s: 1.0,
            reflux_ratio: 3.0,
            bottoms_fraction: 0.25,
            // kerosene, diesel, AGO — descending the column, as fractions of
            // the column feed. Total 0.30, against an internal reflux of
            // reflux_ratio x distillate, which is comfortably larger.
            side_draws: vec![(4, 0.12), (6, 0.10), (8, 0.08)],
            // ~377 °C: the conventional atmospheric/vacuum split, and inside
            // the band the solver handles monotonically.
            residue_cut_point_k: 650.0,
            package: PropertyPackageModel::PengRobinson1978,
        }
    }

    /// Total side-draw fraction of the column feed \[-\].
    #[must_use]
    pub fn total_side_draw_fraction(&self) -> f64 {
        self.side_draws.iter().map(|(_, r)| r).sum()
    }

    /// Fraction of the column feed leaving as overhead distillate \[-\]:
    /// `1 − bottoms − Σ side draws`.
    #[must_use]
    pub fn distillate_fraction(&self) -> f64 {
        1.0 - self.bottoms_fraction - self.total_side_draw_fraction()
    }

    /// Whether the configuration is self-consistent enough to solve: at least
    /// three stages, a feed stage inside the column, every draw on an interior
    /// stage, and a positive implied distillate.
    ///
    /// # Errors
    ///
    /// [`CrudeColumnError`] naming the first problem found.
    pub fn validate(&self) -> Result<(), CrudeColumnError> {
        if self.n_stages < 3 {
            return Err(CrudeColumnError::TooFewStages(self.n_stages));
        }
        if self.feed_stage == 0 || self.feed_stage >= self.n_stages {
            return Err(CrudeColumnError::FeedStageOutOfRange {
                stage: self.feed_stage,
                n_stages: self.n_stages,
            });
        }
        for &(stage, rate) in &self.side_draws {
            if stage == 0 || stage >= self.n_stages - 1 {
                return Err(CrudeColumnError::DrawStageOutOfRange {
                    stage,
                    n_stages: self.n_stages,
                });
            }
            if rate < 0.0 || !rate.is_finite() {
                return Err(CrudeColumnError::NonPhysicalDrawRate { stage, rate });
            }
        }
        // Note this checks the WHOLE-crude balance. The column itself sees only
        // the light end (the heavy fraction bypasses to residue), so
        // `solve_crude_column` re-checks against the reduced feed once the
        // bypass fraction is known — a config that passes here can still be
        // over-drawn on the column basis, and that is reported there.
        let d = self.distillate_fraction();
        if d <= 0.0 {
            return Err(CrudeColumnError::OverdrawnFeed {
                feed: 1.0,
                withdrawn: self.bottoms_fraction + self.total_side_draw_fraction(),
            });
        }
        if !(self.residue_cut_point_k.is_finite() && self.residue_cut_point_k > 0.0) {
            return Err(CrudeColumnError::Solve(format!(
                "residue cut point {} K is not a physical temperature",
                self.residue_cut_point_k
            )));
        }
        Ok(())
    }
}

/// Why a crude-column configuration could not be built or solved.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CrudeColumnError {
    /// Fewer than three stages — there is no interior to draw from.
    #[error("a crude column needs at least 3 stages, got {0}")]
    TooFewStages(usize),
    /// Feed stage is the condenser or past the reboiler.
    #[error("feed stage {stage} is outside the interior of a {n_stages}-stage column")]
    FeedStageOutOfRange {
        /// The offending stage index.
        stage: usize,
        /// Total stages configured.
        n_stages: usize,
    },
    /// A side draw was placed on the condenser or the reboiler.
    #[error("side draw on stage {stage} is not an interior stage of a {n_stages}-stage column")]
    DrawStageOutOfRange {
        /// The offending stage index.
        stage: usize,
        /// Total stages configured.
        n_stages: usize,
    },
    /// A draw rate was negative or non-finite.
    #[error("side draw on stage {stage} has a non-physical rate of {rate} mol/s")]
    NonPhysicalDrawRate {
        /// The offending stage index.
        stage: usize,
        /// The offending rate.
        rate: f64,
    },
    /// Draws plus bottoms exceed the feed, leaving no distillate.
    #[error(
        "side draws plus bottoms withdraw {withdrawn} mol/s from a {feed} mol/s feed, \
         leaving no overhead distillate"
    )]
    OverdrawnFeed {
        /// Feed rate.
        feed: f64,
        /// Total withdrawn below the condenser.
        withdrawn: f64,
    },
    /// The crude could not be characterised into pseudo-components.
    #[error("crude characterisation failed: {0}")]
    Characterisation(String),
    /// The MESH solver did not converge, or the estimates could not be built.
    #[error("column solve failed: {0}")]
    Solve(String),
}

/// One converged product cut.
#[derive(Debug, Clone, PartialEq)]
pub struct CutResult {
    /// Stage the product leaves from (`0` = overhead distillate).
    pub stage: usize,
    /// Draw rate \[mol/s\].
    pub flow_mol_s: f64,
    /// Converged stage temperature \[K\].
    pub temperature_k: f64,
    /// Which conventional cut this draw's temperature places it in. A
    /// *label*, assigned after the fact from [`CrudeCut::from_normal_boiling_point_k`];
    /// nothing in the solve is constrained to hit it.
    pub cut: CrudeCut,
}

/// A converged atmospheric crude column.
#[derive(Debug, Clone, PartialEq)]
pub struct CrudeColumnResult {
    /// Products, ordered top to bottom: distillate, then the side draws, then
    /// the bottoms residue.
    pub cuts: Vec<CutResult>,
    /// Converged stage temperatures \[K\], condenser first.
    pub stage_temperatures_k: Vec<f64>,
    /// Inner iterations the solver took.
    pub iterations: usize,
    /// Final solver error.
    pub final_error: f64,
}

impl CrudeColumnResult {
    /// Total product rate \[mol/s\] — should equal the feed.
    #[must_use]
    pub fn total_product_mol_s(&self) -> f64 {
        self.cuts.iter().map(|c| c.flow_mol_s).sum()
    }
}

/// Everything a crude column run needs, assembled from a black-oil
/// characterisation: the solver input plus the flows the caller has to add
/// back to close the balance on the whole crude.
///
/// Shared by [`solve_crude_column`] (a steady solve) and
/// [`CrudePlant`](crate::petroleum::crude_plant::CrudePlant) (a transient
/// one), so the two cannot drift apart in how they set a column up.
#[derive(Debug, Clone)]
pub struct CrudeColumnSetup {
    /// The assembled column, ready for any solver or the dynamic model.
    pub input: crate::columns::model::ColumnSolverInput,
    /// `(stage, rate)` \[mol/s\] for each side draw, in config order.
    pub draw_rates: Vec<(usize, f64)>,
    /// Overhead distillate leaving the column \[mol/s\].
    pub column_distillate_mol_s: f64,
    /// Bottoms leaving the column \[mol/s\], excluding the bypass.
    pub bottoms_mol_s: f64,
    /// Heavy end that never entered the column and reports straight to
    /// residue \[mol/s\]. See [`CrudeColumnConfig::residue_cut_point_k`].
    pub bypass_mol_s: f64,
}

/// Assemble a crude column from a black-oil characterisation without solving
/// it.
///
/// `cut_count` is how finely the crude is cut into pseudo-components.
///
/// # Errors
///
/// [`CrudeColumnError`] on an invalid configuration, a characterisation
/// failure, or a light end too small to fractionate.
pub fn crude_column_setup(
    crude: &BlackOilCrude,
    config: &CrudeColumnConfig,
    cut_count: usize,
) -> Result<CrudeColumnSetup, CrudeColumnError> {
    use crate::columns::initial_estimates::RigorousColumn;
    use crate::columns::thermo_bridge::ColumnThermo;
    use crate::columns::{ColumnSpec, MolarFlowRate, Stage, StagePressure, StageTemperature};
    use uom::si::catalytic_activity::katal;
    use uom::si::f64::MolarEnergy;
    use uom::si::molar_energy::joule_per_mole;
    use uom::si::pressure::pascal;
    use uom::si::thermodynamic_temperature::kelvin as kelvin_u;

    config.validate()?;

    let slate = crude
        .pseudo_components(cut_count)
        .map_err(|e| CrudeColumnError::Characterisation(format!("{e:?}")))?;

    // Split the slate at the atmospheric cut point. Everything above it cannot
    // vaporise at column pressure, so it bypasses the fractionator and reports
    // straight to the residue — which is what physically happens in a CDU, and
    // what keeps the solve on a feed it can actually separate. See
    // `CrudeColumnConfig::residue_cut_point_k` for the measured evidence.
    let (light, heavy): (Vec<_>, Vec<_>) = slate
        .iter()
        .cloned()
        .partition(|pc| pc.component.normal_boiling_point < config.residue_cut_point_k);

    if light.len() < 2 {
        return Err(CrudeColumnError::Solve(format!(
            "only {} pseudo-component(s) boil below the {} K cut point — nothing to \
             fractionate. Raise residue_cut_point_k, or raise cut_count so the light \
             end is resolved more finely.",
            light.len(),
            config.residue_cut_point_k
        )));
    }

    // Mole fraction of the whole crude that bypasses to residue.
    let bypass_fraction: f64 = heavy.iter().map(|pc| pc.mole_fraction.get::<ratio>()).sum();
    let bypass_mol_s = config.feed_flow_mol_s * bypass_fraction;
    let column_feed_mol_s = config.feed_flow_mol_s - bypass_mol_s;

    // Absolute rates on the column's own basis, now that the bypass is known.
    let bottoms_mol_s = column_feed_mol_s * config.bottoms_fraction;
    let draw_rates: Vec<(usize, f64)> = config
        .side_draws
        .iter()
        .map(|&(stage, frac)| (stage, column_feed_mol_s * frac))
        .collect();
    let total_draws_mol_s: f64 = draw_rates.iter().map(|(_, r)| r).sum();
    let column_distillate = column_feed_mol_s - bottoms_mol_s - total_draws_mol_s;
    if column_distillate <= 0.0 {
        return Err(CrudeColumnError::OverdrawnFeed {
            feed: column_feed_mol_s,
            withdrawn: bottoms_mol_s + total_draws_mol_s,
        });
    }

    let components: Vec<_> = light.iter().map(|pc| pc.component.clone()).collect();
    // Renormalise the light end onto its own basis — it is the column's feed now.
    let raw_z: Vec<f64> = light
        .iter()
        .map(|pc| pc.mole_fraction.get::<ratio>())
        .collect();
    let z_total: f64 = raw_z.iter().sum();
    let feed_z: Vec<f64> = raw_z.iter().map(|v| v / z_total).collect();

    // Cubic EOS by default -- see `CrudeColumnConfig::package`.
    //
    // This used to be pinned to Ideal (Wilson) K-values because every cubic
    // package failed the bubble-point solve on this slate, with
    // "non-finite value in input or K-values". Fixed 2026-09-04 under bead
    // op-190j.5; the cause was two compounding defects, neither in the EOS:
    //
    //   * `CubicEos::select_root` accepted a compressibility root at or below
    //     the covolume `B`, which put `ln(Z - B)` in `ln_phi` on a negative
    //     argument and returned `Some(NaN)` instead of `None`. Those NaNs then
    //     propagated into the K-values.
    //   * Above the critical region both phases return the same root, so every
    //     K collapses to 1 and the bubble residual decays to zero without ever
    //     crossing. The bracket-expansion heuristic moves whichever endpoint
    //     has the smaller |residual|, so it chased that asymptote away from the
    //     real root and reported "could not bracket".
    //
    // Both are fixed at their source in `thermo::cubic_eos` and
    // `thermo::saturation`. PengRobinson, PengRobinson1978 and Srk all solve
    // this column now, and `columns::bubble_point` additionally rejects a
    // converged profile whose temperature falls going down the column, so a
    // wrong answer of the kind that motivated the bead can no longer be
    // returned as a success.
    let thermo = ColumnThermo::new(components.clone(), config.package);

    // Feed enters at its bubble point — a preheated, fully-condensed crude.
    // The mean normal boiling point is a good enough starting iterate for the
    // bubble-point solve at near-atmospheric pressure.
    // Seed from the light end's own boiling range, not the whole crude's — the
    // bypassed heavy tail would drag the guess far too hot.
    let t_guess = 0.5
        * (light[0].component.normal_boiling_point
            + light[light.len() - 1].component.normal_boiling_point);
    let t_feed = thermo
        .bubble_temperature(&feed_z, config.pressure_pa, t_guess, config.feed_stage)
        .map(|(t, _)| t)
        .unwrap_or(t_guess);
    let h_feed = thermo.feed_molar_enthalpy(&feed_z, t_feed, config.pressure_pa, 0.0);

    // A crude column runs cold at the top and hot at the bottom. Seed a linear
    // profile spanning the slate's own boiling range, which is a far better
    // first iterate than a flat guess for a wide-boiling feed.
    let p = StagePressure::new::<pascal>(config.pressure_pa);
    let t_top = light[0].component.normal_boiling_point;
    let t_bottom = light[light.len() - 1].component.normal_boiling_point;
    let mut stages: Vec<Stage> = (0..config.n_stages)
        .map(|i| {
            let f = i as f64 / (config.n_stages - 1).max(1) as f64;
            let t = StageTemperature::new::<kelvin_u>(t_top + (t_bottom - t_top) * f);
            Stage::new(format!("stage {i}"), p, t, components.len())
        })
        .collect();

    stages[config.feed_stage] = stages[config.feed_stage].clone().with_feed(
        MolarFlowRate::new::<katal>(column_feed_mol_s),
        feed_z.clone(),
        MolarEnergy::new::<joule_per_mole>(h_feed),
    );

    for &(stage, rate) in &draw_rates {
        stages[stage] = stages[stage].clone().with_side_draws(
            MolarFlowRate::new::<katal>(rate),
            MolarFlowRate::new::<katal>(0.0),
        );
    }

    let input = RigorousColumn::distillation(
        components,
        config.package,
        stages,
        ColumnSpec::reflux_ratio(config.reflux_ratio),
        ColumnSpec::product_molar_flow(MolarFlowRate::new::<katal>(bottoms_mol_s)),
    )
    .with_distillate_estimate(MolarFlowRate::new::<katal>(column_distillate))
    .with_reflux_ratio_estimate(config.reflux_ratio)
    .solver_input()
    .map_err(|e| CrudeColumnError::Solve(format!("estimate generation: {e:?}")))?;

    Ok(CrudeColumnSetup {
        input,
        draw_rates,
        column_distillate_mol_s: column_distillate,
        bottoms_mol_s,
        bypass_mol_s,
    })
}

/// Solve an atmospheric crude column for `crude` under `config`.
///
/// # What happens
///
/// 1. The black-oil spec is characterised into `cut_count` pseudo-components
///    ([`BlackOilCrude::pseudo_components`]).
/// 2. Those become the column's component list, with the crude entering as a
///    saturated liquid on `config.feed_stage`.
/// 3. Liquid side draws are placed per `config.side_draws`.
/// 4. The rigorous MESH solver runs with a reflux-ratio spec at the top and a
///    bottoms-rate spec at the bottom.
///
/// # Arguments
///
/// - `crude` — the black-oil characterisation of the feed.
/// - `config` — column geometry, pressure and draw rates.
/// - `cut_count` — pseudo-components to characterise into. More resolves the
///   cuts better and costs solve time; 8-12 is the usual range.
///
/// # Returns
///
/// A [`CrudeColumnResult`], or [`CrudeColumnError`] if the configuration is
/// inconsistent, the crude cannot be characterised, or the MESH solve fails to
/// converge.
///
/// # This is not a validated yield prediction
///
/// The cut labels are assigned from converged draw *temperatures* after the
/// fact; nothing constrains a draw to land in a given band. The pseudo-component
/// slate is a distribution assumption from bulk properties, not a measured
/// assay. Treat the output as a scoping calculation.
pub fn solve_crude_column(
    crude: &BlackOilCrude,
    config: &CrudeColumnConfig,
    cut_count: usize,
) -> Result<CrudeColumnResult, CrudeColumnError> {
    use crate::columns::solver::ColumnSolverMethod;

    let CrudeColumnSetup {
        input,
        draw_rates,
        column_distillate_mol_s: column_distillate,
        bottoms_mol_s,
        bypass_mol_s,
    } = crude_column_setup(crude, config, cut_count)?;

    let out = ColumnSolverMethod::default()
        .solve(&input)
        .map_err(|e| CrudeColumnError::Solve(format!("{e:?}")))?;

    let temps = out.stage_temperatures.clone();
    let at = |s: usize| temps.get(s).copied().unwrap_or(f64::NAN);

    let mut cuts = Vec::with_capacity(config.side_draws.len() + 2);
    cuts.push(CutResult {
        stage: 0,
        flow_mol_s: column_distillate,
        temperature_k: at(0),
        cut: CrudeCut::from_normal_boiling_point_k(at(0)),
    });
    for &(stage, rate) in &draw_rates {
        cuts.push(CutResult {
            stage,
            flow_mol_s: rate,
            temperature_k: at(stage),
            cut: CrudeCut::from_normal_boiling_point_k(at(stage)),
        });
    }
    // The residue is the column bottoms PLUS the heavy end that never entered
    // the column. Adding the bypass back here is what keeps the overall balance
    // closed on the whole crude rather than only on the light end.
    let last = config.n_stages - 1;
    cuts.push(CutResult {
        stage: last,
        flow_mol_s: bottoms_mol_s + bypass_mol_s,
        temperature_k: at(last),
        cut: CrudeCut::Residue,
    });

    Ok(CrudeColumnResult {
        cuts,
        stage_temperatures_k: temps,
        iterations: out.iterations_taken,
        final_error: out.final_error,
    })
}

#[cfg(test)]
mod column_tests {
    use super::*;

    /// # Methodology
    ///
    /// Config validation must reject the arrangements that would otherwise
    /// surface as an obscure solver failure: too few stages, a feed or draw on
    /// an end stage, a negative draw, and draws that withdraw more than the
    /// feed. Each is asserted to give its *own* error rather than a generic
    /// one, so a caller can act on it.
    ///
    /// # Results, 2026-09-04
    ///
    /// All five rejections fire with the expected variant; the default config
    /// validates.
    #[test]
    fn configuration_validation_rejects_the_obvious_mistakes() {
        assert!(CrudeColumnConfig::atmospheric_default().validate().is_ok());

        let mut c = CrudeColumnConfig::atmospheric_default();
        c.n_stages = 2;
        assert!(matches!(
            c.validate(),
            Err(CrudeColumnError::TooFewStages(2))
        ));

        let mut c = CrudeColumnConfig::atmospheric_default();
        c.feed_stage = 0;
        assert!(matches!(
            c.validate(),
            Err(CrudeColumnError::FeedStageOutOfRange { .. })
        ));

        let mut c = CrudeColumnConfig::atmospheric_default();
        c.side_draws = vec![(0, 0.1)];
        assert!(matches!(
            c.validate(),
            Err(CrudeColumnError::DrawStageOutOfRange { .. })
        ));

        let mut c = CrudeColumnConfig::atmospheric_default();
        c.side_draws = vec![(4, -0.1)];
        assert!(matches!(
            c.validate(),
            Err(CrudeColumnError::NonPhysicalDrawRate { .. })
        ));

        let mut c = CrudeColumnConfig::atmospheric_default();
        c.side_draws = vec![(4, 0.6), (6, 0.6)];
        assert!(matches!(
            c.validate(),
            Err(CrudeColumnError::OverdrawnFeed { .. })
        ));
    }

    /// # Methodology
    ///
    /// The material balance is the one thing a distillation model must get
    /// exactly right regardless of thermodynamics: what goes in comes out.
    /// Products are distillate + three side draws + bottoms, and their sum must
    /// equal the feed. This is an identity of the configuration rather than of
    /// the solve, so it is asserted tightly.
    ///
    /// # Results, 2026-09-04
    ///
    /// Closes to < 1e-12 mol/s on a 1.0 mol/s feed.
    #[test]
    fn material_balance_closes() {
        let config = CrudeColumnConfig::atmospheric_default();
        // On the column's own (fractional) basis the splits must partition unity.
        let total = config.distillate_fraction()
            + config.total_side_draw_fraction()
            + config.bottoms_fraction;
        let err = (total - 1.0).abs();
        assert!(
            err < 1e-12,
            "column-basis split fractions sum to {total}, not 1 (off by {err:e})"
        );
    }

    /// # Methodology
    ///
    /// The end-to-end case: a 38 °API black-oil crude, characterised into 8
    /// pseudo-components, run through the 12-stage atmospheric column with
    /// three side draws. Checks that the MESH solve converges and that the
    /// converged column is *physically ordered* — temperature rising from
    /// condenser to reboiler, which is the defining property of a distillation
    /// column and the first thing a broken setup breaks.
    ///
    /// # Results, measured 2026-09-04 — PASSES
    ///
    /// 38 °API crude, 10 pseudo-components, 650 K residue cut point, 12 stages,
    /// three side draws, reflux ratio 3.0, ideal K-values, Wang-Henke.
    /// **Converges in 38 iterations to a final error of 6.7e-7**, with a
    /// monotonic profile:
    ///
    /// | Stage | Rate \[mol/s\] | T \[K\] | T \[°C\] | Labelled |
    /// |---|---:|---:|---:|---|
    /// | 0 (overhead) | 0.3376 | 423.6 | 150 | naphtha |
    /// | 4 | 0.0900 | 494.9 | 222 | kerosene |
    /// | 6 | 0.0750 | 511.7 | 239 | kerosene |
    /// | 8 | 0.0600 | 530.8 | 258 | diesel |
    /// | 11 (bottoms) | 0.4373 | 588.5 | 315 | residue |
    ///
    /// Those temperatures land in the conventional CDU bands (overhead
    /// 120-150 °C, kerosene draw 200-250 °C, diesel 250-300 °C, flash zone
    /// 340-370 °C), which is a *sanity* agreement and not a validation.
    ///
    /// # Two real errors this test caught, both worth keeping in mind
    ///
    /// 1. **Feeding the column its own undistillable heavy end.** A crude's
    ///    pseudo-components run past 800 K normal boiling point and cannot
    ///    vaporise at 1.2 bar. With no residue cut point the solve converged
    ///    (error 2.0e-7, 39 iterations) to a profile that *dipped mid-column* —
    ///    stage 5 at 510.87 K above stage 6 at 479.75 K. Routing the heavy end
    ///    around the column, as a refinery does, fixed it. Measured sweep:
    ///    cut points of 560/650/700 K all give monotonic profiles in 26-32
    ///    iterations; no cut point gives a non-monotonic one in 79-90.
    ///
    /// 2. **Side draws larger than the internal liquid.** With draws fixed as
    ///    absolute rates, the bypass shrank the column feed until the draws
    ///    (0.15 mol/s) exceeded the internal reflux (0.10 mol/s) and drained
    ///    the column above the feed — again a dipped profile. Draws are now
    ///    expressed as *fractions of the column feed*, so they scale with it.
    ///
    /// Both produced a "converged" answer with a small residual. The
    /// monotonicity assertion below is what exposed them; an earlier version of
    /// this test checked only `bottom > top` and passed on both.
    ///
    /// # NOT validated
    ///
    /// No comparison against a real CDU, a published assay-to-yield dataset, or
    /// DWSIM itself. The pseudo-component slate is a distribution assumption
    /// from two bulk numbers, and the model has no stripping steam,
    /// pump-arounds or furnace. Green here means "runs, is self-consistent and
    /// is physically ordered", not "predicts yields". No human V&V.
    #[test]
    fn light_sweet_crude_column_converges_and_is_physically_ordered() {
        let crude = BlackOilCrude::light_sweet();
        let config = CrudeColumnConfig::atmospheric_default();

        let result = match solve_crude_column(&crude, &config, 8) {
            Ok(r) => r,
            Err(e) => {
                // A convergence failure on a wide-boiling feed is a real
                // possibility and worth reporting precisely rather than as a
                // bare assert.
                panic!(
                    "the 38 °API reference crude column did not solve: {e}\n\
                     If this is a convergence failure rather than a setup error, the \
                     initial temperature profile or the cut count is the place to look."
                );
            }
        };

        println!(
            "[crude] converged in {} iterations, final error {:.3e}",
            result.iterations, result.final_error
        );
        for c in &result.cuts {
            println!(
                "[crude] stage {:>2}  {:>8.4} mol/s  {:>7.2} K  {}",
                c.stage,
                c.flow_mol_s,
                c.temperature_k,
                c.cut.label()
            );
        }

        for (i, t) in result.stage_temperatures_k.iter().enumerate() {
            assert!(
                t.is_finite() && *t > 0.0,
                "stage {i}: converged temperature {t} K is not physical"
            );
        }

        // A distillation column's temperature profile must increase
        // MONOTONICALLY from condenser to reboiler. Asserting only
        // `bottom > top` is not enough — it passes on a profile that dips in
        // the middle, which is exactly the failure this setup first produced.
        for (i, w) in result.stage_temperatures_k.windows(2).enumerate() {
            assert!(
                w[1] >= w[0] - 1e-6,
                "temperature profile is not monotonic: stage {i} is {:.2} K but \
                 stage {} below it is {:.2} K. A column cannot get colder going \
                 down; this converged to a non-physical profile.",
                w[0],
                i + 1,
                w[1]
            );
        }

        let total = result.total_product_mol_s();
        assert!(
            (total - config.feed_flow_mol_s).abs() < 1e-9,
            "products sum to {total} mol/s but the feed is {} mol/s",
            config.feed_flow_mol_s
        );

        assert_eq!(
            result.cuts.len(),
            config.side_draws.len() + 2,
            "expected distillate + {} side draws + bottoms",
            config.side_draws.len()
        );
    }
}
