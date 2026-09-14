//! Fingerprint the reconstructed U-238 evaluation, so two checkouts can be
//! compared without running any transport.
//!
//! # Why this exists
//!
//! The FHR ring-RPT explicit-TRISO case moved by **−1632 pcm (≈5.4σ)** between
//! commit `23cd2549` and `0cd9a22c` — same problem, same settings, same seed.
//! In the same logs, U-238 reconstruction wall time fell from **91.7 s to
//! 26.8 s**. Two candidate causes: the 33 njoy commits that landed in between
//! changed the reconstruction, or the `DeltaDomain` refactor changed the cube
//! transport path it was supposed to leave alone.
//!
//! Reading the diff cannot separate those — and in this workspace reading has a
//! poor record: of the defects found this session, every one came from running
//! an oracle and none from reading the port. So measure instead. This program
//! touches **no transport code at all**: it only reconstructs U-238 and prints
//! numbers. Run it in both checkouts and diff the output.
//!
//! - Output **identical** ⇒ the cross sections did not change ⇒ the k shift is
//!   in the transport path, i.e. the refactor, and it must be bisected.
//! - Output **differs** ⇒ the reconstruction changed ⇒ the k shift is the data,
//!   and the recorded V&V baseline is stale rather than wrong.
//!
//! The grid-size line is the most direct test of the "coarser grid" reading of
//! the 3.4× speedup; the σ values say whether any coarsening actually moved the
//! physics, which a speedup alone does not establish.
//!
//! ```text
//! cargo run --release -p outram-mc-libs --features endf-pebble-cases \
//!     --example u238_recon_fingerprint
//! ```

fn main() {
    use outram_mc_libs::material::nuclide::Nuclide;
    use std::path::Path;

    let path = Path::new("reference-data/endf/n-092_U_238.endf");
    let t0 = std::time::Instant::now();
    let u238 =
        Nuclide::from_endf_file(path, "U238", 600.0, 1e-3).expect("U-238 reconstruction failed");
    let elapsed = t0.elapsed();

    // Grid size across the whole range, and within the resolved-resonance band
    // where the resonance integral that drives a pebble's k actually lives.
    let full = u238.native_energy_grid(1.0e-5, 2.0e7);
    let rrr = u238.native_energy_grid(1.0, 2.0e4);
    let urr = u238.native_energy_grid(2.0e4, 1.5e5);
    println!("# U-238 reconstruction fingerprint @ 600 K, tol 1e-3");
    println!("recon_seconds            {:.1}", elapsed.as_secs_f64());
    println!("grid_points_total        {}", full.len());
    println!("grid_points_1eV_20keV    {}", rrr.len());
    println!("grid_points_20k_150keV   {}", urr.len());

    // Fixed probe energies: thermal, the three big low-lying U-238 capture
    // resonances, the resolved/unresolved boundary, the URR band, and fast.
    const PROBE_EV: &[f64] = &[
        0.0253, 1.0, 6.674, 20.87, 36.68, 66.03, 102.6, 1.0e3, 1.0e4, 1.9e4, 2.0e4, 3.0e4, 5.0e4,
        1.0e5, 1.5e5, 5.0e5, 1.0e6, 2.0e6, 1.4e7,
    ];
    println!("\n# energy_eV  total_b  elastic_b  capture_b  fission_b  inelastic_b");
    for &e in PROBE_EV {
        let x = u238.xs_at_energy(e, 600.0);
        let capture = x.absorption - x.fission;
        println!(
            "{e:>10.4e} {:>12.6e} {:>12.6e} {:>12.6e} {:>12.6e} {:>12.6e}",
            x.total, x.elastic, capture, x.fission, x.inelastic
        );
    }

    // A resonance-integral-like quantity: sum of capture over a 1/E weight on
    // the reconstructed grid, 1 eV to 20 keV. One number that responds to a
    // coarser grid in the way a pebble's k does, unlike a point value.
    let mut ri = 0.0;
    for w in rrr.windows(2) {
        let (a, b) = (w[0], w[1]);
        let em = (a * b).sqrt();
        let x = u238.xs_at_energy(em, 600.0);
        ri += (x.absorption - x.fission) * (b / a).ln();
    }
    println!("\ncapture_RI_1eV_20keV_b   {ri:.6}");

    vv_gate(&u238, full.len(), rrr.len(), urr.len(), ri);
}

