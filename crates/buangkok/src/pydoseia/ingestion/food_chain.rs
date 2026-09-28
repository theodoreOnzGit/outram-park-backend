// SPDX-License-Identifier: GPL-3.0-only
//! The food-chain equations of pyDOSEIA's `ingestion_dose`, one nuclide at a
//! time, written in upstream's operation order. Ported from `dosefunc.py` and
//! `raddcffunc.py` at commit `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`,
//! Copyright (c) 2024 Dr. Biswajit Sadhu, MIT (see `crates/buangkok/NOTICE`).
//! The equations are IAEA SRS 19 (2001) eqs. for direct deposition, soil
//! uptake, pasture, stored feed, milk and meat, and IAEA-TECDOC-1616 (2009)
//! for the H-3 and C-14 specific-activity models, as upstream cites them.

use super::{IngestionParameters, TransferFactors};

/// Upstream's per-day deposition rate `d * v_d * chi/Q`, Bq m^-2 d^-1 (with
/// `d` the release per day: `Q / 365` for a long-term release, and the
/// released Bq itself for a single plume, as upstream).
#[must_use]
pub fn deposition_rate_per_day(day_discharge: f64, v_d_m_per_s: f64, chi_over_q: f64) -> f64 {
    day_discharge * v_d_m_per_s * chi_over_q
}

/// `ingestion_weathering_correction_real` for one element: effective removal
/// rates from plants and from soil, 1/d, `(lambda_w + lambda_i,
/// lambda_s + lambda_i)` with `lambda_i` the decay constant per day.
#[must_use]
pub fn effective_removal_rates(tf: TransferFactors, lambda_i_per_d: f64) -> (f64, f64) {
    (
        tf.lambda_w_per_d + lambda_i_per_d,
        tf.lambda_s_per_d + lambda_i_per_d,
    )
}

/// Decay constant per day as upstream converts it: `lambda * 24 * 3600`.
#[must_use]
pub fn per_day(lambda_per_s: f64) -> f64 {
    lambda_per_s * 24.0 * 3600.0
}

/// Food-crop concentrations (SRS 19 section 5.1), Bq/kg.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CropConcentrations {
    /// Direct deposition on the crop, `C_vi1`.
    pub c_vi1: f64,
    /// Soil concentration (crop root zone), `C_si`, Bq/kg dry soil.
    pub c_si: f64,
    /// Root uptake, `C_vi2 = Fv2 C_si`.
    pub c_vi2: f64,
    /// At consumption, `(C_vi1 + C_vi2) exp(-lambda_i t_h)`.
    pub cvi: f64,
}

/// Food crops for human consumption (upstream's "veg route").
#[must_use]
pub fn food_crop(
    dep_per_day: f64,
    lambda_eiv: f64,
    lambda_eis: f64,
    fv2: f64,
    lambda_i_per_d: f64,
    rho_crop: f64,
    p: &IngestionParameters,
) -> CropConcentrations {
    let c_vi1 = (dep_per_day * p.alpha_wet_crops * (1.0 - (-lambda_eiv * p.t_e_food_crops).exp()))
        / lambda_eiv;
    let c_si = (dep_per_day * (1.0 - (-lambda_eis * p.t_b).exp())) / (lambda_eis * rho_crop);
    let c_vi2 = fv2 * c_si;
    let cvi = (c_vi1 + c_vi2) * (-lambda_i_per_d * p.t_h_wet_crops).exp();
    CropConcentrations {
        c_vi1,
        c_si,
        c_vi2,
        cvi,
    }
}

/// Pasture, feed, milk and meat (SRS 19 section 5.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimalConcentrations {
    /// Direct deposition on forage, Bq/kg dry.
    pub c_vi1: f64,
    /// Pasture soil, Bq/kg dry soil.
    pub c_si: f64,
    /// Root uptake by pasture, `Fv1 C_si`.
    pub c_vi2: f64,
    /// Fresh pasture at grazing, `C_pasture`.
    pub cvi_animal: f64,
    /// Stored feed, `C_pi`.
    pub cpi: f64,
    /// Average feed, `f_p C_pasture + (1 - f_p) C_pi`.
    pub c_ai: f64,
    /// Milk, Bq/L.
    pub c_mi: f64,
    /// Meat, Bq/kg.
    pub c_fi: f64,
}

/// Upstream's milk and meat route for one nuclide.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn animal_products(
    dep_per_day: f64,
    lambda_eiv: f64,
    lambda_eis: f64,
    fv1: f64,
    fm: f64,
    ff: f64,
    lambda_i_per_d: f64,
    rho_pasture: f64,
    p: &IngestionParameters,
) -> AnimalConcentrations {
    let c_vi1 =
        (dep_per_day * p.alpha_dry_forage * (1.0 - (-lambda_eiv * p.t_e_forage_grass).exp()))
            / lambda_eiv;
    let c_si = (dep_per_day * (1.0 - (-lambda_eis * p.t_b).exp())) / (lambda_eis * rho_pasture);
    let c_vi2 = fv1 * c_si;
    let cvi_animal = (c_vi1 + c_vi2) * (-lambda_i_per_d * p.t_h_animal_pasture).exp();
    let cpi = (c_vi1 + c_vi2) * (-lambda_i_per_d * p.t_h_animal_stored_feed).exp();
    let c_ai = p.f_p * cvi_animal + (1.0 - p.f_p) * cpi;
    let c_mi = fm * ((c_ai * p.q_m) + (p.c_wi * p.q_w)) * (-lambda_i_per_d * p.t_m).exp();
    let c_fi = ff * ((c_ai * p.q_f) + (p.c_wi * p.q_w_meat)) * (-lambda_i_per_d * p.t_f).exp();
    AnimalConcentrations {
        c_vi1,
        c_si,
        c_vi2,
        cvi_animal,
        cpi,
        c_ai,
        c_mi,
        c_fi,
    }
}

