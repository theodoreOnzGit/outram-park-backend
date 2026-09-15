//! Notebook: **`mdgxs-part-i`** (multi-delayed-group XS) — verification tests (op-6tz.6.4).
//!
//! Notebook provenance: openmc-notebooks `mdgxs-part-i.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! This notebook computes **delayed-neutron** multigroup data for U-235 and
//! Pu-239 over 6 delayed (precursor) groups: `mgxs.DelayedNuFissionXS`,
//! `mgxs.ChiDelayed`, `mgxs.Beta`, and `mgxs.DecayRate` (λ ≈ 0.013–2.85 s⁻¹).
//! The notebook reads that data from the ENDF/B-VIII.0 library through OpenMC.
//! njoy now parses the same delayed chain directly off the ENDF tape:
//!
//! - **ENDF MF=1/455** → [`DelayedNuBar`]: the 6 precursor decay constants λ
//!   \[s⁻¹\] and the total delayed ν̄_d(E).
//! - **ENDF MF=5/455** → [`DelayedChi`]: the delayed fission spectrum χ_delayed,
//!   one subsection per precursor group.
//!
//! The reference for the decay constants and β is therefore the **same
//! ENDF/B-VIII.0 U-235 evaluation** the notebook's `mgxs.DecayRate` /
//! `mgxs.Beta` read; these tests assert njoy recovers those library values and
//! that they satisfy the physical properties (monotonic-in-group λ, delayed
//! fraction β ≈ 0.0065, per-group fractions summing to 1, χ normalized to 1).
//!
//! # Results (2026-07-15, ENDF/B-VIII.0 U-235 `n-092_U_235-ENDF8.0.endf`, MAT 9228)
//!
//! - **Decay constants λ (MF=1/455, LDG=0, 6 groups)**, measured, in s⁻¹:
//!   `[0.013336, 0.032739, 0.12078, 0.30278, 0.84949, 2.853]` — strictly
//!   increasing, all within the physical 0.01–3 s⁻¹ band. These match the
//!   ENDF/B-VIII.0 values the notebook's `mgxs.DecayRate` tabulates.
//! - **Delayed fraction β** = ν̄_d(0.0253 eV) / ν̄_total(0.0253 eV)
//!   = 0.015850 / 2.42985 = **0.006523**, within the accepted U-235 nominal
//!   β ≈ 0.0065.
//! - **Per-group delayed fractions p_k(0.0253 eV)** (MF=5/455 subsection
//!   weights): `[0.0350, 0.1807, 0.1725, 0.3868, 0.1586, 0.0664]`, Σ = 1.000000.
//! - **χ_delayed (MF=5/455, LF=5 general evaporation, θ ≡ 1)**: 6 groups, every
//!   sampled density ≥ 0, trapezoidal normalization per group 0.9947–0.9988
//!   (≈ 1, small tape-rounding deficit).
//!
//! # Still `#[ignore]` (bead op-6tz.6.4)
//!
//! - Delayed-group **MGXS collapse** (`mgxs.DelayedNuFissionXS` /
//!   `mgxs.ChiDelayed` condensed over an energy-group structure) needs the
//!   GROUPR-style group-collapse engine + a flux weight; and Part II's precursor
//!   concentrations need transport tallies (`outram-mc-libs`). See
//!   `mdgxs_part_ii.rs`.

use std::fs::File;

use njoy_outram_park_fork::nuclear_data::delayed::{DelayedChi, DelayedNuBar};
use njoy_outram_park_fork::nuclear_data::secondary::NuBar;
use njoy_outram_park_fork::{endf::tape::Tape, MtReaction};
use uom::si::frequency::hertz;

const U235_MAT: i32 = 9228;

fn u235_tape() -> Tape {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("n-092_U_235-ENDF8.0.endf");
    Tape::read(File::open(&p).expect("open U-235 ENDF tape")).expect("parse U-235 tape")
}

/// Notebook op: `mgxs.DecayRate` — the 6-group precursor decay constants λ.
///
/// Methodology: parse ENDF MF=1/455 (LDG=0) for U-235 and read the precursor
/// decay constants. Pass criterion: exactly 6 groups; every λ positive and in the
/// physical 0.01–3 s⁻¹ band; λ strictly increasing with group index; and each λ
/// within 1e-3 (relative) of the ENDF/B-VIII.0 value the notebook's
/// `mgxs.DecayRate` tabulates.
///
/// Result (2026-07-15): λ = [0.013336, 0.032739, 0.12078, 0.30278, 0.84949,
/// 2.853] s⁻¹ — all criteria met.
#[test]
fn precursor_decay_rates() {
    // The MT tag now has a reader behind it.
    let _tag = MtReaction::Mt455NubarDelayed;
    let d = DelayedNuBar::from_endf(&u235_tape(), U235_MAT)
        .expect("MF=1/455 parse")
        .expect("U-235 has MF=1/455 delayed-neutron data");

    assert_eq!(d.n_groups(), 6, "U-235 has 6 delayed precursor groups");
    assert!(
        !d.ldg1_energy_dependent,
        "U-235 uses energy-independent λ (LDG=0)"
    );

    let lam = d.lambdas();
    println!("U-235 precursor decay constants λ = {lam:?} s⁻¹");

    // Physical band + monotonic-in-group property.
    for (g, &l) in lam.iter().enumerate() {
        assert!(
            (0.01..=3.0).contains(&l),
            "λ[{g}] = {l} s⁻¹ outside 0.01–3 band"
        );
    }
    for g in 1..lam.len() {
        assert!(
            lam[g] > lam[g - 1],
            "λ not increasing at group {g}: {lam:?}"
        );
    }

    // ENDF/B-VIII.0 reference values (read from this tape; the notebook's
    // mgxs.DecayRate reads the same library).
    let reference = [0.013336, 0.032739, 0.12078, 0.30278, 0.84949, 2.853];
    for (g, (&got, &want)) in lam.iter().zip(&reference).enumerate() {
        let rel = (got - want).abs() / want;
        assert!(
            rel < 1e-3,
            "λ[{g}] = {got} vs reference {want} (rel {rel:.1e})"
        );
    }

    // The uom accessor returns the same value as a dimensioned rate.
    let l0 = d.decay_constant(0).unwrap().get::<hertz>();
    assert!((l0 - lam[0]).abs() < 1e-12);
}

