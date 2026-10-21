// SPDX-License-Identifier: GPL-3.0-only
//! Ingestion: pyDOSEIA's terrestrial food-chain model (IAEA SRS 19 screening
//! equations for leafy vegetables / food crops, pasture, stored feed, milk and
//! meat) and its specific-activity models for H-3 and C-14 (IAEA TECDOC-1616).
//!
//! > **Research, education and V&V only** (`RESPONSIBLE_USE.md`). Never a
//! > dose to a real person.
//!
//! # Provenance
//!
//! Ported from pyDOSEIA `dosefunc.py` (`DoseFunc.ingestion_dose` with its
//! nested `conc_tritium_in_terrestrial_plant`,
//! `conc_tritium_in_terrestrial_animal`, `conc_c14_in_terrestrial_plants`,
//! `conc_c14_in_terrestrial_animal`, `conc_c14_in_fish`; `zeroing_ingestion`)
//! and `raddcffunc.py` (`dcf_list_ingestion`, `fv_list_ecerman_ingestion`,
//! `ingestion_weathering_correction_real`, `ingestion_weathering_correction`,
//! `effective_surface_soil_density_rho`), upstream
//! <https://github.com/BiswajitSadhu/pyDOSEIA> at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`. Copyright (c) 2024 Dr. Biswajit
//! Sadhu; MIT licence (full notice in `crates/buangkok/NOTICE`). The model
//! equations are those of IAEA Safety Reports Series No. 19 (2001) section 5
//! and IAEA-TECDOC-1616 (2009), which upstream cites; the handful of scalar
//! constants upstream hard-codes (soil densities, humidities, water contents,
//! stable-carbon contents, concentration ratios) are reproduced as upstream's
//! code literals with its citations.
//!
//! # No coefficient table ships with this crate
//!
//! | Upstream sheet | Content | Source | Here |
//! |---|---|---|---|
//! | `Dose_ecerman_final.xlsx` / `eco_param` | element transfer factors `Fv1`, `Fv2`, `Fm`, `Ff`, soil and plant loss rates | IAEA SRS 19 Tables VII, X, XI (IAEA copyright) | **not copied**; read your own with [`EcoParamTable::from_csv`] |
//! | `Dose_ecerman_final.xlsx` / `ingestion_gsr3` | ingestion e(g), six ages; `HTO`, `OBT` rows for tritium | IAEA GSR Part 3 Schedule III / ICRP 72 (copyright) | **not copied**; read your own with [`IngestionDcfTable::from_csv`] |
//!
//! The code-to-code test uses **synthetic** tables in these layouts.
//!
//! # Two drivers: upstream's, and a corrected one
//!
//! [`upstream::ingestion_dose_upstream`] reproduces `ingestion_dose` exactly,
//! including its list-indexing defect (D8: the per-element lists skip H and C
//! but are indexed by position in the full nuclide list, so any H or C
//! nuclide that is not at the end shifts or breaks every later nuclide), its
//! output layout (non-H/C rows, then H-3, then C-14; transposed when all three
//! kinds are present, and no result at all for exactly `[H-3, C-14]`: D9), the
//! C-14 air concentration left per year (D10), and the C-14 milk/meat test on
//! the whole product list (D11). [`corrected::ingestion_dose_per_nuclide`] is
//! a labelled **divergence** that computes each nuclide on its own and fixes
//! D8-D11; the default everywhere else stays upstream's.

pub mod corrected;
pub mod food_chain;
pub mod upstream;

use crate::pydoseia::csv::{col, num, split_csv};
use crate::pydoseia::dcf::{nan_max, AgeBracket, InhalationDcfTable};

