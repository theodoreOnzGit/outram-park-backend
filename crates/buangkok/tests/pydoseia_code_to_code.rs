// SPDX-License-Identifier: GPL-3.0-only
//! # pyDOSEIA code-to-code verification of `buangkok::pydoseia`
//!
//! **Methodology.** `verification_and_validation/pydoseia_code_to_code/
//! gen_pydoseia_reference.py` executes the upstream pyDOSEIA Python (commit
//! `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`, MIT) over a grid of inputs and
//! writes every output at full `repr()` precision to
//! `tests/data/pydoseia_reference.csv`. All inputs are synthetic: `SYN-*`
//! nuclides, synthetic coefficient and decay-chain tables in upstream's own
//! file layouts (`tests/data/pydoseia_synthetic_*.csv`), and synthetic hourly
//! met records with deliberate gaps and bin-edge values. This test replays each
//! row through the Rust port and compares, per function group, against a
//! relative tolerance chosen from the arithmetic (see [`TOLERANCES`]). NaN must
//! match NaN, and an exact zero must match an exact zero.
//!
//! **Results** (2026-09-28, upstream `dca4cdc3`, python 3.14.7, numpy 2.5.3,
//! pandas 3.0.6): recorded per group in `docs/pydoseia-code-to-code.md`, which
//! quotes the table this test prints under `--nocapture`.
//!
//! The mutation tests at the end show the comparison can fail: each one swaps
//! in a plausible alternative (the exact sector width, `ln 2`, the corrected
//! calm correction, an exact-match daughter lookup, an age-bracket shift) and
//! asserts that the fixture then **rejects** the port.

#![cfg(not(target_os = "android"))]

#[path = "pydoseia_c2c/tranche2.rs"]
mod tranche2;

use std::collections::BTreeMap;

use buangkok::pydoseia::dcf::{
    external_dcf, AgeBracket, ExternalDcfTable, InhalationDcfTable, LungAbsorptionType,
    ProgenyChains, ProgenyCorrection,
};
use buangkok::pydoseia::dispersion::{
    self, CalmCorrection, MeanSpeedScaling, PlumeGeometry, Receptor, StabilityClass,
};
use buangkok::pydoseia::dose::{self, Release, Weathering};
use buangkok::pydoseia::met::{self, MetClimatology, RawMetRecord};
use buangkok::pydoseia::nuclide;
use buangkok::pydoseia::units::DilutionFactor;
use uom::si::f64::{Length, Radioactivity, Velocity};
use uom::si::length::meter;
use uom::si::radioactivity::becquerel;
use uom::si::velocity::meter_per_second;

const FIXTURE: &str = include_str!("data/pydoseia_reference.csv");
const INHALATION: &str = include_str!("data/pydoseia_synthetic_inhalation_dcf.csv");
const SURFACE: &str = include_str!("data/pydoseia_synthetic_surface_dcf.csv");
const SUBMERSION: &str = include_str!("data/pydoseia_synthetic_submersion_dcf.csv");
const CHAINS: &str = include_str!("data/pydoseia_synthetic_progeny_chains.csv");
const CHAIN_HALF_LIVES: &str = include_str!("data/pydoseia_synthetic_progeny_half_lives.csv");
const MET: &str = include_str!("data/pydoseia_synthetic_met.csv");

/// The generator's synthetic nuclides: (name, element, half-life text). Must
/// match `NUCLIDES` in `gen_pydoseia_reference.py`.
const NUCLIDES: [(&str, &str, &str); 8] = [
    ("SYN-1", "Co", "5.0 y"),
    ("SYN-2", "I", "8.0 d"),
    ("SYN-3", "Cs", "30.0 y"),
    ("SYN-4", "Kr", "10.0 y"),
    ("SYN-5", "F", "110.0 m"),
    ("SYN-6", "Sr", "29.0 y"),
    ("SYN-7", "Tc", "6.0 h"),
    ("SYN-8", "H", "12.0 y"),
];
/// The generator's `mean_speeds` for the mean-speed scaling cases, m/s.
const MEAN_SPEEDS: [f64; 6] = [1.1, 1.7, 2.3, 3.4, 2.6, 1.9];
/// The generator's met-data geometry: release 30 m, measurement 10 m.
const MET_RELEASE_HEIGHT_M: f64 = 30.0;

