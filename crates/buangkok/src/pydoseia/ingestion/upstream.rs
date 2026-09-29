// SPDX-License-Identifier: GPL-3.0-only
//! `DoseFunc.ingestion_dose` exactly as upstream runs it, list indexing and
//! output layout included. Ported from pyDOSEIA `dosefunc.py` at commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`, Copyright (c) 2024
//! Dr. Biswajit Sadhu, MIT (see `crates/buangkok/NOTICE`).
//!
//! Use [`super::corrected::ingestion_dose_per_nuclide`] for new work; this
//! function exists so that the port can be compared with upstream number for
//! number, and so that upstream's defects D8-D11 are demonstrable.

use super::food_chain::{
    animal_products, c14_in_animal, c14_in_plant, deposition_rate_per_day, food_crop, per_day,
    tritium_in_animal, tritium_in_plant, AnimalProduct, Climate, VegetationType, C14_S_AIR,
    TRITIUM_CR_S, TRITIUM_GAMMA, TRITIUM_R_P,
};
use super::{
    DietaryIntake, EcoParamTable, IngestionDcf, IngestionDcfTable, IngestionParameters, Receiver,
    SoilType, TransferFactors,
};
use crate::pydoseia::dcf::AgeBracket;
use crate::pydoseia::dose::deposition_velocity_m_per_s;

/// Release mode, which sets the per-day discharge and the diet multiplier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestionReleaseMode {
    /// `long_term_release`: releases are Bq/y; the per-day discharge is
    /// `Q / 365` and the diet is multiplied by 365.
    LongTerm,
    /// `single_plume`: releases are Bq; the discharge is used as is and the
    /// diet is multiplied by 1 (upstream's `consumption_time_frac = 1`; the
    /// config's `consumption_time_food` is never read).
    SinglePlume,
}

/// Everything `ingestion_dose` reads, per call.
#[derive(Debug, Clone, PartialEq)]
pub struct UpstreamIngestionInputs {
    /// `rads_list`.
    pub nuclides: Vec<String>,
    /// `element_list` (same length).
    pub elements: Vec<String>,
    /// Decay constants, 1/s (`lambda_of_rads`), per nuclide.
    pub decay_constants_per_s: Vec<f64>,
    /// Releases per nuclide (`annual_discharge_bq_rad_list` or
    /// `instantaneous_release_bq_list`).
    pub releases_bq: Vec<f64>,
    /// Release mode.
    pub mode: IngestionReleaseMode,
    /// Maximum `chi/Q` for the distance, s/m^3.
    pub chi_over_q: f64,
    /// `inges_param_dict`.
    pub parameters: IngestionParameters,
    /// `inges_param_dict_adult`.
    pub diet_adult: DietaryIntake,
    /// `inges_param_dict_infant`.
    pub diet_infant: DietaryIntake,
    /// `soiltype`.
    pub soil: SoilType,
    /// `climate` (H-3 only).
    pub climate: Climate,
    /// `veg_type_list` (H-3 only; upstream iterates `[veg_type_list]`, so it
    /// must be a single type: a YAML list raises `TypeError`).
    pub veg_type: VegetationType,
    /// `animal_feed_type` (H-3 and C-14).
    pub animal_feed_type: VegetationType,
    /// `animal_product_list_for_tritium`.
    pub animal_products_h3: Vec<AnimalProduct>,
    /// `animal_product_list_for_C14`.
    pub animal_products_c14: Vec<AnimalProduct>,
}

/// Why upstream's `ingestion_dose` raises (or returns nothing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamIngestionError {
    /// A per-element list indexed past its end (`IndexError`, defect D8).
    IndexError(&'static str),
    /// Upstream prints `sum_hto_obt_animal_milk_tritium` before any milk
    /// product has defined it (`NameError`): the first tritium animal product
    /// is not a milk (defect D11b).
    NameError,
    /// Exactly `['H-3', 'C-14']` (or both with nothing else): no branch of the
    /// output assembly matches and upstream returns whatever the previous call
    /// left (`None` on a fresh object; defect D9).
    NoBranchMatches,
    /// A NaN age bracket (cannot happen for adult/infant; kept for totality).
    Age,
}

