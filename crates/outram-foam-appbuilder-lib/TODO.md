# outram-foam-appbuilder-lib — TODO

Remaining work on the solver test suites and the solvers themselves, captured
after wiring up the pimpleFoam / rhoCentralFoam / rhoPimpleFoam tutorial tests.

## Test-suite status

| Tutorial | Active tests | Ignored | Blocker |
|---|---|---|---|
| `pimple_foam_cavity` | mesh load, velocity-vs-icoFoam (1.4 % L∞), Ghia Re=100 (coarse + fine mesh), pressure-solver comparison | — | none |
| `rho_central_foam_shock_tube` | mesh load, pressure (0.7 % L1), shock position | — | none |
| `rho_pimple_foam_aerofoil_naca0012` | mesh load | Cp, CL, mass conservation | test bodies are empty TODOs; also needs turbulence wall functions (**not** a k-ω SST stub — that claim was wrong, see below) |
| `turbulence_coupling` (tests/) | analytic k-ω decay through PIMPLE, closure-changes-momentum | — | none |
| `fv_scheme_selection` (tests/) | Gauss linear vs upwind vs Ghia 1982, backward-ddt selection | — | none |

## Solvers

### pimpleFoam (`solvers::pimple_foam`)
Fixed/added this session: momentum Laplacian sign, pressure-source sign,
`constrainHbyA` boundary flux, PCG pressure solve, per-step BC re-application,
**proper PISO corrector loop** (H(U) re-evaluated each pass — the Co≈0.85
stability fix), and **`fvc::ddtCorr`** with the `fvcDdtPhiCoeff` limiter. The
cavity now runs at icoFoam's dt = 5e-3 and matches to 1.4 %. Remaining:
- [x] **Second-order convection** (`Gauss linear`) option — **done**, in
  `solvers::schemes::div_vec_scheme`. Measured on the Re = 100 cavity, 20×20
  (`tests/fv_scheme_selection.rs`, 2026-08-07): centreline RMS error vs Ghia
  et al. (1982) falls from 0.0363 to 0.0224 (−38 %). The *limited*/TVD schemes
  (`linearUpwind`, `vanLeer`, `MUSCL`, `limitedLinear`) remain unimplemented
  and return `AppBuilderError::UnsupportedScheme` — they need a face-`r`
  reconstruction that `outram-foam-basic-lib` does not expose.
- [x] Un-ignore `cavity_ghia_benchmark_re100` — **done**; it, its fine-mesh
  variant, and `cavity_pressure_solver_comparison_fine_mesh` are all active and
  passing in `tutorials/pimple_foam_cavity.rs`.

### rhoCentralFoam (`solvers::rho_central_foam`)
Fixed/added: boundary-face flux (the end cells were missing their wall pressure
force, producing a 5× spike), and **2nd-order vanLeer MUSCL reconstruction**
(`fvc::reconstruct_pos_neg`) — shock-tube L1 error dropped 3.7 % → 0.7 %. The
KNP flux now matches OpenFOAM rhoCentralFoam's scheme family. Nothing
outstanding for the Sod tube.

#### Sod Shock Tube

- [x] **CSV output — done.** `tests/sod_shock_tube_validation/main.rs` writes
  both `sod_shock_tube_rhocentralfoam_vs_table_ii.csv` (`:688`, the 9-station
  Table II comparison) and `sod_shock_tube_profile_vs_exact_riemann.csv`
  (`:755`, the full profile with L2/L∞ norms in its header).
- [ ] **Add the BibTeX block below to `main.rs`'s module doc.** The module
  currently carries only a prose citation (`main.rs:19-21`); the verbatim
  entry is still wanted in the comments:

```
@article{Sod1978,
  author  = {Sod, Gary A.},
  title   = {A Survey of Several Finite Difference Methods for Systems of Nonlinear Hyperbolic Conservation Laws},
  journal = {Journal of Computational Physics},
  volume  = {27},
  number  = {1},
  pages   = {1--31},
  year    = {1978},
  doi     = {10.1016/0021-9991(78)90023-2},
  url     = {https://hal.science/hal-01635155/document},
  note    = {Open-access mirror: \url{https://hal.science/hal-01635155/document}}
}
```




### Convection schemes (`outram-foam-basic-lib::fvc::muscl`)
`reconstruct_pos_neg` (Upwind / Linear / VanLeer / Minmod limiters) is the
explicit, density-based path used by rhoCentralFoam. Remaining:
- [ ] **Limited *implicit* convection for `fvm::div`** (deferred correction) —
  pimpleFoam still uses first-order upwind in its momentum matrix, the bulk of
  the remaining 1.4 % cavity difference. This needs the limited face value as an
  explicit source correction on top of the upwind matrix, not the explicit
  reconstruction used here.