/// Notebook op: `mgxs.Beta` / `mgxs.DelayedNuFissionXS` — the delayed fraction β
/// and the per-group delayed emission weights.
///
/// Methodology: read total ν̄ (MF=1/452) and delayed ν̄_d (MF=1/455) at the
/// 2200 m/s point and form β = ν̄_d / ν̄_total; read the 6 per-group fractions
/// p_k(E) from the MF=5/455 subsection headers. Pass criterion: β within the
/// accepted U-235 nominal range 0.006–0.007; the 6 fractions sum to 1; the
/// implied per-group delayed fractions β_k = β·p_k sum back to β.
///
/// Result (2026-07-15): β = 0.015850 / 2.42985 = 0.006523; Σ p_k = 1.000000.
///
/// Note: this is the ENDF *nominal* delayed fraction β, not the transport-adjoint
/// **effective** β_eff (which needs a flux/importance weight from a transport
/// solve — deferred to `outram-mc-libs`).
#[test]
fn delayed_beta_and_nu_fission() {
    let tape = u235_tape();
    let d = DelayedNuBar::from_endf(&tape, U235_MAT).unwrap().unwrap();
    let nu = NuBar::from_endf(&tape, U235_MAT)
        .expect("MF=1/452 parse")
        .expect("U-235 has total ν̄");

    let e = 0.0253;
    let nu_d = d.nu_delayed_at(e);
    let nu_t = nu.at(e);
    let beta = nu_d / nu_t;
    println!("U-235 @ 0.0253 eV: ν̄_d = {nu_d:.6}, ν̄_total = {nu_t:.6}, β = {beta:.6}");
    assert!(
        nu_d > 0.0 && nu_d < 0.05,
        "delayed ν̄_d = {nu_d} out of range"
    );
    assert!(
        (0.006..=0.007).contains(&beta),
        "β = {beta:.6} outside accepted U-235 0.006–0.007"
    );

    // Per-group fractions p_k(E) from the delayed spectrum subsection headers.
    let chi = DelayedChi::from_endf(&tape, U235_MAT).unwrap().unwrap();
    assert_eq!(chi.n_groups(), 6);
    let p_k: Vec<f64> = chi
        .groups
        .iter()
        .map(|g| {
            // Fraction is ≈ constant in E for U-235; take the low-energy value.
            g.fraction.first().map(|&(_, y)| y).unwrap_or(0.0)
        })
        .collect();
    let sum_p: f64 = p_k.iter().sum();
    println!("U-235 per-group delayed fractions p_k = {p_k:?}, Σ = {sum_p:.6}");
    assert!((sum_p - 1.0).abs() < 1e-3, "Σ p_k = {sum_p} should be 1");

    // Implied per-group delayed fractions β_k = β·p_k sum back to β.
    let sum_beta_k: f64 = p_k.iter().map(|&p| beta * p).sum();
    assert!(
        (sum_beta_k - beta).abs() < 1e-9,
        "Σ β_k = {sum_beta_k} ≠ β = {beta}"
    );
}

/// Notebook op: `mgxs.ChiDelayed` — the delayed fission-neutron energy spectrum.
///
/// Methodology: parse ENDF MF=5/455 for U-235 (6 subsections, LF=5 general
/// evaporation with θ ≡ 1, so the tabulated g is χ_delayed(E') directly). Pass
/// criterion: 6 groups; every sampled spectral density non-negative; each group's
/// trapezoidal normalization ∫χ_delayed dE' ≈ 1 (within ±2 %, allowing for
/// tape-rounding of the tabulated points).
///
/// Result (2026-07-15): 6 groups, all densities ≥ 0, per-group normalization
/// 0.9947–0.9988.
#[test]
fn delayed_chi_spectrum() {
    let chi = DelayedChi::from_endf(&u235_tape(), U235_MAT)
        .expect("MF=5/455 parse")
        .expect("U-235 has an MF=5/455 delayed fission spectrum");

    assert_eq!(chi.n_groups(), 6, "U-235 has 6 delayed spectrum groups");
    for (k, g) in chi.groups.iter().enumerate() {
        assert_eq!(
            g.lf, 5,
            "U-235 delayed χ group {k} is LF=5 (general evaporation)"
        );
        assert!(!g.spectrum.is_empty(), "group {k} has a tabulated spectrum");
        assert!(g.min_density() >= 0.0, "group {k} has a negative density");
        let norm = g.normalization();
        println!(
            "U-235 χ_delayed group {k}: {} pts, ∫ = {norm:.6}",
            g.spectrum.len()
        );
        assert!(
            (0.98..=1.02).contains(&norm),
            "group {k} χ_delayed normalization {norm:.6} not ≈ 1"
        );
    }
}
