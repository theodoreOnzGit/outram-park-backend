//! Differential comparison against a run of ONIX 7328dc6 (gh:#1404, epic #1336).
//!
//! Methodology, numbers and the defects found are in
//! `verification_and_validation/onix_differential/` (`README.md`, `PREDICTIONS.md`
//! written before the runs, `RESULTS.md`). The reference CSVs there were produced
//! by `onix_driver.py` running the upstream Python (`onix.salameche.cram.CRAM16`,
//! `mat_builder.get_xs_mat`, `get_decay_mat`, and the arithmetic of
//! `burn.burn_microstep`) with identical inputs; this file recomputes every case
//! with this crate and counts bit-identical values and ulp distances.
//!
//! Set `ONIX_DIFF_PRINT=1` and pass `--nocapture` to print the tables.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use outram_park_fork_onix::{
    cram16, BurnupMatrix, DecayData, DecayMode, DepletionSystem,
    FissionYields, Nuclide, ReactionChannel, ReactionRates,
};

const CRAM_CASES: &str =
    include_str!("../verification_and_validation/onix_differential/cram_cases.csv");
const CASE2_INPUTS: &str =
    include_str!("../verification_and_validation/onix_differential/case2_inputs.csv");
const CASE2_MATRIX: &str =
    include_str!("../verification_and_validation/onix_differential/case2_matrix.csv");
const CASE3_BURN: &str =
    include_str!("../verification_and_validation/onix_differential/case3_burn.csv");

fn print_enabled() -> bool {
    std::env::var("ONIX_DIFF_PRINT").is_ok()
}

fn p(v: &str) -> f64 {
    v.parse::<f64>().expect("reference value parses as f64")
}

/// Distance in units in the last place; `u64::MAX` across a sign change.
fn ulps(a: f64, b: f64) -> u64 {
    if a == b {
        return 0;
    }
    if a.is_sign_negative() != b.is_sign_negative() {
        return u64::MAX;
    }
    let (x, y) = (a.to_bits() as i64, b.to_bits() as i64);
    (x - y).unsigned_abs()
}

// ------------------------------------------------------------------ case 1

/// One CRAM16 case: matrix, initial vector, dt, and both upstream outputs.
struct CramCase {
    name: String,
    dt: f64,
    a: BurnupMatrix,
    n0: Vec<f64>,
    out_ld: Vec<f64>,
    out_d: Vec<f64>,
    out_r: Vec<f64>,
}

fn load_cram_cases() -> Vec<CramCase> {
    let mut mats: BTreeMap<String, Vec<(usize, usize, f64)>> = BTreeMap::new();
    let mut n0s: BTreeMap<String, BTreeMap<usize, f64>> = BTreeMap::new();
    let mut outs: BTreeMap<(String, u64), [BTreeMap<usize, f64>; 3]> = BTreeMap::new();
    for line in CRAM_CASES.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        let name = f[1].to_string();
        match f[0] {
            "A" => mats
                .entry(name)
                .or_default()
                .push((f[3].parse().unwrap(), f[4].parse().unwrap(), p(f[5]))),
            "N0" => {
                n0s.entry(name)
                    .or_default()
                    .insert(f[3].parse().unwrap(), p(f[5]));
            }
            "OUT_LD" | "OUT_D" | "OUT_R" => {
                let dt = p(f[2]);
                let e = outs.entry((name, dt.to_bits())).or_default();
                let i: usize = f[3].parse().unwrap();
                let slot = match f[0] {
                    "OUT_LD" => 0,
                    "OUT_D" => 1,
                    _ => 2,
                };
                e[slot].insert(i, p(f[5]));
            }
            other => panic!("unknown row kind {other}"),
        }
    }
    let mut cases = Vec::new();
    for ((name, dt_bits), [ld, dd, rr]) in outs {
        let n0m = &n0s[&name];
        let n = n0m.len();
        let mut a = BurnupMatrix::zeros(n);
        for &(i, j, v) in &mats[&name] {
            a.set(i, j, v);
        }
        cases.push(CramCase {
            name,
            dt: f64::from_bits(dt_bits),
            a,
            n0: (0..n).map(|i| n0m[&i]).collect(),
            out_ld: (0..n).map(|i| ld[&i]).collect(),
            out_d: (0..n).map(|i| dd[&i]).collect(),
            out_r: (0..n).map(|i| rr[&i]).collect(),
        });
    }
    cases
}

