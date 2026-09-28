// SPDX-License-Identifier: GPL-3.0-only
//! Second-tranche groups of the pyDOSEIA code-to-code fixture: QUADPACK,
//! ingestion, plume shine, DCF screening, plume rise and the driver. Included
//! by `tests/pydoseia_code_to_code.rs`; the methodology and the results are in
//! `docs/pydoseia-code-to-code.md` (generator:
//! `verification_and_validation/pydoseia_code_to_code/tranche2.py`).

use std::collections::BTreeMap;

use buangkok::pydoseia::assessment::{self, AssessmentTables, IngestionFailure, SummaryIngestion};
use buangkok::pydoseia::config::{DilutionSource, MetSettings, PyDoseiaConfig, ReleaseScenario};
use buangkok::pydoseia::dcf::LungAbsorptionType;
use buangkok::pydoseia::dcf_screening::{
    compute_max_dcf, AlternateNames, ScreeningSource, ScreeningTable, ScreeningTables,
};
use buangkok::pydoseia::dispersion::StabilityClass;
use buangkok::pydoseia::ingestion::corrected::{
    ingestion_dose_per_nuclide, IngestionModel, IngestionNuclide, IngestionSettings,
};
use buangkok::pydoseia::ingestion::food_chain::{AnimalProduct, Climate, VegetationType};
use buangkok::pydoseia::ingestion::upstream::{
    ingestion_dose_upstream, IngestionReleaseMode, UpstreamIngestionError, UpstreamIngestionInputs,
};
use buangkok::pydoseia::ingestion::{
    ingestion_weathering_correction_unused, zero_milk_and_meat, DietaryIntake, EcoParamTable,
    IngestionDcf, IngestionDcfTable, IngestionParameters, Receiver, SoilType,
};
use buangkok::pydoseia::met::MetClimatology;
use buangkok::pydoseia::plume_rise;
use buangkok::pydoseia::plume_shine::{
    self, AttenuationTable, GammaLine, GammaLineTable, PlumeShineGeometry, PlumeShineMode,
    PlumeShineRelease, PointSourceUnit,
};
use buangkok::pydoseia::quadpack::{qagse, tplquad};
use buangkok::pydoseia::units::DilutionFactor;
use buangkok::pydoseia::{dcf::AgeBracket, dose, nuclide};
use uom::si::f64::Length;
use uom::si::length::meter;

use super::{Row, Variant};

const ING_DCF: &str = include_str!("../data/pydoseia_synthetic_ingestion_dcf.csv");
const ECO: &str = include_str!("../data/pydoseia_synthetic_eco_param.csv");
const GAMMA: &str = include_str!("../data/pydoseia_synthetic_gamma_lines.csv");
const ATT: &str = include_str!("../data/pydoseia_synthetic_attenuation.csv");
const SCR_T7: &str = include_str!("../data/pydoseia_synthetic_screening_table7.csv");
const SCR_T5: &str = include_str!("../data/pydoseia_synthetic_screening_table5.csv");
const SCR_A2: &str = include_str!("../data/pydoseia_synthetic_screening_table_a2.csv");
const SCR_AG: &str = include_str!("../data/pydoseia_synthetic_screening_annex_g.csv");
const SCR_AF: &str = include_str!("../data/pydoseia_synthetic_screening_annex_f.csv");
const SCR_T4: &str = include_str!("../data/pydoseia_synthetic_screening_table4.csv");

