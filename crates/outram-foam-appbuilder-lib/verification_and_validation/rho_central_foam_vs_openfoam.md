# `rho_central_foam` vs upstream rhoCentralFoam, shockTube (gh:#1363, row 2)

**Research, education and V&V only.** Not for reactor operation, licensing or
safety decisions (`RESPONSIBLE_USE.md`). AI-run; not yet reviewed by a human.

## Which upstream, and what exactly was run
The port's doc comment cites `applications/solvers/compressible/rhoCentralFoam/`,
which exists only in the **ESI** line (OpenFOAM-v2506 here). The Foundation line
(OpenFOAM-dev 852527377c7e) has no rhoCentralFoam: it became the `shockFluid`
module (`applications/modules/shockFluid`, run through foamRun), which needs the
thermophysical, momentumTransport and fluidThermo libraries, not built here and
not affordable in the disk budget. So the comparison is against **ESI v2506
`rhoCentralFoam.C`**; the Foundation `shockFluid` text was not run and not read
for this record (same Kurganov-Tadmor algorithm by lineage, not checked).

`psiThermo`, the turbulence model and their libraries (libfluidThermophysicalModels,
libturbulenceModels, libcompressibleTurbulenceModels, ... a build of the order of
hundreds of MB and an hour of two-core compile) were NOT built. Instead
`upstream_openfoam_gh1363/driver/rhoCentralDump.C` is rhoCentralFoam.C line for
line (directed interpolation with the real `interpolate()` of
`directionInterpolate.H`, Kurganov flux, `solve(fvm::ddt(rho) + fvc::div(phi))`
etc. with the `diagonal` solver of the tutorial's fvSolution, `e = rhoE/rho -
0.5 |U|^2`, `p = rho/psi`) built on the unmodified upstream libOpenFOAM and
libfiniteVolume, with the thermo classes used directly:
`constTransport<species::thermo<hConstThermo<perfectGas<specie>>, sensibleInternalEnergy>>`,
and a transcription of `hePsiThermo::calculate()` (T = THE(e,p,T), psi) on cells
and boundary faces. **This is not the unmodified upstream binary**: it is the
upstream algorithm with thermo/turbulence plumbing replaced. The shockTube
tutorial has mu = 0 so `inviscid` is true and the viscous branches (muEff, tauMC,
laplacians) are not executed in upstream either; the driver aborts if mu > 0.

Case: v2506 tutorial `compressible/rhoCentralFoam/shockTube` (100 cells,
x in [-5, 5] m, T = 348.432 K / p = 1e5 Pa for x < 0, T = 278.746 K / p = 1e4 Pa
for x >= 0, U = 0, hConst Cp = 1004.5, molWeight 28.96, perfectGas, mu = 0;
`fluxScheme Kurganov`; `reconstruct(rho) vanLeer; reconstruct(U) vanLeerV;
reconstruct(T) vanLeer`) with a FIXED dt = 1e-5 s (instead of the tutorial's
adjustTimeStep maxCo 0.2) so both codes take identical steps; steps 1, 2, 10, 100
and 700 (t = 0.007 s, the tutorial end time) are dumped as IEEE-754 bit patterns
(`scripts/run_shock.sh`; mesh = v2506 blockMesh output committed under
`shock/constant/polyMesh`).

## Method
`tests/upstream_shock_tube_gh1363.rs` reads the committed polyMesh and reference,
sets the port's initial state from the same T and p, runs `RhoCentralFoam` with the
same dt, and compares rho, Ux, T and p per cell at the dumped steps: count of
bit-identical cells and the worst relative difference (against the field's max
|value|, and the worst per-cell relative difference). The port stores `e` and no
`T`: its T is `e / Cv`.

## Predictions (written 2026-11-11, BEFORE the port was run)
Read: upstream `rhoCentralFoam.C`, `directionInterpolate.H`, `readFluxScheme.H`,
`centralCourantNo.H`, `createFields.H`; and the port's `rho_central_foam/mod.rs`.
Differences I can see by reading, each with its expected size:
1. **Hard-coded gamma = 1.4.** Upstream's gamma is Cp/Cv from the case:
   Cv = Cp - R, R = RR/W = 287.1 (dump constants), so gamma = 1.4002..., not 1.4.
   The sound speed `c^2 = gamma R T` differs by about 1.8e-4 relative.
   Predicted effect at step 1: no difference in cells away from the initial jump
   (U = 0, flux = p, which has no gamma in it); in the two cells at the jump a
   relative difference of order 1e-4 in rho/U/T/p (through `a_pos`, `ap`, `am`).
2. **Reconstructed variables differ.** Upstream reconstructs rho, rhoU (vector
   limiter vanLeerV), rPsi (the T scheme), e and `c = sqrt(Cp/Cv*rPsi)` itself;
   the port reconstructs rho, the velocity components (scalar vanLeer) and e, and
   derives p and c from the reconstructed rho and e. Reconstructing `c` is not the
   same as `sqrt(gamma (gamma-1) e_pos)` away from a constant state, and U is not
   rhoU/rho reconstructed. Predicted: at step 1 the initial state is piecewise
   constant, so every limiter is 0 at the jump (r = 0) and the two formulations
   coincide apart from item 1; from step 2 on the difference is real and of order
   the limited slope times the nonlinearity, i.e. **1e-4 to 1e-2 of the field
   range in the wave region at steps 10 to 700**.
3. **Arithmetic order everywhere** (fluxes built as `a_pos*phiv_pos - aSf` in
   upstream against the port's `(a_R F_L - a_L F_R + a_L a_R (W_R - W_L))/(a_R - a_L)`;
   the explicit update `rho - dt*div` against `ddt` + diagonal solve; the port
   recomputing rho*U from U every step where upstream keeps rhoU as a field;
   boundary faces as the physical flux of the owner cell against the KNP formula with
   equal pos/neg states). Predicted: ulp-level (1e-16 relative) differences after
   step 1 on every cell that is not at a wave, in addition to 1 and 2.
4. **Bit-identity of the initial state is not automatic**: upstream `rho = psi*p`
   with `psi = 1/(R T)`; the port's tutorial builds `p/(R T)` (an ulp). The test
   builds the initial state with upstream's order so any difference after step 1 is
   the solver's.
5. **Clamps** (`rho >= 1e-6`, `e >= 0`, `p >= 0`): not active in this problem
   (rho in 0.1 to 1.3, e > 0 in the port's convention). **T**: the port's `e = Cv T`
   differs from upstream's `Es = Cp (T - Tref) - R T`-type offset by a constant,
   which is exact in exact arithmetic (energy is additive) but not in floating point.

Expected outcome, stated plainly: **not bit-identical** (T cannot be, since the port
derives it), step 1 agreement of order 1e-4 at the two jump cells and exact in
the other 98 for rho and U; and a real, formulation-level disagreement of up to
1e-2 of range in the waves later. The fix, if the predictions hold, is to translate
upstream's formulation (items 1 and 2) into `rho_central_foam`.

## Results (measured 2026-11-11, release; `tests/upstream_shock_tube_gh1363.rs`)

**Process note, and what was NOT measured.** The predictions above were committed
before the port was run, but the first port run was already the TRANSLATED solver:
the formulation (items 1 to 3, 5) was rewritten from upstream's source before any
comparison, so the predicted sizes (1e-4 at the jump from gamma, up to 1e-2 in
the waves from reconstructing different variables) were never measured against the
old code. They remain unverified predictions; the old formulation is in git
history (`9ee808d07`). Item 4 (initial state in upstream order) was applied as predicted.

**Final: bit-identical.** For `rho`, `Ux`, `T`, `p`, `e`: **100 of 100 cells
bit-identical at every dumped step** (1 to 10, 20, 50, 100, 300, 700; dt = 1e-5 s,
t = 0.007 s at step 700), worst difference 0.0. Also bit-identical: the mesh
geometry the port reads (cell volumes `V` 100/100, cell centres `Cx` 100/100,
face `Sf_x` 99/99) and the gas constants (`R`, `Cp`, `Cv`), `Es` at two states and
the `THE` iteration (`assert_eq` on the bit patterns, inside `build_port`).
The test pins exactly this and relaxes nothing.

The port's temperature is the one it keeps as upstream does (`T = THE(e, p, T)`
with the OLD `p`), not a derived `p/(rho R)`. The test sets the initial state with
upstream's arithmetic (`psi = 1/(R T)`, `rho = psi*p`, `e = Es(p, T)`, and the file's `T` as
the first iterate through `set_initial_temperature`).

### How it was reached: every divergence, in the order found
Each was located by finding the first step, quantity and cell that differed, and
then dumping intermediate upstream quantities (`driver/rhoCentralDump.C` can print
face fluxes, reconstruction weights, limiters and gradients) against the port's.

| # | what | first seen | cause | fix |
|---|---|---|---|---|
| 1 | gamma, energy variable, formulation | by reading | hard-coded gamma = 1.4; reconstructs `U`, `e`, derives `p` and `c`; clamps; recomputes `rho*U` from `U` each step | `step` rewritten from `rhoCentralFoam.C` (`PerfectGas`, persistent `rhoU`, `rhoE`, `T`, `psi`, boundary-face state, `vanLeerV` on `rhoU`, `solve(ddt + div)` arithmetic) |
| 2 | step 1, rho/p/e/T, 26 of 100 cells, 1 ulp | measured | the appbuilder `read_poly_mesh` had its own older copy of the cell geometry: **63 of 100 cell volumes and 64 of 100 centres off upstream, up to 3.1e-15 relative**; rho = (r rho0 V)/(r V) then rounds differently | `read_poly_mesh` now calls basic-lib's `face_centre_and_area` and the new `cell_geometry_from_faces` (the upstream-order code of gh:#1489, factored out of `PolyMesh`) |
| 3 | step 2, Ux, 4 cells (62, 63, 92, 93), 1.46e-15 | measured | `fvc::grad` summed the `empty` patch faces; upstream's empty patch has no faces. The y, z gradient components differ at round-off, which enters the TVD limiter through `d . grad`; upstream's limiter weight `wNeg[62] = 0.4999999999999939`, the port's `0.50000000000000195`, so a one-ulp tie in `w*(P-N)+N` went the other way | `fvc::grad` and `fvc::grad_vec` skip `PatchKind::Empty` |
| 4 | step 9, Ux, cells 14 and 15, ~4e-14 | measured | upstream does not solve a momentum component along an `empty` normal (`solutionD = -1`); the port did, seeding round-off in `rhoU_y`, `rhoU_z`, which the vector limiter sees | `solution_directions` (upstream `calcDirections`), those components keep their old value |

Intermediate states, for the record (worst difference over field max): after fix 1
only, step 1 had 26 of 100 `rho` cells off by 1 ulp and step 700 was within 6.4e-14
(`p`); after fixes 1 and 2, step 1 was bit-identical and step 2 differed only in 4 `Ux`
cells (1.5e-15 absolute), step 700 within 5.4e-15; after fix 3, bit-identical through
step 8 and `Ux` differed in 2 cells at steps 9 and 10, step 700 within 4.6e-15; after
fix 4, bit-identical everywhere.

### Defects found, by what they cost
Fix 2 (mesh geometry) is not specific to this solver: **every solver using the
appbuilder `read_poly_mesh` had cell volumes up to 14 ulp off upstream's**.
Fix 3 changes `fvc::grad` for every 2-D case with `empty` patches (the y or z
gradient component, and through limiters the x component at round-off level).
The re-run of the suites and the moved pins are listed at the end of this file.

### The 1-ulp `pow`/trig allowance
Not used: this case calls no `pow`, `exp` or trigonometric function of the
platform libm (only `sqrt`, which is correctly rounded).

### Not covered
Foundation `shockFluid` (not built, not read); the viscous branches of
rhoCentralFoam (mu > 0: `tauMC`, the `muEff` laplacians); `Tadmor` flux scheme
(the code path exists in neither port nor tested); boundary types other than
zeroGradient/empty (the port treats every non-empty patch as zeroGradient, as the
tutorial has); mesh motion; local time stepping; `adjustTimeStep` (fixed dt used on both
sides); non-uniform T/p/U initial conditions beyond the tutorial's two states; 3-D.
The upstream side is the real upstream algorithm on the real upstream libraries
but not the unmodified binary (see the first section).

## Suites re-run (release, 2026-11-11 to 2026-11-12) and moved numbers
Changes that can move other results: `fvm::div_vec_linear` and `Gauss linear` now
fully implicit (row 1), `FvMatrix::set_reference_openfoam` in `PimpleFoam`, the
mesh reader's geometry, `fvc::grad`/`grad_vec` skipping `empty` patches, the
`RhoCentralFoam` rewrite. Results, whole suites, `--lib --tests`:

| suite | result |
|---|---|
| `outram-foam-basic-lib` | 551 lib tests + all integration tests pass, 0 failed (20 ignored as before) |
| `outram-foam-appbuilder-lib` | 415 lib + every integration test pass, 0 failed. (After the first fix the old deferred-correction unit test failed once: it asserted the old operator; rewritten, see the row 1 record.) The scheme unit tests were re-run after the `GaussLinearDeferred` addition: 7 pass; the full appbuilder suite was NOT re-run after that addition and the later doc edits |
| `outram-foam-cli` | all pass (Sod CI smoke, cavity CI smoke, solvers) |
| `tampines-steam-tables` | 1049 lib + all integration pass (it uses `RhoCentralFoam` in `hem_pipe_verification`) |
| `dhoby-ghaut` example `foam_studio_web` (headless) | FIRST RUN: **2 failed** (`a_coarse_coil_shows_dean_flow`, `headless_coil_shows_dean_flow`: "the solve diverged"); cause: the coil runs used `Gauss linear`, and the fully implicit central scheme (as upstream's) diverges on those coarse grids at Re = 200 where the old deferred-correction operator did not. Fix: ~~explicit `DivScheme::GaussLinearDeferred` (the old operator, documented as NOT an OpenFOAM scheme) at the three coil call sites; pins unmoved;~~ **CORRECTED 2026-11-12 (gh:#1500)**: that was a stopgap; the scheme is now `#[deprecated]` and the coil runs use `GaussLinear` at Co 1 (cause of the divergence: Co 4 is too high for the implicit central matrix; see `helical_coil_dean_gh1157.md`); re-run **19 passed, 0 failed, 7 ignored** |
| `outram-park-digital-twin-engine` `double_pipe` (22 tests) | 22 passed, 0 failed |

Numbers that moved (old to new; cause: `rhoCentralFoam` is now upstream's algorithm;
no tolerance was changed and no test needed one):
- Sod whole-field `L2` relative to peak: rho 1.41 % to 1.51 %, u 5.17 % to 5.39 %, p 1.06 % to 1.21 %;
  `Linf` rho 8.61 % to 8.73 %, u 48.04 % to 50.34 %, p 7.45 % to 7.77 %; faithful Table II
  stations, worst error: p 0.43 % to 0.08 %, u 0.96 % to 0.32 %, rho 0.43 % to 0.14 %
  (`sod_shock_tube_validation/RESULTS.md`, `AGENTS.md`).
- Tutorial shockTube mean `L1` against the old OpenFOAM reference run (adjustable dt, 6
  digits): rho 0.57 % to 0.71 %, U 1.41 % to 1.62 %, p 0.66 % to 0.83 %.
- Unchanged: the converged Ghia Re = 100 numbers of `tests/fv_scheme_selection.rs`
  (Gauss linear RMS 0.0057, max 0.0127: the deferred and the implicit operator reach
  the same converged solution), Foam Studio headless pins, `double_pipe` pins.

Not re-run: the other workspace crates (any that call `fvc::grad` on an `empty`-patch mesh
or use the appbuilder `read_poly_mesh`); `docs/outram-foam-appbuilder-lib-api.md` (the
generated mirror) was not regenerated.