/// Comparison summary of one vector against a reference.
#[derive(Debug, Clone, PartialEq)]
struct VecStat {
    entries: usize,
    bit_identical: usize,
    max_ulps: u64,
    /// `max |r - o| / max |o|`
    normwise: f64,
    /// worst `|r - o| / |o|` over entries with `|o| >= 1e-12 * max |o|`
    worst_significant_rel: f64,
}

fn vec_stat(r: &[f64], o: &[f64]) -> VecStat {
    let omax = o.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    let mut s = VecStat {
        entries: r.len(),
        bit_identical: 0,
        max_ulps: 0,
        normwise: 0.0,
        worst_significant_rel: 0.0,
    };
    for (&a, &b) in r.iter().zip(o) {
        if a.to_bits() == b.to_bits() {
            s.bit_identical += 1;
        }
        s.max_ulps = s.max_ulps.max(ulps(a, b));
        s.normwise = s.normwise.max((a - b).abs() / omax);
        if b.abs() >= 1e-12 * omax {
            s.worst_significant_rel = s.worst_significant_rel.max(((a - b) / b).abs());
        }
    }
    s
}

/// Per `(case, dt)`: this crate vs the long-double upstream and vs the all-double one.
fn case1_stats() -> Vec<(String, f64, VecStat, VecStat, VecStat)> {
    load_cram_cases()
        .into_iter()
        .map(|c| {
            let r = cram16(&c.a, c.dt, &c.n0).expect("cram16");
            let vs_ld = vec_stat(&r, &c.out_ld);
            let vs_d = vec_stat(&r, &c.out_d);
            let vs_r = vec_stat(&r, &c.out_r);
            (c.name, c.dt, vs_ld, vs_d, vs_r)
        })
        .collect()
}

#[test]
fn case1_cram16_vs_onix_cram16() {
    let stats = case1_stats();
    assert_eq!(stats.len(), 12, "4 matrices x 3 time steps");
    let (mut be_ld, mut be_d, mut be_r, mut tot) = (0, 0, 0, 0);
    for (name, dt, ld, d, r) in &stats {
        if print_enabled() {
            println!(
                "case1 {name:7} dt={dt:>9}: vs_ld bit {}/{} norm {:.2e} sigrel {:.2e} maxulp {} | vs_d bit {}/{} norm {:.2e} sigrel {:.2e} maxulp {} | vs_replica bit {}/{} norm {:.2e}",
                ld.bit_identical, ld.entries, ld.normwise, ld.worst_significant_rel, ld.max_ulps,
                d.bit_identical, d.entries, d.normwise, d.worst_significant_rel, d.max_ulps,
                r.bit_identical, r.entries, r.normwise
            );
        }
        be_ld += ld.bit_identical;
        be_d += d.bit_identical;
        be_r += r.bit_identical;
        tot += ld.entries;
    }
    if print_enabled() {
        println!("case1 total entries {tot}, bit-identical vs ld {be_ld}, vs d {be_d}, vs replica {be_r}");
    }
    // Pins (measured 2026-11-13, numpy 2.4.6 / OpenBLAS LAPACK, x86_64 Linux).
    assert_eq!(tot, 96);
    // Against numpy's LAPACK `zgesv` solve: few entries are bit-equal, and the
    // long-double tail of the upstream function changes nothing.
    assert_eq!((be_ld, be_d), (6, 6));
    // Against a Python replica of this crate's elimination (textbook complex
    // division, same pivoting and operation order) inside the upstream source:
    // every entry is bit-identical, which places the whole difference in the
    // pole solves (LAPACK vs the textbook elimination).
    assert_eq!(be_r, 96, "replica must reproduce the crate bit for bit");
    for (name, dt, ld, d, _) in &stats {
        assert_eq!(ld.bit_identical, d.bit_identical);
        if name == "scalar" && *dt > 1.0 {
            // exp(-86.4) .. exp(-2592) underflow the answer: both sides return
            // CRAM's absolute rounding noise (~5e-16, alpha_0*n0 = 5.3e-16).
            continue;
        }
        assert!(ld.normwise < 5e-14, "{name} dt={dt}: normwise {}", ld.normwise);
        assert!(
            ld.worst_significant_rel < 1e-10,
            "{name} dt={dt}: significant-entry rel {}",
            ld.worst_significant_rel
        );
    }
}