/// Relative tolerance per function group, with the reason.
///
/// `0.0` means **bit-exact** is required: the port performs the same IEEE-754
/// operations in the same order as upstream and calls the same libm `pow`
/// (CPython's `float.__pow__` and numpy's scalar power both reach C `pow`).
///
/// Groups whose upstream path calls **numpy's `exp`** were also measured
/// bit-exact (2026-09-28, x86-64 Linux), but numpy dispatches `exp` to
/// CPU-specific SIMD kernels that are not guaranteed to round like libm, so
/// those groups allow a few ulp (`4e-16`; `1e-15` where ~150 such terms are
/// summed per sector). `speed_means` goes through a pandas mean whose summation
/// order is not the port's left-to-right sum; measured 4.8e-16 (2 ulp).
const TOLERANCES: &[(&str, f64, &str)] = &[
    ("sigmay", 0.0, "one pow, one multiply"),
    ("sigmaz", 0.0, "one pow, multiply, add"),
    ("height_factor", 0.0, "one pow"),
    ("master_single", 4.0e-16, "numpy exp (measured bit-exact)"),
    ("master_sector", 4.0e-16, "numpy exp (measured bit-exact)"),
    ("dilution_no_met", 4.0e-16, "numpy exp (measured bit-exact)"),
    ("tjfd", 0.0, "integer counts"),
    ("tjfd_missing", 0.0, "integer counts after truncation"),
    ("calm_factors", 0.0, "integer ratios, one division"),
    (
        "speed_means",
        2.0e-15,
        "pandas mean summation order (measured 4.8e-16)",
    ),
    (
        "dilution_met_long_term",
        1.0e-15,
        "numpy exp in ~150-term sums (measured bit-exact)",
    ),
    ("half_life", 0.0, "one multiply, one divide"),
    ("dcf_inhalation", 0.0, "table lookup and max"),
    ("dcf_surface", 0.0, "lookup, multiply-add in upstream order"),
    (
        "dcf_submersion",
        0.0,
        "lookup, multiply-add in upstream order",
    ),
    ("deposition_velocity", 0.0, "constants"),
    (
        "effective_lambda",
        4.0e-16,
        "numpy exp (measured bit-exact)",
    ),
    ("dose_inhalation", 0.0, "products in upstream order"),
    (
        "dose_ground_shine",
        4.0e-16,
        "carries the effective_lambda exp (measured bit-exact)",
    ),
    ("dose_submersion", 0.0, "products in upstream order"),
];

// --------------------------------------------------------------------------
// Fixture parsing
// --------------------------------------------------------------------------

struct Row {
    group: String,
    inputs: BTreeMap<String, String>,
    output: String,
    values: Vec<f64>,
}

fn parse_value(s: &str) -> f64 {
    match s {
        "nan" => f64::NAN,
        "inf" => f64::INFINITY,
        "-inf" => f64::NEG_INFINITY,
        _ => s.parse().unwrap_or_else(|_| panic!("bad number `{s}`")),
    }
}

fn fixture() -> Vec<Row> {
    FIXTURE
        .lines()
        .filter(|l| !l.starts_with('#'))
        .skip(1)
        .map(|l| {
            let f: Vec<&str> = l.splitn(4, ',').collect();
            let inputs = f[1]
                .split(';')
                .filter(|kv| !kv.is_empty())
                .map(|kv| {
                    let (k, v) = kv.split_once('=').unwrap();
                    (k.to_string(), v.to_string())
                })
                .collect();
            Row {
                group: f[0].to_string(),
                inputs,
                output: f[2].to_string(),
                values: f[3].split('|').map(parse_value).collect(),
            }
        })
        .collect()
}

impl Row {
    fn f(&self, k: &str) -> f64 {
        parse_value(&self.inputs[k])
    }
    fn b(&self, k: &str) -> bool {
        self.inputs[k] == "true"
    }
    fn s(&self, k: &str) -> &str {
        &self.inputs[k]
    }
}

