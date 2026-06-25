# openfoam-appbuilder-lib — TODO

Remaining work on the solver test suites and the solvers themselves, captured
after wiring up the pimpleFoam / rhoCentralFoam / rhoPimpleFoam tutorial tests.

## Test-suite status

| Tutorial | Active tests | Ignored | Blocker |
|---|---|---|---|
| `pimple_foam_cavity` | mesh load, velocity-vs-icoFoam (1.4 % L∞) | Ghia Re=100 | mesh is Re=10; needs a ν=1e-3 re-run |
| `rho_central_foam_shock_tube` | mesh load, pressure (3.7 % L1), shock position | — | none |
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
Fixed this session: boundary-face flux (the end cells were missing their wall
pressure force, producing a 5× spike). Remaining:
- [ ] **MUSCL / vanLeer 2nd-order reconstruction** — the port is first-order
  KNP; OpenFOAM uses vanLeer limiting. This is the bulk of the 3.7 % L1 shock-tube
  error (contact/shock smearing on 100 cells).

### rhoPimpleFoam (`solvers::rho_pimple_foam`)
- [ ] **Apply the proven pimpleFoam coupling fixes** — this solver still has the
  same structure that was broken in pimpleFoam: `- fvm::laplacian_vec` (should be
  `+`), `+= phi_int` pressure source (should be `-=`, negated), unconstrained
  HbyA boundary flux, Gauss-Seidel pressure solve, a single-pass corrector that
  never re-evaluates H(U), and no ddtCorr. It will diverge as-is. Port the full
  set now proven on pimpleFoam (sign, sign, constrainHbyA, `solve_cg`, per-step
  BC re-application, the PISO corrector loop restructure, and `fvc::ddtCorr`),
  then validate.
- [ ] **k-ω SST turbulence model** (Layer 4, `openfoam-turbulence-lib`) — currently
  a stub. Required to un-ignore the aerofoil Cp / CL / mass-conservation tests
  (the case is RAS and cannot be reproduced laminar).

## Library (`openfoam-basic-lib`)
- [ ] Consider having `FvMatrix::solve` auto-select `solve_cg` for symmetric
  (`upper == lower`) systems instead of requiring callers to pick. PCG was ~170×
  faster than Gauss-Seidel on the 400-cell pressure Poisson here.

## I/O (`io::field_reader`, `io::poly_mesh`)
- [ ] BC reader maps unmodelled OpenFOAM BC types (inletOutlet, fixedFluxPressure,
  waveTransmissive, calculated, …) to `ZeroGradient` as a best-effort fallback.
  Implement the real BCs when the compressible/turbulent cases need them.