#[test]
fn case1_scalar_underflow_cases_return_only_rounding_noise() {
    for c in load_cram_cases().into_iter().filter(|c| c.name == "scalar" && c.dt > 1.0) {
        let r = cram16(&c.a, c.dt, &c.n0).unwrap();
        assert!((r[0] - c.out_ld[0]).abs() < 2e-15, "dt={}", c.dt);
        assert!(r[0].abs() < 1e-15 && c.out_ld[0].abs() < 1e-15);
    }
}

// ------------------------------------------------------------------ case 2

struct UpstreamNuclide {
    zamid: u64,
    total: Option<f64>,
    decay: Vec<(String, f64)>,
    xs: Vec<(String, f64)>,
}

struct Inputs {
    flux: f64,
    nuclides: Vec<UpstreamNuclide>,
    /// (product zamid, parent zamid, percent)
    yields: Vec<(u64, u64, f64)>,
}

fn load_inputs(text: &str) -> Inputs {
    let mut flux = 0.0;
    let mut nuclides: Vec<UpstreamNuclide> = Vec::new();
    let mut yields = Vec::new();
    for line in text.lines().skip(1) {
        let g: Vec<&str> = line.split(',').collect();
        // reaction keys such as "(n,gamma)" contain commas: the key is everything
        // between the zamid and the final value.
        let key = g[2..g.len() - 1].join(",");
        let f: Vec<&str> = vec![g[0], g[1], key.as_str(), g[g.len() - 1]];
        match f[0] {
            "F" => flux = p(f[3]),
            "N" => nuclides.push(UpstreamNuclide {
                zamid: f[1].parse().unwrap(),
                total: None,
                decay: Vec::new(),
                xs: Vec::new(),
            }),
            "D" => {
                let n = nuclides.last_mut().unwrap();
                if f[2] == "total" {
                    n.total = Some(p(f[3]));
                } else {
                    n.decay.push((f[2].to_string(), p(f[3])));
                }
            }
            "X" => nuclides
                .last_mut()
                .unwrap()
                .xs
                .push((f[2].to_string(), p(f[3]))),
            "Y" => yields.push((f[1].parse().unwrap(), f[2].parse().unwrap(), p(f[3]))),
            other => panic!("unknown input kind {other}"),
        }
    }
    Inputs {
        flux,
        nuclides,
        yields,
    }
}

/// ONIX decay-library key to this crate's decay mode, `None` when not expressible.
fn decay_mode(key: &str) -> Option<DecayMode> {
    match key {
        "betaneg" => Some(DecayMode::BetaMinus),
        "betapos" => Some(DecayMode::BetaPlus),
        "alpha" => Some(DecayMode::Alpha),
        "gamma" => Some(DecayMode::IsomericTransition),
        "betanegX" => Some(DecayMode::BetaMinusToIsomer),
        "betaposX" => Some(DecayMode::BetaPlusToIsomer),
        "alphaX" => Some(DecayMode::AlphaToIsomer),
        "neutronX" => Some(DecayMode::NeutronEmissionToIsomer),
        "protonX" => Some(DecayMode::ProtonEmissionToIsomer),
        _ => None,
    }
}

/// ONIX cross-section-library key to this crate's channel, `None` when not expressible.
fn reaction_channel(key: &str) -> Option<ReactionChannel> {
    match key {
        "(n,gamma)" => Some(ReactionChannel::NGamma),
        "(n,2n)" => Some(ReactionChannel::N2n),
        "(n,3n)" => Some(ReactionChannel::N3n),
        "(n,p)" => Some(ReactionChannel::Np),
        "(n,a)" => Some(ReactionChannel::NAlpha),
        "(n,t)" => Some(ReactionChannel::NT),
        "(n,gamma)X" => Some(ReactionChannel::NGammaToIsomer),
        "(n,2n)X" => Some(ReactionChannel::N2nToIsomer),
        "fission" => Some(ReactionChannel::Fission),
        _ => None,
    }
}

