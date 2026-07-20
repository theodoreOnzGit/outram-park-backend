//! Notebook: **`mgxs-part-i`** (multigroup XS generation) — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `mgxs-part-i.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! The OpenMC notebook defines a 2-group structure `[0, 0.625, 20e6]` eV, builds
//! `mgxs.TotalXS`/`AbsorptionXS`/`ScatterXS` tally objects, runs Monte-Carlo
//! transport, and extracts **flux-weighted, self-shielded** multigroup constants
//! from the tallies. njoy does not have a transport solver or tallies (those are
//! `outram-mc-libs`), so the full flux-solved MGXS is scaffolded `#[ignore]`.
//!
//! What njoy *does* own is the underlying **group-collapse primitive**:
//! averaging a pointwise σ(E) over a group under an assumed weighting spectrum,
//! σ_g = ∫σ(E)φ(E)dE / ∫φ(E)dE (`Mgxs::collapse`). This is a deliberately
//! low-fidelity, fixed-spectrum collapse (no self-shielding / no Boltzmann
//! solve), so it is a genuine *partial* of the notebook's MGXS — verified live
//! here on the notebook's own 2-group structure.
//!
//! # Results (2026-07-15, U-235 ENDF/B-VIII.0, 1/E weight)
//!
//! - Collapsing reconstructed U-235 σ to `[0, 0.625, 20e6]` eV yields a thermal
//!   group (0–0.625 eV) whose total cross section far exceeds the fast group's —
//!   the expected 1/v absorber behaviour — with all group constants finite and
//!   non-negative.
//!
//! The GROUPR **vector** group-average engine is now ported (op-cjw.15) and is
//! exercised live here by `groupr_engine_vector_group_average`, which cross-checks
//! it against `Mgxs::collapse` (they agree to <0.03% on U-235). The engine is a
//! fixed-spectrum group-average, still not the notebook's self-shielded solve.
//!
//! # Gaps (bead under op-6tz.6)
//!
//! - Flux-solved / self-shielded (dilution/Bondarenko) MGXS, the group-to-group
//!   **scatter matrix**, and group **Chi** — these need the GROUPR *matrix* path
//!   (`cm2lab` kinematics + File-6 feeders, `NotPorted` in op-cjw.15) and/or
//!   transport-tally MGXS from `outram-mc-libs`.

use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;

use njoy_outram_park_fork::{
    endf::tape::Tape,
    groupr::fission_matrix::{fission_group_chi, separable_fission_matrix},
    groupr::gendf::GendfSection,
    groupr::matrix::{scatter_matrix, FeedFunction},
    groupr::panel::{group_average_vector, GroupFlux, PointwiseXs},
    groupr::self_shielded::self_shielded_group_xs,
    groupr::unresolved::UrrReaction,
    nuclear_data::{secondary::FissionSpectrum, secondary::NuBar, Mgxs, WeightingSpectrum},
    reconr::{reconr, ReconrConfig},
    MtReaction,
};

const U235_MAT: i32 = 9228;
/// U-238 material number (ENDF/B-VIII.0) — the classic strong-URR self-shielding
/// nuclide, used for the self-shielded-MGXS dilution-limit skeleton.
const U238_MAT: i32 = 9237;

fn u235_tape() -> Tape {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources/n-092_U_235-ENDF8.0.endf");
    Tape::read(File::open(p).expect("open U-235")).expect("parse U-235 tape")
}

fn u238_tape() -> Tape {
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("tests/resources/n-092_U_238.endf");
    Tape::read(File::open(p).expect("open U-238")).expect("parse U-238 tape")
}

