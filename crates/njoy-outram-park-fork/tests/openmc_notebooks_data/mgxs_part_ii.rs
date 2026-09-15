//! Notebook: **`mgxs-part-ii`** (multigroup XS: matrices & Chi) — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `mgxs-part-ii.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! Part II extends the pin-cell MGXS to **group-to-group scatter matrices**
//! (`mgxs.ScatterMatrixXS`) and the fission spectrum vector (`mgxs.Chi`), on the
//! notebook's own **8-group "fine" structure**
//! `[0, 0.058, 0.14, 0.28, 0.625, 4.0, 5.53e3, 821e3, 20e6]` eV. In the notebook
//! these are solved from a Monte-Carlo pin-cell transport run (flux-weighted,
//! self-shielded tallies), which this data crate does not have — that path lives
//! in `outram-mc-libs`. What njoy *does* own is the **fixed-spectrum GROUPR
//! primitives** those quantities are built from, and those are what these tests
//! consume live:
//!
//! - the GROUPR **matrix** engine `groupr::matrix::scatter_matrix` with the
//!   two-body isotropic-CM elastic feed
//!   ([`FeedFunction::TwoBodyElastic`]) — the MT=2 (elastic) slice of the
//!   notebook's `ScatterMatrixXS`, exercised on real reconstructed U-235 data,
//!   verified by the correctness properties every valid elastic transfer matrix
//!   obeys (non-negativity, no up-scatter, sum-to-vector / detailed balance);
//! - the group-collapse of the fission emission spectrum
//!   `groupr::fission_matrix::fission_group_chi` — the njoy analogue of
//!   `mgxs.Chi`, verified by the two properties any physical group Chi obeys
//!   (non-negative, sums to 1) plus the born-fast signature.
//!
//! These are fixed-spectrum (1/E-weight) GROUPR reductions, **not** the
//! notebook's self-shielded, flux-solved tallies — a genuine *partial* of the
//! notebook, on the same 8-group structure. Because the notebook numbers come
//! from a pin-cell MC solve on H1/O16/U-nuclides (e.g. the printed moderator-cell
//! H1 elastic `1->1 = 0.233979 ± 0.003921` b), they are not reproducible without
//! a transport solve; the live tests below therefore verify the data operations
//! by physical/analytical properties, the same standard as the crate's other
//! GROUPR tests and the Part I live tests.
//!
//! # What stays `#[ignore]`d (accurate as of 2026-07-23)
//!
//! `mgxs.ScatterMatrixXS(nu=True)` is the **full** nu-scatter matrix: every
//! scattering reaction (inelastic MT=51..91 discrete + continuum, (n,2n) MT=16,
//! …) with its outgoing multiplicity, summed. Building that needs the File-6
//! (MF=6) laboratory secondary-energy/angle kinematic feeders — GROUPR `getff` /
//! `cm2lab` — which op-cjw.15 leaves `NotPorted`; only the two-body elastic feed
//! (`getdis`) is ported. The full nu-scatter matrix therefore stays honestly
//! `#[ignore]`d (`nu_scatter_matrix_full_mt`, gap bead **op-6tz.6.6**); the
//! tractable elastic (MT=2) slice is LIVE.

use std::fs::File;
use std::sync::Arc;

use njoy_outram_park_fork::{
    endf::tape::Tape,
    groupr::fission_matrix::fission_group_chi,
    groupr::gendf::GendfSection,
    groupr::matrix::{scatter_matrix, FeedFunction},
    groupr::panel::{group_average_vector, GroupFlux, PointwiseXs},
    nuclear_data::{secondary::FissionSpectrum, WeightingSpectrum},
    reconr::{reconr, ReconrConfig},
    MtReaction,
};

const U235_MAT: i32 = 9228;

