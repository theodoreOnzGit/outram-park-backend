// SPDX-License-Identifier: GPL-3.0-only
//! **Divergence from upstream**: ingestion dose computed one nuclide at a
//! time, fixing pyDOSEIA defects D8-D11 (see `docs/pydoseia-code-to-code.md`).
//! The equations are upstream's ([`super::food_chain`]); only the bookkeeping
//! and the two clear bugs change:
//!
//! - **D8** each nuclide uses its own transfer factors and its own
//!   concentrations, whatever its position in the list;
//! - **D9** every nuclide gets a row, in input order, for any mix of H-3,
//!   C-14 and other nuclides;
//! - **D10** for a long-term release the C-14 air concentration is divided by
//!   the seconds in a year (`365 * 24 * 3600`), as upstream already does for
//!   H-3, so that `C_air` is Bq/m^3 and not Bq s/(y m^3);
//! - **D11** C-14 milk and meat are added according to **each** product,
//!   not according to whether the product *list* contains any milk or meat;
//!   and a tritium product list starting with meat no longer raises.
//!
//! Everything else (constants, units, the per-day diet times 365, the H-3
//! division by a year for a single plume) is upstream's. Not checked against
//! SRS 19 or TECDOC-1616 beyond what the code comments cite.

use super::food_chain::{
    animal_products, c14_in_animal, c14_in_plant, deposition_rate_per_day, effective_removal_rates,
    food_crop, per_day, tritium_in_animal, tritium_in_plant, AnimalProduct, Climate,
    VegetationType, C14_S_AIR, TRITIUM_CR_S, TRITIUM_GAMMA, TRITIUM_R_P,
};
use super::upstream::IngestionReleaseMode;
use super::{DietaryIntake, IngestionDcf, IngestionParameters, SoilType, TransferFactors};
use crate::pydoseia::units::{DilutionFactor, EffectiveDose};

/// Which model a nuclide gets. The caller decides; upstream decides by the
/// exact names `"H-3"` and `"C-14"` in one place and by the element symbols
/// `H` and `C` in another (part of D8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IngestionModel {
    /// The SRS 19 deposition model with the element's transfer factors
    /// ([`TransferFactors::ZERO`] if it has none, as upstream).
    Deposition {
        /// Total deposition velocity, m/s.
        deposition_velocity_m_per_s: f64,
        /// Element transfer factors.
        transfer: TransferFactors,
    },
    /// The TECDOC-1616 specific-activity model for tritium.
    Tritium,
    /// The specific-activity model for C-14.
    Carbon14,
}

/// One nuclide's inputs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IngestionNuclide {
    /// Model.
    pub model: IngestionModel,
    /// Decay constant, 1/s.
    pub decay_constant_per_s: f64,
    /// Release: Bq/y (long term) or Bq (single plume).
    pub release_bq: f64,
    /// Ingestion coefficient ([`IngestionDcf::Tritium`] for H-3).
    pub dcf: IngestionDcf,
}

/// Settings shared by all nuclides.
#[derive(Debug, Clone, PartialEq)]
pub struct IngestionSettings {
    /// Release mode.
    pub mode: IngestionReleaseMode,
    /// Food-chain parameters.
    pub parameters: IngestionParameters,
    /// Receiver's diet (per day).
    pub diet: DietaryIntake,
    /// Soil type.
    pub soil: SoilType,
    /// Climate (H-3).
    pub climate: Climate,
    /// Vegetables eaten (H-3 and C-14).
    pub veg_type: VegetationType,
    /// Animal feed (H-3 and C-14).
    pub animal_feed_type: VegetationType,
    /// Animal products for H-3.
    pub animal_products_h3: Vec<AnimalProduct>,
    /// Animal products for C-14.
    pub animal_products_c14: Vec<AnimalProduct>,
}

/// Ingestion dose by route.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct IngestionDoseByRoute {
    /// Vegetables / food crops.
    pub veg: EffectiveDose,
    /// Milk.
    pub milk: EffectiveDose,
    /// Meat.
    pub meat: EffectiveDose,
}

impl IngestionDoseByRoute {
    /// Sum of the three routes.
    #[must_use]
    pub fn total(self) -> EffectiveDose {
        EffectiveDose::from_millisieverts(
            self.veg.millisieverts() + self.milk.millisieverts() + self.meat.millisieverts(),
        )
    }
}

fn msv(v: f64) -> EffectiveDose {
    EffectiveDose::from_millisieverts(v)
}

