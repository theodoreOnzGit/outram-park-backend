//! Measure the **infinite-dilution resonance integral** of the reconstructed
//! evaluation, and compare it against NJOY2016's own PENDF *and* against the
//! published value.
//!
//! # Why this exists, when `u238_vs_njoy_pendf` already agrees to 0.04 %
//!
//! That oracle samples **19 discrete probe energies** — resonance peak centres,
//! the URR band, and fast. It pins the *peak heights*. It cannot see the thing
//! that actually drives resonance escape: the **area** under each resonance.
//!
//! A grid that is too coarse between the nodes loses area without moving any
//! node value, so it passes a point-wise comparison and still under-captures in
//! transport. Transport looks σ up by lin-lin interpolation on **our** grid, so
//!
//! ```text
//! RI_ours = ∫ σ_γ(E) dE/E   evaluated on our own grid
//! ```
//!
//! *is* the effective resonance integral the Monte Carlo actually sees. That is
//! what this program integrates, exactly (segment-wise: a lin-lin σ over a 1/E
//! weight has the closed form `a·ln(E₁/E₀) + b·(E₁−E₀)`), on four grids:
//!
//! | quantity | what a shortfall means |
//! |---|---|
//! | `RI_ours`  (our σ, our grid)     | what transport sees |
//! | `RI_njoy`  (NJOY σ, NJOY grid)   | the reference |
//! | `RI_ours_on_njoy_grid`           | our σ is fine, our **grid** is too coarse |
//! | `RI_njoy_on_our_grid`            | our grid is too coarse for NJOY's σ too |
//!
//! The four-way split separates *data* from *grid density*, which the point
//! oracle structurally cannot do.
//!
//! # External oracle — independent of NJOY
//!
//! The infinite-dilution capture resonance integral of U-238 is a measured,
//! published quantity: **RI_∞ = 275.7 b** for ENDF/B-VIII.0 (evaluated;
//! experiment 277 ± 3 b), conventionally `∫ σ_γ dE/E` from the 0.5 eV cadmium
//! cutoff upward. It is essentially temperature-independent at infinite dilution
//! (Doppler broadening conserves the area under a resonance to high accuracy),
//! so a 600 K reconstruction must reproduce it. For U-235 the reference points
//! are RI_f ≈ 275 b and RI_γ ≈ 144 b.
//!
//! # Running
//!
//! ```text
//! U238_PENDF=/home/user/u238oracle/tape22 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases --example u238_resonance_integral
//!
//! NJOY_MAT=9228 NJOY_TAPE=n-092_U_235.endf NJOY_NUCLIDE=U235 \
//! U238_PENDF=/home/user/u235oracle/tape22 cargo run --release \
//!     -p outram-mc-libs --features endf-pebble-cases --example u238_resonance_integral
//! ```
//!
//! The NJOY deck that produces `tape22` is in `examples/u238_vs_njoy_pendf.rs`.
//! Without `U238_PENDF` the NJOY columns are skipped and the published
//! comparison still runs.

use njoy_outram_park_fork::endf::tape::Tape;
use njoy_outram_park_fork::groupr::panel::PointwiseXs;
use njoy_outram_park_fork::groupr::pendf_feed::read_pendf_cross_section;
use njoy_outram_park_fork::reference_data::reference_endf;
use outram_mc_libs::material::nuclide::Nuclide;

const TEMP: f64 = 600.0;

/// Bands to integrate over \[eV\]. The last is the conventional RI definition.
/// The resolved resonances that carry the resonance integral, each with enough
/// width either side to include the wings the self-shielding depends on.
const WINDOWS: &[(f64, f64, &str)] = &[
    (6.0, 7.5, "6.674 eV"),
    (19.5, 22.5, "20.87 eV"),
    (35.0, 38.5, "36.68 eV"),
    (64.0, 68.0, "66.03 eV"),
    (100.0, 105.0, "102.6 eV"),
    (180.0, 220.0, "189/208 eV"),
];

const BANDS: &[(f64, f64, &str)] = &[
    (0.5, 10.0, "0.5 eV - 10 eV   (6.67 eV resonance)"),
    (10.0, 100.0, "10 eV  - 100 eV  (20.9/36.7/66/102 eV)"),
    (100.0, 1.0e3, "100 eV - 1 keV"),
    (1.0e3, 2.0e4, "1 keV  - 20 keV  (resolved tail)"),
    (2.0e4, 1.0e5, "20 keV - 100 keV (unresolved)"),
    (0.5, 1.0e5, "0.5 eV - 100 keV  <- compare to RI_inf"),
];

