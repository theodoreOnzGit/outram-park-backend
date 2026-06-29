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
- [ ] **Re-run the Ghia Re=100 benchmark on a refined mesh (40×40, 80×80, …)**
  and capture the new centreline `U_x/U_lid` profiles in the test doc comments.
  The current result is a coarse 20×20 first-order-upwind solution
  (max|err| ≈ 0.063, RMS ≈ 0.036 vs Ghia 1982); most of the gap is numerical
  diffusion that mesh refinement (and/or second-order convection) should close.
  - **Blocker:** there is no structured-grid (blockMesh-equivalent) mesh
    generator in the stack. The cavity mesh is read from pre-generated OpenFOAM
    `polyMesh` files, and `openfoam-basic-lib` only has the manual
    `FvMeshBuilder`. Refining means either (a) running OpenFOAM `blockMesh`
    externally to emit new `polyMesh` + regenerate the icoFoam reference
    fields, or (b) **writing a Rust blockMesh-style Cartesian generator** that
    emits an N×N cavity `polyMesh` plus the `0/U`, `0/p` fields with the correct
    movingWall/fixedWalls/frontAndBack BCs. Option (b) is the cleaner long-term
    path — it keeps the case self-contained with no OpenFOAM dependency and
    unblocks parametric mesh-convergence studies for all tutorials.

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
