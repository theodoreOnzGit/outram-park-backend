# gh:#1380 predictions, committed BEFORE any ESI or Rust comparison run

Date: 2026-11-14. Subject: `outram_foam_mesh::dynamic_mesh` (solid-body mesh motion
and `meshPhi`) against OpenFOAM.com v2506 `moveDynamicMesh` loop
(`dynamicMotionSolverFvMesh` + `solidBodyMotionSolver`), probed by `dumpMeshMotion`.

Cases: meshes `warped3d` (hex, 120 cells) and `dual_warped3d` (polyhedral), motions
`linearMotion`, `rotatingMotion`, `axisRotationMotion`, `oscillatingLinearMotion`;
deltaT 0.01, 5 steps; every quantity compared by IEEE-754 bit pattern.

Reasoning shared by all predictions: the port repeats the upstream expression trees
operation for operation (quaternion `R()`, `septernion` composition, `transformPoints`,
`face::centre`, `triangle::sweptVol`, dot `x*x+y*y+z*z`, cross, `vs/s` normalisation).
ESI is compiled `-O3` without `-ffast-math` or `-march`, so no FMA contraction or
reassociation; rustc also never contracts. `sin`/`cos`/`sqrt` are the system glibc /
IEEE in both.

| # | Quantity | Prediction |
|---|---|---|
| D1 | `points` after linearMotion | 100 % bit-identical (the only operation is `x - (-v*t)`). |
| D2 | `points` after rotatingMotion / axisRotationMotion | 100 % bit-identical. The one risk is glibc `sin`/`cos` differing between a C++ and a Rust call; both link the same shared libm, so no difference expected. |
| D3 | `points` after oscillatingLinearMotion | 100 % bit-identical (same `sin`). |
| D4 | `meshPhi` (all faces, both meshes, all motions) | 100 % bit-identical: the `sweptVol` sum is order-sensitive but ported in order. Fallback if a residual appears: 1-ulp differences confined to faces with more than 4 vertices (the centre computation), localised and replayed. |
| D5 | `faceCentres`, `faceAreas`, `cellCentres`, `cellVolumes` on moved points | 100 % bit-identical: `check_mesh::esi_geometry` is already bit-identical to ESI on the static meshes (gh:#1364). |
| D6 | Geometric conservation law (independent of ESI): for every cell, the sum of `meshPhi` over its faces (owner +, neighbour -) times deltaT equals `V_new - V_old` | rigid motions: `|sum*dt| < 1e-12 * V` per cell (round-off only, not bit-zero). |
| D7 | Zero-rotation guard: `mag(R - I) > SMALL` skips the rotation for linear motion | linear points equal `x + v t` evaluated as `x - (-(v t))`, bit-identical to plain addition. |

Not attempted this session (recorded as child issues): `displacementLaplacian`
(needs the GAMG/PCG solver and `cellDisplacement` interpolation; bit-identity with
upstream's solver is not expected), pointPatch mesh-motion boundary conditions,
topology changers (`dynamicRefineFvMesh`), `sixDoFRigidBodyMotion`.
