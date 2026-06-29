# openfoam-appbuilder-lib — TODO

Remaining work on the solver test suites and the solvers themselves, captured
after wiring up the pimpleFoam / rhoCentralFoam / rhoPimpleFoam tutorial tests.

## Test-suite status

| Tutorial | Active tests | Ignored | Blocker |
|---|---|---|---|
| `pimple_foam_cavity` | mesh load, velocity-vs-icoFoam (1.4 % L∞) | Ghia Re=100 | mesh is Re=10; needs a ν=1e-3 re-run |
| `rho_central_foam_shock_tube` | mesh load, pressure (0.7 % L1), shock position | — | none |
| `rho_pimple_foam_aerofoil_naca0012` | mesh load | Cp, CL, mass conservation | k-ω SST turbulence stub |

## Solvers

### pimpleFoam (`solvers::pimple_foam`)
Fixed/added this session: momentum Laplacian sign, pressure-source sign,
`constrainHbyA` boundary flux, PCG pressure solve, per-step BC re-application,
**proper PISO corrector loop** (H(U) re-evaluated each pass — the Co≈0.85
stability fix), and **`fvc::ddtCorr`** with the `fvcDdtPhiCoeff` limiter. The
cavity now runs at icoFoam's dt = 5e-3 and matches to 1.4 %. Remaining:
- [ ] **Second-order convection** (`Gauss linear`) option — the port uses
  first-order upwind (`fvm::div`), which is the bulk of the remaining 1.4 %
  cavity difference vs icoFoam. A linear/limited-linear scheme would tighten it.
- [ ] Un-ignore `cavity_ghia_benchmark_re100` after re-running the case at
  ν = 1e-3 (Re = 100) so the shipped Ghia 1982 data applies.

### rhoCentralFoam (`solvers::rho_central_foam`)
Fixed/added: boundary-face flux (the end cells were missing their wall pressure
force, producing a 5× spike), and **2nd-order vanLeer MUSCL reconstruction**
(`fvc::reconstruct_pos_neg`) — shock-tube L1 error dropped 3.7 % → 0.7 %. The
KNP flux now matches OpenFOAM rhoCentralFoam's scheme family. Nothing
outstanding for the Sod tube.

### Convection schemes (`openfoam-basic-lib::fvc::muscl`)
`reconstruct_pos_neg` (Upwind / Linear / VanLeer / Minmod limiters) is the
explicit, density-based path used by rhoCentralFoam. Remaining:
- [ ] **Limited *implicit* convection for `fvm::div`** (deferred correction) —
  pimpleFoam still uses first-order upwind in its momentum matrix, the bulk of
  the remaining 1.4 % cavity difference. This needs the limited face value as an
  explicit source correction on top of the upwind matrix, not the explicit
  reconstruction used here.

### rhoPimpleFoam (`solvers::rho_pimple_foam`)
- [ ] **Apply the proven pimpleFoam coupling fixes** — this solver still has the
  same structure that was broken in pimpleFoam: `- fvm::laplacian_vec` (should be
  `+`), `+= phi_int` pressure source (should be `-=`, negated), unconstrained
  HbyA boundary flux, Gauss-Seidel pressure solve, a single-pass corrector that
  never re-evaluates H(U), and no ddtCorr. It will diverge as-is. Port the full
  set now proven on pimpleFoam (sign, sign, constrainHbyA, `solve_cg`, per-step
  BC re-application, the PISO corrector loop restructure, and `fvc::ddtCorr`),
  then validate.
- [ ] **k-ω SST turbulence model** — now **implemented and unit-tested** in
  `openfoam-turbulence-lib` (F1/F2 blending, νt stress limiter, k/ω transport,
  wall distance, `div_dev_rho_reff`). Still to do for the aerofoil: (a) wire the
  model into `RhoPimpleFoam` (call `div_dev_rho_reff` in the momentum predictor
  and `correct()` after the pressure loop), and (b) turbulence wall-function
  boundary conditions (`nutkWallFunction`, `omegaWallFunction`, …) — without
  them the near-wall k/ω are unphysical on a y⁺ > 11 mesh.

### Mesh refinement for the lid-driven cavity (`pimple_foam_cavity`)
- [x] **Re-run the Ghia Re=100 benchmark on a refined mesh.** Done on a 41×41
  blockMesh-generated mesh (`constant_fine_mesh/polyMesh`, 1681 cells) added by
  hand. New test `cavity_ghia_benchmark_re100_fine_mesh` advances ν = 1e-3
  (Re = 100) to steady state at dt = 2e-3 (Co ≈ 0.8) and compares the vertical
  centreline U_x to the 17 Ghia 1982 points. Refinement roughly tripled the
  accuracy over the 20×20 first-order-upwind solution:
  `max|err|` 0.0634 → **0.0194**, RMS 0.0363 → **0.0113** (U_x/U_lid). Captured
  CSV is in the test's doc comment.
- [ ] **Optional: go finer / second-order.** An 80×80+ mesh and/or the
  second-order `Gauss linear` convection option (see the pimpleFoam item above)
  would push toward the 2 % a fine second-order reference reaches. Blocked on a
  generator only for *programmatic* sweeps — see below; one-off meshes can keep
  being added by hand like `constant_fine_mesh`.

## Library (`openfoam-basic-lib`)
- [ ] Consider having `FvMatrix::solve` auto-select `solve_cg` for symmetric
  (`upper == lower`) systems instead of requiring callers to pick. PCG was ~170×
  faster than Gauss-Seidel on the 400-cell pressure Poisson here.
- [ ] **Structured Cartesian mesh generator (blockMesh equivalent)** — a Rust
  function that builds an `FvMesh` / writes a `polyMesh` for a box of `nx×ny×nz`
  cells with named boundary patches. Unblocks cavity mesh refinement (above) and
  parametric mesh-convergence studies generally. See root CLAUDE.md: Layer-5
  solver loops stay in solver crates, but a primitive mesh generator belongs in
  `openfoam-basic-lib` alongside the polyMesh reader.

## I/O (`io::field_reader`, `io::poly_mesh`)
- [ ] BC reader maps unmodelled OpenFOAM BC types (inletOutlet, fixedFluxPressure,
  waveTransmissive, calculated, …) to `ZeroGradient` as a best-effort fallback.
  Implement the real BCs when the compressible/turbulent cases need them.