/// Relative tolerance per second-tranche group, with the reason. `0.0` is
/// bit-exact.
pub const TOLERANCES: &[(&str, f64, &str)] = &[
    (
        "quadpack",
        0.0,
        "the same QUADPACK arithmetic; libm exp/log/sin/sqrt",
    ),
    ("tplquad", 0.0, "nested dqagse"),
    (
        "ingestion",
        0.0,
        "SRS 19 / TECDOC-1616 products and libm exp",
    ),
    ("dcf_ingestion", 0.0, "lookup and max"),
    ("eco_lookup", 0.0, "lookup"),
    ("ingestion_weathering_unused", 0.0, "one add"),
    ("zeroing", 0.0, "copies"),
    ("gamma_lines", 0.0, "one division"),
    ("attenuation", 0.0, "numpy.interp and three operations"),
    ("limits_single", 0.0, "sigmas (libm pow) and sums"),
    ("limits_sector", 0.0, "sigmas (libm pow) and sums"),
    ("limits_legacy", 0.0, "sigmas (libm pow) and sums"),
    (
        "plume_shine_single",
        0.0,
        "nested dqagse over a libm kernel",
    ),
    (
        "plume_shine_long_term",
        0.0,
        "nested dqagse over a libm kernel",
    ),
    (
        "plume_shine_met",
        1.0e-14,
        "numpy einsum sums 54 products per sector in its own order",
    ),
    ("point_source", 0.0, "products"),
    ("dcf_screening", 0.0, "lookup and max"),
    ("plume_rise", 0.0, "libm pow"),
    ("driver", 0.0, "the pathways above, unchanged"),
    ("driver_dcf_report", 0.0, "lookup, multiply-add"),
    (
        "driver_summary",
        0.0,
        "pandas sums (pairwise and Kahan) reproduced in order",
    ),
    (
        "boundary_totals_text",
        6.0e-4,
        "read back from upstream's text report, printed to 4 significant figures",
    ),
    ("driver_raises", 0.0, "a flag"),
];

/// Tables shared by the second-tranche groups.
pub struct Inputs2 {
    ing_dcf: IngestionDcfTable,
    eco: EcoParamTable,
    gamma: GammaLineTable,
    att: AttenuationTable,
    screening: ScreeningTables,
}

pub fn inputs2() -> Inputs2 {
    let t = |s, text| ScreeningTable::from_csv(s, text).unwrap();
    Inputs2 {
        ing_dcf: IngestionDcfTable::from_csv(ING_DCF).unwrap(),
        eco: EcoParamTable::from_csv(ECO).unwrap(),
        gamma: GammaLineTable::from_csv(GAMMA).unwrap(),
        att: AttenuationTable::from_csv(ATT).unwrap(),
        screening: ScreeningTables {
            inhalation: vec![
                t(ScreeningSource::Table7Jaeri, SCR_T7),
                t(ScreeningSource::Table5Jaeri, SCR_T5),
                t(ScreeningSource::TableA2Doe, SCR_A2),
                t(ScreeningSource::AnnexGIcrp119, SCR_AG),
            ],
            ingestion: vec![
                t(ScreeningSource::AnnexFIcrp119, SCR_AF),
                t(ScreeningSource::Table4Jaeri, SCR_T4),
            ],
        },
    }
}

/// Half-life text of every nuclide the second tranche uses (the generator's
/// first-tranche `NUCLIDES`/`EXTRA_HALF_LIVES` plus `EXTRA_PRIMARY_HALF_LIVES`).
fn half_life(n: &str) -> &'static str {
    match n {
        "SYN-1" => "5.0 y",
        "SYN-2" => "8.0 d",
        "SYN-3" => "30.0 y",
        "SYN-4" => "10.0 y",
        "SYN-5" => "110.0 m",
        "SYN-6" => "29.0 y",
        "SYN-7" => "6.0 h",
        "SYN-8" => "12.0 y",
        "SYN-10" => "250.0 s",
        "H-3" => "12.0 y",
        "C-14" => "5000.0 y",
        _ => panic!("{n}"),
    }
}

fn lambda(n: &str) -> f64 {
    nuclide::upstream_decay_constant(nuclide::parse_primary_half_life(half_life(n)).unwrap())
}

fn element(n: &str) -> &'static str {
    match n {
        "SYN-1" => "Co",
        "SYN-2" => "I",
        "SYN-3" => "Cs",
        "SYN-4" => "Kr",
        "SYN-5" => "F",
        "SYN-6" => "Sr",
        "SYN-7" => "Tc",
        "SYN-8" | "H-3" => "H",
        "SYN-10" => "Ru",
        "C-14" => "C",
        _ => panic!("{n}"),
    }
}

