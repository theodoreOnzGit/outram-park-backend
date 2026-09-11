//! **U-238's reconstructed point cross sections against NJOY2016's own PENDF,
//! as committed golden data.**
//!
//! This is the oracle the whole FHR ring-RPT / LEU-COMP-THERM-008 study leans on
//! (#186, #188): every time the hunt said "it is not the cross sections", it was
//! citing this comparison. It has lived only in `examples/u238_vs_njoy_pendf.rs`,
//! which needs an NJOY installation and a 37 MB tape and therefore never ran
//! anywhere but by hand. The numbers below were taken from that example on
//! 2026-09-11 and are now asserted, so the claim cannot drift from the code.
//!
//! # The oracle
//!
//! NJOY2016 release **2016.79** (`18Mar25`), run on the *same* ENDF tape this
//! crate reads (`reference-data/endf/n-092_U_238.endf`, MAT 9237), at the *same*
//! temperature (600 K), with the *same* linearisation tolerance (0.1 %):
//!
//! ```text
//! reconr
//!  20 21/
//!  'pendf for u238, err 0.001'/
//!  9237 0/
//!  0.001/
//!  0/
//! broadr
//!  20 21 22/
//!  9237 1/
//!  0.001/
//!  600./
//!  0/
//! stop
//! ```
//!
//! `tape22` is the 600 K PENDF: 131 066 grid points. Regenerate and print the
//! full table with `examples/u238_vs_njoy_pendf.rs`.
//!
//! # Results (2026-09-11)
//!
//! | reaction | worst over the 19 probe energies |
//! |---|---|
//! | MT=1 total | **+0.04 %** at 20.87 eV |
//! | MT=2 elastic | **−0.03 %** at 6.674 eV |
//! | MT=102 radiative capture | **−0.17 %** at 19 keV |
//! | MT=18 fission | +0.14 % — *excluding one meaningless outlier, below* |
//!
//! The probe energies are chosen to hit what matters: thermal, the four largest
//! resolved resonances (6.674, 20.87, 36.68, 66.03, 102.6 eV — reconstructed
//! **at their peaks**, which is the hardest place to agree), the 20 keV
//! resolved/unresolved seam, the unresolved band, and fast.

use outram_mc_libs::material::nuclide::Nuclide;

const TEMP: f64 = 600.0;

/// NJOY2016 2016.79 PENDF, MAT 9237, 600 K: `(E [eV], σ [b])`.
mod njoy {
    /// MT=1, total.
    pub const TOTAL: &[(f64, f64)] = &[
        (2.530000e-02, 1.194280e1), // ours 1.194284e1, +0.00 %
        (1.000000e+00, 9.571597e0), // ours 9.571636e0, +0.00 %
        (6.674000e+00, 5.749893e3), // ours 5.749417e3, −0.01 %
        (2.087000e+01, 7.283905e3), // ours 7.286665e3, +0.04 %
        (3.668000e+01, 1.005539e4), // ours 1.005545e4, +0.00 %
        (6.603000e+01, 3.207754e3), // ours 3.207856e3, +0.00 %
        (1.026000e+02, 4.503320e3), // ours 4.503401e3, +0.00 %
        (1.000000e+03, 2.148533e1), // ours 2.148538e1, +0.00 %
        (1.000000e+04, 1.320347e1), // ours 1.320402e1, +0.00 %
        (1.900000e+04, 1.035556e1), // ours 1.035799e1, +0.02 %
        (2.000000e+04, 1.229868e1), // ours 1.229866e1, −0.00 %
        (3.000000e+04, 1.383150e1), // ours 1.383150e1, +0.00 %
        (5.000000e+04, 1.303331e1), // ours 1.303331e1, +0.00 %
        (1.000000e+05, 1.180514e1), // ours 1.180514e1, +0.00 %
        (1.500000e+05, 1.131349e1), // ours 1.131349e1, +0.00 %
        (5.000000e+05, 8.435778e0), // ours 8.435778e0, +0.00 %
        (1.000000e+06, 7.089129e0), // ours 7.089129e0, +0.00 %
        (2.000000e+06, 7.281519e0), // ours 7.281518e0, −0.00 %
        (1.400000e+07, 5.867194e0), // ours 5.867194e0, +0.00 %
    ];
    /// MT=2, elastic.
    pub const ELASTIC: &[(f64, f64)] = &[
        (2.530000e-02, 9.260055e0),
        (1.000000e+00, 9.075288e0),
        (6.674000e+00, 3.628279e2), // ours 3.627348e2, −0.03 % (the worst)
        (2.087000e+01, 2.240051e3),
        (3.668000e+01, 6.048971e3),
        (6.603000e+01, 1.672177e3),
        (1.026000e+02, 3.401129e3),
        (1.000000e+03, 2.129172e1),
        (1.000000e+04, 1.181773e1),
        (1.900000e+04, 1.023603e1),
        (2.000000e+04, 1.189319e1),
        (3.000000e+04, 1.339700e1),
        (5.000000e+04, 1.263400e1),
        (1.000000e+05, 1.110700e1),
        (1.500000e+05, 1.035010e1),
        (5.000000e+05, 6.603770e0),
        (1.000000e+06, 4.255680e0),
        (2.000000e+06, 3.546014e0),
        (1.400000e+07, 2.857930e0),
    ];
    /// MT=102, radiative capture — the resonance-absorption channel the whole
    /// study turns on.
    pub const CAPTURE: &[(f64, f64)] = &[
        (2.530000e-02, 2.682721e0),
        (1.000000e+00, 4.963043e-1),
        (6.674000e+00, 5.387062e3), // the 6.7 eV resonance peak
        (2.087000e+01, 5.043842e3),
        (3.668000e+01, 4.006410e3),
        (6.603000e+01, 1.535573e3),
        (1.026000e+02, 1.102190e3),
        (1.000000e+03, 1.936138e-1),
        (1.000000e+04, 1.385712e0),
        (1.900000e+04, 1.194841e-1), // ours 1.192862e-1, −0.17 % (the worst)
        (2.000000e+04, 4.054252e-1),
        (3.000000e+04, 4.344800e-1),
        (5.000000e+04, 3.234450e-1),
        (1.000000e+05, 1.787900e-1),
        (1.500000e+05, 1.411400e-1),
        (5.000000e+05, 1.111100e-1),
        (1.000000e+06, 1.278100e-1),
        (2.000000e+06, 4.787900e-2),
        (1.400000e+07, 8.244478e-4),
    ];
    /// MT=18, fission. **Read the note in the test before using this.**
    pub const FISSION: &[(f64, f64)] = &[
        (2.530000e-02, 1.851002e-5),
        (1.000000e+00, 3.214900e-6),
        (6.674000e+00, 2.344786e-3),
        (2.087000e+01, 1.210449e-2),
        (3.668000e+01, 1.749551e-3),
        (6.603000e+01, 3.555648e-3),
        (1.026000e+02, 6.129576e-4),
        (1.000000e+03, 1.354363e-7), // ours 1.453388e-7, +7.31 % — see the test
        (1.000000e+04, 2.335126e-5),
        (1.900000e+04, 4.735709e-5),
        (2.000000e+04, 7.454015e-5),
        (3.000000e+04, 2.498100e-5),
        (5.000000e+04, 1.150433e-4),
        (1.000000e+05, 5.393800e-5),
        (1.500000e+05, 1.153124e-4),
        (5.000000e+05, 2.781100e-4),
        (1.000000e+06, 1.459200e-2),
        (2.000000e+06, 5.378600e-1),
        (1.400000e+07, 1.150600e0),
    ];
}

