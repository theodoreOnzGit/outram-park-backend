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
  inlet cell) ~~was **not attempted**: this round went to onePhase; it is a twoPhase item.~~ **DONE 2026-11-13 (round 5, "Item 4"):** localised to
  the first divergent operation and fixed.
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
| steps 1-2 | U.vapour | 2.1e-5 (step 1), 5.7e-6 (step 2): ~~**a known, unexplained residual** (below)~~ **CORRECTED 2026-11-13:** explained and fixed in round 5 (the construction-time `p` patch, "Item 4"): 1.35e-11 and 1.96e-11 now |
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
- ~~The U.vapour residual at steps 1-2 (absent vapour, inlet cell): after D5 the vapour inlet-face flux differs by 4e-6
  relative; it is consistent with the old-time vapour `rho` patch value being about 0.57 % lower in upstream, cause not found.~~
  **CORRECTED 2026-11-13:** cause found and fixed (round 5, "Item 4"): it was the construction-time patch of `p` (the case file's 1e5, not
  `p_rgh`'s 1.4e5), which sets the patch densities of the absent phase and their lazily stored old-time copies. The "0.57 % lower" guess was not
  the mechanism (the patch densities differ by 40 %, 0.7462 against 0.5330; the flux effect is 4e-6 because the liquid dominates the partial elimination).
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
  ~~**With the option this run stalls after t = 9.5 s** (step 4903, over 20 min of CPU with no further reference time
  reached; stopped by hand; cause not investigated: a stalled solve or a collapsing step, not looked at). Because a
  hang cannot be a test, `one_d_boiling_full_shipped_vs_upstream` runs the default and says so in its comment; its
  gated part (t <= 6 s) cannot differ, the first hit being after the onset. **Open item** for the maintainer.~~
  **CORRECTED 2026-11-13 (round 5, "Item 2"):** it was a collapsing time step in the port's own post-onset trajectory,
  not a stalled solve and not the control flow; explained, guarded, and no longer reproduced by the default construction.
  `one_d_boiling_full_shipped_vs_upstream` now sets the option.
* ~~Not ported for twoPhase, still open: `calcCumulContErrs` only in the iteration flagged converged (onePhase
  finding 3; the twoPhase port adds it every step), which is part of why the cumulative errors above are far off.~~
  **DONE 2026-11-13 (round 5, "Item 3").**
* `one_d_boiling_default_is_the_corrected_pimple_ending` pins the twoPhase default (flag `false`, default run skips 0
  steps, upstream option skips 4, 1D_boiling reaches the condition 18 times).


# Round 5 (gh:#1361, 2026-11-13): twoPhase PIMPLE control flow against the real binary, step by step

AI-written, not human-reviewed. Research, education and V&V only. Upstream rebuilt (ESI v2506 and GeN-Foam 652b3da,
`scripts/upstream/build-esi-v2506-and-genfoam.sh`); the numbers below are from that build. Predictions are committed
BEFORE the port runs they concern.

## What upstream's `customPimpleControl` does, read before any run (the specification)

Read: `customPimpleControl.{C,H}`, `customPimpleControlI.H`, ESI `pimpleControl.{C,H}`, `pimpleControlI.H`,
`solutionControl.H`, and every GeN-Foam call site of `finalIter`/`firstIter`/`isFinalIteration`.

* `customPimpleControl` declares its OWN `int corr_` (public). `loop()` and `criteriaSatisfied()` advance and read
  that one. The inline, NON-virtual `pimpleControl::finalIter()`, `firstIter()` and `storeInitialResiduals()` read
  the BASE `solutionControl::corr_`, which is never advanced, so it stays 0. Consequences (all verified in the
  upstream log, which prints `ini res = 0` in every "PIMPLE loop:" line):
  1. `finalIter()` is `converged_` only. At `corr == nOuterCorrectors` without convergence it is false, so the
     data flag `finalIteration` is NOT set for the last iteration (only the flagged iteration is "final").
  2. `storeInitialResiduals()` is always false, so `residualControl_[f].initialResidual` stays 0, `relative =
     last/(0 + SMALL)` is huge and `relCheck` is false for any `relTol` in these cases: the criterion is the
     ABSOLUTE one, `residuals.last() < tolerance`, only.
  3. `calcCumulContErrs` is under `if (pimple_.finalIter())` -> only in the iteration flagged converged.
* The criteria use `previousPIMPLEFirstPISORes_.last()` for `p_rgh` (`useFirstPISOInitialResidual true`: the initial
  residual of the first PISO corrector of the last outer iteration, stored by `updateFirstPISOPrevPIMPLE()` after each
  `p_rgh` solve with `corrPISO_ == 1`) and `maxResidual().last()` for `h.liquid` (the last `h.liquid` initial residual).
* Flag then run: criteria met at the START of outer iteration c (c >= 2) sets `converged_`, runs iteration c as the
  final one; the next `loop()` prints "converged in c iterations" and clears it, EXCEPT c == nOuterCorrectors, where the
  `corr == nCorr + 1` branch returns first and leaves `converged_` set (the stale flag, round 4).

Where the port stood before this round (read, not run): the loop is the same call sequence; the cumulative continuity
error was added after the loop in EVERY step, including a skipped one (item 3 of the task; the onePhase port had
already moved it into the iteration flagged converged).

## Predictions (written BEFORE the port flow run)

The reference is a fresh upstream run: 1D_boiling, tight protocol, 12 s, default write control, `scripts/extract_pimple_flow.sh`
on its log: one line per step with the iterations run, the ending (`conv<N>`, `notconv`, or `conv0` = skipped step) and the
`p_rgh`/`h.liquid` residuals the criteria printed. Upstream: 6157 steps, 8 skipped steps (conv0), each preceded by a step
flagged in its last iteration. The port is run with `with_upstream_stale_pimple_converged_flag()`.

| id | prediction | reason |
|---|---|---|
| F1 | steps 1-617 (single phase): the port's iteration count and ending equal upstream's at EVERY step | the fields agree to 1e-10 there (round 3), so the criteria see the same numbers |
| F2 | the residuals in the lists of F1 agree to better than 1e-6 relative wherever they exceed 1e-8 | same state; the first-PISO `p_rgh` initial residual and the `h.liquid` initial residual are computed on the same matrices. Entries at the 1e-14 noise floor are not compared |
| F3 | the first step with a different iteration count is after the onset (> 618), and at that step the deciding residual (the one that crossed 1e-5 in one code and not in the other) is within a factor 2 of 1e-5 in BOTH codes | the two post-onset trajectories differ by 1e-6 in alpha at the onset and amplify (round 3 noise floor): a threshold crossing, not a control-flow difference |
| F4 | step 751, flagged in the last iteration by the port, is not flagged by upstream: upstream's last printed `p_rgh` residual is 1.5264e-5 (read from the log), above 1e-5. I predict the port's value is also within a factor 2 of 1e-5 | same as F3 |
| F5 | no code defect is found in the loop's control flow itself. The explanation of "the option moves the run away from upstream" is that after the onset a single threshold crossing changes the trajectory, not that the translation differs | the call sequence of `step()` equals `customPimpleControl::loop()` line by line (see above) |

A control experiment that removes the chaos (the port's only possible failing is then the loop): the same case with
`nOuterCorrectors 5` (upstream's `NOUTER` hook of `boiling_protocol.sh`), stopped before the onset (t <= 3 s, every step written).
At 5 outer iterations many steps meet the criteria exactly at iteration 5 (stale flag, skipped next step) and others never meet
them (`notconv`), so both branches of the loop are exercised on a single-phase flow whose fields agree to 1e-10.

| id | prediction |
|---|---|
| C1 | at `nOuterCorrectors 5` the port with the upstream option and upstream skip THE SAME steps (the same list of step numbers), at least 5 of them |
| C2 | every field of every step, including the skipped ones, agrees as in round 3 (`p`, `p_rgh`, `T.liquid`, `U.liquid` <= 1e-9; `T.vapour`, `T.interface`, `U.vapour` <= 1e-7), with no growth across skipped steps |
| C3 | the cumulative continuity errors, which the port now adds only in the iteration flagged converged, agree with upstream's final value to 1e-4 relative (round-off of the 1e-13 scale cancelling, as for onePhase 3e-10 absolute), where before this round they were a factor of a few off |

### Item 4 (the `U.vapour` residual): localised before any fix, hypothesis committed before the fix

`nOuterCorrectors 1`, `nCorrectors 1` (new `NCORR` knob of `boiling_protocol.sh`), step 1, port against the real binary,
tight protocol (measured 2026-11-13): `U.liquid` agrees to 1.1e-13, `p` to 2e-7 of 1.4e5, `T.liquid` to 1e-13 and `U.vapour` in
cells 1-39 to 9.4e-12, but **`U.vapour` in cell 0 (the inlet cell) is off by 2.07e-6** (relative 2.1e-6); the same cell with
`nOuterCorrectors 2` (one corrector) is off by 8.2e-5 and cell 1 by 6.1e-6. Upstream's `phi.vapour` is uniform to 1e-15
over all 39 internal faces AND the inlet face (-9.8058157811760707e-4), i.e. the vapour flux at the inlet face follows the same
relation as at the interior faces. The port's inlet-cell velocity implies an inlet-face flux 4.2e-6 (relative) larger.

Read: `alphaRhof2byDt = interpolate(max(alpha2, residualAlpha) rho2)/dt` and `alphaRhof20byDt` (same with the old
time) enter `rAUf2` and `phiS2`; for an absent phase (alpha = 0) they are `residualAlpha rho2`, so the BOUNDARY value of `rho2` at the
inlet decides the inlet-face flux. The port printed (temporary diagnostic, removed): at step 1, outer iteration 1,
`p.b[inlet] = 1.4e5` (cell 1.0e5), vapour `rho.b = 0.7462` against the cell's `0.5330`, `rho_old_b = 0.7462`. Upstream: `p_` is
`READ_IF_PRESENT` from `0/fluidRegion/p`, whose inlet patch is `calculated; value uniform 1e5`, and the thermo packages are
constructed from it, so the vapour patch density is `rho(1e5, 652.15) = 0.5330` (equal to the cell) at construction, and the
patch of `p` becomes `p_rgh + rho gh = 1.4e5` only at the first `p_ = max(p_rgh_ + rho_*gh_, pMin_)` (`updatePP_rghRho_2p.H`, after the first
`p_rgh` solve). The port set `p`'s patches to `p_rgh`'s (`case.rs`) and again to `p_rgh + rho gh` after the thermo construction
(`TwoPhaseSolver::new`), so its construction-time patch densities, and the old-time copies of them that `oldTime()` stores
lazily, belong to 1.4e5.

| id | prediction (committed before the fix) |
|---|---|
| U1 | `p`'s patch values are read from the case file (`calculated` + `value`) and NOT overwritten at construction; the first update is the existing one after the first `p_rgh` solve. `U.vapour` cell 0 at (1 outer, 1 corrector) moves from 2.07e-6 to below 1e-10, and the whole `U.vapour` field of steps 1 and 2 at the 24-outer-iteration protocol moves from 2.1e-5 and 5.7e-6 to below 1e-9 |
| U2 | the other fields at steps 1 to 65 move by less than 1e-11 relative (they are insensitive to an absent phase's inlet density), so no recorded bound of the first-65-step gate needs touching; the `U.vapour` bound of steps 1-2 then tightens to the common one (the bound is tightened, never relaxed) |
| U3 | onset window (steps 616-627) and the nOuter-6 control (item C2) keep passing at their bounds; the cumulative continuity errors of the control move by less than 1e-6 relative |
| U4 | the old form stays callable: `TwoPhaseCase::without_p_patch_from_file()` reproduces 2.07e-6 / 2.1e-5 / 5.7e-6 |


## Round 5 results (measured 2026-11-13, release; tests in `tests/genfoam_two_phase_vs_upstream.rs`)

Upstream reference runs (real binary, serial, tight protocol unless said): 1D_boiling to 12 s: 6157 steps, 254 s wall; its log gives, per step,
the iterations run and how the loop ended. Endings over the 6157 steps: conv6 952, conv10 811, conv11 799, conv12 739, conv9 550, conv13 474,
notconv 421, conv14 272, conv15 220, conv16 188, conv8 176, conv17 166, conv18 80, conv19 62, conv5 55, conv7 50, conv20 46, conv21 34, conv23 23,
conv22 19. **Skipped steps (`converged in 0 iterations`): 8, at steps 1811, 1813, 3005, 3678, 5648, 5650, 5652, 6012** (t = 6.92 s to 10.55 s), each
preceded by a step whose criteria were first met in the 24th iteration. At the shipped tolerances (4401 steps): 8 skips at 729, 731, 733, 753, 1140, 1150, 1322, 1969.

### Items 1 and 3 (control flow, `calcCumulContErrs`): the predictions

| id | held? | measured |
|---|---|---|
| F1 | **held, and by more than predicted** | with the upstream option, the port's iteration count and ending equal upstream's at **every one of the first ~~723~~ 758 steps** (**CORRECTED 2026-11-13**: 723 was the value before commit 9db557a69, see "Round 5 addendum"; re-measured: 758 steps equal, 129 of them `notconv` in both). The informative part is steps 1-636 (iteration counts 5 to 23 and 24, many different endings). In steps 637-~~723~~ 758, where the two trajectories have already separated, all ~~87~~ 122 steps are `notconv` in both codes, so equality there is weak evidence. First difference: step ~~724~~ **759** (`tests: one_d_boiling_pimple_flow_tight_matches_upstream_until_the_trajectories_split`). Before the item-4 fix the first difference was step 752 |
| F2 | **MISSED in size** | the criteria residuals do not agree to 1e-6: over steps 1-617 the worst relative differences of entries above 1e-8 are 1.7e-4 (`p_rgh` first-PISO) and 3.7e-4 (`h.liquid`); by magnitude (`p_rgh`): 1.1e-5 at 1e-6..1e-5, 1.3e-5 at 1e-4..1e-3, 1.7e-4 at 1e-7..1e-6. Cause: the OpenFOAM-normalised initial residual divides by a norm built from `A x - A xRef` that is small for a near-uniform field, so field differences of 1e-11 (the linear-solver stopping difference, GAMG against the port's ILU-BiCGStab at 1e-14) show up at 1e-5..1e-4 in a residual of 1e-6..1e-4. No decision flipped: all 723 endings are equal |
| F3 | **held in kind, missed in detail** | the first differing step (~~724~~ **759**, see the addendum) is after the onset (618). The deciding residual is NOT within a factor 2 of 1e-5 in both codes: ~~at step 724 upstream's last printed `p_rgh` residual is 2.2e-4 and the port met the criteria~~ **CORRECTED 2026-11-13** for the current code: at step 759 upstream meets the criteria at the start of iteration 23 (`conv23`), the port's last printed `p_rgh` residual is 2.73e-4 and it runs all 24 iterations (the 724 figures, upstream 2.2e-4 with the port meeting the criteria, belong to the code before 9db557a69; upstream's own residual at 759 is Not re-checked: the committed flow reference holds iterations and endings only and the upstream log was not kept). The reason is that the trajectories had already separated (~~residual lists first differ by 17 % at step 637~~ Not re-checked after the change, for the same reason; the clocks ~~by 0.03 s at step 700: upstream 6.28394, port 6.33092~~ **0.054 s at step 700: upstream 6.28394, port 6.33825** (the old pair differs by 0.047 s, not the 0.03 s the text said)), so the two codes are no longer at the same state, which is what the noise-floor section of round 3 predicts |
| F4 | **held, for the pre-fix run** | step 751 of the first run (before the item-4 fix; after it the corresponding steps were 723/724, and are none after 9db557a69: the first port step flagged in its last iteration is then 1556): upstream's last `p_rgh` residual 1.5264e-5, the port's 9.737e-6, both within a factor 2 of 1e-5 |
| F5 | **held** | no difference in the loop's control flow was found; the option "moving the run away from upstream" in round 4 was a trajectory effect (see "Moved numbers") |
| C1 | **held** | `nOuterCorrectors 6`, tight, 115 steps (to t = 1 s): upstream skips **55 steps** (every second step from step 4), the port skips the same 55, every iteration count and ending equal |
| C2 | **held after item 4, missed before it** | fields per step, port vs upstream, 115 steps including the 55 skipped ones, worst relative difference: `p` 4.2e-12 (step 1; 9.8e-13 on a skipped step), `p_rgh` 4.2e-12, `T.liquid` 2.5e-13, `T.fixedPower` 1.4e-13, `U.liquid` 9.3e-12, `T.vapour` 1.3e-10, `T.interface` 4.5e-10, `U.vapour` 2.5e-11, `alpha.vapour` and `dmdt` exactly 0 in both. The first run of this test failed at step 1 on `U.vapour` (8.9e-5, bound 5e-5): that was item 4. No growth across skipped steps |
| C3 | **held** | cumulative continuity error liquid: port -5.040486e-5, upstream -5.040441e-5 kg/m3 (relative 9e-6, absolute 4.5e-10, the onePhase size); vapour 0 in both |
| extra | held | `nOuterCorrectors 7`, 3 s, 315 steps (309 `conv6`, 2 skipped, 4 `notconv`): iteration count and ending identical at every step |

**What the port's loop was and is.** `step()` was already the same call sequence as `customPimpleControl::loop()` (read in full above), so no control-flow
change was needed for item 1. The one translation defect in this part of the code was item 3. Not ported, recorded (none occurs in this case): the
`*Final` variants of `fvSolution` entries (`p_rghFinal`, `h.vapourFinal`, `T.interfaceFinal`; upstream sets `finalIteration` only in the flagged
iteration, so for a case whose Final settings differ the last unflagged iteration would use the non-Final ones), `useFirstPISOInitialResidual false`
(the port always uses the first-PISO residual for `p_rgh`) and `stopIfFirstResRelChangeAboveRelTol`. 1D_boiling's `p_rgh`/`p_rghFinal`, `h.vapour`/`h.vapourFinal` and
`T.interface`/`T.interfaceFinal` entries are pairwise equal.

### Item 3: `calcCumulContErrs`

Now called in the iteration flagged converged (`if (pimple_.finalIter())` is `converged_` only, see above), as for onePhase; a step that never meets its
criteria and a skipped step add nothing. It applies to both modes (it is upstream's behaviour, not a defect of the flag; the diagnostic does not feed the physics).
`StepReport::converged_at` and `TwoPhaseSolver::pimple_trace` record where it happened. **Decision deferred to the maintainer:** whether the corrected default
should also accumulate in every step (the evident intent of `finalIter()`), which would make the number independent of the flag bug; it is a diagnostic
only and I kept upstream's behaviour in both modes, as round 4 did for onePhase.

### Item 2: the shipped-tolerance "stall"

Reproduced with the round-4 code (earlier `p` patch, the upstream option; `GH1361_ABL_PPATCH=1`): the run reaches step 5035 at t = 9.949786 s with `dt = 1.9e-3`,
then `dt` is 2.4e-7 at step 5036 (a factor 8000 down in one step, so the Courant number of step 5035 was about 8000 times the controller's target), and from step 5037 `dt` is below the resolution of t (9.95 has ulp 1.8e-15): the clock stops,
`maxDeltaT` can only grow a zero step by a factor of zero, and the loop runs for ever (13804 steps and counting after 400 s, all at t = 9.949786127634228).
It is **a collapsing time step in the port's own post-onset trajectory, not a stalled solve and not the loop**; the last reference time written was t = 9.5 s, as
round 4 saw. It does not occur with the corrected `p` patch (item 4): the run with the option completes, ~~5292 steps (upstream 4401), 6 skipped steps (1716, 1779,
2438, 4727, 4787, 4833; upstream 729, 731, 733, 753, 1140, 1150, 1322, 1969)~~ **CORRECTED 2026-11-13** (after 9db557a69, see the addendum): 5473 steps (upstream 4401; the port's skipped steps are Not re-checked; upstream's are 729, 731, 733, 753, 1140, 1150, 1322, 1969), 6.8 s of CPU (not re-timed). A zero or collapsed step is now an error
(`AppBuilderError::Diverged`, `one_d_boiling_a_collapsed_time_step_is_an_error`); upstream has no such guard and would spin the same way from such a state.
The first count difference at the shipped tolerances is step 729 (upstream skips, the port had not been flagged at 728); the loose solvers have already moved the
residual lists by 1 % at step 282.

### Item 4: the `U.vapour` residual, first divergent operation

Localised by `nOuterCorrectors 1`, `nCorrectors 1` (above): cell 0 only. Cause, read in `thermalHydraulicsModel.C` (`p_` is `READ_IF_PRESENT` from
`0/fluidRegion/p`, whose inlet patch is `calculated; value uniform 1e5`) and `pEqnf_2p.H`/`updatePP_rghRho_2p.H` (`p_` patches are assigned only after the first `p_rgh` solve):
the port set the patches of `p` to `p_rgh`'s 1.4e5 at read time and again to `p_rgh + rho gh` after the thermo construction, so the vapour patch density (and its `oldTime()`
copy) was 0.7462 instead of 0.5330. Fix: `TwoPhaseCase::read` takes the patch values from the file (`p_patch_from_file`, default `true`), the construction-time reset
is kept only for the ablation `TwoPhaseCase::without_p_patch_from_file()`. Not a kovan-reviewed function.

| quantity | before | after | prediction |
|---|---|---|---|
| `U.vapour`, cell 0, 1 outer iteration, 1 corrector | 2.07e-6 | 9.3e-12 | U1 held (below 1e-10) |
| `U.vapour`, step 1 / step 2, 24 outer iterations (full protocol) | 2.06e-5 / 5.66e-6 | 1.35e-11 / 1.96e-11 | U1 held (below 1e-9) |
| `U.vapour`, cell 0, 2 outer iterations, 1 corrector | 8.2e-5 | 1.0e-13 | |
| other fields, steps 1-65 | within the round-3 bounds (`p`, `p_rgh` 1e-11, `T.liquid` 1e-12, `U.liquid` 1e-10, `T.vapour` 1e-9, `T.interface` 5e-9) | within the same bounds (e.g. step 1: `p` 4.0e-14, `T.liquid` 5.2e-16, `T.vapour` 1.2e-12, `T.interface` 5.5e-12; step 65: `p` 9.6e-13, `T.liquid` 2.5e-13) | U2 held: the gate passes at its unchanged bounds, which was the prediction; the per-step maxima were not re-tabulated |
| onset window, steps 616-627 | `alpha.vapour` 3.4e-6, `dmdt` 3.1e-6 | 3.42e-6, 3.08e-6 | U3 held: bounds unchanged |
| ablation `without_p_patch_from_file()` | | 2.06e-5, 5.66e-6 | U4 held (`one_d_boiling_p_patch_ablation_reproduces_the_recorded_u_vapour_miss`) |

The gate bound for steps 1-2 of `one_d_boiling_first_steps_tight_vs_upstream` was tightened from 5e-5 (`U.vapour`) to the common 1e-10; no bound was relaxed.

### Moved numbers (old to new, with cause)

| number | old | new | cause |
|---|---|---|---|
| full tight run, port steps, default ending | 7454 | 6548 | item 4 (trajectory) |
| same, upstream option (upstream: 6157) | 5569 (diff 588) | 6555 (diff 398) | item 4 |
| steps flagged in the last iteration, option | after 751, 1766, 2033, 2969 | 16 steps (723, 725, 773, 1146, 1392, 1394, 1396, 1398, 1861, 1867, 3202, 3772, 4326, 6160, 6364, 6413); upstream skips 8 | item 4; chaotic after the onset |
| cumulative continuity error, tight, option (liquid, vapour, kg/m3; upstream -2.296, 1.155e-3) | (-7.521, 5.86e-3) | (-1.821, 8.15e-4) | item 3 and item 4 together (not separated) |
| same, default ending | (-4.407, 3.89e-5) | (-1.446, 6.93e-4) | same |
| cumulative continuity error, shipped (upstream -0.4466, 2.00e-4) | default (round 3) -1.860, 8.51e-4 | option (new) -0.5570, 2.77e-4 | same; the old figure is the default run's, the new one the option's: not the same run |
| t = 12 s fields, option, worst relative: `p`, `T.liquid`, `T.fixedPower`, `U.liquid` | 4.8e-4, 1.5e-2, 1.6e-2, 3.2e-3 | 6.1e-4, 2.1e-2, 2.2e-2, 4.4e-3 | post-onset sensitivity |
| same, default ending | 1.7e-5, 4.9e-4, 4.6e-4, 1.1e-4 | 4.5e-4, 1.5e-2, 1.6e-2, 3.3e-3 | post-onset sensitivity |
| shipped-tolerance run with the option | stalled at step ~5036, t = 9.9498 s | completes, 5292 steps | item 4, plus the new guard |
| first step whose PIMPLE iteration count or ending differs from upstream, tight, option | 724 | **759** | 9db557a69, perfectGas `psi = 1/(R T)` (addendum) |
| full tight run, port steps, option (upstream: 6157) | 6555 | 5567 | same |
| full tight run, port steps, default ending | 6548 | 5608 | same |
| steps flagged in the last iteration, option | 16 steps (723, 725, 773, ...) | 10 steps (1556, 1756, 1931, 2416, 5037, 5039, 5041, 5167, 5272, 5339) | same |
| same, default ending | | 9 steps (1556, 5049, 5050, 5051, 5219, 5279, 5337, 5339, 5418) | same |
| cumulative continuity error, tight, option (upstream -2.296, 1.155e-3) | (-1.821, 8.15e-4) | (-1.449, 7.49e-4) | same |
| same, default ending | (-1.446, 6.93e-4) | (-1.599, 7.02e-4) | same |
| t = 12 s fields, option, worst relative: `p`, `p_rgh`, `T.liquid`, `T.fixedPower`, `U.liquid` | 6.1e-4, 7.8e-5, 2.1e-2, 2.2e-2, 4.4e-3 | 2.3e-4, 3.8e-5, 7.1e-3, 7.5e-3, 1.5e-3 | same; post-onset sensitivity, not evidence of closeness |
| t = 12 s fields, default ending, `p`, `p_rgh`, `T.liquid`, `T.fixedPower`, `U.liquid` | 4.5e-4, (not recorded), 1.5e-2, 1.6e-2, 3.3e-3 | 2.8e-4, 5.0e-5, 8.5e-3, 9.0e-3, 1.8e-3 | same |
| shipped-tolerance run with the option, port steps (upstream 4401), cumulative continuity error liquid, vapour (upstream -0.4466, 2.00e-4) | 5292, (-0.5570, 2.77e-4) | 5473, (-1.432, 7.74e-4) | same; the first port step flagged in its last iteration is now 763 |

**On the 12 s numbers.** The default ending was the closer one to upstream at t = 12 s before this round (`p` 1.7e-5), and is 4.5e-4 now; the option's closeness flipped
the other way. A fix confined to two steps at the inlet moved every post-onset number by up to two orders of magnitude, which is what the round-3 noise floor says: **no post-onset
figure here is evidence about the flag or about the loop**, in either direction. The gated and quotable comparisons are the pre-onset rows, the onset window, and the
control-flow comparisons above (steps 1-~~723~~ 758 identical; the `nOuterCorrectors 6` and `7` controls on single-phase flow, where the trajectories do not separate).

### What remains (round 5)

* The post-onset trajectory cannot be reproduced point-wise (round 3); the control-flow claim is therefore bounded by step ~~723~~ 758 (tight) and by the controls.
* `*Final` fvSolution variants, `useFirstPISOInitialResidual false` and `stopIfFirstResRelChangeAboveRelTol` are not read (none is exercised by 1D_boiling).
* Decision for the maintainer: should the corrected-ending default also make `calcCumulContErrs` run in every step? (see Item 3).
* The cumulative continuity error is still a factor 1.25 (shipped) to 1.3 (tight) from upstream's after the onset: it integrates a cancelling quantity along a chaotic trajectory, so it is not expected to close.

# Round 5 addendum (gh:#1361, #1362; 2026-11-13): why the first differing PIMPLE step moved from 724 to 759

AI-written, not human-reviewed. Research, education and V&V only. The test `one_d_boiling_pimple_flow_tight_matches_upstream_until_the_trajectories_split`
found 759 after commit 9db557a69 (gh:#1362 row 4, thermophysics against ESI v2506); its parent 5e42d5727 gives 724 (bisected by the coordinator).

## Predictions (written BEFORE any run of this addendum)

Read first (no run): the five changes of 9db557a69 against the code twoPhase calls (`solver/two_phase/thermo.rs`: vapour `PerfectGas` + `HConstThermo` +
`ConstTransport`, liquid `IcoPolynomial` + `HPolynomialThermo` (Foundation form, `new`) + `PolynomialTransport`; it uses `rho`, `psi`, `cp`, `mu`, `kappa`, `hs`, `T(hs)`).

| id | prediction | reason |
|---|---|---|
| M1 | the only change that reaches the solver is **perfectGas `psi = 1/(R T)`** (vapour). Reverting it alone restores 724; reverting any other piece alone leaves 759 | `P_REF` is read only by entropy (`S`), which twoPhase never calls; `alphah` is never called (twoPhase builds `kappa = Cp mu rPr` itself through `kappa`, unchanged); PengRobinson and `rhoConst` are not selected by any two-phase tutorial; `GasConstant` defaults to CODATA, so `r()` is unchanged; `hPolynomial` ESI form is opt-in (`new_esi_v2506`), the solver uses `new` |
| M2 | the change in `psi` is at most 1 ulp (new `1/(R T)` against old `(p/(R T))/p`), so it is a move toward upstream's operations (v2506 `perfectGasI.H`) but of size ~1e-16 | arithmetic |
| M3 | the field differences against upstream on steps 1-65 (<= 1e-9 gate) and in the onset window do not change measurably (all change by well under the 1e-11 linear-solver noise already there); they neither shrink nor grow beyond that | the dominant difference is the linear-solver stopping (GAMG against ILU-BiCGStab at 1e-14), 5 orders above 1 ulp |
| M4 | therefore the step 724 -> 759 is not a measurable change in distance from upstream: after the onset round 3 showed 1e-6 perturbations growing to O(1) within 0.5 s, so a 1e-16 perturbation re-draws the threshold-crossing step. The change is correct in form (bit-identical operation to upstream) and the new step is a different draw of the same chaotic noise | round 3 noise floor |

## Round 5 addendum results (measured 2026-11-13, release; scratch builds that edited one function of `outram-foam-basic-lib` at a time and were reverted, nothing of them is committed)

Command for every row: the four tests `one_d_boiling_pimple_flow_tight_matches_upstream_until_the_trajectories_split`, `one_d_boiling_first_steps_tight_vs_upstream`,
`one_d_boiling_boiling_onset_tight_vs_upstream`, `one_d_boiling_full_tight_vs_upstream`, `--nocapture`.

| scratch build | first differing PIMPLE step |
|---|---|
| HEAD (all of 9db557a69) | 759 |
| HEAD with only `PerfectGas::psi` back to `self.rho(p, t) / p` | **724** (parent's value) |
| HEAD with only `P_REF` back to 101 325 Pa | 759 |
| HEAD with only `ConstTransport::alpha_h` back to `kappa / Cp` | 759 |

| id | held? | measured |
|---|---|---|
| M1 | **held** | the single piece is `PerfectGas::psi`. The others cannot reach twoPhase: no call of `s()`/`S` (P_REF), of `alpha_h` (the solver uses `kappa` from `props`), no PengRobinson or `rhoConst` in `thermo.rs`, `PolynomialStack` is built with `HPolynomialThermo::new` (the Foundation form, unchanged), `GasConstant` defaults to CODATA (unchanged). Only P_REF and `alpha_h` were run alone; the other three are by reading |
| M2 | **held** | `1/(R T)` is exactly v2506 `perfectGasI.H`; the 9db557a69 dump test found the old form 1 ulp off upstream at 27 of 40 grid points |
| M3 | **held, stronger than predicted** | steps 1-65 (10 fields, 65 steps), the 12 onset-window steps (616-627) and all 12 comparisons at 0.5 s to 6.0 s: every printed value (3 digits) is IDENTICAL before and after. Worst relative differences are the recorded ones (steps 1-65: `p` 9.55e-13, `T.interface` 7.59e-10; onset window: `alpha.vapour` 3.42e-6, `U.vapour` 1.86e-8). The reason is that `psi` of the vapour is multiplied by `alpha.vapour`, which is 0 until step 617, so nothing before the onset can see it. The field differences neither shrank nor grew where they can be measured |
| M4 | **held** | from 6.5 s on, whole fields differ O(1) between the two builds (`alpha.vapour` against upstream at 6.5 s: 7.9e-2 new, 6.9e-1 old; at 7.0 s 2.1e-1 new, 1.5e-2 old; at 9.0 s 1.0e-1 new, 5.4e-2 old; at 12 s `p` 2.3e-4 new, 6.1e-4 old, `T.liquid` 7.1e-3 new, 2.1e-2 old), with no consistent sign per time or field: noise, not a trend. The port had 900 steps at t = 6.5 s new against 1406 old |

**Decision: the change is a correct move toward upstream in form, and cannot be shown to be one in distance.** Form: the port now performs upstream's operation bit for bit. Distance: unmeasurable before the onset (identical), noise after it. The pin is moved to 759 with this cause. No tolerance was relaxed.

**Claims re-checked.** Re-measured and corrected above: "identical through step 723" (now 758), "all 87 steps are `notconv`" (122 over 637-758), the clock difference at step 700 (0.054 s; the old text said 0.03 s while its own numbers gave 0.047 s), the flagged steps, port step counts, cumulative continuity errors and 12 s fields in "Moved numbers". Not re-checked: that the residual lists first differ by 17 % at step 637 and upstream's last residual at step 759 (needs upstream's log; the committed `boiling_tight_full_flow` reference holds iterations and endings only); the shipped-tolerance first count difference at step 729 (the port's first flagged step is now 763, so it is probably unchanged, but the steps before were not compared).

### A finding about the upstream reference (not caused by this change)

A fresh run of the upstream binary in this environment (`/home/user/upstream-build`, GeN-Foam binary of 2026-11-13 01:17, OpenFOAM built 2026-11-12 23:39), tight protocol, `scripts/boiling_protocol.sh`, is repeatable (two runs identical) and agrees with the committed
`boiling_tight_full_flow` reference at steps 1-639 (iterations, endings, clocks). At step 640 it differs: dt 0.0101 s against the reference's 0.0008 s, and the run then aborts with SIGFPE (`Foam::divide`) at t = 6.35153, step 667, at both `endTime` 6.5 and 12. The committed reference run reached 12 s (6157 steps). So the reference of steps 640 onward was produced by a different build of upstream than the one now installed (the environment was rebuilt; the compiler or flags may differ), and it cannot be regenerated here. The comparison before the onset (steps 1-639) is unaffected; the steps 640-758 of this test's reference, and every post-onset reference number, are a record of one build's chaotic trajectory. Recorded on gh:#1361; not investigated further.

# Round 6 (gh:#1361 item "MSFR transient"; companion gh:#1475): `2D_MSFR` `transient` stage, first 50 steps

AI-written, not human-reviewed. Research, education and V&V only.

## What exists in the port, and what does not (read before any run, 2026-11-25)

Searched `src/genfoam` for `msfr`, `transient`, `pointKinetics`, `precursor`, `liquidFuel`, `moving`:

| piece the `transient` stage needs | upstream routine | port before this round |
|---|---|---|
| diffusion flux + precursor transient with circulating fuel | `diffusionNeutronics::correctPhysics`, `solveNeutronics.H`, `fluxEq.H`, `precEq.H` (`eigenvalueNeutronics false`, `liquidFuel true`) | `DiffusionNeutronics::step`: single pass, stationary-fuel precursors, `k_eff` fixed, no transport, no neutron iteration; **not a translation**. `precursor_drift` is the steady (`ddt` dropped) equation only. |
| TH region `onePhase`, `momentumMode cellCentered`, `solveFluidMechanics false`, `porousKEpsilon`, Boussinesq `heRhoThermo`, `fixedTemperature` powerModel, `ReynoldsPower` | `solvers/onePhase`, `EEqn_1p`, cellCentered `pEqn`, `libFFSSrc2` closures | `OnePhaseFcSolver` (round 4) is a translation of the **faceCentered** path only, without turbulence; `solver::one_phase::OnePhaseSolver` is a constant-property stand-in. The coupled loop (`multi_region`) runs the lumped `MeshThermalHydraulics` (#1475). |
| `meshToMesh cellVolumeWeight` between the two (identical) 8595-cell meshes | `meshHandler::mapTheseFields` | approximate (nearest cell + global rescale; #651) |
| `multiPhysicsSolver::correctPhysics` Picard loop | as named | `MultiPhysicsSolver` (generic, enum-dispatched), not wired to a case reader |
| adaptive `deltaT` (Courant from TH, `maxPowerVariation` from neutronics) | `setDeltaT.C`, `maxDeltaT` | `PseudoTimeClock`/`neutronics_max_delta_t` (neutronics half only) |

So the port **cannot run the coupled MSFR transient**: the TH half (cell-centred energy equation with frozen flow, porous k-epsilon, HX power model, `ReynoldsPower`) and the case-driven `multiPhysicsSolver` are not ported; they are filed as children of #1475. What this round does instead is the **neutronics half of the transient, driven one-way by upstream's own TH fields** (an operator-split comparison): upstream's mapped `TFuel`/`rhoCool` of every step enter the port's neutronics; the port's own flux, precursor and power history is compared with upstream's, step by step. That isolates the diffusion transient, the liquid-fuel precursor transport, the cross-section feedback and the albedo boundary from the TH.

## Upstream run (real unmodified `GeN-Foam` binary, built 2026-11-13, serial)

`scripts/run_msfr_chain.sh` replays `Allrun`'s `setSteadyStateTH`, `setSteadyStateEN`, `setTransient` with sed/perl (`foamDictionary` is not built here). Protocol: `writePrecision 17` everywhere, so the restart state is exact; steady stages with the **shipped** solver tolerances (the transient start state is an input of both codes, as in round 3); the `transient` stage at the **tight protocol** (every linear solver `tolerance 1e-14`, `relTol 0`), `writeControl timeStep; writeInterval 1`. The sed edits only plain `tolerance x;`/`relTol x;` lines: the first attempt also rewrote the lines of the `/* ... relTol 0.01;*/` comment block of `neutroRegion/fvSolution` and removed its `*/`, which made upstream abort with "Entry 'fluxStar0' not found" (a harness defect, fixed).

Measured before any port run: steadyStateTH 531 s (15 s of flow), steadyStateEN 71 s (`keff_` = 0.96031194083006322 at t = 165, `pTarget` 2e7; the tutorial's expected 0.960283 is 3.0 pcm away), transient 65 s for 142 steps to t = 166. Every step: `Multiphysics loop converged after 1 iterations`, so the neutronics of step n sees the TH state of the end of step n-1 (mapped once at the start of the step; the written `neutroRegion/<t>/TFuel`, `rhoCool` are exactly those). `U` is frozen (`fluidRegion/U` and `neutroRegion/U` identical in all steps), `fluxStar_i == flux_i` (discontinuity factors 1), `TFuel == TCool` bitwise, `rhoCool = 4125 (1 - 2e-4 (T - 900))` **bitwise** (Boussinesq). Per step: `deltaT` 0.001 then 8.1e-4, 9.8e-4, ... 7.47e-3 from step 15 (Courant-limited); the neutron iteration count is 50 (the cap) at steps 1-2, then 37, 1, 22, 1, 8 at steps 3-7, and 1 from step 8 on; the final `neutroResidual` is within 0.01 to 0.4 % of the 1e-6 stopping value at steps 2-14 (9.9967e-7 at step 3), so the iteration count is a discrete decision taken close to a threshold. **Aitken acceleration was refused at all 157 attempts and performed 0 times** (upstream declares the three integral-flux `List<scalar>` inside the loop, uninitialised: the test reads indeterminate memory). `power:` goes from 2.0000000e7 to 20003657.5 in step 1 and to 20013566 at step 50.

## Predictions (written BEFORE the first port run; committed first)

Inputs of the port: committed `cases/msfr_neutro` (mesh, `nuclearData`), `refs/msfr_transient.ref.txt.gz` (start state, `T` of each step, upstream's results). Port solvers at `1e-14`.

| id | quantity | prediction |
|---|---|---|
| Q1 | per-step `power`, relative difference port - upstream, all 50 steps | `<= 1e-9`; if not, the first step above 1e-9 and its cause are the finding. Round 1 gave +0.3 pcm in `k_eff` through the same cross-section path at 6-digit inputs, so an offset of order 1e-6 from the cross sections (RBF/log law) or the albedo boundary is the alternative; I put the odds near even |
| Q2 | neutron iteration count per step | equal at >= 45 of 50 steps; the steps 2-14 whose final residual is within 0.4 % of 1e-6 are the ones expected to differ if the port's initial-residual normalisation differs from OpenFOAM's by more than that |
| Q3 | precursor totals `sum V prec_k` per group, per step | `<= 1e-9` relative, failing no earlier than Q1 |
| Q4 | per-cell flux and `prec` at step 50 | worst cell `<= 1e-8` of the field maximum |
| Q5 | power change since the start state (`power(50) - 2e7`, +1.3566e4 W upstream) | port reproduces it to 1e-6 relative of the change (the change is a 6.8e-4 effect, large against solver noise) |
| Q6 | first quantity to diverge | the neutron iteration count at one of the near-threshold steps (3-14), not the power |
| Q7 | wall time | port < 5 min for 50 steps |

## Round 6, first run (2026-11-25, port with the data-driven chi): predictions Q1-Q7 mostly missed

Result of the first run (`msfr_transient_neutronics_matches_upstream_over_50_steps`, 12.6 s wall): power relative difference 7.4e-6 at step 1, 6.7e-5 at step 2, 1.8e-4 from step 3 to step 50 (worst 1.84e-4); neutron iteration counts equal at 43 of 50 steps (steps 2-8 differ: 33/50, 1/37, 21/1, 1/22, 10/1, 1/8, 2/1); precursor totals up to 1.07e-4, flux integrals 1.9e-4; step-50 per-cell 1.9e-4; power change since 2e7 at step 50: port +1.0045e4 W, upstream +1.3566e4 W (26 % off). **Q1 missed, Q2 missed narrowly (43 of 50 against >= 45), Q3 missed, Q4 missed, Q5 missed, Q6 missed in kind (the power diverges at step 1), Q7 held.**

How the cause was found (all diagnostics are `#[ignore]`d tests in the same file; each reads the upstream run):

1. Step 1, first iteration, per-group first-solve initial residual: port `[7.35e-6, 7.83e-6, 2.51e-6, 1.94e-6, 1.56e-6, 2.05e-6]`, upstream `[9.6e-7, 1.14e-6, 1.57e-6, 1.84e-6, 1.66e-6, 1.83e-6]`: the start state (upstream's own) is out of balance in the port by 7x in the fast groups.
2. The static eigenvalue of the port at upstream's t = 165 s state is +0.315 pcm from upstream's `keff_` (same as round 1's +0.3 pcm, never explained there); upstream's `keff_` is itself still creeping by 5e-8 per step at t = 165 s, so this alone is not a diagnosis.
3. Geometry: `upstream_driver/dump_laplacian_geometry` (ESI `V`, `magSf`, linear weights, `nonOrthDeltaCoeffs`, `deltaCoeffs`, every patch): port equal to <= 3.6e-14 relative everywhere. Albedo coefficients: scaling any of the four patches by 1 +- 1e-3 raises the residual, minimum at 1.0. Not the cause.
4. `upstream_driver/dump_xs` calls GeN-Foam's **own `XS` class** (`xs.correct()` on the same case) and dumps every cross section per cell at 17 digits. Port against it, 8595 cells: `D`, `nuSigmaEff`, `sigmaPow`, `sigmaRemoval`, `IV`, `BetaTot` **bit-identical (0 of 8595 differ)**, `sigmaFromTo` identical except 1 cell at 1.1e-15; **`chiPrompt` and `chiDelayed` differ in all 8595 cells** (worst 3.3e-5 `chiPrompt0`, 7.7e-2 `chiDelayed0`, 0.34 `chiDelayed5`).
5. Upstream routine: `XS::init()` (`XS.C:297`) calls `.build()` on the `D`, `nuSigmaEff`, `sigmaPow`, `sigmaRemoval` and `sigmaFromTo` lists and **not** on `chiPromptList_`/`chiDelayedList_`; `nuclearDataOneEnergy::get` (`nuclearDataOneEnergy.C:94`) returns the reference value when `reducedParamIdx_` is empty (never built). So upstream's chi is the reference chi in every cell, though `nuclearData` supplies a chi per state. The port built and interpolated them. A port difference of the kind CLAUDE.md names ("what upstream does that we do not"), found by reading upstream after the measurement.

## Round 6, second run: hypothesis and predictions (written BEFORE the second run; committed first)

Change: `CrossSectionData::with_upstream_unparametrised_chi()` (new option, ~~**default unchanged: parametrised**~~ **the default since 2026-11-26**), used by this test's setup. ~~Whether upstream's omission should become the default is **deferred to the maintainer**~~ **DECIDED 2026-11-26** (maintainer: "follow upstream", "deprecate ours locally"): upstream's form is the default; the data-driven chi is `CrossSectionData::with_data_driven_chi()`, `#[deprecated]`.

| id | prediction |
|---|---|
| H1 | with the option, `diagnose_cross_sections_against_upstream_xs_class` shows every cross-section quantity equal to upstream's at <= 1e-14 relative (chi 0 cells differ) |
| H2 | the step-1 first-iteration group residuals fall to within 5 % of upstream's `[9.6e-7, 1.14e-6, 1.57e-6, 1.84e-6, 1.66e-6, 1.83e-6]` (they differ only by the precursor re-solve, which is the same in both) |
| H3 | per-step power relative difference <= 1e-8 at all 50 steps (solver differences, PCG/DIC vs basic-lib CG, at 1e-14); bit-identity is not expected |
| H4 | neutron iteration count equal at >= 48 of 50 steps (the near-threshold steps 3-8 are the risk; the first-iteration residual of step 4 is 9.9756e-7, 0.24 % under the threshold) |
| H5 | precursor totals and flux integrals <= 1e-8 per step; step-50 per-cell <= 1e-8 of the field maximum |
| H6 | power change since 2e7 at step 50 reproduced to 1e-5 relative |

## Round 6 results (measured 2026-11-25, release; `tests/genfoam_msfr_transient_vs_upstream.rs`)

Command: `cargo test --release -p outram-foam-appbuilder-lib --test genfoam_msfr_transient_vs_upstream -- --nocapture`. Upstream: the 50 steps of `2D_MSFR` `transient`, tight protocol, real binary. Port: `DiffusionNeutronics::upstream_transient_step` (new, translation of `solveNeutronics.H`), `CrossSectionData::with_upstream_unparametrised_chi()`, driven one-way by upstream's own per-step `TFuel`/`rhoCool`; the port advances its own flux and precursors from upstream's t = 165 s state.

### The comparison (second run; chi at the reference state as upstream has it)

| quantity (per step, 50 steps) | worst relative difference | where |
|---|---|---|
| `power:` (17 digits) | **6.2e-14** (step 50); 1.5e-14 at step 1 | no step above 1e-13 |
| neutron iteration count | **equal at 50 of 50 steps** (50, 50, 37, 1, 22, 1, 8, then 1) | includes the near-threshold steps 3-8 |
| per-iteration `neutroResidual` (steps 1-3, 8 iterations) | agree to 9-10 digits (1.8360963420e-6 against 1.8360963422e-6) | |
| per-group flux integral `sum V flux_g` | 6.3e-14 (step 50) | |
| per-group precursor total `sum V prec_k` | **1.3e-11** (step 50), 5e-14 at step 1; grows steadily | groups 5-7 (the long-lived ones); -6e-13 (group 0) to +1.3e-11 (group 6) |
| step 50, flux per cell, relative to the field maximum | 1.9e-13 | |
| step 50, `prec` per cell, relative to the field maximum | 3.5e-10 | |
| `power(50) - 2e7`: port 1.356621e4 W, upstream 1.356621e4 W | 9.1e-11 | |
| wall time, 50 steps (8595 cells, 6 groups, 8 precursor groups) | 16.0 s | |

**No step diverges**: there is no first diverging step. The round-off allowance (1e-9) is used by the precursor totals and the per-cell precursors only (1.3e-11 and 3.5e-10); flagged for the maintainer per CLAUDE.md. Cause, stated as a hypothesis, not tested: the precursor equation is solved by basic-lib's BiCGStab where upstream uses `PBiCG`/`DILU`, both stopped at a normalised residual of 1e-14; the signed difference grows with the group index (longer half-life, more transport, worse conditioned), which is the signature of a convergence-limited linear solve and not of a different equation. The flux, whose solver is the same family (CG), agrees to 6e-14. Bit-identity was not reached; it would need upstream's `PBiCG` with `DILU`.

### Predictions

| id | held? | measured |
|---|---|---|
| Q1 (first run) power <= 1e-9 | **missed** | 7.4e-6 at step 1, 1.84e-4 worst: the data-driven chi |
| Q2 (first run) iteration counts equal at >= 45 | **missed** (43) | |
| Q3 (first run) precursor totals <= 1e-9 | **missed** | 1.07e-4 |
| Q4 (first run) per cell <= 1e-8 | **missed** | 1.9e-4 |
| Q5 (first run) power change to 1e-6 | **missed** | port +1.0045e4 W, upstream +1.3566e4 W |
| Q6 (first run) the iteration count diverges first | **missed** | the power diverged at step 1 (7.4e-6) and the counts at step 2 |
| Q7 wall < 5 min | **held** | 12.6 s then 16.0 s |
| H1 every cross section equal to upstream's at <= 1e-14 | **held** | `D`, `nuSigmaEff`, `sigmaPow`, `sigmaRemoval`, `IV`, `BetaTot` and `chi` (with the option): 0 of 8595 cells differ; `sigmaFromTo` and `sigmaRemoval5`: 1 cell at 1.1e-15 |
| H2 first-iteration group residuals within 5 % of upstream's | **held, much better** | `[9.60032217e-7, 1.13688670e-6, 1.57305138e-6, 1.83609634e-6, 1.66103678e-6, 1.83135442e-6]` against upstream's `[9.6003221720e-7, 1.1368866958e-6, 1.5730513809e-6, 1.8360963422e-6, 1.6610367771e-6, 1.8313544219e-6]`: 9 digits |
| H3 power <= 1e-8 | **held** (6.2e-14) | 5 orders under the prediction |
| H4 iteration counts equal at >= 48 | **held** (50) | |
| H5 totals and per cell <= 1e-8 | **held** | 1.3e-11 and 3.5e-10 |
| H6 power change to 1e-5 | **held** (9.1e-11) | |

### Defects found and fixed (by reading upstream after the measurement pointed at it)

1. **chi interpolation** (above). `XS::init()` never `build()`s `chiPromptList_`/`chiDelayedList_`; `nuclearDataOneEnergy::get` then returns the reference value. The port parametrised chi with the other cross sections. Added `CrossSectionData::with_upstream_unparametrised_chi()` (the option); the default stays data-driven (the evaluation supplies a chi per state, and the maintainer's rule is that supplied physics is on by default). ~~**Deferred to the maintainer: should upstream's omission become the default?**~~ **Decided 2026-11-26: yes** (see below). Effect of the omission on this case (the ablation, pinned in `msfr_transient_with_data_driven_chi_misses_by_the_pinned_amount`): 1.8e-4 in power and 43/50 iteration counts. It acts through the prompt/delayed split, a 3e-5 to 0.34 relative change of chi in the hot cells.
2. The cell-wise cross sections themselves (all other quantities) were already bit-identical: round 1's per-cell cross-section path is verified at this level for the first time.

### Not defects, recorded because they will mislead

* **The "+0.3 pcm" of round 1 (and `genfoam_tutorial_keff`) was never a sub-pcm measurement.** The port's static eigenvalue at upstream's t = 165 s state is +0.333 pcm from `keff_` with the unparametrised chi too (the chi option does not move it), and the transient, which uses the same operator, agrees to 1e-14: upstream's `keff_` is still creeping (5.0e-8 per 0.25 s step at t = 165 s; it was 0.96175 at t = 47.75 s, 0.9603119 at 165 s, and the tutorial's own expected value is 0.960283) so it is not the eigenvalue of its own operator at any finite time. A static `k_eff` against a pseudo-time `keff_` resolves the creep, not the operator. The check that does resolve the operator is the residual of the same equations (H2) and the transient (H3).
* **Aitken acceleration** is refused at all 157 attempts in the 143-step upstream run (and in the first 50 steps), i.e. never performed; the port does not implement it (module documentation of `upstream_transient`). The text of `fluxEq.H` reads three uninitialised `List<scalar>`; ~~deferred to the maintainer whether to port the evident intent (persisting lists), which would change upstream's observable behaviour.~~ **Decided 2026-11-26** ("follow upstream"): not ported; the port keeps upstream's observable behaviour (no acceleration).

### What the port still cannot do (coupled TH), filed

**CORRECTED 2026-11-26 (round 9): the TH half and the coupled loop now run in the port and agree with the real binary over the first 50 steps (see "Round 9 results" below); the paragraph that follows is the state at the end of round 6.** ~~The TH half of the stage was not run: `onePhase` with `momentumMode cellCentered`, `solveFluidMechanics false`, `porousKEpsilon`, Boussinesq `heRhoThermo`, `fixedTemperature` power model, `ReynoldsPower`, and `multiPhysicsSolver` with `meshToMesh cellVolumeWeight` (#1475, #651). The comparison above takes the TH state as input. Children filed on gh:#1475, see the issue comment.~~

### Gates

`cargo test --release -p outram-foam-appbuilder-lib --lib --tests` (2026-11-25): 579 passed, 0 failed, 25 ignored over 43 test binaries (lib 445 passed, 2 ignored); `cargo test --release -p petir --test no_platform_libm`: 4 passed. `cargo check --release -p outram-foam-appbuilder-lib --lib --no-default-features` (the `no_std` build): clean. Children filed on gh:#1475: #1773 (cell-centred energy-only onePhase), #1774 (case-driven multiPhysicsSolver, identical-mesh meshToMesh, coupled setDeltaT), #1775 (porousKEpsilon, alphat). No `exp`, `ln`, `pow` call was added: the new step uses only `+ - * /` and the existing `CrossSectionData` (which goes through PETIR `RealMath`).

### Moved numbers (old to new, with cause)

~~None. `CrossSectionData`'s default is unchanged (data-driven chi);~~ **2026-11-26: the default changed to upstream's chi; see "Default change, 2026-11-26" below.** Before it: `genfoam_tutorial_keff`, the guided-slab and SN comparisons are unchanged because the option is off in them.

### New upstream probes (committed under `upstream_driver/`, built with `wmake` against the built ESI v2506 and GeN-Foam libraries, objects not kept)

`dump_laplacian_geometry` (ESI `fvMesh` geometry per face and cell), `dump_xs` (GeN-Foam's own `XS` class: every cross section per cell at 17 digits). `dump_xs` is the oracle that localised the chi defect in one run after three hypotheses (albedo, geometry, keff scaling) had been eliminated by measurement; it is reusable for ESFR and any other `nuclearData`.


## Default change, 2026-11-26: upstream's unparametrised chi

Maintainer: "follow upstream", "deprecate ours locally". `CrossSectionData::from_input` now keeps `chiPrompt`/`chiDelayed` at the reference state (GeN-Foam 652b3da `XS::init()` never builds the chi interpolators); the port's earlier data-driven chi is `with_data_driven_chi()`, marked `#[deprecated]` and kept only as the pinned ablation (`msfr_transient_with_data_driven_chi_misses_by_the_pinned_amount`). Aitken acceleration: not ported, as upstream behaves.

**Re-measured (2026-11-26, release):** every GeN-Foam target. `--lib`: 445 passed, 2 ignored. Integration: common 5, gfhr 2, godiva_sn 0 (1 ignored), msfr_transient 2 (7 ignored), neutronics 3, one_phase 5 (2 ignored), point_kinetics 2, psbt 3, pseudo_time 3, sn 2, tutorial_keff 8, two_phase_1d_boiling 3, two_phase_vs_upstream 12 (3 ignored); 0 failed. `genfoam_tutorial_keff` was run **with `GENFOAM_UPSTREAM=/home/user/upstream-build/GeN-Foam-src`** under both defaults and its printed numbers are identical (ESFR reference state +871.1 pcm, MSFR converged state -1.1 pcm, MSFR reference states 0.973858 / 0.950415 / 1.075834): **no recorded number moved.**

**Finding:** without `GENFOAM_UPSTREAM` (or `/workspace/GeN-Foam`), all eight `genfoam_tutorial_keff` tests print `SKIP` and pass, so an ordinary `cargo test` run does not exercise them.




# Round 7 (gh:#1361, 2026-11-26): MULES, `1D_boiling` AS SHIPPED against the real binary

AI-written, not human-reviewed. Research, education and V&V only.

## What was searched before writing anything (CLAUDE.md "search before building")

- `grep -rniE "MULES|limitSum|explicitSolve|nAlphaSubCycles|interfaceCompression" crates/**/*.rs`: hits only in
  `outram-foam-multiphase/src/two_fluid_pimple_esi.rs` (`mules_lambda`, an ESI `MULES::limiter` written for one mesh
  family of the gh:#1363/#1367 cases: internal-face limiter with `Sp = Su = 0`, no `nLimiterIter`/`extremaCoeff`/
  `smoothLimiter`, no boundary handling, no convection scheme, no sub-cycling, on that crate's own field types), the
  "no MULES" notes of `pimple.rs`, `two_fluid.rs`, `drift_flux.rs`, `two_fluid_pimple.rs`, `tampines`
  `multiphase_1d/two_fluid.rs` (a 1-D `limitSum` analogue) and the docs of this crate.
  `outram-foam-basic-lib` holds no MULES and no `limitedScheme`/`vanLeer` (only upwind/linear).
- Checked and rejected for reuse: `mules_lambda` does not compose (different storage, no boundary-face values) and is ESI's
  `MULES::limiter`, not GeN-Foam's `myMULES` (different `limit` return convention, `explicitSolve` that relaxes). Ported
  from the upstream source instead, with the source files cited in the headers.
- Upstream read: `solvers/twoPhase/include/alphaEqns_2p.H`, `alphaEqnsRoutines/solveAlphaMULES.H` (540 lines),
  `myMULES/{myMULES.C, myMULES.H, myMULESTemplates.C}`; ESI v2506 `LimitedScheme`, `NVDTVD`, `vanLeer`,
  `limitedSurfaceInterpolationScheme::weights`, `gaussGrad::calcGrad`, `GeometricField::relax`. No kovan-reviewed
  function is touched (no `kind = "review"` block under `src/genfoam`).

## What upstream does with `alpha { solver "MULES"; adjustSubCycles true; alphaMaxCo 0.25; }` (the specification)

`solvePhase` defaults to `both` (the shipped case sets none), so `solveAlphas(rDeltaT)` runs.
`nSubCycles = min(ceil((CoNum_+SMALL)/alphaMaxCo), maxSubCycles = 50)` when `adjustSubCycles` and both phases are present,
else `nSubCycles` (1); `CoNum_` and `bothPhasesArePresent_` are the values the previous step's `maxDeltaT` left. Per
sub-cycle: `alphaPhic_k` = three vanLeer fluxes (`div(phi,alpha)` of the total flux, `div(phir,alpha)` of the relative
flux and of the structure), `correctInflowOutflow`, `myMULES::limit(..., returnCorr = true)` (so `alphaPhic_k` becomes the
limited CORRECTION `lambda (alphaPhic - phiBD)`, not the flux), sources `Sp`/`Su` from `dgdt` with `minAlphaSp = 1e-6`, a
"preserve phi" step (`deltaPhi`), then `explicitSolve` with `psi.relax()` (`alpha.*` is relaxed 0.5), clamp to [0, 1],
and a `zeroSum` / `movingAlpha/(alpha1+alpha2)` renormalisation. Sub-cycles reassign `alpha.oldTime()` and restore it and
average the fluxes; `alphaRhoPhi = interpolate(rho) alphaPhi` (LINEAR; implicitUpwind used upwind).

## Predictions (written BEFORE any port run of this round; committed first)

Upstream facts measured first (real binary, tight protocol, every step written, `scripts/boiling_mules_protocol.sh`, run
to `endTime` 7): it runs the case as shipped through step 649 (t = 6.2933 s) and then ABORTS with "Negative initial
temperature T0: -2354.9" in `hPolynomialThermo` (the vapour, after the flow excursion). `alpha.vapour` first exceeds 0
at step 616 (max 3.2e-9); sub-cycles are 1 until then and reach 12 at the excursion. References:
`refs/boiling_mules_tight_head.ref.txt.gz` (steps 1-70) and `refs/boiling_mules_tight_onset.ref.txt.gz` (steps 560-649),
both from that one fresh run of the installed binary (NOT the committed `boiling_tight_full_flow` reference).

| id | quantity | prediction | if missed |
|---|---|---|---|
| M1 | steps 1-70, `deltaT` | identical to 1e-12 at every step | `CoNum_`/`maxDeltaT` state |
| M2 | steps 1-70, p, p_rgh, T.liquid, U.liquid, T.vapour, T.interface, U.vapour | the round-3 bounds hold (p 1e-11, T.liquid 1e-12, U.liquid 1e-10, T.vapour 1e-9, T.interface/U.vapour 5e-9); alpha.vapour and dmdt exactly 0 | MULES disturbs the single-phase state |
| M3 | steps 560-615 (single phase to the onset) | p, T.liquid, U.liquid <= 1e-9 relative at every step | accumulated round-off |
| M4 | the sub-cycle count per step | equals upstream's log ("No SubCycles") at every step 1-649 | `CoNum_` ordering or `bothPhasesArePresent_` |
| M5 | steps 616-627 (onset window), alpha.vapour and dmdt, relative to their maxima | <= 1e-8 (the explicit MULES update has no linear-solver tail, unlike implicitUpwind's 3.4e-6) | limiter/flux translation difference |
| M6 | steps 616-627, p, T, U | <= 1e-8 | |
| M7 | first step after the onset at which any field leaves 1e-6 | in 628..649, or none by 649 | |


## What was ported (new code only; no existing function that carries a kovan review stamp was touched)

| item | upstream | port |
|---|---|---|
| `myMULES::limiter`, `limit(returnCorr = true)`, `explicitSolve` (6-argument form, `rho = one`, no moving mesh) | `myMULESTemplates.C` | `two_phase/mules.rs` (`limiter`, `limit_return_corr`, `explicit_update`); `limitSum` is declared by upstream but never called by GeN-Foam: not ported |
| `Gauss vanLeer` / `upwind` / `linear` convection of `fvc::flux(phi, vf, scheme)` | ESI `LimitedScheme`, `NVDTVD`, `vanLeer`, `limitedSurfaceInterpolationScheme::weights`, `gaussGrad` | `mules.rs` (`interpolate`, `flux`, `gauss_linear_grad`) |
| `solveAlphaMULES.H`, `solvePhase both`: `alphaPhic`, preserve-phi, `Sp`/`Su`, relaxation, clamp, `zeroSum`, sub-cycles with `oldTime` bookkeeping, `alphaRhoPhi` | `solveAlphaMULES.H` | `two_phase/alpha_mules.rs` |
| `CoNum_` and `bothPhasesArePresent_` as `maxDeltaT` leaves them | `twoPhase::correctCourant`, `maxDeltaT` | `TwoPhaseSolver::{co_num_upstream, phases_present}` |
| `alphaf` by linear interpolation when the alpha solver is not `implicitUpwind` | `refsAndControls_2p.H` | `correct_pressure` |
| reader: `alpha { solver MULES; ... }`, `fvSchemes` `div(phi,alpha)` / `div(phir,alpha)`, `alpha.<fluid>` relaxation factors, `minAlphaSp` (MULES default 1e-6, not 1e-4), `nLimiterIter`, `smoothLimiter`, `extremaCoeff`, `maxSubCycles`, `nSubCycles` | | `case.rs`: `AlphaSolver`, `MulesControls` |

**MULES is now the default whenever a case selects it** (`TwoPhaseCase::read` of the shipped `1D_boiling` has no deviation). The
earlier behaviour stays callable as `TwoPhaseCase::with_alpha_solver_implicit_upwind()` (reported in `deviations`); the pinned
`genfoam_two_phase_1d_boiling` numbers (P1-P5) are the implicitUpwind run and call it. A case with `solvePhase` set under MULES
(`1D_HX/twoPhase`, `2D_KNS37-L22`: the single-phase `solveAlpha` lambda, `limit(returnCorr = false)`) is a hard error in the reader, not a
silent substitution. Any other alpha-solver word is now also a hard error (it used to be replaced by implicitUpwind silently).

## Defect found by the comparison, and how (translating upstream)

The first run missed the onset: U.vapour was 3.3e-5 off at step 616 (alpha.vapour first > 0), against 9e-11 for the noise-floor control. The
fields written by the real binary (`alphaPhi.*`, `phi.*`, `Kd.liquid.vapour`, `dmdt`) localised it to the vapour face fluxes of
faces 28-30 (phi.vapour 3e-5 off, alphaPhi.vapour and alpha.vapour at the 1e-6 of the alpha noise, phi.liquid exact), and
`Kd.liquid.vapour` (upstream 5.55 in a cell whose alpha is 0, port 1) showed a slip velocity the port did not have. The cause was in
upstream `refsAndControls_2p.H`, read AFTER the symptom (the port's comment said "alphaf by upwind (implicitUpwind)"): **upstream uses
`upwindScheme.interpolate(alpha)` for `alphaf` only when the alpha solver is `implicitUpwind`, and `fvc::interpolate(alpha)` (linear)
otherwise.** The port now does the same. U.vapour at step 616: 3.3e-5 -> 9.4e-11; steps 616-627 U.liquid 1e-6 -> 3.8e-9. Only the MULES path
changes (implicitUpwind keeps upwind `alphaf`). Predicted sign/size before the fix: not predicted (found by localisation).

## Results (measured 2026-11-26, release; `tests/genfoam_two_phase_mules_vs_upstream.rs`, 6 gates; runs 3 s)

Reference: ONE fresh run of the installed upstream binary (tight protocol, `writePrecision 17`, every step; the same binary that
cannot reproduce the committed `boiling_tight_full_flow` after step 639 and aborts here at step 650: "Negative initial temperature
T0 = -2354.9", `hPolynomialThermo`, vapour, after the flow excursion; an upstream abort, not a port result). Committed references:
`refs/boiling_mules_tight_head.ref.txt.gz` (steps 1-70) and `boiling_mules_tight_onset.ref.txt.gz` (560-649), 0.27 and 0.36 MB. The older
`boiling_tight_*` references are NOT used by this round. Worst relative difference over the 40 cells (39 faces), relative to the
field's own maximum:

| span | step size, sub-cycles | p | T.liquid | T.vapour | U.liquid | U.vapour | alpha.vapour | dmdt | alphaPhi.vapour | phi.vapour |
|---|---|---|---|---|---|---|---|---|---|---|
| 1-2 | identical | 1.4e-13 | 3.5e-16 | 3.5e-12 | 1.1e-13 | 2.0e-11 | 0 (exact) | 0 | 0 | 2.0e-11 |
| 3-70 | identical | 1.0e-12 | 3.0e-13 | 2.3e-10 | 9.4e-12 | 1.6e-11 | 0 | 0 | 0 | 1.6e-11 |
| 560-615 | identical | 7.9e-12 | 1.0e-11 | 3.0e-11 | 2.7e-11 | 2.7e-11 | 0 | 0 | 0 | 2.7e-11 |
| 616-627 (onset window; vapour first > 0 at 616) | identical | 8.3e-10 | 1.4e-11 | 2.8e-11 | 3.8e-9 | 3.0e-8 | 4.0e-6 | 3.6e-6 | 4.0e-6 | 3.3e-8 |
| 628-634 | identical | 4.1e-8 | 7.2e-10 | 2.9e-9 | 2.3e-7 | 3.8e-7 | 6.3e-6 | 5.4e-6 | 4.7e-6 | 4.1e-7 |

(T.interface: 1.5e-11 at steps 1-2, 7.6e-10 at 3-70, 3.1e-11 at 560-615, 8e-9 at 628-634. alphaPhi.liquid <= 1.3e-10 to step 615, 2.1e-7 at 628-634.)

**First diverging step.** The number of outer iterations equals upstream's at every step to 648 (6, 7 at step 628, then 24). Step size equal to
1e-12 through 636, differs at **637** (3.9e-2); the number of MULES sub-cycles equals upstream's through 637, differs at **638**. The first field to
leave 1e-6 is a FLOW field, at **step 635** (p 2.7e-4, U.vapour 8.0e-4); `alpha.vapour`/`dmdt`/`alphaPhi.vapour` are above 1e-6 already from the onset
step 616 (1.7e-6) because the relative error of a quantity that is born at 1e-9 is the single-phase 1e-11 divided by the
superheat (~0.015 K). Not bit-identical anywhere: the residual in the single-phase span is the p_rgh GAMG solve of upstream against the
port's solver (round 3), inside the 1e-9 allowance (cause explained, flagged for the maintainer as CLAUDE.md asks).

**Noise-floor control** (`mules_noise_floor_is_the_size_of_the_onset_window`): the port against ITSELF with the initial liquid temperature scaled by
1 + 1e-9 (single-phase disturbance of the size port-vs-upstream shows before the onset): alpha.vapour 1.0e-6 at step 616 (port vs upstream 1.7e-6),
the flow fields leave 1e-6 at step **636** (port vs upstream 635). At step 627 the control has p 6e-11, U.liquid 3e-10 against port-vs-upstream
8e-10 and 4e-9: the disturbances differ in direction, which this control does not separate from a ~10x genuine difference; stated as
not distinguished. The same ratio against round 3's implicitUpwind window (p 3.6e-10, U.liquid 1.7e-9, U.vapour 1.9e-8 at 617-627) is 2x.

**What the pre-round-7 substitution misses** (`mules_ablation_to_implicit_upwind_misses_the_shipped_run_by_the_pinned_amount`): the port with
implicitUpwind against the MULES run of the real binary: p 8.5e-5, U.liquid 2.0e-3 at step 70 (upwind `alphaf`, and an alpha equation that is
not upstream's, change the single-phase pressure coupling), p 2.9e-4 at step 600, and the onset itself off by 1.0 of the vapour at step 616
(the implicit vapour appears at a different step). This is the size of what "all twoPhase comparisons so far ran implicitUpwind in both
codes" did NOT test.

## Predictions: held and missed

| id | result |
|---|---|
| M1 held | `deltaT` identical to 1e-12 for steps 1-636 (not only 1-70) |
| M2 held | steps 1-70: every bound of round 3 holds, alpha and dmdt exactly 0 |
| M3 held | 560-615: all <= 3.0e-11 (predicted 1e-9) |
| M4 held | sub-cycle count equal to upstream's at every step 1-637 (first miss 638, after the trajectories separate) |
| M5 **missed** | alpha.vapour and dmdt at the onset 1.7e-6 / 4.3e-7, not <= 1e-8: my reason ("no linear-solver tail") was wrong for the relative error, which is amplified by the superheat; the noise-floor control gives the same size |
| M6 **missed in the second half** | p <= 1e-8 held to 627 (8.3e-10) but U.vapour (3.0e-8) and U.liquid (3.8e-9) are above 1e-8 at 627 |
| M7 held (in the sense predicted) | first flow field > 1e-6 at step 635, inside 628..649 |
| (not predicted) | the `alphaf` defect above, found by localisation; first run U.vapour 3.3e-5 |

## Moved numbers (old -> new, cause)

- Reading the shipped `1D_boiling`: `TwoPhaseCase::deviations` 1 entry -> 0, `controls.alpha` implicitUpwind -> MULES (default change as the maintainer's "correct physics is the default" rule requires).
- No pinned V&V number moved: the implicitUpwind comparisons (rounds 3-5) construct their case with `implicitUpwind` in `fvSolution`; `genfoam_two_phase_1d_boiling` P1-P5 call `with_alpha_solver_implicit_upwind()` and are unchanged.
- The `alphaf` change touches only the MULES path (`correct_pressure` keeps upwind for implicitUpwind).

## What remains (round 7)

- MULES with `solvePhase` set (`solveAlpha` lambda: `limit(returnCorr = false)`, `movingAlpha - alpha` flux, `explicitSolve` with the 9-argument limiter): used by `1D_HX/twoPhase`, `2D_KNS37-L22`; reader refuses it.
- Coupled patches (`syncTools::syncFaceList`, `cyclic`) and `wedge` (`lambda = 0`) in the limiter; moving meshes; `rho` other than `geometricOneField`.
- Bit-identity needs a bit-identical p_rgh solve (GAMG).
- **The sub-cycle path (`nSubCycles > 1`) is not compared bitwise**: upstream's own run first needs 2 sub-cycles at step 637 (and 12 at step 648), after the trajectories have separated. It is checked by the count agreeing (to step 637) and by `mules_sub_cycles_restore_old_time_and_keep_the_fractions_bounded` (forced 3 sub-cycles: `oldTime` restored, fractions fill the moving volume, bounded). A case with a non-uniform initial alpha (the reader takes only `uniform` fields) would exercise the limiter at large Courant number before the chaos; not built.
- `docs/outram-foam-appbuilder-lib-api.md` (generated mirror) not regenerated: `kovan-cli` is not built here.
- The installed upstream build still cannot reproduce the committed post-onset references (round 5 finding); not touched.


# Round 8 (gh:#1222, gh:#1225): TRACE wall boiling, `twoParameters` regime map, `byRegime` dispatch

AI-written, not human-reviewed. Research, education and V&V only (`RESPONSIBLE_USE.md`). Upstream: the real
unmodified `GeN-Foam` binary (652b3da0cc55, ESI v2506, serial), the same installed build as rounds 3-7.

## What was found before any code was written (read from the shipped tutorials, 2026-11-26)

| claim in #1222, #1225 and the gap audit | what the tutorials show |
|---|---|
| "every PSBT case selects `multiRegimeBoilingTRACE`" | **No.** `PhaseII_Ex1_01_5215` (the case the closure comparison of round 3 used) selects `multiRegimeBoiling` (ported), a EMPTY `regimeMapModels {}` and `porousKEpsilon2PhaseCorrected` turbulence. `Phase_Ex1_12223` selects `multiRegimeBoilingTRACE`, `twoParameters`, `byRegime` (dispersion, vapour diameter, interfacial area), `Bestion` drag, `LockhartMartinelli`, `waterTRACE` and is LAMINAR. `PhaseII_Ex2_04_6770` and `PhaseII_Ex3_11-0312` select `multiRegimeBoilingTRACECHF` + `lookUpTableCHF` + `multiRegimeBoilingVapourTRACE` and RAS turbulence. |
| "the PSBT cases can run once these closures are ported" | **No.** Every PSBT case is a through-flow case: `U` `fixedValue` inlet, `pressureInletOutletVelocity` outlet, `slip` walls, `inletOutlet` `alpha`, `fixedFluxPressure` `p_rgh`. `TwoPhaseCase::read` refuses the velocity types by name, and `scalar_bcs` turns `fixedFluxPressure` and `inletOutlet` into `calculated` SILENTLY. The three `PhaseII` cases additionally need the `porousKEpsilon2PhaseCorrected` turbulence model, which the two-phase solver does not have (laminar only). **None of the four shipped `1D_PSBT_SC` cases can run end to end in the port.** Filed as child issues (below), not worked around. |

So the end-to-end goal of the task is NOT met; this round ports what the two issues name, wires it into the solver, and
compares it with the real binary on a **closed variant** of `Phase_Ex1_12223`: the same mesh, models, power and initial fields,
with only the boundary-condition files changed to the types `1D_boiling` (the case the solver was built for) uses
(`tests/data/genfoam_psbt_closed_gh1361/PROVENANCE.md`). That is a code-to-code comparison on a pressure-driven variant of the
PSBT channel, not the PSBT experiment and not the shipped tutorial.

## What was searched before writing anything (CLAUDE.md "search before building")

- `grep -rniE "twoParameters|regimeDomain|regimeBoundary|byRegime|hashlittle|Rand48|lookup3" crates/**/*.rs`: the port had `RegimeMap1D`
  (`closures/interfacial/regime_map.rs`, `oneParameter`, which says in its module doc that `twoParameters` and the
  `byRegime` dispatch are deferred), a hand-written `ByRegime` (`f64`, dispersion and contact partition only) in
  `solver/two_phase/mod.rs`, and no hash, `Random` or polygon code anywhere. Reused: `RegimeInterpolationMode` and the
  `(name, weight)` list `regime_weights` returns.
- Heat-transfer closures: `closures/heat_transfer/{boiling,chf,fs_htc}.rs` already had Gorenflo, Basu, RezkallahSims, Chen, SahaZuber
  and the `multiRegimeBoiling` pure function. Reused (Gorenflo and Basu through the solver, SahaZuber changed, below).
  `multi_regime_boiling_htc` itself does not compose (it carries the heat-flux-superposition exponent and a suppression factor
  TRACE does not use, and returns no `q_SC`), so `multiRegimeBoilingTRACE` is a new function.
- Upstream read in full: `regimeMapModel.{H,C}`, `regimeMapModelTemplates.C`, `twoParameters.{H,C}`, `regimeDomain2D.{H,C}`,
  `regimeBoundary2D.{H,C}`, `byRegimeModel.{H,C}`, `byRegimeModels.C`, `multiRegimeBoilingTRACE`, `...TRACECHF`, `...VapourTRACE`,
  `GorenfloFSHeatTransferCoefficient.C`, `BasuTONB.C`, `TONBModel.C`, `SahaZuberSubCooledBoilingFraction.C`, `RezkallahSims...C`,
  `phaseChangeModel.C` (`correct`, `limitMassTransfer`), `heatDrivenPhaseChange.C`, `FSPair.C` `correct`, `structure.C`
  (`Twall_`), `fluid.C` (`correctAlphaRhoMagU`), `reconstructU_2p.H`, ESI `Random`, `Rand48`, `Vector2D`, `Hasher.C`,
  `HashTable.C`. No kovan-reviewed function was touched (no `kind = "review"` block in `closures/` or `solver/`).

## What upstream does that the port did not (found by reading, before any comparison)

1. **`twoParameters` walks a `HashTable<regimeDomain2D, word>`** to find the first polygon that contains the point. That order is
   the bucket order of Jenkins' `hashlittle` of the regime name in a 128-bucket table, newest-first within a bucket, not the
   dictionary order. A point on a shared edge belongs to whichever polygon the walk meets first.
2. `sign_` and the test ray of a polygon are computed from the UNNORMALISED points; the ray is `Foam::Random(69 (id+1917))` (a `drand48`
   generator through libstdc++'s `generate_canonical`), a fresh ray per boundary, the last one kept; and `Vector2D(rng.sample01(),
   rng.sample01())` draws `y` first (GCC evaluates arguments right to left: measured with a g++ -O3 probe, `tests` doc).
3. `multiRegimeBoilingTRACE` reads **`Twall`, a separate field** (`T.wall`), zero until the first `structure::correct`, which copies `Tact`.
   It hard-wires the cubic flux superposition, never evaluates its suppression model, and writes `dmdtW = iAact q_SC / |L|`.
4. `phaseChangeModel::correct` adds `dmdtW` to `dmdt`, splits `dmdt` into `(1-fw) dmdt` and `fw dmdt` (`fw = dmdtW/dmdt` where `|dmdt| > 1e-2`),
   adds the wall part to `c1` or `c2` by sign, and zeroes `dmdtW` after use.
5. `SahaZuber` (`SahaZuberSubCooledBoilingFraction.C`) is `T_ld = max(T_sat - q Dh/(0.0065 max(Re Pr, 7e4)), 0)` with NO division by the
   thermal conductivity. The port had added the `k` of the published correlation and claimed to "deviate from upstream's literal C++".
   Per the maintainer's 2026-11-26 "follow upstream", upstream's form is now the default (`SubcooledBoilingFraction::SahaZuber`) and the
   earlier form is `SahaZuberPublished`. The two existing unit tests of the published formula now name `SahaZuberPublished` (values unchanged).
6. `BasuTONB.C` converts the contact angle as `deg*2.0*pi/360.0` and cubes it with `pow(.,3)`; the port used a `uom` degree conversion and `powi`.
   Now `deg*2.0*pi/360.0` and `r_powf(3.0)`.
7. `alphaRhoMagU.<fluid>` (`fluid::correctAlphaRhoMagU`, in `reconstructU_2p.H`) is the second axis of the PSBT regime map; the port had no such field.

## Predictions (written BEFORE the first port run on this case; committed first)

Upstream facts measured first (real binary, tight protocol, every step written, `scripts/psbt_closed_protocol.sh`, endTime 0.6, serial):
the vapour first appears at **step 14** (t = 0.1399 s, max `alpha.vapour` 3.8e-3), 0.22 at t = 0.38 s; the step size starts at 5e-3 and
falls to 7.8e-4 by t = 0.375 s (`maxCoTwoPhase 0.5`); the GAMG `p_rgh` solve at tolerance 1e-14 often stops at its 1000-iteration cap with a
final residual of 1e-12 to 1e-13 (a round-off floor, reached in the same way as the `1D_boiling` tight runs).

| id | quantity | prediction | cause if missed |
|---|---|---|---|
| T1 | `TwoPhaseCase::read` of the closed case | succeeds with an empty `deviations` | a model key the reader still refuses |
| T2 | step size `dt`, steps 1-13 | equal to upstream's to 1e-9 relative | `Courant` / `maxDeltaT` differences |
| T3 | steps 1-13 (single phase): `p`, `T.liquid`, `T.fixedPower` | <= 1e-9 relative to the field maximum | the round-3 GAMG-vs-port p_rgh floor |
| T4 | steps 1-13: `U.liquid`, `alphaRhoMagU.liquid` | <= 1e-8 | same |
| T5 | steps 1-13: `htc.liquid.structure` (TRACE, single phase but wall above saturation) | <= 1e-8 | a wrong branch of the TRACE map, `Twall` lag, Basu/Gorenflo/RezkallahSims arithmetic |
| T6 | the step at which the port's `alpha.vapour` first exceeds 0 | step 14 (the same as upstream's) | `dmdtW`/`SahaZuber` form |
| T7 | steps 14-30: `alpha.vapour`, `dmdt.liquid.vapour`, `areaDensity.interface` | <= 1e-4 relative (amplified by the small vapour fraction, as in round 7) | regime weights, `byRegime` area, `fw` split |
| T8 | steps 14-30: `p`, `T.liquid`, `U.liquid` | <= 1e-6 | |
| T9 | first step at which any compared field leaves 1e-6 | at or after step 14; none before | |

If T5 or T6 miss, the first suspect is the SahaZuber form (upstream's no-`k` form makes `T_ld` hotter than the published one, so the sub-cooled fraction
and with it `dmdtW` is larger), then the Gorenflo/`Twall` evaluation order.

## Round 8 results (measured 2026-11-26, release; `tests/genfoam_psbt_trace_vs_upstream.rs`, 4 gates, runs 1 s)

Reference: ONE fresh run of the installed upstream binary on the closed variant (tight protocol, `writePrecision 17`, every step, serial,
`scripts/psbt_closed_protocol.sh`): 417 steps, then **upstream aborts with SIGFPE at t = 0.4925 s** (its own numerical blow-up: `T.liquid`
max 682 K, `T.vapour` max 760 K in the last outer iteration; an upstream result, not a port result). Committed reference:
`refs/psbt_closed_head.ref.txt.gz` (steps 1-40, 0.29 MB); the gates use steps 1-20. 75 cells. Worst relative difference over the cells,
relative to the field's own maximum:

| span | dt | p | T.liquid | T.vapour | T.fixedPower | U.liquid | U.vapour | alpha.vapour | dmdt | htc.liquid.structure | areaDensity | alphaRhoMagU.liquid |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1-8 (single phase, wall below ONB) | equal to 1e-12 | 7.9e-12 | 2.0e-11 | 8.1e-13 | 4.2e-12 | 4.7e-10 | 9.5e-9 | 0 (exact) | 0 | 2.1e-10 | 0 | 4.7e-10 |
| 9-13 (single phase, wall above ONB: TRACE nucleate-boiling branch) | equal | 2.1e-9 | 2.0e-8 | 2.5e-10 | 5.6e-9 | 1.7e-7 | 1.6e-7 | 0 | 0 | **7.8e-9 (step 9), 6.2e-7 (step 12)** | 0 | 1.7e-7 |
| 14 (vapour onset, both codes) | equal | 5.3e-7 | 1.5e-6 | 2.4e-8 | 1.7e-6 | 5.8e-5 | 3.9e-4 | 2.0e-3 | 2.8e-4 | 3.3e-4 | 2.8e-4 | 7.2e-5 |
| 15-20 | <= 5e-5 | 2.0e-6 | 1.4e-6 | 2.6e-7 | 1.7e-6 | 1.9e-5 | 4.7e-4 | 9.1e-4 | 9.4e-4 | 4e-4 | 9e-4 | 2.8e-5 |

Not bit-identical anywhere (the p_rgh solves differ, as in rounds 3-7). **The outer-iteration counts equal upstream's at steps 1, 2 and 3: 12, 0
(step 2 is SKIPPED by upstream: "PIMPLE: converged in 0 iterations", its `customPimpleControl` defect, reproduced by the port) and 11.** The step size
is equal to 1e-12 through step 14; the vapour appears at step 14 in both codes.

**First diverging step: 9 (htc.liquid.structure 7.8e-9, the first value above the 1e-9 round-off allowance).** It is the first step at which the wall is above
the onset of nucleate boiling (`Tw - TONB` = 0.4 to 0.8 K in the cells of the heated zone), i.e. the first use of the TRACE cubic superposition. From there
the structure temperature (and with it `U`, `p`) follows and grows by about 10x per step to 1e-6 at step 12-14. **Cause: not identified.** The noise-floor
control (`psbt_closed_noise_floor_dump`: the port against itself with the initial liquid temperature scaled by 1 + 1e-12) reaches T.fixedPower 5.3e-11 and htc
1.1e-8 at step 13 and alpha.vapour 1.2e-8 at step 14, so the port-vs-upstream difference is ~100x (T.fixedPower, htc) and ~1e5x (alpha.vapour at 14) that of a noise-floor
disturbance: a real, small, model difference, not chaos. It is smooth along the channel (htc 1.3e-9 at cell 5, 8e-9 at cell 20), and the sensitivity of the TRACE
htc to the onset temperature is (1/htc) dhtc/dTONB = -1e-2/K, so it corresponds to a difference of the order 5e-7 K in `T_ONB` or `T_sat` (T.interface agrees to 1e-10 K), i.e.
5e-7 relative in a Basu input (`rho_vapour`, `|L|`, `kappa`) or in the Gorenflo pair. Candidates not yet tested (each needs an upstream run that writes the
intermediate field, which the real binary does not): the liquid `kappa` placeholder (upstream refreshes `fluid::kappa()` at the end of `EEqns`, the port's field
holds the thermo value at the last `thermo_correct`), the vapour density in the cells where the vapour is below `thermoResidualAlpha`, `L` before the adjust.
Filed as a child issue; the gate `nucleate_boiling_branch_differs_from_upstream_by_the_pinned_amount` pins the measured size.

### Defects found by the comparison, and how (translating upstream)

1. **`phi.*` relaxation was not ported** (found by the PISO residual sequence of step 1, outer iteration 1: upstream 1, 3.0866e-5, 1.2221e-5, 4.84e-6 (ratio 0.40 per
   corrector); the port 1, 3.86e-5, 1.91e-5, 9.5e-6 (ratio 0.50)). Upstream: `refsAndControls_2p.H` `phi1.storePrevIter()` at the start of the outer iteration and
   `reconstructU_2p.H` `phi1.relax()` in every corrector; `relaxationFactors.fields "phi.*" 0.8` is in every PSBT case. Ported (`TwoPhaseControls::relax_phi`). After: the sequence
   equals upstream's (second corrector 3.08655856e-5 against 3.08655860e-5, 1.2e-8 relative; twelfth 2.9236e-9 against 2.9238e-9). Step 1 `p` 1.9e-10 -> 7.9e-12,
   `T.fixedPower` 3.2e-11 -> 1.2e-12, step 2 `T.fixedPower` 3.9e-3 (the port ran 8 outer iterations where upstream runs none) -> 1.2e-12 (skipped in both). Predicted before the fix: not predicted.
   No earlier ported case has a `phi.*` factor, so no pinned number moved.
2. `Nusselt` and `NusseltAndWall` computed `nu*k/dh` and `B*(Re^C Pr^D)`; upstream is `(kappa/Dh)*(A + B*pow(Re,C)*pow(Pr,D)*Tratio)` (and `pow(Re*Pr, C)` when `C == D`).
   Now upstream's association. **Measured effect: none** on the step-9 htc (7.80e-9 before and after), so it is not the cause above; kept because upstream's form is the default.
3. `SahaZuber` (no `k`, `Basu` angle and `pow(.,3)`): in this span they influence only `dmdtW` (no vapour before step 14), so the comparison cannot yet separate the old and new
   forms. The old `SahaZuber` is `SahaZuberPublished`, pinned by the two existing unit tests (values unchanged).

### Predictions: held and missed

| id | result |
|---|---|
| T1 held | the closed case reads with an empty `deviations` |
| T2 held | `dt` equal to 1e-12 for steps 1-14 |
| T3 held for steps 1-11, **missed after** | `p` <= 1e-9 to step 11 (2.4e-10), 1.1e-9 at 12, 2.1e-9 at 13; T.liquid 6.7e-10 at step 10, 9.9e-9 at 12 |
| T4 held for steps 1-9, **missed after** | U.liquid <= 1e-8 to step 9 (2.8e-10), 1.5e-8 at step 10, 1.6e-7 at step 12 |
| T5 held to step 9, **missed after** | htc <= 1e-8 to step 9 (7.8e-9), 2.7e-8 at step 10, 6.2e-7 at step 12: the nucleate-boiling branch |
| T6 held | `alpha.vapour` first above 0 at step 14, as upstream |
| T7 **missed** | alpha 2.0e-3, dmdt 2.8e-4 at the onset (predicted 1e-4); my reasoning ("amplified by the small vapour fraction") is the noise-floor argument, which the control shows does not explain it (1.2e-8) |
| T8 **missed** | `p` <= 1e-6 held at 14 (5.3e-7), 2.0e-6 at 15; `T.liquid` 1.5e-6, `U.liquid` 5.8e-5 |
| T9 held | nothing leaves 1e-6 before step 14 (worst 1.7e-7 at step 13) |

### Moved numbers (old -> new, cause)

- `SubcooledBoilingFraction::SahaZuber` now upstream's form (no `k`); the earlier form is `SahaZuberPublished`; the two unit tests that pin the published form were renamed to it, values unchanged.
- `Nusselt` / `NusseltAndWall` htc: operation order changed to upstream's `(kappa/Dh)*(A + B*pow(Re,C)*pow(Pr,D)*Tratio)` (an ulp; the earlier `nu*k/dh` stays callable as
  `FsForcedConvectionHtc::heat_transfer_coefficient_earlier_association`, selected per solver by `TwoPhaseSolver::with_earlier_nusselt_association`). **One pin is affected:**
  the 12 s `1D_boiling` transient of `tests/genfoam_two_phase_1d_boiling.rs` (`one_d_boiling_full_transient`, implicitUpwind, P1-P5) runs through the chaotic post-onset flow excursion
  and, with upstream's order, ends in `Diverged` (the step size collapses to NaN) where it earlier ran to the end: a one-ulp change moved a qualitative outcome, which is not a "moved pin for rounding" within the
  1e-4 propagated allowance. ~~Not accepted as a move: the test selects the earlier order (no value, tolerance or gate changed; recorded in the test) and **this is flagged for the maintainer**.~~ **DECIDED 2026-11-26** (maintainer, "1b"): the test runs upstream's order and its divergence is a recorded failure, gh:#1811; it is `#[ignore]`d with that reason (re-measured: `Diverged { iter: 0, residual: NaN }`), and `one_d_boiling_earlier_nusselt_association_is_selectable` reaches the earlier order. Upstream's own build aborts on this case after the onset (rounds 3 and 7), so the sensitivity is shared. The other 1D_boiling gates
  (rounds 3-7, which compare with the real binary up to the onset) pass under the upstream order. All 610 tests of the crate pass (35 ignored).
- `TwoPhaseCase::read` of `regimeMapModels {}` (empty): `None` instead of the error "exactly one regime map is supported".
- Basu contact angle: `deg*2*pi/360` and `pow(theta, 3.0)` (round-off).

### What remains

- **No shipped `1D_PSBT_SC` case runs end to end.** Through-flow boundary conditions (`fixedValue` velocity inlet, `pressureInletOutletVelocity`, `slip`, `inletOutlet` alpha, `fixedFluxPressure`), and for the three
  `PhaseII` cases the `porousKEpsilon2PhaseCorrected` turbulence model, are needed (child issues). `scalar_bcs` still turns `fixedFluxPressure` and `inletOutlet` into `calculated`
  silently; it should refuse them by name (it is shared with `one_phase_fc`, whose gFHR comparison depends on the present reading, so it was not changed here).
- The origin of the 7.8e-9 htc difference at the first nucleate-boiling step (above).
- `TRACECHF`/`VapourTRACE` are not wired (their annular-flow models and `lookUpTableCHF` are not ported; `lookUpTableCHF` is deferred to the maintainer, the table having no licence statement).
- `byRegime` for drag, virtual-mass, multiplier and htc models is not wired (the generic `ByRegimeModel<M>` exists).
- The comparison stops at step 20 (gated) / 40 (reference): upstream's own run ends at step 417 with SIGFPE.
- `docs/outram-foam-appbuilder-lib-api.md` (generated mirror) not regenerated: `kovan-cli` is not built here. The new functions are not in any `kovan.toml`.




# Round 9 (issue 1361; issues 1773, 1774, 1775): the `2D_MSFR` transient stage, COUPLED

AI-written, not human-reviewed. Research, education and V&V only.

## What was searched before writing anything (CLAUDE.md "search before building")

`OnePhaseFcSolver` (`solver/two_phase/one_phase_fc.rs`: faceCentered path, refuses `momentumMode cellCentered`, turbulence,
`Boussinesq`, `fixedTemperature`), `solver::one_phase::OnePhaseSolver` (constant-property stand-in, no reader),
`MultiPhysicsSolver`/`MeshHandler`/`MeshToMesh` (`multi_region/`: lumped region models with `Time`/uom, nearest-cell plus a global
rescale), `PseudoTimeClock`/`neutronics_max_delta_t`/`adjust_delta_t` (neutronics-only, one solver, a clock that starts at 0),
`closures/` (no `constant` FS htc model; `ReynoldsPower` exists but only feeds `Kd`), `structure::power_model::FixedTemperature`
(an uom-typed model, not a field), `fv.rs`/`pbicgstab.rs`/`thermo.rs` (reused). **Reused:** `fv::{Geometry, Eqn}`, `pbicgstab_diagonal`,
the `fvMatrix::relax` translation (copied, cited), `upstream_transient_step`, `neutronics_max_delta_t`'s formula. **New:**
`one_phase_cc.rs`, `multi_region/coupled_loop.rs`, `dump_mesh_to_mesh`.

## What upstream does, read BEFORE any port code (the specification)

* `onePhase::correctPhysics` (`onePhase.C:134`): `while (pimple_.loop()) { if (solveFluidMechanics or solveEnergy) correctModels(true,true);
  if (solveFluidMechanics) correctFluidMechanics(); if (solveEnergy) correctEnergy(); }`. **`turbulence().correct()` is in `UEqn_1p.H:12`
  only**: with `solveFluidMechanics false` it never runs, so `k`, `epsilon`, `nut`, `alphat` are READ from the restart and frozen (the open
  question of #1775, answered). `EEqn_1p.H:10`: `alphaEff = thermo.alphaEff(turbulence().alphat())` = `CpByCpv (alpha_thermo + alphat)`,
  `CpByCpv = 1` for an enthalpy form, `alpha_thermo = mu rPr` (constTransport).
* **`fluid::rho()` returns `rho0` (constant) for a Boussinesq EOS** (`fluid.C:795`): the equation's `ddt(alpha, rho, he)`, `alpha rho (U.g)`
  and `ddt(alpha, rho, K)` use `rho0 = 4125`; only the thermo's own `rho(T)` (mapped to the neutronics as `rhoCool`) varies. `Boussinesq::H = 0`
  in v2506, so `hs = Cp (T - Tstd)`.
* `".*"` is the LAST pattern of `fvSolution.solvers`, so `h` is solved by `PBiCGStab`/`diagonal` (the log's `diagonalPBiCGStab`), not by the
  `"h.*"` smoothSolver above it (OpenFOAM takes the last matching pattern). `relaxationFactors.equations ".*" 0.9`.
* `contErr` is computed once at construction (`div(alphaRhoPhi)`, the `ddt` part is exactly 0) and never again with `solveFluidMechanics false`.
  The wedge patches carry a ~1e-11 residual flux in the restart; it enters `div`, `contErr`, the `K` flux and the Courant number.
* Structure: `fixedTemperature` powerModel (`correct()` is empty; `correctT` copies the restart `T.fixedTemperature`); source
  `iAact Hs (Tact - T + he/Cp)` explicit and `Sp(iAact Hs/Cp, he)` implicit; `htc = value` on the model zone (`constantModel`).
  `dragModels` (`ReynoldsPower`) only fill `Kd`, a momentum coefficient: not evaluated by the port (it does not enter the energy equation).
* `multiPhysicsSolver::correctPhysics`: `residual` is declared once and only `max`-ed; `mapTheseFields` at the top of each iteration; a
  `correctOnlyEnergy_` solver runs `correctTightlyCoupledPhysics` from iteration 2 on. The fluid's `getResidual` is the `max` of the five
  `h` initial residuals, the neutronics' `max(flux-solve initial residuals, neutroResidual)`.
* `meshHandler`: `meshToMesh(src, tgt, imCellVolumeWeight, pmAABB, normalise = false)`; `mapTgtToSrc` with `plusEqOp`:
  `result *= 1 - sum(w); result += w src`. **Measured** (`upstream_driver/dump_mesh_to_mesh`, built with `wmake` against the built ESI v2506,
  objects not kept; overlap volume 0.30568267086529161 both ways): in both directions, for all 8595 cells, one overlap, the cell itself,
  weight EXACTLY `1.0` (0 cells differ). The issue's "weight = 1 up to an ulp" was a guess; the weight is 1, the map is a copy.
* `setDeltaT.C`: `adjustDeltaT` takes `min` over the solvers' `maxDeltaT()` (here the one `multiPhysicsSolver`'s `min` of its two),
  `min(1.2 deltaT, limit)`; `Time::setDeltaT` then adjusts only for `writeControl adjustableRunTime` (not this case).
  `onePhase::maxDeltaT`: `Co = 0.5 max(surfaceSum(|alphaPhi|)/alpha/V) dt`, `fact = min(min(maxCo/(Co+SMALL), 1+0.1 maxCo/(Co+SMALL)), 1.2)`;
  `diffusionNeutronics::maxDeltaT`: the power-variation form, only for `time > controlDict deltaT`.

## Upstream run (real unmodified binary, serial, 2026-11-26)

`PROTOCOL=tight END_TIME=165.31 scripts/run_msfr_chain.sh transient <root>` from upstream's own t = 165 s restart (the round-6 start state):
50 steps, 35 s wall, `Multiphysics loop converged after 1 iterations` at every step. `scripts/extract_msfr_fluid_reference.sh` writes
`refs/msfr_fluid_transient.ref.txt.gz` (fluid `T` of every step, `deltaT` after every step, the five `h` initial residuals and
`T (avg min max)` lines, Courant numbers and `power:`).

## Predictions (written BEFORE the first coupled run; committed first)

| id | quantity | prediction |
|---|---|---|
| R1 | start state | fluid `T` of the restart equals the neutronics reference `T_0` to 1e-15; the port's `sum_g flux sigmaPow` at t = 165 s equals upstream's restart `powerDensity` to <= 1e-9 (risk: the restart field was written with the previous step's cross sections) |
| R2 | step sizes (the first thing compared) | step 1 is exactly 1e-3; every later step size agrees to <= 1e-9 relative (power-variation limited to step 14, Courant limited from step 15 on); I put 65 % on all 50 |
| R3 | fluid `T` per step, worst cell relative difference | <= 1e-9 at all 50 steps, expected 1e-12 to 1e-10, growing slowly with the step (same algorithm, PBiCGStab and the same relaxation; sources differ only in operation order). If a step is above 1e-9 the first one is step 1 and the cause is a term of `EEqn_1p.H` (ranked: kinetic-energy term, `relax`, the wedge boundary terms) |
| R4 | the five `h` initial residuals of each step | the first of step 1 agrees to 1e-4 relative (it is a genuine algebraic residual, 5.45e-8, not round-off), the factor of 10 between iterations is reproduced |
| R5 | `T (avg min max)` after each outer iteration | <= 1e-12 relative |
| R6 | power per step | <= 1e-9 relative at all 50 steps (round 6 gave 6e-14 with upstream's T as input; the T agreement of R3 carries over) |
| R7 | Courant number | <= 1e-9 relative |
| R8 | the coupling loop | converges after 1 iteration at 50 of 50 steps |
| R9 | `Re` field at steps 1, 10, 25, 50 | <= 1e-12 relative |


## Round 9 results (measured 2026-11-26, release; `tests/genfoam_msfr_coupled_vs_upstream.rs`, 3 tests, 15 s)

Command: `cargo test --release -p outram-foam-appbuilder-lib --test genfoam_msfr_coupled_vs_upstream -- --nocapture`. Port: the coupled loop
described above, run from upstream's t = 165 s restart, **nothing of upstream's per-step results used as input** (step size, power, `T` are the
port's own). Upstream: the 50-step tight run above.

### The first run, and what it found (recorded because it missed)

The first coupled run gave: step 1 fluid `T` 6.7e-16, `T (avg min max)` 2.3e-16, first `h` residual 4.7e-10, power 1.5e-14, **but `deltaT` of
step 2 = 1.2e-3 where upstream has 8.146e-4**, and every later step size, `T` (from step 2, 6.3e-7 at step 50) and power (2.4e-5) followed. The
first diverging quantity is therefore the step size after step 1, not a field. Cause, found by reading upstream (not by tuning):
`diffusionNeutronics::correctPhysics` sets `pTotOld_ = power()` BEFORE `solveNeutronics.H` calls `xs_.correct()`, and `power()` is
`sum flux sigmaPow` with the `XS` object's CURRENT `sigmaPow`, which at the first step of a restart is the **reference state's** (the `XS` class
is constructed there; chi was the same story in round 6). The reference-state `power()` is 2.06370139e7 W, 3.07 % above the 2.0003657e7 W of
step 1, so `maxPowerVariation 0.025 / 0.0307 = 0.8146`: upstream's step-2 size is 0.8146 deltaT (the log's `deltaT = 0.00081458923923836437`),
not 1.2 deltaT. The port's `upstream_transient_step` took `power_old` from its already-refreshed cross sections. Fix (new function,
`DiffusionNeutronics::upstream_power_at_reference_state`; the harness uses it for step 1 only, as upstream does): all 50 step sizes agree to 4.8e-13.

### The comparison (second run)

| quantity (per step, 50 steps) | worst relative difference | where |
|---|---|---|
| step size `deltaT` (the first thing compared) | **4.8e-13** | steps 1-14 (the neutronics `maxPowerVariation` limit, then 1.2x growth); from step 15 on the Courant limit, 0 to 2.3e-16 |
| fluid `T`, worst cell | **2.5e-14** (step 14); 6.7e-16 at step 1 | every step below 1e-13 |
| `T (avg min max)` after each of the 5 outer iterations | **1.5e-15** | |
| `power:` | **6.7e-14** (step 50); 1.5e-14 at step 1 | |
| first `h` initial residual of each step | **1.3e-10** | a genuine algebraic residual of 5e-8 to 1e-7 |
| all five `h` initial residuals (the last is ~1e-11, round-off of the sums) | 4.2e-6 (step 1, iteration 5) | not gated; the ratio of 10 per iteration is reproduced |
| Courant number (avg, max) | **4.8e-13** | |
| `Re` field at steps 1, 10, 25, 50 | **1.7e-15** | |
| neutron iteration count | equal at **50 of 50** steps (50, 50, 37, 1, 22, 1, 8, then 1) | |
| coupling loop | converged after 1 iteration at **50 of 50** steps | |
| wall time, 50 steps | 15.4 s (upstream 35 s, serial) | |

**No step diverges** once the step-size rule is as upstream's: there is no first diverging step. Bit-identity was not reached (the round-off
allowance 1e-9 is used by nothing: the worst number is 2.5e-14). The residual differences are the linear-solver and sum-order round-off of two
independent codes: `PBiCGStab`/`diagonal` is a translation (round 3), the matrix is assembled with `fv::Eqn`, the Courant sum runs over faces in
the same order but `0.5 * max(...)` differs in the last bit.

### Predictions: held and missed

| id | held? | measured |
|---|---|---|
| R1 fluid `T` of the restart equals the reference `T_0` to 1e-15; port power density at t = 165 s equals upstream's to 1e-9 | **missed twice** | 3.2e-7 and 1.8e-8. Not a port defect: the neutronics region's restart `TFuel` is the fluid `T` of the step BEFORE the last one (the mapping runs at the start of a step), so the pair of restart fields is not synchronous. The port starts step 1 from upstream's own `powerDensity` file, which is what upstream maps |
| R2 step sizes <= 1e-9, 65 % on all 50 | **missed in the first run** (step 2: 47 %), **held after the fix** (4.8e-13) | cause above: `pTotOld_` at the reference state |
| R3 fluid `T` <= 1e-9 at all 50 steps, expected 1e-12 to 1e-10; if not, the first failing step is step 1 and a term of `EEqn_1p.H` | **held after R2's fix**; the first run failed from step 2 for the step-size reason, NOT an `EEqn` term (my ranked causes were wrong in kind); measured 2.5e-14, better than the expected range | |
| R4 first `h` residual of step 1 to 1e-4 | **held** (4.7e-10) | |
| R5 `T (avg min max)` <= 1e-12 | **held** (1.5e-15) | |
| R6 power <= 1e-9 | **held after the fix** (6.7e-14) | |
| R7 Courant <= 1e-9 | **held after the fix** (4.8e-13) | |
| R8 loop converges in 1 iteration at 50/50 | **held** | |
| R9 `Re` <= 1e-12 | **held** (1.7e-15) | |

### Findings that are not defects, recorded because they will mislead

* `meshToMesh` between the two identical meshes: weight exactly 1.0 and the identity addressing in all 4 x 8595 entries (measured on upstream),
  not "1 up to an ulp".
* `fluid::rho()` is the constant `rho0` for a Boussinesq EOS: reading `thermo.rho()` there (as `one_phase_fc.rs` does for its EOSs) would put a
  2e-4 x dT density variation into `ddt(alpha rho he)` that upstream does not have.
* `".*"` (`PBiCGStab`) beats `"h.*"` (`smoothSolver`) in `fvSolution` because OpenFOAM takes the LAST matching pattern; the port's older
  `lookup_regex` takes the first. `one_phase_cc.rs` has its own last-match lookup; the older helper is left as it was (it is used by cases whose
  patterns do not overlap).
* The neutronics residual used for the loop test is `max(first-iteration group residuals, neutroResidual history)`, a lower bound of upstream's
  `residual_` (which also takes the initial residuals of every later flux solve); both are far below 5e-5 (<= 2e-6 in all 50 steps of the log).
  The decision "converged after 1 iteration" is therefore the same, but the VALUE of `residual_` is not reproduced.

### What the port still cannot do

* `solveFluidMechanics true` (UEqn/pEqn with `momentumMode cellCentered`, `porousKEpsilon` solving: `k`, `epsilon`, `nut`, `alphat` are only READ):
  the `steadyStateTH` stage (issue 1775, filed as a child). `ReynoldsPower` is parsed and ignored (`Kd` does not enter the energy equation).
* The general `cellVolumeWeight` supermesh (two different meshes; issue 651): `IdenticalMeshMap::new` refuses them by name.
* More than one coupling iteration per step was exercised only by unit tests (upstream never needed it in the 50 steps), and `correctTightlyCoupledPhysics`
  only against `correctPhysics` with one outer iteration, not against upstream.
* Steps beyond 50 (upstream's run goes on to 142 steps at the same protocol); the energy equation's `fixedTemperature` time profile; heat exchangers.

### Gates

**Run (2026-11-26, release):** `cargo test --release -p outram-foam-appbuilder-lib --test genfoam_msfr_coupled_vs_upstream`: 3 passed, 0 failed (the 50-step comparison, `correctTightlyCoupledPhysics` against one outer iteration, the refusal by name); `cargo check --release -p outram-foam-appbuilder-lib --lib`: clean.
**NOT run, not claimed:** the full `cargo test --release -p outram-foam-appbuilder-lib --tests --no-fail-fast`, the crate's `--lib` unit tests (the new ones in `coupled_loop.rs` and `one_phase_cc.rs`), `cargo test --release -p petir --test no_platform_libm`, and the `--no-default-features` (`no_std`) check. The shared target directory's build lock was held for hours by another session's `cargo test -p dhoby-ghaut`, and a private target directory did not get through the dev-dependency build (`faer`, `outram-blender`, `dwsim-libs`) on the machine's remaining capacity and disk. The new files were written to the `no_std` rules (`alloc::`, `core::`, file access under `std`) but that is not checked. To be run by whoever has the machine.

### Moved numbers (old to new, with cause)

None. No earlier gated number depends on a function changed here: only new functions and new files were added (`one_phase_cc.rs`, `coupled_loop.rs`,
two new methods and one new free function in `upstream_transient.rs`).


# Round 10 (gh:#1223 `lookUpTableCHF`, gh:#1224 `CachardLiquid`/`CachardVapour`): `multiRegimeBoilingTRACECHF` and `multiRegimeBoilingVapourTRACE` wired, vs the real binary on closed `1D_CHF` variants

AI-written, not human-reviewed. Research, education and V&V only (`RESPONSIBLE_USE.md`). Upstream: the real unmodified `GeN-Foam`
binary (652b3da, ESI v2506, serial), the same installed build as rounds 3-9.

## What was found before any code was written (read from the shipped tutorials and upstream, 2026-11-26)

| claim | what the sources show |
|---|---|
| gh:#1223 comment: `featureCases/1D_CHF/{imposedPower,imposedTemperature}` use `chfTables` | **No.** Both select `criticalHeatFluxModel { type constantCHF; value 1.66e6; }`; only `1D_PSBT_SC/PhaseII_Ex2_04_6770` and `PhaseII_Ex3_11-0312_power_increase` (byte-identical tables, `cmp`) contain `chfTables`. `1D_CHF` exercises `multiRegimeBoilingTRACECHF` (constantCHF, `GroeneveldStewart` Leidenfrost, `CachardLiquid`) and `multiRegimeBoilingVapourTRACE` (`CachardVapour`) but NOT `lookUpTableCHF`. Its README says "no lookup tables have been implemented yet", which is stale (the class exists, upstream's `porousMedium.md` also still says "empty class"): not our file, noted. |
| the shipped `1D_CHF` cases can run once the closures are wired | **No**, four more needs. (1) Through-flow boundary conditions: `U.liquid` `fixedValue` inlet, `pressureInletOutletVelocity` outlet, `slip` walls, `inletOutlet` alpha, `fixedFluxPressure` `p_rgh` (gh:#1801). (2) The structure power models with time profiles: `imposedPower` is `fixedPower` WITH `powerTimeProfile` (the two-phase reader refuses it: "not wired") and `imposedTemperature` is `fixedTemperature` with `temperatureTimeProfile` (the two-phase solver has only `fixedPower`). (3) Turbulence: both `turbulenceProperties.*` say `simulationType laminar` (the `LaheyKEpsilon` block is inert), so gh:#1802 is NOT needed here. (4) Run length: 77 s / 180 s of simulated time with the hysteresis ramp, far beyond what a tight-protocol step-by-step comparison can cover (upstream's own PSBT run aborts with SIGFPE after 417 steps). |
| upstream `CachardLiquid`/`CachardVapour` bind `pi` to pressure | Confirmed by reading: `CachardLiquid...C:124` (`const scalar& pi(p_[celli])`) and `CachardVapour...C:96`, then `pow(pi/max(DRi,1e-6),2)`: `(p/0.01)^2 = 2e18` at 15 MPa. The film thickness `delta` is ~1e9 m, so `h_gamma = kappa_g/delta` is ~1e-11 and the post-CHF coefficient of the liquid is only the radiation term. |

So, as in round 8, the shipped cases cannot run; the comparison is on **closed variants** (`tests/data/genfoam_chf_closed_gh1361`, `PROVENANCE.md`
there): the shipped `imposedPower` mesh, models and initial fields; boundary conditions as in the round-8 closed PSBT case (zero-gradient
velocities and `alpha`, fixed `p_rgh` at both ends with a 1.2e4 Pa drive); **constant** `powerDensity 5e9` instead of the shipped 5e8-3e9 ramp (the
profile is not wired, above), chosen so the wall crosses the critical heat flux within the first few steps. Two variants:

* `constant_chf`: the shipped `constantCHF 1.66e6`, `GroeneveldStewart`, `CachardLiquid`, `CachardVapour`, contact partition 1.0 as shipped. Because the shipped
  `contactPartitionModel` gives the liquid all of the contact, the vapour-side htc is multiplied by 0 and `VapourTRACE` is invisible in the written fields.
* `lookup_table_chf`: the same with `lookUpTableCHF` (the PSBT table and axes, `PitchToDiameter 1.32631578947`, `#include "chfTables"`) and contact partition 0.7
  so that `VapourTRACE` shows. **Not a shipped case in any respect**; a code-to-code comparison only.

## What was searched before writing anything (CLAUDE.md "search before building")

- `grep -rniE "lookUpTable|chfTables|CachardLiquid|CachardVapour|InterpolateTable|hGammaCachard"` over `crates/`: the 1-D `ScalarInterpolateTable`
  (`common/interpolate_table.rs`, bit-identical to upstream's 1-D table, round 3 "libCommon driver"), the pure functions `multi_regime_boiling_trace_chf` /
  `multi_regime_boiling_vapour_trace` (round 8), `LeidenfrostTemperature::GroeneveldStewart`, `CriticalHeatFlux::Constant`. Reused. Not present: the `Field`/`FieldField`
  instantiations of the table (new `common/interpolate_table_nd.rs`), `lookUpTableCHF::value`, the Cachard models, the solver wiring and `#include` following in `TwoPhaseCase::read`.
- `crates/outram-foam-multiphase/src/chf.rs` has a `GroeneveldLut` (an 8 mm tube, its own table) - a different model and data, not used: upstream's `lookUpTableCHF` is the specification.
- Upstream read in full: `lookUpTableCHF.{H,C}`, `InterpolateTableGF.H`, `InterpolateTablesGF.{H,C}`, `PtrListScalarFieldFieldINewGF.H`, `CHFModel.C`, `constantCHF.C`,
  `CachardLiquid...C`, `CachardVapour...C`, `multiRegimeBoilingTRACECHF...C`, `multiRegimeBoilingVapourTRACE...C`, `GroeneveldStewartTLF.C`, `fixedPower.C`, `fixedTemperature.C`, `powerModel.C`.
  No kovan-reviewed function was touched (`heat_transfer/review.md`, `two_phase` has no review block).

## What was ported (new code; the old forms stay callable)

* `common/interpolate_table_nd.rs`: `InterpolateTableGF::operator()` over `f64`, `Vec<f64>`, `Vec<Vec<f64>>` (one generic function; upstream's linear walk, `(1-alpha) a + alpha b` per element).
* `closures/heat_transfer/chf_table.rs`: `LookUpTableChf::{from_dict, value}` = `lookUpTableCHF.{H,C}` (`ERROR` out-of-bounds policy as upstream's default: an `Err` naming the cell).
* `closures/heat_transfer/cachard.rs`: `CachardLiquid`, `CachardVapour`. **Default = upstream, bit for bit, including the pressure-as-pitch defect**; the intended reading is the named option `CachardPitch::Pitch` / `with_pitch` (maintainer decision pending: gh:#1224).
* `solver/two_phase`: `FsHtcModel::{MultiRegimeBoilingTraceChf, MultiRegimeBoilingVapourTrace}`, fields `chf_bool`, `wf_transition_boiling`, `t_leidenfrost` (what upstream's `CHFBoolMultiRegimeBoilingTRACECHF`, `wfTransitionBoiling`, `LeidenfrostTemperature` hold; upstream writes none of them), `TwoPhaseModels::chf_tables`; `TwoPhaseCase::read` follows `#include` in `phaseProperties` (`FoamFile::read_with_includes`); the liquid pair is evaluated before the vapour pair in `correct_fs_pair` as upstream's pair order, so the vapour model reads the fields of the same correction.
* the data: `tests/data/genfoam_chf_gh1223/chfTables` (bit-identical, `PROVENANCE.md`: GeN-Foam 652b3da, GPL-3.0, Groeneveld et al. 2007; DOI not verified).

## Predictions (written BEFORE the first port run of this round; committed first)

Upstream facts measured first (real binary, tight protocol, every step written, `scripts/chf_closed_run.sh`, endTime 0.3, serial): `constant_chf` runs 86 steps and exits
normally (no SIGFPE); `lookup_table_chf` 226 steps, normally. The wall (`T.fixedPower`) is at ~626 K at step 1 (`T_sat` ~ 615 K) and rises ~3.7 K per step; `htc.liquid.structure`
is ~25000 W/m2K at step 3 in `lookup_table_chf` and ~500 afterwards (the film-boiling form), so the regime changes within steps 3-6. `alpha.vapour` first exceeds 1e-3 at step 4.

| id | quantity | prediction | cause if missed |
|---|---|---|---|
| U1 | both variants read with an empty `deviations`; `chfTables` follows the `#include` | held | a key the reader refuses |
| U2 | `dt`, steps 1-30, both variants | equal to upstream's to 1e-9 relative | Courant / `maxDeltaT` |
| U3 | steps 1-3 (pre-CHF, nucleate branch with `dmdtW`): `p`, `T.*`, `U.*`, `htc` | <= 1e-6 relative to the field maximum (round 8 found 7.8e-9 at its first nucleate step, grown 10x/step; the wall here heats 3.7 K/step, so faster growth) | the unexplained round-8 nucleate-boiling difference |
| U4 | the step at which `htc.liquid.structure` first leaves its pre-CHF value (the CHF crossing) | the same step as upstream, in both variants | `T_CHF`, `q_CHF` (table), `T_LF` |
| U5 | film-boiling steps (after the transition): `htc.liquid.structure` (= radiation + the ~1e-11 `h_gamma`) | <= 1e-8 relative (a closed form of `T_wall`, `T_sat`) | `Twall` lag, `T.interface` |
| U6 | `lookup_table_chf`: `htc.vapour.structure` (`2 k_g/delta` with `delta` ~ 1e9: ~1e-9 W/m2K) | <= 1e-6 relative once post-CHF, 0 before | the contact partition, `kappa.vapour` placeholder (round 8) |
| U7 | `alpha.vapour`, `dmdt.liquid.vapour`, steps 4-20 | <= 1e-3 relative (the round-8 vapour onset was 2e-3 / 2.8e-4) | `dmdtW` post-CHF branches |
| U8 | first step at which any compared field leaves 1e-6 | step 3 to 8 (the CHF crossing amplifies the round-8 difference); none at 1 or 2 | |
| U9 | the table: `lookUpTableCHF::value` on the 3-D table itself | bit-identical to upstream's class on a stand-alone driver (no run needed) | not run in this round: a libCommon-style driver for `lookUpTableCHF` needs `FSPair` and a mesh; deferred, recorded below |

If U3 or U8 miss the first suspect is the round-8 nucleate-branch difference (sensitivity -1e-2/K to `T_ONB`), second the Gorenflo `T_CHF` evaluation
(`3.72*pow(9.0,2)` against `3.72*9.0*9.0` in the Leidenfrost ramp, an ulp), third `kappa` of the vapour (the Cachard liquid model reads `kappa.vapour`, which upstream refreshes at the end of `EEqns`).
