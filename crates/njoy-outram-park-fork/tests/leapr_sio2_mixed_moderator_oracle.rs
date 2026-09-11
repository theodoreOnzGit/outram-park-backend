//! LEAPR mixed-moderator (`nss = 1`, `b7 = 0`) verification against an
//! NJOY2016 LEAPR run — the `copys` / `ssm` merge / second-`T_eff` path.
//!
//! Oracle: upstream NJOY2016 (`ac5adf5`, 2016.79, gfortran 13.3.0) `leapr`
//! run on the crate's own embedded `tsl-SiO2-alpha.leapr` deck (Si in
//! α-quartz, MAT 47, with oxygen as a short-collision-time secondary
//! scatterer: card 6 `1 0 15.862 7.4975 1`), unmodified;
//! `reference-data/leapr/tsl-SiO2-alpha-njoy2016-leapr.{endf,njoy-input}`.
//! The deck declares 5 temperatures (293.6, 350, 400, 500, 800 K) and
//! carries a second set of 5 temperature blocks for the oxygen, which LEAPR
//! runs with `alpha / arat`, `arat = aws/awr = 0.570` (`leapr.f90:328`).
//!
//! **Prediction** (stated before measuring): if the second pass and the
//! merge `ssm = (sbs/sb) ssm_O + ssm_Si` (`leapr.f90:3013-3025`) are ported
//! faithfully, every tabulated `S(alpha, beta)` agrees with the oracle to
//! the tape's 7 printed figures (a few 1e-7 relative), at all five
//! temperatures; the first effective-temperature TAB1 is the principal's
//! `T_eff` (508.3 K at 293.6 K per the NJOY listing) and the second the
//! secondary's (486.4 K). A wrong `srat`, or the passes swapped, moves
//! `S` by O(10 %) or more, and a missing second pass leaves the second
//! TAB1 absent — none of which a 1e-5 budget tolerates.
//!
//! Measured results are recorded in `reference-data/leapr/README.md`.

use njoy_outram_park_fork::endf::records::SectionCursor;
use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::leapr::deck::LeaprDeck;
use njoy_outram_park_fork::leapr::decks::{locate_deck, SabMaterial};
use njoy_outram_park_fork::leapr::generate::{generate_tape, ElasticChannel};
use njoy_outram_park_fork::leapr::vintage::PhysicalConstants;
use njoy_outram_park_fork::reference_data::reference_file_or_skip;
use njoy_outram_park_fork::thermr::mf7::parse_mf7_at_temperature;
use njoy_outram_park_fork::units::Temperature;
use uom::si::thermodynamic_temperature::kelvin;

const MAT: i32 = 47;
/// The tape stores 7 significant figures (`sigfig(s, 7, 0)`, `leapr.f90:3521`),
/// so two faithful computations differ by at most a rounding ulp, ~5e-7.
const S_TOL: f64 = 1e-5;
const TEFF_TOL_K: f64 = 1e-3;

/// The embedded deck, regenerated with the constants NJOY2016 `ac5adf5`
/// carries (`phys.f90:22`, `bk = 8.617333262e-5`). The deck's `EVAL` date
/// predates NJOY's CODATA change, so the inferred vintage would be the
/// legacy `bk = 8.617385e-5` — right for the *published* tape, wrong for
/// this rerun (measured: `T_eff` -4e-6, lambda +1.1e-5 with the legacy set).
fn deck() -> LeaprDeck {
    LeaprDeck::parse(
        &locate_deck(SabMaterial::SiO2Alpha)
            .expect("deck is embedded")
            .text,
    )
    .expect("deck parses")
    .with_constants(PhysicalConstants::Codata2018)
}

