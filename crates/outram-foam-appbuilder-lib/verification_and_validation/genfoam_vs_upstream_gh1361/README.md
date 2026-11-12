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
| S5 | SP3 on the same two slabs (the tutorials' `0.orig` already holds `defaultFlux2`; only `regionSolvers { neutroRegion SP3Neutronics; }` was changed), real binary at the tight protocol: k_eff and flux per cell, port `Sp3Neutronics::solve_eigenvalue` with the same boundary conditions on both moments (top/bottom fixedValue 0, walls zeroGradient). Upstream: case 1 `keff_ = 1.0078490573983063`, case 2 `1.0110843916601269` (last digits jitter by 9e-16) | the port was written from `SP3Neutronics`, same discrete system: `abs(dk) < 1e-9`, flux shape max relative difference `< 1e-7`. If it misses, the first suspect is the second-moment boundary treatment (upstream reads `defaultFlux2` per patch; the port applies one `BoundaryCondition` to both moments). SP3 (1.0078) is above diffusion (1.0000) because SP3 transports the leakage with a smaller effective diffusion at these dimensions: a physics sanity check, not a comparison. |
| S6 | `reactorCases/Godiva_SN` (23 groups, `legendreMoments 3`, `quadratureSet` of 4 directions per octant, unstructured sphere mesh) | **not comparable on identical inputs this round**, two blockers read from the port source before any run: (a) the port's `AngularQuadrature` is baked-in level-symmetric (`quadrature.rs:18`: "this port bakes the standard sets") and has no reader for the case's `quadratureSet`; (b) anisotropic scattering moments P1 to P3 are not implemented (gh:#1474). Running the port on its own quadrature and P0 would be a different problem, not a comparison, so it is not run. The upstream binary was run for the record (below). |