fn case_nuclides(case: &str) -> Vec<&'static str> {
    match case {
        "plain" => vec!["SYN-1", "SYN-2", "SYN-3", "SYN-6", "SYN-7", "SYN-10"],
        "with_h3" => vec!["SYN-1", "SYN-3", "SYN-6", "SYN-10", "H-3"],
        "with_c14" => vec!["SYN-1", "SYN-3", "SYN-5", "SYN-4", "C-14"],
        "with_h3_c14" => vec!["SYN-1", "SYN-2", "SYN-3", "H-3", "C-14"],
        "h3_only" => vec!["H-3"],
        "c14_only" => vec!["C-14"],
        "h3_c14_only" => vec!["H-3", "C-14"],
        "h3_first" => vec!["H-3", "SYN-1", "SYN-3"],
        "c14_middle" => vec!["SYN-1", "C-14", "SYN-3"],
        "h_named_syn8" => vec!["SYN-1", "SYN-3", "SYN-8"],
        _ => panic!("{case}"),
    }
}

fn products(key: &str) -> Vec<AnimalProduct> {
    use AnimalProduct as P;
    match key {
        "milk_meat" => vec![P::CowMilk, P::GoatMeat],
        "milks_egg" => vec![P::GoatMilk, P::CowMilk, P::BeefMeat, P::PorkMeat, P::Egg],
        "milk_only" => vec![P::CowMilk],
        "meat_only" => vec![P::GoatMeat, P::LambMeat],
        "meat_first" => vec![P::BroilerMeat, P::CowMilk],
        _ => panic!("{key}"),
    }
}

fn climate(s: &str) -> Climate {
    match s {
        "Continental" => Climate::Continental,
        "Maritime" => Climate::Maritime,
        "Arctic" => Climate::Arctic,
        "Mediterranean" => Climate::Mediterranean,
        _ => panic!("{s}"),
    }
}

fn veg(s: &str) -> VegetationType {
    match s {
        "leafy_vegetables" => VegetationType::LeafyVegetables,
        "non_leafy_vegetables" => VegetationType::NonLeafyVegetables,
        "root_crops" => VegetationType::RootCrops,
        "all_others" => VegetationType::AllOthers,
        _ => panic!("{s}"),
    }
}

fn soil(s: &str) -> SoilType {
    if s == "peatsoil" {
        SoilType::PeatSoil
    } else {
        SoilType::OtherSoil
    }
}

fn m(v: f64) -> Length {
    Length::new::<meter>(v)
}