/// Every effective-temperature TAB1 that trails the S(alpha, beta) tables of
/// an MF=7/MT=4 section, in file order (`leapr.f90:3578-3617`: two for a
/// mixed moderator, one otherwise).
fn trailing_teff_tables(tape: &Tape, mat: i32) -> Vec<Vec<(f64, f64)>> {
    let sec = tape.section(mat, 7, 4).expect("MF=7/MT=4 present");
    let mut cur = SectionCursor::new(&sec.rows);
    let _head = cur.read_cont().unwrap();
    let _b = cur.read_list().unwrap();
    let tab2 = cur.read_tab2().unwrap();
    let nbt = tab2.head.n2 as usize;
    for _ in 0..nbt {
        let t = cur.read_tab1().unwrap();
        for _ in 0..t.head.l1 {
            cur.read_list().unwrap();
        }
    }
    let mut out = Vec::new();
    while cur.remaining() > 0 {
        out.push(cur.read_tab1().unwrap().pairs);
    }
    out
}

#[test]
fn sio2_alpha_deck_parses_both_scatterers_temperature_blocks() {
    let d = deck();
    assert!(
        d.is_mixed_moderator(),
        "card 6 `1 0 15.862 7.4975 1` is nss=1, b7=0"
    );
    assert_eq!(d.ntempr(), 5);
    assert_eq!(
        d.secondary_temperatures.len(),
        5,
        "cards 10-19 read a second time"
    );
    assert!(
        d.unsupported_features().is_empty(),
        "{:?}",
        d.unsupported_features()
    );
    let i2 = d.input_at_secondary(0, 293.6).unwrap();
    assert!((i2.arat - 15.862 / 27.84423).abs() < 1e-12);
    // The oxygen spectrum is its own, not a copy of the silicon one.
    assert_ne!(i2.continuous.rho, d.input_at(0).unwrap().continuous.rho);
}

#[test]
fn sio2_alpha_mixed_moderator_reproduces_njoy_leapr_at_every_temperature() {
    let Some(path) = reference_file_or_skip(
        "leapr",
        "tsl-SiO2-alpha-njoy2016-leapr.endf",
        "leapr-sio2-mixed-moderator",
    ) else {
        return;
    };
    let njoy = Tape::read_file(&path).expect("NJOY LEAPR tape parses");
    let d = deck();

    let njoy_teff = trailing_teff_tables(&njoy, MAT);
    assert_eq!(
        njoy_teff.len(),
        2,
        "NJOY writes principal and secondary T_eff"
    );

    let mut total_points = 0usize;
    let mut worst = (0.0f64, String::new());
    let mut tail_worst = (0.0f64, String::new());
    let mut teff_worst = (0.0f64, String::new());
    for (it, t_k) in d.temperatures_k().into_iter().enumerate() {
        let oracle = parse_mf7_at_temperature(&njoy, MAT, Some(t_k))
            .expect("parse the NJOY tape")
            .incoherent_inelastic
            .expect("MAT 47 has MT=4");
        let ours_tape = generate_tape(&d, Temperature::new::<kelvin>(t_k), ElasticChannel::Omit)
            .unwrap_or_else(|e| panic!("Si in SiO2 regenerates at {t_k} K: {e:?}"));
        let ours = parse_mf7_at_temperature(&ours_tape, MAT, Some(t_k))
            .expect("parse the regenerated tape")
            .incoherent_inelastic
            .expect("regenerated tape has MT=4");

        assert_eq!(oracle.beta.len(), ours.beta.len(), "beta grid at {t_k} K");
        for (bo, br) in oracle.s_tables.iter().zip(&ours.s_tables) {
            assert_eq!(
                bo.s.len(),
                br.s.len(),
                "alpha grid at beta={} {t_k} K",
                bo.beta
            );
            for (ia, (&s_njoy, &s_ours)) in bo.s.iter().zip(&br.s).enumerate() {
                if s_njoy <= 1e-30 {
                    // NJOY's `smin` floor; both sides zero it.
                    assert!(
                        s_ours <= 1e-30,
                        "S floor at alpha={} beta={} {t_k} K",
                        bo.alpha[ia],
                        bo.beta
                    );
                    continue;
                }
                total_points += 1;
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
                            "S(alpha={}, beta={}) at {t_k} K: ours {s_ours:.7e} njoy {s_njoy:.7e}",
                            bo.alpha[ia], bo.beta
                        ),
                    );
                }
            }
        }

        // Both effective temperatures: principal first, secondary second.
        let ours_teff = trailing_teff_tables(&ours_tape, MAT);
        assert_eq!(ours_teff.len(), 2, "two T_eff TAB1s at {t_k} K");
        for (k, (nj, us)) in njoy_teff.iter().zip(&ours_teff).enumerate() {
            let (t_nj, teff_nj) = nj[it];
            let (t_us, teff_us) = us[0];
            assert!(
                (t_nj - t_us).abs() < 1e-6,
                "T column of T_eff table {k} at {t_k} K"
            );
            let d = (teff_nj - teff_us).abs();
            if d > teff_worst.0 {
                teff_worst = (
                    d,
                    format!("table {k} at {t_k} K: ours {teff_us} njoy {teff_nj}"),
                );
            }
        }
        eprintln!(
            "[SiO2-alpha {t_k} K] T_eff principal {} K, secondary {} K (NJOY {} / {}); S worst so far {:.3e}",
            ours_teff[0][0].1, ours_teff[1][0].1, njoy_teff[0][it].1, njoy_teff[1][it].1, worst.0
        );
    }
    eprintln!(
        "[SiO2-alpha] worst T_eff deviation {:.4} K at {}",
        teff_worst.0, teff_worst.1
    );
    eprintln!(
        "[SiO2-alpha] {total_points} S(alpha,beta) points over {} temperatures, worst (S>1e-12) {:.3e} at {}; tail (S<=1e-12) worst {:.3e} at {}",
        d.ntempr(),
        worst.0,
        worst.1,
        tail_worst.0,
        tail_worst.1
    );
    assert!(
        total_points > 40_000,
        "expected >40k comparable points, got {total_points}"
    );
    assert!(worst.0 < S_TOL, "worst {:.3e} at {}", worst.0, worst.1);
    assert!(
        teff_worst.0 < TEFF_TOL_K,
        "worst T_eff deviation {:.4} K at {}",
        teff_worst.0,
        teff_worst.1
    );
}

