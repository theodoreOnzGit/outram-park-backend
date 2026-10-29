//! `bwr_mark1` against the digitised BSAF Unit 1 figures (NEA No. 7525,
//! Figs 2.1, 2.2, 2.5; `crates/kovan-literature/derived/nea7525-bsaf-unit1-figures/`).
//!
//! **Methodology** (instruments fixed in `bwr_mark1::compare` before any
//! digitised curve was seen, 2026-10-29):
//! - **Measured series** (Fig 2.1a RPV pressure, Fig 2.1b drywell
//!   pressure): the model, sampled every 60 s to 26 h with every mechanism
//!   on, is interpolated at each measured time inside the run; reported
//!   are the count, the mean and the largest absolute difference, MPa. The
//!   drywell file's four points marked `upper-suspect` are excluded, as its
//!   README says.
//! - **Code families** (Figs 2.1a, 2.1b, 2.2, 2.5a Xe, 2.5b I; nine codes
//!   each): the fraction of the times 0, 0.25, ... 25 h at which the model
//!   lies within the codes' envelope (at least two codes covering the time).
//!   These are code predictions, not measurements: a differential test,
//!   not a validation.
//!
//! **Results** (2026-10-29, the model at `eca21cc`, the data at `8067264`):
//! in the test's doc comment, with the interpretation; pinned.
//!
//! Acknowledgement: OECD Nuclear Energy Agency, *Benchmark Study of the
//! Accident at the Fukushima Daiichi Nuclear Power Plant*, NEA No. 7525,
//! © OECD 2021. Facts and numbers only.

use sembawang::bwr_mark1::compare::{against_measured, histories, inside_envelope, Histories};
use sembawang::bwr_mark1::params::Params;
use sembawang::bwr_mark1::Mechanisms;
use std::path::PathBuf;

fn data_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kovan-literature/derived/nea7525-bsaf-unit1-figures")
}

