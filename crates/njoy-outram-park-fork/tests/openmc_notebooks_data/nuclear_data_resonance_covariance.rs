//! Notebook: **`nuclear-data-resonance-covariance`** — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `nuclear-data-resonance-covariance.ipynb`,
//! commit `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! The OpenMC notebook loads Gd-157 with `IncidentNeutron.from_endf(covariance=True)`,
//! reads the ENDF MF=32 resonance-parameter covariance matrix, converts it to a
//! correlation matrix, samples perturbed resonance parameters (multivariate
//! normal), and reconstructs the elastic cross section from each sample. njoy has
//! **not** ported the MF=32 reader or the sampling→reconstruction loop
//! (`errorr::resprx` returns `NotPorted`), so the end-to-end workflow is
//! scaffolded `#[ignore]`. The one piece that *is* ported and testable today is
//! the **covariance → correlation transform** (COVR `subroutine corr`), which the
//! notebook performs with numpy from the diagonal; it is verified here against a
//! hand-computed reference.
//!
//! # Results (2026-07-15)
//!
//! - `CovarianceMatrix::to_correlation` on `[[4, 1.2], [1.2, 9]]` gives unit
//!   diagonal and off-diagonal 1.2 / sqrt(4·9) = 0.2; `relative_std_dev` gives
//!   `[2, 3]`. Matches the closed form exactly.
//!
//! # Gaps (beads under op-6tz.6)
//!
//! - ENDF MF=32 resonance-parameter covariance reader.
//! - Multivariate-normal parameter sampler + reconstruct-from-sampled-parameters.

use njoy_outram_park_fork::covr::CovarianceMatrix;

/// Notebook op: covariance → correlation (and per-group relative standard
/// deviations). This is the ported COVR math; the reference is computed by hand.
#[test]
fn covariance_to_correlation_matches_closed_form() {
    // A 2×2 auto-covariance: variances 4 and 9, covariance 1.2.
    let cov = CovarianceMatrix::from_row_major(2, vec![4.0, 1.2, 1.2, 9.0])
        .expect("well-formed 2×2 buffer");

    let rsd = cov.relative_std_dev();
    assert!((rsd[0] - 2.0).abs() < 1e-12, "rsd[0] = {}", rsd[0]);
    assert!((rsd[1] - 3.0).abs() < 1e-12, "rsd[1] = {}", rsd[1]);

    let corr = cov.to_correlation();
    // Unit diagonal for positive variances.
    assert!(
        (corr.get(0, 0) - 1.0).abs() < 1e-12,
        "corr(0,0) = {}",
        corr.get(0, 0)
    );
    assert!(
        (corr.get(1, 1) - 1.0).abs() < 1e-12,
        "corr(1,1) = {}",
        corr.get(1, 1)
    );
    // Off-diagonal = cov / (rsd_i · rsd_j) = 1.2 / (2·3) = 0.2, symmetric.
    let expected = 1.2 / (2.0 * 3.0);
    assert!(
        (corr.get(0, 1) - expected).abs() < 1e-12,
        "corr(0,1) = {}",
        corr.get(0, 1)
    );
    assert!(
        (corr.get(1, 0) - expected).abs() < 1e-12,
        "corr(1,0) = {}",
        corr.get(1, 0)
    );
}

/// Notebook op: `IncidentNeutron.from_endf(f, covariance=True)` +
/// `.resonance_covariance.ranges[0].covariance`.
///
/// Requires an **ENDF MF=32 resonance-parameter covariance reader** — not ported
/// (see op-6tz.6 "MF=32 resonance covariance" bead). The COVR correlation math
/// above is ready to consume such a matrix once the reader exists.
#[test]
#[ignore = "requires ENDF MF=32 resonance-parameter covariance reader (op-6tz.6)"]
fn read_resonance_covariance_from_endf() {
    panic!("njoy has no ENDF MF=32 resonance-covariance reader yet (errorr::resprx is NotPorted)");
}

/// Notebook op: `.sample(n_samples)` (multivariate normal) → `sample.reconstruct(E)`.
///
/// Requires the MF=32 matrix (above) plus a multivariate-normal sampler wired to
/// perturb resonance parameters, then RECONR reconstruction of each sample.
/// RECONR reconstruction from *nominal* parameters exists; the sampling loop does
/// not (op-6tz.6 "MF=32 resonance covariance" bead).
#[test]
#[ignore = "requires resonance-parameter sampling + reconstruct-from-sampled (op-6tz.6)"]
fn sample_and_reconstruct_perturbed_resonances() {
    panic!("njoy has no resonance-parameter multivariate-normal sampler / reconstruct-from-sample loop yet");
}