/// Diagnostic: the `contin`-stage effective temperature and Debye-Waller
/// lambda of both scatterers at 293.6 K, against NJOY's `iprint=1` listing
/// (`start`, `leapr.f90:719-723`: principal 508.341 K / 1.893760, secondary
/// 486.448 K / 2.063704).
#[test]
fn sio2_alpha_contin_stage_values_vs_njoy_listing() {
    use njoy_outram_park_fork::leapr::frequency::FrequencyModel;
    let d = deck();
    eprintln!(
        "[deck] EVAL date {:?}, inferred constants {:?}, using {:?}",
        d.evaluation_date(),
        d.evaluation_date()
            .map(PhysicalConstants::for_evaluation_date),
        d.constants()
    );
    let t_k = 293.6;
    for (label, input, teff_njoy, lambda_njoy) in [
        (
            "principal",
            d.input_at_temperature(0, t_k).unwrap(),
            508.341,
            1.893760,
        ),
        (
            "secondary",
            d.input_at_secondary(0, t_k).unwrap(),
            486.448,
            2.063704,
        ),
    ] {
        let f = FrequencyModel::start(
            &input.continuous.rho,
            input.continuous.delta_ev,
            input.tev(),
            input.continuous.tbeta,
        );
        eprintln!(
            "[{label}] ni={} delta={} rho[0]={} rho[last]={} tev={} tbeta={} twt={} nd={} -> T_eff {:.4} (njoy {teff_njoy}) lambda {:.6} (njoy {lambda_njoy})",
            input.continuous.rho.len(),
            input.continuous.delta_ev,
            input.continuous.rho[0],
            input.continuous.rho[input.continuous.rho.len() - 1],
            input.tev(),
            input.continuous.tbeta,
            input.continuous.twt,
            input.oscillators.len(),
            f.tbar * t_k,
            f.f0
        );
    }
}