/// Upstream's `inges_param_dict`: SRS 19 food-chain parameters (days, m^2/kg,
/// kg/d, m^3/d, Bq/m^3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IngestionParameters {
    /// Interception per unit mass, wet food crops, m^2/kg.
    pub alpha_wet_crops: f64,
    /// Interception per unit mass, dry forage, m^2/kg.
    pub alpha_dry_forage: f64,
    /// Crop exposure period during growth, food crops, d.
    pub t_e_food_crops: f64,
    /// Crop exposure period, forage grass, d.
    pub t_e_forage_grass: f64,
    /// Duration of the discharge (soil build-up), d.
    pub t_b: f64,
    /// Harvest-to-consumption delay, food crops, d.
    pub t_h_wet_crops: f64,
    /// Delay for fresh pasture, d.
    pub t_h_animal_pasture: f64,
    /// Delay for stored feed, d.
    pub t_h_animal_stored_feed: f64,
    /// Radionuclide concentration in the animals' water, Bq/m^3.
    pub c_wi: f64,
    /// Fraction of the year on fresh pasture.
    pub f_p: f64,
    /// Upstream key `alpha` (read, unused).
    pub alpha: f64,
    /// Upstream key `t_e` (read, unused).
    pub t_e: f64,
    /// Milk collection-to-consumption delay, d.
    pub t_m: f64,
    /// Meat collection-to-consumption delay, d.
    pub t_f: f64,
    /// Dry feed eaten by a milk animal, kg/d.
    pub q_m: f64,
    /// Water drunk by a milk animal, m^3/d.
    pub q_w: f64,
    /// Feed eaten by a meat animal, kg/d.
    pub q_f: f64,
    /// Water drunk by a meat animal, m^3/d.
    pub q_w_meat: f64,
}

impl IngestionParameters {
    /// The fallback in `dosefunc.py` when the config gives no
    /// `inges_param_dict` (`t_h_wet_crops = 1`, `t_f = 20`).
    pub const DOSEFUNC_FALLBACK: Self = Self {
        alpha_wet_crops: 0.3,
        alpha_dry_forage: 3.0,
        t_e_food_crops: 60.0,
        t_e_forage_grass: 30.0,
        t_b: 11000.0,
        t_h_wet_crops: 1.0,
        t_h_animal_pasture: 0.0,
        t_h_animal_stored_feed: 90.0,
        c_wi: 0.0,
        f_p: 0.7,
        alpha: 3.0,
        t_e: 30.0,
        t_m: 1.0,
        t_f: 20.0,
        q_m: 16.0,
        q_w: 0.06,
        q_f: 1.2,
        q_w_meat: 0.004,
    };

    /// The default the input generator (`get_inges_param_dict`) writes into a
    /// config: as [`Self::DOSEFUNC_FALLBACK`] but `t_h_wet_crops = 14`.
    pub const INPUT_GENERATOR_DEFAULT: Self = Self {
        t_h_wet_crops: 14.0,
        ..Self::DOSEFUNC_FALLBACK
    };
}

/// Upstream's dietary intake dictionary (`inges_param_dict_adult` /
/// `_infant`): `DID_veg`, `DID_milk`, `DID_meat`, `DID_fish`,
/// `DID_water_and_beverage`.
///
/// The pathway multiplies these by 365 for a long-term release and by 1 for a
/// single plume, so they are **per day**. The input generator's defaults are
/// per day (1.05 kg/d of vegetables for an adult); the fallback in
/// `dosefunc.py` holds **annual** values (76.7 kg of vegetables) that are then
/// multiplied by 365 again (defect D12). Fish and water are read but no
/// pathway uses them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DietaryIntake {
    /// Vegetables, kg/d.
    pub veg: f64,
    /// Milk, L/d.
    pub milk: f64,
    /// Meat, kg/d.
    pub meat: f64,
    /// Fish, kg/d (unused upstream).
    pub fish: f64,
    /// Water and beverages, m^3/d (unused upstream).
    pub water_and_beverage: f64,
}

impl DietaryIntake {
    /// Input-generator default, adult (`get_inges_param_dict_adult`).
    pub const INPUT_GENERATOR_ADULT: Self = Self {
        veg: 1.050,
        milk: 0.500,
        meat: 0.040,
        fish: 0.050,
        water_and_beverage: 0.002,
    };
    /// Input-generator default, infant (`get_inges_param_dict_infant`).
    pub const INPUT_GENERATOR_INFANT: Self = Self {
        veg: 0.215,
        milk: 0.400,
        meat: 0.0032876,
        fish: 0.004109,
        water_and_beverage: 0.0007123,
    };
    /// `dosefunc.py` fallback, adult. **Annual** values used as daily ones
    /// (defect D12); kept for fidelity.
    pub const DOSEFUNC_FALLBACK_ADULT: Self = Self {
        veg: 76.7,
        milk: 182.5,
        meat: 14.6,
        fish: 18.3,
        water_and_beverage: 0.73,
    };
    /// `dosefunc.py` fallback, infant (same caveat, D12).
    pub const DOSEFUNC_FALLBACK_INFANT: Self = Self {
        veg: 78.5,
        milk: 146.0,
        meat: 1.2,
        fish: 1.5,
        water_and_beverage: 0.26,
    };
}

