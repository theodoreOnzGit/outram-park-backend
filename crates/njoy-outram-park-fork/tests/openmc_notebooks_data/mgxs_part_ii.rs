//! Notebook: **`mgxs-part-ii`** (multigroup XS: matrices & Chi) — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `mgxs-part-ii.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! Part II extends the pin-cell MGXS to **group-to-group scatter matrices**
//! (`mgxs.ScatterMatrixXS`), the fission spectrum vector (`mgxs.Chi`),
//! per-nuclide tallies, energy condensation, and a deterministic (OpenMOC)
//! cross-check. All of these are flux-solved, tally-based quantities plus a
//! transport verification, none of which this data crate provides. njoy's
//! `Mgxs` carries only *diagonal* group cross sections (no transfer matrix) and a
//! pointwise `FissionSpectrum` (not a group-collapsed Chi vector). The whole
//! notebook is therefore scaffolded `#[ignore]`.
//!
//! # Gaps (bead under op-6tz.6, shared with mgxs-part-i)
//!
//! - Group-to-group scatter matrix (needs MF=6 kinematics / GROUPR `getff`).
//! - Group-collapsed Chi vector.
//! - Transport-based (OpenMOC/MC) verification.

/// Notebook op: `mgxs.ScatterMatrixXS(nu=True)` — the group-to-group scatter
/// matrix. njoy's `Mgxs` has no transfer matrix; building one needs MF=6
/// scattering kinematics (GROUPR `getff`), not ported.
#[test]
#[ignore = "requires group-to-group scatter matrix (MF=6 kinematics / GROUPR getff) (op-6tz.6)"]
fn scatter_matrix_xs() {
    panic!("njoy Mgxs carries diagonal σ_g only; no group-to-group scatter matrix (MF=6 kinematics unported)");
}

/// Notebook op: `mgxs.Chi` — the group-collapsed fission spectrum vector. njoy
/// has a pointwise `FissionSpectrum` but no group-collapsed Chi over an MGXS
/// group structure.
#[test]
#[ignore = "requires group-collapsed Chi over an MGXS structure (op-6tz.6)"]
fn group_collapsed_chi() {
    panic!("njoy has pointwise χ (FissionSpectrum) but no group-collapsed Chi vector");
}
