# GeN-Foam port vs upstream GeN-Foam 652b3da0cc55, differential comparison (gh:#1361)

AI-written, not human-reviewed. Research, education and V&V only
(`RESPONSIBLE_USE.md`). Round 1 covers plan steps 2 and 3 of the scoping
comment on gh:#1361. ~~Step 4 (ESI thermo/turbulence closure) and step 5 (the
full GeN-Foam binary) are not started.~~ **UPDATED 2026-11-12:** round 3 built the thermal-hydraulics
libraries and the full binary (without `thermoTools`, which no case run needs) and compares `1D_boiling`,
PSBT Ex1 and gFHR: see "Round 3" below.

Upstream: GeN-Foam `652b3da0cc55` on OpenFOAM-ESI **v2506** (the version
GeN-Foam declares, `README.md:34`), partial build with dummy Pstream, gcc 13.3,
`-O3`, double precision, 32-bit labels. Upstream sources are unmodified; the only
edit to its build files is removing the link-only `-lfvOptions` from
`src/classes/common/Make/options` (the library is not built here and no
`common` object references an fvOptions symbol; the link succeeds without it).

Rule applied: "ESI preferred"; where the port follows Foundation and GeN-Foam
needs ESI behaviour, the ESI form is added as an option and the Foundation
form stays callable (maintainer, 2026-11-12).

## Step 2: `libCommon` and `Test-radialBasisFunctions`

### Predictions (written BEFORE any port run; committed first)

Method. `scripts/build_and_run_common_dump.sh` builds upstream's `libCommon`,
runs upstream's own `Test-radialBasisFunctions`, then runs
`upstream_driver/common_dump/genfoamCommonDump.C`, a small driver of ours that
calls upstream's `solvePolyharmonicSpline` / `polyharmonicSpline`,
`scalarInterpolateTableGF` and `timeProfile::value` on generated inputs
(splitmix64, exact doubles, printed at 17 digits) and prints inputs and
results to `refs/common_dump.ref.txt.gz`. The Rust test
`tests/genfoam_common_vs_upstream.rs` reads inputs and reference from that file
and runs the port on the identical doubles.

Reading both sources before measuring gave these first-divergent-operation
hypotheses, each with a predicted sign (none has a sign, they are round-off) and
size:

| id | quantity | prediction | cause if it holds |
|---|---|---|---|
| P1 | RBF inverse matrix `invRBFmatrix` (upstream `LUscalarMatrix::inv`: LU, then unit-vector `LUBacksubstitute`) | the port has no inverse today. After adding one by the same two translated routines (`lu_decompose`, `lu_back_substitute`, which match `LUDecompose`/`LUBacksubstitute` line by line for non-singular input), **bit-identical, 100 % of entries** | none expected |
| P2 | RBF weights `w` | the port solves `A w = v` by one backsubstitution, upstream forms `inv(A)` and computes `w = inv * v` (row-wise `result[i] += mat(i,j)*x[j]`). **Not bit-identical** on a majority of the random cases; relative difference of the largest weight below 1e-8 for modes 1 and 3; larger (possibly 1e-6) for the 36-point `testSmall` set whose `v` is 9e-12 and whose coordinates are 300..1000 | operation order: direct backsubstitution vs inverse times vector |
| P3 | RBF value `f(x)` given identical weights | mode 1 and 3 (sqrt, multiply only): bit-identical. Mode 2 and 4 (`log`): PETIR `r_ln` vs glibc `log` differ by at most 1 ulp in a minority of terms; the sum may carry that | `ln` implementation |
| P4 | `scalarInterpolateTableGF::operator()` | all in-range and out-of-range values bit-identical, all 3 policies x 2 methods (same expression `(1-alpha)*y0 + alpha*y1`, same extrapolation expression) | none |
| P5 | `integral(k)` and `getNextPoint` | `getNextPoint` bit-identical. `integral`: bit-identical for `k = 1`; for `k = 2, 3, 0.5, 1.5` a few results differ by at most 1 ulp (glibc `pow` vs PETIR `r_powf`) | `pow` implementation |
| P6 | `timeProfile::value` constant and clamped (outside table) | bit-identical | none |
| P7 | `timeProfile::value` inside a table | **NOT bit-identical**: the port uses basic-lib `interpolate_xy` (`y0 + t*(y1-y0)`, `t = (x-x0)/(x1-x0)`, Foundation `interpolateXY` lineage); ESI `Function1Types::Table` uses `linearInterpolationWeights` (`w0 = (t1-t)/dt`, `w1 = 1-w0`, `value = w0*y0 + w1*y1`). Expect 10 to 40 % of interior points to differ, by at most 4 ulp; knots and midpoints of equal-sized brackets mostly agree | the weight formulation (ESI vs Foundation lineage) |

Remedy decided before the run (applies only if the prediction holds): P2 is
a port defect, fix by translating upstream (`inverse` then `inv * v`); P7 is an
ESI-vs-Foundation difference, so `TimeProfile::value` takes the ESI form (GeN-Foam
requires ESI) and the Foundation form stays callable as
`TimeProfile::value_foundation`.

(Step 2 results: see "Step 2 results" below.)

## Step 3: libNeutronicsSolvers and the guided slab cases

### What was built, and what ran

`scripts/build_neutronics_stack.sh` builds `libMultiRegion`, `libNeutronicsSolvers` and the **unmodified
`GeN-Foam.C` + `include/setDeltaT.C`** into an executable that links only the neutronics path. This is
the real GeN-Foam main, not a transcription of it (the scoping comment had expected a transcription).
Only link lines of three `Make/options` were pruned, removing libraries that are not built here and that
the neutronics path never calls (fvOptions, regionModels, solidSpecie, ODE, genericPatchFields, the
thermo and turbulence libraries, FFS*, ThermoMechanics, offbeat, OFSolvers); the link succeeded with no
unresolved symbol, which is the evidence that the scoping comment's reading (headers only, no thermo
symbols in `neutronics/`) was right. Headers for the thermo/turbulence/ODE/region libraries were made
reachable by `wmakeLnInclude -u` (symlinks only).

The cases ran with the real binary, serial, on the tutorials' own polyMesh/nuclearData:
`guidedCases/1_reactorSlab_1D_1Gr_neutronicDiffusion` (151 cells) and
`guidedCases/2_reactorSlabReflected_1D_1Gr_neutronicDiffusion` (751 cells, core + 2 reflectors).
Protocol (`scripts/tight_protocol.sh`, applied to the copy): `writePrecision 17`, every linear solver
`tolerance 1e-14, relTol 0`, `endTime 60`. These change precision only, never mesh, cross sections or
boundary conditions. The tutorials as shipped stop at their solver tolerances (1e-6, relTol 1e-3), see S0.

### Predictions (written BEFORE the port run; committed first)

The two tutorials are eigenvalue cases. GeN-Foam does not run a power iteration to convergence in one
call: with `eigenvalueNeutronics true` each *time step* does one flux solve per group, renormalises to
`pTarget`, and updates `keff_ *= power/pTarget` (`calcKeff.H`), so the converged `keff_` is the
eigenvalue of the discrete operator and the "iteration history" is the keff per pseudo-time step. The
port has no such time-stepping eigenvalue driver (`solve_eigenvalue` is a static power iteration), so a
step-by-step history comparison is not possible without porting that driver; this round records it as
**not compared** and compares what is comparable.

| id | quantity | prediction |
|---|---|---|
| S0 | upstream's own keff as the tutorials ship them (solver tolerance 1e-6 / relTol 1e-3) vs tight (1e-14 / 0), case 1 | shipped run `keff_ = 1.0000037`, tight run `1.0000076499876376`: the shipped value is not converged, differing by about -3.9e-6 (-0.39 pcm). |
| S1 | k_eff, port (k tolerance 1e-13, linear 1e-14) vs upstream tight, both cases | both codes solve the same discrete operator (uniform orthogonal mesh, `Gauss linear` face D, zero-flux Dirichlet at top and bottom, zeroGradient walls): `abs(k_port - k_up) < 1e-12`. Case 1: upstream 1.0000076499876376; case 2: 1.0000305055413801 (last digits jitter by 3e-16). |
| S2 | independent third reference | a tridiagonal power iteration written in the test (Thomas algorithm, per-cell D, linear face interpolation, half-cell Dirichlet distance) agrees with both to 1e-12. If it disagrees with BOTH, my reading of the operator is wrong, not the codes. |
| S3 | flux per cell, shapes normalised to unit sum | max over cells of the relative difference port vs upstream < 1e-9. |
| S4 | iteration history | not compared (no port driver); the port's outer iteration count at the tight tolerance is reported next to upstream's 230 time steps only as context. |

S1-S4 were committed before the diffusion port run. The next two were written after the diffusion result
was in and BEFORE the SP3 port run (the diffusion numbers cannot inform them: SP3 adds a second moment
and its boundary treatment).