/// Who eats: upstream's `receiver`, which fixes the ingestion coefficient age
/// (adult 18, infant 1) and the diet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Receiver {
    /// `'adult'`, age 18.
    Adult,
    /// `'infant'`, age 1.
    Infant,
}

impl Receiver {
    /// The age upstream uses for the coefficient lookup.
    #[must_use]
    pub const fn age_years(self) -> f64 {
        match self {
            Self::Adult => 18.0,
            Self::Infant => 1.0,
        }
    }

    /// Upstream's driver (`agewise_ingestion_dose`): `age > 17` is an adult,
    /// `age == 1` an infant, and **any other age has no ingestion dose** (the
    /// driver then raises `UnboundLocalError`: defect D13). `None` for those.
    #[must_use]
    pub fn from_driver_age(age: f64) -> Option<Self> {
        if age > 17.0 {
            Some(Self::Adult)
        } else if age == 1.0 {
            Some(Self::Infant)
        } else {
            None
        }
    }
}

/// Soil type for the effective surface density (`soiltype`), SRS 19 Table IX.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoilType {
    /// `'peatsoil'`: 50 kg/m^2 (pasture), 100 kg/m^2 (crops).
    PeatSoil,
    /// `'othersoil'`: 130 kg/m^2 (pasture), 260 kg/m^2 (crops).
    OtherSoil,
}

impl SoilType {
    /// `(rho_pasture_depth_lt_11, rho_crop_depth_ge_11)`, kg/m^2 dry soil.
    #[must_use]
    pub const fn surface_densities(self) -> (f64, f64) {
        match self {
            Self::PeatSoil => (50.0, 100.0),
            Self::OtherSoil => (130.0, 260.0),
        }
    }
}

/// Element transfer factors and loss rates, one row of upstream's `eco_param`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferFactors {
    /// Soil loss rate `lambda_s`, 1/d.
    pub lambda_s_per_d: f64,
    /// Soil-to-pasture concentration factor `Fv1`.
    pub fv1: f64,
    /// Soil-to-crop concentration factor `Fv2`.
    pub fv2: f64,
    /// Plant-surface loss rate `lambda_w`, 1/d.
    pub lambda_w_per_d: f64,
    /// Feed-to-milk transfer `Fm`, d/L.
    pub fm_milk_d_per_l: f64,
    /// Feed-to-meat transfer `Ff`, d/kg.
    pub ff_meat_d_per_kg: f64,
}

impl TransferFactors {
    /// All zeros: what upstream assigns to an element missing from the table.
    pub const ZERO: Self = Self {
        lambda_s_per_d: 0.0,
        fv1: 0.0,
        fv2: 0.0,
        lambda_w_per_d: 0.0,
        fm_milk_d_per_l: 0.0,
        ff_meat_d_per_kg: 0.0,
    };
}

/// Upstream's `eco_param` sheet.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EcoParamTable {
    /// `(Element field, factors)` in file order.
    pub rows: Vec<(String, TransferFactors)>,
}

impl EcoParamTable {
    /// Read a CSV with upstream's columns `Element, lambda_s_per_d, Fv1, Fv2,
    /// lambda_w_per_d, Fm_Milk_d_per_L, Ff_Meat_d_per_kg`.
    ///
    /// # Errors
    /// A missing column.
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let (h, rows) = split_csv(text);
        let e = col(&h, "Element")?;
        let c = [
            col(&h, "lambda_s_per_d")?,
            col(&h, "Fv1")?,
            col(&h, "Fv2")?,
            col(&h, "lambda_w_per_d")?,
            col(&h, "Fm_Milk_d_per_L")?,
            col(&h, "Ff_Meat_d_per_kg")?,
        ];
        Ok(Self {
            rows: rows
                .into_iter()
                .map(|r| {
                    (
                        r.get(e).cloned().unwrap_or_default(),
                        TransferFactors {
                            lambda_s_per_d: num(r.get(c[0])),
                            fv1: num(r.get(c[1])),
                            fv2: num(r.get(c[2])),
                            lambda_w_per_d: num(r.get(c[3])),
                            fm_milk_d_per_l: num(r.get(c[4])),
                            ff_meat_d_per_kg: num(r.get(c[5])),
                        },
                    )
                })
                .collect(),
        })
    }

    /// Upstream's lookup in `fv_list_ecerman_ingestion`: the **first** row
    /// whose `Element` field contains the symbol as a whitespace-delimited
    /// token (regex `(?:\s|^)X(?:\s|$)`). `None` if there is none (upstream
    /// then records the element as having no transfer factors and uses
    /// zeros).
    #[must_use]
    pub fn lookup(&self, element: &str) -> Option<TransferFactors> {
        self.rows
            .iter()
            .find(|(field, _)| {
                field.split_whitespace().any(|t| t == element) && !element.is_empty()
            })
            .map(|(_, f)| *f)
    }
}

