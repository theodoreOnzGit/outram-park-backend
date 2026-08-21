//! LEAPR temperature-block selection, and the Si-in-SiC kernel it was found by.
//!
//! # What this file covers
//!
//! A LEAPR deck's card 10 carries a **positive** temperature when cards 11-19
//! (frequency spectrum, translational weight, oscillators) follow it, and a
//! **negative** one when those cards are inherited from the preceding block.
//! Crystalline decks declare their spectrum once and inherit it at every
//! temperature; the liquid decks here (`HInH2O`, `DInD2O`, `OInD2O`) re-specify
//! it at every temperature, because a liquid's frequency distribution genuinely
//! changes with temperature.
//!
//! `generate_tape` produces one temperature per call, so it has to choose a
//! block. Until 2026-08-21 it used block 0 unconditionally, which silently
//! substituted the coldest spectrum for every liquid request.
//!
//! # Methodology and measured results (2026-08-21)
//!
//! **Si-in-SiC (MAT 43), kernel against the published evaluation.** Regenerate
//! the incoherent-inelastic `S(alpha, beta)` from the embedded
//! `tsl-SiinSiC.leapr` deck at each of the 8 temperatures the deck declares
//! (300, 400, 500, 600, 700, 800, 1000, 1200 K) and compare every tabulated
//! point against `reference-data/endf/tsl-SiinSiC.endf`, the ENDF/B-VIII.0
//! evaluation the deck produced. Pass criterion: every point with
//! `S_tape > 1e-12` agrees within 1 %.
//!
//! Result: **306,033 points compared across the 8 temperatures, worst deviation
//! 0.0000 %** — agreement to the 6 significant digits ENDF stores, at every
//! temperature. `T_eff` matches the evaluation exactly at all 8 (421.3892 K at
//! 300 K rising to 1234.096 K at 1200 K). Interpretation: this port's LEAPR
//! inelastic path (frequency → phonon expansion → `endout`) reproduces a
//! published evaluation for a two-sublattice compound, which is a stronger
//! statement than the self-consistency checks the module previously carried.
//!
//! **H-in-H2O (MAT 1), block selection.** Its deck declares 18 temperatures and
//! re-specifies the spectrum at each. Requesting 293.6 K — a temperature the
//! deck *declares* — previously used block 0's 283.6 K spectrum. Measured effect
//! on the published-evaluation comparison in
//! `leapr_h2o_secondary_scatterer.rs`:
//!
//! | Quantity | Published | block 0 (old) | correct block (now) |
//! |---|---|---|---|
//! | `T_eff` | 1194.3 K | 1195.35 K (+0.09 %) | **1194.33 K (+0.0028 %)** |
//! | σ_inel(0.0253 eV) | 52.10 b/H | 52.408 (+0.59 %) | **52.105 (+0.01 %)** |
//! | σ_inel(1 eV) | 21.713 b/H | 21.847 (+0.62 %) | **21.713 (+0.00 %)** |
//! | σ_inel(8 eV) | 20.707 b/H | 20.835 (+0.62 %) | **20.707 (+0.00 %)** |
//!
//! The residual ~0.6 % that file previously attributed to generation-versus-
//! evaluation differences was entirely this defect.

use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{locate_deck, SabMaterial};
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::reference_data::reference_endf_or_skip;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use njoy_outram_park_fork::units::Temperature;
use njoy_outram_park_fork::NjoyError;
use uom::si::thermodynamic_temperature::kelvin;

fn deck(material: SabMaterial) -> LeaprDeck {
    LeaprDeck::parse(&locate_deck(material).expect("deck is embedded").text)
        .expect("deck parses")
}

/// The crystalline decks declare one spectrum and inherit it; the three liquid
/// decks re-specify it per temperature. Pins which is which, so that a future
/// deck edit that changes a deck's character cannot slip past unnoticed.
#[test]
fn only_the_liquid_decks_respecify_their_spectrum_per_temperature() {
    let mut respecify = Vec::new();
    for m in SabMaterial::all() {
        let Ok(located) = locate_deck(m) else { continue };
        let Ok(d) = LeaprDeck::parse(&located.text) else {
            continue;
        };
        if d.ntempr() > 1 && !d.temperatures.iter().skip(1).all(|b| b.inherited) {
            respecify.push(format!("{m:?}"));
        }
    }
    respecify.sort();
    assert_eq!(
        respecify,
        vec!["DInD2O".to_string(), "HInH2O".to_string(), "OInD2O".to_string()],
        "exactly the three liquid decks re-specify their spectrum per temperature"
    );
}

/// An off-grid temperature stays legal on a deck that inherits one spectrum.
///
/// This is the HTR-10 use case the module doc advertises (graphite at 523 K, a
/// temperature no evaluation tabulates) and the SiC case the TRISO work uses at
/// 293.6 K. Neither temperature is declared by its deck, and both must keep
/// working: the spectrum is temperature-independent, so only `tev` changes.
#[test]
fn off_grid_temperature_is_allowed_when_every_block_inherits() {
    for (m, t_k) in [
        (SabMaterial::CrystallineGraphite, 523.0),
        (SabMaterial::SiInSiC, 293.6),
        (SabMaterial::CInSiC, 293.6),
    ] {
        let d = deck(m);
        assert!(
            !d.temperatures_k().iter().any(|t| (t - t_k).abs() < 1e-6),
            "{m:?} must not declare {t_k} K, or this test proves nothing"
        );
        generate_tape(
            &d,
            Temperature::new::<kelvin>(t_k),
            ElasticChannel::Generate,
        )
        .unwrap_or_else(|e| panic!("{m:?} at {t_k} K must still generate: {e:?}"));
    }
}

