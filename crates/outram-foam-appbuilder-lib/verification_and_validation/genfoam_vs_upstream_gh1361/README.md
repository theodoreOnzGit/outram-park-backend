# GeN-Foam port vs upstream GeN-Foam 652b3da0cc55, differential comparison (gh:#1361)

AI-written, not human-reviewed. Research, education and V&V only
(`RESPONSIBLE_USE.md`). Round 1 covers plan steps 2 and 3 of the scoping
comment on gh:#1361. Step 4 (ESI thermo/turbulence closure) and step 5 (the
full GeN-Foam binary) are not started.

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
Any comparison against a shipped log is limited to that. S4 (iteration history): not compared.
The upstream history (230 pseudo-time steps at the tight protocol) is committed in the reference; the port
has no driver that updates keff once per step. A port of that driver (`solveNeutronics.H` eigenvalue branch,
`calcKeff.H`, `setDeltaT`/`maxDeltaT`) is the remaining work for S4.

S6, Godiva_SN: not compared (blockers above). The real binary runs it as shipped (P3 scattering, its own
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

1. **S4, the iteration history.** Port the eigenvalue branch of `solveNeutronics.H` + `calcKeff.H` + `normFluxes.H`
   and `setDeltaT`/`maxDeltaT` as a pseudo-time driver, with basic-lib's PCG/DIC at the case's own solver
   settings, then compare keff, power and the first residual per step against the 230-step history already
   committed in `refs/guided_slabs.ref.txt.gz` (and `sp3_slabs`). Without it the "iteration history" is
   compared only as the converged end state.
2. **Godiva_SN (S6).** Needs a reader for `constant/neutroRegion/quadratureSet` (the port bakes level-symmetric
   sets) and P1 to P3 scattering moments (gh:#1474). The upstream binary runs the case as-is; the first
   pseudo-time step takes several minutes at 3.2 GB resident, 20 steps are shipped. A `legendreMoments 0`
   run with the case's quadrature is the first identical-input comparison once the reader exists.
3. **SP3 on the ESFR** and the transient (`step`) paths, which need the TH state of the ESFR run.
4. **Plan steps 4 and 5** (ESI thermo/turbulence closure, the full binary): not started, as instructed.
5. **Singular-matrix behaviour** of `lu_decompose` against ESI (noted above) if a caller ever relies on it.