fn m(v: f64) -> Length {
    Length::new::<meter>(v)
}

fn stab(code: f64) -> StabilityClass {
    StabilityClass::from_code(code as u8).unwrap()
}

// --------------------------------------------------------------------------
// Inputs shared by the groups
// --------------------------------------------------------------------------

struct Inputs {
    inhalation: InhalationDcfTable,
    surface: ExternalDcfTable,
    submersion: ExternalDcfTable,
    chains: ProgenyChains,
    met: BTreeMap<(i64, i64), MetClimatology>,
    t2: tranche2::Inputs2,
    runs: tranche2::DriverRuns,
}

fn raw_met_years() -> Vec<(Vec<RawMetRecord>, i64)> {
    let mut years: Vec<(String, Vec<RawMetRecord>)> = Vec::new();
    for line in MET.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        let opt = |s: &str| {
            if s.is_empty() {
                None
            } else {
                Some(s.parse::<f64>().unwrap())
            }
        };
        let rec = RawMetRecord {
            hour: f[1].parse().unwrap(),
            speed_kmph: opt(f[2]),
            direction_deg: opt(f[3]),
            stability_code: f[4].chars().next().map(met::stability_code_from_letter),
        };
        match years.last_mut() {
            Some((s, v)) if s == f[0] => v.push(rec),
            _ => years.push((f[0].to_string(), vec![rec])),
        }
    }
    // Day counts: the generator's `sheets = {"2001": 30, "2002": 25}`.
    years
        .into_iter()
        .map(|(s, v)| (v, if s == "2001" { 30 } else { 25 }))
        .collect()
}

fn inputs() -> Inputs {
    let raw = raw_met_years();
    let slices: Vec<(&[RawMetRecord], i64)> = raw.iter().map(|(v, d)| (v.as_slice(), *d)).collect();
    let mut met = BTreeMap::new();
    for (s, e) in [(0, 24), (6, 18)] {
        met.insert((s, e), MetClimatology::from_raw_years(&slices, s, e));
    }
    let mut inp = Inputs {
        inhalation: InhalationDcfTable::from_csv(INHALATION).unwrap(),
        surface: ExternalDcfTable::from_csv(SURFACE).unwrap(),
        submersion: ExternalDcfTable::from_csv(SUBMERSION).unwrap(),
        chains: ProgenyChains::from_csv(CHAINS, CHAIN_HALF_LIVES).unwrap(),
        met,
        t2: tranche2::inputs2(),
        runs: tranche2::DriverRuns::empty(),
    };
    let runs = tranche2::driver_runs(&inp, &inp.t2);
    inp.runs = runs;
    inp
}

fn absorption(s: &str) -> LungAbsorptionType {
    match s {
        "F" => LungAbsorptionType::F,
        "M" => LungAbsorptionType::M,
        "S" => LungAbsorptionType::S,
        "V" => LungAbsorptionType::V,
        "Max" => LungAbsorptionType::Max,
        _ => panic!("{s}"),
    }
}

fn decay_constant(half_life_text: &str) -> f64 {
    nuclide::upstream_decay_constant(nuclide::parse_primary_half_life(half_life_text).unwrap())
}

fn dfs(v: &[DilutionFactor]) -> Vec<f64> {
    v.iter().map(|d| d.seconds_per_cubic_meter()).collect()
}

// --------------------------------------------------------------------------
// The port, per group. `variant` lets the mutation tests swap in alternatives.
// --------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Variant {
    Faithful,
    ExactSectorWidth,
    TrueLn2,
    CorrectedCalm,
    ExactDaughterLookup,
    ShiftedAgeBracket,
    /// Second tranche: GSL's `qag` (via `petir`) in place of `dqagse`.
    PetirQag,
    /// Second tranche: exact-name gamma-line lookup instead of the substring.
    ExactGammaMatch,
    /// Second tranche: the corrected per-nuclide ingestion driver (D8-D11).
    CorrectedIngestion,
    /// Second tranche: `zeroing_ingestion` as intended (not pandas 3's no-op).
    ZeroMilkAndMeat,
    /// Second tranche: summary ingestion counted once (D20 corrected).
    SingleCountedIngestion,
}