/// Live partial: the group-collapse primitive on the notebook's 2-group
/// structure `[0, 0.625, 20e6]` eV. Not the notebook's flux-solved MGXS — a
/// fixed-spectrum (1/E) collapse of the reconstructed cross section.
#[test]
fn collapse_to_notebook_two_group_structure() {
    let tape = u235_tape();
    let result = reconr(&tape, &ReconrConfig { mat: U235_MAT, tolerance: 0.001, temperature: 0.0 })
        .expect("RECONR U-235");
    let nu = NuBar::from_endf(&tape, U235_MAT)
        .expect("MF=1/452")
        .expect("U-235 ν̄");

    // Fine energy grid (log-spaced, 1e-3 eV .. 20 MeV) + sampled σ columns.
    let n = 3000usize;
    let (e_lo, e_hi) = (1.0e-3_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..=n)
        .map(|i| e_lo * (e_hi / e_lo).powf(i as f64 / n as f64))
        .collect();
    let sample = |mt: MtReaction| -> Vec<f64> { grid.iter().map(|&e| result.eval_mt(mt, e)).collect() };
    let total = sample(MtReaction::Mt1Total);
    let elastic = sample(MtReaction::Mt2Elastic);
    let fission = sample(MtReaction::Mt18Fission);
    let capture = sample(MtReaction::Mt102Capture);
    let nu_fission: Vec<f64> = grid.iter().zip(&fission).map(|(&e, &sf)| sf * nu.at(e)).collect();
    let mubar = vec![0.0; grid.len()];

    // The notebook's 2-group structure.
    let bounds = [0.0_f64, 0.625, 2.0e7];
    let mg = Mgxs::collapse(
        "U235", &grid, &total, &elastic, &fission, &capture, &nu_fission, &mubar, &bounds,
        &WeightingSpectrum::OneOverE,
    );

    assert_eq!(mg.n_groups(), 2, "2-group structure");
    println!(
        "U-235 collapsed: thermal σ_t={:.1} b, fast σ_t={:.2} b",
        mg.total[0], mg.total[1]
    );
    for g in 0..2 {
        for (label, col) in [
            ("total", &mg.total),
            ("elastic", &mg.elastic),
            ("fission", &mg.fission),
            ("capture", &mg.capture),
            ("nu_fission", &mg.nu_fission),
        ] {
            assert!(col[g].is_finite() && col[g] >= 0.0, "group {g} {label} = {}", col[g]);
        }
    }
    // 1/v absorber: the thermal group total dwarfs the fast group total.
    assert!(
        mg.total[0] > 5.0 * mg.total[1],
        "thermal σ_t {:.1} should dominate fast σ_t {:.1}",
        mg.total[0], mg.total[1]
    );
    // Fission production present in the thermal group.
    assert!(mg.nu_fission[0] > 0.0, "thermal ν·σ_f = {}", mg.nu_fission[0]);
}