fn ingestion_row(row: &Row, inp: &Inputs2, variant: Variant) -> Vec<f64> {
    let names = case_nuclides(row.s("case"));
    let n = names.len();
    let mode = if row.s("mode") == "long_term" {
        IngestionReleaseMode::LongTerm
    } else {
        IngestionReleaseMode::SinglePlume
    };
    let receiver = if row.s("receiver") == "adult" {
        Receiver::Adult
    } else {
        Receiver::Infant
    };
    let (params, adult, infant) = if row.s("params") == "ingen" {
        (
            IngestionParameters::INPUT_GENERATOR_DEFAULT,
            DietaryIntake::INPUT_GENERATOR_ADULT,
            DietaryIntake::INPUT_GENERATOR_INFANT,
        )
    } else {
        (
            IngestionParameters::DOSEFUNC_FALLBACK,
            DietaryIntake::DOSEFUNC_FALLBACK_ADULT,
            DietaryIntake::DOSEFUNC_FALLBACK_INFANT,
        )
    };
    let releases: Vec<f64> = (0..n).map(|i| 1.0e10 * (1.0 + 0.3 * i as f64)).collect();
    let chi = row.f("chi");
    if variant == Variant::CorrectedIngestion {
        // The labelled divergence: one row per nuclide, in input order.
        let age = AgeBracket::from_age_years(receiver.age_years()).unwrap();
        let nuclides: Vec<IngestionNuclide> = names
            .iter()
            .enumerate()
            .map(|(i, nm)| IngestionNuclide {
                model: match *nm {
                    "H-3" => IngestionModel::Tritium,
                    "C-14" => IngestionModel::Carbon14,
                    e => IngestionModel::Deposition {
                        deposition_velocity_m_per_s: dose::deposition_velocity_m_per_s(element(e)),
                        transfer: inp
                            .eco
                            .lookup(element(e))
                            .unwrap_or(buangkok::pydoseia::ingestion::TransferFactors::ZERO),
                    },
                },
                decay_constant_per_s: lambda(nm),
                release_bq: releases[i],
                dcf: inp.ing_dcf.lookup(nm, age),
            })
            .collect();
        let s = IngestionSettings {
            mode,
            parameters: params,
            diet: if receiver == Receiver::Adult {
                adult
            } else {
                infant
            },
            soil: soil(row.s("soil")),
            climate: climate(row.s("climate")),
            veg_type: veg(row.s("veg")),
            animal_feed_type: veg(row.s("feed")),
            animal_products_h3: products(row.s("h3")),
            animal_products_c14: products(row.s("c14")),
        };
        return ingestion_dose_per_nuclide(&nuclides, DilutionFactor::new(chi), &s)
            .iter()
            .flat_map(|r| {
                [
                    r.veg.millisieverts(),
                    r.milk.millisieverts(),
                    r.meat.millisieverts(),
                ]
            })
            .collect();
    }
    let ui = UpstreamIngestionInputs {
        nuclides: names.iter().map(|s| s.to_string()).collect(),
        elements: names.iter().map(|s| element(s).to_string()).collect(),
        decay_constants_per_s: names.iter().map(|s| lambda(s)).collect(),
        releases_bq: releases,
        mode,
        chi_over_q: chi,
        parameters: params,
        diet_adult: adult,
        diet_infant: infant,
        soil: soil(row.s("soil")),
        climate: climate(row.s("climate")),
        veg_type: veg(row.s("veg")),
        animal_feed_type: veg(row.s("feed")),
        animal_products_h3: products(row.s("h3")),
        animal_products_c14: products(row.s("c14")),
    };
    match (
        ingestion_dose_upstream(&ui, &inp.eco, &inp.ing_dcf, receiver),
        row.output.as_str(),
    ) {
        (
            Err(UpstreamIngestionError::IndexError(_) | UpstreamIngestionError::NameError),
            "raises",
        ) => vec![1.0],
        (Err(UpstreamIngestionError::NoBranchMatches), "returns_none") => vec![1.0],
        (Ok(out), shape) => {
            let got = format!(
                "shape_{}x{}",
                out.rows.len(),
                out.rows.first().map_or(0, Vec::len)
            );
            if got == shape {
                out.rows.iter().flatten().copied().collect()
            } else {
                vec![f64::NAN; 0]
            }
        }
        _ => vec![],
    }
}

fn quad_f(name: &str) -> fn(f64) -> f64 {
    match name {
        "exp_neg" => |x: f64| (-x).exp(),
        "inv_sqrt" => |x: f64| 1.0 / x.sqrt(),
        "log" => |x: f64| x.ln(),
        "sin50" => |x: f64| (50.0 * x).sin(),
        "runge" => |x: f64| 1.0 / (1.0 + 25.0 * x * x),
        "inv_x" => |x: f64| 1.0 / x,
        "spike" => |x: f64| (-1.0e4 * (x - 0.3).powf(core::hint::black_box(2.0))).exp(),
        _ => panic!("{name}"),
    }
}