/// An off-grid temperature is **refused** on a deck whose spectrum is
/// per-temperature, rather than silently answered from another temperature's
/// spectrum.
///
/// 295 K sits between H-in-H2O's declared 293.6 K and 300 K, farther than
/// NJOY's `T/1000 + 5` match tolerance from neither... it is in fact *within*
/// tolerance of both, so the nearest is taken. 337 K is the genuinely off-grid
/// case: the deck declares 323.6 K and 350 K, both more than 5.3 K away.
#[test]
fn off_grid_temperature_is_refused_when_the_spectrum_is_per_temperature() {
    let d = deck(SabMaterial::HInH2O);
    let err = generate_tape(
        &d,
        Temperature::new::<kelvin>(337.0),
        ElasticChannel::Omit,
    )
    .expect_err("337 K is off-grid for a per-temperature-spectrum deck and must be refused");
    assert!(
        matches!(err, NjoyError::NotPorted(_)),
        "expected NotPorted explaining the refusal, got {err:?}"
    );
}

/// Requesting a temperature the deck **declares** uses that block's spectrum.
///
/// Verified through `T_eff`, the scalar most sensitive to the spectrum: the
/// published evaluation reports 1194.3 K, block 0's 283.6 K spectrum yields
/// 1195.35 K (+0.09 %), and the correct 293.6 K block yields 1194.33 K
/// (+0.0028 %). Asserting better than 0.02 % separates the two unambiguously
/// while leaving room for arithmetic drift.
#[test]
fn a_declared_temperature_uses_its_own_block() {
    let d = deck(SabMaterial::HInH2O);
    assert!(
        d.temperatures_k().iter().any(|t| (t - 293.6).abs() < 1e-6),
        "H-in-H2O declares 293.6 K"
    );
    let tape = generate_tape(
        &d,
        Temperature::new::<kelvin>(293.6),
        ElasticChannel::Omit,
    )
    .expect("H-in-H2O regenerates at a declared temperature");
    let ii = parse_mf7_at_temperature(&tape, SabMaterial::HInH2O.mat(), Some(293.6))
        .expect("parse MF=7")
        .incoherent_inelastic
        .expect("H2O has MT=4");
    let teff = ii.teff_table.first().map(|(_, e)| *e).expect("teff table");

    const PUBLISHED_TEFF_K: f64 = 1194.3;
    let rel = (teff - PUBLISHED_TEFF_K).abs() / PUBLISHED_TEFF_K;
    assert!(
        rel < 2.0e-4,
        "T_eff = {teff} K must match the evaluation's {PUBLISHED_TEFF_K} K to better than \
         0.02 % (got {:.4} %). Block 0's 283.6 K spectrum gives 1195.35 K / +0.09 %, so a \
         failure here means the wrong temperature block was selected.",
        rel * 100.0
    );
}

/// Si-in-SiC: the regenerated kernel reproduces the ENDF/B-VIII.0 evaluation at
/// every temperature the deck declares. See the module docs for the full
/// methodology and results.
///
/// Data-gated: skips (does not fail) when the reference tape is absent, per the
/// crate's no-tape-inside-crates rule.
#[test]
fn si_in_sic_kernel_reproduces_the_published_evaluation_at_every_temperature() {
    let Some(path) = reference_endf_or_skip("tsl-SiinSiC.endf", "Si-in-SiC kernel validation")
    else {
        return;
    };
    let tape = njoy_outram_park_fork::endf::Tape::read(
        std::fs::File::open(&path).expect("open the reference tape"),
    )
    .expect("reference tape parses");

    let d = deck(SabMaterial::SiInSiC);
    let mat = SabMaterial::SiInSiC.mat();
    let mut total_points = 0usize;
    let mut worst = 0.0f64;

    for t_k in d.temperatures_k() {
        let published = parse_mf7_at_temperature(&tape, mat, Some(t_k))
            .expect("parse the reference tape")
            .incoherent_inelastic
            .expect("MAT 43 has MT=4");
        let regenerated = {
            let t = generate_tape(&d, Temperature::new::<kelvin>(t_k), ElasticChannel::Generate)
                .unwrap_or_else(|e| panic!("Si-in-SiC regenerates at {t_k} K: {e:?}"));
            parse_mf7_at_temperature(&t, mat, Some(t_k))
                .expect("parse the regenerated tape")
                .incoherent_inelastic
                .expect("regenerated tape has MT=4")
        };

        assert_eq!(
            published.beta.len(),
            regenerated.beta.len(),
            "beta grid length must match at {t_k} K"
        );

        for (bp, br) in published.s_tables.iter().zip(&regenerated.s_tables) {
            for (ia, &s_pub) in bp.s.iter().enumerate() {
                let Some(&s_reg) = br.s.get(ia) else { continue };
                if s_pub > 1e-12 {
                    total_points += 1;
                    let rel = (s_reg - s_pub).abs() / s_pub;
                    if rel > worst {
                        worst = rel;
                    }
                    assert!(
                        rel < 0.01,
                        "S(alpha={}, beta={}) at {t_k} K: regenerated {s_reg:.6e} vs published \
                         {s_pub:.6e} ({:.4} %, budget 1 %)",
                        bp.alpha[ia],
                        bp.beta,
                        rel * 100.0
                    );
                }
            }
        }
    }

    assert!(
        total_points > 300_000,
        "expected >300k comparable points across the 8 temperatures, got {total_points}"
    );
    eprintln!(
        "[Si-in-SiC] {total_points} points compared across {} temperatures, worst {:.6} %",
        d.ntempr(),
        100.0 * worst
    );
}
