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

    println!(
        "\nPublished infinite-dilution reference points (independent of NJOY):\n  \
         U-238 capture RI_inf = 275.7 b (ENDF/B-VIII.0; experiment 277 +/- 3 b)\n  \
         U-235 fission RI_inf ~ 275 b, U-235 capture RI_inf ~ 144 b\n  \
         Doppler broadening conserves resonance area, so a 600 K reconstruction\n  \
         must still reproduce these."
    );
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