fn port(row: &Row, inp: &Inputs, variant: Variant) -> Option<Vec<f64>> {
    let age_bracket = |a: f64| {
        let a = if variant == Variant::ShiftedAgeBracket && a == 1.0 {
            1.5
        } else {
            a
        };
        AgeBracket::from_age_years(a).unwrap()
    };
    let lambda = |text: &str| {
        let hl = nuclide::parse_primary_half_life(text).unwrap();
        if variant == Variant::TrueLn2 {
            core::f64::consts::LN_2 / hl
        } else {
            nuclide::upstream_decay_constant(hl)
        }
    };
    let v = match row.group.as_str() {
        "sigmay" => vec![dispersion::sigma_y(stab(row.f("stab")), m(row.f("x"))).get::<meter>()],
        "sigmaz" => vec![dispersion::sigma_z(stab(row.f("stab")), m(row.f("x"))).get::<meter>()],
        "height_factor" => vec![dispersion::height_correction_factor(
            stab(row.f("stab")),
            m(row.f("release_height")),
            m(row.f("measurement_height")),
        )],
        "master_single" | "master_sector" => {
            let rec = if row.b("central") {
                Receptor::GroundLevelCentreline
            } else {
                Receptor::Offset {
                    y: m(row.inputs.get("y").map_or(0.0, |s| parse_value(s))),
                    z: m(row.f("z")),
                }
            };
            let t = if row.group == "master_single" {
                dispersion::master_equation_single_plume(
                    m(row.f("sigma_y")),
                    m(row.f("sigma_z")),
                    row.f("factor"),
                    m(row.f("release_height")),
                    rec,
                )
            } else {
                let mut t = dispersion::master_equation_sector_averaged(
                    m(row.f("x")),
                    m(row.f("sigma_z")),
                    m(row.f("release_height")),
                    rec,
                );
                if variant == Variant::ExactSectorWidth {
                    t.pre_expo *= dispersion::SECTOR_WIDTH_RAD / (22.5_f64.to_radians());
                }
                t
            };
            vec![if row.output == "pre_expo" {
                t.pre_expo
            } else {
                t.expo
            }]
        }
        "dilution_no_met" => {
            let geo = PlumeGeometry {
                release_height: m(row.f("release_height")),
                measurement_height: m(row.f("measurement_height")),
                receptor: Receptor::GroundLevelCentreline,
            };
            let scaling = if row.b("scale_with_mean_speed") {
                MeanSpeedScaling::PerClass(MEAN_SPEEDS.map(Velocity::new::<meter_per_second>))
            } else {
                MeanSpeedScaling::UnitSpeed
            };
            let d = if row.s("mode") == "single_plume" {
                dispersion::dilution_single_plume_no_met(m(row.f("x")), geo, scaling)
            } else {
                dispersion::dilution_long_term_no_met(m(row.f("x")), geo, scaling)
            };
            if row.output == "internal_max" {
                vec![dispersion::upstream_internal_max_dilution_factor(&d).seconds_per_cubic_meter()]
            } else {
                dfs(&d)
            }
        }
        "tjfd" | "tjfd_missing" | "calm_factors" | "speed_means" | "dilution_met_long_term" => {
            let clim = &inp.met[&(row.f("start") as i64, row.f("end") as i64)];
            match row.group.as_str() {
                "tjfd" => {
                    let t = &clim.years[row.f("year") as usize].tjfd[row.f("stab") as usize - 1];
                    t.iter().flatten().copied().collect()
                }
                "tjfd_missing" => {
                    let t = &clim.years[row.f("year") as usize].missing_corrected
                        [row.f("stab") as usize - 1];
                    t.iter().flatten().map(|c| *c as f64).collect()
                }
                "calm_factors" => {
                    let t = &clim.years[row.f("year") as usize].missing_corrected;
                    if variant == Variant::CorrectedCalm {
                        met::calm_correction_factors_lowest_speed_class(t).to_vec()
                    } else {
                        met::calm_correction_factors(t).to_vec()
                    }
                }
                "speed_means" => met::speed_distribution(&clim.all_records(), 0.90)
                    .1
                    .to_vec(),
                _ => {
                    let geo = PlumeGeometry {
                        release_height: m(MET_RELEASE_HEIGHT_M),
                        measurement_height: m(10.0),
                        receptor: Receptor::GroundLevelCentreline,
                    };
                    let calm = match (row.b("calm_correction"), variant) {
                        (false, _) => CalmCorrection::Off,
                        (true, Variant::CorrectedCalm) => CalmCorrection::LowestSpeedClassTotal,
                        (true, _) => CalmCorrection::Upstream,
                    };
                    dfs(&dispersion::dilution_long_term_with_met(
                        m(row.f("x")),
                        geo,
                        clim,
                        calm,
                    ))
                }
            }
        }
        "half_life" => {
            let hl = nuclide::parse_primary_half_life(row.s("text")).unwrap();
            if row.output == "half_life_s" {
                vec![hl]
            } else {
                vec![lambda(row.s("text"))]
            }
        }
        "dcf_inhalation" => NUCLIDES
            .iter()
            .map(|(n, _, _)| {
                inp.inhalation
                    .lookup(n, absorption(row.s("type")), age_bracket(row.f("age")))
            })
            .collect(),
        "dcf_surface" | "dcf_submersion" => {
            let table = if row.group == "dcf_surface" {
                &inp.surface
            } else {
                &inp.submersion
            };
            let progeny = if row.b("progeny") {
                ProgenyCorrection::IncludeShortLived {
                    ignore_half_life_s: row.f("ignore_half_life"),
                }
            } else {
                ProgenyCorrection::ParentOnly
            };
            NUCLIDES
                .iter()
                .map(|(n, _, _)| {
                    let p = if variant == Variant::ExactDaughterLookup {
                        exact_daughter_dcf(
                            table,
                            &inp.chains,
                            n,
                            age_bracket(row.f("age")),
                            progeny,
                        )
                    } else {
                        external_dcf(table, &inp.chains, n, age_bracket(row.f("age")), progeny)
                    };
                    if row.output == "corrected" {
                        p.corrected
                    } else {
                        p.uncorrected
                    }
                })
                .collect()
        }
        "deposition_velocity" => NUCLIDES
            .iter()
            .map(|(_, e, _)| dose::deposition_velocity_m_per_s(e))
            .collect(),
        "effective_lambda" => {
            let w = if row.b("weathering") {
                Weathering::SoilLossRates
            } else {
                Weathering::Off
            };
            NUCLIDES
                .iter()
                .map(|(_, e, hl)| {
                    dose::effective_buildup_time_s(lambda(hl), e, w, row.f("exposure_period_y"))
                })
                .collect()
        }
        "dose_inhalation" | "dose_ground_shine" | "dose_submersion" => {
            let chi = DilutionFactor::new(row.f("chi_over_q"));
            let age = row.f("age");
            let progeny = if row.b("progeny") {
                ProgenyCorrection::IncludeShortLived {
                    ignore_half_life_s: 1800.0,
                }
            } else {
                ProgenyCorrection::ParentOnly
            };
            let w = if row.b("weathering") {
                Weathering::SoilLossRates
            } else {
                Weathering::Off
            };
            NUCLIDES
                .iter()
                .enumerate()
                .map(|(i, (n, e, hl))| {
                    let q = Radioactivity::new::<becquerel>(1.0e9 * (1.0 + 0.5 * i as f64));
                    let release = if row.s("mode") == "single_plume" {
                        Release::Instantaneous(q)
                    } else {
                        Release::AnnualDischarge(q)
                    };
                    let bracket = age_bracket(age);
                    match row.group.as_str() {
                        "dose_inhalation" => {
                            let dcf = inp.inhalation.lookup(n, absorption(row.s("type")), bracket);
                            dose::inhalation_dose(chi, release, dcf, age)
                                .unwrap()
                                .millisieverts()
                        }
                        "dose_ground_shine" => {
                            let dcf = external_dcf(&inp.surface, &inp.chains, n, bracket, progeny)
                                .selected(progeny);
                            let t = dose::effective_buildup_time_s(lambda(hl), e, w, 30.0);
                            dose::ground_shine_dose(
                                chi,
                                release,
                                dose::deposition_velocity_m_per_s(e),
                                t,
                                dcf,
                            )
                            .millisieverts()
                        }
                        _ => {
                            let dcf =
                                external_dcf(&inp.submersion, &inp.chains, n, bracket, progeny)
                                    .selected(progeny);
                            dose::submersion_dose(chi, release, dcf).millisieverts()
                        }
                    }
                })
                .collect()
        }
        other => {
            return Some(
                tranche2::port(row, inp, &inp.t2, &inp.runs, variant)
                    .unwrap_or_else(|| panic!("unhandled fixture group `{other}`")),
            )
        }
    };
    Some(v)
}