fn at(v: &[f64], i: usize, what: &'static str) -> Result<f64, UpstreamIngestionError> {
    v.get(i)
        .copied()
        .ok_or(UpstreamIngestionError::IndexError(what))
}

/// The three routes of one nuclide in upstream's output, in upstream's order
/// `(veg, milk, meat)`, mSv (or mSv/y).
pub type Routes = [f64; 3];

/// Upstream's result: the stacked array, and the elements that had no
/// transfer factors (upstream's `notransfer_factor_rad`, used by the report).
#[derive(Debug, Clone, PartialEq)]
pub struct UpstreamIngestionOutput {
    /// Rows exactly as upstream's array: one `[veg, milk, meat]` per non-H-3,
    /// non-C-14 nuclide in list order, then H-3, then C-14. When H-3, C-14 and
    /// at least one other nuclide are all present, upstream **transposes** the
    /// array; `transposed` is then true and `rows` holds the three route rows
    /// (veg, milk, meat), each of nuclide length.
    pub rows: Vec<Vec<f64>>,
    /// Whether `rows` is upstream's transposed layout.
    pub transposed: bool,
    /// Elements missing from the eco-parameter table.
    pub no_transfer_factors: Vec<String>,
}

/// `DoseFunc.ingestion_dose` for one distance and receiver, with upstream's
/// arithmetic and indexing.
///
/// # Errors
/// Where upstream raises; see [`UpstreamIngestionError`].
#[allow(clippy::too_many_lines)]
pub fn ingestion_dose_upstream(
    inp: &UpstreamIngestionInputs,
    eco: &EcoParamTable,
    dcf_table: &IngestionDcfTable,
    receiver: Receiver,
) -> Result<UpstreamIngestionOutput, UpstreamIngestionError> {
    use UpstreamIngestionError as E;
    let p = &inp.parameters;
    let is_hc = |e: &String| e == "H" || e == "C";
    // Source term per day.
    let day_discharge: Vec<f64> = inp
        .releases_bq
        .iter()
        .map(|q| match inp.mode {
            IngestionReleaseMode::LongTerm => q / 365.0,
            IngestionReleaseMode::SinglePlume => *q,
        })
        .collect();
    let v_d: Vec<f64> = inp
        .elements
        .iter()
        .map(|e| deposition_velocity_m_per_s(e))
        .collect();
    let mut dep = Vec::with_capacity(day_discharge.len());
    for (idx, d) in day_discharge.iter().enumerate() {
        dep.push(deposition_rate_per_day(
            *d,
            at(&v_d, idx, "list_deposition_vel")?,
            inp.chi_over_q,
        ));
    }
    let lambda_i: Vec<f64> = inp
        .decay_constants_per_s
        .iter()
        .map(|l| per_day(*l))
        .collect();
    // fv_list_ecerman_ingestion: compacted lists over non-H/C elements.
    let mut tf: Vec<TransferFactors> = Vec::new();
    let mut no_tf = Vec::new();
    for e in inp.elements.iter().filter(|e| !is_hc(e)) {
        match eco.lookup(e) {
            Some(t) => tf.push(t),
            None => {
                no_tf.push(e.clone());
                tf.push(TransferFactors::ZERO);
            }
        }
    }
    let lambda_s: Vec<f64> = tf.iter().map(|t| t.lambda_s_per_d).collect();
    let lambda_w: Vec<f64> = tf.iter().map(|t| t.lambda_w_per_d).collect();
    let fv1: Vec<f64> = tf.iter().map(|t| t.fv1).collect();
    let fv2: Vec<f64> = tf.iter().map(|t| t.fv2).collect();
    let fm: Vec<f64> = tf.iter().map(|t| t.fm_milk_d_per_l).collect();
    let ff: Vec<f64> = tf.iter().map(|t| t.ff_meat_d_per_kg).collect();
    // ingestion_weathering_correction_real: compacted, but indexed by the
    // FULL element position (D8).
    let mut l_eiv = Vec::new();
    let mut l_eis = Vec::new();
    for (ndx, e) in inp.elements.iter().enumerate() {
        if !is_hc(e) {
            l_eiv
                .push(at(&lambda_w, ndx, "lambda_w_per_d_list")? + at(&lambda_i, ndx, "lambda_i")?);
            l_eis
                .push(at(&lambda_s, ndx, "lambda_s_per_d_list")? + at(&lambda_i, ndx, "lambda_i")?);
        }
    }
    let (rho_pasture, rho_crop) = inp.soil.surface_densities();
    // Veg route.
    let mut cvi_list = Vec::new();
    for (ndx, e) in inp.elements.iter().enumerate() {
        if !is_hc(e) {
            let c = food_crop(
                at(&dep, ndx, "deposition_rate_per_day")?,
                at(&l_eiv, ndx, "lambda_eiv_list")?,
                at(&l_eis, ndx, "lambda_eis_list")?,
                at(&fv2, ndx, "f_v2_list")?,
                at(&lambda_i, ndx, "lambda_i")?,
                rho_crop,
                p,
            );
            cvi_list.push(c.cvi);
        }
    }
    // Milk and meat route.
    let mut cmi_list = Vec::new();
    let mut cfi_list = Vec::new();
    for (ndx, e) in inp.elements.iter().enumerate() {
        if !is_hc(e) {
            let a = animal_products(
                at(&dep, ndx, "deposition_rate_per_day")?,
                at(&l_eiv, ndx, "lambda_eiv_list")?,
                at(&l_eis, ndx, "lambda_eis_list")?,
                at(&fv1, ndx, "f_v1_list")?,
                at(&fm, ndx, "Fm_Milk_d_per_L_list")?,
                at(&ff, ndx, "Ff_Meat_d_per_kg_list")?,
                at(&lambda_i, ndx, "lambda_i")?,
                rho_pasture,
                p,
            );
            cmi_list.push(a.c_mi);
            cfi_list.push(a.c_fi);
        }
    }
    let age = AgeBracket::from_age_years(receiver.age_years()).ok_or(E::Age)?;
    let dcfs: Vec<IngestionDcf> = inp
        .nuclides
        .iter()
        .map(|n| dcf_table.lookup(n, age))
        .collect();
    let diet = match receiver {
        Receiver::Adult => inp.diet_adult,
        Receiver::Infant => inp.diet_infant,
    };
    let mult = match inp.mode {
        IngestionReleaseMode::LongTerm => 365.0,
        IngestionReleaseMode::SinglePlume => 1.0,
    };
    let (did_veg, did_milk, did_meat) = (diet.veg * mult, diet.milk * mult, diet.meat * mult);
    let single = |d: &IngestionDcf| match d {
        IngestionDcf::Single(v) => *v,
        // numpy would multiply by the list; upstream never reaches this for a
        // non-H-3 name, since only 'H-3' gets a pair.
        IngestionDcf::Tritium { .. } => f64::NAN,
    };

    let mut ingestion_dose: Vec<Routes> = Vec::new();
    let mut tritium_row: Option<Routes> = None;
    let mut c14_row: Option<Routes> = None;
    for (ndx, name) in inp.nuclides.iter().enumerate() {
        if name != "H-3" && name != "C-14" {
            let d = single(&dcfs[ndx]);
            let veg = at(&cvi_list, ndx, "Cvi_list")? * d * did_veg * 1000.0;
            let milk = at(&cmi_list, ndx, "Cmi_list")? * d * did_milk * 1000.0;
            let meat = at(&cfi_list, ndx, "Cfi_list")? * d * did_meat * 1000.0;
            ingestion_dose.push([veg, milk, meat]);
        }
        if name == "H-3" {
            let (dcf_hto, dcf_obt) = dcfs
                .iter()
                .find_map(|d| match d {
                    IngestionDcf::Tritium { hto, obt } => Some((*hto, *obt)),
                    IngestionDcf::Single(_) => None,
                })
                .ok_or(E::IndexError("tritium dcf"))?;
            let q = inp.releases_bq[inp.nuclides.iter().position(|n| n == "H-3").unwrap_or(ndx)];
            let plant = tritium_in_plant(
                inp.chi_over_q,
                q,
                inp.climate,
                inp.veg_type,
                TRITIUM_CR_S,
                TRITIUM_GAMMA,
                TRITIUM_R_P,
            );
            let hto = plant.c_pfw_hto * dcf_hto * did_veg * 1000.0;
            let obt = plant.c_pfw_obt * dcf_obt * did_veg * 1000.0;
            let sum_veg = 0.0 + (hto + obt);
            let mut sum_milk = 0.0;
            let mut sum_meat = 0.0;
            let mut milk_defined = false;
            for prod in &inp.animal_products_h3 {
                let feed_plant = tritium_in_plant(
                    inp.chi_over_q,
                    q,
                    inp.climate,
                    inp.animal_feed_type,
                    TRITIUM_CR_S,
                    TRITIUM_GAMMA,
                    TRITIUM_R_P,
                );
                let (c_hto, _c_f_obt, c_obt) = tritium_in_animal(
                    feed_plant.c_tfwt,
                    0.0,
                    *prod,
                    inp.animal_feed_type,
                    TRITIUM_R_P,
                );
                if prod.is_milk() {
                    let h = c_hto * dcf_hto * did_milk * 1000.0;
                    let o = c_obt * dcf_obt * did_milk * 1000.0;
                    sum_milk += h + o;
                    milk_defined = true;
                }
                if !milk_defined {
                    return Err(E::NameError);
                }
                if prod.is_meat() {
                    let h = c_hto * dcf_hto * did_meat * 1000.0;
                    let o = c_obt * dcf_obt * did_meat * 1000.0;
                    sum_meat += h + o;
                }
            }
            tritium_row = Some([sum_veg, sum_milk, sum_meat]);
        }
        if name == "C-14" {
            let q = inp.releases_bq[inp.nuclides.iter().position(|n| n == "C-14").unwrap_or(ndx)];
            let d = single(&dcfs[ndx]);
            let c_air = q * inp.chi_over_q;
            let c_pfw = c14_in_plant(c_air, inp.veg_type, C14_S_AIR);
            let sum_veg = 0.0 + c_pfw * d * did_veg * 1000.0;
            let mut sum_milk = 0.0;
            let mut sum_meat = 0.0;
            let any_milk = inp.animal_products_c14.iter().any(|x| x.is_milk());
            let any_meat = inp.animal_products_c14.iter().any(|x| x.is_meat());
            for prod in &inp.animal_products_c14 {
                let c_pfw = c14_in_plant(c_air, inp.animal_feed_type, C14_S_AIR);
                let c_apf = c14_in_animal(c_pfw, inp.animal_feed_type, *prod, 1.0);
                if any_milk {
                    sum_milk += c_apf * d * did_milk * 1000.0;
                }
                if any_meat {
                    sum_meat += c_apf * d * did_meat * 1000.0;
                }
            }
            c14_row = Some([sum_veg, sum_milk, sum_meat]);
        }
    }
    let has_h = inp.nuclides.iter().any(|n| n == "H-3");
    let has_c = inp.nuclides.iter().any(|n| n == "C-14");
    let n = inp.nuclides.len();
    let to_rows = |v: Vec<Routes>| v.into_iter().map(|r| r.to_vec()).collect::<Vec<_>>();
    let (rows, transposed) = if has_h && has_c && n > 2 {
        let mut all = ingestion_dose.clone();
        all.extend(tritium_row);
        all.extend(c14_row);
        let t: Vec<Vec<f64>> = (0..3).map(|k| all.iter().map(|r| r[k]).collect()).collect();
        (t, true)
    } else if has_h && !has_c && n > 1 {
        let mut all = ingestion_dose.clone();
        all.extend(tritium_row);
        (to_rows(all), false)
    } else if !has_h && has_c && n > 1 {
        let mut all = ingestion_dose.clone();
        all.extend(c14_row);
        (to_rows(all), false)
    } else if has_h && n == 1 {
        (to_rows(tritium_row.into_iter().collect()), false)
    } else if has_c && n == 1 {
        (to_rows(c14_row.into_iter().collect()), false)
    } else if !has_h && !has_c && n > 0 {
        (to_rows(ingestion_dose), false)
    } else {
        return Err(E::NoBranchMatches);
    };
    Ok(UpstreamIngestionOutput {
        rows,
        transposed,
        no_transfer_factors: no_tf,
    })
}
