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

1. The solvers here run on 1-D, axis-aligned pipe meshes: the boundary face
   normal is exactly `(+-1,0,0)` and `Cf - C` is purely axial. Then
   `nHat.d = +-dx`, `nHat*(nHat.d) = dx` and `sqrt(dx^2) = |dx|` exactly, so the
   new distance is bit-identical to the old. **Prediction: no existing number
   moves, not even in the last bit.** That covers the `rhoPimpleFoam` unit tests
   and any `edw_pimple_*`-type FNV-1a-64 hashes in
   `tests/cross_platform_determinism.rs`; the property-library V&V
   (`water_critical_point_iapws95.md`, the CoolProp reference tests) does not
   call these operators at all and cannot move.
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
distance from `DeltaCoeff::Orthogonal.boundary_delta` (basic-lib, gh:#1488).

**Before / after** (`cargo test --release -p outram-park-fork-coolprop --lib --tests`):

| | before | after |
|---|---|---|
| passed / failed / ignored | 325 / 0 / 4 | 328 / 0 / 4 |
| lib | 246 passed (236 s) | 249 (+3 new gh1494 tests) |
| `tests/cross_platform_determinism.rs` | 3 passed | 3 passed, no golden changed |
| all other `tests/*` | pass | pass |

**Moved numbers: none**, as predicted (axis-aligned 1-D mesh; the property
library V&V does not call these operators). **New tests:** the same three
as in `tampines-steam-tables` (`..._uses_the_patch_normal_delta_gh1494`), on a
skewed 2-cell mesh, boundary coefficient 4 instead of 2.828. Same scope
limits and the same statement on tolerances and kovan review stamps as there.