/// A plausible alternative to upstream's substring daughter lookup: exact name
/// match. Used only by the mutation test.
fn exact_daughter_dcf(
    table: &ExternalDcfTable,
    chains: &ProgenyChains,
    nuclide: &str,
    age: AgeBracket,
    progeny: ProgenyCorrection,
) -> buangkok::pydoseia::dcf::ExternalDcfPair {
    let mut p = external_dcf(table, chains, nuclide, age, ProgenyCorrection::ParentOnly);
    if let ProgenyCorrection::IncludeShortLived { ignore_half_life_s } = progeny {
        for (d, y) in chains.short_lived_daughters(nuclide, ignore_half_life_s) {
            p.corrected += table.lookup_exact(&d, age) * y;
        }
    }
    p
}

// --------------------------------------------------------------------------
// Comparison
// --------------------------------------------------------------------------

fn all_tolerances() -> BTreeMap<&'static str, f64> {
    TOLERANCES
        .iter()
        .chain(tranche2::TOLERANCES)
        .map(|(g, t, _)| (*g, *t))
        .collect()
}

/// Relative deviation; `None` if the pair is a hard mismatch (NaN vs number,
/// zero vs non-zero, differing infinities).
fn rel_dev(actual: f64, expected: f64) -> Option<f64> {
    if expected.is_nan() || actual.is_nan() {
        return (expected.is_nan() && actual.is_nan()).then_some(0.0);
    }
    if expected.is_infinite() || actual.is_infinite() {
        return (expected == actual).then_some(0.0);
    }
    if expected == 0.0 || actual == 0.0 {
        return (expected == actual).then_some(0.0);
    }
    Some(((actual - expected) / expected).abs())
}