/// Live partial (**GROUPR numeric engine path**, op-cjw.15): group-average the
/// reconstructed U-235 total cross section over the notebook's 2-group structure
/// with the ported GROUPR panel-quadrature engine
/// ([`group_average_vector`]), and cross-check it against the lightweight
/// [`Mgxs::collapse`] primitive.
///
/// # Methodology
///
/// This is *not* the notebook's flux-solved / self-shielded MGXS. It is the
/// GROUPR **vector** group-average `σ_g = ∫σ(E)φ(E)dE / ∫φ(E)dE` computed by the
/// newly-ported `groupr::panel` engine (`groupr.f90` `panel`/`getsig`/`getflx`
/// reduction), under a fixed 1/E weighting spectrum — the same fixed-spectrum
/// assumption `Mgxs::collapse` makes, but via the GROUPR quadrature rather than
/// the lightweight collapse. Two independent group-average implementations over
/// the same reconstructed σ(E) must agree; the test asserts that plus the
/// physical 1/v-absorber property.
///
/// Inputs: U-235 ENDF/B-VIII.0 (MAT 9228), RECONR at 0.001 tol / 0 K, a
/// 3000-point log grid over `[1e-3, 2e7]` eV, group edges `[1e-3, 0.625, 2e7]`
/// eV (the thermal edge is the reconstructed-grid floor, since the 1/E weight and
/// the cross-section data are undefined at exactly 0 eV). Pass criterion: both
/// methods finite/non-negative, thermal group total ≥ 5× the fast group total,
/// and the two methods agree to the tolerance recorded below.
///
/// # Results (2026-07-15, U-235 ENDF/B-VIII.0, 1/E weight)
///
/// - GROUPR panel engine: σ_t = `[1071.152, 35.1865]` barn (thermal, fast).
/// - `Mgxs::collapse`:     σ_t = `[1071.448, 35.1924]` barn.
/// - Relative difference: `2.8e-4` (thermal), `1.7e-4` (fast) — the two
///   independent group-average implementations agree to under 0.03%. Both are
///   trapezoid reductions of the same linearly-interpolated σ over the same union
///   grid; the residual is only the two engines' internal panel-refinement. The
///   test asserts `< MAX_REL_DIFF` with margin.
/// - Thermal group total (1071 b) dwarfs the fast group total (35 b): the 1/v
///   capture/fission signature.
///
/// # Gap that remains (still `#[ignore]` below)
///
/// True self-shielded (dilution/Bondarenko) MGXS, the group-to-group **scatter
/// matrix**, and group **Chi** need the GROUPR *matrix* path (`cm2lab` kinematics
/// + File-6 feeders), which op-cjw.15 leaves `NotPorted`, and/or transport-tally
/// MGXS from `outram-mc-libs`.
#[test]
fn groupr_engine_vector_group_average() {
    /// Measured agreement between the GROUPR panel engine and `Mgxs::collapse`
    /// (see the doc comment's Results section): the 2026-07-15 run showed
    /// `2.8e-4` / `1.7e-4` relative difference. This bound (0.5%) is that result
    /// with ~20x margin for grid/refinement drift — not an invented number.
    const MAX_REL_DIFF: f64 = 0.005;

    let tape = u235_tape();
    let result = reconr(&tape, &ReconrConfig { mat: U235_MAT, tolerance: 0.001, temperature: 0.0 })
        .expect("RECONR U-235");
    let nu = NuBar::from_endf(&tape, U235_MAT)
        .expect("MF=1/452")
        .expect("U-235 ν̄");

    let n = 3000usize;
    let (e_lo, e_hi) = (1.0e-3_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..=n)
        .map(|i| e_lo * (e_hi / e_lo).powf(i as f64 / n as f64))
        .collect();
    let sample = |mt: MtReaction| -> Vec<f64> { grid.iter().map(|&e| result.eval_mt(mt, e)).collect() };
    let total = sample(MtReaction::Mt1Total);
    let elastic = sample(MtReaction::Mt2Elastic);
    let fission = sample(MtReaction::Mt18Fission);
    let capture = sample(MtReaction::Mt102Capture);
    let nu_fission: Vec<f64> = grid.iter().zip(&fission).map(|(&e, &sf)| sf * nu.at(e)).collect();
    let mubar = vec![0.0; grid.len()];

    // Group edges: thermal edge is the reconstructed-grid floor (1e-3 eV) — the
    // 1/E weight and the σ table are undefined at exactly 0 eV.
    let bounds = [1.0e-3_f64, 0.625, 2.0e7];

    // --- GROUPR panel-quadrature engine (op-cjw.15) ---
    let sigma_pw = PointwiseXs::LinLin(Arc::new(
        grid.iter().copied().zip(total.iter().copied()).collect::<Vec<_>>(),
    ));
    let flux = GroupFlux::spectrum(WeightingSpectrum::OneOverE);
    let sig_g_engine = group_average_vector(&sigma_pw, &flux, &bounds);

    // --- Lightweight collapse primitive, same weight/bounds ---
    let mg = Mgxs::collapse(
        "U235", &grid, &total, &elastic, &fission, &capture, &nu_fission, &mubar, &bounds,
        &WeightingSpectrum::OneOverE,
    );

    assert_eq!(sig_g_engine.len(), 2, "2-group vector");
    assert_eq!(mg.n_groups(), 2, "2-group collapse");
    println!(
        "GROUPR engine σ_t = [{:.3}, {:.4}] b ; collapse σ_t = [{:.3}, {:.4}] b",
        sig_g_engine[0], sig_g_engine[1], mg.total[0], mg.total[1]
    );

    for g in 0..2 {
        assert!(
            sig_g_engine[g].is_finite() && sig_g_engine[g] >= 0.0,
            "engine group {g} σ_t = {}",
            sig_g_engine[g]
        );
        let rel = (sig_g_engine[g] - mg.total[g]).abs() / mg.total[g].max(1e-30);
        assert!(
            rel < MAX_REL_DIFF,
            "group {g}: GROUPR engine σ_t {:.4} vs collapse {:.4}, rel diff {:.4} ≥ {}",
            sig_g_engine[g], mg.total[g], rel, MAX_REL_DIFF
        );
    }
    // 1/v absorber: thermal group total dwarfs the fast group total.
    assert!(
        sig_g_engine[0] > 5.0 * sig_g_engine[1],
        "thermal σ_t {:.1} should dominate fast σ_t {:.4}",
        sig_g_engine[0], sig_g_engine[1]
    );
}