/// An ingestion coefficient: one value, or tritium's HTO and OBT pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IngestionDcf {
    /// Sv/Bq.
    Single(f64),
    /// Tritium, Sv/Bq: tritiated water and organically bound tritium.
    Tritium {
        /// HTO.
        hto: f64,
        /// OBT.
        obt: f64,
    },
}

/// Upstream's `ingestion_gsr3` sheet: `Nuclide` and the six e(g) columns of
/// [`InhalationDcfTable::AGE_COLUMNS`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct IngestionDcfTable {
    /// `(nuclide, coefficients by age bracket)`.
    pub rows: Vec<(String, [f64; 6])>,
}

impl IngestionDcfTable {
    /// Read a CSV with columns `Nuclide` and the six `e_g_age_g_*_Sv/Bq`.
    ///
    /// # Errors
    /// A missing column.
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let (h, rows) = split_csv(text);
        let n = col(&h, "Nuclide")?;
        let ages: Vec<usize> = InhalationDcfTable::AGE_COLUMNS
            .iter()
            .map(|c| col(&h, c))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            rows: rows
                .into_iter()
                .map(|r| {
                    (
                        r.get(n).cloned().unwrap_or_default(),
                        core::array::from_fn(|k| num(r.get(ages[k]))),
                    )
                })
                .collect(),
        })
    }

    fn max_for(&self, name: &str, age: AgeBracket) -> f64 {
        nan_max(
            self.rows
                .iter()
                .filter(|(n, _)| n == name)
                .map(|(_, v)| v[age.column()]),
        )
    }

    /// Upstream's `dcf_list_ingestion` for one nuclide: the largest
    /// coefficient over rows named exactly `nuclide` (NaN if none); for
    /// `"H-3"` the `HTO` and `OBT` rows instead.
    #[must_use]
    pub fn lookup(&self, nuclide: &str, age: AgeBracket) -> IngestionDcf {
        if nuclide == "H-3" {
            IngestionDcf::Tritium {
                hto: self.max_for("HTO", age),
                obt: self.max_for("OBT", age),
            }
        } else {
            IngestionDcf::Single(self.max_for(nuclide, age))
        }
    }
}

/// Upstream's `ingestion_weathering_correction` (the **unused** variant: the
/// pathway calls `ingestion_weathering_correction_real` instead). Adds a
/// 14-day weathering half-life (`0.693 / (14 * 86400)` 1/s) to the decay
/// constant of every element except H, C and the noble gases, when enabled.
#[must_use]
pub fn ingestion_weathering_correction_unused(
    decay_constant_per_s: f64,
    element: &str,
    enabled: bool,
) -> f64 {
    if !enabled {
        return decay_constant_per_s;
    }
    match element {
        "H" | "C" | "He" | "Ne" | "Ar" | "Kr" | "Xe" | "Rn" => decay_constant_per_s,
        _ => decay_constant_per_s + 0.693 / f64::from(14 * 24 * 3600),
    }
}

/// Upstream's `zeroing_ingestion` intent: the milk and meat doses of a
/// nuclide whose element has no transfer factors are set to zero.
///
/// Note: upstream writes `df.loc[rad][1:] = 0`, a chained assignment. Under
/// pandas copy-on-write (pandas 3, used for the fixture) it modifies a copy and
/// the report is **unchanged** (defect D14; checked in the fixture). This
/// function does what the code says it means; the code-to-code test records
/// both.
#[must_use]
pub fn zero_milk_and_meat(route: [f64; 3], element_has_no_transfer_factors: bool) -> [f64; 3] {
    if element_has_no_transfer_factors {
        [route[0], 0.0, 0.0]
    } else {
        route
    }
}
