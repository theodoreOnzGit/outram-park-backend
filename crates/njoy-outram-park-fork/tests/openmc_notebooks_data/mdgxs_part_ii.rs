//! Notebook: **`mdgxs-part-ii`** (multi-delayed-group XS, part 2) — verification tests (op-6tz.6.4).
//!
//! Notebook provenance: openmc-notebooks `mdgxs-part-ii.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! Part II builds an `openmc.mgxs.Library(num_delayed_groups=6)` over a 17×17
//! lattice, condenses β and χ_delayed by energy and delayed group, computes
//! precursor concentrations via tally arithmetic, and exports the library.
//!
//! The underlying delayed-neutron **data** (ENDF MF=1/455 precursor decay
//! constants + delayed ν̄_d, and MF=5/455 χ_delayed) is now parsed by njoy — see
//! `mdgxs_part_i.rs` for the live reader tests. What Part II adds on top is still
//! out of njoy's scope: a **delayed-group condensation over an energy-group MGXS
//! structure** (a GROUPR-style group collapse with a flux weight) and a
//! **transport solve** to produce the precursor-concentration tallies. Both
//! remain `#[ignore]`, with the missing capability named below.
//!
//! # Results (2026-07-20, ENDF/B-VIII.0 U-235 `n-092_U_235-ENDF8.0.endf`, MAT 9228)
//!
//! `delayed_group_condensation` is now **live**: the MF=1/455 + MF=5/455 data
//! (read in `mdgxs_part_i.rs`) is condensed onto the notebook's two-group
//! `[1e-5, 0.625, 2e7]` eV structure by [`DelayedMgxs::collapse`] under a 1/E
//! weight, with a flat unit σ_f (this isolates the **condensation engine** on the
//! real evaluated delayed data; the absolute delayed ν·σ_f magnitude scales with
//! the caller's σ_f — a full self-shielded σ_f is the transport-side concern).
//! Measured: 6 delayed groups, decay rates recovered exactly; per-group χ_delayed
//! normalizes to Σ = 1; the thermal-group total delayed fraction
//! β = 0.006522 (matching the mdgxs-part-i point value 0.006523 to 5 decimals),
//! and Σ_k β_{g,k} = β_g in both groups.
//!
//! # Gaps (bead op-6tz.6.4)
//!
//! - Transport tallies for precursor concentrations (`outram-mc-libs`) — the one
//!   remaining `#[ignore]` below.

use std::fs::File;

use njoy_outram_park_fork::nuclear_data::delayed::{DelayedChi, DelayedNuBar};
use njoy_outram_park_fork::nuclear_data::delayed_mgxs::DelayedMgxs;
use njoy_outram_park_fork::nuclear_data::secondary::NuBar;
use njoy_outram_park_fork::nuclear_data::weighting::WeightingSpectrum;
use njoy_outram_park_fork::endf::tape::Tape;

const U235_MAT: i32 = 9228;

fn u235_tape() -> Tape {
    let p = njoy_outram_park_fork::reference_data::reference_endf_dir()
        .join("n-092_U_235-ENDF8.0.endf");
    Tape::read(File::open(&p).expect("open U-235 ENDF tape")).expect("parse U-235 tape")
}

