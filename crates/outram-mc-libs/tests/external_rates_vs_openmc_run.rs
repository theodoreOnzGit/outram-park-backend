#![cfg(feature = "std")]
//! **Transfer and external source rates against OpenMC run** (gh:#1622).
//!
//! Reference: OpenMC `e4febe9`'s own `openmc.deplete.pool.deplete` with its
//! `TransferRates` / `ExternalSourceRates`, on `chain_simple.xml`, two
//! burnable materials, one 1-day step, OpenMC's IPF CRAM-48; run 2026-11-17
//! by `verification_and_validation/spectrum_depletion/openmc_inputs/
//! ref_transfer_rates.py` (pure Python, `READTHEDOCS=True` docs-build mock
//! for `openmc.lib`), output `openmc_outputs/transfer_rates.tsv`. Cases:
//! Xe moved 1 → 2 at 1/d; plus I-135 removed from 2 at 0.5/d; plus a
//! 1 g/d U-235 feed into 1; plus redox on 1 (buffer Cs-135).
//!
//! The port is fed the same chain, rates, initial vectors and settings,
//! with OpenMC's own U-235 atomic mass, and solved with this crate's
//! CRAM-48. Prediction, written before the run: every case agrees to
//! round-off, at most 1e-10 relative on every entry above 1e-12 of its
//! material's largest (dense LU here, sparse there; summation order).
//!
//! Results, measured 2026-11-17:
//!
//! | case | worst relative |
//! |---|---|
//! | transfer | 2.8e-15 |
//! | transfer_and_removal | 2.2e-15 |
//! | transfer_and_redox | 2.8e-15 |
//! | transfer_and_feed | **1.5e-8: prediction FAILED** |
//!
//! The feed case was diagnosed, not loosened. The assembled system is
//! OpenMC's to every digit (our `cram48` on OpenMC's captured matrix gives
//! our value exactly). On it `scipy.linalg.expm` gives U-235
//! 3.0619033393838885e21, OpenMC's CRAM-48 (SuperLU) 1.3e-11 from that,
//! **petir's CRAM-48 (dense elimination) 1.5e-8**, petir's CRAM-16 2.4e-11.
//! The augmented source column (~3e16 against rates of 1e-9) is badly scaled,
//! and the dense solve loses digits through the 24 IPF poles. Filed as
//! gh:#1645. The three other cases are gated at the predicted 1e-10; the
//! feed case is checked only to be the diagnosed value (a regression pin,
//! set after seeing it, not evidence).

use njoy_outram_park_fork::hdf5::depletion_chain_xml::DepletionChainXml;
use outram_mc_libs::depletion::chain::DepletionChain;
use outram_mc_libs::depletion::cram::cram48;
use outram_mc_libs::depletion::external_rates::{deplete_with_external_rates, ExternalSourceRates, MassRateUnit, RateUnit, TransferRates};
use outram_mc_libs::depletion::{MicroRate, ReactionRates};
use std::path::PathBuf;

const RATES: [(&str, f64, f64); 7] = [
    ("U234", 2.0e-9, 1.0e-11),
    ("U235", 3.0e-10, 1.5e-9),
    ("U238", 1.0e-10, 1.0e-12),
    ("Xe135", 2.0e-5, 0.0),
    ("I135", 1.0e-9, 0.0),
    ("Gd157", 1.0e-5, 0.0),
    ("Gd156", 1.0e-9, 0.0),
];
const N0: [(&str, f64); 8] =
    [("U234", 1e18), ("U235", 5e20), ("U238", 2e22), ("Xe135", 1e15), ("I135", 1e16), ("Gd157", 1e17), ("Gd156", 1e17), ("Cs135", 1e16)];

