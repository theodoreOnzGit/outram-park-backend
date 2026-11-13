# Turbulence models against OpenFOAM-v2506 (ESI), gh:#1362 row 5, slice 2

Upstream line: ESI v2506 only (unmodified `libturbulenceModels`,
`libincompressibleTurbulenceModels`). Foundation (dev) was not read or run.

## Method

`driver/turbDump.C` (+ `outram-foam-basic-lib/.../upstream_solvers_gh1362/driver/dumpSolver`,
an lduMatrix solver that prints the matrix it is handed instead of solving) runs
the REAL model per case: a 12x10x3 graded blockMesh duct (360 cells, orthogonal),
polynomial frozen fields (no libm) for U, phi, k, epsilon, omega, nuTilda, all
scalars zeroGradient, nu = 1e-5, Euler dt 0.25 (binary exact, so ddt adds no
rounding). It calls `validate()` (real `correctNut`) and `correct()`; the
dumpSolver prints each assembled scalar matrix and every registered internal
field alive inside `correct()`. The driver also TRANSCRIBES each model's source
terms from the upstream `.C` using the real fvc/fvm operators on the real fields
(named, printable per-cell terms) and AUDITS the transcribed equation bit for bit
against the matrix the real model assembled (`audit` lines). The Rust test then
compares the port per cell against: real `nut`, and the audited source terms.

The port exposes the same terms through new `source_terms*` functions (see below),
fed with the UPSTREAM gradients so that the model formulas are isolated from the
operator ulp orderings of gh:#1489.

Fields are chosen to exercise code paths (divU != 0, SST limiters active in part
of the core), not to hit any value.

## Predictions (written before the first run; Rust side not yet written)

1. **nut** (k-epsilon, k-omega): the port's epsilon bound `Cmu k^2/(1e5 nu)` is
   inactive on these fields (epsilon >= 0.1 against a bound <= 1e-3), so nut agrees
   to ulp (`(Cmu k) k` against `Cmu (k k)`), not bit for bit.
2. **G**: upstream `GbyNu = gradU && devTwoSymm(gradU)` carries `-(2/3)(div U)^2`
   relative to the port's `2 S:S`: the port differs structurally (>1e-10, order
   1e-2 where |div U| is large) in k-epsilon, k-omega and SST; in SA none.
3. **divU terms**: upstream `SuSp((2/3)..divU)` terms do not exist in the port:
   structural in every equation of k-epsilon, k-omega, SST.
4. **k-omega omega production**: port `gamma G omega/k`, upstream `gamma GbyNu`:
   algebraically equal where divU = 0, differing at the `(2/3) divU^2` term and ulps.
5. **SST**: port gamma1/gamma2 derived 0.5532/0.4403 against upstream 5/9, 0.44;
   port wall distance = distance to the nearest wall FACE CENTRE against meshWave:
   differs for most cells (up to >10 % near walls, port larger); port clips CDkOmega
   at 1e-10 in the omega source (upstream only inside F1); port F2 uses the
   un-damped nut formula without upstream `F23` in `GbyNu`; port omega production
   `gamma |S|^2` has no `min(GbyNu0, ...)` limiter. All structural.
6. **SA**: port sigma = 2/3, upstream 0.66666: ~5e-6 relative in Cw1, the
   diffusivity and the Cb2/sigma term; y differs as in 5. Omega, fv1, fv2 equal to ulp.
7. **Smagorinsky**: delta, k, nut equal to ulp (libm `cbrt` only).
8. **Audit**: every transcription reproduces the real matrix diag/source bit for bit.

(Results and corrections are appended below.)