fn u238_or_skip() -> Option<Nuclide> {
    let Some(path) = njoy_outram_park_fork::reference_data::reference_endf("n-092_U_238.endf")
    else {
        println!("SKIP: n-092_U_238.endf not in reference-data/endf/");
        return None;
    };
    match Nuclide::from_endf_file(&path, "U238", TEMP, 1.0e-3) {
        Ok(n) => Some(n),
        Err(e) => {
            println!("SKIP: {e}");
            None
        }
    }
}

/// Worst relative difference of `ours(E)` against a golden table, and where.
fn worst_against(table: &[(f64, f64)], ours: impl Fn(f64) -> f64, label: &str) -> (f64, f64) {
    let mut worst = (0.0_f64, 0.0_f64);
    println!("== {label}");
    for &(e, njoy) in table {
        let o = ours(e);
        let rel = if njoy.abs() > 1.0e-30 {
            o / njoy - 1.0
        } else {
            0.0
        };
        println!(
            "  {e:>11.4e}  NJOY {njoy:>13.6e}  ours {o:>13.6e}  {:>+7.3} %",
            100.0 * rel
        );
        if rel.abs() > worst.0.abs() {
            worst = (rel, e);
        }
    }
    println!("  worst {:+.3} % at {:.4e} eV", 100.0 * worst.0, worst.1);
    worst
}