/// Climate for the tritium model (upstream `climate_humidity`): latitude,
/// absolute humidity `H_a` (kg/m^3) and relative humidity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Climate {
    /// `'Mediterranean'`: 34, 0.0115, 0.6.
    Mediterranean,
    /// `'Continental'`: 48, 0.0087, 0.71.
    Continental,
    /// `'Maritime'`: 50, 0.0078, 0.795. (The input generator offers
    /// `'meritime'`, which the pathway's dictionary does not contain.)
    Maritime,
    /// `'Arctic'`: 50, 0.0067, 0.73. (The input generator offers `'arctic'`.)
    Arctic,
}

impl Climate {
    /// `(latitude, H_a, RH)`.
    #[must_use]
    pub const fn humidity(self) -> (f64, f64, f64) {
        match self {
            Self::Mediterranean => (34.0, 0.0115, 0.6),
            Self::Continental => (48.0, 0.0087, 0.71),
            Self::Maritime => (50.0, 0.0078, 0.795),
            Self::Arctic => (50.0, 0.0067, 0.73),
        }
    }
}

/// Vegetation (or animal feed) type for the H-3 and C-14 models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VegetationType {
    /// `'leafy_vegetables'`.
    LeafyVegetables,
    /// `'non_leafy_vegetables'`.
    NonLeafyVegetables,
    /// `'root_crops'`.
    RootCrops,
    /// `'all_others'`.
    AllOthers,
}

impl VegetationType {
    /// `(WEQ, WCp)`: water equivalent factor (L/kg dry) and water content
    /// (upstream's `veg_type_data`, TECDOC-1616 Table 3).
    #[must_use]
    pub const fn weq_wcp(self) -> (f64, f64) {
        match self {
            Self::LeafyVegetables => (0.51, 0.92),
            Self::NonLeafyVegetables => (0.53, 0.92),
            Self::RootCrops => (0.52, 0.87),
            Self::AllOthers => (0.56, 0.495),
        }
    }

    /// Stable carbon content `S_p`, gC/kg fresh weight (upstream's
    /// `veg_type_S_p_data`).
    #[must_use]
    pub const fn stable_carbon(self) -> f64 {
        match self {
            Self::LeafyVegetables | Self::NonLeafyVegetables => 30.0,
            Self::RootCrops => 46.0,
            Self::AllOthers => 219.0,
        }
    }
}

/// Animal products of the H-3 and C-14 models.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimalProduct {
    /// `'cow_milk'`.
    CowMilk,
    /// `'goat_milk'`.
    GoatMilk,
    /// `'goat_meat'`.
    GoatMeat,
    /// `'lamb_meat'`.
    LambMeat,
    /// `'beef_meat'`.
    BeefMeat,
    /// `'pork_meat'`.
    PorkMeat,
    /// `'broiler_meat'`.
    BroilerMeat,
    /// `'egg'` (has ratios but no dose route upstream).
    Egg,
}

impl AnimalProduct {
    /// `CR_a_HTO`, the HTO concentration ratio.
    #[must_use]
    pub const fn cr_hto(self) -> f64 {
        match self {
            Self::CowMilk => 0.87,
            Self::GoatMilk => 0.83,
            Self::GoatMeat | Self::PorkMeat => 0.67,
            Self::LambMeat => 0.78,
            Self::BeefMeat | Self::Egg => 0.66,
            Self::BroilerMeat => 0.76,
        }
    }

    /// `CR_a_OBT`, the OBT concentration ratio.
    #[must_use]
    pub const fn cr_obt(self) -> f64 {
        match self {
            Self::CowMilk => 0.24,
            Self::GoatMilk => 0.32,
            Self::GoatMeat => 0.43,
            Self::LambMeat => 0.55,
            Self::BeefMeat => 0.40,
            Self::PorkMeat | Self::Egg => 0.64,
            Self::BroilerMeat => 0.5,
        }
    }

    /// `S_a`, stable carbon in the product, gC/kg (TECDOC-1616 Table 12).
    #[must_use]
    pub const fn stable_carbon(self) -> f64 {
        match self {
            Self::CowMilk => 65.0,
            Self::GoatMilk => 71.0,
            Self::GoatMeat => 170.0,
            Self::LambMeat => 280.0,
            Self::BeefMeat => 200.0,
            Self::PorkMeat => 300.0,
            Self::BroilerMeat => 150.0,
            Self::Egg => 160.0,
        }
    }

