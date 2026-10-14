//! **H in H₂O baked from LEAPR's own tape against the published tape** (2026-10-14).
//!
//! The five-route campaign's ENDF route (route 4) no longer reads
//! `tsl-HinH2O.endf`: maintainer direction 2026-10-14 is that LEAPR always runs
//! and the published `tsl-*.endf` tapes are oracles. Route 4 regenerates the
//! law with LEAPR and bakes it with `ThermalScattering::from_tape`. This test
//! runs the same three calls as `examples/icsbep_five_route_keff.rs`'s
//! `h_in_h2o_from_leapr` and compares the result with the published tape
//! baked by `from_endf_file`, the path route 4 used before.
//!
//! # Methodology
//!
//! - **LEAPR side:** the ENDF/B-VIII.0 deck compiled into
//!   `njoy-outram-park-fork` (`tsl-HinH2O.leapr`), its 293.6 K block, with the
//!   deck's own physical constants (its `EVAL` date), through
//!   `leapr::run::run_deck_at_temperature`; the tape is read back from its
//!   text and baked by `ThermalScattering::from_tape` at 293.6 K.
//! - **Oracle:** `reference-data/endf/tsl-HinH2O.endf` (ENDF/B-VIII.0, MAT 1)
//!   baked by `ThermalScattering::from_endf_file` at 293.6 K, same grids.
//! - **Compared:** σ_inel and σ_total per H at 200 log-spaced energies from
//!   1e-5 eV to the cutoff, and the cutoff and selected temperature.
//! - **Pass criterion:** every σ within **1e-3** relative, THERMR's `tol`,
//!   the tolerance both tables are linearised to; a difference below it is
//!   not resolved by either table. Fixed before measuring. Skips when the
//!   published tape is absent.
//!
//! # Results (2026-10-14)
//!
//! **Identical.** LEAPR's 293.6 K tape stores the same S(α,β) as the
//! published tape in all **70 374** values (same β and α grids), so the baked
//! tables agree exactly: σ_inel and σ_total differ by **0** at all 200
//! energies to the 10.0 eV cutoff. The 1e-3 criterion above was fixed before
//! measuring; the identity assertion (`same == n`) was added after it, to pin
//! what was measured. Release build, 33 s.
//!
//! Interpretation: for H in H₂O at 293.6 K, regenerating with LEAPR changes
//! nothing route 4 transports; the deck's own constants (its `EVAL` date)
//! reproduce the published tape to the bit.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{embedded_deck_text, SabMaterial};
use njoy_outram_park_fork::leapr::run::run_deck_at_temperature;
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use outram_mc_libs::material::thermal::ThermalScattering;

const TEMP_K: f64 = 293.6;

#[test]
fn h_in_h2o_from_leapr_matches_the_published_tape_to_thermr_tol() {
    let Some(tsl) = reference_endf_or_skip("tsl-HinH2O.endf", "H in H2O LEAPR vs tsl") else { return };
    let deck = LeaprDeck::parse(embedded_deck_text(SabMaterial::HInH2O).unwrap()).unwrap();
    let run = run_deck_at_temperature(&deck, TEMP_K).unwrap();
    let tape = Tape::read(std::io::Cursor::new(run.write_text().unwrap())).unwrap();
    let leapr = ThermalScattering::from_tape(&tape, deck.mat, TEMP_K, "c_H_in_H2O").unwrap();
    let published = ThermalScattering::from_endf_file(tsl.to_str().unwrap(), 1, TEMP_K, "c_H_in_H2O").unwrap();

    // The stored S(alpha, beta) itself, value by value: what the baked tables
    // are made from.
    let ii = |t: &Tape| parse_mf7_at_temperature(t, 1, Some(TEMP_K)).unwrap().incoherent_inelastic.unwrap();
    let (a, b) = (ii(&tape), ii(&Tape::read_file(&tsl).unwrap()));
    assert_eq!((a.beta.len(), a.s_tables.len()), (b.beta.len(), b.s_tables.len()), "beta grids");
    let (mut n, mut same, mut worst_s) = (0usize, 0usize, 0.0_f64);
    for (x, y) in a.s_tables.iter().zip(&b.s_tables) {
        assert_eq!(x.alpha, y.alpha, "alpha grid at beta {}", x.beta);
        for (p, q) in x.s.iter().zip(&y.s) {
            n += 1;
            if p == q {
                same += 1;
            } else if *q > 1e-30 {
                worst_s = worst_s.max((p / q - 1.0).abs());
            }
        }
    }
    println!("S(alpha,beta): {same} of {n} stored values identical; worst relative difference elsewhere {worst_s:.3e}");
    assert_eq!(same, n, "S(alpha,beta) no longer identical to the published tape (worst {worst_s:.3e})");

    assert!((leapr.selected_temperature_k() - TEMP_K).abs() < 1e-9, "LEAPR law at {} K", leapr.selected_temperature_k());
    let rel_cut = (leapr.cutoff_ev() - published.cutoff_ev()).abs() / published.cutoff_ev();
    assert!(rel_cut < 1e-6, "cutoff {} eV against {} eV", leapr.cutoff_ev(), published.cutoff_ev());

    let (lo, hi) = (1.0e-5_f64.ln(), (published.cutoff_ev() * 0.999).ln());
    let (mut worst_inel, mut worst_tot, mut at) = (0.0_f64, 0.0_f64, 0.0);
    for k in 0..200 {
        let e = (lo + (hi - lo) * k as f64 / 199.0).exp();
        let di = (leapr.inelastic_xs(e) / published.inelastic_xs(e) - 1.0).abs();
        let dt = (leapr.total_xs(e) / published.total_xs(e) - 1.0).abs();
        if di > worst_inel {
            (worst_inel, at) = (di, e);
        }
        worst_tot = worst_tot.max(dt);
    }
    println!("measured: worst |sigma_inel| rel {worst_inel:.3e} (at {at:.4e} eV), worst |sigma_total| rel {worst_tot:.3e}, 200 energies to {:.4} eV", published.cutoff_ev());
    assert!(worst_inel < 1e-3, "sigma_inel differs by {worst_inel:.3e} at {at:.4e} eV");
    assert!(worst_tot < 1e-3, "sigma_total differs by {worst_tot:.3e}");
}
