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



/// **Beyond the model's window: 0-500 h against Fig 2.3a's measured
/// drywell pressure** (the same instrument; diagnostic, pinned so F12 has a
/// baseline). Results, 2026-10-29: 94 measured points, mean 0.653 MPa, max
/// ~~0.818~~ 0.915 MPa (since the failed vessel's gas leaves to the drywell,
/// the same day: mean 0.6535 -> 0.6539). The model's drywell, vented on the recorded order near 24 h,
/// falls to 0.16 MPa at 30 h, then with no long-term leak path rises to a
/// 0.97 MPa plateau held from about 100 h to 500 h; the plant fell to 0.15
/// MPa by about 257 h. Every BSAF code followed the measurement by
/// imposing a variable-area containment leak from 50 h (NEA No. 7525,
/// p.19); that leak, and the injection that bypasses the core, are F12,
/// and imposing an area to follow the curve would be fitting it.
#[test]
fn the_model_beyond_its_window_against_fig_2_3a() {
    let m = histories(Mechanisms::default(), Params::default(), 600.0, 500.0 * 3600.0);
    let a = against_measured(&m.t_h, &m.p_dw_mpa, &read_series("fig2.3a-measurement.csv"));
    println!("0-500 h drywell against Fig 2.3a: {a:?}");
    assert_eq!(a.n, 94);
    assert!((a.mean_abs - 0.6539).abs() < 1e-3 && (a.max_abs - 0.9145).abs() < 1e-3, "{a:?}");
}


/// **Cesium against Fig 2.6** (gh:#1047; the envelope instrument fixed
/// before any curve was seen). Methodology: cesium released by CORSOR-M
/// (154.0 kg inventory, the benchmark specification's), carried as vapour
/// with the UO2 seed aerosol out of the vessel (no in-vessel deposition,
/// stated), through the containment by the sectional aerosol and
/// TRAP-MELT2 condensation (`rn_volume`); no pool scrubbing (SPARC-90 not
/// built; MELCOR's default with pool interaction off), so the S/C water's
/// cesium is what settles onto the pool surface. Fractions of the
/// inventory against the nine codes' envelope, 0-25 h in 0.25 h steps.
///
/// Results, 2026-10-29:
/// - **Cs in the S/C water (Fig 2.6c): inside the envelope 85.4 % of 89
///   times.** 0.064 at 10 h, 0.706 at 20 h, 0.807 at 26 h, just above the
///   highest code at the end (JAEA 0.787; IBRAE 0.000, SNL 0.33). Predicted
///   before the run: at or above the top late on, so mostly outside.
///   **Missed**: inside most of the time, crossing the top only late.
/// - **Cs on drywell structures (Fig 2.6a): inside 46.7 % of 75 times.**
///   0.081 at 10 h, then 0.000 by 20 h: the deposit re-evaporates as the
///   drywell structures heat (the ex-vessel debris's heat goes into them),
///   which five of BSAF's nine codes also show as remobilisation.
///
/// These are a differential test against codes, not a validation. The
/// result rests on \[A\] areas (drywell floor, pool surface) and on the
/// missing in-vessel deposition and pool scrubbing.
#[test]
fn cesium_against_fig_2_6() {
    let m = model();
    let grid: Vec<f64> = (0..=100).map(|i| i as f64 * 0.25).collect();
    let a = inside_envelope(&m.t_h, &m.cs_dw_structures, &family("2.6a"), &grid);
    let c = inside_envelope(&m.t_h, &m.cs_sc_water, &family("2.6c"), &grid);
    println!("Cs on drywell structures {a:?}; Cs in S/C water {c:?}");
    assert_eq!((a.1, c.1), (75, 89));
    assert!((a.0 - 0.4667).abs() < 1e-3 && (c.0 - 0.8539).abs() < 1e-3, "{a:?} {c:?}");
}


/// **Iodine against Fig 2.6b and 2.6d** (gh:#1047; same method as cesium,
/// 11.98 kg inventory, the benchmark specification's; released by the same
/// CORSOR-M row, as I2 vapour).
///
/// Results, 2026-10-29: **inside the codes' envelope 0 % of the time** for
/// both iodine on drywell structures (76 times counted) and in the S/C water
/// (89): the model keeps all 10.19 kg of released iodine airborne. At the
/// 344 K wetwell I2's saturation concentration is about 0.19 kg/m^3 against
/// a loading of about 0.004, so it never condenses. Predicted before the
/// run: near zero, inside only where the lowest codes are near zero;
/// consistent (every code keeps some iodine at every counted time). This is
/// what MELCOR's defaults give without SPARC (released iodine is I2; CsI
/// forms only when requested): iodine's retention in the codes comes from
/// pool scrubbing (SPARC-90 scrubs I2 vapour) and chemistry, neither built.
#[test]
fn iodine_against_fig_2_6() {
    let m = model();
    let grid: Vec<f64> = (0..=100).map(|i| i as f64 * 0.25).collect();
    let b = inside_envelope(&m.t_h, &m.i_dw_structures, &family("2.6b"), &grid);
    let d = inside_envelope(&m.t_h, &m.i_sc_water, &family("2.6d"), &grid);
    println!("I on drywell structures {b:?}; I in S/C water {d:?}");
    assert_eq!((b, d), ((0.0, 76), (0.0, 89)));
}
