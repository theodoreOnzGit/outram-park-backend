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

---

# Slice 3 (gh:#1377): realizableKE, RNGkEpsilon, WALE

Models picked from the gh:#1377 list by size and use: `realizableKE.C` 340 lines,
`RNGkEpsilon.C` 331, `WALE.C` 191 (the smallest in each family, and the ones the issue
names as priority for HX/pipe demos and LES). Not reached in this slice (filed as
children of gh:#1377): `LaunderSharmaKE`, `kOmegaSSTLM`, `kEqn`, `dynamicKEqn`, the DES
family, `LRR`/`SSG`, `v2f`, `kOmega2006`.

Same frozen duct, same fields, same driver (`turbDump.C`, models `realizableKE`,
`RNGkEpsilon`, `WALE` added). The new Rust models have ONE arithmetic: the upstream one
(there is no pre-existing "port form" to keep), so the comparison is ESI-form only, and
the table below counts identical cells per term.

## Predictions (written and committed BEFORE the driver was extended or run)

1. **RNGkEpsilon**: uses only `+ - * / sqrt mag` (no transcendental function). Every
   term (nut, GbyNu, G, eta, R, eps E/SuSp/Sp, k E/SuSp/Sp) is **360/360 bit-identical**
   when fed upstream's `grad(U)` and `div(phi)`.
2. **realizableKE**: `S2`, `magS`, `GbyNu`, `G`, `eta`, `C1`, eps `E`, eps `Sp`, k terms:
   **360/360 bit-identical** (only `+ - * / sqrt`). The chain `W -> phis -> As -> rCmu ->
   nut` goes through `acos` and `cos`: PETIR against glibc libm will differ in a minority
   of cells; predicted at most 3 ulp and at least 70 % of cells identical in `rCmu`
   (the `acos` argument is clamped to [-1,1]; the test field may saturate it, which would
   make `phis` exactly 0 or pi/3 and identical).
3. **WALE** (upstream `delta` fed in): `Sd`-chain terms (`magSqrSd`, `magSqr(symm)`) are
   bit-identical (no libm). `pow(x, 5/2)` and `pow(x, 5/4)` differ in a minority of cells
   by at most 2 ulp (PETIR `powf` against glibc `pow`), so `k` and `nut` differ in the
   same cells by at most 3 ulp; at least 70 % of cells identical. With the port's own
   `cbrt` delta, the delta/k/nut differences are those of Smagorinsky (delta 156/360).
4. No cell is expected to differ by a formula error; any difference that is not
   explained by a libm function is a defect in the port, to be localised and fixed by
   translating upstream, not by a tolerance.

## Slice 3 results (2026-11-13, release; same 360-cell duct, upstream `grad(U)`, `div(phi)` fed in)

**Audit of the transcription: held.** For RNGkEpsilon and realizableKE the transcribed
epsilon and k equations reproduce the matrices the REAL model assembled bit for bit (diag,
source, upper, lower: 360/360 and 894/894; 8 audit lines each). WALE is algebraic (its
transcription is checked against the real `nut`, below).

| model / term | identical / 360 | max ulp | note |
|---|---|---|---|
| RNGkEpsilon: nut (real and transcribed), GbyNu, G, eta, R, eps E/SuSp/Sp, k E/SuSp/Sp | 360 each | 0 | no libm |
| realizableKE: S2, magS, W, Us, eta, C1, G, eps E/Sp, k E/SuSp/Sp | 360 each | 0 | no libm |
| realizableKE: phis (`acos`) | 353 | 2 | PETIR `acos` against glibc |
| realizableKE: As (`cos`) | 344 | 1 | PETIR `cos` against glibc |
| realizableKE: rCmu | 346 | 3 | inherited from `As` |
| realizableKE: nut (real `validate()` and transcribed) | 346 | 4 | inherited from `rCmu` |
| WALE, upstream delta fed in: magSqrSd, magSqrS, pow52, k, nut | 360 each | 0 | |
| WALE, upstream delta fed in: pow54 (`pow(x, 5/4)`) | 358 | 1 | PETIR `powf` against glibc |
| WALE, own delta (`cbrt`): delta / k / nut | 156 / 185 / 173 | 2 / 7 / 6 | same `cbrt` as Smagorinsky |

All counts are pinned in `tests/openfoam_turbulence_code_to_code.rs` (both directions) with
ulp ceilings (realizableKE 4, WALE with upstream delta 1, with own delta 7).

### Predictions against results

- Held: 1 (RNG 360/360 everywhere); 2 for the libm-free terms; the libm chain differs by at
  most 3 ulp in `rCmu` and 96 % of cells are identical (>= 70 % predicted); 4 (every
  difference is explained by a libm function; no formula defect found).
- Off: 2 said "at most 3 ulp", but `nut` reaches 4 ulp (it multiplies `rCmu` by `k^2/epsilon`
  and so inherits and adds one rounding). 3 predicted `pow(x, 5/2)` would differ in a
  minority of cells; it is 360/360 identical on this field; only `pow(x, 5/4)` differs (2
  cells, 1 ulp), and `k` and `nut` are 360/360 identical with upstream's delta, so the
  predicted "same cells, up to 3 ulp" did not occur.

### Defects found

None in the formulas. Documentation defects fixed in the same change (strike-through in
place): `src/prelude.rs` said only `KOmegaSST` works and the others `todo!()`-panic, and
`src/les/mod.rs` said `Smagorinsky` is a scaffold; `grep todo! src/` finds neither.

### Honest scope of the new models

`source_terms` is compared with upstream; `correct()` is NOT. Known differences of
`correct()` from upstream: no wall functions (zero-gradient walls), `bound()` is a plain
floor at `small` (upstream averages neighbouring cells below the limit), no `fvOptions`,
no relaxation, the port's own `SolverSettings`. The new models have a single arithmetic
(upstream's); the older models' `SourceForm::Port` split is not repeated for them.

---

# Slice 4 (gh:#1545, gh:#1544 under gh:#1377): DES family

Models in this slice, in the order given by the maintainer: `SpalartAllmarasDES`,
`SpalartAllmarasDDES`, `SpalartAllmarasIDDES`, `kOmegaSSTDES`. Same frozen duct, fields and
driver; wall distance is ESI's `meshWave` (`wallDist::y()`), the LES width is ESI's
`cubeRootVol` (`delta`, `deltaCoeff 1`), both read from the dump and fed to the port, as in
slice 3. SA-IDDES additionally needs `hmax` (from `IDDESDelta`'s `maxDeltaxyz`) and the
`IDDESDelta` width: both fed from the dump (the delta classes themselves are NOT ported in
this slice). `useSigma true` (the `Ssigma` invariants branch) is NOT ported in this slice.

## Predictions (written and committed BEFORE the driver was extended or run)

1. **SpalartAllmarasDES** (`useSigma false`, `lowReCorrection true`, `CDES 0.65`,
   `fwStar 0.424`): `chi`, `fv1`, `fv2`, `psi` (`sqrt` only), `lLES`, `dTilda` (`min`),
   `Omega`, `Stilda`, `r`, `gradSqr`, `nuTilda_E`: **360/360 bit-identical** when fed
   upstream's `grad(U)`, `grad(nuTilda)`, `y`, `delta`. `fw` (one `pow(x, 1/6)`) and the `Sp`
   coefficient that contains it: at least 98 % identical, at most 1 ulp.
2. **SpalartAllmarasDDES** (standard shielding, `Cd1 8`, `Cd2 3`): `fd = 1 - tanh(pow(Cd1 r, Cd2))`
   goes through libm `pow` and `tanh`: at least 90 % of cells identical in `fd`, at most 2 ulp.
   `dTilda`, `Stilda`, `r`, `fw` and the equation terms differ only in cells where `fd`
   differs (a cell where `fd` is bit-identical must be bit-identical downstream).
   The test field must put cells on both sides of the switch (`lRAS - lLES` of both signs;
   `fd` strictly between 0 and 1 in a minority of cells); if it does not, the field is
   changed (and disclosed here) to exercise the branch, never to hit a value.
3. **SpalartAllmarasIDDES** (`fe true`, `Cdt1 8`, `Cdt2 3`, `Cl 3.55`, `Ct 1.63`): adds `exp`,
   `pow(.., -9)`, `pow(.., -11.09)`, `pow(.., 10)`, `tanh`: at least 85 % of cells identical in
   `fdTilda`/`dTilda`, at most 3 ulp. `alpha`, `lLES`, `fB`-independent algebra with no libm
   (`r`, `psi`, `alpha`): 360/360.
4. **kOmegaSSTDES** (`useSigma false`): `F1` and `F23` reuse the SST arithmetic whose `tanh`
   difference against glibc is already pinned (SST: F1 334/360, F23 339/360, at most 3 ulp,
   with upstream `y`); the DES terms inherit those cells only: `CDES = F1 (0.82 - 0.6) + 0.6`,
   `lLES`, `dTilda = min(lLES, lRAS)`, `epsilonByk = sqrt(k)/dTilda` are bit-identical in every
   cell where `F1` is, and `lRAS = sqrt(k)/(betaStar omega)`, `GbyNu0`, `GbyNu` (= `GbyNu0`,
   unlimited), `G`, `Pk`, `S2`, `CDkOmega` are 360/360. `nut`: 360/360 as in SST (`F23` ulps
   do not reach `nut` there).
5. **Audit**: every transcribed matrix reproduces the matrix the REAL model assembled, bit for
   bit (diag, source, upper, lower); a failure is a transcription error, not a model result.
6. No difference not explained by `tanh`, `exp` or `pow` is expected; any such difference is
   a defect in the port, localised and fixed by translating upstream, not by a tolerance.

## Slice 4 results (2026-11-13, release; same 360-cell duct; upstream `grad(U)`, `grad(nuTilda)`, `grad(k)`, `grad(omega)`, `div(phi)`, `y` (meshWave), `delta`, `hmax` fed in)

**Audit of the transcription: held** for all four models (SA family: `nuTilda` diag/source 360/360,
upper/lower 894/894; kOmegaSSTDES: omega and k, same counts). **Regime coverage** of the frozen
field (counted from the dump, not tuned: the field was NOT changed for this slice): LES region
(`dTilda < y`) in 72 (DES, DDES), 48 (IDDES), 162 (kOmegaSSTDES) of 360 cells; DDES `fd` in
(0.01, 0.99) in 72 cells, <= 0.01 in 8, >= 0.99 in 280; IDDES `fe > 0` in 312 cells; SST-DES `F1` in
(0.01, 0.99) in 138 cells. Both sides of every switch are exercised.

| model / term (upstream y, delta) | identical / 360 | max ulp | note |
|---|---|---|---|
| SA-DES: every term (`psi`, `lLES`, `dTilda`, `Stilda`, `r`, `fw`, `nuTilda_E`, `nuTilda_Sp`, `LESRegion`, `nut`) | 360 each | 0 | |
| SA-DDES: `r_fd`, `lLES`, `dTilda`, `Stilda`, `r`, `fw`, `nuTilda_Sp`, `LESRegion` | 360 each | 0 | |
| SA-DDES: `fd` (`pow`, `tanh`) | 355 | 4 | PETIR against glibc; a glibc replay of `1 - tanh(pow(8 r, 3))` on the dumped `r` reproduces upstream 360/360 |
| SA-IDDES: `alpha`, `fB`, `ft`, `fl`, `fe1`, `fe`, `lLES`, `dTilda`, `fw`, `nuTilda_Sp` | 360 each | 0 | |
| SA-IDDES: `fdt` (`pow`, `tanh`) | 343 | 2 | does not reach `fdTilda = max(1 - fdt, fB)` or `dTilda` |
| kOmegaSSTDES: `S2`, `CDkOmega`, `nut`, `lRAS`, `GbyNu0`, `GbyNu`, `G`, `Pk`, `LESRegion`, `k_E`, `k_SuSp` | 360 each | 0 | |
| kOmegaSSTDES: `F1` / `F23` (`tanh`) | 334 / 339 | 2 / 1 | same counts as plain SST (slice 2) |
| kOmegaSSTDES: `CDES` / `lLES` / `dTilda` / `epsByk` = `k_Sp` | 356 / 357 / 359 / 359 | 1 / 2 / 1 / 1 | inherited from `F1` |
| kOmegaSSTDES: `gamma` / `beta` / `omega_E` / `omega_Sp` / `omega_SuSp1` / `omega_SuSp2` | 356 / 358 / 357 / 358 / 358 / 343 | 1 / 1 / 1 / 1 / 3 / 3 | inherited from `F1` |
| own `cbrt` delta (SA-DES / SA-DDES): `delta` | 156 | 2 | same `cbrt` as Smagorinsky/WALE |
| own `cbrt` delta: `lLES` / `r` / `fw` / `nuTilda_Sp` (DES) | 186 / 336 / 337 / 335 | 3 / 4 / 6 / 10 | the `delta` ulps amplify through `r^6` in `fw` |
| own `cbrt` delta: `lLES` / `r` / `fw` / `nuTilda_Sp` (DDES) | 186 / 341 / 341 / 341 | 3 / 7 / 9 / 14 | |

All counts are pinned (both directions) in `tests/openfoam_turbulence_code_to_code.rs`
(`sa_des_vs_openfoam`, `sa_ddes_vs_openfoam`, `sa_iddes_vs_openfoam`,
`k_omega_sst_des_vs_openfoam`) with ulp ceilings 10 (SA-DES own delta; 0 with upstream delta),
14 (SA-DDES own delta), 2 (IDDES), 3 (kOmegaSSTDES).

### Predictions against results

- Held: 1 (SA-DES 360/360 everywhere, including `fw`: >= 98 % predicted), 3 for the libm-free terms,
  4 (kOmegaSSTDES: terms inherit the SST `F1` cells only; `nut` 360/360), 5 (audits), 6 (every
  difference is `pow`/`tanh` or, with the port's own delta, `cbrt`).
- Better than predicted: 2 and 3 said >= 90 % / 85 % of cells identical in `fd` / `dTilda`; `fd` is
  355/360 and every downstream DDES term is 360/360 (not localised cell by cell why the 5 differing
  `fd` cells do not reach `dTilda`; the likely reason is that `max(y - lLES, 0)` is 0 there).
  IDDES `fdt` is 343/360 but `dTilda` 360/360.
- Off: 2 said "at most 2 ulp" for `fd`; it is 4 ulp (`pow(x, 3)` then `tanh` compound).

### Defects found

None in the formulas (no difference that a libm function does not explain).

### Honest scope

Terms only: no `correct()`, no `TurbulenceModel` impl for the DES family (the SA transport
solve and SST step exist for the plain models; wiring `dTilda` into them is the next step).
Not ported: `useSigma true` (`Ssigma`: needs `acos`/`cos` and tensor invariants), `shielding
ZDES2020`, `ft2 true`, `kOmegaSSTDDES` / `kOmegaSSTIDDES`, and the `IDDESDelta` / `maxDeltaxyz`
classes (`delta` and `hmax` are fed from the dump; `hmax` was 0.3333 in every cell of this duct, so
the `hmax` formula is not exercised by its own geometry). Foundation (dev) not read or run.

---

# Slice 5 (gh:#1544 under gh:#1377): kEqn

## Predictions (written and committed BEFORE the driver was extended or run)

1. **kEqn** (`Ck 0.094`, `Ce 1.048` from `LESModel`, `cubeRootVol` delta): arithmetic only
   (`sqrt`, `*`, `/`), no other libm function. With upstream's `delta` fed in, **every** term is
   360/360 bit-identical: `nut = Ck sqrt(k) delta`, `GbyNu`, `G = nut GbyNu`, `k_SuSp = (2/3) divU`,
   `k_Sp = Ce sqrt(k)/delta`. With the port's own `cbrt` delta: `delta` 156/360 (as Smagorinsky and
   WALE), and `nut`, `k_Sp`, `G` differ in a subset of those cells only, by at most 7 ulp.
2. The transcribed `k` matrix reproduces the matrix the REAL model assembled, bit for bit.
3. `dynamicKEqn` is attempted after kEqn; it needs the `LESfilter` classes (`simpleFilter`,
   interpolate-and-sum over faces) on scalar, vector and symmetric-tensor fields, which the crate
   does not have. Prediction: not reachable cheaply in this slice; if it is not, that is recorded as
   deferred on gh:#1544 rather than approximated.
