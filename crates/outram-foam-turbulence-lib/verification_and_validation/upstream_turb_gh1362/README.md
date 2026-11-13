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

## Fields were changed once after the first run (disclosed)

The first run used an omega field that made `grad k . grad omega < 0` in all 360 cells,
so the positive-cross-diffusion branch of SST (`CDkOmega > 1e-10`) was never exercised
(and F1 saturated at 1 almost everywhere, hiding the `tanh` libm difference). The omega
polynomial was changed three times, only to put cells on both sides of each SST branch
(`scripts/sst_coverage_stats.py`: nut limiter active in 82 cells, `Pk` and `GbyNu`
limiter active in 2 cells, `CDkOmega > 1e-10` in 32, F1 in (0.01, 0.99) in 138, F1 = 1 in
148). No comparison value was looked at to choose it. All numbers below are from the final
fields. Limitation that remains: the `Pk`/`GbyNu` limiter is active in only 2 cells.

## Results (2026-11-13, release; 360 cells, duct 4 x 1 x 0.5, graded in y)

**Audit of the transcription (prediction 8: held).** For every model the transcribed
equation reproduces the matrix the REAL model assembled, bit for bit: diag, source, upper,
lower (360/360 and 894/894), for epsilon, k (k-epsilon), omega, k (k-omega), omega, k (SST),
nuTilda (SA). The model's own registered fields (`kEpsilon:G`, `kEpsilon:GbyNu`,
`kOmega:G`, `kOmegaSST:G/gamma/beta/GbyNu`) equal the transcribed ones bit for bit.
Smagorinsky has no matrix (algebraic): its transcription is checked against the real `nut`.

**ESI form (`SourceForm::EsiV2506`), UPSTREAM gradients and wall distance fed in:**
k-epsilon, k-omega, SA: every term 360/360 bit-identical (nut, G, GbyNu, E, Sp, SuSp, chi, fv1,
fv2, Omega, Stilda, r, fw, gradSqr). SST: y, nut, S2, CDkOmega, GbyNu0, G, GbyNu, Pk, k terms
360/360; the terms downstream of `tanh` (F1 334/360, F23 339/360, gamma 356, beta 358,
omega_E 357, omega_SuSp1 358, omega_Sp 358, omega_SuSp2 343) differ by at most 3 ulp
(PETIR `tanh` against glibc; a python glibc replay of upstream's F1 and F23 from the dumped
inputs reproduces them 360/360, `scripts/sst_tanh_replay.py`). Smagorinsky: with upstream's
delta fed in, delta/k/nut 360/360; with the port's own `cbrt` (PETIR), delta 156/360 (2 ulp),
k 189/360 (8 ulp), nut 170/360 (7 ulp): libm only.

**Default (`SourceForm::Port`) against upstream, identical/360 and worst relative difference
(upstream gradients; for SST/SA with upstream y):**

| term | k-epsilon | k-omega | SST | SA |
|---|---|---|---|---|
| nut | 267 (4e-16) | 360 | 360 | 360 |
| GbyNu / G / production | 0, 8.0e-2 | 0, 8.0e-2 | 0, 8.0e-2 (GbyNu0) | n/a |
| explicit omega/eps source | 0, 8.0e-2 | 0, 8.0e-2 | 0, 6.5e-1 (limiter missing) | 360 (E) |
| SuSp (div U) terms | absent, 1.0 | absent, 1.0 | absent, 1.0 | n/a |
| implicit Sp | eps 231 (2e-16), k 360 | 360, 360 | omega 294 (3e-16), k 360 | 0, 7.5e-6 |
| gamma | n/a | n/a | 0, 4.3e-3 | n/a |
| F1 / F23 | n/a | n/a | 268 (10 ulp) / 316 (4 ulp) | n/a |
| CDkOmega | n/a | n/a | 23, 1.0 (clip at 1e-10) | n/a |
| Cb2/sigma term | n/a | n/a | n/a | 0, 1.0e-5 |

Wall distance: port (nearest wall-face centre) against `meshWave`: 265/360 identical, the
other 95 cells larger by up to 8.9 %. With the port's own y the SST F1 differs up to 4.5e-1,
F23 1.5e-1, gamma 3.1e-2, SA fw 2.1e-1 and the SA `Sp` term 4.6e-1 (rows "(own y)" in the test).