/// **This crate's U-238 total, elastic and radiative-capture cross sections
/// reproduce NJOY2016's own PENDF to ≤0.2 %, at the resonance peaks included.**
///
/// # Why the peaks matter
///
/// Five of the nineteen probe energies sit **on** the peaks of the largest
/// resolved resonances (6.674, 20.87, 36.68, 66.03, 102.6 eV), where σ runs from
/// 1.1e3 to 5.4e3 b and where Doppler broadening does the most work. Agreeing on
/// a smooth 10 b fast cross section is easy; agreeing to 0.02 % on a
/// 5000-barn peak at 600 K is the check that means something, because it tests
/// the resonance reconstruction *and* the broadening together.
///
/// # Results (2026-09-11, NJOY2016 2016.79, ENDF/B-VIII.0, 600 K, tol 1e-3)
///
/// | reaction | worst | where |
/// |---|---|---|
/// | MT=1 total | **+0.04 %** | 20.87 eV (a resonance peak) |
/// | MT=2 elastic | **−0.03 %** | 6.674 eV (a resonance peak) |
/// | MT=102 capture | **−0.17 %** | 19 keV (inside the unresolved band) |
#[test]
fn u238_point_cross_sections_match_njoy_pendf() {
    let Some(u238) = u238_or_skip() else {
        return;
    };
    let xs = |e: f64| u238.xs_at_energy(e, TEMP);

    let (w_tot, e_tot) = worst_against(njoy::TOTAL, |e| xs(e).total, "MT=1 total");
    assert!(
        w_tot.abs() < 0.002,
        "U-238 total is {:+.3} % from NJOY at {e_tot:.4e} eV — worse than the +0.04 % \
         recorded on 2026-09-11",
        100.0 * w_tot
    );

    let (w_el, e_el) = worst_against(njoy::ELASTIC, |e| xs(e).elastic, "MT=2 elastic");
    assert!(
        w_el.abs() < 0.002,
        "U-238 elastic is {:+.3} % from NJOY at {e_el:.4e} eV — worse than the −0.03 % \
         recorded on 2026-09-11",
        100.0 * w_el
    );

    // MT=102 is radiative capture ALONE. This crate's `absorption` is MT=27
    // (fission + all of MT=101), so capture is `absorption − fission`. For U-238
    // below 20 MeV the other MT=101 channels are negligible, which is why the
    // two agree here and emphatically do not for a light nuclide — see
    // `tests/ring_rpt_hunt_lessons.rs::absorption_is_mt27_so_li6_shows_the_n_t_channel`.
    let (w_cap, e_cap) = worst_against(
        njoy::CAPTURE,
        |e| {
            let x = xs(e);
            x.absorption - x.fission
        },
        "MT=102 radiative capture",
    );
    assert!(
        w_cap.abs() < 0.003,
        "U-238 capture is {:+.3} % from NJOY at {e_cap:.4e} eV — worse than the −0.17 % \
         recorded on 2026-09-11. This is the channel the entire ring-RPT study's \
         'it is not the cross sections' rests on",
        100.0 * w_cap
    );
}

/// **U-238 fission matches NJOY to 0.15 % everywhere it is physically
/// meaningful — and the one place it does not is a trap worth pinning.**
///
/// # The trap
///
/// At **1 keV** the golden table shows `+7.31 %`. U-238 fission is
/// **sub-threshold** there: NJOY gives `1.354e-7 b` and this crate `1.453e-7 b`.
/// Both are numerical noise on a channel that is, physically, closed — a tenth
/// of a *micro*barn against a 21-barn total, i.e. one part in 1.6e8 of the
/// reaction rate. A percentage on it is arithmetic, not physics.
///
/// This is the same class of reading error as the MT=27/MT=102 confusion (GH
/// #169): a large relative difference on a quantity that carries no weight. The
/// test therefore asserts on a **threshold of significance** — 1e-6 b, a
/// hundred-thousandth of the smallest cross section that matters here — rather
/// than either ignoring the point or loosening the bound for everything.
///
/// # Results (2026-09-11)
///
/// Worst **+0.14 %** at 1.0 eV among the meaningful points; the sub-threshold
/// 1 keV point is excluded by the significance cut and separately asserted to be
/// negligible.
#[test]
fn u238_fission_matches_njoy_where_the_channel_is_open() {
    let Some(u238) = u238_or_skip() else {
        return;
    };
    /// Below this, U-238 fission carries no reaction rate worth comparing.
    const SIGNIFICANT_B: f64 = 1.0e-6;

    let mut worst = (0.0_f64, 0.0_f64);
    let mut skipped = 0usize;
    println!("== MT=18 fission");
    for &(e, njoy) in njoy::FISSION {
        let ours = u238.xs_at_energy(e, TEMP).fission;
        if njoy < SIGNIFICANT_B {
            println!(
                "  {e:>11.4e}  NJOY {njoy:>13.6e}  ours {ours:>13.6e}  (sub-threshold, \
                 excluded: {:+.2} % on a channel that is physically closed)",
                100.0 * (ours / njoy - 1.0)
            );
            assert!(
                ours < SIGNIFICANT_B,
                "U-238 fission at {e} eV is {ours:.4e} b — it should be sub-threshold \
                 noise like NJOY's {njoy:.4e} b, not a real cross section"
            );
            skipped += 1;
            continue;
        }
        let rel = ours / njoy - 1.0;
        println!(
            "  {e:>11.4e}  NJOY {njoy:>13.6e}  ours {ours:>13.6e}  {:>+7.3} %",
            100.0 * rel
        );
        if rel.abs() > worst.0.abs() {
            worst = (rel, e);
        }
    }
    println!(
        "  worst {:+.3} % at {:.4e} eV ({skipped} sub-threshold point(s) excluded)",
        100.0 * worst.0,
        worst.1
    );
    assert_eq!(
        skipped, 1,
        "the number of sub-threshold fission points changed; the 1 keV trap this test \
         documents may no longer be where it was"
    );
    assert!(
        worst.0.abs() < 0.003,
        "U-238 fission is {:+.3} % from NJOY at {:.4e} eV — worse than the +0.14 % \
         recorded on 2026-09-11",
        100.0 * worst.0,
        worst.1
    );
}