fn plume_rads() -> [(&'static str, f64); 3] {
    [("SYN-1", 2.0e12), ("SYN-3", 5.0e11), ("SYN-6", 1.0e12)]
}

fn gamma_lines(inp: &Inputs2, n: &str, variant: Variant) -> Vec<GammaLine> {
    if variant == Variant::ExactGammaMatch {
        // A plausible "fix" of the substring lookup: exact nuclide names only.
        let t = GammaLineTable {
            rows: inp
                .gamma
                .rows
                .iter()
                .filter(|r| r.nuclide == n)
                .cloned()
                .collect(),
        };
        t.plume_shine_lines(n)
    } else {
        inp.gamma.plume_shine_lines(n)
    }
}

/// The generator's driver scenarios (`DRIVER_SCENARIOS` / `driver_config`).
fn driver_config(scenario: &str) -> PyDoseiaConfig {
    let (single, met, rads, ages, dists, boundary): (
        bool,
        bool,
        Vec<&str>,
        Vec<f64>,
        Vec<f64>,
        f64,
    ) = match scenario {
        "long_term_no_met" | "user_dilution_factor" => (
            false,
            false,
            vec!["SYN-1", "SYN-3", "SYN-2", "SYN-10"],
            vec![1.0, 18.0],
            vec![150.0, 800.0],
            500.0,
        ),
        "single_plume_h3_c14" => (
            true,
            false,
            vec!["SYN-1", "SYN-3", "H-3", "C-14"],
            vec![1.0, 18.0],
            vec![150.0, 800.0],
            800.0,
        ),
        "long_term_met" => (
            false,
            true,
            vec!["SYN-1", "SYN-6"],
            vec![18.0],
            vec![300.0],
            1200.0,
        ),
        "age_10" => (
            false,
            false,
            vec!["SYN-1", "SYN-3", "SYN-2", "SYN-10"],
            vec![1.0, 10.0],
            vec![150.0, 800.0],
            500.0,
        ),
        _ => panic!("{scenario}"),
    };
    let q: Vec<f64> = (0..rads.len())
        .map(|i| 3.0e11 * (1.0 + 0.4 * i as f64))
        .collect();
    let release = if single {
        ReleaseScenario::SinglePlume {
            instantaneous_release_bq: q,
        }
    } else {
        ReleaseScenario::LongTerm {
            annual_discharge_bq: q,
        }
    };
    let mut c = PyDoseiaConfig::input_generator_defaults(release);
    c.nuclides = rads.iter().map(|s| s.to_string()).collect();
    c.elements = rads.iter().map(|s| element(s).to_string()).collect();
    c.absorption_types = vec![LungAbsorptionType::Max; rads.len()];
    c.downwind_distances_m = dists;
    c.plant_boundary_m = boundary;
    c.age_group = ages;
    c.release_height_m = 30.0;
    c.measurement_height_m = 10.0;
    c.consider_progeny = true;
    c.weathering_corr = true;
    c.exposure_period_y = 30.0;
    if met {
        c.met = Some(MetSettings {
            calm_correction: true,
            start_operation_time: 0,
            end_operation_time: 24,
            num_days: vec![30, 25],
            sampling_time: 60.0,
        });
    }
    if scenario == "user_dilution_factor" {
        c.dilution =
            DilutionSource::UserSupplied(vec![(150.0, 1.0e-5), (800.0, 2.0e-6), (500.0, 3.0e-6)]);
    }
    c
}

fn assessment_tables(t1: &super::Inputs, inp: &Inputs2) -> AssessmentTables {
    AssessmentTables {
        inhalation: t1.inhalation.clone(),
        surface: t1.surface.clone(),
        submersion: t1.submersion.clone(),
        chains: t1.chains.clone(),
        ingestion: inp.ing_dcf.clone(),
        eco: inp.eco.clone(),
        gamma: inp.gamma.clone(),
        attenuation: inp.att.clone(),
    }
}

/// Memoised driver runs, keyed by scenario.
pub struct DriverRuns(BTreeMap<String, assessment::AssessmentResults>);

impl DriverRuns {
    pub fn empty() -> Self {
        Self(BTreeMap::new())
    }
}

pub fn driver_runs(t1: &super::Inputs, inp: &Inputs2) -> DriverRuns {
    let tables = assessment_tables(t1, inp);
    let mut out = BTreeMap::new();
    for sc in [
        "long_term_no_met",
        "single_plume_h3_c14",
        "long_term_met",
        "user_dilution_factor",
        "age_10",
    ] {
        let cfg = driver_config(sc);
        let lambdas: Vec<f64> = cfg.nuclides.iter().map(|n| lambda(n)).collect();
        let met: Option<&MetClimatology> = t1.met.get(&(0, 24));
        let r = assessment::run_assessment(&cfg, &tables, &lambdas, met).unwrap();
        out.insert(sc.to_string(), r);
    }
    DriverRuns(out)
}

fn cell<'a>(runs: &'a DriverRuns, sc: &str, d: f64, age: f64) -> &'a assessment::CellResult {
    let r = &runs.0[sc];
    let xi = r.distances_m.iter().position(|x| *x == d).unwrap();
    r.cells[xi].iter().find(|c| c.age == age).unwrap()
}