/// The `mgxs-part-ii` notebook "fine" energy-group structure, eV, ascending.
///
/// Verbatim from the notebook (`fine_groups = openmc.mgxs.EnergyGroups()` with
/// `group_edges = [0., 0.058, 0.14, 0.28, 0.625, 4.0, 5.53e3, 821.0e3, 20.0e6]`),
/// **except** the `0` eV floor is raised to `1e-3` eV: the 1/E weighting spectrum
/// and the reconstructed cross-section table are undefined at exactly 0 eV. Eight
/// groups.
const NOTEBOOK_FINE_BOUNDS: [f64; 9] = [
    1.0e-3, 0.058, 0.14, 0.28, 0.625, 4.0, 5.53e3, 821.0e3, 2.0e7,
];

fn u235_tape() -> Tape {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("n-092_U_235-ENDF8.0.endf");
    Tape::read(File::open(p).expect("open U-235")).expect("parse U-235 tape")
}

/// Reconstruct U-235 σ(E) on a fine log grid over `[1e-3, 2e7]` eV and return
/// `(grid, reconstructed-value-of `mt`)`.
fn reconstruct_u235(mt: MtReaction) -> (Vec<f64>, Vec<f64>) {
    let tape = u235_tape();
    let result = reconr(
        &tape,
        &ReconrConfig {
            mat: U235_MAT,
            tolerance: 0.001,
            temperature: 0.0,
        },
    )
    .expect("RECONR U-235");
    let n = 3000usize;
    let (e_lo, e_hi) = (1.0e-3_f64, 2.0e7_f64);
    let grid: Vec<f64> = (0..=n)
        .map(|i| e_lo * (e_hi / e_lo).powf(i as f64 / n as f64))
        .collect();
    let xs: Vec<f64> = grid.iter().map(|&e| result.eval_mt(mt, e)).collect();
    (grid, xs)
}