    /// In upstream's milk list (`cow_milk`, `goat_milk`).
    #[must_use]
    pub const fn is_milk(self) -> bool {
        matches!(self, Self::CowMilk | Self::GoatMilk)
    }

    /// In upstream's meat list (`goat_meat`, `lamb_meat`, `beef_meat`,
    /// `broiler_meat`, `pork_meat`; its `cow_meat` has no ratios and would
    /// raise `KeyError`).
    #[must_use]
    pub const fn is_meat(self) -> bool {
        matches!(
            self,
            Self::GoatMeat | Self::LambMeat | Self::BeefMeat | Self::BroilerMeat | Self::PorkMeat
        )
    }
}

/// Upstream's defaults for the tritium plant model: `CR_s = 0.23`,
/// `gamma = 0.909`, `R_p = 0.54`.
pub const TRITIUM_CR_S: f64 = 0.23;
/// Vapour-pressure ratio HTO/H2O.
pub const TRITIUM_GAMMA: f64 = 0.909;
/// OBT/TFWT concentration ratio.
pub const TRITIUM_R_P: f64 = 0.54;
/// Stable carbon in air, gC/m^3 (upstream `S_air = 0.20`).
pub const C14_S_AIR: f64 = 0.20;

/// Output of [`tritium_in_plant`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TritiumPlant {
    /// Water equivalent factor.
    pub weq: f64,
    /// Water content.
    pub wcp: f64,
    /// Tissue-free water tritium.
    pub c_tfwt: f64,
    /// HTO in the fresh plant.
    pub c_pfw_hto: f64,
    /// OBT in the fresh plant.
    pub c_pfw_obt: f64,
}

/// `conc_tritium_in_terrestrial_plant`: air HTO from the release (Bq/y times
/// `chi/Q`, divided by `365 * 24 * 3600`), then air moisture, soil water,
/// tissue-free water, HTO and OBT in the plant.
///
/// Note (upstream behaviour, kept): the division by a year is applied for a
/// single-plume release as well, where the release is in Bq, not Bq/y.
#[must_use]
pub fn tritium_in_plant(
    chi_over_q: f64,
    discharge: f64,
    climate: Climate,
    veg: VegetationType,
    cr_s: f64,
    gamma: f64,
    r_p: f64,
) -> TritiumPlant {
    let c_hto_atm = (discharge * chi_over_q) / f64::from(365 * 24 * 3600);
    let (_lat, h_a, rh) = climate.humidity();
    let (weq, wcp) = veg.weq_wcp();
    let c_am = c_hto_atm / h_a;
    let c_sw = cr_s * c_am;
    let c_tfwt = ((rh * c_am) + (1.0 - rh) * c_sw) / gamma;
    let c_pfw_obt = (1.0 - wcp) * weq * r_p * c_tfwt;
    let c_pfw_hto = wcp * c_tfwt;
    TritiumPlant {
        weq,
        wcp,
        c_tfwt,
        c_pfw_hto,
        c_pfw_obt,
    }
}

/// `conc_tritium_in_terrestrial_animal`: `(C_afw_T_HTO, C_f_OBT,
/// C_afw_T_OBT)`. Upstream calls it with `C_f_HTO = 0` (the HTO in drinking
/// water is not modelled), so the HTO term is always zero there.
#[must_use]
pub fn tritium_in_animal(
    c_tfwt: f64,
    c_f_hto: f64,
    product: AnimalProduct,
    feed: VegetationType,
    r_p: f64,
) -> (f64, f64, f64) {
    let (weq, _wcp) = feed.weq_wcp();
    let c_afw_t_hto = product.cr_hto() * c_f_hto;
    let c_f_obt = weq * r_p * c_tfwt;
    let c_afw_t_obt = product.cr_obt() * c_f_obt;
    (c_afw_t_hto, c_f_obt, c_afw_t_obt)
}

/// `conc_c14_in_terrestrial_plants`: `C_air S_p / S_air`, Bq/kg fresh.
#[must_use]
pub fn c14_in_plant(c_air: f64, veg: VegetationType, s_air: f64) -> f64 {
    (c_air * veg.stable_carbon()) / s_air
}

/// `conc_c14_in_terrestrial_animal`: `f_c C_pfw S_a / S_p`, Bq/kg fresh
/// (upstream uses `f_c = 1`).
#[must_use]
pub fn c14_in_animal(c_pfw: f64, feed: VegetationType, product: AnimalProduct, f_c: f64) -> f64 {
    (f_c * c_pfw * product.stable_carbon()) / feed.stable_carbon()
}

/// C-14 in fish, `C_DIC * S_f` (default `S_f = 120` gC/kg).
///
/// **Divergence from upstream:** upstream's `conc_c14_in_fish(C_air, S_f)`
/// ignores `C_air` and reads an undefined `C_DIC` (a `NameError` if called;
/// it is never called, and marked TODO). This takes the dissolved inorganic
/// C-14 concentration explicitly, which is what the formula needs.
#[must_use]
pub fn c14_in_fish(c_dic: f64, s_f: f64) -> f64 {
    c_dic * s_f
}
