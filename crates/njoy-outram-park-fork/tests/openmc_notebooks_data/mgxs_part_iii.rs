//! Notebook: **`mgxs-part-iii`** (MGXS library export + MG-mode) — verification tests (op-6tz.6).
//!
//! Notebook provenance: openmc-notebooks `mgxs-part-iii.ipynb`, commit
//! `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT).
//!
//! # Methodology
//!
//! Part III builds an `openmc.mgxs.Library` for a 17×17 assembly, exports it to
//! **HDF5** (`build_hdf5_store`), then runs a multigroup-mode transport
//! calculation and compares its k-eff to the continuous-energy result (the
//! notebook reports ~688 pcm). njoy *does* have a multigroup library container
//! (`MgxsLibrary::pack`/`from_blob`), but in its own compact **MGXL** blob
//! format, not the OpenMC HDF5 layout, and the MG-mode transport + k-eff
//! comparison is a transport capability (`outram-mc-libs`). The notebook is
//! scaffolded `#[ignore]`.
//!
//! # Gaps (bead under op-6tz.6, shared with mgxs-part-i)
//!
//! - OpenMC-format (HDF5) MGXS library export (njoy has MGXL, not HDF5).
//! - MG-mode transport + CE-vs-MG k-eff verification (transport).

/// Notebook op: `openmc.mgxs.Library(...).build_hdf5_store(...)` — export the MGXS
/// library in OpenMC's HDF5 layout. njoy serialises to its own MGXL blob
/// (`MgxsLibrary::pack`), not HDF5; the format bridge is unbuilt.
#[test]
#[ignore = "requires OpenMC-format (HDF5) MGXS export; njoy uses its own MGXL blob (op-6tz.6)"]
fn export_mgxs_library_hdf5() {
    panic!("njoy MgxsLibrary serialises to MGXL, not OpenMC HDF5; no format bridge yet");
}

/// Notebook op: MG-mode transport and continuous-energy-vs-multigroup k-eff
/// comparison. Requires a transport solver in both CE and MG modes
/// (`outram-mc-libs`).
#[test]
#[ignore = "requires MG-mode transport + CE-vs-MG k-eff (outram-mc-libs / op-6tz transport slice)"]
fn mg_mode_keff_verification() {
    panic!("MG-mode transport + CE-vs-MG k-eff is a transport capability — outram-mc-libs owns this");
}