fn main() {
    let mat: i32 = std::env::var("NJOY_MAT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9237);
    let tape_name = std::env::var("NJOY_TAPE").unwrap_or_else(|_| "n-092_U_238.endf".into());
    let nuc_name = std::env::var("NJOY_NUCLIDE").unwrap_or_else(|_| "U238".into());

    let tape = reference_endf(&tape_name).expect("ENDF tape");
    eprintln!("reconstructing {nuc_name} (MAT {mat}) @ {TEMP} K, tol 1e-3 ...");
    let ours = Nuclide::from_endf_file(&tape, &nuc_name, TEMP, 1.0e-3).expect("reconstruction");
    eprintln!("done.\n");

    // NJOY's own PENDF, if one was supplied.
    let njoy_tape = std::env::var("U238_PENDF").ok().map(|p| {
        eprintln!("reading NJOY PENDF {p} ...");
        Tape::read_file(std::path::Path::new(&p)).expect("PENDF parses")
    });

    // (MT, label, how to read it back out of our MicroXS)
    let reactions: [(i32, &str); 2] = [(102, "capture"), (18, "fission")];

    // Our RI and NJOY's over 0.5 eV - 100 keV, kept for the V&V gate below.
    let mut ri_capture_full_band: Option<(f64, f64)> = None;

    for (mt, label) in reactions {
        let njoy_pairs: Option<Vec<(f64, f64)>> =
            njoy_tape
                .as_ref()
                .and_then(|t| match read_pendf_cross_section(t, mat, mt) {
                    Ok(x) => match x.xs {
                        PointwiseXs::LinLin(p) => Some((*p).clone()),
                        _ => None,
                    },
                    Err(_) => None,
                });

        println!("\n======== MT={mt}  {label}  ({nuc_name}) ========");
        if let Some(p) = &njoy_pairs {
            println!("NJOY PENDF grid: {} points total", p.len());
        } else {
            println!("NJOY PENDF: not available (set U238_PENDF) - our columns only");
        }
        println!(
            "\n{:<34} {:>9} {:>9} {:>12} {:>12} {:>12} {:>12}",
            "band", "n_ours", "n_njoy", "RI_ours", "RI_njoy", "ours@njoyG", "njoy@oursG"
        );

        for &(lo, hi, name) in BANDS {
            let our_grid = ours.native_energy_grid(lo, hi);
            let our_sig = |e: f64| {
                let x = ours.xs_at_energy(e, TEMP);
                if mt == 18 {
                    x.fission
                } else {
                    x.absorption - x.fission
                }
            };

            let ri_ours = integrate_1_over_e(&our_grid, our_sig);

            let (n_njoy, ri_njoy, ri_ours_on_njoy, ri_njoy_on_ours) = match &njoy_pairs {
                Some(pairs) => {
                    let njoy_grid = clip_grid(pairs.iter().map(|&(e, _)| e), lo, hi);
                    let njoy_sig = |e: f64| interp_linlin(pairs, e);
                    (
                        njoy_grid.len(),
                        integrate_1_over_e(&njoy_grid, njoy_sig),
                        integrate_1_over_e(&njoy_grid, our_sig),
                        integrate_1_over_e(&our_grid, njoy_sig),
                    )
                }
                None => (0, f64::NAN, f64::NAN, f64::NAN),
            };

            println!(
                "{name:<34} {:>9} {:>9} {ri_ours:>12.4} {ri_njoy:>12.4} {ri_ours_on_njoy:>12.4} {ri_njoy_on_ours:>12.4}",
                our_grid.len(),
                n_njoy
            );
            if mt == 102 && (lo, hi) == (0.5, 1.0e5) {
                ri_capture_full_band = Some((ri_ours, ri_njoy));
            }
        }

        if let Some(pairs) = &njoy_pairs {
            let (lo, hi) = (0.5, 1.0e5);
            let our_grid = ours.native_energy_grid(lo, hi);
            let njoy_grid = clip_grid(pairs.iter().map(|&(e, _)| e), lo, hi);
            let our_sig = |e: f64| {
                let x = ours.xs_at_energy(e, TEMP);
                if mt == 18 {
                    x.fission
                } else {
                    x.absorption - x.fission
                }
            };
            let a = integrate_1_over_e(&our_grid, our_sig);
            let b = integrate_1_over_e(&njoy_grid, |e| interp_linlin(pairs, e));
            println!(
                "\n  RI(0.5 eV - 100 keV): ours {a:.4} b   NJOY {b:.4} b   -> {:+.2} %",
                100.0 * (a - b) / b
            );
        }
    }

    // ── Resonance SHAPE, which the integral cannot see ────────────────────────
    //
    // A resonance integral is invariant under Doppler broadening: it is the
    // AREA, and broadening conserves it. So a resonance reconstructed too narrow
    // and too tall passes the integral check exactly, passes a probe at the peak
    // only if the peak height happens to match, and still **over-self-shields**
    // in transport -- less absorption, higher p, higher k. That is the shape of
    // the ring-RPT residual, so it has to be excluded on shape and not on area.
    //
    // Compared on NJOY's OWN grid points inside each window, so nothing is
    // interpolated on our side except at NJOY's nodes.
    if let Some(t) = njoy_tape.as_ref() {
        if let Ok(x) = read_pendf_cross_section(t, mat, 102) {
            if let PointwiseXs::LinLin(pairs) = x.xs {
                println!("\n======== MT=102 capture: resonance SHAPE on NJOY's own grid ========");
                println!(
                    "{:<22} {:>8} {:>14} {:>14} {:>11} {:>11}",
                    "window [eV]",
                    "points",
                    "peak NJOY [b]",
                    "peak ours [b]",
                    "worst rel",
                    "rms rel"
                );
                for &(lo, hi, label) in WINDOWS {
                    let (mut worst, mut sum_sq, mut n) = (0.0_f64, 0.0_f64, 0usize);
                    let (mut peak_n, mut peak_o) = (0.0_f64, 0.0_f64);
                    for &(e, sn) in pairs.iter().filter(|&&(e, _)| e >= lo && e <= hi) {
                        let so = {
                            let v = ours.xs_at_energy(e, TEMP);
                            v.absorption - v.fission
                        };
                        if sn > 1.0 {
                            let rel = (so - sn) / sn;
                            if rel.abs() > worst.abs() {
                                worst = rel;
                            }
                            sum_sq += rel * rel;
                            n += 1;
                        }
                        if sn > peak_n {
                            peak_n = sn;
                        }
                        if so > peak_o {
                            peak_o = so;
                        }
                    }
                    if n == 0 {
                        continue;
                    }
                    println!(
                        "{label:<22} {n:>8} {peak_n:>14.4e} {peak_o:>14.4e} {:>10.3}% {:>10.3}%",
                        100.0 * worst,
                        100.0 * (sum_sq / n as f64).sqrt()
                    );
                }
                println!(
                    "\n  Shape and area are independent: broadening moves the peak and the\n  \
                     wings while conserving the integral, so agreement HERE plus agreement\n  \
                     on the integral together pin sigma_gamma(E) completely over the band\n  \
                     that matters for resonance escape."
                );
            }
        }
    }

    println!(
        "\nPublished infinite-dilution reference points (independent of NJOY):\n  \
         U-238 capture RI_inf = 275.7 b (ENDF/B-VIII.0; experiment 277 +/- 3 b)\n  \
         U-235 fission RI_inf ~ 275 b, U-235 capture RI_inf ~ 144 b\n  \
         Doppler broadening conserves resonance area, so a 600 K reconstruction\n  \
         must still reproduce these."
    );

    if nuc_name == "U238" {
        vv_gate(ri_capture_full_band);
    } else {
        println!(
            "\n(No V&V gate for {nuc_name}: the published reference points asserted \
             below are U-238's.)"
        );
    }
}

/// U-238's published infinite-dilution capture resonance integral, barns.
///
/// `RI_inf = int sigma_gamma dE/E` from the 0.5 eV cadmium cutoff upward.
/// **275.7 b** is the ENDF/B-VIII.0 evaluated value; the measured quantity is
/// **277 +/- 3 b**. It is essentially temperature-independent at infinite
/// dilution, because Doppler broadening conserves the area under a resonance to
/// high accuracy — so a 600 K reconstruction must reproduce it.
const U238_CAPTURE_RI_INF_B: f64 = 275.7;

/// V&V gate: the resonance integral against two independent oracles.
///
/// # Why the integral and not the point cross section
///
/// `examples/u238_vs_njoy_pendf.rs` already pins sigma_gamma at 19 probe
/// energies to 0.3 %. That pins the **peak heights**, and it structurally cannot
/// see the thing that drives resonance escape: the **area** under each
/// resonance. A grid too coarse between the nodes loses area without moving any
/// node value, so it passes a point-wise comparison and still under-captures in
/// transport. This gate closes that gap.
///
/// # The oracles
///
/// 1. **Published RI_inf = 275.7 b** (ENDF/B-VIII.0 evaluated; experiment
///    277 +/- 3 b). Independent of NJOY entirely — it is a tabulated physical
///    quantity, so this half of the gate runs with no oracle tape on disk.
/// 2. **NJOY2016's own PENDF**, when `U238_PENDF` points at one. That comparison
///    additionally separates *data* from *grid density*, via the four-way split
///    printed above.
///
/// # Results (2026-09-11, ENDF/B-VIII.0 @ 600 K, tol 1e-3)
///
/// `RI(0.5 eV - 100 keV) = 274.637 b` on this crate's own reconstruction and its
/// own grid — which is what transport actually integrates, since transport looks
/// sigma up by lin-lin interpolation on that same grid.
///
/// Against the published 275.7 b that is **-0.39 %**, and well inside the
/// experimental 277 +/- 3 b (1.1 %). Against NJOY's own PENDF on NJOY's own grid
/// it was **+0.00 %**.
///
/// # Tolerance
///
/// **2 %** against the published value. That is not slack for this crate: it is
/// the room the *comparison* needs. RI_inf depends on the cadmium-cutoff
/// convention (0.5 eV here), on the upper limit, and on the evaluation revision,
/// and the experimental value itself carries +/- 3 b (1.1 %). Asserting tighter
/// than the oracle's own spread would produce failures that say nothing about
/// the code. The NJOY comparison, which has none of those ambiguities because
/// both sides use the same convention, is gated 40x tighter at 0.5 %.
fn vv_gate(ri_capture_full_band: Option<(f64, f64)>) {
    use outram_mc_libs::vv::assert_relative;

    println!("\n=== V&V gate: U-238 capture resonance integral ===");

    let Some((ri_ours, ri_njoy)) = ri_capture_full_band else {
        panic!(
            "the 0.5 eV - 100 keV capture band produced no resonance integral. \
             That band is the last row of BANDS and the one the published RI_inf \
             is quoted over; without it this program has measured nothing."
        );
    };

    assert!(
        ri_ours.is_finite() && ri_ours > 0.0,
        "RI over 0.5 eV - 100 keV came out as {ri_ours}, which is not a cross \
         section. A reconstruction grid coarser than the resonance widths can do \
         this."
    );

    assert_relative(
        "RI(0.5 eV - 100 keV) vs the published infinite-dilution value",
        ri_ours,
        U238_CAPTURE_RI_INF_B,
        0.02,
    );

    if ri_njoy.is_finite() && ri_njoy > 0.0 {
        assert_relative(
            "RI(0.5 eV - 100 keV) vs NJOY2016's own PENDF, on NJOY's own grid",
            ri_ours,
            ri_njoy,
            0.005,
        );
    } else {
        println!(
            "  [skip] no NJOY PENDF supplied (set U238_PENDF), so only the \
             published-value half of this gate ran. The deck that produces the \
             tape is in examples/u238_vs_njoy_pendf.rs."
        );
    }
}

/// `∫ σ(E) dE/E` over an ascending grid, exactly for a lin-lin σ: on each
/// segment σ = a + bE, so the integral is `a·ln(E₁/E₀) + b·(E₁−E₀)`.
///
/// Taking the endpoint values from `sigma` and treating the segment as linear is
/// exactly what transport does when it interpolates on that same grid — so the
/// result is the resonance integral the transport kernel actually sees.
fn integrate_1_over_e<F: Fn(f64) -> f64>(grid: &[f64], sigma: F) -> f64 {
    if grid.len() < 2 {
        return 0.0;
    }
    let mut acc = 0.0;
    let mut s0 = sigma(grid[0]);
    for w in grid.windows(2) {
        let (e0, e1) = (w[0], w[1]);
        let s1 = sigma(e1);
        if e1 > e0 && e0 > 0.0 {
            let b = (s1 - s0) / (e1 - e0);
            let a = s0 - b * e0;
            acc += a * (e1 / e0).ln() + b * (e1 - e0);
        }
        s0 = s1;
    }
    acc
}

/// The grid points of an ascending source inside `[lo, hi]`, with both endpoints
/// pinned — the same contract as `Nuclide::native_energy_grid`, so the two grids
/// are compared on equal terms.
fn clip_grid<I: Iterator<Item = f64>>(src: I, lo: f64, hi: f64) -> Vec<f64> {
    let mut g: Vec<f64> = src
        .filter(|e| e.is_finite() && *e > lo && *e < hi)
        .collect();
    g.insert(0, lo);
    g.push(hi);
    g.dedup_by(|a, b| (*a - *b).abs() <= 1e-12 * b.abs().max(1.0));
    g
}

/// Linear interpolation on an ascending `(E, σ)` table; zero outside it,
/// matching `gety1`.
fn interp_linlin(pairs: &[(f64, f64)], e: f64) -> f64 {
    if pairs.is_empty() || e < pairs[0].0 || e > pairs[pairs.len() - 1].0 {
        return 0.0;
    }
    let i = match pairs.binary_search_by(|p| p.0.partial_cmp(&e).unwrap()) {
        Ok(i) => return pairs[i].1,
        Err(i) => i,
    };
    let (e0, s0) = pairs[i - 1];
    let (e1, s1) = pairs[i];
    if e1 == e0 {
        return s1;
    }
    s0 + (s1 - s0) * (e - e0) / (e1 - e0)
}
