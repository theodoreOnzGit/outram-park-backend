#![cfg(feature = "std")]
//! **OpenMC's transfer-rate regression cases, on OpenMC's own rates**
//! (gh:#1626 arm 3).
//!
//! Reference: the twelve committed result files of OpenMC `e4febe9`'s
//! `tests/regression_tests/deplete_with_transfer_rates` (fuel `f` and water
//! `w`, `chain_simple.xml`, one 1-day Predictor step at 174 W or at 0 W;
//! Xe removed from or fed to `f`, moved to `w`, Gd-157 redox buffer, a
//! 0.1 g/s natural-U source, and U-235 moved to `w`). Dumped by
//! `verification_and_validation/spectrum_depletion/openmc_inputs/
//! dump_transfer_regression.py` to `openmc_outputs/transfer_regression.tsv`.
//!
//! Each file holds OpenMC's beginning-of-step atoms and normalised per-atom
//! reaction rates, from its own transport, and its end-of-step atoms. A
//! Predictor step is one matrix exponential on those rates, so the port is
//! fed OpenMC's rates (the chain's `build_matrix`), its transfer rates,
//! redox and sources (`depletion::external_rates`), and CRAM-48, and asked
//! for OpenMC's end-of-step atoms. This is the "OpenMC's own rates fed to
//! our integrator" half of #1626, for the transfer-rate case; no transport,
//! no statistics.
//!
//! **Prediction, written before the run:** the cases with no external
//! source agree within 1e-10 relative on every entry above 1e-12 of its
//! material's largest (as the transfer arm of #1622 did, 2.8e-15). The
//! external-source cases carry the augmented system that PETIR's dense
//! CRAM-48 solves to only ~1e-8 (gh:#1645), so they are predicted at
//! ~1e-8, not better. A caveat on the reference: the files were committed
//! by an older OpenMC than `e4febe9` (upstream regenerates them only when
//! they fail its own 1e-3 tolerance), so a difference larger than round-off
//! is first to be checked against `e4febe9` itself before it is read as a
//! port defect.
//!
//! **Results, measured 2026-11-17:**
//! - 11 of the 12 files replayed. `depletion_with_ext_source_and_transfer`
//!   was written without reaction rates (`write_rates` off), so its step
//!   cannot be replayed without transport; it is reported, not checked.
//! - no external source (9 cases, incl. 0 W decay-only, Xe removal, feed,
//!   transfer to `w`, redox): worst **2.0e-15** (prediction held);
//! - external 0.1 g/s natural-U source (2 cases): worst **1.0e-13**, better
//!   than the ~1e-8 predicted. The #1645 feed case added 1 g/d of U-235
//!   (about 3e16 atoms/s against 1e-9 rates); this source is 2.6e20 atoms/s
//!   into a vector of 1e22, a better-scaled augmented column. Not
//!   investigated further: the prediction was a bound from #1645, and #1645
//!   stays open.
//! - The port's natural abundances of U equal OpenMC's
//!   (5.4e-5, 0.007204, 0.992742); atomic masses are OpenMC's, read from the
//!   file.
//!
//! The gates stay at the prediction (1e-10 without a source, 1e-7 with).

use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;
use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::cram::cram48;
use outram_mc_libs::depletion::external_rates::{deplete_with_external_rates, ExternalSourceRates, MassRateUnit, RateUnit, TransferRates};
use outram_mc_libs::depletion::{MicroRate, ReactionRates};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Default)]
struct Case {
    source_rate: f64,
    dt: f64,
    /// (step, material) -> nuclide -> atoms
    atoms: HashMap<(usize, String), HashMap<String, f64>>,
    /// material -> nuclide -> (gamma, fission)
    rates: HashMap<String, HashMap<String, (f64, f64)>>,
}

