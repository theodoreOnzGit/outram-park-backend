# checkMesh consolidation (gh:#1480)

Research, education and V&V only (`RESPONSIBLE_USE.md`).

## What moved

The OpenFOAM mesh-quality checks existed three times. They are now one module,
`outram_foam_mesh::check_mesh` (`src/check_mesh.rs`):

| old copy | now |
|---|---|
| `outram-park-fork-cfmesh` `src/checks.rs` (`check_quality`, `faces_failing_mesh_quality`, `QualityReport`, `MeshQualityControls`) | code moved to `check_mesh`; `checks.rs` is an adapter (cfMesh geometry in, shared checks) re-exporting the types. `cell_geometry` stays (it is geometry, used crate-wide). |
| `outram-foam-mesh` `mesh_quality.rs` `assess_quality` (metrics, `face_skewness`, `cell_aspect_ratio`) | calls `check_quality` with `CheckOptions::foam_mesh_legacy()`; its own geometry loop stays. |
| `outram-foam-mesh` `poly_topology.rs` `PolyPatchMesh::quality` (+ private `face_skewness`) | calls `check_quality` with `foam_mesh_legacy()` and `sliver_metrics: false`. |

Reference followed: the cfMesh version (maintainer), itself a port of OpenFOAM-dev
`852527377c7e87e33d51b19ca8974b5db60af28b`, `src/meshCheck/checkMesh.C`,
`polyMeshCheck/polyMeshCheckQuality.C`, `primitiveMeshCheck/primitiveMeshCheck.C`
(read in `upstream_source/OpenFOAM-dev`).

## Differences between the copies (read from upstream and the code)

See the table in gh:#1486. In short: boundary skewness (dev projects onto the
normal; `outram-foam-mesh` mirrored the owner cell, which is identically zero),
`svHat` divide vs reciprocal-multiply, skewness guard (1e-150 vs 1e-300),
degenerate-face handling, aspect-ratio definition, and face/cell-centre geometry.
Maintainer decision: **consolidate the code, not the behaviour**; every difference is
an explicit `CheckOptions` field and each caller keeps its numbers.

## Method and prediction

Prediction, written before the first run of the new code: all results
bit-identical, because each option reproduces an old formula operation for
operation. Instruments:

1. `tests/check_mesh_consolidation.rs`: 15 quantities from `assess_quality` and
   `PolyPatchMesh::quality` on two deterministic jittered 6x5x4 hex grids
   (points displaced up to 0.1 and 0.25 cell widths), recorded as `f64::to_bits`
   from the **old** code (commit `27df6f6c9`) and asserted equal on the new.
   Castellated / snapped sphere meshes were tried first and rejected as an
   instrument: castellation numbers points in `HashMap` order (gh:#1004), and the
   unchanged old code gave last-bit-different sums between runs.
2. The cfMesh pins (10 tests, plus the 9 `checks` tests), run before the change
   (baseline) and after.

## Results (2026-11-11, release, x86_64)

- `check_mesh_consolidation_is_bit_identical`: **pass**, all 30 values bit-identical.
  `max_boundary_skewness` is exactly 0 on both meshes (the mirrored form is
  identically zero; see gh:#1486).
- cfMesh `--lib` (`checks::`, `pinned_bit_for_bit`, `layer_strategies_are_deterministic`,
  `fingerprint`, `restart`): 10 pins pass before and 22 selected tests pass after the
  change, bit-identical.
- Further results (feature `cell-centre-dual` pin, wasm32, Android, lockstep, full
  suites) are appended below as they are run.
