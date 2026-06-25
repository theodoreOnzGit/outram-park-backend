# openfoam-appbuilder-lib — TODO

Remaining work on the solver test suites and the solvers themselves, captured
after wiring up the pimpleFoam / rhoCentralFoam / rhoPimpleFoam tutorial tests.

## Test-suite status

| Tutorial | Active tests | Ignored | Blocker |
|---|---|---|---|
| `pimple_foam_cavity` | mesh load, velocity-vs-icoFoam (3.6 % L∞) | Ghia Re=100 | mesh is Re=10; needs a ν=1e-3 re-run |
| `rho_central_foam_shock_tube` | mesh load, pressure (3.7 % L1), shock position | — | none |
| `rho_pimple_foam_aerofoil_naca0012` | mesh load | Cp, CL, mass conservation | k-ω SST turbulence stub |

## Solvers

### pimpleFoam (`solvers::pimple_foam`)
Fixed this session: momentum Laplacian sign, pressure-source sign, `constrainHbyA`
boundary flux, PCG pressure solve, per-step BC re-application. Remaining:
- [ ] **`fvc::ddtCorr` flux reconstruction** — without it the simplified
  Rhie–Chow flux is stable only to Co ≈ 0.1, so the cavity runs at dt = 5e-4
  instead of icoFoam's 5e-3. Adding ddtCorr would let it run at icoFoam's dt.
- [ ] **Second-order convection** (`Gauss linear`) option — the port uses
  first-order upwind (`fvm::div`), which is the bulk of the 3.6 % cavity
  difference vs icoFoam. A linear/limited-linear scheme would tighten agreement.
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
  HbyA boundary flux, and Gauss-Seidel pressure solve. It will diverge as-is.
  Port the four fixes (sign, sign, constrainHbyA, `solve_cg`) plus per-step BC
  re-application, then validate.
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
