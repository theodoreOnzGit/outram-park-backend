// SPDX-License-Identifier: GPL-3.0

//! **Route 4's data are njoy's `process`, bit for bit** (gh:#931).
//!
//! # Methodology
//!
//! Since gh:#931, [`Nuclide::process_evaluation`] (route 4 of the five-route
//! ICSBEP study, and the first half of every `Nuclide::from_tape` /
//! `from_endf_file`) is a thin call into `process::reconstruct` +
//! `process::cross_sections_at` at `process`'s defaults (maintainer
//! decision 2026-10-17: PURR 20 / 64 / 10 000, HEATR before PURR, the 0 K
//! elastic over the whole range, PENDF-rounded). This test pins it.
//!
//! U-234 from `reference-data/endf/` (ENDF/B-VIII.0; fissile, with an
//! unresolved range, so PURR's tables are compared too) at 293.6 K, RECONR
//! and BROADR tolerance 1e-3:
//!
//! 1. **Route 4 against `process::cross_sections`**, called directly with
//!    the explicit settings PURR 20 / 64 / 10 000 and HEATR on (so the pin
//!    holds the maintainer's settings even if `process`'s defaults move):
//!    the material header, the resonance limit, the unresolved table, every
//!    section's MT, LR, QI and every `(E, σ)` pair, the 0 K elastic and
//!    every word of PURR's tables ([`UrrProbabilityTables::to_f64s`]),
//!    compared as bits, no tolerance.
//! 2. **HEATR, at the table level** (the transport-level ablation is in the
//!    V&V record): the same call `without_heatr()`. Predicted, from
//!    `purr::tables::from_endf_with_mt301` (HEATR's MT=301 only fills the
//!    heating column and draws no random number): every section and every
//!    non-heating table word identical; the heating column non-zero with
//!    HEATR and all zero without it.
//! 3. **The old chain is a different product** ([`Nuclide::process_evaluation_pre_931`],
//!    the named ablation): broadened sections identical (the #929 result,
//!    P1), PURR's band values not (16 / 2000 against 64 / 10 000), so the
//!    comparison in (1) could have failed.
//! 4. A tape whose material is not the `mat` asked for is refused.
//!
//! **Pass criterion:** (1) and the stated parts of (2) and (3) identical in
//! every bit; the stated differences present.
//!
//! # Results, 2026-10-17
//!
//! See the test output (`cargo test --release -p outram-mc-libs --test
//! route4_is_process -- --nocapture`), recorded in
//! `verification_and_validation/route4_process_931/README.md`.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::process::{self, CrossSectionSettings};
use njoy_outram_park_fork::purr::UrrProbabilityTables;
use njoy_outram_park_fork::reconr::ReconrResult;
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;
use outram_mc_libs::material::processed::ProcessedEvaluation;
use uom::si::f64::ThermodynamicTemperature;
use uom::si::thermodynamic_temperature::kelvin;

const TEMP_K: f64 = 293.6;
const TOL: f64 = 1.0e-3;

/// Every bit of a PENDF-style result: header, limits and each section.
fn recon_bits(r: &ReconrResult) -> Vec<u64> {
    let m = &r.material;
    let mut v: Vec<u64> = [m.za, m.awr, m.elis, m.emax]
        .iter()
        .map(|x| x.to_bits())
        .collect();
    v.extend([m.lrp, m.lfi, m.nlib, m.nfor].iter().map(|&i| i as u64));
    v.push(r.resonance_upper_limit.unwrap_or(f64::NAN).to_bits());
    for row in r.unresolved_table.iter().flatten() {
        v.extend(row.iter().map(|x| x.to_bits()));
    }
    for s in &r.sections {
        v.extend([
            s.mt.number() as u64,
            s.lr as u64,
            s.qi.to_bits(),
            s.pairs.len() as u64,
        ]);
        for &(e, x) in &s.pairs {
            v.extend([e.to_bits(), x.to_bits()]);
        }
    }
    v
}

fn pairs_bits(p: &[(f64, f64)]) -> Vec<u64> {
    p.iter()
        .flat_map(|&(e, x)| [e.to_bits(), x.to_bits()])
        .collect()
}

/// PURR's tables as `(every word but the heating column, the heating
/// column)`, walking [`UrrProbabilityTables::to_f64s`]'s layout.
fn split_heating(t: &UrrProbabilityTables) -> (Vec<u64>, Vec<f64>) {
    let w = t.to_f64s();
    let n_e = w[7] as usize;
    let mut at = 8 + n_e;
    let mut rest: Vec<u64> = w[..at].iter().map(|x| x.to_bits()).collect();
    let mut heating = Vec::new();
    for _ in 0..n_e {
        let nb = w[at] as usize;
        let body = 1 + nb + 4 * nb;
        rest.extend(w[at..at + body].iter().map(|x| x.to_bits()));
        heating.extend_from_slice(&w[at + body..at + body + nb]);
        at += body + nb;
    }
    assert_eq!(at, w.len(), "layout walked to the end");
    (rest, heating)
}

