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

(Results follow below once the runs are made.)