/// Ingestion dose for each nuclide, in input order (the corrected driver; see
/// the module docs for what differs from upstream).
#[must_use]
pub fn ingestion_dose_per_nuclide(
    nuclides: &[IngestionNuclide],
    chi_over_q: DilutionFactor,
    s: &IngestionSettings,
) -> Vec<IngestionDoseByRoute> {
    let chi = chi_over_q.seconds_per_cubic_meter();
    let mult = match s.mode {
        IngestionReleaseMode::LongTerm => 365.0,
        IngestionReleaseMode::SinglePlume => 1.0,
    };
    let (did_veg, did_milk, did_meat) = (s.diet.veg * mult, s.diet.milk * mult, s.diet.meat * mult);
    let (rho_pasture, rho_crop) = s.soil.surface_densities();
    nuclides
        .iter()
        .map(|n| {
            let lambda_i = per_day(n.decay_constant_per_s);
            match (n.model, n.dcf) {
                (
                    IngestionModel::Deposition {
                        deposition_velocity_m_per_s,
                        transfer,
                    },
                    IngestionDcf::Single(d),
                ) => {
                    let day = match s.mode {
                        IngestionReleaseMode::LongTerm => n.release_bq / 365.0,
                        IngestionReleaseMode::SinglePlume => n.release_bq,
                    };
                    let dep = deposition_rate_per_day(day, deposition_velocity_m_per_s, chi);
                    let (l_eiv, l_eis) = effective_removal_rates(transfer, lambda_i);
                    let crop = food_crop(
                        dep,
                        l_eiv,
                        l_eis,
                        transfer.fv2,
                        lambda_i,
                        rho_crop,
                        &s.parameters,
                    );
                    let a = animal_products(
                        dep,
                        l_eiv,
                        l_eis,
                        transfer.fv1,
                        transfer.fm_milk_d_per_l,
                        transfer.ff_meat_d_per_kg,
                        lambda_i,
                        rho_pasture,
                        &s.parameters,
                    );
                    IngestionDoseByRoute {
                        veg: msv(crop.cvi * d * did_veg * 1000.0),
                        milk: msv(a.c_mi * d * did_milk * 1000.0),
                        meat: msv(a.c_fi * d * did_meat * 1000.0),
                    }
                }
                (IngestionModel::Tritium, IngestionDcf::Tritium { hto, obt }) => {
                    let plant = tritium_in_plant(
                        chi,
                        n.release_bq,
                        s.climate,
                        s.veg_type,
                        TRITIUM_CR_S,
                        TRITIUM_GAMMA,
                        TRITIUM_R_P,
                    );
                    let veg = plant.c_pfw_hto * hto * did_veg * 1000.0
                        + plant.c_pfw_obt * obt * did_veg * 1000.0;
                    let feed = tritium_in_plant(
                        chi,
                        n.release_bq,
                        s.climate,
                        s.animal_feed_type,
                        TRITIUM_CR_S,
                        TRITIUM_GAMMA,
                        TRITIUM_R_P,
                    );
                    let (mut milk, mut meat) = (0.0, 0.0);
                    for prod in &s.animal_products_h3 {
                        let (c_hto, _, c_obt) = tritium_in_animal(
                            feed.c_tfwt,
                            0.0,
                            *prod,
                            s.animal_feed_type,
                            TRITIUM_R_P,
                        );
                        if prod.is_milk() {
                            milk +=
                                c_hto * hto * did_milk * 1000.0 + c_obt * obt * did_milk * 1000.0;
                        }
                        if prod.is_meat() {
                            meat +=
                                c_hto * hto * did_meat * 1000.0 + c_obt * obt * did_meat * 1000.0;
                        }
                    }
                    IngestionDoseByRoute {
                        veg: msv(veg),
                        milk: msv(milk),
                        meat: msv(meat),
                    }
                }
                (IngestionModel::Carbon14, IngestionDcf::Single(d)) => {
                    let c_air = match s.mode {
                        IngestionReleaseMode::LongTerm => {
                            (n.release_bq * chi) / f64::from(365 * 24 * 3600)
                        }
                        IngestionReleaseMode::SinglePlume => n.release_bq * chi,
                    };
                    let veg = c14_in_plant(c_air, s.veg_type, C14_S_AIR) * d * did_veg * 1000.0;
                    let (mut milk, mut meat) = (0.0, 0.0);
                    for prod in &s.animal_products_c14 {
                        let c_pfw = c14_in_plant(c_air, s.animal_feed_type, C14_S_AIR);
                        let c_apf = c14_in_animal(c_pfw, s.animal_feed_type, *prod, 1.0);
                        if prod.is_milk() {
                            milk += c_apf * d * did_milk * 1000.0;
                        }
                        if prod.is_meat() {
                            meat += c_apf * d * did_meat * 1000.0;
                        }
                    }
                    IngestionDoseByRoute {
                        veg: msv(veg),
                        milk: msv(milk),
                        meat: msv(meat),
                    }
                }
                // A model/coefficient mismatch (e.g. tritium model with a
                // single coefficient): no dose can be formed.
                _ => IngestionDoseByRoute {
                    veg: msv(f64::NAN),
                    milk: msv(f64::NAN),
                    meat: msv(f64::NAN),
                },
            }
        })
        .collect()
}