/// Per group: (cases, values, max relative deviation, failures).
fn compare(variant: Variant) -> BTreeMap<String, (usize, usize, f64, usize)> {
    let inp = inputs();
    let tol = all_tolerances();
    let mut out: BTreeMap<String, (usize, usize, f64, usize)> = BTreeMap::new();
    for row in fixture() {
        let t = *tol
            .get(row.group.as_str())
            .unwrap_or_else(|| panic!("no tolerance for `{}`", row.group));
        let actual = port(&row, &inp, variant).unwrap();
        let e = out.entry(row.group.clone()).or_insert((0, 0, 0.0, 0));
        e.0 += 1;
        if actual.len() != row.values.len() {
            // A different shape (or a raise where upstream returned, or the
            // reverse) is a failure of the whole row.
            e.1 += row.values.len();
            e.2 = f64::INFINITY;
            e.3 += 1;
            if variant == Variant::Faithful {
                eprintln!(
                    "{} {:?}: length {} != {}",
                    row.group,
                    row.inputs,
                    actual.len(),
                    row.values.len()
                );
            }
            continue;
        }
        for (a, x) in actual.iter().zip(&row.values) {
            e.1 += 1;
            match rel_dev(*a, *x) {
                Some(d) => {
                    e.2 = e.2.max(d);
                    if d > t {
                        e.3 += 1;
                    }
                }
                None => {
                    e.2 = f64::INFINITY;
                    e.3 += 1;
                }
            }
        }
    }
    out
}