/// Builds this crate's system from the exported ONIX inputs. Returns the system,
/// the zamids in index order and the number of ONIX channels the crate cannot express.
fn build_system(inp: &Inputs) -> (DepletionSystem, Vec<u64>, usize) {
    let mut sys = DepletionSystem::new();
    let mut order = Vec::new();
    let mut skipped = 0;
    let mut by_parent: HashMap<u64, Vec<(Nuclide, f64)>> = HashMap::new();
    for &(prod, parent, pct) in &inp.yields {
        by_parent
            .entry(parent)
            .or_default()
            .push((Nuclide::from_zamid(prod), pct));
    }
    for n in &inp.nuclides {
        let decay = match n.total {
            None => DecayData::stable(),
            Some(total) => {
                let mut branches = Vec::new();
                for (k, frac) in &n.decay {
                    match decay_mode(k) {
                        Some(mode) => branches.push((mode, *frac)),
                        None => skipped += 1,
                    }
                }
                DecayData {
                    lambda_total: total,
                    branches,
                }
            }
        };
        let mut channels = Vec::new();
        for (k, sigma) in &n.xs {
            match reaction_channel(k) {
                Some(ch) => channels.push((ch, *sigma)),
                None => skipped += 1,
            }
        }
        let rates = ReactionRates::from_xs_flux(&channels, inp.flux);
        let fy = match by_parent.get(&n.zamid) {
            Some(list) => FissionYields::from_percent(list),
            None => FissionYields::empty(),
        };
        sys.add_nuclide(Nuclide::from_zamid(n.zamid), decay, rates, fy)
            .expect("unique nuclide");
        order.push(n.zamid);
    }
    (sys, order, skipped)
}

fn load_matrix(text: &str) -> BTreeMap<(u64, u64), f64> {
    let mut m = BTreeMap::new();
    for line in text.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        if f[0] == "A" {
            m.insert((f[1].parse().unwrap(), f[2].parse().unwrap()), p(f[3]));
        }
    }
    m
}

fn is_isomer(z: u64) -> bool {
    z % 10 != 0
}

/// Entry-by-entry result of the burnup-matrix comparison.
#[derive(Debug, Default, Clone, PartialEq)]
struct MatStat {
    onix_entries: usize,
    port_entries: usize,
    bit_identical: usize,
    within_4_ulp: usize,
    worse: usize,
    only_in_onix: usize,
    only_in_port: usize,
    worst_rel_among_both: f64,
    /// subset counts restricted to entries touching an isomer (zamid ending 1)
    iso_onix: usize,
    iso_bit_identical: usize,
    iso_only_in_onix: usize,
    iso_only_in_port: usize,
    iso_worse: usize,
    skipped_channels: usize,
}

fn matrix_stat(inp: &Inputs, expected: &BTreeMap<(u64, u64), f64>) -> MatStat {
    let (sys, order, skipped) = build_system(inp);
    let a = sys.build_matrix();
    let n = order.len();
    let mut port: BTreeMap<(u64, u64), f64> = BTreeMap::new();
    for i in 0..n {
        for j in 0..n {
            let v = a.get(i, j);
            if v != 0.0 {
                port.insert((order[i], order[j]), v);
            }
        }
    }
    let keys: BTreeSet<_> = expected.keys().chain(port.keys()).copied().collect();
    let mut s = MatStat {
        onix_entries: expected.len(),
        port_entries: port.len(),
        skipped_channels: skipped,
        ..Default::default()
    };
    for k in keys {
        let iso = is_isomer(k.0) || is_isomer(k.1);
        if iso && expected.contains_key(&k) {
            s.iso_onix += 1;
        }
        if print_enabled() && expected.get(&k).map(|v| v.to_bits()) != port.get(&k).map(|v| v.to_bits()) {
            println!("case2 diff {:?}: onix {:?} port {:?}", k, expected.get(&k), port.get(&k));
        }
        match (expected.get(&k), port.get(&k)) {
            (Some(&o), Some(&r)) => {
                let u = ulps(r, o);
                if u == 0 {
                    s.bit_identical += 1;
                    if iso {
                        s.iso_bit_identical += 1;
                    }
                } else if u <= 4 {
                    s.within_4_ulp += 1;
                } else {
                    s.worse += 1;
                    if iso {
                        s.iso_worse += 1;
                    }
                }
                s.worst_rel_among_both = s.worst_rel_among_both.max(((r - o) / o).abs());
            }
            (Some(_), None) => {
                s.only_in_onix += 1;
                if iso {
                    s.iso_only_in_onix += 1;
                }
            }
            (None, Some(_)) => {
                s.only_in_port += 1;
                if iso {
                    s.iso_only_in_port += 1;
                }
            }
            (None, None) => unreachable!(),
        }
    }
    s
}

