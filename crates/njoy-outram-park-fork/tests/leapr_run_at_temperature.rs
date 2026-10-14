//! **`leapr::run::run_deck_at_temperature`, and `run_deck` on a cold
//! hydrogen deck**, against NJOY2016's own LEAPR tapes (GitHub #871).
//!
//! # Methodology
//!
//! - **One block of a multi-temperature deck.** `run_deck_at_temperature`
//!   cuts a deck to the block for one temperature and runs LEAPR's whole
//!   driver on it; `process::thermal_scattering` relies on this. The claim is
//!   that LEAPR's temperature loop is independent per block, so the cut deck's
//!   law is the full deck's at that temperature. Checked on the Si-in-α-quartz
//!   deck (`reference-data/leapr/tsl-SiO2-alpha-njoy2016-leapr.njoy-input`,
//!   five temperatures, a mixed moderator with a second scatterer pass)
//!   at 350 K, its second block, against the 350 K block of NJOY2016's tape
//!   from the whole deck: every `S(α,β)` above `1e-30`, and `T_eff`. Pass:
//!   1e-9 relative (the port reproduces the whole tape to 1.0e-13,
//!   `tests/leapr_sio2_mixed_moderator_oracle.rs`).
//! - **`run_deck` on a cold-moderator deck.** `run_deck` wrote `isym = isabt`
//!   and dropped `coldh`'s positive-beta half until 2026-10-14, so a cold
//!   H2/D2 deck came out as a symmetric law. NJOY2016's test-22 deck (para
//!   hydrogen at 20 K, `ncold = 2`) through `run_deck_text`, against NJOY's
//!   tape: the signed 209-point beta grid and every `S` above `1e-30`. Pass:
//!   1e-9 relative (`generate_tape` reproduces it to 1.0e-13,
//!   `tests/leapr_cold_hydrogen_njoy_oracle.rs`).
//!
//! # Results (2026-10-14)
//!
//! Both pass, read back through the tape's text as THERMR reads it: SiO2 at
//! 350 K, 8 652 values **identical** (worst 0), `T_eff` 540.9517 K on both;
//! para-H2 at 20 K, 9 175 values **identical**, 209 betas. (Compared in
//! memory, before `endout`'s rows go through text, the worst is 1.0e-13.)
//! Before the `run_deck` fix the para-hydrogen law could not have had 209
//! betas: `endout` doubles the grid only for an odd `isym`, and `isabt = 0`.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::run::{run_deck_at_temperature, run_deck_text, LeaprRun};
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use njoy_outram_park_fork::thermr::mf7::{parse_mf7_at_temperature, IncoherentInelastic};

const TOL: f64 = 1.0e-9;

/// The LEAPR deck NJOY read: from the `leapr` line to the `stop` line.
fn leapr_section(text: &str) -> String {
    let start = text.find("leapr\n").expect("a leapr line");
    let body = &text[start..];
    let end = body.find("\nstop").expect("a stop line");
    format!("{}\nstop\n", &body[..end])
}

/// LEAPR's tape written as text and read back.
fn as_text(run: &LeaprRun) -> Tape {
    Tape::read(std::io::Cursor::new(run.write_text().unwrap())).unwrap()
}

fn worst_s(o: &IncoherentInelastic, r: &IncoherentInelastic) -> (f64, usize) {
    assert_eq!(o.beta.len(), r.beta.len(), "beta grid");
    let (mut worst, mut n) = (0.0f64, 0usize);
    for (ot, rt) in o.s_tables.iter().zip(&r.s_tables) {
        assert_eq!(ot.alpha.len(), rt.alpha.len());
        for (&a, &b) in ot.s.iter().zip(&rt.s) {
            if b.abs() < 1.0e-30 {
                continue;
            }
            n += 1;
            worst = worst.max((a - b).abs() / b.abs());
        }
    }
    (worst, n)
}

#[test]
fn one_block_of_a_five_temperature_mixed_deck_is_njoys_block() {
    let label = "leapr-run-at-temperature";
    let Some(deck) = reference_file_or_skip("leapr", "tsl-SiO2-alpha-njoy2016-leapr.njoy-input", label) else { return };
    let Some(tape) = reference_file_or_skip("leapr", "tsl-SiO2-alpha-njoy2016-leapr.endf", label) else { return };
    let deck = LeaprDeck::parse(&leapr_section(&std::fs::read_to_string(deck).unwrap()))
        .unwrap()
        .with_constants(PhysicalConstants::Codata2018);
    assert!(deck.temperatures_k().len() == 5 && deck.is_mixed_moderator());
    let run = run_deck_at_temperature(&deck, 350.0).expect("LEAPR at 350 K");
    assert_eq!(run.output.temperatures_k, vec![350.0], "one block, at the request");
    // As THERMR reads it: through the tape's text. (In memory, `endout`'s
    // temperature row holds 350.00000000003496.)
    let ours = parse_mf7_at_temperature(&as_text(&run), 47, Some(350.0)).unwrap().incoherent_inelastic.unwrap();
    let njoy = Tape::read_file(&tape).unwrap();
    let theirs = parse_mf7_at_temperature(&njoy, 47, Some(350.0)).unwrap().incoherent_inelastic.unwrap();
    assert_eq!(theirs.temperature_k, 350.0);
    let (worst, n) = worst_s(&ours, &theirs);
    let teff = |ii: &IncoherentInelastic| ii.teff_table.iter().find(|p| p.0 == 350.0).map(|p| p.1);
    println!("SiO2 at 350 K: {n} values, worst {worst:.3e}; T_eff {:?} vs {:?}", teff(&ours), teff(&theirs));
    assert!(n > 5000 && worst < TOL, "{n} values, worst {worst:e}");
    assert_eq!(teff(&ours), teff(&theirs), "T_eff");
}

#[test]
fn run_deck_writes_a_cold_hydrogen_law_signed() {
    let label = "leapr-run-at-temperature";
    let Some(deck) = reference_file_or_skip("leapr", "tsl-para-H2-20K-njoy2016-leapr.njoy-input", label) else { return };
    let Some(tape) = reference_file_or_skip("leapr", "tsl-para-H2-20K-njoy2016-leapr.endf", label) else { return };
    let run = run_deck_text(&leapr_section(&std::fs::read_to_string(deck).unwrap())).expect("run_deck on para-H2");
    let mat = run.output.mat;
    let ours = parse_mf7_at_temperature(&as_text(&run), mat, Some(20.0)).unwrap().incoherent_inelastic.unwrap();
    let theirs = parse_mf7_at_temperature(&Tape::read_file(&tape).unwrap(), mat, Some(20.0))
        .unwrap()
        .incoherent_inelastic
        .unwrap();
    assert_eq!((ours.beta.len(), ours.lasym), (209, theirs.lasym), "the signed beta grid and LASYM");
    let (worst, n) = worst_s(&ours, &theirs);
    println!("para-H2 at 20 K through run_deck: {n} values, worst {worst:.3e}");
    assert!(n > 9000 && worst < TOL, "{n} values, worst {worst:e}");
}