#[test]
fn openmc_transfer_rate_regressions_reproduced_on_their_own_rates() {
    let v = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/spectrum_depletion");
    let xml = DepletionChainXml::parse(&std::fs::read_to_string(v.join("openmc_inputs/chain_simple_tests.xml")).unwrap()).unwrap();
    let chain = DepletionChain::from_chain_xml(&xml, 0.0253).unwrap();
    let names: Vec<String> = chain.nuclide_names().iter().map(|s| (*s).to_owned()).collect();

    let text = std::fs::read_to_string(v.join("openmc_outputs/transfer_regression.tsv")).unwrap();
    let mut mass: HashMap<String, f64> = HashMap::new();
    let mut abundance: Vec<(String, f64)> = Vec::new();
    let mut cases: HashMap<String, Case> = HashMap::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["mass", n, x] => {
                mass.insert((*n).into(), x.parse().unwrap());
            }
            ["abundance", n, x] => abundance.push(((*n).into(), x.parse().unwrap())),
            ["time", c, t0, t1, s] => {
                let e = cases.entry((*c).into()).or_default();
                e.dt = t1.parse::<f64>().unwrap() - t0.parse::<f64>().unwrap();
                e.source_rate = s.parse().unwrap();
            }
            ["atoms", c, step, m, n, x] => {
                cases.entry((*c).into()).or_default().atoms.entry((step.parse().unwrap(), (*m).into())).or_default().insert((*n).into(), x.parse().unwrap());
            }
            ["rate", c, m, n, rx, x] => {
                let e = cases.entry((*c).into()).or_default().rates.entry((*m).into()).or_default().entry((*n).into()).or_default();
                match *rx {
                    "(n,gamma)" => e.0 = x.parse().unwrap(),
                    "fission" => e.1 = x.parse().unwrap(),
                    other => panic!("unexpected reaction {other}"),
                }
            }
            _ => panic!("bad line {line}"),
        }
    }
    assert_eq!(cases.len(), 12);

    // The port's natural abundances of U against OpenMC's.
    let ours: Vec<(String, f64)> = outram_mc_libs::model::isotope::Element::from_z(92).unwrap().natural_abundance().into_iter().map(|(i, f)| (i.name(), f)).collect();
    for (n, f) in &abundance {
        let o = ours.iter().find(|(m, _)| m == n).map(|x| x.1);
        println!("abundance {n}: OpenMC {f}, port {o:?}");
    }

    // Gd157 is the redox buffer, oxidation states as upstream's test.
    let os: [(&str, i32); 5] = [("I", -1), ("Xe", 0), ("Cs", 1), ("Gd", 3), ("U", 4)];
    let mats = vec!["1".to_string(), "2".to_string()];

    let mut keys: Vec<&String> = cases.keys().collect();
    keys.sort();
    let mut worst_plain = 0.0f64;
    let mut worst_source = 0.0f64;
    let mut checked = 0;
    for case in keys {
        let c = &cases[case];
        if c.rates.is_empty() {
            // Written without `write_rates`: the step cannot be replayed
            // without transport.
            println!("{case:>40}: no reaction rates in the reference file, not checked");
            continue;
        }
        checked += 1;
        let words: Vec<&str> = case.split('_').collect();
        let mut tr = TransferRates::new(mats.clone(), names.clone(), 1);
        let mut es = ExternalSourceRates::new(mats.clone(), 1);
        let (mut use_tr, mut use_es) = (false, false);
        // test_transfer_rates' parameter table, by reference-file name.
        let xe: Option<(f64, Option<&str>)> = match case.as_str() {
            "no_depletion_only_removal" | "depletion_with_removal" | "depletion_with_removal_and_redox" => Some((1e-5, None)),
            "no_depletion_only_feed" | "depletion_with_feed" => Some((-1e-5, None)),
            "no_depletion_with_transfer" => Some((-1e-5, Some("2"))),
            "depletion_with_transfer" | "depletion_with_transfer_and_redox" => Some((1e-5, Some("2"))),
            _ => None,
        };
        if let Some((rate, dest)) = xe {
            tr.set_transfer_rate("1", &["Xe"], rate, RateUnit::PerSecond, None, dest).unwrap();
            use_tr = true;
        }
        if words.contains(&"redox") {
            tr.set_redox("1", &[("Gd157", 1.0)], &os, None).unwrap();
            use_tr = true;
        }
        if words.contains(&"ext") {
            es.set_external_source_rate("1", &[("U", 1.0)], 1e-1, MassRateUnit::GramPerSecond, None, |n| mass.get(n).copied()).unwrap();
            use_es = true;
        }
        if case == "depletion_with_ext_source_and_transfer" {
            tr.set_transfer_rate("1", &["U235"], 1e-1, RateUnit::PerSecond, None, Some("2")).unwrap();
            use_tr = true;
        }

        let mut matrices = Vec::new();
        let mut n0 = Vec::new();
        for m in &mats {
            let mut rates = ReactionRates::zero();
            for (nuc, (g, f)) in &c.rates[m] {
                rates.set(nuc, MicroRate { gamma: *g, fission: *f, n2n: 0.0 });
            }
            matrices.push(chain.build_matrix(&rates));
            n0.push(names.iter().map(|n| c.atoms[&(0, m.clone())][n]).collect::<Vec<f64>>());
        }
        let out = deplete_with_external_rates(cram48, &names, matrices, n0, c.dt, 0, use_tr.then_some(&tr), use_es.then_some(&es)).unwrap();

        let mut worst = 0.0f64;
        let mut at = String::new();
        for (mi, m) in mats.iter().enumerate() {
            let big = out[mi].iter().cloned().fold(0.0, f64::max);
            for (j, n) in names.iter().enumerate() {
                let r = c.atoms[&(1, m.clone())][n];
                if r.abs() < 1e-12 * big {
                    continue;
                }
                let rel = ((out[mi][j] - r) / r).abs();
                if rel > worst {
                    worst = rel;
                    at = format!("mat {m} {n}: ours {:e}, OpenMC {r:e}", out[mi][j]);
                }
            }
        }
        println!("{case:>40} (P = {} W): worst {worst:.2e}  {at}", c.source_rate);
        if use_es {
            worst_source = worst_source.max(worst);
        } else {
            worst_plain = worst_plain.max(worst);
        }
    }
    assert_eq!(checked, 11);
    println!("no source: worst {worst_plain:.2e}; with source: worst {worst_source:.2e}");
    assert!(worst_plain <= 1e-10, "{worst_plain}");
    assert!(worst_source <= 1e-7, "{worst_source}");
}