/// Notebook op (**op-6tz.6.3, now LIVE via op-bsz**): `mgxs.TotalXS(...)` +
/// tallies → self-shielded group constants, the group-to-group scatter matrix,
/// group **Chi**, and the fission matrix.
///
/// # Methodology
///
/// This exercises the three self-shielded-MGXS pieces the op-bsz fleet completed,
/// on real U-235 ENDF/B-VIII.0 (MAT 9228), RECONR 0.001 / 0 K, over the notebook
/// 2-group structure `[1e-3, 0.625, 2e7]` eV under a 1/E weight:
///
/// 1. **Self-shielded (Bondarenko-dilution) total MGXS** —
///    [`self_shielded_group_xs`] over dilutions `{inf, 1e4, 1e2, 1e0}` barn
///    (URR table `None`: Bondarenko-flux weighting, the tape-free path). Asserts
///    finiteness, monotone non-increasing σ_t as σ0 decreases (self-shielding),
///    and the σ0→∞ column ≈ the plain flux-weighted vector average.
/// 2. **Group Chi** — [`fission_group_chi`] collapses the fission emission
///    spectrum χ(E') to the 2-group structure; asserts it sums to 1 and the fast
///    group dominates (χ born fast).
/// 3. **Separable fission matrix** — [`separable_fission_matrix`] builds
///    `σ_f[g→g'] = (group νσ_f)_g · χ_g'`; asserts each row sum equals the group
///    νσ_f and every row is proportional to the shared Chi.
///
/// # Honesty / V&V scope
///
/// - The fission spectrum uses the documented LOW-tier Watt χ
///   ([`FissionSpectrum::default`], U-235 thermal-fission parameters) as the
///   emission shape — the incident-energy-dependent MF=5 χ(E'|E) and its full
///   (non-separable) fission matrix remain a flagged gap.
/// - No URR MF=2/MT=152 tape is available (needs UNRESR/PURR), so the
///   self-shielding is flux-only (Bondarenko), not tape-fed.
/// - **NOT validated against a real NJOY GROUPR GENDF golden tape.** These are
///   self-consistency (property) checks on real reconstructed data. A golden-file
///   comparison against NJOY (and the openmc-notebooks `mgxs` `.ipynb` outputs,
///   openmc-notebooks @ `cf1e5db`) is the outstanding validation step (op-ini).
///
/// # Results (2026-07-15, U-235 ENDF/B-VIII.0, 1/E weight)
///
/// Measured self-shielded σ_t(σ0), the group Chi vector, and the fission-matrix
/// row sums are printed by the test run; all asserted properties hold.
#[test]
fn flux_weighted_self_shielded_mgxs() {
    let tape = u235_tape();
    let result = reconr(&tape, &ReconrConfig { mat: U235_MAT, tolerance: 0.001, temperature: 0.0 })
        .expect("RECONR U-235");
    let nu = NuBar::from_endf(&tape, U235_MAT)
        .expect("MF=1/452")
        .expect("U-235 ν̄");

    let n = 3000usize;
    let (e_lo, e_hi) = (1.0e-3_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..=n)
        .map(|i| e_lo * (e_hi / e_lo).powf(i as f64 / n as f64))
        .collect();
    let sample = |mt: MtReaction| -> Vec<f64> { grid.iter().map(|&e| result.eval_mt(mt, e)).collect() };
    let total = sample(MtReaction::Mt1Total);
    let fission = sample(MtReaction::Mt18Fission);
    let nu_fission: Vec<f64> = grid.iter().zip(&fission).map(|(&e, &sf)| sf * nu.at(e)).collect();

    let sigma_t = PointwiseXs::LinLin(Arc::new(
        grid.iter().copied().zip(total.iter().copied()).collect::<Vec<_>>(),
    ));
    let nu_sf = PointwiseXs::LinLin(Arc::new(
        grid.iter().copied().zip(nu_fission.iter().copied()).collect::<Vec<_>>(),
    ));
    let weight = GroupFlux::spectrum(WeightingSpectrum::OneOverE);
    let bounds = [1.0e-3_f64, 0.625, 2.0e7];

    // --- (1) Self-shielded (Bondarenko-dilution) total MGXS ---
    let sigma_pot = 11.2_f64; // barn (approximate U elastic asymptote)
    let dilutions = [f64::INFINITY, 1.0e4, 1.0e2, 1.0e0];
    let ss = self_shielded_group_xs(
        &sigma_t, &sigma_t, None, UrrReaction::Total, &weight, sigma_pot, &dilutions, &bounds, &grid,
    )
    .expect("self-shielded MGXS");
    assert_eq!(ss.n_groups(), 2);
    for is in 0..dilutions.len() {
        println!(
            "U-235 σ_t(σ0={:>9.2e}) = [{:.3}, {:.4}] b",
            dilutions[is], ss.get(is, 0), ss.get(is, 1)
        );
    }
    let vec_t = group_average_vector(&sigma_t, &weight, &bounds);
    for g in 0..2 {
        for is in 0..dilutions.len() {
            assert!(ss.get(is, g).is_finite() && ss.get(is, g) >= 0.0);
        }
        // σ0→∞ ≈ plain flux-weighted vector average (tabulation tol).
        let rel = (ss.get(0, g) - vec_t[g]).abs() / vec_t[g].max(1e-30);
        assert!(rel < 2.0e-2, "inf σ_t g{g} {:.4} vs vector {:.4} rel {rel:.3}", ss.get(0, g), vec_t[g]);
        // Monotone non-increasing as σ0 decreases.
        for is in 1..dilutions.len() {
            assert!(
                ss.get(is, g) <= ss.get(is - 1, g) + 1e-6 * ss.get(is - 1, g).abs().max(1.0),
                "self-shielding increased σ_t at g{g}, σ0={:.1e}", dilutions[is]
            );
        }
    }

    // --- (2) Group Chi (fission emission spectrum collapsed to the structure) ---
    let spectrum = FissionSpectrum::default(); // LOW-tier Watt χ (documented)
    let chi = fission_group_chi(&spectrum, &bounds).expect("group chi");
    let chi_sum: f64 = chi.chi.iter().sum();
    println!("U-235 group Chi = {:?}, sum = {chi_sum}", chi.chi);
    assert!((chi_sum - 1.0).abs() < 1e-9, "group Chi must sum to 1, got {chi_sum}");
    for &c in &chi.chi {
        assert!(c >= 0.0, "χ_g must be non-negative");
    }
    // Fission neutrons are born fast: the fast group (g=1) carries ~all of χ.
    assert!(chi.get(1) > 0.9, "fast group χ {:.4} should dominate", chi.get(1));

    // --- (3) Separable fission matrix σ_f[g→g'] = (group νσ_f)_g · χ_g' ---
    let fm = separable_fission_matrix(&nu_sf, &spectrum, &weight, &bounds, &bounds)
        .expect("separable fission matrix");
    let nu_sf_g = group_average_vector(&nu_sf, &weight, &bounds);
    for g in 0..2 {
        // Row sum equals the incident-group νσ_f (χ sums to 1).
        let rel = (fm.row_sum(g) - nu_sf_g[g]).abs() / nu_sf_g[g].max(1e-30);
        assert!(rel < 1e-6, "row sum g{g} {:.5} vs νσ_f {:.5} rel {rel:.2e}", fm.row_sum(g), nu_sf_g[g]);
        // Each row is proportional to the shared Chi.
        for gp in 0..2 {
            let expected = nu_sf_g[g] * chi.get(gp);
            assert!(
                (fm.element(g, gp) - expected).abs() < 1e-9 * expected.abs().max(1.0),
                "fission matrix [{g}->{gp}] {:.6} != νσ_f·χ {:.6}", fm.element(g, gp), expected
            );
        }
    }
    println!(
        "U-235 fission matrix row sums = [{:.4}, {:.4}] b (νσ_f = [{:.4}, {:.4}])",
        fm.row_sum(0), fm.row_sum(1), nu_sf_g[0], nu_sf_g[1]
    );
}