/// V&V gate: the fingerprint, pinned.
///
/// # Why this program needs a gate at all
///
/// Its purpose is to be **compared between two checkouts** by eye, so an
/// absolute assertion looks like it fights that purpose. It does not: what the
/// program was written to detect — a silent change in the reconstruction — is
/// exactly what a pin detects, and a pin detects it without anyone having to
/// remember to run it twice and diff.
///
/// **A failure here is the signal this program exists to produce**, not
/// necessarily a defect. Something in the reconstruction moved. Find out what,
/// decide whether it was intended, and then either fix it or record the new
/// values below with the date and the reason.
///
/// # What this catches that a point-value check does not
///
/// `grid_points_1eV_20keV` is the direct test of the "coarser grid" reading of a
/// speedup — the case this program was written for was a 3.4x reconstruction
/// speedup accompanied by a -1632 pcm (5.4 sigma) shift in a pebble k. A
/// speedup alone establishes nothing; the grid count and the resonance integral
/// together say whether any coarsening actually moved the physics.
///
/// # Results (2026-09-11, ENDF/B-VIII.0 @ 600 K, tol 1e-3)
///
/// ```text
///   grid_points_total          292 286
///   grid_points_1eV_20keV      291 119
///   grid_points_20k_150keV         222
///   capture_RI_1eV_20keV_b     273.695427
/// ```
///
/// The resonance integral here is over **1 eV - 20 keV** on the reconstruction's
/// own grid, with a midpoint rule; that is deliberately a different quantity
/// from `examples/u238_resonance_integral.rs`'s 274.637 b, which runs
/// 0.5 eV - 100 keV with the exact lin-lin quadrature. They are not meant to
/// match, and neither is a check on the other.
///
/// # Tolerances
///
/// The resonance integral is pinned at **0.5 %** — loose enough for grid
/// reshuffling that does not move the physics, tight enough that a real change
/// in resonance area fails. The grid counts are pinned at **20 %**, which is a
/// deliberately coarse "order of magnitude" bar: it catches a grid that halved
/// or doubled, which is what a reconstruction-algorithm change looks like, and
/// ignores the few-hundred-point jitter of a tolerance-driven adaptive grid.
fn vv_gate(
    u238: &outram_mc_libs::material::nuclide::Nuclide,
    n_total: usize,
    n_rrr: usize,
    n_urr: usize,
    ri: f64,
) {
    use outram_mc_libs::vv::assert_reproduces_recorded;

    /// Recorded 2026-09-11: `int sigma_gamma dE/E` over 1 eV - 20 keV on this
    /// crate's own reconstruction grid, midpoint rule.
    const RECORDED_RI_B: f64 = 273.695_427;
    const RECORDED_GRID_TOTAL: f64 = 292_286.0;
    const RECORDED_GRID_RRR: f64 = 291_119.0;
    const RECORDED_GRID_URR: f64 = 222.0;

    println!(
        "
=== V&V gate: reconstruction fingerprint ==="
    );

    assert_reproduces_recorded(
        "capture RI over 1 eV - 20 keV, on this crate's own grid",
        ri,
        RECORDED_RI_B,
        0.005,
    );
    assert_reproduces_recorded(
        "reconstructed grid points, whole range",
        n_total as f64,
        RECORDED_GRID_TOTAL,
        0.20,
    );
    assert_reproduces_recorded(
        "reconstructed grid points, 1 eV - 20 keV (the resolved resonances)",
        n_rrr as f64,
        RECORDED_GRID_RRR,
        0.20,
    );
    assert_reproduces_recorded(
        "reconstructed grid points, 20 - 150 keV (unresolved)",
        n_urr as f64,
        RECORDED_GRID_URR,
        0.20,
    );

    // The 6.674 eV resonance peak is the single most load-bearing number in the
    // whole pebble study: it dominates the resonance integral, and a peak that
    // is too narrow and too tall conserves the AREA while over-self-shielding in
    // transport -- which is precisely the failure mode a resonance-integral
    // check cannot see. NJOY's own PENDF gives 5386.8 b for MT=102 there.
    let peak = u238.xs_at_energy(6.674, 600.0);
    assert_reproduces_recorded(
        "the 6.674 eV capture resonance peak",
        peak.absorption - peak.fission,
        5_386.779,
        0.002,
    );
}