#[test]
fn case2_burnup_matrix_vs_onix_mat_builder() {
    let inp = load_inputs(CASE2_INPUTS);
    let expected = load_matrix(CASE2_MATRIX);
    let s = matrix_stat(&inp, &expected);
    if print_enabled() {
        println!("case2 {s:#?}");
    }
    assert_eq!(s.onix_entries, expected.len());
    // Pins (2026-11-13). Before the isomer fix (gh:#1404): port_entries 91,
    // bit_identical 78, within_4_ulp 9, worse 4, only_in_onix 8, only_in_port 0,
    // skipped_channels 6, worst_rel 148.
    assert_eq!(s.skipped_channels, 0, "every ONIX channel is expressible");
    assert_eq!((s.onix_entries, s.port_entries), (99, 99));
    assert_eq!((s.only_in_onix, s.only_in_port), (0, 0));
    assert_eq!((s.bit_identical, s.within_4_ulp, s.worse), (88, 11, 0));
    assert_eq!((s.iso_onix, s.iso_bit_identical), (17, 17));
    // The 11 non-identical entries are all operation-order ulps: diagonals
    // (ONIX sums sigma then scales; this crate sums scaled rates) and fission
    // yields ((sigma*pct*1e-2)*1e-24*phi against (sigma*1e-24*phi)*(pct*1e-2)).
    assert!(s.worst_rel_among_both < 3e-16, "{}", s.worst_rel_among_both);
}

// ------------------------------------------------------------------ case 3

/// Per step: stats of this crate's inventory vs ONIX's.
fn case3_stats() -> Vec<VecStat> {
    let inp = load_inputs(CASE2_INPUTS);
    let (sys, order, _) = build_system(&inp);
    let mut expected: BTreeMap<usize, BTreeMap<u64, f64>> = BTreeMap::new();
    for line in CASE3_BURN.lines().skip(1) {
        let f: Vec<&str> = line.split(',').collect();
        expected
            .entry(f[0].parse().unwrap())
            .or_default()
            .insert(f[1].parse().unwrap(), p(f[2]));
    }
    let n0: Vec<f64> = order.iter().map(|z| expected[&0][z]).collect();
    let dt = 30.0 * 86400.0;
    let history = sys
        .deplete_steps_floored(&n0, &[dt; 10], 1e-24)
        .expect("deplete");
    let mut out = Vec::new();
    for (k, n) in history.iter().enumerate() {
        let o: Vec<f64> = order.iter().map(|z| expected[&(k + 1)][z]).collect();
        out.push(vec_stat(n, &o));
    }
    out
}

#[test]
fn case3_ten_step_burn_vs_onix() {
    let stats = case3_stats();
    assert_eq!(stats.len(), 10);
    if print_enabled() {
        for (i, s) in stats.iter().enumerate() {
            println!("case3 step {}: {s:?}", i + 1);
        }
    }
    // Pins (2026-11-13). Before the isomer fix the same run gave normwise 3.3e-7
    // (step 1) growing to 2.4e-6 (step 10), with significant-entry relative error 1.0.
    for (i, s) in stats.iter().enumerate() {
        assert!(s.normwise < 5e-14, "step {}: normwise {}", i + 1, s.normwise);
        assert!(
            s.worst_significant_rel < 2e-13,
            "step {}: significant rel {}",
            i + 1,
            s.worst_significant_rel
        );
    }
}

// ------------------------------------------------------------ real libraries

/// One-off run on ONIX's own bundled default libraries (`decay_lib_reduced`,
/// `xs_lib`, `fy_lib_reduced`; ENDF/B-VIII-derived, MIT repository), over ONIX's
/// `reduced_nucl_set`. The data are NOT committed (this crate bundles no nuclear
/// data, `AGENTS.md`): run `onix_driver.py --real` and point `ONIX_REAL_DIR` at its
/// output directory. Recorded in `RESULTS.md`; ignored by default.
#[test]
#[ignore = "needs the uncommitted ONIX library dump: set ONIX_REAL_DIR"]
fn real_libraries_burnup_matrix_vs_onix() {
    let dir = std::env::var("ONIX_REAL_DIR").expect("ONIX_REAL_DIR");
    let inputs = std::fs::read_to_string(format!("{dir}/real_inputs.csv")).unwrap();
    let matrix = std::fs::read_to_string(format!("{dir}/real_matrix.csv")).unwrap();
    let s = matrix_stat(&load_inputs(&inputs), &load_matrix(&matrix));
    println!("real {s:#?}");
}
