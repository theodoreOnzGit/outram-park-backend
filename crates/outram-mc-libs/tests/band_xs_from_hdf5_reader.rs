// SPDX-License-Identifier: GPL-3.0

//! **The band-averaged oracle, recomputed in Rust from the `.h5`** — GitHub #303.
//!
//! `band_averaged_xs_vs_openmc.rs` already makes #303's row-1 comparison — our
//! ENDF-reconstructed cross sections against OpenMC's own, band-averaged with a
//! flux-free `dE/E` weight. What it does **not** do is read the `.h5` itself:
//! its oracle numbers are produced by
//! `verification_and_validation/openmc_godiva_cross_code/band_xs_oracle.py` and
//! **transcribed by hand** into that test's `const` arrays.
//!
//! A transcribed oracle is a real weak point, and this workspace has been
//! bitten by one before: the `transport_decomposition.py` nearest-point bug
//! biased `<mu_el>` to 0.2740 against a true 0.2645, and it was the **fourth**
//! appearance of that trap. A number that reaches a test by passing through a
//! script and a human is a number nothing re-derives.
//!
//! `njoy_outram_park_fork::hdf5::nuclide_read` closes that: the oracle can now
//! be computed from the library **in Rust, in-process**. This test does exactly
//! that and checks it against the transcribed values, so:
//!
//! - it verifies the new reader on a **real fissile nuclide's** cross sections,
//!   against numbers produced by an independent tool (h5py + numpy), and
//! - it puts a gate under the transcription, so a future edit to either side
//!   cannot drift unnoticed.
//!
//! It deliberately does **not** modify `band_averaged_xs_vs_openmc.rs`. That
//! test carries a careful argument about which instrument is meaningful in the
//! resolved-resonance region, and rewiring its oracle is a separate change from
//! showing the oracle can be re-derived.
//!
//! # Why the comparison is band-averaged and not pointwise
//!
//! Taken from that test, because the reasoning is the same and worth not
//! restating wrongly: a **pointwise** comparison of two independently generated
//! energy grids inside resonances is dominated by grid placement, not physics,
//! so it cannot exclude anything. The lethargy average
//! `<sigma> = int sigma dE/E / int dE/E` needs no flux, so it also cannot be
//! contaminated by the flux difference such a comparison is usually used to
//! explain.
//!
//! # Results, 2026-09-24
//!
//! Printed by the test.

use njoy_outram_park_fork::hdf5::nuclide_read::{read_nuclide, ReadNuclide};

/// Matching `band_averaged_xs_vs_openmc.rs`'s `BANDS` and `NSUB` exactly, so
/// the two are comparable.
const BANDS: &[(f64, f64)] = &[
    (2.0e4, 6.7e4),
    (6.7e4, 1.74e5),
    (1.74e5, 6.0e5),
    (6.0e5, 1.9e6),
    (1.9e6, 3.0e6),
    (3.0e6, 6.0e6),
];
const NSUB: usize = 4000;

/// `[elastic, fission, capture, inelastic, sum]` per band, transcribed from
/// `band_xs_oracle.py` — the same constants `band_averaged_xs_vs_openmc.rs`
/// carries. This test's job is to re-derive them.
const OPENMC_U235: &[[f64; 5]] = &[
    [11.017651, 1.987839, 0.647687, 0.029321, 13.682498],
    [9.622001, 1.543524, 0.431340, 0.376125, 11.972990],
    [6.888111, 1.237761, 0.218033, 1.190966, 9.534871],
    [3.986001, 1.187215, 0.103090, 1.793700, 7.070005],
    [4.072335, 1.263977, 0.045174, 2.191125, 7.572610],
    [4.405270, 1.123938, 0.011639, 2.237270, 7.778117],
];
const OPENMC_U238: &[[f64; 5]] = &[
    [13.053376, 0.000078, 0.390016, 0.040916, 13.484387],
    [11.019992, 0.000079, 0.174228, 0.578911, 11.773210],
    [8.018997, 0.000238, 0.114975, 1.390207, 9.524417],
    [4.311844, 0.129384, 0.100363, 2.684440, 7.226032],
    [3.842433, 0.542298, 0.033442, 3.138706, 7.556880],
    [4.341587, 0.552582, 0.006346, 2.884852, 7.785367],
];

fn xs_dir() -> Option<std::path::PathBuf> {
    let p = std::path::PathBuf::from(std::env::var("OUTRAM_OPENMC_XS_DIR").ok()?);
    p.join("U235.h5").exists().then_some(p)
}

/// Lethargy average of `f` over `[lo, hi]` — trapezoid in `ln E`, i.e. the
/// `dE/E` weight. Identical to the sibling test's, on purpose.
fn band_average(lo: f64, hi: f64, f: impl Fn(f64) -> f64) -> f64 {
    let (l0, l1) = (lo.ln(), hi.ln());
    let mut acc = 0.0;
    let mut prev = f(lo);
    for i in 1..NSUB {
        let ln = l0 + (l1 - l0) * i as f64 / (NSUB - 1) as f64;
        let v = f(ln.exp());
        acc += 0.5 * (v + prev) * (l1 - l0) / (NSUB - 1) as f64;
        prev = v;
    }
    acc / (l1 - l0)
}