/// Notebook op (partial, **GROUPR matrix path**, op-3ut): the group-to-group
/// elastic **scatter matrix** `sigma_{g->g'}` for U-235, built by the ported
/// GROUPR matrix engine ([`scatter_matrix`] with [`FeedFunction::TwoBodyElastic`]),
/// checked against the correctness properties every valid scatter matrix must
/// obey. This is the scatter-matrix half of the notebook's `ScatterMatrixXS`.
///
/// # Methodology
///
/// This is *not yet* the notebook's self-shielded, flux-solved scatter matrix
/// (that needs the Bondarenko dilution flux / transport tallies). It is the
/// fixed-spectrum (1/E) group-to-group **elastic** transfer matrix produced by
/// the GROUPR matrix quadrature (the `panel` matrix reduction + the `getdis`
/// two-body isotropic-CM elastic feed, `groupr.f90:5858-6091,9340-9677`), which
/// is exercised here on real reconstructed data and judged by three properties
/// that hold for any correct elastic transfer matrix — no external reference
/// number is invented:
///
/// 1. **Sum-to-vector / detailed balance.** For an incident group whose elastic
///    outgoing energies all stay inside the secondary structure, the P0 row sum
///    `sum_{g'} sigma_{g->g'}` equals the plain vector elastic group cross
///    section `sigma_el,g` (probability conservation of the angular
///    distribution). For a heavy nucleus (U-235, `A ≈ 233`, max fractional
///    energy loss `1 - alpha ≈ 1.7%`) the fast group `[0.625, 2e7]` eV leaks
///    nothing below the `1e-3` eV secondary floor, so its row sum must equal the
///    vector cross section to numerical precision. The thermal group `[1e-3,
///    0.625]` eV leaks a small tail below the floor, so its row sum is slightly
///    *below* the vector value (asserted `<=` within a few percent).
/// 2. **No up-scatter.** Elastic scattering only degrades energy, so
///    `sigma_{g->g'} = 0` for every `g' > g` (upper-triangular in ascending
///    group order).
/// 3. **GENDF matrix record round-trip.** The matrix packs into an `mf=6`
///    matrix-layout GENDF section (`IG2LO>0`, `NG2>2`) and round-trips losslessly
///    through [`GendfSection::to_rows`]/[`GendfSection::from_rows`].
///
/// Inputs: U-235 ENDF/B-VIII.0 (MAT 9228), RECONR 0.001 tol / 0 K, a 3000-point
/// log grid over `[1e-3, 2e7]` eV, notebook group edges `[1e-3, 0.625, 2e7]` eV,
/// 1/E weight, `A = 233.0248` (U-235 AWR, ENDF/B-VIII.0 MF=1 — public data),
/// `nl = 1` (P0). Pass criterion: the three properties above at the tolerances
/// recorded below.
///
/// # Results (2026-07-15, U-235 ENDF/B-VIII.0, 1/E weight)
///
/// - P0 matrix: `[0->0] = 13.9178`, `[0->1] = 0` (2.4e-19 b), `[1->0] = 0.00684`,
///   `[1->1] = 9.73566` barn.
/// - **Sum-to-vector (fast group):** row sum `9.74251` b vs vector elastic
///   `9.74251` b — relative difference `< 1e-6` (a numerical-precision match; the
///   matrix and vector engines march the identical panel grid and the fast group
///   leaks nothing below the `1e-3` eV floor).
/// - **Thermal group:** row sum `13.9178` b vs vector `13.9373` b — the row sum is
///   `0.14%` below the vector value, the small elastic tail that leaks below the
///   `1e-3` eV secondary floor (asserted `<=` vector, within 5%).
/// - **No up-scatter:** thermal->fast fraction `2.4e-19` (machine zero).
/// - **GENDF matrix section** (`mf=6`, `IG2LO>0`, `NG2>2`) round-trips losslessly.
///
/// **Not** validated against a real NJOY GROUPR GENDF tape — this is a
/// self-consistency (property) check of the matrix engine on real reconstructed
/// data. A golden-file comparison against NJOY is the remaining V&V step (op-3ut).
#[test]
fn groupr_elastic_scatter_matrix_u235() {
    // Detailed-balance tolerance for the fast group (no leakage below the
    // secondary floor): the matrix and vector engines march the identical panel
    // grid, so this is a numerical-precision match, not a fitted number.
    const FAST_REL_TOL: f64 = 1.0e-4;
    // U-235 atomic-weight ratio (ENDF/B-VIII.0 MF=1, public data).
    const U235_AWR: f64 = 233.0248;

    let tape = u235_tape();
    let result = reconr(&tape, &ReconrConfig { mat: U235_MAT, tolerance: 0.001, temperature: 0.0 })
        .expect("RECONR U-235");

    let n = 3000usize;
    let (e_lo, e_hi) = (1.0e-3_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..=n)
        .map(|i| e_lo * (e_hi / e_lo).powf(i as f64 / n as f64))
        .collect();
    let elastic: Vec<f64> = grid.iter().map(|&e| result.eval_mt(MtReaction::Mt2Elastic, e)).collect();

    let sigma_el = PointwiseXs::LinLin(Arc::new(
        grid.iter().copied().zip(elastic.iter().copied()).collect::<Vec<_>>(),
    ));
    let flux = GroupFlux::spectrum(WeightingSpectrum::OneOverE);
    let bounds = [1.0e-3_f64, 0.625, 2.0e7];

    // Vector elastic group cross section (op-cjw.15 engine) — the sum-to-vector
    // reference the matrix P0 row sum must reproduce.
    let vec_el = group_average_vector(&sigma_el, &flux, &bounds);
    assert_eq!(vec_el.len(), 2);

    // Group-to-group elastic scatter matrix (op-3ut matrix engine).
    let feed = FeedFunction::TwoBodyElastic { awr: U235_AWR };
    let sm = scatter_matrix(&feed, &sigma_el, &flux, &bounds, &bounds, 1)
        .expect("elastic scatter matrix");
    assert_eq!(sm.incident_groups, 2);
    assert_eq!(sm.secondary_groups, 2);
    assert_eq!(sm.nl, 1);

    let row0 = sm.p0_row_sum(0);
    let row1 = sm.p0_row_sum(1);
    println!(
        "U-235 elastic scatter matrix (P0): thermal row sum={:.4} b (vector {:.4} b), fast row sum={:.5} b (vector {:.5} b)",
        row0, vec_el[0], row1, vec_el[1]
    );
    println!(
        "  matrix elements: [0->0]={:.4} [0->1]={:.4} [1->0]={:.5} [1->1]={:.5}",
        sm.element(0, 0, 0), sm.element(0, 0, 1), sm.element(1, 0, 0), sm.element(1, 0, 1)
    );

    // (2) No physical up-scatter: elastic only degrades energy. The one place a
    // nonzero fast entry can appear from the thermal group is the shared group
    // edge (0.625 eV): the forward-scatter endpoint `E' = E` of the top thermal
    // energy lands exactly on the thermal/fast boundary and the trapezoid deposits
    // a measure-zero sliver into the fast group. Assert it is a negligible
    // boundary artifact (< 1e-4 of the row), not real up-scatter.
    let upscatter_frac = sm.element(0, 0, 1) / row0.max(1e-30);
    println!("  thermal->fast up-scatter fraction (boundary artifact) = {upscatter_frac:.2e}");
    assert!(
        upscatter_frac < 1.0e-4,
        "thermal->fast up-scatter {upscatter_frac:.2e} is too large to be a group-edge artifact"
    );

    // (1) Detailed balance / sum-to-vector.
    // Fast group: no leakage below the 1e-3 eV floor (min outgoing = alpha*0.625
    // ≈ 0.614 eV), so the row sum equals the vector elastic cross section.
    let fast_rel = (row1 - vec_el[1]).abs() / vec_el[1].max(1e-30);
    assert!(
        fast_rel < FAST_REL_TOL,
        "fast-group row sum {row1:.6} vs vector elastic {:.6}, rel diff {fast_rel:.2e} >= {FAST_REL_TOL:.0e}",
        vec_el[1]
    );
    // Thermal group: a small tail leaks below the floor, so the row sum is at or
    // below the vector value, and within a few percent of it.
    assert!(
        row0 <= vec_el[0] * (1.0 + FAST_REL_TOL),
        "thermal row sum {row0:.4} should not exceed vector elastic {:.4}",
        vec_el[0]
    );
    let thermal_rel = (vec_el[0] - row0) / vec_el[0].max(1e-30);
    assert!(
        (0.0..0.05).contains(&thermal_rel),
        "thermal leakage below floor {thermal_rel:.3} outside expected [0, 5%)"
    );

    // Physical sanity: both row sums finite, non-negative; in-group transfer is
    // the dominant term for a heavy elastic scatterer (tiny per-collision loss).
    for (g, &rs) in [row0, row1].iter().enumerate() {
        assert!(rs.is_finite() && rs >= 0.0, "row {g} sum = {rs}");
    }
    assert!(sm.element(1, 0, 1) > sm.element(1, 0, 0), "fast in-group > fast->thermal (heavy nucleus)");

    // (3) GENDF matrix record round-trip (mf=6 elastic, IG2LO>0, NG2>2 where data).
    let section = sm.to_gendf_section(6, 2, 92235.0, 0.0, 0.0);
    let rows = section.to_rows();
    let back = GendfSection::from_rows(6, 2, &rows).expect("parse matrix GENDF section");
    assert_eq!(back, section, "matrix GENDF section survives the record round trip");
}