#[test]
fn every_fixture_value_is_reproduced_within_its_group_tolerance() {
    let r = compare(Variant::Faithful);
    let tol = all_tolerances();
    println!("| group | cases | values | max rel dev | tolerance |");
    println!("|---|---:|---:|---:|---:|");
    let mut failed = Vec::new();
    for (g, (cases, values, max, fails)) in &r {
        println!(
            "| `{g}` | {cases} | {values} | {max:.2e} | {:.0e} |",
            tol[g.as_str()]
        );
        if *fails > 0 {
            failed.push(format!(
                "{g}: {fails} of {values} values out of tolerance (max {max:e})"
            ));
        }
    }
    let total: usize = r.values().map(|v| v.1).sum();
    println!("total values compared: {total}");
    assert_eq!(
        r.len(),
        TOLERANCES.len() + tranche2::TOLERANCES.len(),
        "every group in the fixture is exercised"
    );
    assert!(failed.is_empty(), "{failed:#?}");
}

// --------------------------------------------------------------------------
// Mutation tests: the suite must be able to fail.
// --------------------------------------------------------------------------

fn failures_in(variant: Variant, group: &str) -> usize {
    compare(variant).get(group).map_or(0, |e| e.3)
}

#[test]
fn mutation_exact_sector_width_is_rejected() {
    assert!(failures_in(Variant::ExactSectorWidth, "master_sector") > 0);
}

#[test]
fn mutation_true_ln2_is_rejected() {
    assert!(failures_in(Variant::TrueLn2, "half_life") > 0);
    assert!(failures_in(Variant::TrueLn2, "effective_lambda") > 0);
}

#[test]
fn mutation_corrected_calm_correction_is_rejected() {
    // The D1 correction is a real divergence from upstream: the fixture must
    // tell it apart, both in the factors and in the dilution factors.
    assert!(failures_in(Variant::CorrectedCalm, "calm_factors") > 0);
    assert!(failures_in(Variant::CorrectedCalm, "dilution_met_long_term") > 0);
}

#[test]
fn mutation_exact_daughter_lookup_is_rejected() {
    // Upstream's substring match (D4) is visible in the fixture through the
    // SYN-9 / SYN-9m pair at the 1e6 s half-life threshold.
    assert!(failures_in(Variant::ExactDaughterLookup, "dcf_surface") > 0);
}

#[test]
fn mutation_age_bracket_shift_is_rejected() {
    assert!(failures_in(Variant::ShiftedAgeBracket, "dcf_inhalation") > 0);
    assert!(failures_in(Variant::ShiftedAgeBracket, "dose_inhalation") > 0);
}

#[test]
fn mutation_petir_qag_instead_of_dqagse_is_rejected() {
    // The checked-and-rejected alternative: GSL's qag (no extrapolation).
    assert!(failures_in(Variant::PetirQag, "quadpack") > 0);
}

#[test]
fn mutation_exact_gamma_line_lookup_is_rejected() {
    assert!(failures_in(Variant::ExactGammaMatch, "gamma_lines") > 0);
    assert!(failures_in(Variant::ExactGammaMatch, "plume_shine_single") > 0);
    assert!(failures_in(Variant::ExactGammaMatch, "plume_shine_long_term") > 0);
    assert!(failures_in(Variant::ExactGammaMatch, "plume_shine_met") > 0);
}

#[test]
fn mutation_corrected_ingestion_driver_is_rejected() {
    // D8-D11 are real divergences: the fixture must tell the corrected
    // per-nuclide driver apart from upstream's.
    assert!(failures_in(Variant::CorrectedIngestion, "ingestion") > 0);
}

#[test]
fn mutation_intended_zeroing_is_rejected() {
    assert!(failures_in(Variant::ZeroMilkAndMeat, "zeroing") > 0);
}

#[test]
fn mutation_single_counted_summary_ingestion_is_rejected() {
    assert!(failures_in(Variant::SingleCountedIngestion, "driver_summary") > 0);
}

/// The fixture exercises the upstream NaN path (a nuclide with no row of the
/// requested absorption type), so a port that returned 0 there would fail.
#[test]
fn the_fixture_contains_nan_cases() {
    assert!(fixture()
        .iter()
        .any(|r| r.group == "dcf_inhalation" && r.values.iter().any(|v| v.is_nan())));
}