/// The port for one second-tranche row; `None` if the group is not one of
/// this tranche's.
#[allow(clippy::too_many_lines)]
pub fn port(
    row: &Row,
    t1: &super::Inputs,
    inp: &Inputs2,
    runs: &DriverRuns,
    variant: Variant,
) -> Option<Vec<f64>> {
    let v = match row.group.as_str() {
        "quadpack" => {
            let f = quad_f(row.s("func"));
            let eps = row.f("eps");
            let r = if variant == Variant::PetirQag {
                let v = petir::integration::qag(
                    petir::integration::QkRule::Qk21,
                    f,
                    row.f("a"),
                    row.f("b"),
                    eps,
                    eps,
                    50,
                )
                .map_or(f64::NAN, |r| r.value);
                buangkok::pydoseia::quadpack::QuadResult {
                    value: v,
                    abserr: f64::NAN,
                    ier: -1,
                    neval: 0,
                }
            } else {
                qagse(f, row.f("a"), row.f("b"), eps, eps, 50)
            };
            vec![match row.output.as_str() {
                "value" => r.value,
                "abserr" => r.abserr,
                "ier" => f64::from(r.ier),
                "neval" => r.neval as f64,
                o => panic!("{o}"),
            }]
        }
        "tplquad" => vec![tplquad(
            |x, y, z| (-(x * x + 2.0 * y * y + 3.0 * z * z)).exp() * (1.0 + x * y),
            [0.0, 1.5, -1.0, 2.0, -0.5, 1.0],
            1.49e-4,
            1.49e-4,
        )],
        "ingestion" => ingestion_row(row, inp, variant),
        "dcf_ingestion" => {
            let age = AgeBracket::from_age_years(row.f("age")).unwrap();
            ["SYN-1", "SYN-3", "H-3", "C-14", "SYN-99"]
                .iter()
                .flat_map(|n| match inp.ing_dcf.lookup(n, age) {
                    IngestionDcf::Single(v) => vec![v],
                    IngestionDcf::Tritium { hto, obt } => vec![hto, obt],
                })
                .collect()
        }
        "eco_lookup" => {
            let els: Vec<&str> = row
                .s("elements")
                .split('|')
                .filter(|e| *e != "H" && *e != "C")
                .collect();
            let tf: Vec<_> = els.iter().map(|e| inp.eco.lookup(e)).collect();
            let z = buangkok::pydoseia::ingestion::TransferFactors::ZERO;
            if let Some(list) = row.output.strip_prefix("no_transfer:") {
                let missing: Vec<&str> = els
                    .iter()
                    .zip(&tf)
                    .filter(|(_, t)| t.is_none())
                    .map(|(e, _)| *e)
                    .collect();
                vec![if missing.join("|") == list { 1.0 } else { 0.0 }]
            } else {
                tf.iter()
                    .map(|t| {
                        let t = t.unwrap_or(z);
                        match row.output.as_str() {
                            "lambda_s" => t.lambda_s_per_d,
                            "lambda_w" => t.lambda_w_per_d,
                            "fv1" => t.fv1,
                            "fv2" => t.fv2,
                            "fm" => t.fm_milk_d_per_l,
                            "ff" => t.ff_meat_d_per_kg,
                            o => panic!("{o}"),
                        }
                    })
                    .collect()
            }
        }
        "ingestion_weathering_unused" => ["SYN-1", "SYN-2", "SYN-4", "SYN-8", "SYN-5"]
            .iter()
            .map(|n| {
                ingestion_weathering_correction_unused(lambda(n), element(n), row.b("weathering"))
            })
            .collect(),
        "zeroing" => {
            // upstream: SYN-1 (Co) and SYN-10 (Ru, no transfer factors)
            let t = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];
            let zero = variant == Variant::ZeroMilkAndMeat;
            vec![t[0], zero_milk_and_meat(t[1], zero)].concat()
        }
        "gamma_lines" => {
            let l = gamma_lines(inp, row.s("nuclide"), variant);
            if row.output == "energy_mev" {
                l.iter().map(|g| g.energy_mev).collect()
            } else {
                l.iter().map(|g| g.emission_probability).collect()
            }
        }
        "attenuation" => {
            let c = inp.att.air_coefficients(row.f("energy"));
            vec![c.k, c.mu_per_m, c.mu_a_per_m, c.mfp_m]
        }
        "limits_single" | "limits_sector" | "limits_legacy" => {
            let s = StabilityClass::from_code(row.f("stab") as u8).unwrap();
            let (x, h, mfp) = (m(row.f("x")), m(row.f("h")), row.f("mfp"));
            match row.group.as_str() {
                "limits_single" => plume_shine::integration_limits_single_plume(s, x, h, mfp),
                "limits_sector" => plume_shine::integration_limits_sector_averaged(s, x, h, mfp),
                _ => plume_shine::integration_limits_legacy(s, x, h, mfp, row.f("n")),
            }
            .to_vec()
        }
        "plume_shine_single" | "plume_shine_long_term" => {
            let single = row.group == "plume_shine_single";
            let mode = if single {
                PlumeShineMode::SinglePlume {
                    y: row.f("y"),
                    z: row.f("z"),
                }
            } else {
                PlumeShineMode::SectorAveraged { z: row.f("z") }
            };
            let g = PlumeShineGeometry {
                distance: m(row.f("x")),
                release_height: m(30.0),
                mode,
            };
            plume_rads()
                .iter()
                .flat_map(|(n, q)| {
                    let rel = if single {
                        PlumeShineRelease::InstantaneousBq(*q)
                    } else {
                        PlumeShineRelease::AnnualDischargeBq(*q)
                    };
                    plume_shine::per_class(&gamma_lines(inp, n, variant), &inp.att, g, rel)
                })
                .collect()
        }
        "plume_shine_met" => {
            let g = PlumeShineGeometry {
                distance: m(row.f("x")),
                release_height: m(30.0),
                mode: PlumeShineMode::SectorAveraged { z: 0.0 },
            };
            let met = &t1.met[&(0, 24)];
            plume_rads()
                .iter()
                .flat_map(|(n, q)| {
                    plume_shine::per_sector_with_met(
                        &gamma_lines(inp, n, variant),
                        &inp.att,
                        g,
                        met,
                        m(10.0),
                        PlumeShineRelease::AnnualDischargeBq(*q),
                    )
                })
                .collect()
        }
        "point_source" => {
            let unit = if row.s("unit") == "mSv/hr" {
                PointSourceUnit::MilliSievertPerHour
            } else {
                PointSourceUnit::MilliRoentgenPerHour
            };
            if row.s("case") == "defaults" {
                plume_shine::POINT_SOURCE_DEFAULT_DISTANCES_M
                    .iter()
                    .map(|d| {
                        plume_shine::point_source_dose(
                            &plume_shine::POINT_SOURCE_DEFAULT_IR192_KEV,
                            &plume_shine::POINT_SOURCE_DEFAULT_IR192_YIELD,
                            1.0e6,
                            *d,
                            5.0e-5,
                            unit,
                        )
                    })
                    .collect()
            } else {
                [3.0, 7.5, 250.0]
                    .iter()
                    .map(|d| {
                        plume_shine::point_source_dose(
                            &[662.0, 1173.0],
                            &[0.85, 0.5],
                            12.5,
                            *d,
                            1.0,
                            unit,
                        )
                    })
                    .collect()
            }
        }
        "dcf_screening" => {
            let n = row.s("nuclide");
            let alt = if n == "SCR-2" {
                AlternateNames {
                    doe_std_1196: Some("SCR-2A".into()),
                    ..AlternateNames::default()
                }
            } else {
                AlternateNames::default()
            };
            match compute_max_dcf(&inp.screening, n, row.s("type"), &alt, row.f("age")) {
                None => {
                    if row.output == "none" {
                        vec![1.0]
                    } else {
                        vec![]
                    }
                }
                Some(d) => {
                    let flags: String = [d.inhalation, d.ingestion]
                        .iter()
                        .map(|o| if o.is_some() { 'v' } else { 'n' })
                        .collect();
                    if row.output == format!("inh_ing:{flags}") {
                        vec![
                            d.inhalation.unwrap_or(f64::NAN),
                            d.ingestion.unwrap_or(f64::NAN),
                        ]
                    } else {
                        vec![]
                    }
                }
            }
        }
        "plume_rise" => vec![match row.s("case") {
            "neutral_defaults" => {
                plume_rise::plume_rise_neutral_unstable(10.0, 100.0, 2.0, 5.0, 8.0)
            }
            "stable_defaults" => plume_rise::plume_rise_stable_upstream(10.0, 2.0, 5.0),
            "neutral" => plume_rise::plume_rise_neutral_unstable(
                row.f("w0"),
                row.f("x"),
                row.f("u"),
                row.f("di"),
                row.f("de"),
            ),
            "stable" => {
                plume_rise::plume_rise_stable_upstream(row.f("w0"), row.f("u"), row.f("di"))
            }
            c => panic!("{c}"),
        }],
        "driver" => {
            let sc = row.s("scenario");
            let r = &runs.0[sc];
            match row.output.as_str() {
                "distances" => r.distances_m.clone(),
                "dilution" => {
                    let xi = r
                        .distances_m
                        .iter()
                        .position(|x| *x == row.f("distance"))
                        .unwrap();
                    r.dilution[xi]
                        .iter()
                        .map(|d| d.seconds_per_cubic_meter())
                        .collect()
                }
                "max_chi" => {
                    let xi = r
                        .distances_m
                        .iter()
                        .position(|x| *x == row.f("distance"))
                        .unwrap();
                    vec![r.max_chi_over_q[xi].seconds_per_cubic_meter()]
                }
                "doses" => {
                    let c = cell(runs, sc, row.f("distance"), row.f("age"));
                    [
                        &c.pathways.inhalation,
                        &c.pathways.ground_shine,
                        &c.pathways.submersion,
                    ]
                    .iter()
                    .flat_map(|v| v.iter().copied())
                    .collect()
                }
                "ingestion" => {
                    let c = cell(runs, sc, row.f("distance"), row.f("age"));
                    let out = c.ingestion.as_ref().unwrap();
                    out.rows.iter().flatten().copied().collect()
                }
                o => panic!("{o}"),
            }
        }
        "driver_dcf_report" => {
            let cfg = driver_config(row.s("scenario"));
            let tables = assessment_tables(t1, inp);
            let rep = assessment::dcf_report(
                &cfg,
                &tables,
                &ScreeningTables::default(),
                &[],
                row.f("age"),
            );
            rep.iter()
                .flat_map(|(_, gs, sub)| {
                    if row.output == "surface_pairs" {
                        [gs.corrected, gs.uncorrected]
                    } else {
                        [sub.corrected, sub.uncorrected]
                    }
                })
                .collect()
        }
        "driver_summary" => {
            let mode = if variant == Variant::SingleCountedIngestion {
                SummaryIngestion::PerNuclideRowsOnly
            } else {
                SummaryIngestion::UpstreamDoubleCounted
            };
            let rows = assessment::summary_rows(&runs.0[row.s("scenario")], mode);
            let r = rows
                .iter()
                .find(|r| r.distance_m == row.f("distance") && r.age == row.f("age"))
                .unwrap();
            vec![
                r.inhalation,
                r.ground_shine,
                r.submersion,
                r.ingestion,
                r.total,
            ]
        }
        "boundary_totals_text" => {
            let sc = row.s("scenario");
            let cfg = driver_config(sc);
            let totals = assessment::plant_boundary_totals(
                &cfg,
                &runs.0[sc],
                assessment::Zeroing::ChainedAssignmentNoOp,
                &[],
            );
            let ai = cfg
                .age_group
                .iter()
                .position(|a| *a == row.f("age"))
                .unwrap();
            let ni = cfg
                .nuclides
                .iter()
                .position(|n| n == row.s("nuclide"))
                .unwrap();
            let t = totals[ai].as_ref().unwrap()[ni];
            vec![
                t.inhalation,
                t.ground_shine,
                t.submersion,
                t.ingestion,
                t.total,
            ]
        }
        "driver_raises" => {
            let r = &runs.0["age_10"];
            let raised = r
                .cells
                .iter()
                .flatten()
                .any(|c| matches!(c.ingestion, Err(IngestionFailure::AgeHasNoReceiver)));
            vec![if raised { 1.0 } else { 0.0 }]
        }
        _ => return None,
    };
    Some(v)
}