| id | quantity | prediction |
|---|---|---|
| S5 | SP3 on the same two slabs (the tutorials' `0.orig` already holds `defaultFlux2`; only `regionSolvers { neutroRegion SP3Neutronics; }` was changed), real binary at the tight protocol: k_eff and flux per cell, port `Sp3Neutronics::solve_eigenvalue` with the same boundary conditions on both moments (top/bottom fixedValue 0, walls zeroGradient). Upstream: case 1 `keff_ = 1.0078490573983063`, case 2 `1.0110843916601269` (last digits jitter by 9e-16) | the port was written from `SP3Neutronics`, same discrete system: `abs(dk) < 1e-9`, flux shape max relative difference `< 1e-7`. If it misses, the first suspect is the second-moment boundary treatment (upstream reads `defaultFlux2` per patch; the port applies one `BoundaryCondition` to both moments). ~~SP3 (1.0078) is above diffusion (1.0000) because SP3 transports the leakage with a smaller effective diffusion at these dimensions: a physics sanity check, not a comparison.~~ **WITHDRAWN 2026-11-12**, it was a guess, see Results. |
| S6 | `reactorCases/Godiva_SN` (23 groups, `legendreMoments 3`, `quadratureSet` of 4 directions per octant, unstructured sphere mesh) | **not comparable on identical inputs this round**, two blockers read from the port source before any run: (a) the port's `AngularQuadrature` is baked-in level-symmetric (`quadrature.rs:18`: "this port bakes the standard sets") and has no reader for the case's `quadratureSet`; (b) anisotropic scattering moments P1 to P3 are not implemented (gh:#1474). Running the port on its own quadrature and P0 would be a different problem, not a comparison, so it is not run. The upstream binary was run for the record (below). |

## Results (measured 2026-11-12; reference runs from the scripts here; tests are `tests/genfoam_common_vs_upstream.rs` and `tests/genfoam_neutronics_vs_upstream.rs`)

### Step 2: libCommon, quantity by quantity

Upstream's own `Test-radialBasisFunctions` (built and run): the three tests (1-D, 2-D, 2-D small values)
pass. Our driver then ran upstream's functions on 34 RBF cases (modes 1 to 4; 1-D, 2-D, 36-point 2-D
small-value set, random sets of 1 to 5 parameters and 6 to 20 points, a physical-scale set), 18 tables
and 13 time profiles.

| id | quantity | n | predicted | measured BEFORE the fix | cause | AFTER the fix |
|---|---|---:|---|---|---|---|
| P1 | `invRBFmatrix` entries | 8193 | bit-identical | **0 differ** (held) | `lu_decompose` + `lu_back_substitute` match `LUDecompose`/`LUBacksubstitute` for non-singular input | same |
| P2 | RBF weights | 562 | not bit-identical, `max abs dw / max abs w < 1e-8` | **528 differ**, 29 of 34 vectors not identical, `max abs dw / max abs w = 6.4e-14` (held in direction; the 1e-6 worry for the small-value set was too pessimistic) | port solved `A w = v` by one back-substitution; upstream forms `inv(A)` then `w = inv * v` (`result[i] += mat(i,j)*x[j]`). First divergent operation: the product `inv * v` against a direct back-substitution | **0 differ** |
| P2' | RBF value with the port's own weights | 280 | follows P2 | 240 differ, up to 26039 ulp = 4.8e-12 relative (a value cancelling to near zero) | the weights | **0 differ** |
| P3 | RBF value given UPSTREAM's weights (evaluator alone) | 280 (140 in modes 2 and 4, which use `ln`) | modes 1 and 3 identical, modes 2 and 4 within 1 ulp of glibc | **0 differ in all 280** (the `ln` of PETIR equals glibc's on every term here) | none | same |
| P4 | `InterpolateTable` values, 3 policies x 2 methods (in range and out of range) | 618 | bit-identical | **0 differ** (held) | | same |
| P5 | `integral(k)`, k = 1, 2, 3, 0.5, 1.5, and `getNextPoint` | 90 and 504 | `getNextPoint` identical; integral identical at k = 1, possibly 1 ulp at the other k (`pow`) | **0 differ in both** (PETIR `r_powf` equals glibc `pow` on all 72) | none | same |
| P6 | `timeProfile` constant, and a table outside its range (clamp) | 4 and 41 | bit-identical | **0 differ** (held) | | same |
| P7 | `timeProfile` table inside its range | 667 | 10 to 40 % differ, at most 4 ulp | **242 differ (36 %, held)**, **up to 24 ulp = 4.7e-15 relative (the size prediction MISSED: the ordinates change sign, so a small sum cancels)** | port used the Foundation `interpolateXY` `y0 + t*(y1-y0)`; GeN-Foam runs ESI `Function1Types::Table`: `w0 = (t1-x)/(t1-t0)`, `w1 = 1-w0`, `w0*y0 + w1*y1` | **0 differ** |

Difference counts are bit comparisons of IEEE doubles, not tolerances: no threshold was set or moved.
The two superseded forms stay callable and are pinned at the numbers above
(`direct_solve_is_the_old_form_and_differs_from_upstream`, `foundation_form_of_a_table_differs_from_upstream`).