/// Keeps the half-life helper used by the doses honest against the fixture's
/// own decay constants.
#[test]
fn decay_constants_match_the_fixture_half_life_rows() {
    for row in fixture()
        .iter()
        .filter(|r| r.group == "half_life" && r.output == "decay_constant")
    {
        assert_eq!(
            decay_constant(row.s("text")),
            row.values[0],
            "{}",
            row.s("text")
        );
    }
}

/// Upstream's own regression table (`MetFunc.test_single_plume_glc_hukkoo`,
/// called from `output_to_txt`): the single-plume ground-level dilution factor
/// for a 100 m release measured at 100 m, unit wind speed, six classes at nine
/// distances. Upstream attributes it to Hukkoo and Bapat (p. 98); the
/// 17-digit values look computed rather than transcribed, so this is a
/// regression check against upstream, not a validation. Upstream compares with
/// `np.allclose` (atol 1e-8, which every value here passes trivially); the port
/// reproduces every value **exactly** (measured 2026-09-28). Upstream never
/// reaches the check: the guard reads the config key
/// `'like_to_scale_with_mean_speed:'` (trailing colon) and raises `KeyError`
/// whenever the other conditions hold (defect D23).
#[test]
fn upstream_hukkoo_self_test_table_is_reproduced_exactly() {
    const T: [[f64; 9]; 6] = [
        [
            2.2840147833303766e-14,
            5.590981589039797e-07,
            1.5015465916440028e-05,
            1.8534496171567465e-05,
            9.310282356034462e-06,
            3.6973162422584492e-06,
            9.083901472613404e-07,
            4.64985088543347e-07,
            5.809360379016258e-08,
        ],
        [
            5.841151371898279e-22,
            1.881938226103609e-09,
            8.545950204498517e-07,
            1.2296547461666177e-05,
            1.689222515044389e-05,
            1.3596757373581253e-05,
            6.948090247268007e-06,
            4.718541203801888e-06,
            1.2717701082737338e-06,
        ],
        [
            7.956707894464337e-42,
            1.091680781377853e-14,
            2.634808537371747e-09,
            1.504493947251421e-06,
            7.156250389638748e-06,
            1.2763487559346726e-05,
            1.1755542375765323e-05,
            9.483132536764242e-06,
            3.5384718119021803e-06,
        ],
        [
            1.9691262342948032e-107,
            1.7554619140561087e-32,
            2.322138749483794e-18,
            1.645624503785968e-10,
            3.9826750086653584e-08,
            8.73472028784478e-07,
            4.620161828899931e-06,
            6.349041184912812e-06,
            6.810849638824183e-06,
        ],
        [
            6.22620696059002e-181,
            9.192930646978123e-57,
            1.5133024136726587e-31,
            1.0210504603823029e-16,
            6.745515294647785e-12,
            5.643704797281375e-09,
            4.787367464209608e-07,
            1.3403801979441503e-06,
            4.725940039623769e-06,
        ],
        [
            0.0,
            2.5748671022239412e-139,
            1.0642330561676144e-74,
            9.189248563624478e-36,
            5.821816484706907e-23,
            3.877805752304435e-15,
            5.70587584195314e-10,
            8.896730306387878e-09,
            5.399181471824446e-07,
        ],
    ];
    const D: [f64; 9] = [
        100.0, 200.0, 300.0, 500.0, 700.0, 1000.0, 1600.0, 2000.0, 4000.0,
    ];
    let geometry = PlumeGeometry {
        release_height: m(100.0),
        measurement_height: m(100.0),
        receptor: Receptor::GroundLevelCentreline,
    };
    for (j, x) in D.iter().enumerate() {
        let v =
            dispersion::dilution_single_plume_no_met(m(*x), geometry, MeanSpeedScaling::UnitSpeed);
        for (s, row) in T.iter().enumerate() {
            assert_eq!(
                v[s].seconds_per_cubic_meter(),
                row[j],
                "x = {x} m, class {s}"
            );
        }
    }
}
