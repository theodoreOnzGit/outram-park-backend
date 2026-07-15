//! Notebook: **`mdgxs-part-i`** (multi-delayed-group XS) — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `mdgxs-part-i.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! This notebook computes **delayed-neutron** multigroup data for U-235 and
//! Pu-239 over 6 delayed (precursor) groups: `mgxs.DelayedNuFissionXS`,
//! `mgxs.ChiDelayed`, `mgxs.Beta`, and `mgxs.DecayRate` (λ ≈ 0.013–2.85 s⁻¹).
//! njoy currently parses only the **total** ν̄ (ENDF MF=1/452); the delayed chain
//! — delayed ν̄ and precursor decay constants (MF=1/455) and the delayed fission
//! spectrum (MF=5/455) — is not parsed. `MtReaction::Mt455NubarDelayed` exists as
//! an enum tag only, with no reader behind it. Every operation here is therefore
//! scaffolded `#[ignore]`.
//!
//! # Gaps (bead under op-6tz.6)
//!
//! - ENDF MF=1/455 delayed ν̄ + precursor decay constants λ.
//! - ENDF MF=5/455 delayed fission spectrum χ_delayed.
//! - Delayed-group MGXS (β, delayed ν·σ_f, χ_delayed) collapse.

use njoy_outram_park_fork::MtReaction;

/// Notebook op: `mgxs.DecayRate` — the 6-group precursor decay constants λ.
/// Requires an ENDF MF=1/455 reader (delayed ν̄ + λ). Only the MT tag exists.
#[test]
#[ignore = "requires ENDF MF=1/455 delayed-neutron reader (decay constants λ) (op-6tz.6)"]
fn precursor_decay_rates() {
    // The MT tag is defined, but no parser is wired to it.
    let _tag = MtReaction::Mt455NubarDelayed;
    panic!("njoy has no ENDF MF=1/455 reader — delayed ν̄ / precursor decay constants unavailable");
}

/// Notebook op: `mgxs.Beta` / `mgxs.DelayedNuFissionXS` — delayed fraction and
/// delayed fission production per precursor group. Requires MF=1/455.
#[test]
#[ignore = "requires delayed ν̄ (ENDF MF=1/455) + delayed-group MGXS (op-6tz.6)"]
fn delayed_beta_and_nu_fission() {
    panic!("njoy parses only total ν̄ (MF=1/452); no delayed ν̄ or β");
}

/// Notebook op: `mgxs.ChiDelayed` — the delayed fission-neutron energy spectrum.
/// Requires an ENDF MF=5/455 reader.
#[test]
#[ignore = "requires ENDF MF=5/455 delayed fission spectrum reader (op-6tz.6)"]
fn delayed_chi_spectrum() {
    panic!("njoy has no ENDF MF=5/455 reader — delayed χ unavailable");
}
