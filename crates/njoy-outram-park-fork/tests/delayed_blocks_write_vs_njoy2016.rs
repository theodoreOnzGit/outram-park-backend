// SPDX-License-Identifier: GPL-3.0

//! **The delayed-neutron blocks this crate writes, against NJOY2016's own.**
//!
//! # Methodology
//!
//! `acer::delayed_blocks` ports `acefc.f90:5997-6253`. The four blocks it
//! writes — DNU, BDD, DNEDL, DNED — depend only on MF=1/MT=455 and MF=5/MT=455,
//! not on the reconstructed energy grid, so they can be compared against NJOY's
//! **word for word** without running RECONR. For each reference table in
//! `reference-data/ace` (made by `RECONR+BROADR+PURR+ACER`, ACER's default
//! `ismooth = 1`), the words between NJOY's own JXS(24)..JXS(27) locators are
//! taken as the oracle and ours must reproduce every one **as a Type-1 file
//! prints it** (12 significant digits), and `NXS(8)`.
//!
//! # Results (2026-09-26)
//!
//! Printed per nuclide and block by the test.

use njoy_outram_park_fork::acer::delayed_blocks;
use njoy_outram_park_fork::acer::{jxs, nxs, read};
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::reference_data::{ace_reference_file_or_skip, reference_endf};

fn njoy_span(t: &read::RawAceTable, from: usize, to_loc: i32) -> Vec<f64> {
    let a = (t.jxs[from] - 1) as usize;
    let b = (to_loc - 1) as usize;
    t.xss[a..b].to_vec()
}

#[test]
fn delayed_blocks_reproduce_njoys_words() {
    for (name, tape_file, mat) in [
        ("U234", "n-092_U_234-ENDF8.0.endf", 9225),
        ("U235", "n-092_U_235-ENDF8.0.endf", 9228),
        ("U238", "n-092_U_238.endf", 9237),
    ] {
        let rel = format!("reference-njoy/endf-b-viii.0/293.6K/{name}.ace.gz");
        let Some(p) = ace_reference_file_or_skip(&rel, "delayed-blocks") else {
            return;
        };
        let theirs = read::read(&p).expect("read NJOY's table");
        let Some(tp) = reference_endf(tape_file) else {
            println!("SKIP {name}: tape absent");
            return;
        };
        let tape = Tape::read_file(&tp).expect("tape");
        let ours = delayed_blocks::build(&tape, mat, true)
            .expect("build")
            .expect("these evaluations carry delayed data");

        assert_eq!(
            ours.n_groups as i32, theirs.nxs[nxs::NDNF],
            "{name}: NXS(8) ours {} NJOY {}",
            ours.n_groups, theirs.nxs[nxs::NDNF]
        );

        // DNED ends where the next present block starts: GPD, else MTRP, else END.
        let next_after_dned = [11usize, jxs::MTRP]
            .iter()
            .map(|&j| theirs.jxs[j])
            .find(|&l| l > theirs.jxs[jxs::DNED])
            .unwrap_or(theirs.jxs[jxs::END] + 1);
        let blocks = [
            ("DNU", &ours.dnu, njoy_span(&theirs, jxs::DNU, theirs.jxs[jxs::BDD])),
            ("BDD", &ours.bdd, njoy_span(&theirs, jxs::BDD, theirs.jxs[jxs::DNEDL])),
            ("DNEDL", &ours.dnedl, njoy_span(&theirs, jxs::DNEDL, theirs.jxs[jxs::DNED])),
            ("DNED", &ours.dned, njoy_span(&theirs, jxs::DNED, next_after_dned)),
        ];
        let mut failures = Vec::new();
        for (block, got, want) in blocks {
            let mut n_bad = 0usize;
            let mut first: Option<(usize, f64, f64)> = None;
            let mut worst = 0.0f64;
            // At a Type-1 file's precision (`1pE20.11`): the file is the artefact
            // being reproduced, and upstream's `sigfig` bias (x1.0000000000001)
            // lives below it.
            let printed = |x: f64| format!("{x:.11e}");
            for (i, ((g, _), w)) in got.iter().zip(want.iter()).enumerate() {
                if printed(*g) != printed(*w) {
                    n_bad += 1;
                    first.get_or_insert((i, *g, *w));
                    let r = if *w == 0.0 { g.abs() } else { ((g - w) / w).abs() };
                    worst = worst.max(r);
                }
            }
            println!(
                "{name} {block:<5}: ours {} words, NJOY {}, differing {n_bad} (worst rel \
                 {worst:.2e}), first {first:?}",
                got.len(),
                want.len()
            );
            if got.len() != want.len() || n_bad > 0 {
                failures.push(format!("{name} {block}"));
            }
        }
        assert!(failures.is_empty(), "blocks not reproduced: {failures:?}");
    }
}
