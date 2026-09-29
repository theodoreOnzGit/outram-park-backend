// SPDX-License-Identifier: GPL-3.0

//! **A single NU block beside a DNU block is prompt nu, and the total is
//! prompt + delayed** — GitHub #365 audit.
//!
//! # What was wrong
//!
//! `acer::ce_laws::decode_nu` returned a single NU block as the *total* nu-bar
//! whatever else the table held. OpenMC reads it as **prompt** when the table
//! carries DNU (`openmc/data/reaction.py:257-258`: `whichnu = 'prompt' if
//! ace.jxs[24] > 0 else 'total'`), so the old reading dropped the delayed
//! neutrons, about 1 % of nu on U-235.
//!
//! # Methodology
//!
//! NJOY writes both blocks whenever it writes DNU, so no held table has the
//! single-block form. The test **constructs** one from NJOY2016's U-235
//! (`reference-data/ace`): JXS(2) is moved to the prompt block's own `LNU`
//! word, which leaves a table whose only NU block is the prompt one, beside
//! the original DNU.
//!
//! 1. **The rule.** `decode_nu` on it must equal prompt(E) + delayed(E) at
//!    1000 log-spaced energies to 1e-12 relative. That is OpenMC's definition
//!    of the total for this form. prompt is the same table decoded with DNU
//!    removed, and delayed is `acer::delayed::decode_delayed`'s nu_d.
//! 2. **The old reading is visibly wrong.** The prompt block alone must differ
//!    from NJOY's own total block by more than 0.5 % at 1 MeV.
//!
//! Also printed, not gated: the agreement with NJOY's own total block at that
//! block's grid points. ENDF MT=452 is tabulated separately from MT=455/456,
//! so the two are equal only to the evaluation's own consistency. My a-priori
//! expectation of 1e-6 (7-figure rounding) was wrong; 2.8e-6 was measured, so
//! it is reported rather than turned into a gate after the fact.
//!
//! # Results (2026-09-29)
//!
//! Rule: exact to the printed precision. Old reading: −0.660 % at 1 MeV.
//! Against NJOY's independent total: worst 2.82e-6 relative.

use njoy_outram_park_fork::acer::ce_laws::decode_nu;
use njoy_outram_park_fork::acer::delayed::decode_delayed;
use njoy_outram_park_fork::acer::jxs;
use njoy_outram_park_fork::acer::read::read;
use njoy_outram_park_fork::reference_data::ace_reference_file_or_skip;

#[test]
fn single_prompt_block_plus_dnu_gives_the_total() {
    let Some(p) = ace_reference_file_or_skip(
        "reference-njoy/endf-b-viii.0/293.6K/U235.ace.gz",
        "single NU block + DNU",
    ) else {
        return;
    };
    let orig = read(&p).expect("read U235");
    let nu0 = (orig.jxs[jxs::NU] - 1) as usize;
    assert!(
        orig.xss[nu0] < 0.0,
        "NJOY writes prompt AND total for U-235"
    );
    let total = decode_nu(&orig).unwrap().unwrap();

    let mut single = orig.clone();
    single.jxs[jxs::NU] += 1; // now points at the prompt block's LNU word
    assert!(single.xss[(single.jxs[jxs::NU] - 1) as usize] > 0.0);
    let got = decode_nu(&single).unwrap().unwrap();

    let at = |x: &[f64], y: &[f64], e: f64| {
        let k = x.partition_point(|&v| v <= e).clamp(1, x.len() - 1);
        let (x0, x1, y0, y1) = (x[k - 1], x[k], y[k - 1], y[k]);
        y0 + (y1 - y0) * (e - x0) / (x1 - x0)
    };
    let mut worst = 0.0f64;
    for (&e, &want) in total.energy.iter().zip(&total.nu_total) {
        let v = at(&got.energy, &got.nu_total, e);
        worst = worst.max(((v - want) / want).abs());
    }
    // 1. The rule: prompt + delayed.
    let mut prompt_only = single.clone();
    prompt_only.jxs[jxs::DNU] = 0;
    let p1 = decode_nu(&prompt_only).unwrap().unwrap();
    let d = decode_delayed(&orig).unwrap().expect("U-235 has DNU");
    let mut rule = 0.0f64;
    for k in 0..1000 {
        let e = 1.0e-5 * (2.0e7f64 / 1.0e-5).powf(k as f64 / 999.0);
        let want = at(&p1.energy, &p1.nu_total, e) + at(&d.energy, &d.nu_delayed, e);
        let v = at(&got.energy, &got.nu_total, e);
        rule = rule.max(((v - want) / want).abs());
    }
    // 2. What the old reading (prompt taken as total) gave at 1 MeV.
    let old =
        (at(&p1.energy, &p1.nu_total, 1.0e6) / at(&total.energy, &total.nu_total, 1.0e6)) - 1.0;
    println!(
        "prompt + delayed rule: worst {rule:.2e}; vs NJOY's independent total block: worst {worst:.2e}; \
         prompt-as-total error at 1 MeV: {:+.3} %",
        old * 100.0
    );
    assert!(
        rule < 1.0e-12,
        "decode_nu is not prompt + delayed: {rule:.2e}"
    );
    assert!(
        old.abs() > 5.0e-3,
        "the old reading must be visibly wrong here"
    );
}