/// Notebook op (**LIVE partial**): the elastic (MT=2) slice of
/// `mgxs.ScatterMatrixXS` — the group-to-group elastic transfer matrix
/// `sigma_{g->g'}` for U-235 on the notebook's 8-group "fine" structure, built by
/// the ported GROUPR matrix engine ([`scatter_matrix`] +
/// [`FeedFunction::TwoBodyElastic`]) and judged by the correctness properties
/// every valid elastic transfer matrix must obey.
///
/// # Methodology
///
/// This is *not* the notebook's self-shielded, flux-solved `ScatterMatrixXS`
/// (that needs an MC transport solve, `outram-mc-libs`), and it is only the
/// **elastic** reaction — the full `nu=True` all-reactions matrix needs File-6
/// kinematic feeders that are unported (see `nu_scatter_matrix_full_mt`). It is
/// the fixed-spectrum (1/E) group-to-group **elastic** transfer matrix produced
/// by the GROUPR matrix quadrature (`panel` matrix reduction + the two-body
/// isotropic-CM elastic feed, `groupr.f90`), on real reconstructed U-235 data,
/// verified by three properties that hold for any correct elastic transfer
/// matrix (no notebook MC number is reproducible without a flux solve, so none is
/// asserted as a target):
///
/// 1. **Non-negativity.** Every P0 transfer element is finite and `>= 0`.
/// 2. **No up-scatter.** Elastic scattering only degrades energy, so
///    `sigma_{g->g'} = 0` for `g' > g` — apart from a measure-zero trapezoid
///    sliver deposited across a shared group edge by the forward-scatter endpoint
///    `E' = E`, asserted `< 1e-4` of the row (a boundary artifact, not physical
///    up-scatter).
/// 3. **Sum-to-vector / detailed balance.** For an incident group whose elastic
///    outgoing energies all stay inside the secondary structure, the P0 row sum
///    `sum_{g'} sigma_{g->g'}` equals the plain vector elastic group cross
///    section `sigma_el,g`. For U-235 (`A ≈ 233`, max fractional energy loss
///    `1 - alpha ≈ 1.7%`) every group above the lowest one keeps its outgoing
///    energy above the `1e-3` eV secondary floor, so its row sum matches the
///    vector value to numerical precision; only the lowest group `[1e-3, 0.058]`
///    eV leaks a thin tail below the floor, so its row sum is at or slightly
///    below the vector value.
///
/// A GENDF matrix-record round-trip is also checked
/// ([`GendfSection::to_rows`]/[`GendfSection::from_rows`]).
///
/// Inputs: U-235 ENDF/B-VIII.0 (MAT 9228), RECONR 0.001 tol / 0 K, a 3000-point
/// log grid over `[1e-3, 2e7]` eV, notebook 8-group fine edges (0-eV floor raised
/// to 1e-3 eV), 1/E weight, `A = 233.0248` (U-235 AWR, ENDF/B-VIII.0 MF=1 —
/// public data), `nl = 1` (P0).
///
/// # Results (2026-07-23, U-235 ENDF/B-VIII.0, 1/E weight)
///
/// - 8x8 P0 elastic transfer matrix built; all elements finite and non-negative.
/// - **Sum-to-vector**, groups 1..7 (bottom edge `>= 0.058` eV, no sub-floor
///   leakage): P0 row sum matches the vector elastic group XS to relative
///   difference `5.0e-11` (numerical-precision match — the matrix and vector
///   engines march the identical panel grid). Measured per-group row sums (b):
///   `[13.85764, 13.56302, 13.40720, 12.34001, 11.90795, 9.52700, 3.66545]` for
///   groups 1..7, each equal to the vector elastic value to that precision.
/// - **Lowest group** `[1e-3, 0.058]` eV: row sum `14.09242` b vs vector
///   `14.12326` b — `0.218%` below (rel `2.18e-3`), the thin elastic tail leaking
///   below the `1e-3` eV floor (asserted `<=` vector within 5%).
/// - **No up-scatter:** every `g' > g` entry is a measure-zero group-edge
///   artifact `< 1e-4` of the row.
/// - **GENDF matrix section** (`mf=6`, elastic) round-trips losslessly.
///
/// This is a self-consistency (property) check on real reconstructed data,
/// **not** validated against a NJOY GROUPR GENDF golden tape (op-3ut / op-ini
/// remaining).
#[test]
fn scatter_matrix_xs() {
    // Detailed-balance tolerance for a group with no sub-floor leakage: the
    // matrix and vector engines march the identical panel grid, so this is a
    // numerical-precision match, not a fitted number.
    const REL_TOL: f64 = 1.0e-4;
    // U-235 atomic-weight ratio (ENDF/B-VIII.0 MF=1, public data).
    const U235_AWR: f64 = 233.0248;

    let (grid, elastic) = reconstruct_u235(MtReaction::Mt2Elastic);
    let sigma_el = PointwiseXs::LinLin(Arc::new(
        grid.iter()
            .copied()
            .zip(elastic.iter().copied())
            .collect::<Vec<_>>(),
    ));
    let flux = GroupFlux::spectrum(WeightingSpectrum::OneOverE);
    let bounds = NOTEBOOK_FINE_BOUNDS;
    let n_groups = bounds.len() - 1; // 8

    // Vector elastic group XS (op-cjw.15 engine) — the sum-to-vector reference.
    let vec_el = group_average_vector(&sigma_el, &flux, &bounds);
    assert_eq!(vec_el.len(), n_groups);

    // Group-to-group elastic scatter matrix (op-3ut matrix engine), P0 only.
    let feed = FeedFunction::TwoBodyElastic { awr: U235_AWR };
    let sm = scatter_matrix(&feed, &sigma_el, &flux, &bounds, &bounds, 1)
        .expect("elastic scatter matrix");
    assert_eq!(sm.incident_groups, n_groups);
    assert_eq!(sm.secondary_groups, n_groups);
    assert_eq!(sm.nl, 1);

    // Property 1: non-negativity, everywhere.
    for g in 0..n_groups {
        for gp in 0..n_groups {
            let v = sm.element(g, 0, gp);
            assert!(v.is_finite() && v >= 0.0, "element [{g}->{gp}] = {v}");
        }
    }

    // Property 2: no up-scatter (g' > g) beyond a group-edge trapezoid sliver.
    for g in 0..n_groups {
        let row = sm.p0_row_sum(g).max(1e-30);
        for gp in (g + 1)..n_groups {
            let frac = sm.element(g, 0, gp) / row;
            assert!(
                frac < 1.0e-4,
                "up-scatter [{g}->{gp}] fraction {frac:.2e} too large for a group-edge artifact"
            );
        }
    }

    // Property 3: sum-to-vector / detailed balance.
    println!("U-235 elastic scatter matrix (P0) on the notebook 8-group fine structure:");
    for g in 0..n_groups {
        let row = sm.p0_row_sum(g);
        let rel = (row - vec_el[g]).abs() / vec_el[g].max(1e-30);
        println!(
            "  group {g} [{:>9.3e},{:>9.3e}] eV: row sum {row:.5} b, vector {:.5} b, rel {rel:.2e}",
            bounds[g],
            bounds[g + 1],
            vec_el[g]
        );
        if g == 0 {
            // Lowest group leaks a thin elastic tail below the 1e-3 eV floor:
            // row sum at or below the vector value, within a few percent.
            assert!(
                row <= vec_el[g] * (1.0 + REL_TOL),
                "lowest-group row sum {row:.5} should not exceed vector {:.5}",
                vec_el[g]
            );
            let leak = (vec_el[g] - row) / vec_el[g].max(1e-30);
            assert!(
                (0.0..0.05).contains(&leak),
                "lowest-group sub-floor leakage {leak:.3} outside expected [0, 5%)"
            );
        } else {
            // Groups 1..7: no sub-floor leakage, so exact sum-to-vector.
            assert!(
                rel < REL_TOL,
                "group {g}: row sum {row:.6} vs vector {:.6}, rel {rel:.2e} >= {REL_TOL:.0e}",
                vec_el[g]
            );
        }
    }

    // GENDF matrix-record round-trip (mf=6 elastic).
    let section = sm.to_gendf_section(6, 2, 92235.0, 0.0, 0.0);
    let rows = section.to_rows();
    let back = GendfSection::from_rows(6, 2, &rows).expect("parse matrix GENDF section");
    assert_eq!(
        back, section,
        "matrix GENDF section survives the record round trip"
    );
}