#[test]
fn route4_is_process_bit_for_bit() {
    let Some(path) = reference_endf("n-092_U_234-ENDF8.0.endf") else {
        eprintln!("SKIP: U-234 not in reference-data/endf/");
        return;
    };
    let tape = Tape::read_file(&path).expect("tape");
    let mat = tape.materials()[0];
    let t = ThermodynamicTemperature::new::<kelvin>(TEMP_K);

    // (1) Route 4 against `process`, with the maintainer's settings spelled out.
    let route4 = Nuclide::process_evaluation(&tape, mat, TEMP_K, TOL, TOL).expect("route 4");
    let settings = CrossSectionSettings::default()
        .with_reconr_tolerance(TOL)
        .with_broadr_tolerance(TOL)
        .with_purr(20, 64, 10_000);
    assert!(
        settings.heatr && settings.elastic_0k,
        "HEATR and the 0 K elastic on"
    );
    let xs = process::cross_sections(&tape, t, &settings).expect("process");
    assert_eq!(route4.temp_k.to_bits(), TEMP_K.to_bits());
    let n_section_words = recon_bits(&route4.recon).len();
    assert_eq!(recon_bits(&route4.recon), recon_bits(&xs.pendf), "sections");
    let el = xs
        .elastic_0k
        .as_deref()
        .expect("process carries the 0 K elastic");
    assert_eq!(
        pairs_bits(&route4.elastic_0k),
        pairs_bits(el),
        "0 K elastic"
    );
    assert!(
        route4.elastic_0k.last().expect("elastic").0 > 1.0e6,
        "the whole range, not cut at 25 keV"
    );
    let (u4, x4) = (
        route4.urr.as_ref().expect("URR tables"),
        xs.urr.as_ref().expect("URR tables"),
    );
    let w = |t: &UrrProbabilityTables| t.to_f64s().iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(w(u4), w(x4), "PURR tables");
    let n_purr_words = w(u4).len();
    assert_eq!(u4.n_bands(), 20);
    let words = ProcessedEvaluation::from_cross_sections(xs.clone()).to_f64s();
    assert_eq!(
        route4
            .to_f64s()
            .iter()
            .map(|x| x.to_bits())
            .collect::<Vec<_>>(),
        words.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        "the whole product, as the browser pool ships it"
    );

    // (2) HEATR only fills PURR's heating column.
    let no_heatr =
        Nuclide::process_evaluation_with(&tape, mat, t, &settings.clone().without_heatr())
            .expect("without HEATR");
    assert_eq!(
        recon_bits(&no_heatr.recon),
        recon_bits(&route4.recon),
        "HEATR moves no section"
    );
    assert_eq!(
        pairs_bits(&no_heatr.elastic_0k),
        pairs_bits(&route4.elastic_0k)
    );
    let (rest_on, heat_on) = split_heating(u4);
    let (rest_off, heat_off) = split_heating(no_heatr.urr.as_ref().expect("URR tables"));
    assert_eq!(
        rest_on, rest_off,
        "HEATR moves no band value, probability or energy"
    );
    assert!(
        heat_off.iter().all(|&h| h == 0.0),
        "no HEATR: heating column zero"
    );
    assert!(
        heat_on.iter().any(|&h| h != 0.0),
        "HEATR: heating column filled"
    );

    // (3) The pre-#931 chain: same broadened sections, different tables.
    let pre = Nuclide::process_evaluation_pre_931(&tape, mat, TEMP_K, TOL, TOL).expect("pre-931");
    assert_eq!(
        recon_bits(&pre.recon),
        recon_bits(&route4.recon),
        "#929 P1: same sections"
    );
    let (rest_pre, _) = split_heating(pre.urr.as_ref().expect("URR tables"));
    let differ = rest_pre
        .iter()
        .zip(&rest_on)
        .filter(|(a, b)| a != b)
        .count();
    assert!(
        differ > 0,
        "16 / 2000 tables must differ from 64 / 10 000 ones"
    );
    assert!(
        pre.elastic_0k.iter().all(|&(e, _)| e <= 2.5e4),
        "pre-931 keeps <= 25 keV"
    );

    // The nuclide keeps the 0 K elastic to 25 keV on both, and carries URR and DBRC.
    let n = Nuclide::from_processed(&tape, mat, "U234", TEMP_K, route4).expect("nuclide");
    assert!(n.has_urr_probability_tables() && n.has_dbrc());

    // (4) The wrong MAT is refused, not processed.
    let err = Nuclide::process_evaluation(&tape, mat + 1, TEMP_K, TOL, TOL).expect_err("wrong MAT");
    assert!(err.to_string().contains("MAT"), "{err}");

    eprintln!(
        "U-234 at {TEMP_K} K: {} section words, {} 0 K elastic pairs, {} PURR words identical to \
         process; HEATR fills {} heating words and moves nothing else; pre-931 tables differ in {} \
         of {} non-heating words",
        n_section_words,
        el.len(),
        n_purr_words,
        heat_on.len(),
        differ,
        rest_on.len()
    );
}