### Predictions that held, and the ones that did not

- Held: 1 (k-epsilon nut to ulp: 267/360 within 2 ulp; k-omega nut 360/360 identical), 2 (G
  differs structurally, 8.0 % worst), 3, 4, 7, 8; the SST gamma difference (4.3e-3) and the
  CDkOmega clip.
- Wrong: 5 said the port's wall distance "differs for most cells": 265/360 are identical,
  the rest by up to 8.9 %. It also said the port's F2/nut differ for lack of `F23`; with F3
  off (the v2506 default) the port's nut is bit-identical to upstream's given the same y.
  The port's SST `Pk` limit equals upstream's (`10 beta* k omega`).
- Wrong: 6 said the SA sigma difference is ~5e-6; sigma differs by 1e-5 (0.66666 against
  2/3) and the effect on `Cw1` and the `Sp` term is 7.5e-6, on the `Cb2/sigma` term 1.0e-5.
- Not predicted: the size of the G difference (8.0 %, because the test field has |div U|
  up to 0.33), and that the SST omega production is up to 65 % away in the 2 limited cells.

### Defects (port against ESI v2506; nothing fixed in `correct()`; each is a maintainer decision)

1. `G` omits `-(2/3)(div U)^2` relative to `GbyNu = gradU && devTwoSymm(gradU)` (k-epsilon,
   k-omega, SST): equal only for divergence-free `U`. Predicted effect of the fix:
   zero on divergence-free fields, O(|div U|^2 / |S|^2) otherwise.
2. The `SuSp` terms `((2/3) C div U)` in every k, epsilon, omega equation are absent
   (same remark). The port's docs state this ("single-phase incompressible reduction").
3. SST: gamma1/gamma2 derived (0.5532, 0.4403) against 5/9, 0.44 (4.3e-3 in gamma); no
   `GbyNu` limiter in the omega production; `CDkOmega` clipped at 1e-10 in the source
   (upstream clips only inside F1); the cross-diffusion is added explicitly rather than as
   `SuSp((F1-1) CDkOmega/omega)`; wall distance to the nearest wall-face centre.
4. SA: sigma 2/3 against upstream's literal 0.66666 (1e-5); wall distance as above.
5. k-epsilon: the port bounds epsilon at `Cmu k^2/(1e5 nu)` before forming nut and G;
   upstream forms nut from raw epsilon and bounds epsilon only after its solve
   (`epsilonMin = small`). Inactive on these fields (epsilon >= 0.105 against a bound
   <= 1e-3): the hypothesis of the previous round is a structural difference that this
   field set cannot show as a number; it is NOT measured.
6. The port's `correct()` is a transient solve with its own `SolverSettings`; this record
   compares the terms it assembles, not its solution.
7. `epsilonWallFunction`, `omegaWallFunction`, `kqRWallFunction`: absent. Not small (they
   modify the values and G of wall-adjacent cells through the patch field and need the
   cell-to-patch addressing and corner weights the port's patch fields do not carry):
   filed as a follow-up on gh:#1362, not ported.

### Options added (defaults unchanged)

`SourceForm::{Port, EsiV2506}`, `FrozenState`, `SourceTerms` and
`KEpsilon/KOmega/KOmegaSST/SpalartAllmaras/Smagorinsky::source_terms`, plus
`KOmegaSST::wall_distance`, `SpalartAllmaras::wall_distance`. `correct()` is not
changed and does not call them. Would making the ESI form the default move downstream
numbers? Yes for any consumer with `div U != 0` (G and `SuSp`), for SST in every cell
through gamma (up to 4.3e-3) and the `GbyNu` limiter, and for SA through sigma (1e-5);
which consumers use these models was NOT measured.

### Not covered

The matrix totals of the port (its `fvm::ddt/div/laplacian` assembly) were not compared with
the real matrices: the port assembles them inside `correct()`. A bit-level comparison of the
solved fields would need a transient comparison with matching linear-solver settings.
Foundation (dev) was not read or run. Periodic and AMI patches are not covered.