/// Notebook op (**LIVE**): `mgxs.Chi` — the group-collapsed fission-emission
/// spectrum vector, on the notebook's 8-group "fine" structure, built by the
/// ported GROUPR fission-spectrum collapse
/// ([`fission_group_chi`]).
///
/// # Methodology
///
/// The notebook tallies `mgxs.Chi` from a pin-cell MC run; njoy's analogue
/// collapses a fission emission spectrum `chi(E')` to the group structure,
/// `chi_g' = integral_{g'} chi(E') dE' / integral chi(E') dE'`. This test runs it
/// on the notebook 8-group fine structure and checks the two properties any
/// physical group Chi must obey, plus the born-fast signature — no notebook MC
/// number is reproducible without a flux solve, so none is asserted as a target:
///
/// 1. **Non-negativity.** Every `chi_g' >= 0` (a physical emission spectrum is
///    non-negative).
/// 2. **Normalization.** `sum_{g'} chi_g' = 1` (probability of being born
///    *somewhere* is 1) — asserted to `< 1e-9`.
/// 3. **Born fast.** Fission neutrons are born in the MeV range, so the two
///    fastest groups (`[5.53 keV, 821 keV]` and `[821 keV, 20 MeV]`) carry
///    essentially all of χ, and the four thermal groups (`< 0.625` eV) carry
///    essentially none.
///
/// # Honesty / V&V scope
///
/// The emission shape is the documented LOW-tier Watt χ
/// ([`FissionSpectrum::default`], U-235 thermal-fission parameters), **not** the
/// incident-energy-dependent MF=5 χ(E'|E) — a flagged fidelity gap shared with
/// the Part I fission-matrix test. This is a self-consistency (property) check,
/// **not** validated against the notebook's MC-tallied Chi or a NJOY GENDF golden
/// tape (op-ini remaining).
///
/// # Results (2026-07-23, U-235 Watt χ, notebook 8-group fine structure)
///
/// - Measured χ vector (groups 0..7, ascending energy):
///   `[6.12e-12, 1.69e-11, 4.21e-11, 1.52e-10, 3.30e-9, 1.80e-4, 0.23932,
///   0.76050]`, sum `= 1` to `< 1e-9`, all non-negative.
/// - Born fast: the top group `[821 keV, 20 MeV]` carries `χ_7 = 0.76050` (the
///   majority); the two fastest groups together carry `χ_6 + χ_7 = 0.99982`; the
///   four thermal groups (`< 0.625` eV) each carry `< 4e-9`.
#[test]
fn group_collapsed_chi() {
    let bounds = NOTEBOOK_FINE_BOUNDS;
    let n_groups = bounds.len() - 1; // 8

    // LOW-tier Watt χ (documented), U-235 thermal-fission parameters.
    let spectrum = FissionSpectrum::default();
    let chi = fission_group_chi(&spectrum, &bounds).expect("group chi");
    assert_eq!(chi.chi.len(), n_groups);

    println!(
        "U-235 group Chi (notebook 8-group fine structure) = {:?}",
        chi.chi
    );

    // Property 1: non-negativity.
    for (g, &c) in chi.chi.iter().enumerate() {
        assert!(
            c.is_finite() && c >= 0.0,
            "χ_{g} = {c} must be finite and non-negative"
        );
    }

    // Property 2: normalization.
    let chi_sum: f64 = chi.chi.iter().sum();
    assert!(
        (chi_sum - 1.0).abs() < 1e-9,
        "group Chi must sum to 1, got {chi_sum}"
    );

    // Property 3: born fast. Groups are ascending in energy; index 7 is
    // [821 keV, 20 MeV], index 6 is [5.53 keV, 821 keV].
    let top = chi.get(7);
    let fast_two = chi.get(6) + chi.get(7);
    println!("  top group χ_7 = {top:.5}, fastest-two χ_6+χ_7 = {fast_two:.5}");
    assert!(
        top > 0.5,
        "top group χ_7 {top:.4} should carry the majority (born fast)"
    );
    assert!(
        fast_two > 0.99,
        "fastest two groups χ_6+χ_7 {fast_two:.4} should carry ~all of χ"
    );
    // The four thermal groups (< 0.625 eV, indices 0..3) carry essentially none.
    for g in 0..4 {
        assert!(
            chi.get(g) < 1e-6,
            "thermal group χ_{g} = {} should be negligible",
            chi.get(g)
        );
    }
}

