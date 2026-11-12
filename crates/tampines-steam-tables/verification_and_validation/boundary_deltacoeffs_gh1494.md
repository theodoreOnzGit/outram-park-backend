# Boundary `deltaCoeffs` in the forked FV operators (gh:#1494, from gh:#1488)

## Prediction (written and committed BEFORE the code change, 2026-11-12)

**Defect.** The forked `fvm::laplacian`, `fvm::laplacian_vec` and
`fvc::sn_grad` (`openfoam_algorithms/openfoam_source/fv_operators/`) divide by
`|Cf - C|` on a boundary face. ESI v2506 uses `1/mag(fvPatch::delta())` with
`fvPatch::delta() = nHat*(nHat & (Cf - Cn))` (`fvPatch.C:156`); gh:#1488 fixed
the same defect in `outram-foam-basic-lib`.

**Change.** The fork keeps its own field and matrix types, so the three
operators cannot be re-exported from basic-lib (option (b) of #1494). They take
the boundary distance from the fixed
`outram_foam_basic_lib::fv_operators::fvm::DeltaCoeff::Orthogonal.boundary_delta`
(`FvMesh` is already basic-lib's), so upstream's operation order lives in one
place and this is not a third copy.

**What I predict will move, with sign and size.**

1. The solvers here run on 1-D, axis-aligned pipe meshes
   (`create_one_d_mesh`): the boundary face normal is exactly `(+-1,0,0)` and
   `Cf - C` is purely axial. Then `nHat.d = +-dx`, `nHat*(nHat.d) = dx` and
   `sqrt(dx^2) = |dx|` exactly, so the new distance is bit-identical to the old.
   **Prediction: no existing number moves, not even in the last bit.** That
   covers the Edwards 24-cell blowdown (`tests/edwards_blowdown.rs`), the
   `rhoPimpleFoam` unit tests (including
   `axial_conduction_matches_analytical_fourier_decay`), the HEM V&V records and
   the Part B `edw_pimple_*` FNV-1a-64 hashes in
   `tests/cross_platform_determinism.rs`.
2. A value can only move on a mesh whose boundary face is not perpendicular to
   `Cf - C`. The **new** unit tests (a skewed 2-cell mesh) will show the
   boundary conductance and the `sn_grad` on a fixedValue face LARGER by
   `1/cos(theta)` than the old `1/|d|` form (positive, +41 % at 45 degrees).
   These are new tests, not moved numbers.
3. If any existing hash or V&V number moves, the 1-D-mesh assumption in (1) is
   wrong. That is a finding to investigate, not a golden to re-pin.