### rhoPimpleFoam (`solvers::rho_pimple_foam`)
- [x] **Apply the proven pimpleFoam coupling fixes** — **done.** This item used
  to claim the solver still had pimpleFoam's broken structure and "will diverge
  as-is". That is no longer true and has not been for some time: the port now
  has `+ fvm::laplacian_vec` (`mod.rs:235`), the negated `-= phi_int` pressure
  source (`:304-305`), `constrainHbyA` (`:296`), the PCG `solve_cg` pressure
  solve (`:350`), and the restructured PISO corrector loop (`:184-190`). The
  low-Mach cavity stability test in that module passes.
- [ ] **`fvc::ddtCorr` for rhoPimpleFoam** — the one genuine remainder of the
  above. The Rhie-Chow transient flux correction is not applied in this solver
  (no `ddt_corr` call), unlike `pimple_foam`.
- [x] **Wire the turbulence closures into the solvers.** Done 2026-08-07.
  `src/turbulence/mod.rs` adds the enum-dispatched `TurbulenceClosure`
  (`Laminar` | `KOmegaSST` | `KEpsilon` | `KOmega` | `SpalartAllmaras` |
  `Smagorinsky`) over `outram-foam-turbulence-lib`; `PimpleFoam` and
  `RhoPimpleFoam` each carry one, assemble the momentum stress term from
  `div_dev_reff`, and call `correct()` after the pressure correctors.
  `RhoPimpleFoam` converts kinematic ↔ dynamic (μ_eff = μ + ρν_t,
  α_eff = α + ρν_t/Pr_t) and feeds the closures the volumetric flux φ/ρ_f.
  **Laminar is the default**, so no existing result changed (verified: the
  cavity fields are bit-identical). Verification in
  `tests/turbulence_coupling.rs` — homogeneous k-ω decay through the PIMPLE loop
  matches the analytic law to 1.8e-5 (ω) and converges at first order in k
  (observed order 1.00).

  Before this, **~3 178 lines of turbulence closures had zero call sites
  anywhere in the workspace** while `Cargo.toml` already declared the
  dependency. The old note that k-ω SST was "a stub in `RhoPimpleFoam`" was
  wrong: it was fully implemented, just never connected.
- [ ] **Turbulence wall functions** (`nutkWallFunction`, `omegaWallFunction`,
  `kqRWallFunction`) — **the remaining blocker for every wall-bounded RAS case**,
  including the aerofoil. The closures use zero-gradient near-wall BCs, so ω is
  never driven to its `6ν/(β y²)` asymptote and ν_t = k/ω is unbounded near a
  wall. Measured (`tests/turbulence_coupling.rs`, 2026-08-07): a Re = 100
  lid-driven cavity with Wilcox k-ω develops ν_t/ν ≈ 260–330. Until this is
  fixed, no wall-bounded RAS result from this stack may be compared with a
  friction correlation and called validated.
  `outram-foam-turbulence-lib::wall_functions` has `y_plus`/`u_tau`/`nu_t_wall`
  as standalone helpers, but they are not wired in as patch boundary conditions;
  doing so is a Layer-4 change in that crate.
- [ ] **Rhie–Chow `ddtCorr` for non-Euler time schemes.** `DdtScheme::Backward`
  is now honoured by `PimpleFoam`, but `outram-foam-basic-lib`'s
  `fvc::ddt_corr` implements only the Euler form while `rAU` picks up BDF2's
  `1.5 V/Δt` diagonal. Measured consequence (`tests/fv_scheme_selection.rs`):
  the Euler and Backward cavity runs converge to steady states differing by
  1.0e-2 to 2.9e-2 m/s, and the gap *grows* as Δt is refined. Needs OpenFOAM's
  `backwardDdtScheme::fvcDdtPhiCorr` in `outram-foam-basic-lib`.
- [ ] **Limited/TVD `div` schemes.** `crate::solvers::schemes::div_vec_scheme`
  implements `Gauss upwind` and `Gauss linear`; `linearUpwind`, `vanLeer`,
  `MUSCL` and `limitedLinear` return `AppBuilderError::UnsupportedScheme` (never
  a silent fallback). They need a face limiter driven by a reconstructed upwind
  gradient for a vector field. `outram-foam-basic-lib::limiters::FluxLimiter`
  supplies the ψ(r) functions; the face-`r` reconstruction is what is missing.
- [ ] **`grad`, `laplacian`, `snGrad`, `interpolation` scheme selections are
  still not consulted by any solver.** Only `ddt` and `div` are live. Wiring the
  laplacian selection should target `outram-foam-basic-lib`'s new
  `fvm::laplacian_corrected` / `NonOrthoScheme` and `fvc::grad_least_squares`
  rather than a fresh implementation.

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

## Performance — pressure solve dominates (MEASURED, high priority)

The `cavity_ghia_benchmark_re100_fine_mesh` run (41×41, 6000 steps to t=12 s)
takes ~24 s; an equivalent OpenFOAM run finishes in a fraction of that. Root
cause was measured by instrumenting every pressure solve:

- 6000 steps × 2 PISO correctors = **12 000 pressure solves**
- **every solve runs a flat 272 PCG iterations** — same count on the first step
  and the last, i.e. no cheaper near steady state
- **≈ 3.26 million sparse mat-vec products** total — this is essentially the
  whole wall-clock time

Fixes, in order of bang-for-buck:

- [x] **Warm-start `solve_cg` with the current field (biggest lever).** Done.
  `conjugate_gradient` now takes an optional initial guess `x0`, and
  `FvMatrix::solve_cg_with_guess` seeds the solve from the previous field;
  `pimple_foam::step` passes `self.p`. Mirrors `Foam::PCG::scalarSolve`, which
  uses the incoming `psi` as the initial guess. Returns in 0 iterations when the
  guess already meets tolerance.
- [x] **Better pressure preconditioner than Jacobi.** Done — replaced the
  diagonal (Jacobi) preconditioner with **DIC** (a faithful port of
  `Foam::DICPreconditioner`: `calcReciprocalD` + forward/backward sweep) in
  `outram-foam-basic-lib/src/ldu_matrix/solvers/conjugate_gradient.rs`. Combined
  with the warm start: per-solve PCG iters 272 → ~40, total CG iters 3.26M →
  0.50M (**6.6×**), cavity test ~24 s → ~12 s, solution unchanged
  (Ghia RMS 0.0113).
- [x] **GAMG (algebraic multigrid) — implemented.** `outram-foam-basic-lib`
  `ldu_matrix::gamg` + `FvMatrix::solve_gamg{,_with_guess}`; selectable in
  pimpleFoam via `PressureSolver::Gamg`. Serial port of `GAMGSolver` +
  `algebraicPairGAMGAgglomeration` (pairwise agglomeration on `|upper|`,
  Galerkin `Pᵀ A P` coarse operators, recursive V-cycle with GS pre/post
  smoothing + line-search correction scale, dense-LU coarsest). V-cycle count
  is mesh-independent (unit-tested). **Finding:** on the 1681-cell cavity GAMG
  reproduces the PCG field bit-for-bit (2.6e-9) but runs slower (42.8 s vs
  12.4 s) — too small to reach multigrid's crossover, and the hierarchy is
  rebuilt every solve. **PCG stays the cavity default**
  (`cavity_pressure_solver_comparison_fine_mesh` documents this). To make GAMG
  win at scale: (a) **cache the agglomeration** across time steps (re-restrict
  coefficients only — the sparsity pattern is fixed), (b) **GAMG-preconditioned
  CG** for Krylov robustness, (c) demonstrate on a much finer mesh once the
  structured-mesh generator exists.
- [ ] **Loosen intermediate-corrector tolerance + add a relative tolerance.**
  `step` solves every corrector to absolute `tolerance: 1e-8`. OpenFOAM runs
  non-final correctors at ~1e-6 with a `relTol` and only tightens the final
  corrector. Add `rel_tol` to `SolverSettings` and use a looser tol on all but
  the last PISO corrector.
- [ ] **Steady-state early exit + `adjustTimeStep`.** `run()`
  (`pimple_foam/mod.rs`) marches a fixed 6000 steps to `endTime` with no
  convergence check and no adaptive `dt`. Add a residual-based steady-state
  stop and honour `adjustTimeStep` (CLAUDE.md already requires
  `adjust_delta_t(co_max, dt_max)` per step) so the cavity stops when converged
  instead of running all 120 transit times.
- [ ] **Reduce per-step allocation churn / parallelize (lower priority).**
  Operator-overloaded field arithmetic (e.g. `hbya - rau.clone() * fvc::grad(p)`)
  allocates fresh fields each corrector, and the cell/face loops are serial.
  Minor next to the CG iteration count, but worth revisiting once the solver
  itself is fixed (see the `Arc<RwLock<T>>` threading model in CLAUDE.md).

## Library (`outram-foam-basic-lib`)
- [ ] Consider having `FvMatrix::solve` auto-select `solve_cg` for symmetric
  (`upper == lower`) systems instead of requiring callers to pick. PCG was ~170×
  faster than Gauss-Seidel on the 400-cell pressure Poisson here.
- [ ] **Structured Cartesian mesh generator (blockMesh equivalent)** — a Rust
  function that builds an `FvMesh` / writes a `polyMesh` for a box of `nx×ny×nz`
  cells with named boundary patches. Unblocks cavity mesh refinement (above) and
  parametric mesh-convergence studies generally. See root CLAUDE.md: Layer-5
  solver loops stay in solver crates, but a primitive mesh generator belongs in
  `outram-foam-basic-lib` alongside the polyMesh reader.

## I/O (`io::field_reader`, `io::poly_mesh`)
- [ ] BC reader maps unmodelled OpenFOAM BC types (inletOutlet, fixedFluxPressure,
  waveTransmissive, calculated, …) to `ZeroGradient` as a best-effort fallback.
  Implement the real BCs when the compressible/turbulent cases need them.