/// Lin-lin interpolation of a cross section read off the file, on the file's own
/// grid. This is what OpenMC itself does between grid points, so it is the
/// right evaluation for reproducing an oracle computed from the same arrays.
fn sigma(n: &ReadNuclide, grid: &[f64], mt: i32, e: f64) -> f64 {
    let Some(rx) = n.reactions.get(&mt) else {
        return 0.0;
    };
    if e <= grid[0] || e >= *grid.last().unwrap() {
        return 0.0;
    }
    let i = grid.partition_point(|&v| v <= e) - 1;
    // Below the threshold the reaction does not occur.
    if i < rx.threshold_idx {
        return 0.0;
    }
    let (j0, j1) = (i - rx.threshold_idx, i + 1 - rx.threshold_idx);
    if j1 >= rx.xs.len() {
        return 0.0;
    }
    let f = (e - grid[i]) / (grid[i + 1] - grid[i]);
    rx.xs[j0] + f * (rx.xs[j1] - rx.xs[j0])
}

/// Sum over the discrete inelastic levels and the continuum, which is what the
/// oracle's "inelastic" column is: MT=51..91.
fn inelastic(n: &ReadNuclide, grid: &[f64], e: f64) -> f64 {
    (51..=91).map(|mt| sigma(n, grid, mt, e)).sum()
}

fn check(file: &str, oracle: &[[f64; 5]]) -> Option<f64> {
    let d = xs_dir()?;
    let p = d.join(file);
    if !p.exists() {
        println!("SKIP {file}: not present in OUTRAM_OPENMC_XS_DIR");
        return None;
    }
    let n = read_nuclide(&p).expect("the reference library must read");
    let grid = n.any_energy().expect("an energy grid").clone();

    println!(
        "\n{file}: band-averaged sigma (barn), Rust reader vs the transcribed \
         h5py oracle"
    );
    println!(
        "{:>9} {:>9} {:>10} {:>12} {:>12} {:>9}",
        "band lo", "band hi", "channel", "rust reader", "oracle", "diff %"
    );
    let labels = ["elastic", "fission", "capture", "inelastic", "sum"];
    let mut worst = 0.0f64;

    for (b, &(lo, hi)) in BANDS.iter().enumerate() {
        let el = band_average(lo, hi, |e| sigma(&n, &grid, 2, e));
        let fi = band_average(lo, hi, |e| sigma(&n, &grid, 18, e));
        let ca = band_average(lo, hi, |e| sigma(&n, &grid, 102, e));
        let inl = band_average(lo, hi, |e| inelastic(&n, &grid, e));
        let ours = [el, fi, ca, inl, el + fi + ca + inl];
        for (k, label) in labels.iter().enumerate() {
            let theirs = oracle[b][k];
            if theirs < 1.0e-6 {
                continue; // a channel closed here says nothing
            }
            let rel = (ours[k] - theirs) / theirs * 100.0;
            worst = worst.max(rel.abs());
            println!(
                "{lo:9.3e} {hi:9.3e} {label:>10} {:12.6} {theirs:12.6} {rel:9.3}",
                ours[k]
            );
        }
    }
    Some(worst)
}

/// **THE GATE: the Rust reader reproduces the h5py-generated oracle.**
///
/// Both sides read the *same* `.h5` arrays, so this is a test of the reader and
/// the band integral, not of any physics — which is exactly what makes a tight
/// bound the right instrument here. Any real disagreement means the reader has
/// mis-assembled a cross section: a wrong threshold offset, the wrong MT, or a
/// grid misalignment.
///
/// The 2 % bound is not tight to the 4000-point trapezoid's own error; it is
/// set to catch a *structural* reader defect (an off-by-one in the threshold
/// offset moves a band average by far more than 2 %) without failing on
/// quadrature differences between this integral and the oracle script's. The
/// measured worst case is printed, so a reader can see the real margin rather
/// than infer it from the bound.
#[test]
fn the_rust_reader_reproduces_the_band_averaged_oracle() {
    let mut ran = 0;
    for (file, oracle) in [("U235.h5", OPENMC_U235), ("U238.h5", OPENMC_U238)] {
        let Some(worst) = check(file, oracle) else {
            continue;
        };
        ran += 1;
        println!("{file}: worst |diff| = {worst:.3} %");
        assert!(
            worst < 2.0,
            "{file}: the Rust reader disagrees with the h5py oracle by {worst:.3} %, \
             which is a structural defect rather than quadrature -- check the \
             threshold offset, the MT selection and the grid alignment"
        );
    }
    // A SKIP MUST NOT READ AS A PASS.
    if ran == 0 {
        eprintln!(
            "SKIP the_rust_reader_reproduces_the_band_averaged_oracle: set \
             OUTRAM_OPENMC_XS_DIR to a directory holding U235.h5 / U238.h5 built \
             by NJOY2016 ACE -> openmc.data.IncidentNeutron.from_ace -> \
             export_to_hdf5. That library is a generated ~74 MB artefact and is \
             deliberately not committed."
        );
    }
}