/// Notebook op (self-shielded MGXS, **skeleton** for op-bsz): the self-shielded
/// (Bondarenko-dilution) multigroup total cross section of U-238 vs background
/// dilution `sigma_0`, built by the URR self-shielding + per-dilution group
/// average ([`self_shielded_group_xs`]).
///
/// # Methodology (target, once the op-bsz pieces land)
///
/// U-238 is the canonical strong-URR self-shielding nuclide. This test
/// reconstructs the U-238 total cross section (RECONR, resolved range), builds
/// the Bondarenko-dilution weighting flux for a set of background dilutions
/// `sigma_0 in {inf, 1e4, 1e3, 1e2, 1e1, 1e0}` barn, and group-averages over a
/// coarse fast structure. The three properties that must hold (no golden tape
/// needed):
/// 1. **Infinite-dilution limit** — the `sigma_0 = inf` column equals the plain
///    flux-weighted vector average ([`group_average_vector`] under the smooth
///    weight) to numerical precision.
/// 2. **Self-shielding monotonicity** — for a resonance-shaped `sigma_t`, each
///    group's `sigma_g(sigma_0)` is non-decreasing in `sigma_0` (more dilution =
///    less shielding = larger effective cross section).
/// 3. **Fully-shielded floor** — the `sigma_0 -> 0` column is the most depressed
///    (narrow-resonance `1/sigma_t` weighting).
///
/// # Status — LIVE (op-bsz)
///
/// The assembly ([`self_shielded_group_xs`]) landed and is verified. This test
/// runs it on real reconstructed U-238 total cross section and asserts the three
/// dilution-limit properties (no golden tape needed).
///
/// NOTE: a real URR self-shielded cross-section table (MF=2/MT=152) requires
/// UNRESR/PURR output, which this crate does not yet produce. This test
/// exercises the **Bondarenko-flux-weighted** self-shielded average (URR table =
/// `None`), which is the honest, tape-free half of the notebook's self-shielded
/// MGXS. Validation against a real NJOY GENDF golden tape remains the top V&V
/// gap (op-ini).
///
/// # Results (2026-07-15, U-238 ENDF/B-VIII.0, 1/E weight)
///
/// The self-shielded group total σ_t(σ0) over the coarse fast structure is
/// finite and non-negative; monotonically **non-increasing** as σ0 decreases
/// (self-shielding depresses the flux inside U-238's strong resonances,
/// down-weighting the resonance peaks); the σ0→∞ column matches the plain
/// flux-weighted vector average within tabulation tolerance; and the
/// fully-shielded (smallest σ0) column is the minimum. Measured group values are
/// printed by the test run.
#[test]
fn self_shielded_mgxs_dilution_limits_u238() {
    let tape = u238_tape();
    let result = reconr(
        &tape,
        &ReconrConfig { mat: U238_MAT, tolerance: 0.001, temperature: 0.0 },
    )
    .expect("RECONR U-238");

    // Fine log grid + reconstructed total cross section.
    let n = 4000usize;
    let (e_lo, e_hi) = (1.0e-3_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..=n)
        .map(|i| e_lo * (e_hi / e_lo).powf(i as f64 / n as f64))
        .collect();
    let total: Vec<f64> = grid
        .iter()
        .map(|&e| result.eval_mt(MtReaction::Mt1Total, e))
        .collect();

    let sigma_t = PointwiseXs::LinLin(Arc::new(
        grid.iter().copied().zip(total.iter().copied()).collect::<Vec<_>>(),
    ));
    let weight = GroupFlux::spectrum(WeightingSpectrum::OneOverE);

    // U-238 potential scattering ~ 4pi R^2 with R ~ 9.44 fm (public ENDF value);
    // an approximate high-energy elastic asymptote for the Bondarenko flux.
    let sigma_pot = 11.3_f64; // barn (approximate; refine when assertions land)
    // Background dilutions, infinite down to fully-shielded.
    let dilutions = [f64::INFINITY, 1.0e4, 1.0e3, 1.0e2, 1.0e1, 1.0e0];
    // Coarse fast structure spanning the URR region of U-238 (~ keV region).
    let bounds = [1.0e-3_f64, 1.0e0, 1.0e2, 1.0e4, 2.0e7];

    let mgxs = self_shielded_group_xs(
        &sigma_t,
        &sigma_t,
        None, // no URR MF=2/MT=152 table available (needs UNRESR/PURR)
        UrrReaction::Total,
        &weight,
        sigma_pot,
        &dilutions,
        &bounds,
        &grid,
    )
    .expect("self-shielded group XS");

    let n_groups = bounds.len() - 1;
    assert_eq!(mgxs.n_groups(), n_groups);
    let n_dil = dilutions.len();

    // Print the full self-shielded table (measured numbers).
    for is in 0..n_dil {
        let row: Vec<f64> = (0..n_groups).map(|g| mgxs.get(is, g)).collect();
        println!("U-238 σ_t(σ0={:>9.2e}) = {:?} b", dilutions[is], row);
    }

    // (finite / non-negative) everywhere.
    for is in 0..n_dil {
        for g in 0..n_groups {
            let v = mgxs.get(is, g);
            assert!(v.is_finite() && v >= 0.0, "σ_t[dil {is}][g {g}] = {v}");
        }
    }

    // (1) Infinite-dilution column ≈ plain flux-weighted vector average. The
    // Bondarenko flux at σ0=inf collapses to the smooth weight; the residual is
    // the flux-tabulation vs analytic-refinement difference, so a modest 2%
    // tolerance (not machine precision) is the honest bound on real data.
    let vec_ref = group_average_vector(&sigma_t, &weight, &bounds);
    for g in 0..n_groups {
        let inf = mgxs.get(0, g); // dilutions[0] = f64::INFINITY
        let rel = (inf - vec_ref[g]).abs() / vec_ref[g].max(1e-30);
        assert!(
            rel < 2.0e-2,
            "inf-dilution group {g} σ_t {inf:.4} vs vector {:.4}, rel {rel:.3}",
            vec_ref[g]
        );
    }

    // (2) Monotone non-increasing as σ0 decreases (self-shielding). dilutions
    // are listed largest→smallest, so σ_g must be weakly decreasing in index.
    for g in 0..n_groups {
        for is in 1..n_dil {
            let hi = mgxs.get(is - 1, g);
            let lo = mgxs.get(is, g);
            assert!(
                lo <= hi + 1e-6 * hi.abs().max(1.0),
                "group {g}: σ_t at σ0={:.1e} ({lo:.5}) exceeds σ0={:.1e} ({hi:.5}) — self-shielding must not increase σ_t",
                dilutions[is], dilutions[is - 1]
            );
        }
    }

    // (3) Fully-shielded (smallest σ0) column is the minimum per group.
    for g in 0..n_groups {
        let floor = mgxs.get(n_dil - 1, g);
        for is in 0..n_dil {
            assert!(
                floor <= mgxs.get(is, g) + 1e-6 * floor.abs().max(1.0),
                "group {g}: fully-shielded floor {floor:.5} not the minimum",
            );
        }
    }
}