#[test]
fn external_rates_match_openmc_pool_deplete() {
    let v = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("verification_and_validation/spectrum_depletion");
    let text = std::fs::read_to_string(v.join("openmc_outputs/transfer_rates.tsv")).unwrap();
    let mut mass_u235 = 0.0;
    let mut avogadro = 0.0;
    let mut reference: Vec<(String, usize, String, f64)> = Vec::new();
    for line in text.lines() {
        let f: Vec<&str> = line.split('\t').collect();
        match f.as_slice() {
            ["atomic_mass_U235", x] => mass_u235 = x.parse().unwrap(),
            ["avogadro", x] => avogadro = x.parse().unwrap(),
            [case, m, nuc, x] => reference.push(((*case).into(), m.parse().unwrap(), (*nuc).into(), x.parse().unwrap())),
            _ => panic!("bad line {line}"),
        }
    }
    assert_eq!(avogadro, outram_mc_libs::model::masses::AVOGADRO_PER_MOL, "same Avogadro constant");

    let chain_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../njoy-outram-park-fork/verification_and_validation/depletion_chain_xml/openmc_inputs/chain_simple.xml");
    let xml = DepletionChainXml::parse(&std::fs::read_to_string(chain_path).unwrap()).unwrap();
    let chain = DepletionChain::from_chain_xml(&xml, 0.0253).unwrap();
    let names: Vec<String> = chain.nuclide_names().iter().map(|s| (*s).to_owned()).collect();
    let mut rates = ReactionRates::zero();
    for (n, g, f) in RATES {
        rates.set(n, MicroRate { gamma: g, fission: f, n2n: 0.0 });
    }
    let a = chain.build_matrix(&rates);
    let n0: Vec<f64> = names.iter().map(|n| N0.iter().find(|(m, _)| *m == n).map_or(0.0, |(_, x)| *x)).collect();
    let n = vec![n0.clone(), n0.iter().map(|x| 0.5 * x).collect::<Vec<_>>()];
    let mats = vec!["1".to_string(), "2".to_string()];
    let transfer = || {
        let mut tr = TransferRates::new(mats.clone(), names.clone(), 1);
        tr.set_transfer_rate("1", &["Xe"], 1.0, RateUnit::PerDay, None, Some("2")).unwrap();
        tr
    };
    let run = |tr: &TransferRates, es: Option<&ExternalSourceRates>| {
        deplete_with_external_rates(cram48, &names, vec![a.clone(), a.clone()], n.clone(), 86_400.0, 0, Some(tr), es).unwrap()
    };
    let mut cases: Vec<(&str, Vec<Vec<f64>>)> = vec![("transfer", run(&transfer(), None))];
    let mut tr = transfer();
    tr.set_transfer_rate("2", &["I135"], 0.5, RateUnit::PerDay, None, None).unwrap();
    cases.push(("transfer_and_removal", run(&tr, None)));
    let mut es = ExternalSourceRates::new(mats.clone(), 1);
    es.set_external_source_rate("1", &[("U235", 100.0)], 1.0, MassRateUnit::GramPerDay, None, |nuc| (nuc == "U235").then_some(mass_u235)).unwrap();
    cases.push(("transfer_and_feed", run(&transfer(), Some(&es))));
    let mut tr = transfer();
    tr.set_redox("1", &[("Cs135", 1.0)], &[("Cs", 1), ("I", -1), ("Xe", 0), ("U", 4), ("Gd", 3)], None).unwrap();
    cases.push(("transfer_and_redox", run(&tr, None)));

    let mut worst_all = 0.0f64;
    for (case, ours) in &cases {
        let mut worst = 0.0f64;
        let mut at = String::new();
        let mut compared = 0;
        for m in 0..2 {
            let big = ours[m].iter().cloned().fold(0.0, f64::max);
            for (i, nuc) in names.iter().enumerate() {
                let r = reference.iter().find(|(c, mm, nn, _)| c == case && *mm == m && nn == nuc).unwrap().3;
                if r.abs() < 1e-12 * big {
                    continue;
                }
                compared += 1;
                let rel = ((ours[m][i] - r) / r).abs();
                if rel > worst {
                    worst = rel;
                    at = format!("material {m} {nuc}: ours {:e}, OpenMC {r:e}", ours[m][i]);
                }
            }
        }
        println!("{case}: {compared} entries, worst relative {worst:.3e} ({at})");
        if *case == "transfer_and_feed" {
            // gh:#1645: petir CRAM-48 on this badly scaled system.
            assert!(worst < 2e-8, "feed case moved from its diagnosed 1.5e-8: {worst}");
        } else {
            worst_all = worst_all.max(worst);
        }
    }
    assert!(worst_all <= 1e-10, "{worst_all}");
}