**Defects fixed (both by translating upstream).**
1. `solve_polyharmonic_spline` now forms the inverse and multiplies, as `solvePolyharmonicSpline` does
   (new `polyharmonic_spline_inverse`, new basic-lib `SquareMatrix::inverse` and `multiply_vector`). The old
   one-back-substitution form is kept as `solve_polyharmonic_spline_direct`. This is not an ESI/Foundation
   question (the RBF code is GeN-Foam's own); no option name beyond `_direct` was needed.
2. `TimeProfile::value` uses the ESI table form. This IS an ESI-versus-Foundation difference, so per the
   maintainer's rule the ESI form is the default (GeN-Foam requires it) and the Foundation form is kept
   callable as `TimeProfile::value_foundation`; basic-lib's `interpolate_xy` is untouched.

**Noted, not changed.** `SquareMatrix::lu_decompose` differs from ESI `LUDecompose` for an exactly singular
matrix only (it writes `f64::EPSILON` where ESI writes `SMALL` and aborts on an all-zero row); the port
returns `Err(Singular)` by design and no regular path reaches it.

### Step 3: the guided slabs (real GeN-Foam binary, tight protocol)

| case | model | upstream k_eff | port - upstream | tridiagonal reference - upstream | port - tridiagonal | flux per cell, max relative difference of the unit-sum shape | port outer iterations |
|---|---|---|---:|---:|---:|---:|---:|
| guided 1 (151 cells) | diffusion | 1.0000076499876376 | -5.8e-14 | +6.6e-14 | -1.2e-13 | 5.7e-13 | 47 |
| guided 2 (751 cells, reflected) | diffusion | 1.0000305055413801 | -4.6e-14 | +3.9e-14 | -8.5e-14 | 3.4e-13 | 43 |
| guided 1 | SP3 | 1.0078490573983063 | -1.7e-13 | n/a | n/a | 2.3e-12 | 53 |
| guided 2 | SP3 | 1.0110843916601269 | -1.3e-13 | n/a | n/a | 1.4e-12 | 46 |

Predictions S1, S2, S3, S5 all held, at the predicted bounds (1e-12 on k, 1e-9 and 1e-7 on the shapes), with a
margin of one to four orders of magnitude. The port needed no change for these: the diffusion and SP3 operators,
the Dirichlet/zeroGradient treatment, the face-D interpolation and the eigenvalue update agree with upstream,
and the independent tridiagonal solve agrees with both, so the agreement is not shared error between the two
codes' reading of the equation. Differences are linear-solver residual level (1e-13 absolute tolerance),
not bit identity: the two codes use different linear solvers and iteration structures.

S0: the tutorials as shipped stop at their solver tolerances (1e-6, relTol 1e-3) after 106 pseudo-time steps
and report `keff_ = 1.0000037` (case 1) and `1.0000227` (case 2), against the converged `1.0000076499876376`
and `1.0000305055413801`: the shipped numbers are unconverged by -3.9e-6 and -7.8e-6 (-0.39 and -0.78 pcm).
Any comparison against a shipped log is limited to that. ~~S4 (iteration history): not compared.~~ **CORRECTED 2026-11-12:** compared step by step in round 2 ("S4 results").
The upstream history (230 pseudo-time steps at the tight protocol) is committed in the reference; the port
has no driver that updates keff once per step. A port of that driver (`solveNeutronics.H` eigenvalue branch,
`calcKeff.H`, `setDeltaT`/`maxDeltaT`) is the remaining work for S4.

~~S6, Godiva_SN: not compared (blockers above).~~ **UPDATED 2026-11-12:** both blockers are removed in round 2 (reader and moments ported); see "SN results". The real binary runs it as shipped (P3 scattering, its own
`quadratureSet`): the first pseudo-time step took about 9 minutes on one core at 3.2 GB resident and moved
`keff_` from the seed 0.980106 to 0.938566; the shipped case runs 20 steps (about 3 hours), so the run was
stopped after step 1 and is NOT a reference. Recorded only as the cost of the run. See "What remains".

(The sentence in S5's row that SP3 sits above diffusion "because SP3 transports the leakage with a smaller
effective diffusion" was a guess and is withdrawn: nothing here tested it. The sign and size, +0.78 % and +1.11 %
in k, are as measured.)

### Build and run measurements (to rescale the scoping estimates)

All with `WM_NCOMPPROCS=2`, `wmake -j2`, `-O3`, objects deleted after each link. Wall time and peak resident set
from `/usr/bin/time -v`; sizes from `du`/`ls`.

| item | wall | peak RSS | objects (transient) | output | scoping estimate (4 cores) |
|---|---:|---:|---:|---:|---|
| libCommon (8 sources, 132 KB) | 32 s | 0.72 GB | 2.4 MB | `.so` 1.23 MB | 2 to 6 min, 3 to 9 MB, 13 to 63 MB objects |
| Test-radialBasisFunctions | 5 s | | small | 27 KB | |
| genfoamCommonDump (this driver) | not timed (the log has no `time -v`) | | small | 86 KB | |
| libMultiRegion (92 KB source) | 21 s | 0.75 GB | 1.8 MB | `.so` 0.96 MB | not separately estimated |
| libNeutronicsSolvers (508 KB source) | 104 s | 0.71 GB | 9.2 MB | `.so` 2.28 MB | with the above |
| GeN-Foam.C + setDeltaT.C (unmodified) | 7 s | 0.50 GB | 0.3 MB | executable 106 KB | |
| lnInclude symlinks for 14 ESI source directories | seconds | | links only | | |

The N0 tier (common + multiRegion + neutronics + main) took **about 2.8 minutes in all and 4.5 MB of
libraries, 13.4 MB of transient objects**, against the estimate of 2 to 6 minutes for common + neutronics. The
estimate was in range on time and 2 to 5 times too high on size; its +-3x band holds. The estimates for tiers A
and B (thermo/turbulence/fvOptions and the TH libraries, 17 to 50 MB and 14 to 54 MB) are not tested by this
round and should be rescaled by the same ratios with caution (these libraries are small; the estimate's
finiteVolume-like template-heavy part is the risk). Disk on `/` stayed above 4.3 GB throughout; the largest
single consumer was the cargo `target/` directory (11 GB, shared), not the upstream build.

The run of the guided slabs takes under 1 s each (230 steps); Godiva_SN is the expensive one.

### What the 2026-11-11 plan items (rows 3, 4, 7 of the shared-cases table) now say

Row 3 (guided slabs, cell by cell): done, above. Row 7 (SP3): done on the slab geometry, not on the ESFR (its
`neutroRegion` needs the TH state; not attempted). Row 4 (Godiva_SN): blocked, see S6.

### Moved numbers (old to new, with cause)

The RBF weights moved by up to 6.4e-14 relative (the `inv * v` change) and every `TimeProfile` table value by
up to 4.7e-15 relative (the ESI weight form). Re-run after the change (`genfoam_tutorial_keff`, with
`GENFOAM_UPSTREAM` set, so it did not skip): ESFR at upstream's converged state 0.937442 (+60.7 pcm), MSFR
per-cell 0.962032, MSFR as shipped +0.3 pcm, MSFR albedo 0.973858 (0.0 pcm): **unchanged at the six printed
digits**, so no recorded V&V number moved visibly and no test pin was touched. The whole crate suite
(`cargo test --release -p outram-foam-appbuilder-lib --lib --tests`, 29 binaries, quick tier and long tier,
both: the `long-tests` feature is on by default) gave 493 passed, 0 failed, 12 ignored on 2026-11-12.

## What remains (after round 1)

1. ~~**S4, the iteration history.**~~ **DONE 2026-11-12 (round 2, "S4 results" below):** ~~Port the eigenvalue branch of `solveNeutronics.H` + `calcKeff.H` + `normFluxes.H`
   and `setDeltaT`/`maxDeltaT` as a pseudo-time driver, with basic-lib's PCG/DIC at the case's own solver
   settings, then compare keff, power and the first residual per step against the 230-step history already
   committed in `refs/guided_slabs.ref.txt.gz` (and `sp3_slabs`). Without it the "iteration history" is
   compared only as the converged end state.~~
2. ~~**Godiva_SN (S6).**~~ **PARTLY DONE 2026-11-12 (round 2, "SN results" below):** ~~Needs a reader for `constant/neutroRegion/quadratureSet` (the port bakes level-symmetric
   sets) and P1 to P3 scattering moments (gh:#1474). The upstream binary runs the case as-is; the first
   pseudo-time step takes several minutes at 3.2 GB resident, 20 steps are shipped. A `legendreMoments 0`
   run with the case's quadrature is the first identical-input comparison once the reader exists.~~
3. **SP3 on the ESFR** and the transient (`step`) paths, which need the TH state of the ESFR run.
4. **Plan steps 4 and 5** (ESI thermo/turbulence closure, the full binary): not started, as instructed.
5. **Singular-matrix behaviour** of `lu_decompose` against ESI (noted above) if a caller ever relies on it.

# Round 2 (gh:#1361, 2026-11-12): S4 iteration history, then SN (gh:#1474)

AI-written, not human-reviewed. Each block of predictions below was committed BEFORE the port run it concerns.

## S4: the pseudo-time eigenvalue driver

Read first (upstream is the specification): `GeN-Foam.C` (loop: `runTime++`, `correctPhysics()`, `adjustDeltaT`),
`setDeltaT.C`, `solver.C:52` (`deltaTFactor = 1.2`), `diffusionNeutronics::maxDeltaT`,
`diffusion/include/{solveNeutronics,createFluxMatrices,fluxEq,normFluxes}.H`,
`SP3/include/{solveNeutronicsSP3,fluxEqSP3,normFluxesSP3}.H`,
`include/{calcKeff,precEq,initializeNeutroSource,initializeDelayedNeutroSource,calcScatteringSource}.H` and OpenFOAM-ESI
`Time::{run,operator++,adjustDeltaT,setDeltaT}`. Findings that shape the port, none of which were in the round-1 plan:

* With `eigenvalueNeutronics true` the `ddt` coefficient `(1-eigenvalueNeutronics)` is 0, so `deltaT` never enters a flux solve.
  The time loop matters only through the NUMBER of steps (`adjustDeltaT`: growth 1.2 per step, `maxDeltaT 1`, `adjustableRunTime`
  write alignment). The port reproduces that clock too, so the step count and the `Time` column are comparable (to the 8 digits
  of `timePrecision 8`).
* `fluxEq.H` calls `neutroEq.solve()` TWICE per equation (the second inside `residual_ = max(...)`); only the first call's
  initial residual feeds `neutroResidual`. The port does both.
* Precursors in the eigenvalue branch are a diagonal system solved as `(V S/k beta)/(V lambda)`; the port copies that order.
* SP3 starts from `defaultFlux2 uniform 1`, so the first `flux_ = fluxStar - 2 fluxStar2` is negative, `pTot < 0`, and `keff_` is
  clamped to 0.1 (this is why the committed SP3 history begins 0.1, 0.1). The port needs the same start: new
  `Sp3Neutronics::set_uniform_second_moment(1.0)`.
* This is not the algorithm of `solve_eigenvalue` (a static power iteration to a tolerance). That is kept and untouched; the driver is
  new code (`neutronics/pseudo_time.rs`, `diffusion/pseudo_time.rs`, `sp3/pseudo_time.rs`). No kovan-reviewed function is edited
  (`review.md` holds no `kind = "review"` block in `neutronics/`, `diffusion/`, `sp3/`, `sn/`).
* The linear solver is basic-lib's PCG/DIC stopping on `||r||/||b||` at 1e-14, not upstream's (PCG/DIC on the OpenFOAM normalised
  residual at 1e-14, relTol 0). Bit identity is therefore not reachable and is not predicted.

| id | quantity (all steps of guided 1 and 2: diffusion 230 and 230, SP3 355 and 357) | prediction |
|---|---|---|
| T1 | number of steps and final `Time` | equal to the reference (230, 230, 355, 357), final time 60 |
| T2 | `Time` of every step | relative difference below 5e-8 (the reference prints 8 significant digits) |
| T3 | `keff_` per step, port minus upstream | `abs(dk) < 1e-12` for every step of every case, the median about 1e-14. Cause of the residual: the two linear solvers' stopping points, not the algorithm |
| T4 | `power:` per step | relative difference below 1e-12 |
| T5 | first-solve initial residual per step | same normalisation (`lduMatrix::solver::normFactor`): for upstream residuals at or above 1e-6 the relative difference is below 1e-6; below 1e-8 the residual is solver noise on both sides and only the absolute bound `< 1e-9` is claimed |
| T6 | SP3 steps 1 and 2 | `keff_` equals 0.1 exactly in both codes (the clamp) |
| T7 | final flux shape (unit sum), port vs upstream | max relative difference below 1e-10 |

### S4 results (measured 2026-11-12, `tests/genfoam_pseudo_time_vs_upstream.rs`)

Real binary vs `run_pseudo_time_eigenvalue`, same committed mesh and `nuclearData`, port linear tolerance 1e-14.

| case | steps (port / upstream) | max `abs(dk)` (step) | median `abs(dk)` | max rel `dP` | residual: max rel (upstream >= 1e-6) / max abs (< 1e-6) | `Time` max rel | flux shape (unit sum) max rel |
|---|---|---|---|---|---|---|---|
| diffusion, guided 1 | 230 / 230 | 3.3e-14 (146) | 2.5e-14 | 1.4e-14 | 2.3e-10 / 4.6e-15 | 4.2e-8 | 2.6e-13 |
| diffusion, guided 2 | 230 / 230 | 6.2e-14 (193) | 4.8e-14 | 2.0e-14 | 1.3e-10 / 4.9e-15 | 4.2e-8 | 4.3e-13 |
| SP3, guided 1 | 355 / 355 | 4.5e-14 (3) | 3.8e-14 | n/a | 2.8e-9 / 8.7e-14 | 4.9e-8 | 3.7e-13 |
| SP3, guided 2 | 357 / 357 | 1.3e-13 (2) | 1.0e-14 | n/a | 6.1e-9 / 9.3e-14 | 5.0e-8 | 1.5e-13 |

T1, T3, T4, T5, T7 held (the numbers are 1 to 2 orders inside the predicted bounds, as predicted: the residual of the two
linear solvers' stopping points, the algorithm is the same). `Time` (T2) is bounded by the 8-digit print of the reference
(a leading digit of 1 allows 5e-8); the true agreement is better: the first `deltaT` of the SP3 run (1.5000000225e-19, from
`maxPowerVariation` over the first step's power change) is reproduced to the printed digits once the start state is right.

**Misses and defects found by this comparison (all in the new code written for it, none in code that existed; each
fixed by reading upstream).** None changes a number that was recorded before.
1. T6 over-generalised: SP3 step 2 is clamped to 0.1 only in guided 1 (guided 2: 1.4332785577569906). The test now asserts the clamp at
   step 1 for both and at step 2 for guided 1 only. SP3 `keff_` step 1 is 0.1 in both codes, 0.1 and 0.1 at steps 1, 2 for guided 1.
2. First SP3 run: the port's `keff_` left upstream at step 2 (0.381 vs 0.1) with the first step's power 96657 against
   upstream's 36041. First divergent operation: the coefficient fields of the second-moment equation were built on a template
   copy of the FLUX field, so the vacuum patches carried `fixedValue 0` and `fvc::interpolate` gave the second-moment diffusivity
   0 on top and bottom faces. Upstream's coefficient is a field of cross sections, whose patch value is the cell value. After the
   fix the first step's power is 36041.29 (matches the log) and every step agrees. (`field_like` now builds zero-gradient patches.)
3. SP3 time axis: 230 port steps vs 355 upstream. Reading `SP3Neutronics::maxDeltaT` showed it has NO `time > deltaT` guard
   (the diffusion one has); the port had copied the diffusion form. Now `sp3_max_delta_t`.
4. First `deltaT` off by 3e-8 relative (1.49999997750e-19 vs 1.5000000225e-19): upstream's `flux_` before the first step is
   `defaultFlux` (= 1), not `fluxStar - 2 fluxStar2` (= -1), and `pTotOld_` of step 1 is `power()` of it. `set_uniform_second_moment` leaves the
   physical flux untouched.
5. The SP3 reference has no POWER column: the SP3 log prints `pow:`, not `power:` (extraction script unchanged, so T4 is
   diffusion only; the test asserts the column is empty rather than silently skipping it).

The static `solve_eigenvalue` paths are untouched; all four histories pass through `PseudoTimeEigenvalue` only.

## SN (gh:#1474): quadratureSet reader, anisotropic scattering, and a small S_N case

Read first: `SNNeutronics.C` / `SN/include/*.H` (see the list in the module header of `sn/pseudo_time.rs`).
What was found before writing anything, none of it in the round-1 plan:

* `xs_.legendreMoments()` is `1 + legendreMoments` (`XS.C:75`). Godiva's `legendreMoments 3` therefore uses P0 to P3 (the file
  stores P0 to P5; P4 and P5 are ignored). The port's `read_nuclear_data` already counts all stored matrices.
* Upstream's SN uses weights summing to 1 and no `1/(4 pi)` in the source (the port's `SnNeutronics` uses `4 pi` and `1/(4 pi)`); the two give the
  same scalar flux, but the new code follows upstream's convention so every intermediate quantity is comparable.
* The within-group moment source is `sum_{d'} psi_{d'} w_{d'} sum_{L>=1} P_L(Omega_d' . Omega_d)(2L+1) Sigma_{sL}`
  with `P_L(Omega_d . Omega_d')` the Legendre polynomial of the direction cosine (not the spherical-harmonics addition theorem).
  It is evaluated with the CURRENT `psi` of earlier directions (Gauss-Seidel over directions), and re-evaluated for every direction.
* Godiva uses `inletOutlet` (`inletValue 0`) on every patch, the port's `fvm::div` already has it (a V&V test in basic-lib).
* `SNNeutronics::maxDeltaT` has no `time > deltaT` guard (as SP3); Godiva has `adjustTimeStep false`.
* The new code is a separate type `SnPseudoTimeSolver` (+ `QuadratureSet`); `SnNeutronics` (baked-in sets, P0, static power
  iteration) is left exactly as it is, which is the named option that keeps the existing behaviour.

**The small case.** Godiva_SN is 9 minutes and 3.2 GB per step upstream (round 1), so the identical-input comparison uses a case built
from the SAME dictionaries on a small mesh: guided tutorial 1's slab (151 cells, one group, 6 precursor groups, its `polyMesh`
and removal/fission data, committed under `cases/guided1`) turned into an S_N case by `scripts/sn_slab_protocol.sh`:
Godiva's own `quadratureSet` (4 directions per octant, 32 in all), `legendreMoments 3` with P1 = 0.03, P2 = 0.012, P3 = 0.005 added to
P0 = 0.1 (m^-1; **synthetic**: the slab has no published anisotropic data, chosen only so the moment sources are not negligible next to
removal 5.0082; they are inputs of BOTH codes and no result depends on being physical), and `inletOutlet` on the vacuum faces as Godiva
has. Linear solvers at 1e-14 / relTol 0, `adjustTimeStep false`, `deltaT 1`, 6 steps (upstream log for 12 steps: keff_ goes
1.1345, 0.9304, 1.3823, 0.6475, 2.8265, 0.1707, 5, 0.1, 5, 0.1: the single-sweep pseudo-time iteration is unstable on this case, it
reaches the `[0.1, 5]` clamp at step 7. That is upstream's behaviour, recorded, not a defect of the comparison; the comparison
stops at step 6, before the clamp).

### Predictions (written BEFORE the port run; committed first)

| id | quantity | prediction |
|---|---|---|
| N1 | step 1: `keff_` (upstream 1.1345062686135805), `pow:`, first residual (upstream 1) | `abs(dk) < 1e-11`; `pow` relative `< 1e-11`; residual equal to 1 within 1e-9 |
| N2 | steps 2 to 4 `keff_` | `abs(dk) < 1e-9`; the iteration amplifies a solver-level difference at each step (k swings 0.93, 1.38, 0.65) |
| N3 | steps 5 and 6 `keff_` | `abs(dk) < 1e-6` (two more amplifications, k swings to 2.83 and 0.17) |
| N4 | residuals (upstream 0.221, 0.247, 0.432, 0.611, 0.659 for steps 2 to 6) | relative difference `< 1e-5` for steps 2 to 4, `< 1e-3` for steps 5 and 6 |
| N5 | scalar flux at step 6 (unit-sum shape), port vs upstream | max relative difference `< 1e-5` |
| N6 | ablation: port with P0 only (`legendre_moments = 1`) on the same quadrature | step-1 `keff_` differs from upstream by MORE than 1e-4 relative: the anisotropic source carries visible weight (if it does not, the case does not test P1 to P3 and the test must say so) |
| N7 | first divergent operation if a bound is missed | the BiCGStab stopping rule (port: `||r||/||b|| < 1e-13` ILU(0); upstream: normalised residual 1e-14 DILU), which only sets WHERE each linear solve stops, not what it solves |

### Godiva_SN, one step (predictions written BEFORE the port run; committed first)

Upstream run (started before the port ran; log not yet read for the numbers): `scripts/run_tutorial.sh reactorCases/Godiva_SN godiva1`
with `scripts/godiva_one_step.sh` (`endTime 1`, only `angularFlux.*` tightened to `1e-10 / 0`; shipped `1e-7 / 0.1` makes the answer depend on
the solver). Shipped case otherwise: 23 groups, `legendreMoments 3`, `maxInnerIter 3`, `innToOutTolerance 0.01`, `keff 0.980106`, `pTarget 1e6`.
Port: `tests/genfoam_godiva_sn_step.rs` (ignored; needs the tutorial directory).

| id | quantity | prediction |
|---|---|---|
| G1 | step-1 `keff_` and `pow:` | `abs(dk) < 1e-8`, `pow` relative `< 1e-8`; not 1e-14 as on the slab, because 120k-cell tetrahedral BiCGStab solves stop at the 1e-10 residual of two different residual definitions, and 23 groups x 32 directions x up to 3 inner sweeps accumulate that |
| G2 | the number of inner sweeps per group | identical in both codes; not measured separately (the port does not report sweeps, upstream's log has them), inferred from G1: a flipped stopping test would show as a 1e-3 or larger difference in `keff_` |
| G3 | the step-1 `keff_` moves from the seed 0.980106 to about the shipped-tolerance value 0.938566 recorded in round 1 | within 1e-3 of it (that run used 1e-7 / 0.1 solves) |
| G4 | wall time and memory of the port | not predicted (no basis); recorded |

### SN results, small S_N slab (measured 2026-11-12, `tests/genfoam_sn_vs_upstream.rs`)

Real binary vs `SnPseudoTimeSolver` on the same committed polyMesh, `nuclearData`, Godiva `quadratureSet`; port linear tolerance 1e-13.

| step | upstream `keff_` | port - upstream | `pow:` relative | first residual: upstream | relative |
|---:|---|---:|---:|---:|---:|
| 1 | 1.1345062686135805 | +4.4e-16 | 2.1e-16 | 1 | 0 |
| 2 | 0.9304310933743098 | -8.9e-16 | 1.3e-15 | 0.22147 | 3.1e-15 |
| 3 | 1.3822631749216145 | +2.4e-15 | 2.7e-15 | 0.24695 | 6.7e-15 |
| 4 | 0.6475175870102523 | -2.3e-15 | 5.3e-15 | 0.43220 | 3.1e-15 |
| 5 | 2.8264602826244443 | +2.3e-14 | 1.2e-14 | 0.61095 | 3.8e-15 |
| 6 | 0.1706915252753794 | -2.4e-15 | 2.2e-14 | 0.65875 | 2.4e-15 |

Scalar flux at step 6 (unit-sum shape): max relative difference 1.1e-15. Ablation (N6): the same port with P0 only (`legendre_moments = 1`)
gives step-1 `keff_` 1.1348527420932226 against upstream's 1.1345062686135805 (+3.05e-4 relative), so P1 to P3 carry a visible part of
the step and the agreement above is not a P0 agreement.

N1 to N5 and N6 held, but N1 to N5 were set 4 orders too loose: the prediction that the BiCGStab stopping rules (N7) would set the
size of the difference was wrong. The case is small enough (151 cells) that both codes solve every direction to round-off, so the
difference is operation-order round-off (1e-15), and the chaotic amplification of the step iteration (k swings from 0.17 to 2.8) turns that
into 2e-14 by step 5, not into solver-level 1e-9. The bounds were not tightened afterwards: they stay at the predicted values in the test.

What this does and does not verify: the `quadratureSet` reader (the 32 versors and weights enter every direction's streaming and the
Legendre cosines), the Legendre matrices and `legendrePoly`, the anisotropic intra- and extra-group sources, `facePhis_`, the `inletOutlet`
upwind operator, the first-step normalisation, the precursor diagonal solve, `calcKeff.H` and `normFluxesSN.H`, on one group. It does NOT
verify the multi-group paths (`extraSources` from other groups with moments, `sigmaFromToYesNo`), which the one-group slab skips:
Godiva (23 groups) is the only case that exercises them, see below. The synthetic P1 to P3 values mean nothing physically.
Upstream's own behaviour on this case is unstable (single-sweep pseudo-time iteration reaches the keff clamp at step 7); the
comparison stops at step 6, before the clamp.

### SN results, Godiva_SN one step (measured 2026-11-12, `tests/genfoam_godiva_sn_step.rs`, run with `--ignored`)

Shipped case (177,655 tetrahedra, 23 groups, 32 directions, P0 to P3 via `legendreMoments 3`, `inletOutlet`, `keff 0.980106`,
`pTarget 1e6`), `angularFlux.*` solver at `1e-10 / relTol 0` in both codes, one pseudo-time step.

| quantity | upstream (real binary) | port | difference |
|---|---|---|---|
| `keff_` after step 1 | 0.92836368151245729 | 0.9283636815124880 | +3.1e-14 |
| `pow:` | 947207.42604622082 | 947207.4260462521 | 3.3e-15 relative |
| step residual | 0.99999999999989375 | 1 (printed `1.000000e0`) | 1e-13 |
| wall time, one core | 884 s (ExecutionTime 870 s) | 29 min 23 s | port 2.0x slower |
| peak resident memory | about 3.2 GB (round 1) | 5.35 GB | port 1.7x |

G1 held by six orders (predicted 1e-8, got 3e-14 and 3e-15). **G3 missed**: the step-1 `keff_` of the shipped-tolerance run recorded in round 1
(0.938566, `angularFlux` at 1e-7 / relTol 0.1) differs from the tightly solved 0.928364 by 1.0e-2. The "within 1e-3" prediction assumed the
loose solves were nearly converged; they are not (relTol 0.1 stops each direction at 10 % of its starting residual), so the shipped
GeN-Foam Godiva numbers after step 1 are solver-dependent at the 1 % level, a fact to remember when anyone compares against the tutorial's
shipped log. G2 (inner-sweep counts) is not measured separately; it is implied by G1, since a flipped stopping test between the two codes
would have moved `keff_` by far more than 3e-14. G4: recorded in the table.

What the Godiva step verifies beyond the slab: the multi-group paths (`extraSources` from the other 22 groups with P1 to P3 moments, the
`sigmaFromToYesNo` threshold), the 3-D tetrahedral upwind operator with `inletOutlet` on all patches, 23-group Gauss-Seidel
ordering, and the reader on the shipped `quadratureSet`. It is ONE step: Godiva's later steps and the 20-step trajectory
(the round-1 log shows `keff_` moving from the seed) are not compared; the port is 2x slower and 1.7x larger than upstream here, so the 20-step run
(about 10 hours) was not attempted. The shipped tutorial's own solver tolerance (1e-7 / 0.1) is not reproduced by the port, whose solver stops on a
different residual; at that tolerance the two codes would differ by the solver, not by the physics.


## Round 3 (gh:#1361 plan steps 4 and 5): the thermal-hydraulics stack of the real binary

AI-written, not human-reviewed. Research, education and V&V only. Rule: "ESI preferred"; where
GeN-Foam needs ESI behaviour the port gets it as a named option and the Foundation form stays
callable (maintainer, 2026-11-12).

### Build (measured 2026-11-12; `scripts/build_th_stack.sh`, `WM_NCOMPPROCS=2`, `-O3`, objects deleted after each link)

The 19 ESI closure libraries were built earlier (`../esi_thermo_turbulence_closure_build.md`). New here:

| item | wall | peak RSS | output | transient objects |
|---|---:|---:|---:|---:|
| libFFSSrc2 (`thermalHydraulics/src`, 167 .C, `-g` removed from its `Make/options`) | 11 min 53 s (load shared with another build) | 0.83 GB | .so 6.2 MB | 33.7 MB |
| libFFSSolvers2 (`thermalHydraulics/solvers`, **without `-lthermoTools`**) | 1 min 01 s | 0.94 GB | .so 2.3 MB | 6.4 MB |
| GeN-Foam.C + setDeltaT.C relinked with `-lFFSSrc2 -lFFSSolvers2` and the closure libs | 7 s | 0.50 GB | exe 106 KB | |

Total about 13 min, 8.5 MB of libraries. **`thermoTools` was NOT built and is not needed**: the scoping
comment's estimate (it pulls incompressibleMultiphaseSystems, combustionModels, geometricVoF and
saturationModel, "10 to 30 min") did not arise, because neither GeN-Foam library calls a symbol from it
at link or at load time for the cases run here (the link and the runs below are the evidence;
a case that selected a thermoTools-only boundary condition would fail at run time, "Unknown patchField
type"). Disk never fell below 3.8 GB. Only link lines and one `-g` were edited; no `.C`/`.H` file.

### Case defects found in upstream's shipped 1D_boiling (not port defects)

1. `0/fluidRegion/alpha.liquid` declares `type calulated` on the inlet and outlet. ESI v2506 refuses it
   ("Unknown patchField type calulated"); the port's reader accepts it silently. The comparison copies
   fix the spelling (`boiling_protocol.sh`); the port's tolerance is a deliberate choice recorded in
   `tests/data/genfoam_1d_boiling/PROVENANCE.md`.
2. The shipped `alpha` solver is `"MULES"`; the port has implicitUpwind only (gh:#1219). Both codes
   run `implicitUpwind` with `solvePhase vapour` (ESI requires the `solvePhase` entry; the port
   defaults to the vapour).

### Upstream facts measured before any port run (real binary, serial)

| run | steps | wall | final cumulative continuity error liquid / vapour [kg/m3] |
|---|---:|---:|---|
| shipped tolerances (p_rgh GAMG 1e-6, generic PBiCGStab 1e-6 / relTol 1e-3), implicitUpwind, 12 s | 4401 | 123 s | -0.44658 / +1.997e-4 |
| tight (every `solvers` tolerance 1e-14, relTol 0; residualControl untouched), 12 s | 6157 | 303 s | -2.29649 / +1.155e-3 |

**The liquid continuity error of upstream's own run moves by a factor of 5 with the linear-solver
tolerance alone.** The port's recorded -2.7 kg/m3 (gh:#1239) was measured at the shipped tolerances; the
comparison with the like-for-like upstream number is the port rows below, not this one.

### Predictions, 1D_boiling (written BEFORE the first port run; committed first)

Both codes run the same equations on the same 40-cell mesh, so the translation is judged against these
numbers. `T.vapour` is the field most likely to differ (the vapour is absent, `alpha.vapour = 0`, and
its temperature is set by interface coupling at `residualAlpha`).

| id | quantity | prediction | cause if missed |
|---|---|---|---|
| R1 | tight, steps 1-20: `deltaT` | relative difference < 1e-9 at every step (the step is a function of the Courant number, itself a function of `U` to 1e-9) | Courant definition |
| R2 | tight, steps 1-20: p, p_rgh, T.liquid, U.liquid, U.vapour | worst relative difference < 1e-9 at every step | closure or sign difference |
| R3 | tight, steps 1-20: T.vapour, T.interface, dmdt | below 1e-6 (looser than R2: set by the absent vapour) | residualAlpha handling |
| R4 | tight, steps 1-20: alpha.vapour | exactly 0 in both (no boiling in the first 0.06 s) | |
| R5 | tight, full 12 s: first reference time (0.5 s steps) at which any field leaves 1e-6 | at or after boiling onset (the port's recorded onset, about 3-4 s) | outer-loop count flips (residualControl exit) |
| R6 | tight, full: final cumulative liquid continuity error, port vs upstream -2.2965 | the port lies in [-3.0, -1.6]: i.e. #1239 is the case (a continuity error that grows with tighter solves), not a port defect | |
| R7 | shipped tolerances, full: final cumulative liquid error | the port is NOT within 20 % of upstream's -0.4466, because the two codes' loose-tolerance solvers stop at different residuals; the tight run is the comparison | |

### Predictions, PSBT and gFHR against a FRESH run at writePrecision 17 (written BEFORE the port run)

The earlier tests could only reach the ASCII write precision of the committed CSVs (6 significant digits,
6.15e-6 and 7.2e-6). At 17 digits, with the first step only (no vapour, no boiling):

| id | quantity | prediction |
|---|---|---|
| Q1 | PSBT `Ex1_01_5215` first step: port `FsWallFriction::ReynoldsPower` drag factor from upstream's own Re and Dh, per cell | worst relative < 1e-13 (an ulp-level `pow`; 0 differing cells for the pow-free part) |
| Q2 | gFHR first step: each pebble stage (`keffmatrix`, `Tmout`, `Tmav`, `TfS`, `Tfav`, `Tfmax`) from upstream's `TpS`, per cell | worst relative < 1e-12 per stage (the 5e-6 of the old record is the 6-digit ASCII floor) |

The mesh for gFHR is 292,500 cells; the run is one time step to bound disk.

# Round 4 (gh:#1361 plan step 4, continued): the `onePhase` solver against the real binary

AI-written, not human-reviewed. Research, education and V&V only. Rule: "ESI preferred"; the ESI form
is the default and the Foundation form stays callable (maintainer, 2026-11-12).

## What the port had, and what was compared

Read before any run (upstream is the specification): the port's `solver::one_phase::OnePhaseSolver`
(661 lines) is **not a translation of `solvers/onePhase`**. By its own header it runs constant fluid
properties (`he = Cp T`), an isotropic incompressible `pEqn` built on `rho_pimple_foam`, a fixed-surface-temperature
structure, Gauss-Seidel energy, no `faceCentered` momentum, no `contErr` compensation, no kinetic-energy term,
no thermo package, no case reader and no PIMPLE control. It cannot read a GeN-Foam case, so a "same case files"
comparison of it against the binary is not possible; running it on a hand-built equivalent would compare two
different problems. It is left exactly as it is (it is the named older option; its tests pin it).

What the comparison needs is a translation of `onePhase.C`, `UEqn_1p.H`, `pEqnf_1p.H` (+ `pEqnRoutines/*`),
`EEqn_1p.H`, `FSPair::correct` and `structure::linearizedSemiImplicitHeatSource/correct`. The twoPhase port
already holds the machinery these share (heRhoThermo packages, finite-volume operators, `fixedPower`, the
tight-protocol reader), so the new code is `solver::two_phase::one_phase_fc` (a child module, so it reuses the
two-phase module's private helpers; its public items are `OnePhaseFcCase` and `OnePhaseFcSolver`).

**Case.** None of the shipped onePhase tutorials is small and feature-free: `1D_HX/onePhase` needs
`heatExchangers`, `fixedValue` velocity inlets, `slip` walls and `fixedFluxPressure` (none ported in the
twoPhase machinery either) and `2D_cavityBoussinesq` needs the Boussinesq equation of state and a pressure
reference. The case compared first is therefore a **single-phase channel derived from the 1D_boiling mesh**
(`tests/data/genfoam_1d_onephase`, provenance in its `PROVENANCE.md`): 1D_boiling's 40-cell mesh, boundary
conditions (`p_rgh` 1.4e5 / 1e5 fixed, `U` zeroGradient, inlet `T` fixed 652.15 K), liquid thermophysical package
and heater zone, with `regionSolvers fluidRegion onePhase`, the structure and fluid-structure closures in the flat form
1D_HX/onePhase uses, and 1D_HX/onePhase's own `fvSchemes` and `fvSolution`. The heater power density is 4.5e7 W/m3
(1D_boiling has 4.5e8 and boils; there is no phase change in onePhase). Every file is read by the real binary
unmodified; the protocol (`scripts/onephase_protocol.sh`) only sets `writePrecision 17`, writes every step and, for
`TIGHT=1`, sets every linear-solver tolerance in `solvers` to 1e-14 with relTol 0, as in round 3.

Upstream facts measured before any port run (real binary, serial): 1 s of flow is 108 steps in 0.9 s of wall time;
`deltaT` grows by exactly 1.2 per step to the 0.01 cap (step 13 on); at the tight protocol the cumulative continuity
error after 108 steps is -1.2228e-4 kg/m3 (the shipped-tolerance run gave the same sign and magnitude order; not
recorded further). The 1D_HX/onePhase tutorial itself ran to completion as shipped in 24 s.

## Predictions (written BEFORE the first port run; committed first)

Compared per cell on `p, p_rgh, T, U.x, T.fixedPower, htc, Re, contErr, dgdt` (the fields upstream writes),
as `max |a-b| / max |b|`, tight protocol, every step.

| id | quantity | prediction | cause if missed |
|---|---|---|---|
| O1 | step 1, all fields | worst relative < 1e-10 | first divergent operation: one of O5 to O8, or another |
| O2 | steps 2 to 40 | worst relative < 1e-9 each step (round-off amplified by the outer iterations) | |
| O3 | `deltaT` of every step | bit-identical for the 12 capped-growth steps (the 1.2 growth cap binds, so it does not depend on the Courant number), relative < 1e-12 afterwards | `maxDeltaT` or the Courant definition (`sum abs(phi) / alpha`) |
| O4 | cumulative continuity error after 40 steps | relative difference < 1e-6 | the `contErr` call sites (constructor, start of `pEqn`, end of `correctFluidMechanics`) |
| O5 | `UEqn.relax()` and `EEqn.relax()` (`relaxationFactors equations ".*" 1` still runs OpenFOAM's diagonal-dominance step) | **changes nothing**: both matrices are diagonally dominant everywhere here, so omitting it (`without_equation_relaxation`) leaves every field within 1e-12 of the run with it | |
| O6 | the first-step lazy old time of `p` (`fvc::ddt(p_)` is the first request for `p.oldTime()`, as found for twoPhase in round 3) | applies to onePhase too: `without_first_step_lazy_old_time` moves step-1 `p` or `T` by more than 1e-6 relative | |
| O7 | `includeKineticEnergy` (default true, `EEqn_1p.H`; the twoPhase port has no such term) | upstream's `T` differs from a run without the term by about 0.5 U^2 / Cp = 3e-4 K, i.e. 5e-7 of 652 K; with the term the port is inside O1 | |
| O8 | `structure_.correct(Hs*T.prevIter(), Hs)` uses the temperature BEFORE the energy solve | using the temperature after it (what the twoPhase port does) moves `T.fixedPower` at step 1 by more than 1e-9 relative | |

## Round 4 results (measured 2026-11-12; `tests/genfoam_one_phase_vs_upstream.rs`)

Real binary (tight protocol, every step, `writePrecision 17`) against `OnePhaseFcSolver` on
`tests/data/genfoam_1d_onephase`. `deltaT` is **bit-identical at all 108 steps**. Differences are
`max |port - upstream| / max |upstream|` per field (`contErr`, `dgdt`: scaled by `max(alpha rho)/dt` and `1/dt`,
because they are differences of large cancelling terms).

| step | p | p_rgh | T | U | T.fixedPower | htc | Re | contErr | dgdt |
|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| 1 | 3.5e-14 | 3.5e-14 | 5.2e-16 | 4.5e-14 | 1.7e-16 | 2.5e-15 | 8.1e-13 | 1.1e-14 | 6.8e-17 |
| 2 | 2.5e-13 | 2.5e-13 | 5.2e-16 | 6.6e-13 | 1.7e-16 | 1.7e-15 | 5.3e-13 | 8.6e-15 | 6.7e-17 |
| 40 | 1.4e-13 | 1.4e-13 | 4.3e-14 | 2.8e-12 | 5.3e-14 | 1.9e-13 | 2.6e-12 | 2.0e-13 | 5.1e-15 |
| 108 | 7.0e-14 | 7.0e-14 | 1.2e-13 | 5.3e-12 | 1.3e-13 | 1.8e-13 | 2.7e-12 | 1.2e-13 | 4.8e-15 |

Cumulative continuity error after 108 steps: port -8.750610e-6, upstream -8.750923e-6 kg/m3 (3.1e-10 apart).
Not bit-identical (different linear solvers: ILU-BiCGStab at 1e-13 against GAMG at 1e-14; the operation order of
`fvMatrix::relax` and of the `ddt` correction differ by round-off); round-off level, count of bit-identical
fields not taken. All gated at the predicted bounds (O1, O2: 1e-10 at step 1, 1e-9 after), 2 to 4 orders inside.

| id | held? | what was measured |
|---|---|---|
| O1, O2 | held | table above; bounds in the test |
| O3 | held | `deltaT` bit-identical at all 108 steps |
| O4 | **missed as stated** | 3.6e-5 relative (3.1e-10 absolute), not < 1e-6. Cause: round-off. Each `contErr` is a difference of `alpha rho/dt` ~ 4e5 terms, agreeing to 1e-13 of that scale, so 20 summed contributions are 3e-10 apart. The gate is 1e-9 absolute. |
| O5 | **WRONG** | `fvMatrix::relax(1)` is NOT neutral. Without it step-1 `p` is off by **1.5e-5** (`U` 5.6e-6, `T` 4.6e-8). First divergent operation, found by running upstream with `nOuterCorrectors 1`, `nCorrectors 1` (`scripts/onephase_protocol.sh` `NOUTER`, `NCORR`): `U` differed by 1.9e-5 after one outer iteration and one corrector while `T`, `Re`, `htc`, `T.fixedPower` agreed to 1e-13. ESI `fvMatrix::relax` adds `cmptMax(cmptMag(iCoeffs))` (a magnitude) to the diagonal before the dominance test and subtracts the signed `iCoeffs` after, so a convective INFLOW boundary face (negative `iCoeffs`, the inlet of a non-fixed `U` patch) raises the diagonal by `2 |phi_b|` in that cell; `UEqn.A()`, hence `rAUf` of the inlet face, sees it. Fixed by translating `relax`: after the fix 1.1e-14 at 1 corrector, 3.5e-14 at step 1. The prediction that both matrices are diagonally dominant everywhere was true and beside the point. |
| O6 | held | `without_first_step_lazy_old_time`: step-1 `p` off by 1.2e-2 (`T` 5.5e-5) |
| O7 | **WRONG in size** | `includeKineticEnergy` matters at 7e-11 (`p`), 1.8e-13 (`T`) at step 1, not 5e-7: the term enters as `ddt(alpha rho K) + div(alphaRhoPhi K) - contErr K`, which is nearly zero for a uniform `U`; I had priced `K` itself. It is in the port (default true, `without_kinetic_energy` ablation) and inside the gate. |
| O8 | held for `p`, **missed for `T.fixedPower`** | `structure.correct` with the post-solve temperature: `p` 3.5e-9 (> 1e-9), `T.fixedPower` 6.7e-11 (predicted > 1e-9). |

### Defects found and fixed (all by translating upstream)

1. **`fvMatrix::relax` boundary-inflow rule** (O5 above). `relax_diag` / `convective_inflow2`.
2. **Upstream `customPimpleControl::loop` leaves `converged_` set** when the residual criteria are first
   met in the LAST outer iteration (`corr == nOuterCorrectors`): the `corr == nCorr + 1` branch returns
   without clearing it, so the NEXT time step's first `loop()` prints "converged in 0 iterations" and **solves
   nothing**. Found at step 71, where every field jumped (`T` 1.4e-4 relative, growing by 1.4e-4 every second
   step): the upstream log shows steps 71, 73, ..., 107 with no `PIMPLE: iteration` line. This is
   **upstream's behaviour** (a defect of GeN-Foam 652b3da, not a port defect), ~~reproduced by default as
   `OnePhaseFcControls::stale_pimple_converged_flag = true`; `without_stale_pimple_converged_flag()` keeps the
   clean ending callable~~ **CHANGED 2026-11-12 (maintainer decision, "yes", gh:#1361): the default is now the
   corrected clean ending** (`stale_pimple_converged_flag = false`); upstream's defect is the explicit
   `with_upstream_stale_pimple_converged_flag()`, which the upstream comparison test sets. The clean ending
   solves the skipped steps and ends 2.6e-3 (`T`) from upstream's defective run at step 108 (see "Round 4
   follow-up"). **It applies to twoPhase too** (same control class upstream): a twoPhase run whose criteria are
   first met at `corr == nOuterCorrectors` skips the next step upstream. ~~Not yet checked in the twoPhase
   port~~ **CHECKED 2026-11-12:** see "Round 4 follow-up".
3. **`calcCumulContErr` runs only in the iteration flagged converged.** It sits under `if (pimple_.finalIter())`,
   whose inherited definition (`converged_ || corr_ == nCorrPIMPLE` on the BASE class's `corr_`) is true only in
   the flagged iteration; the custom loop keeps its own `corr_`. A step that never meets its criteria adds
   nothing: upstream printed "Cumulative continuity error" in 20 of 108 steps. Before the fix the port added in
   every step (-3.3e-5, a factor 3.8 off); after, -8.75061e-6.
4. In the case reader, not a physics defect: `solvers."h.*"` is overridden by the later `".*"` (OpenFOAM matches
   patterns last-first), so upstream solves `h` with `diagonalPBiCGStab`, not the `smoothSolver` the entry names;
   at the tight protocol both stop at round-off, so the port keeps `smoothSolver` (unchanged from twoPhase).

### Options added (ESI form is the default; the other form stays callable)

`without_equation_relaxation`, `without_first_step_lazy_old_time`, `without_kinetic_energy`,
`with_structure_after_solve_temperature`, ~~`without_stale_pimple_converged_flag`~~ `with_upstream_stale_pimple_converged_flag` (renamed 2026-11-12: the upstream defect is now the explicit option). Their misses are pinned in
`one_phase_channel_options_reproduce_the_recorded_misses`. No Foundation-vs-ESI divergence was found that needed
a new option beyond these; `Time::adjustDeltaT` reuses the ESI form from round 3.

### Moved numbers

None recorded before this round concerned `onePhase` against the binary. The twoPhase port and the older
`OnePhaseSolver` are untouched (`case.rs` helpers were made `pub(super)`, nothing else there changed).

### What remains

* The U.vapour residual noted for the 1D_boiling twoPhase run (5.7e-6 at step 2, 2.1e-5 at step 1: the absent-vapour
  inlet cell) was **not attempted**: this round went to onePhase; it is a twoPhase item.
* ~~Check whether twoPhase hits the stale `converged_` flag (item 2) and add the same named option.~~ **DONE 2026-11-12**, see "Round 4 follow-up".
* `1D_HX/onePhase` itself (heat exchangers, `fixedValue` U inlet, `slip` walls, `fixedFluxPressure`) and
  `2D_cavityBoussinesq` (Boussinesq, closed volume) still cannot be read; each refusal is named in `OnePhaseFcCase::read`.
* `contErr` / cumulative error are round-off limited (1e-13 of the `rho/dt` scale); bit-identity was not pursued.


### Round 3 results (measured 2026-11-12)

Tests: `tests/genfoam_two_phase_vs_upstream.rs` (6 gates + 1 diagnostic), `tests/genfoam_psbt_closures.rs`
(new test `..._fresh_17_digit_psbt_run`), `tests/genfoam_gfhr_pebble_vs_upstream.rs` (new test
`..._fresh_17_digit_gfhr_run`). References (gzipped text, 0.04 to 0.18 MB each): `refs/boiling_tight_steps80`
(65 steps, every step), `refs/boiling_tight_onset` (steps 616-627), `refs/boiling_tight_full` and
`refs/boiling_shipped_full` (every 0.5 s of the 12 s run), `refs/psbt_psbt_ex1_t1` and `_t5`,
`refs/gfhr_chain`. Upstream runs, all the real unmodified binary, serial, ESI v2506:

| run | steps | wall | notes |
|---|---:|---:|---|
| 1D_boiling, shipped tolerances, `implicitUpwind` | 4401 | 123 s | final cumulative continuity error liquid -0.44658 kg/m3 |
| 1D_boiling, tight (every `solvers` tolerance 1e-14, relTol 0) | 6157 | 303 s | liquid -2.29649 kg/m3; boiling onset at step 618, t = 6.02 s |
| PSBT `PhaseII_Ex1_01_5215`, writePrecision 17 | 1065 | 111 s | max `alpha.vapour` 0.12383476677792404: the tutorial's own `Alltest` value (0.123835) reproduced |
| gFHR `steadyStateNSSP`, writePrecision 17, 3 steps (292,500 cells) | 3 | 25 s init + 46 s, 0.82 GB | the tutorial's `-initializeMappedFields` then the run |

#### 1D_boiling, port vs upstream (tight protocol, every step, 40 cells)

The progress of the port's agreement as each defect below was fixed, at step 1 / step 2 / step 20 of the
first 20 steps (worst relative difference of any field, relative to that field's maximum):

| state | step 1 | step 2 | step 20 |
|---|---|---|---|
| port at the start of round 3 | p 1.5e-3, T.vapour 3.0e-2 | p 2.2e-4, T.vapour 2.6e-2, T.interface 1.0e-2 | p 3.7e-4, T.liquid 8.5e-4 |
| + lazy old-time pressure and structure (D1, D2) | p 4.6e-12, T.liquid 1.4e-14, U.vapour 1.1e-5 | T.interface 1.1e-2 | T.liquid 8.5e-4 |
| + `T.interface` limiter as OpenFOAM runs it (D3) | unchanged | p 8e-12, T.interface 1.6e-11 | T.liquid 8.5e-4 (the step size now differs: protocol) |
| + the every-step write control also applied to the port's case copy (my harness), and old-time patch values (D5) | p 7.6e-11, U.vapour 2.1e-5 | p 8.4e-12, U.vapour 5.7e-6 | p 4.5e-13, T.liquid 3.1e-15, T.vapour 1.7e-10 |

Final state, pinned by the tests (all fields p, p_rgh, T.liquid, T.vapour, T.interface, T.fixedPower, U.liquid, U.vapour,
alpha.vapour, dmdt, each as worst |port - upstream| / max |upstream| over the 40 cells):

| span | what | result |
|---|---|---|
| steps 1-65 (to t = 0.5 s) | step size | **identical** at every step (relative difference 0) |
| steps 3-65 | p, p_rgh | <= 4.7e-12 |
| | T.liquid, T.fixedPower | <= 2.5e-13, 1.3e-13 |
| | U.liquid | <= 9.3e-12 |
| | T.vapour, T.interface, U.vapour | <= 2.3e-10, 7.6e-10, 7.0e-10 |
| | alpha.vapour, dmdt | exactly 0 in both (no vapour) |
| steps 1-2 | everything except U.vapour | <= 1e-10 |
| steps 1-2 | U.vapour | 2.1e-5 (step 1), 5.7e-6 (step 2): **a known, unexplained residual** (below) |
| t = 0.5 ... 6.0 s of the full 12 s (12 reference times) | every field | <= 2.5e-11 (the vapour is absent); before D4 the same times were off by 1e-6 to 1e-4 (the port took 77 steps to t = 0.5 s, 66 after) |
| step 616 (t = 6.0 s) | every field | <= 5.5e-12 |
| steps 617-627 (boiling onset) | p, p_rgh | <= 3.6e-10 |
| | T.liquid, T.vapour, T.interface, T.fixedPower | <= 1.2e-11, 2.6e-11, 4.0e-11, 1.0e-11 |
| | U.liquid, U.vapour | <= 1.7e-9, 1.9e-8 |
| | **alpha.vapour, dmdt** | **3.4e-6, 3.1e-6** of their maxima (1.4e-6 and 2.6e-7 at the first boiling step) |
| t >= 6.5 s | all fields | O(1e-2) to O(1): not reproducible point-wise, see "the noise floor" |
| shipped tolerances, t <= 6.0 s | all fields | T.liquid <= 3.0e-5, T.fixedPower <= 3.0e-5, p <= 4.2e-7, p_rgh <= 6.5e-7: the two codes' loose solvers stop at different residuals |

**The noise floor.** Port against ITSELF with only the `alpha` linear solve changed (the old exact solve, an
ablation that moves alpha at onset by 1e-6): identical to 0.0 at t = 6.0 s, and at t = 6.5 s differing by
p 2.3e-2, T.liquid 9.0e-3, U.liquid 1.7e-1, alpha.vapour 7.6e-1, dmdt 3.4e-1 (7.0 s: 5.0e-2, 1.9e-2, 4.0e-1, 5.1e-2, 2.1e-1).
That is the same size as port against upstream at 6.5 s (p 6e-2, alpha.vapour 0.68-0.82). So after the boiling onset this
case is a flow excursion that amplifies a 1e-6 difference in the vapour tail to O(1) within 0.5 s, and **a
pointwise comparison after 6.0 s measures the flow's sensitivity, not a model difference.** The comparison that can
be made, and is gated, is everything up to and through the first twelve boiling steps.

**Where the remaining onset difference comes from.** Upstream solves the `alpha.vapour` equation with the case's
`".*"` entry, `PBiCGStab` with the `diagonal` preconditioner, stopping on a residual normalised by the matrix norm.
The tail of the vapour front (cells at 1e-9 to 1e-17) is therefore solver noise in upstream, not the exact solution
of the triangular upwind system that the port computed before. Reproducing the algorithm and the stopping rule
(D6) cut the alpha difference at the first boiling step from 1.3e-4 to 1.4e-6. Bit-identity of the tail would need
bit-identical inputs at step 616 and the p_rgh solve (upstream GAMG vs the port's ILU-BiCGStab) and was not reached.

**#1239 (liquid continuity error -2.7 kg/m3 unexplained): answered.** Upstream's own run on the same case, with the same
`implicitUpwind` alpha, has a cumulative liquid continuity error of **-0.4466 kg/m3 at the shipped solver tolerances and
-2.2965 kg/m3 at 1e-14**: it moves by a factor 5 with the linear-solver tolerance alone, in the same direction
(more negative when tighter) as the port's (shipped -1.860, tight -4.290). It is a property of the case's boiling
phase (a flow excursion, which the previous section shows is not reproducible point-wise), not a port-specific defect.
The port's numbers are not within 20 % of upstream's at either tolerance, which is what the noise floor predicts.

#### Predictions of this round: what held

| id | held? | measured |
|---|---|---|
| R1 step size, steps 1-20 within 1e-9 | **missed as first run, then held** | first run: up to 2.5e-1. Two causes, both found: my harness did not apply the every-step write control to the port's case copy (the port then snapped its step to the write interval), and the port's snapping was not ESI's `Time::adjustDeltaT` (D4). Now exactly identical for 65 steps |
| R2 p, p_rgh, T.liquid, U within 1e-9 | **missed as first run, then held** | first run: p 1.5e-3 at step 1. D1-D3, D5. Now p 7.6e-11 at step 1, <= 5e-12 from step 3 |
| R3 T.vapour, T.interface, dmdt within 1e-6 | held after the same fixes | 1.7e-10, 7.6e-10, exactly 0 |
| R3 (implied) U.vapour | **missed** | 2.1e-5 at step 1, 5.7e-6 at step 2, then <= 7e-10: inlet cell of the absent vapour |
| R4 alpha.vapour exactly 0 | held | exactly 0 in both |
| R5 first reference time at which any field leaves 1e-6 is at or after the boiling onset (about 3-4 s) | **held in kind, missed in number** | no field leaves 1e-9 before onset; the onset is at t = 6.02 s (port: 6.14 s first normalised alpha > 1e-3), not 3-4 s |
| R6 tight, final liquid error in [-3.0, -1.6] | **missed** | port -4.290 (upstream -2.2965) |
| R7 shipped, not within 20 % of -0.4466 | held | port -1.860 |
| Q1 PSBT drag within 1e-13 | **missed** | t = 5 s (steady): worst 1.2e-11 (84,616 ulp), 836 single-phase cells, none bit-identical. t = 1 s (transient): 8.0e-6 |
| Q2 gFHR chain within 1e-12 | **held, bit-identically** | 2,927 sampled cells, five stages and `keffmatrix`: **0 ulp** |

Q1's miss is informative twice. At t = 5 s the 17-digit number (1.2e-11) shows that the 6.15e-6 recorded in
`genfoam_psbt_closures.rs` was the write precision of its CSV, as stated there. At t = 1 s the same relation is off
by 8.0e-6 at full precision, which is not write precision: `Kd` and `Re` are computed at the start of an outer
iteration and `alphaRhoMagU` at its end, and `residualControl` is 1e-5, so in a transient the three fields are
not one state. **This explanation is a hypothesis, not tested.** The 1.2e-11 at t = 5 s is also not explained to
round-off: the declared hydraulic diameters (8 significant digits in the case) are upstream's exact inputs.
gFHR: the older test's `POWER_DENSITY = 19_965_700` is a rounded value; the case states `19.96567386e6`, and using the
rounded one against the 17-digit run gives 1.8e-8 (Tmout) to 8.9e-8 (Tfmax). With the case's value every stage is
bit-identical.

#### Defects found and fixed by translating upstream (each first divergent operation, its sign and size)

Each is **upstream behaviour made the default** and the port's earlier form kept callable by a named method (the
maintainer's rule: do not delete the dev form). None is in a kovan-reviewed function (`review.md` of
`solver/two_phase/`, `solver/` and `thermal_hydraulics/` carry no `kind = "review"` block).

| id | first divergent operation | upstream does | option (old form) |
|---|---|---|---|
| D1 | `fvc::ddt(p_)` in `EEqns_2p.H`, outer iteration 1 of step 1: `T.liquid` rose 9.6e-4 K in the port, 5.6e-5 K upstream | OpenFOAM creates `p.oldTime()` at its FIRST request, so in step 1 the old pressure is the one after the first `p_rgh` solve and `dpdt` is 0 in iteration 1 | `TwoPhaseControls::lazy_old_time_first_step`, `without_first_step_lazy_old_time()` |
| D2 | `fixedPower::correct`, `T_.oldTime()` and `T_[celli] = ...`: structure `T.fixedPower` +0.1155 K in step 1 upstream, +1.5e-5 K in the port at iteration 2 | `T_[celli]` goes through `operator[]`, which does not call `storeOldTimes`, so the copy made in iteration 1 carries a stale time index and is overwritten with the iteration-1 result at the start of iteration 2 (once): the structure gets one extra step of power in step 1, for the whole run | same switch |
| D3 | `phaseChangeModel::correctInterfacialTemperature`, `maxTInterfaceDdt`: step 2 `T.interface` +4.747 K upstream (saturation jump +37.98 K), +1.375 K in the port | `iT_.oldTime()` is read after `correctField` has overwritten `iT_` and called `correctBoundaryConditions` (`storeOldTimes`), so the limiter compares the new value with itself and never limits (iteration >= 2: with the iteration-1 raw value) | `t_interface_limiter_sees_new_value`, `with_documented_t_interface_rate_limiter()` |
| D4 | step size at step 3 (0.0012086 vs 0.00121) | ESI `Time::adjustDeltaT`: `max(1, round(nSteps))`, growth capped 2x, shrink 0.2x, `writeTimeIndex` from `Time::operator++` | `TimeControls::esi_adjust_delta_t`, `with_foundation_style_write_time_adjust()` (**this is the ESI vs earlier-rule option**) |
| D5 | vapour inlet-face flux at step 1: port -9.80010e-4, upstream -9.80219e-4 m3/s | old-time patch values are stored with the field (`oldTime()` of `rho`, `alpha`), not the owner cell's | none (a plain fix) |
| D6 | tail of `alpha.vapour` at the boiling onset (relative 1.3e-4) | `alpha.vapour` solved by OpenFOAM `PBiCGStab`/`diagonal`, the matrix assembled in OpenFOAM's order | `alpha_solver_openfoam`, `with_exact_alpha_solve()` |

The ablation numbers are pinned (`one_d_boiling_ablations_reproduce_the_recorded_misses`): without D1/D2/D3 step 1 `p` is off by 1.5e-3,
without D3 step 2 `T.interface` by 1.1e-2, and the earlier step rule differs from ESI's by up to 5.3e-1 in the step size over 30 steps.

Two defects were in my own work and are recorded because they would have passed silently: (1) the reference extractor
wrote nothing for an `internalField uniform V;` field (OpenFOAM writes that form for a field whose cells are all equal),
so `T.interface`/`T.vapour` at step 1 were compared against an empty list, which passes; the extractor now expands
it and the comparison asserts equal length; (2) the first references had been written with `writeControl timeStep`
while the port's case copy still said `adjustableRunTime`.

#### Moved numbers (old to new, cause)

| number | old | new | cause |
|---|---|---|---|
| `1D_boiling` port, cumulative liquid continuity error, shipped tolerances (`genfoam_two_phase` README P5) | -2.675 kg/m3 | **-1.860 kg/m3** | D1-D6 (the run is chaotic after onset, so the move is not itself an error estimate) |
| same, vapour | 1.85e-3 | 8.51e-4 kg/m3 | same |
| P3 minimum inlet liquid velocity during boiling | -0.94 m/s | -0.735 m/s | same |
| P4 maximum void at 12 s | 5.9e-12 | 2.4e-10 | same |
| P2 first normalised alpha.vapour > 1e-3 | 6.14 s | 6.14 s | unchanged |
| PSBT drag deviation, t = 5 s | 6.15e-6 (6-digit CSV) | 1.2e-11 (fresh 17-digit run) | higher write precision; the CSV record stays valid |
| gFHR pebble chain, worst stage | 7.2e-6 (6-digit CSV, t = 50 s) | 0 (fresh 17-digit run, t = 0.358511 s, 2,927 cells) | higher write precision, case's power density |
| `genfoam_two_phase_1d_boiling.rs` P1 isothermal | asserted < 1e-3 | 2.1e-5 | the same changes |

No ESFR, MSFR, albedo, Godiva number was re-run or moved by this round (their code paths are untouched; the shared
`FixedPower` is unchanged, `power_model.rs` was not edited).

#### What remains

- onePhase TH against the binary: `featureCases/2D_cavityBoussinesq` runs on the real binary after the build here (not run
  in this round), but the port's `OnePhaseSolver` has no case reader, so the comparison needs a harness that builds the
  mesh, boundary conditions and Boussinesq thermo in Rust. Not started.
- The U.vapour residual at steps 1-2 (absent vapour, inlet cell): after D5 the vapour inlet-face flux differs by 4e-6
  relative; it is consistent with the old-time vapour `rho` patch value being about 0.57 % lower in upstream, cause not found.
- PSBT beyond the closure: the port refuses the case (`multiRegimeBoilingTRACE`, `twoParameters`, `byRegime`), so only the
  `ReynoldsPower` relation is compared. `1D_CHF`, `2D_KNS37-L22` (MULES) and PSBT Ex2/Ex3 stay port gaps.
- Coupled transients (MSFR, ESFR) with the TH region kernel (#1475), thermoMechanics (offbeat stubbed), and the full-binary
  runs that need `thermoTools` (not built here, and not needed for any case run in this round).
- Bit-identity of the alpha tail at the onset needs bit-identical p_rgh solves (GAMG).


## Round 4 follow-up (2026-11-12): corrected convergence flag becomes the default

**Maintainer decision, 2026-11-12 ("yes"), recorded on gh:#1361:** GeN-Foam ports default to the CORRECTED
behaviour; upstream's defect is the explicit, named option. Applied to `onePhase` (faceCentered) as
`OnePhaseFcControls::stale_pimple_converged_flag` default **`false`** and
`OnePhaseFcSolver::with_upstream_stale_pimple_converged_flag()` (replacing `without_stale_pimple_converged_flag`).
`tests/genfoam_one_phase_vs_upstream.rs` sets the upstream option in every comparison against the binary.

**Predictions, written before the re-run (the upstream binary is not available, so only committed references are used):**

* P1. The upstream comparisons (first 40 steps, full 108 steps) with the upstream option are unchanged to the
  last digit: the flag first matters at step 71 and the option restores the old code path exactly (bit-identical
  to the previous default run).
* P2. Default (corrected) vs upstream-bug run at step 108: `T` differs by between 1e-4 and 1e-2 relative (the
  skipped steps accumulated 1.4e-4 per second step in the earlier record, over 19 skipped steps, but the
  corrected run catches up partly); `p` and `U` by less than `T`; `T.fixedPower` follows `T`. Cause: the
  18 or so steps upstream skips (every second step from 71) are solved in the corrected run.
* P3. twoPhase: the port's own loop keeps `converged` local to the step (`two_phase/mod.rs`), so it never
  carries the flag across steps and is already the corrected form. Whether 1D_boiling ever first meets its
  criteria in the last outer iteration (the case in which upstream would skip the next step) is predicted
  to be: no, in the 65-step and onset records.

**P3 (above) was WRONG, and the twoPhase comparison needs the option.** Before the option existed, the twoPhase
port was instrumented (`StepReport::upstream_would_skip_next_step`) and the whole 12 s run printed the steps
whose criteria were first met in the last outer iteration: **1D_boiling does hit the condition**, tight protocol
at steps 751, 1763, 2421, 3309, 5248, 5254, 5273, 5274, 5403, 6033, 6037, 6523, 6615, 6660, 6662, 6664, 6666,
6668, and at the shipped tolerances at 735, 1108, 4848, 4849, 4889, 4997, 5171, 5287, 5288. Every one is after the
boiling onset (step 618, 6.02 s), so the gated pre-onset and onset comparisons are untouched. Upstream skips the
step after each of these; the port (clean ending, the port's old behaviour and the new default) did not.
Added `TwoPhaseControls::stale_pimple_converged_flag` (default `false`) and
`TwoPhaseSolver::with_upstream_stale_pimple_converged_flag()`; the upstream comparison tests set it.

**Predictions for the twoPhase option, written before its first run:**

* P4. The first 65 steps and the onset window (steps 1-627) are unchanged with the option (first hit is step 735
  or later): same bounds hold, bit-identical numbers.
* P5. Pre-onset full-run rows (t <= 6 s) unchanged. Post-onset rows (t >= 6.5 s) move, because the skipped steps
  now exist; the port's earlier O(1) post-onset disagreement (alpha.vapour, dmdt) is mostly flow sensitivity and
  I predict the option does NOT bring it to round-off; I predict `p` and `T.liquid` at t = 12 s move by more
  than 1e-4 and get closer to upstream by less than a factor of ten. The cumulative continuity errors stay far
  from upstream's (-0.4466, 2.0e-4 shipped) because the port adds them every step, not only in the iteration
  flagged converged (the onePhase finding 3); that is a separate, unported item for twoPhase.

### Results (measured 2026-11-12, release, `genfoam_one_phase_vs_upstream`, `genfoam_two_phase_vs_upstream`, `genfoam_two_phase_1d_boiling`)

**onePhase, upstream comparison (P1 held).** With `with_upstream_stale_pimple_converged_flag()` set, the first 40
steps and the full 108-step comparison pass at the same unchanged bounds, and the cumulative continuity error is
the same to 16 digits as before the change (port -8.750609708659356e-6 vs upstream -8.750922625618369e-6 kg/m3).
The flag first bites at step 71, so steps 1-70 could not move.

**onePhase, MOVED NUMBER: default (corrected) vs the upstream-defect run, step 108 (P2 held).** The corrected run
solves all 108 steps; the upstream-option run skips 19 (every second step from 71, as upstream's log shows).
Worst relative difference between the two end states (relative to the field maximum; `contErr` scaled by
`max(alpha rho)/dt`, `dgdt` by `1/dt`, as in `compare`):

| field | p | p_rgh | T | U | T.fixedPower | htc | Re | contErr | dgdt |
|---|---|---|---|---|---|---|---|---|---|
| corrected vs upstream-bug | 4.2e-5 | 2.0e-5 | 2.6e-3 | 3.9e-4 | 2.6e-3 | 1.3e-3 | 3.1e-3 | 3.0e-2 | 1.2e-1 |

Cause: the skipped steps advance time and the structure power but solve nothing, so the upstream run lags the
corrected one by up to 19 solves. Gated by `one_phase_default_is_the_corrected_pimple_ending` (default is `false`,
the default run skips 0 steps, the option skips 19) and by `one_phase_channel_options_reproduce_the_recorded_misses`
(the default end state differs from upstream's by more than 1e-5 in `T`). **Quote the option's numbers when the
claim is "agrees with upstream"; quote the default's when the claim is "is correct".**

**twoPhase: 1D_boiling DOES hit the condition (P3 wrong, above).** The port's own loop was always the clean ending, so
nothing moved in its default behaviour; the option reproduces upstream's skip. Results:

* P4 held: with the option, steps 1-65 and 616-627 pass at their unchanged bounds (first hit is step 735/751).
* Full tight run, rows t <= 6 s (P5, pre-onset part, held): identical to the digit to the default's rows (worst
  2.5e-11 at 6 s, the 1e-10 gate).
* Full tight run, post-onset, default (old) -> with the upstream option (new), worst relative difference at t = 12 s:
  `p` 1.7e-5 -> 4.8e-4, `T.liquid` 4.9e-4 -> 1.5e-2, `T.fixedPower` 4.6e-4 -> 1.6e-2, `U.liquid` 1.1e-4 -> 3.2e-3,
  `T.interface` 2.6e-6 -> 7.0e-5. Steps: default 7454, option 5569, **upstream 6157** (|diff| 1297 -> 588).
  Cumulative continuity error (liquid, vapour) kg/m3: default (-4.407, 3.89e-5), option (-7.521, 5.86e-3), upstream
  (-2.296, 1.155e-3). The option skips steps after 751, 1766, 2033, 2969 (the trajectory differs from the default's
  list 751, 1763, 2421, ...). **P5's last clause was WRONG:** the option did not bring the t = 12 s fields closer to
  upstream, it moved them away (and the step count nearer). The post-onset run is chaotic
  (`one_d_boiling_post_onset_sensitivity_of_the_port_to_the_alpha_solve` shows an O(1) change from a 1e-6
  perturbation), so neither post-onset figure is evidence about the flag; the gated, quotable numbers are the
  pre-onset ones. I did not use the post-onset closeness to choose the default (the maintainer's decision fixes it).
* Full run at the shipped tolerances, default: hits at steps 735, 1108, 4848, 4849, 4889, 4997, 5171, 5287, 5288.
  **With the option this run stalls after t = 9.5 s** (step 4903, over 20 min of CPU with no further reference time
  reached; stopped by hand; cause not investigated: a stalled solve or a collapsing step, not looked at). Because a
  hang cannot be a test, `one_d_boiling_full_shipped_vs_upstream` runs the default and says so in its comment; its
  gated part (t <= 6 s) cannot differ, the first hit being after the onset. **Open item** for the maintainer.
* Not ported for twoPhase, still open: `calcCumulContErrs` only in the iteration flagged converged (onePhase
  finding 3; the twoPhase port adds it every step), which is part of why the cumulative errors above are far off.
* `one_d_boiling_default_is_the_corrected_pimple_ending` pins the twoPhase default (flag `false`, default run skips 0
  steps, upstream option skips 4, 1D_boiling reaches the condition 18 times).
