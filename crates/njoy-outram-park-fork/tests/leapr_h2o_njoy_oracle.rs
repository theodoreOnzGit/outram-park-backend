//! LEAPR H-in-H2O verification against an NJOY2016 LEAPR run — the
//! `contin` + `trans` + `discre` chain with a free-gas secondary (`b7 = 1`).
//!
//! Oracle: upstream NJOY2016 (`ac5adf5`, 2016.79, gfortran 13.3.0) `leapr`
//! run on the crate's embedded `tsl-HinH2O.leapr` deck (CAB model, MAT 1,
//! `EVAL-JUN17`) cut to its 293.6 K temperature block (`ntempr = 1`; the
//! 18-temperature tape is 18.5 MB), `nphon = 200`, 222 alphas x 317 betas,
//! translational weight `twt > 0` and two discrete oscillators;
//! `reference-data/leapr/tsl-HinH2O-293.6K-njoy2016-leapr.{endf,njoy-input}`.
//! NJOY's listing gives `T_eff` 480.905 K after `contin`, 478.107 K after
//! `trans`, 1194.341 K after `discre`, and lambda 1.724930.
//!
//! Until now this path was checked only against the *published*
//! ENDF/B-VIII.0 tape (sigma_inel to ~0.6 %, `T_eff` +0.09 %) — a tape from a
//! different LEAPR build with different constants. This is the like-for-like
//! run.
//!
//! **Prediction** (stated before measuring): with the constants NJOY2016
//! `ac5adf5` carries (`PhysicalConstants::Codata2018`; the deck's `EVAL-JUN17`
//! would otherwise select the legacy `bk`), a faithful `trans`/`discre` gives
//! every tabulated `S(alpha, beta)` to the tape's 7 printed figures and
//! `T_eff = 1194.341 K` exactly. A ~0.6 % residual would be a port defect,
//! not a vintage effect.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{locate_deck, SabMaterial};
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

const MAT: i32 = 1;
const T_K: f64 = 293.6;
const S_TOL: f64 = 1e-5;
const TEFF_NJOY_K: f64 = 1194.341;

#[test]
fn h_in_h2o_reproduces_njoy_leapr_at_293_6_k() {
    let Some(path) = reference_file_or_skip(
        "leapr",
        "tsl-HinH2O-293.6K-njoy2016-leapr.endf",
        "leapr-h2o-njoy-oracle",
    ) else {
        return;
    };
    let njoy = Tape::read_file(&path).expect("NJOY LEAPR tape parses");
    let oracle = parse_mf7_at_temperature(&njoy, MAT, Some(T_K))
        .expect("parse the NJOY tape")
        .incoherent_inelastic
        .expect("MAT 1 has MT=4");

    let d = LeaprDeck::parse(
        &locate_deck(SabMaterial::HInH2O)
            .expect("deck is embedded")
            .text,
    )
    .expect("deck parses")
    .with_constants(PhysicalConstants::Codata2018);
    assert!(
        d.unsupported_features().is_empty(),
        "{:?}",
        d.unsupported_features()
    );
    let t0 = std::time::Instant::now();
    let ours_tape = generate_tape(&d, Temperature::new::<kelvin>(T_K), ElasticChannel::Omit)
        .expect("H in H2O regenerates at 293.6 K");
    let ours = parse_mf7_at_temperature(&ours_tape, MAT, Some(T_K))
        .expect("parse the regenerated tape")
        .incoherent_inelastic
        .expect("regenerated tape has MT=4");
    eprintln!("[H2O] regenerated in {:.2?}", t0.elapsed());

    // B-list: free-gas oxygen secondary, B(7)=1, B(8)=mss*sps, B(9)=aws.
    assert_eq!(oracle.b.len(), ours.b.len(), "B-list length");
    for (k, (&bn, &bo)) in oracle.b.iter().zip(&ours.b).enumerate() {
        assert!(
            (bn - bo).abs() <= 1e-6 * bn.abs().max(1.0),
            "B({}) njoy {bn} ours {bo}",
            k + 1
        );
    }

    assert_eq!(oracle.beta.len(), ours.beta.len(), "beta grid");
    let mut total = 0usize;
    let mut worst = (0.0f64, String::new());
    let mut tail_worst = (0.0f64, String::new());
    for (bo, br) in oracle.s_tables.iter().zip(&ours.s_tables) {
        assert_eq!(bo.s.len(), br.s.len(), "alpha grid at beta={}", bo.beta);
        for (ia, (&s_njoy, &s_ours)) in bo.s.iter().zip(&br.s).enumerate() {
            if s_njoy <= 1e-37 {
                assert!(
                    s_ours <= 1e-37,
                    "S floor at alpha={} beta={}",
                    bo.alpha[ia],
                    bo.beta
                );
                continue;
            }
            total += 1;
            let rel = (s_ours - s_njoy).abs() / s_njoy;
            let slot = if s_njoy > 1e-12 {
                &mut worst
            } else {
                &mut tail_worst
            };
            if rel > slot.0 {
                *slot = (
                    rel,
                    format!(
                        "S(alpha={}, beta={}): ours {s_ours:.7e} njoy {s_njoy:.7e}",
                        bo.alpha[ia], bo.beta
                    ),
                );
            }
        }
    }
    let teff_ours = ours.teff_table[0].1;
    let teff_njoy = oracle.teff_table[0].1;
    eprintln!(
        "[H2O] {total} S(alpha,beta) points, worst (S>1e-12) {:.3e} at {}; tail worst {:.3e} at {}; T_eff ours {teff_ours} njoy {teff_njoy} (listing {TEFF_NJOY_K})",
        worst.0, worst.1, tail_worst.0, tail_worst.1
    );
    assert!(
        total > 40_000,
        "expected >40k comparable points, got {total}"
    );
    assert!(
        (teff_njoy - TEFF_NJOY_K).abs() < 1e-3,
        "oracle tape T_eff vs listing"
    );
    assert!(
        (teff_ours - teff_njoy).abs() < 1e-3,
        "T_eff ours {teff_ours} njoy {teff_njoy}"
    );
    assert!(worst.0 < S_TOL, "worst {:.3e} at {}", worst.0, worst.1);
    assert!(
        tail_worst.0 < 1e-3,
        "tail worst {:.3e} at {}",
        tail_worst.0,
        tail_worst.1
    );
}