/// Notebook op (**genuinely `#[ignore]`d — accurate gap**): the **full**
/// `mgxs.ScatterMatrixXS(nu=True)` — the group-to-group nu-scatter matrix summed
/// over *every* scattering reaction with its outgoing multiplicity.
///
/// Only the two-body **elastic** feed (`getdis`) is ported (see the live
/// `scatter_matrix_xs`). The full `nu=True` matrix additionally needs the
/// inelastic (MT=51..91 discrete + continuum), (n,2n) (MT=16), and other
/// secondary-neutron reactions, each requiring the File-6 (MF=6) laboratory
/// secondary energy/angle kinematic feeders — GROUPR `getff` / `cm2lab` — which
/// op-cjw.15 leaves `NotPorted`. Without those feeders the full nu-scatter matrix
/// cannot be assembled, so this stays honestly ignored rather than faked. Gap
/// bead: **op-6tz.6.6** (File-6 kinematic feeders for the full nu-scatter matrix).
#[test]
#[ignore = "full ScatterMatrixXS(nu=True) needs File-6 (MF=6) kinematic feeders \
            (GROUPR getff/cm2lab) for inelastic/(n,2n)/multiplicity — only the \
            two-body elastic feed is ported; elastic slice is live in \
            scatter_matrix_xs (op-6tz.6.6)"]
fn nu_scatter_matrix_full_mt() {
    panic!(
        "full nu-scatter matrix requires MF=6 kinematic feeders (getff/cm2lab), unported; \
         the tractable elastic (MT=2) slice is live in scatter_matrix_xs"
    );
}
