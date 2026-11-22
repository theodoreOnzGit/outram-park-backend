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

## Results (2026-11-12, release, x86_64-unknown-linux-gnu)

**Decision: option (a), by calling the fixed basic-lib code, not by re-exporting.**
Option (b) does not compose: the fork's `VolScalarField`, `SurfaceScalarField`,
`FvMatrix`, `FvVectorMatrix` and boundary-condition types are its own vendored
copies (only `FvMesh` and the tensor primitives were deduped to basic-lib,
gh:#492), so basic-lib's operators take different argument types. The
dependency direction is not the obstacle (`outram-foam-basic-lib` is already a
dependency and precedes this crate in `scripts/release-lockstep.sh --dry-run`,
no cycle); the type stack is. The three forked operators now take the boundary
distance from `DeltaCoeff::Orthogonal.boundary_delta` (basic-lib, gh:#1488), so
`patch_normal_delta` stays in one place.

**Before / after** (`cargo test --release -p tampines-steam-tables --lib --tests`):

| | before | after |
|---|---|---|
| passed / failed / ignored | 1140 / 0 / 14 | 1143 / 0 / 14 |
| lib | 1049 passed | 1052 (+3 new gh1494 tests) |
| `tests/edwards_blowdown.rs` | passed, 298.00 s | passed, 295.14 s |
| `tests/cross_platform_determinism.rs` (FNV-1a-64 goldens) | 3 passed | 3 passed, no golden changed |
| all other `tests/*` | pass | pass |

**Moved numbers: none.** Predicted none (the pipe mesh is axis-aligned, so
`sqrt(dx^2) = |dx|` exactly); measured none: the determinism goldens are
bit-hashes and did not move, so the Edwards and `rhoPimpleFoam` paths are
bit-identical to before. The prediction could have failed on either golden and
did not.

**New tests** (each fails against the old `|Cf - C|` form: 2.828 vs 4 on the
skewed face): `fvm::laplacian::tests::boundary_conductance_uses_the_patch_normal_delta_gh1494`,
`fvm::laplacian_vec::tests::boundary_conductance_uses_the_patch_normal_delta_gh1494`,
`fvc::sn_grad::tests::boundary_sn_grad_uses_the_patch_normal_delta_gh1494`.
Skewed 2-cell mesh, right face centre `(1, 0.25, 0)`: boundary coefficient 4
(`1/n.d`, `n.d = 0.25`) instead of 2.828 (`1/|d|`). Every changed function is
also reached by the existing solver tests.

**Scope, stated not claimed:** only the non-coupled boundary distance of these
three operators changed. The interior distances, the mixed / fixedGradient
arms and the rest of the vendored operators (`fvc::interpolate`, `grad`, `div`,
`muscl`) are the older copies and were not compared with upstream here.
No tolerance was changed. No kovan-reviewed function was touched (the three
folders' `review.md` blocks are `kind = "note"`).