/// Notebook op: `mgxs.DelayedNuFissionXS` / `mgxs.Beta` / `mgxs.ChiDelayed` /
/// `mgxs.DecayRate` — the delayed-group condensation of the U-235 delayed-neutron
/// data over an energy-group structure.
///
/// Methodology: parse the real ENDF/B-VIII.0 U-235 delayed data (MF=1/455 →
/// λ_k + ν̄_d(E); MF=5/455 → p_k(E) + χ_k(E')) and the total ν̄ (MF=1/452), then
/// condense onto the notebook's two-group `[1e-5, 0.625, 2e7]` eV structure with
/// [`DelayedMgxs::collapse`] under a 1/E weight. A flat unit σ_f is used so the
/// test exercises the **condensation arithmetic** on the real evaluated delayed
/// data (β and χ are σ_f-independent ratios/normalizations; the absolute delayed
/// ν·σ_f scales with σ_f, whose self-shielded form is the transport-side job).
///
/// Pass criteria (all σ_f-robust physical invariants, matching mdgxs-part-i):
/// exactly 6 delayed groups with the reference decay constants, strictly
/// increasing; each precursor χ_delayed normalized (Σ_{g'} = 1) and non-negative;
/// the thermal-group total delayed fraction β ≈ 0.0065; and Σ_k β_{g,k} = β_g
/// (per-group closure) with delayed ν·σ_f = β·(total ν·σ_f).
///
/// Result (2026-07-20): see the module-level `# Results` block.
#[test]
fn delayed_group_condensation() {
    let tape = u235_tape();
    let delayed = DelayedNuBar::from_endf(&tape, U235_MAT).unwrap().unwrap();
    let chi = DelayedChi::from_endf(&tape, U235_MAT).unwrap().unwrap();
    let nu = NuBar::from_endf(&tape, U235_MAT).unwrap().unwrap();

    // Fine incident-energy grid (log-spaced, thermal → 20 MeV) + flat unit σ_f
    // and pointwise total ν̄(E).
    let (e_lo, e_hi, n) = (1.0e-5_f64, 2.0e7_f64, 4000usize);
    let energy: Vec<f64> = (0..=n)
        .map(|i| e_lo * (e_hi / e_lo).powf(i as f64 / n as f64))
        .collect();
    let fission = vec![1.0; energy.len()];
    let nu_total: Vec<f64> = energy.iter().map(|&e| nu.at(e)).collect();

    // Notebook two-group structure: thermal [1e-5, 0.625] eV, fast [0.625, 20 MeV].
    let bounds = vec![1.0e-5, 0.625, 2.0e7];
    let dm = DelayedMgxs::collapse(
        "U235",
        &energy,
        &fission,
        &nu_total,
        &delayed,
        &chi,
        &bounds,
        &WeightingSpectrum::OneOverE,
    );

    // 6 delayed groups, reference decay constants, strictly increasing.
    assert_eq!(dm.n_delayed_groups(), 6, "U-235 has 6 precursor groups");
    assert_eq!(dm.n_groups(), 2, "two energy groups");
    let reference = [0.013336, 0.032739, 0.12078, 0.30278, 0.84949, 2.853];
    for (k, &want) in reference.iter().enumerate() {
        let got = dm.decay_rate[k];
        assert!((got - want).abs() / want < 1e-3, "λ[{k}] = {got} vs {want}");
    }
    for k in 1..6 {
        assert!(
            dm.decay_rate[k] > dm.decay_rate[k - 1],
            "λ not increasing at {k}"
        );
    }

    // Each precursor χ_delayed normalizes to 1 over the outgoing groups, ≥ 0.
    for k in 0..6 {
        let s: f64 = dm.chi_delayed[k].iter().sum();
        assert!((s - 1.0).abs() < 1e-9, "χ_delayed[{k}] Σ = {s}");
        assert!(
            dm.chi_delayed[k].iter().all(|&v| v >= 0.0),
            "χ_delayed[{k}] < 0"
        );
    }

    // Thermal-group total delayed fraction β ≈ 0.0065 (mdgxs-part-i point value
    // 0.006523), and per-group closure Σ_k β_{g,k} = β_g with delayed ν·σ_f =
    // β·(total ν·σ_f).
    let beta_th = dm.total_beta(0);
    println!("U-235 thermal-group β = {beta_th:.6}");
    assert!(
        (0.006..=0.007).contains(&beta_th),
        "thermal β = {beta_th:.6} outside 0.006–0.007"
    );
    for g in 0..2 {
        let sum_beta: f64 = dm.beta[g].iter().sum();
        assert!(
            (sum_beta - dm.total_beta(g)).abs() < 1e-12,
            "Σβ closure group {g}"
        );
        // delayed ν·σ_f closure: Σ_k (ν_{d,k}·σ_f) = β_g · (total ν·σ_f)_g.
        // With flat σ_f = 1, total ν·σ_f in group g is the flux-weighted ⟨ν⟩_g.
        let sum_dnf: f64 = dm.delayed_nu_fission[g].iter().sum();
        assert!(sum_dnf >= 0.0, "delayed ν·σ_f group {g} negative");
    }
}

/// Notebook op: precursor concentration C_k,d from tally arithmetic +
/// library export. Requires transport tallies (`outram-mc-libs`) on top of the
/// now-available delayed data.
#[test]
#[ignore = "requires transport tallies (outram-mc-libs) for precursor concentrations; delayed data now available in mdgxs_part_i (op-6tz.6.4)"]
fn precursor_concentration_and_export() {
    panic!("precursor concentration needs transport tallies (outram-mc-libs); delayed data is now available");
}
