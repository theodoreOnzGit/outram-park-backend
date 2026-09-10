//! LEAPR D-in-D2O verification against an NJOY2016 LEAPR run — the Sköld
//! intermolecular-coherence correction (`nsk = 2`, `skold`,
//! `leapr.f90:2816-2862`) on top of `contin` + `trans` + `discre`.
//!
//! Oracle: upstream NJOY2016 (`ac5adf5`, 2016.79, gfortran 13.3.0) `leapr`
//! run on the crate's embedded `tsl-DinD2O.leapr` deck (CAB model, MAT 11,
//! `EVAL-JUN17`, card 5 `1.9968 3.395 2 0 0 2`: `nsk = 2`, no secondary)
//! cut to its 293.6 K block (`ntempr = 1`; 396 alphas x 396 betas,
//! `nphon = 200`), with the S(kappa) table and `cfrac` of cards 17-19;
//! `reference-data/leapr/tsl-DinD2O-293.6K-njoy2016-leapr.{endf,njoy-input}`.
//! NJOY's listing: `T_eff` 394.719 K after `contin`, 391.784 K after
//! `trans`, 865.561 K after `discre` (unchanged by `skold`); lambda 3.322120.
//!
//! **Prediction** (stated before measuring): with CODATA-2018 constants (the
//! deck's `EVAL-JUN17` would select the legacy `bk`), a faithful `skold`
//! gives every tabulated `S(alpha, beta)` to the tape's 7 printed figures.
//! Without the correction the law is off by O(`cfrac`) wherever
//! `S(kappa) != 1`; a wrong bracket or interpolation law shows up as
//! scattered 1e-3..1e-1 deviations at the alphas where `alpha / S(kappa)`
//! falls between grid points.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{locate_deck, SabMaterial};
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

const MAT: i32 = 11;
const T_K: f64 = 293.6;
const S_TOL: f64 = 1e-5;
const TEFF_NJOY_K: f64 = 865.561;

#[test]
fn d_in_d2o_with_skold_reproduces_njoy_leapr_at_293_6_k() {
    let Some(path) = reference_file_or_skip(
        "leapr",
        "tsl-DinD2O-293.6K-njoy2016-leapr.endf",
        "leapr-d2o-njoy-oracle",
    ) else {
        return;
    };
    let njoy = Tape::read_file(&path).expect("NJOY LEAPR tape parses");
    let oracle = parse_mf7_at_temperature(&njoy, MAT, Some(T_K))
        .expect("parse the NJOY tape")
        .incoherent_inelastic
        .expect("MAT 11 has MT=4");

    let d = LeaprDeck::parse(
        &locate_deck(SabMaterial::DInD2O)
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
        .expect("D in D2O regenerates at 293.6 K");
    let ours = parse_mf7_at_temperature(&ours_tape, MAT, Some(T_K))
        .expect("parse the regenerated tape")
        .incoherent_inelastic
        .expect("regenerated tape has MT=4");
    eprintln!("[D2O] regenerated in {:.2?}", t0.elapsed());

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
        "[D2O] {total} S(alpha,beta) points, worst (S>1e-12) {:.3e} at {}; tail worst {:.3e} at {}; T_eff ours {teff_ours} njoy {teff_njoy} (listing {TEFF_NJOY_K})",
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