/// `time_h,value[,basis]` rows, skipping the header and any row whose
/// basis contains `suspect`.
fn read_series(file: &str) -> Vec<(f64, f64)> {
    let text = std::fs::read_to_string(data_dir().join(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
    text.lines()
        .skip(1)
        .filter(|l| !l.contains("suspect"))
        .map(|l| {
            let mut c = l.split(',');
            (c.next().unwrap().parse().unwrap(), c.next().unwrap().parse().unwrap())
        })
        .collect()
}

const CODES: [&str; 9] = ["ciemat", "iae", "ibrae", "irsn", "jaea", "kaeri", "nra", "snl", "vtt"];

fn family(fig: &str) -> Vec<Vec<(f64, f64)>> {
    CODES.iter().map(|c| read_series(&format!("fig{fig}-{c}.csv"))).collect()
}

fn model() -> Histories {
    histories(Mechanisms::default(), Params::default(), 60.0, 26.0 * 3600.0)
}

/// The model's histories against the digitised figures.
///
/// **Results, 2026-10-29** (the first run; pinned below):
///
/// | Check | Result |
/// |---|---|
/// | Measured RPV pressure (2 points) | mean 3.634, max 6.712 MPa |
/// | Measured drywell pressure (33 points) | mean 0.338, max 0.509 MPa |
/// | Inside the 9-code envelope: RPV / drywell / H2 | 3.3 % / 17.6 % / 9.2 % of times |
/// | Inside the 9-code envelope: Xe / I release | 54.2 % / 93.8 % of times |
///
/// **Interpretation.**
/// - **The RPV was measured at 7.00 MPa at 5.34 h; the model is at 0.29
///   MPa.** Its main steam line has already ruptured by creep (4.78 h),
///   so the measurement contradicts the rupture's timing. The rupture
///   rests on \[A\] inputs (line size and steel, the dome internals) and
///   on an upper-bound gas temperature. For context only, not adopted:
///   with `without_msl_creep` the model is at 4.25 MPa at 5.34 h (mean
///   1.63 MPa over the two points), still low, because the \[A\] hot
///   leak depressurises it. Both leak paths need plant sources (F13).
/// - **The drywell is 0.24-0.51 MPa low over 10-23 h** (0.34 against
///   0.85 MPa at 11.7 h; 0.53 against 0.77 MPa at 22.9 h), with or without
///   the steam-line rupture. ~~Candidates, to test, not to tune: the
///   drywell heat sink is three \[A\] placeholders (4000 m^2, 20 W/(m^2
///   K), 5e8 J/K); no MCCI gas; the pool over-condenses once near
///   saturation.~~ **Tested the same day** (diagnostic runs, nothing
///   adopted): the drywell rides the wetwell (0.342 against 0.330 MPa at
///   11.7 h; the downcomers pass anything above the submergence head),
///   and the pool is at 344 K, about 100 K subcooled, so every kilogram of
///   steam condenses and the containment pressure is the non-condensable
///   gas alone. Hence: the **heat sink** cannot carry it (a tenth of its
///   area raises the drywell 0.02-0.03 MPa); **pool over-condensation**
///   cannot act before the pool nears saturation (about 20 h); what is
///   missing is **about 6e5 mol of non-condensable gas** (0.4 MPa over
///   about 6000 m^3 of containment gas near 500 K: about 1.2 t of H2 or
///   16 t of CO). BSAF's text says the codes held the 15-24 h plateau by
///   balancing **MCCI gas** against a drywell-head flange leak (NEA No.
///   7525, p.19), and the model has no MCCI (F8, blocked on Unit 1's
///   concrete).
/// - **H2** is inside the codes' spread only 9 % of the time: it is
///   lower than all nine codes from about 5 h on (215 kg at 25 h against
///   398-1315 kg).
/// - **Release from fuel** sits inside the I envelope 94 % of the time
///   and the Xe envelope 54 %.
///
/// The measured series are the validation; the envelopes are a
/// differential test against other codes, none of which is the truth.
#[test]
fn the_model_against_the_digitised_bsaf_figures() {
    let m = model();
    let grid: Vec<f64> = (0..=100).map(|i| i as f64 * 0.25).collect();
    let rpv = against_measured(&m.t_h, &m.p_rpv_mpa, &read_series("fig2.1a-measurement.csv"));
    let dw = against_measured(&m.t_h, &m.p_dw_mpa, &read_series("fig2.1b-measurement.csv"));
    let env = |v: &[f64], fig: &str| inside_envelope(&m.t_h, v, &family(fig), &grid);
    let (e_rpv, e_dw, e_h2, e_xe, e_i) =
        (env(&m.p_rpv_mpa, "2.1a"), env(&m.p_dw_mpa, "2.1b"), env(&m.h2_kg, "2.2"), env(&m.fp_released, "2.5a"), env(&m.fp_released, "2.5b"));
    println!("measured RPV pressure: {rpv:?}");
    println!("measured drywell pressure: {dw:?}");
    println!("inside the code envelope (fraction, times counted): RPV {e_rpv:?}, drywell {e_dw:?}, H2 {e_h2:?}, Xe {e_xe:?}, I {e_i:?}");
    // Pinned to the first run: a change to the model moves these, and the
    // move is then recorded here with its old and new value.
    assert_eq!((rpv.n, dw.n), (2, 33));
    assert!((rpv.mean_abs - 3.6344).abs() < 1e-3 && (rpv.max_abs - 6.7123).abs() < 1e-3, "{rpv:?}");
    assert!((dw.mean_abs - 0.3379).abs() < 1e-3 && (dw.max_abs - 0.5092).abs() < 1e-3, "{dw:?}");
    let near = |(f, _): (f64, usize), x: f64| (f - x).abs() < 1e-3;
    assert!(near(e_rpv, 0.0330) && near(e_dw, 0.1758) && near(e_h2, 0.0920) && near(e_xe, 0.5417) && near(e_i, 0.9375));
}

