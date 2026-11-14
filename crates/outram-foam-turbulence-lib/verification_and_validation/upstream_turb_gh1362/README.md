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

## Slice 5 results (2026-11-13, release; same duct; upstream `grad(U)`, `div(phi)`, `delta`)

**Audit held**: the transcribed `k` matrix reproduces the real model's (diag/source 360/360,
upper/lower 894/894).

| kEqn term | upstream delta: identical / 360 | own `cbrt` delta: identical / 360 (max ulp) |
|---|---|---|
| `delta` | 360 | 156 (2) |
| `nut` (real `validate()` and transcribed) | 360 | 158 (3) |
| `GbyNu`, `G`, `k_SuSp` | 360 | 360 |
| `k_Sp = Ce sqrt(k)/delta` | 360 | 156 (3) |

Pinned in `k_eqn_vs_openfoam` (ceiling 7 ulp).

- Predictions: 1 held for upstream delta (360/360 everywhere, no libm); wrong in detail for own
  delta: `G` does not differ, because `G` uses the STORED `nut` (upstream `G = nut GbyNu`, `nut_`
  being the field the model holds), which the test sets to upstream's. 2 held.
  3 (dynamicKEqn) held: not reachable in this slice, see below.
- Defects: none in the formulas.
- `dynamicKEqn`: DEFERRED. It needs `LESfilter` (`simpleFilter`, `laplaceFilter`,
  `anisotropicFilter`) acting on scalar, vector and symmetric-tensor fields (face interpolation
  and `surfaceSum` with boundary values), `fvc::grad` into a `devSymm` tensor, and several
  `filter_(...)` calls per cell whose arithmetic must be reproduced bit for bit; the crate has
  none of those. A decision for the maintainer: build the filters in `outram-foam-basic-lib`
  (shared with `dynamicLagrangian`) rather than inside this crate.
- `kEqn::correct()` is not compared with upstream (own solver settings, plain floor for `bound`,
  no `fvOptions`), as for the slice-3 models.

---

# Slice 6 (gh:#1544 under gh:#1377): LaunderSharmaKE

`kOmega2006` and `v2f` (named in gh:#1545) do NOT exist in ESI v2506: `find` over
`OpenFOAM-v2506/src` for `*kOmega2006*` and `*v2f*` returns nothing (grep over all `.C`/`.H` under `src/` for `v2f` and `kOmega2006` finds nothing). The
incompressible RAS list of v2506 is SpalartAllmaras, kEpsilon, RNGkEpsilon, realizableKE,
LaunderSharmaKE, kEpsilonPhitF, kOmega, kOmegaSST, kOmegaSSTSAS, kOmegaSSTLM, LRR, SSG, EBRSM.
Recorded on gh:#1545; not ported.

## Predictions (written and committed BEFORE the driver was extended or run)

1. **LaunderSharmaKE** (`Cmu 0.09`, `C1 1.44`, `C2 1.92`, `C3 0`, `sigmak 1`, `sigmaEps 1.3`):
   inputs fed from upstream: `grad(U)`, `div(phi)`, `fvc::magSqrGradGrad(U)` and
   `fvc::grad(sqrt(k))` (the port does not own these operators; gh:#1489). The libm functions are
   `exp` (in `fMu` and `f2`). Predicted: `fMu`, `f2` at least 90 % identical, at most 3 ulp;
   recomputed `nut` and the eps `Sp` coefficient (contains `f2`) differ only in the same cells;
   `G`, `E`, `D`, eps explicit `C1 G eps/k`, eps `SuSp`, k `SuSp`, k `Sp` are 360/360 identical
   (`G` and `E` use the STORED `nut`, set to upstream's, so they do not inherit the `exp` ulps).
2. The transcribed epsilon and k matrices reproduce the real matrices bit for bit.
3. No difference is expected that `exp` does not explain.

## Slice 6 results (2026-11-13, release; same duct; upstream `grad(U)`, `div(phi)`, `magSqrGradGrad(U)`, `grad(sqrt(k))` fed in)

**Audit held** (epsilon and k matrices: diag/source 360/360, upper/lower 894/894), for both runs.

**Every term is 360/360 bit-identical, max 0 ulp**: `fMu`, `f2`, recomputed `nut`, `E`, `D`, `G`,
`eps_E1`, `eps_SuSp`, `eps_Sp`, `k_E`, `k_SuSp`, `k_Sp`, at both viscosities below. PETIR `exp`
agrees with glibc `exp` on every argument that occurs here.

**Disclosed field change (a second run, not a changed field).** At the shared `nu = 1e-5` the
low-Reynolds branch is never entered: `k^2/(nu epsilon)` is large, so `fMu` lies in
(0.972, 0.99996) and `f2` is exactly 1.0 in all 360 cells (`0.3 exp(-50)` is below half an ulp
of 1). The identical-everywhere result at `nu = 1e-5` therefore says little about `f2`'s `exp`.
A second case, `LaunderSharmaKE_lowRe`, differs ONLY in `nu = 1e-2` (`transportProperties`
edited by `scripts/run.sh`; no other input changes) and gives `fMu` in (0.036, 0.128), `f2` in
(0.767, 1.0) with 166 distinct values: both damping functions are exercised. Both runs are
pinned (`launder_sharma_ke_vs_openfoam`, `launder_sharma_ke_low_re_vs_openfoam`).

- Predictions: 1 was WRONG on the libm point: it predicted `fMu` and `f2` would differ in up to
  10 % of cells (`exp` ulps); they are 360/360 identical at both viscosities. The structural
  predictions (G, E, D, SuSp, Sp identical; no `exp` ulps leaking into stored-`nut` terms) held. 2
  held. 3 held.
- Defects: none.
- Not covered: `correct()` (none yet; also needs the port's own `fvc::magSqrGradGrad` and
  `grad(sqrt k)`, compared with upstream under gh:#1489), wall functions, the non-isotropic
  `nut` boundary behaviour, `buoyantKEpsilon`.

# Slice 7: LESfilter family (basic-lib) and dynamicKEqn (gh:#1544)

## Predictions (written and committed BEFORE the driver was extended or run)

Setup: `LESfilter` `simple`, `laplace` (`widthCoeff` 2), `anisotropic` (`widthCoeff` 2) of ESI v2506
(`src/TurbulenceModels/turbulenceModels/LES/LESfilters/`) are ported into `outram-foam-basic-lib`
(`fv_operators/les_filter.rs`), generic over scalar, vector, symmTensor and tensor fields, and compared
per cell AND per boundary face against the real filter classes on the frozen 360-cell duct. Inputs:
`k` (zeroGradient scalar), `U` (fixedValue vector), `sqr(U)` (symmTensor, calculated), `symm(grad U)`.

1. **simple**: no libm, same accumulation order (`surfaceSum` over internal faces then boundary faces,
   `lambda (P - N) + N` interpolation, division by the summed `magSf`): 360/360 bit-identical for scalar,
   vector and symmTensor, cells and boundary faces. Confidence 85 %.
2. **anisotropic**: coefficient `sqr(2 V / surfaceSum(mag(Sf_d)))/widthCoeff` has no libm; filtered scalar,
   vector, symmTensor 360/360 identical. The boundary value of the filtered field equals the unfiltered
   boundary value (the coefficient field has a zero boundary). Confidence 75 % (the `vector & tensor`
   contraction order is the likely place to slip).
3. **laplace**: `coeff = pow(V, 2/3)/widthCoeff` uses libm `pow`; the PETIR `r_powf` will differ from glibc
   by 1 ulp in a MINORITY of the distinct cell volumes (the duct has about 10 distinct `V`), and the
   filtered fields differ by at most a few ulp only in cells whose `coeff` differs. The interpolated
   `gamma` has a ZERO boundary value (upstream `coeff_` is built with calculated patches at zero), so no
   diffusive flux crosses the boundary: this is reproduced, not "fixed". If the coefficient is fed with
   upstream's value, the filtered fields are 360/360 identical (confidence 80 %).
4. **dynamicKEqn** (`Ck`, `Ce`, `KK`, `LL`, `MM`, `nut`, `G`, `k_SuSp`, `k_Sp`, with upstream `grad(U)`
   cells AND boundary faces fed in): `KK`, `LL`, `MM`, `Ck` are libm-free and 360/360 identical for the
   `simple` filter (confidence 60 %: ten chained operations in upstream order). `Ce` contains
   `pow(KK, 1.5)` and differs by a few ulp in a minority of cells (PETIR `r_powf`).
5. **The two `D` of upstream**: `correctNut()` (called by `validate()`) passes `symm(grad U)` while
   `correct()` passes `devSymm(grad U)`. With `div U != 0` (up to 0.33 on this field) the two `Ck` differ
   by O(`tr D / |D|`), so the real `nut` after `validate()` equals the `symm` form and NOT the `devSymm`
   form. Predicted: the Rust `symm` form reproduces `nut_real`; the `devSymm` form does not (all cells
   with `|div U| > 0`).
6. **Boundary data matter**: the port's `fvc::grad_vec` leaves the boundary gradient zero-gradient
   (extrapolated), while upstream `gaussGrad::correctBoundaryConditions` replaces the wall-normal
   component. Feeding the port's extrapolated boundary instead of upstream's changes `filter(D)` in the
   cells touching a wall; since the duct is only 3 cells thick in z and the `Ck` expression chains three
   filter passes, predicted: `Ck` differs in more than 300 of 360 cells with the extrapolated boundary.
7. **dynamicKEqn matrix**: the transcribed k equation reproduces the real matrix bit for bit (audit).

## Slice 7 results, part 1: the LESfilter family (2026-11-13, release; same duct)

Driver: `-model LESfilters` applies the REAL `simpleFilter`, `laplaceFilter(mesh, 2)` and
`anisotropicFilter(mesh, 2)` of ESI v2506 to `k` (zeroGradient), `magSqr(U)`, `U` (fixedValue),
`sqr(U)`, `symm(grad U)` (calculated) and `grad U` (tensor), and dumps cells AND boundary faces.
Port: `outram_foam_basic_lib::fv_operators::les_filter` (new; `fvc::surfaceSum` and
`surfaceIntegrate` did not exist in the workspace; a workspace search for another LES filter found
none).

**Every row is 360/360 (cells) and 372/372 (boundary faces), bit-identical, max 0 ulp**, for all
three filters on all six fields (scalar, vector, symmTensor, tensor), also the two coefficient
fields: `laplace` `V^(2/3)/2` (PETIR `r_powf` against glibc `pow`: 360/360 identical) and the
anisotropic per-direction coefficient (1080/1080). 50 rows pinned (`PINS_LES_FILTERS`).

- Prediction 1 (simple identical): held. Prediction 2 (anisotropic identical, boundary = input
  boundary): held; the contraction order was right first time. Prediction 3: the structure held (zero
  boundary `gamma`, identical when fed the upstream coefficient) but the libm half was WRONG: PETIR
  `pow` agrees with glibc on all ten distinct cell volumes, so "own coeff" and "esi coeff" rows are
  the same.
- **Error in my own first run (disclosed).** The first comparison of the tensor `grad U` row was
  1840/3240 (`simple`), max relative difference 0.156, not ulp-sized. Cause: my test fed the dumped
  `gradU_b` (the wall-normal-corrected boundary), but `grad U` is `extrapolatedCalculated` and the
  filter's own `correctBoundaryConditions` re-extrapolates it to the owner-cell value before use.
  The real filter sees the extrapolated boundary; the test now feeds that and all rows are 0 ulp.
  No port change was needed. Consequence for any caller: a filter applied to a freshly computed
  `fvc::grad` field sees an extrapolated boundary, not the corrected one.
- Not covered: coupled (cyclic/AMI) patches, `Mixed`/`fixedGradient` boundary conditions on the
  filtered field, non-orthogonal meshes (the duct is orthogonal; the uncorrected `snGrad` uses
  `nonOrthDeltaCoeffs` as upstream, but only the orthogonal case was compared).

## Slice 7 results, part 2: dynamicKEqn (2026-11-13, release; same duct; upstream `grad(U)` cells AND boundary faces, `div(phi)`, `delta` fed in)

Driver: `-model dynamicKEqn -filter simple|laplace|anisotropic` (cases `dynamicKEqn_*`). The real model
runs (`validate()`, `correct()` into `dumpSolver`); the driver transcribes `Ck(D, KK)` and `Ce(D, KK)`
and ALSO calls the REAL protected `Ck` and `Ce` of the model object (a `using`-declaration on a derived
struct reaches the symbols `libincompressibleTurbulenceModels` exports; dumped as `R_Ck_symm`, `R_Ck_dev`,
`R_Ce`). Port: `outram_foam_turbulence_lib::les::dynamic_k_eqn::DynamicKEqn::{terms, source_terms}`
(terms only, no `correct()`) on top of `les_filter` and `les::pointwise`.

**Audit held** (transcribed k equation against the real matrix, all three filters: diag/source 360/360,
upper/lower 894/894) and **the transcription equals the REAL `Ck`/`Ce` members 360/360** (after the two
corrections below).

**Every term is bit-identical, max 0 ulp, in all three filters** (cells and boundary faces where dumped):
`D = symm(grad U)`, `D = devSymm(grad U)`, `KK` (unclamped and clamped), `LL`, `MM` (both `D`),
`Ck(symm)`, `Ck(devSymm)` against the real member and the transcription, `Ce` against the real member
and the transcription, `nut` after `validate()` and after `correct()` against the real model's `nut`,
`G`, `k_SuSp`, `k_Sp`. The registered `dynamicKEqn:G` equals the transcribed `G`. 27 rows per filter
pinned (`PINS_DYN_*`). PETIR `pow` (in `Ce`) agrees with glibc on every argument that occurs.

### Two upstream behaviours that decided the result (both were wrong in my first runs)

1. **`tmp` reuse gives an expression the PATCH TYPE of an operand, and the next filter's
   `correctBoundaryConditions` then rewrites the boundary.** Same-type `tmp` operators
   (`+`, `-`, `*`, `dev`, `pow`...) reuse the first (or, for `scalar*symmTensor`, the second) `tmp`
   operand's storage and patch types. A `simple` or `laplace` result is `extrapolatedCalculated`
   (it comes from `surfaceSum`/`surfaceIntegrate`), so `MM` and `LL`'s argument carries stale boundary
   values (`D_b + lap`) that `simpleFilter_`'s `correctBoundaryConditions` replaces with the owner-cell
   values; an `anisotropic` result is `calculated` and keeps them. In `Ce`, `this->nuEff()*(...)` is
   `tmp*tmp` and reuses `nuEff`'s calculated type, so there is NO re-extrapolation. My first C++
   transcription passed `nuEff` as a named `const` field, which hands the product the filtered
   difference's (extrapolated) type: its audit failed (laplace 205/360 diag cells, worst 0.12) and
   `Ce` differed from the real member by up to 33 %. Found by calling the real `Ce` and diffing, fixed
   by forming `nuEff` as a `tmp` in the transcription; the port's `DynamicKEqn::sf_entry` applies the
   re-extrapolation exactly where upstream does.
2. **`delta` is a `calculated` field whose boundary is `small = 1e-15`, not the owner-cell value**
   (`LESdelta.C`: constructed with `calculatedFvPatchScalarField` at `small`; `correctBoundaryConditions`
   does nothing to it). It only matters where a boundary value of `delta` reaches a filter
   (`-2 delta sqrt(KK) filter(D)` in `MM`, `2 delta` in `Ce`'s denominator): the anisotropic `MM`
   with `D = devSymm` differed in 1680 of 2160 components until the port used `small`.

### Negative controls (rows expected NOT to be identical, pinned)

| row | simple | laplace | anisotropic |
|---|---|---|---|
| `nut` formed with `devSymm` `D` against the real `nut` after `validate()` (prediction 5) | 280/360 | 248/360 | 203/360 |
| `Ck_dev` with the port-style EXTRAPOLATED `grad(U)` boundary against the real `Ck` (prediction 6) | 280/360 | 360/360 | 203/360 |

### Predictions against results

- 1, 2: held (simple, anisotropic 0 ulp). 3: structure held, libm half WRONG (see part 1).
- 4: HELD, and better than predicted: `Ce` is also 360/360 (I predicted a few ulp from `pow`).
- 5: held in direction, WRONG in extent. The real `nut` after `validate()` is reproduced 360/360 by the
  `symm` form, and the `devSymm` form differs, but in 80 / 112 / 157 of 360 cells (simple / laplace /
  anisotropic), not "all cells with `div U != 0`": `Ck` is clipped to zero in 248 of 360 cells, where
  both forms give `nut = 0`.
- 6: WRONG. I predicted the extrapolated boundary would change `Ck` in more than 300 cells: it changes 80
  (simple), 0 (laplace), 157 (anisotropic). The chained `simple` passes after `LL`/`MM` re-extrapolate
  their own boundaries, which hides most of the boundary gradient for `simple` and `laplace`.
- 7: held after the correction above.

### Defects (port against ESI v2506)

1. `fvc::grad_vec` leaves the boundary gradient extrapolated (zero-gradient), while upstream
   `gaussGrad::correctBoundaryConditions` replaces its wall-normal component
   (`grad(U)` boundary dumped as `gradU_b`). It matters to `dynamicKEqn` through `D`'s boundary values
   (anisotropic filter: 157 cells of `Ck` change). NOT fixed (`grad_vec` is shared and was not
   changed): the dynamic models take the gradient with its boundary faces as an input, as the kEqn and
   LaunderSharma ports take theirs. Recorded for gh:#1489. Not re-verified against `gaussGrad.C` line by
   line in this slice; the evidence is the dump (`gradU_b` differs from the owner-cell value).
2. `dynamicKEqn` has no `correct()` in the port (the k equation assembly, `bound`, and `correctNut` after
   the solve are not ported); `nut` after `correct()` is checked with `k` unchanged, as the dump solver
   does not solve.
3. Upstream's own inconsistency, reproduced: `correctNut()` uses `symm(grad U)` and an unclamped `KK`
   while `correct()` uses `devSymm(grad U)` and `KK` clamped at `SMALL`.

### Honest scope

One 360-cell orthogonal duct, one set of frozen polynomial fields: `Ck` is clipped to zero in 248 cells,
so the positive-`Ck` branch is exercised in 112 cells only; `KK` is at the `SMALL` clamp in the cells
nearest the walls. `laplace`/`anisotropic` with `widthCoeff` 2 only. Non-orthogonal meshes and coupled
patches are not covered. The unused `T_m_dev`/`T_FD_dev` dumps exist only in the anisotropic reference
(added while locating the `delta` boundary).

**Disclosure: a script bug in the first dynamicKEqn runs.** `run.sh` applied the `*_lowRe` viscosity (`nu = 1e-2`) to every case whose model name differed from its case name, which included the new `dynamicKEqn_*` cases; the audit numbers quoted for the failed transcription (laplace 205/360, worst 0.12) were taken at `nu = 1e-2`. The condition is now `case $M in *_lowRe)`; all committed references are at `nu = 1e-5` (checked: `nu_f` = 1e-5 in all three).

## Slice 7 results, part 3: dynamicLagrangian and the corrected Gauss gradient (2026-11-14, release; same duct)

**No predictions were committed for this model before its driver ran** (the rule of this directory
is broken here and the table below is a record, not a test of foresight). What I expected, from reading
`dynamicLagrangian.C` and the part 2 findings: terms 0 ulp for all three filters, the `Sf` boundary
correction irrelevant to the cell results, the explicit `pow(flm*fmm, 1/8)` the only libm exposure.

Driver: `-model dynamicLagrangian -filter ...` (cases `dynamicLagrangian_*`; `flm` and `fmm` are
frozen polynomials `0.02 + 0.3 s (1 + 0.1 x) + 0.01 z` and `0.05 + 0.2 s (1 + 0.05 x) + 0.02 z`,
zeroGradient, both positive so `bound` is the identity; `theta` the default 1.5). The transcription of
`correct()` reproduces the REAL `flm` and `fmm` matrices in all three filters (diag/source 360/360,
upper/lower 894/894), first time. Port: `les::dynamic_lagrangian::DynamicLagrangian::terms` (terms only:
no transport solve, no `bound`, no `correct()`).

**Every term is bit-identical, max 0 ulp, in all three filters, cells and boundary faces:** `S`, `|S|`,
`filter(U)`, `Sf`, `|Sf|`, `L`, `M`, `invT`, `L && M`, `M && M`, and `nut` against the transcription and
against the real `nut` after `validate()` and after `correct()`. 26 rows per filter pinned
(`PINS_LAG_*`). PETIR `pow(x, 1/8)` agrees with glibc on every argument here. The one libm row: with the
port's own `cbrt(V)` delta instead of upstream's `pow(V, 1/3)`, `nut` is bit-identical in 157 of 360
cells and within 8 ulp in the rest (1.03e-15 relative), the same figure as the kEqn row.

### New operator: `fvc::grad_vec_corrected` (basic-lib)

A port of `gaussGrad<vector>::calcGrad` WITH `correctBoundaryConditions` (owner-cell value, then
`g_b += n (sn(U) - n . g_b)` on non-coupled patches). Against the real `fvc::grad(U)`: 3240/3240 cell
components and 3348/3348 boundary-face components bit-identical, in all three runs (`gradU` rows). It does
not replace `fvc::grad_vec` (which leaves the boundary extrapolated, part 2 defect 1: left unchanged
because `grad_vec` is shared): callers that need upstream's boundary gradient can use the new one.
~~Part 2, defect 1: "NOT fixed"~~ **UPDATE 2026-11-14:** the missing operator now exists; `grad_vec`
itself is still the extrapolated form.

### What the Lagrangian model's results do NOT cover

The cell results do not depend on the `Sf` and `M` boundary values at all (they enter only the boundary
values of `L && M`, never the matrices), so the stale-boundary rules of part 2 are exercised only through
the `filter(U)` face values that the gradient of `Uf` interpolates. Not covered: the transport solves, the
`bound` of `flm`/`fmm` below zero, `fvOptions`, non-orthogonal meshes, coupled patches, `widthCoeff` other
than 2.

# Slice 8: kOmegaSSTLM (Langtry-Menter transition on SST) (gh:#1544)

## Frozen field for this model (a disclosed change, decided before any run)

The shared duct fields cannot exercise the transition model: with `k` in (0.02, 0.45) and
`nu = 1e-5`, `RT = k/(nu omega)` is at least ~40 everywhere, so `Fonset3 = max(1 - (RT/2.5)^3, 0)`
is exactly 0, `Fturb = exp(-(0.25 RT)^4)` and `Freattach = exp(-(RT/20)^4)` underflow to 0, and
`Tu = 100 sqrt(2k/3)/|U|` is above 1.3 in every cell (the `Tu <= 1.3` branch of `ReThetat0` is
never entered). The same trap as `LaunderSharmaKE` at `nu = 1e-5` (slice 6). For `-model kOmegaSSTLM`
ONLY the driver therefore uses its own polynomials (the other models' fields are untouched):

* `k_LM = 2.0e-5 + 3.0e-4 s (1 + 0.1 x) + 2.0e-5 z` (2e-5 to ~1.1e-4 m^2/s^2; the shared `omega`, `U`, `nu = 1e-5` are kept)
* `ReThetat = 120 + 6400 s (1 + 0.1 x) + 400 z` (about 120 to 2560, to straddle the `Flength` breaks at 400, 596, 1200 and the `ReThetac` break at 1870)
* `gammaInt = 0.01 + 3.0 s (1 + 0.05 x) + 0.05 z` (0.01 to ~0.94)
* `gammaIntEff` (the model's own field, set before `correct()` through the registry, so the first `correct()` does not run with the constructor's zero) `= 0.02 + 24 s^2 (1 + 0.1 x) + 0.1 z` (so the `clamp(gammaIntEff, 0.1, 1)` of `epsilonByk` is active on both sides; see "changed once" below)

Chosen from the ranges of the dimensionless groups (`RT`, `Tu`, `Ry`, `Rev`) alone, before any run of this
model; the polynomials contain no libm. `scripts/lm_coverage_stats.py` counts how many cells fall in each
branch from the dumped INPUTS. `gammaIntEff` is overwritten by `correct()` at its end
(`max(gammaInt, gammaSep)`); the post-`correct()` value is dumped and compared as `gammaIntEff_real`.

## Predictions (written and committed BEFORE the driver was extended or run)

Setup: the Rust `KOmegaSstLm` reuses `KOmegaSST::source_terms(EsiV2506)` for every SST term the LM model does
not change (`y`, `S2`, `CDkOmega`, base `F1`, `F23`, `nut`, `GbyNu0`, `G`, `GbyNu`, base `Pk`, `k_SuSp`) and
recomputes what the LM overrides change: `F1 = max(F1_SST, F3)` (so `gamma`, `beta`, the omega terms, the
cross-diffusion), `Pk = gammaIntEff Pk_SST`, `epsilonByk = clamp(gammaIntEff, 0.1, 1) betaStar omega`, and the new
`Fthetat`, `ReThetac`, `Flength`, `ReThetat0` (with the `lambda` iteration), `Fonset`, `Fturb`, `Freattach`,
`gammaSep`, `gammaIntEff`, `Pthetat`, `Pgamma`, `Egamma`. The upstream gradients, `y`, the `gammaIntEff` input and
the frozen `ReThetat`, `gammaInt` are fed in.

1. **Audit**: the four transcribed equations (`omega`, `k` with the LM overrides, `ReThetat`, `gammaInt`)
   reproduce the matrices the REAL model assembled bit for bit (diag, source, upper, lower). Confidence 85%.
   The omega and k audits also confirm that the real model calls the LM overrides of `F1`, `Pk`, `epsilonByk`.
2. **Libm-free terms 360/360 identical** (given identical inputs): `ReThetac`, `Fonset`, `Rev`, `RT`, `Omega`, `S`,
   `Us`, `dUsds`, `Pk`, `epsilonByk`. Confidence 85% (the risk is the order of the 9-component
   `magSqr(skew)` sum and of the `U & (U & gradU)` contraction).
3. **`exp`-only terms** (`Fturb`, `Freattach`, `Fwake`, `Fthetat`, `Fsublayer`, `Flength`): PETIR `exp` agreed with glibc
   on every argument of `LaunderSharmaKE` (slice 6), so predicted 360/360 identical, at most 1 ulp where not.
   Confidence 60%. `Fturb` and `Freattach` are mostly in (0, 1), not saturated, by the field choice above.
4. **`pow` terms**: `F3` (`pow(Ry/120, 8)`) identical in at least 95% of cells. `ReThetat0` (`pow(Tu/1.5, 1.5)`,
   `pow(Tu - 0.5658, -0.671)`, `exp`, and a fixed-point iteration) is the weakest term: at least 70% of cells
   identical and at most 8 ulp elsewhere, with the same iteration count in every cell (the stopping test
   `|lambda - lambda0| > 1e-6` is not within ulps of its threshold). A difference there is libm, not a formula error.
5. **Downstream of `tanh`** (`F1_SST`, hence `F1`, `gamma`, `beta`, the omega terms): as in slice 2 (PETIR `tanh`
   against glibc, a few percent of cells, at most 3 ulp). Where `F3 > F1_SST` the LM `F1` takes `F3` and hides it.
6. **Upstream behaviours to reproduce, not fix**: (a) `Pk` multiplies by the UNCLAMPED `gammaIntEff` (up to ~2
   here) while `epsilonByk` uses the CLAMPED one; (b) the first `correct()` of a real run has `gammaIntEff = 0`
   (so `Pk = 0` in the first step; not exercised, the driver sets it); (c) the `ReThetat0`
   `do ... while (lambdaErr > lambdaErr_)` has no iteration cap (`maxLambdaIter` only warns); the port must not cap
   either to stay bit-identical; the iteration count per cell is reported.
7. No cell is expected to differ by a formula error; anything not explained by a libm function is a defect in the port.

## Slice 8 results (2026-11-14, release; same 360-cell duct, model-specific fields above, ESI `y` and upstream gradients fed in)

Port: `outram_foam_turbulence_lib::k_omega_sst_lm::KOmegaSstLm` (new; terms only, no `correct()`).
Test: `k_omega_sst_lm_vs_openfoam`, 44 rows pinned (`PINS_SST_LM`), max 3 ulp allowed.

**Audit (prediction 1: held).** All 16 parts (diag, source, upper, lower of the `omega`, `k`, `ReThetat` and
`gammaInt` matrices) of the transcribed equations equal the matrices the REAL model assembled, bit for bit
(360/360 and 894/894). The `omega` and `k` audits include the LM overrides (`F1 = max(F1_SST, F3)`,
`Pk = gammaIntEff Pk_SST`, `epsilonByk = clamp(gammaIntEff, 0.1, 1) betaStar omega`), so the real model does
call them as transcribed.

**Every LM-specific term is 360/360 bit-identical, max 0 ulp**: `F3`, `F1`, `gamma`, `beta`, `Omega`, `S`, `Us`,
`dUsds`, `delta`, `Fwake`, `Fthetat`, `ReThetac`, `Flength`, `ReThetat0` and its iteration count, `Pthetat`,
`ReThetat_E`, `Rev`, `RT`, `Fonset`, `Pgamma`, `Fturb`, `Egamma`, `Freattach`, `gammaSep`, `gammaIntEff` (against
both the transcription and the value the real `correct()` left in the model, `gammaIntEff_real`), `k_Sp`,
`omega_Sp`, `omega_SuSp1`, `omega_SuSp2`, `k_SuSp`, `S2`, `CDkOmega`, `GbyNu0`.

The only differences are inherited from the SST blending functions (PETIR `tanh` against glibc, slice 2), at most
3 ulp: `nut` 354, `F1sst` 351, `F23` 343, `G` 354, `GbyNu` 359, `PkSst` 355, `Pk` 355, `omega_E` 359, `k_E` 355
of 360. The LM `F1 = max(F1_SST, F3)` is 360/360: all nine cells where `F1_SST` differs have `F3 > F1_SST`
(`F3` is the larger in 310 of 360 cells), which hides the `tanh` ulp there.

### Predictions that held, and the ones that did not

- Held: 1 (audit), 2 (libm-free terms), 3 (`exp`-only terms: PETIR `exp` equals glibc on every argument, 360/360, again),
  4 for `F3` (360/360, at least 95% predicted), 5 (the `tanh` cells), 6 (a) and (c).
- **Wrong, in the favourable direction: 4 for `ReThetat0`.** I predicted at least 70% identical and up to 8 ulp
  (`pow` with non-integer exponents, `exp`, an iteration); all 360 are identical, and so are the 360 iteration counts.
  PETIR `pow` equals glibc `pow` on these arguments, as it did for `laplaceFilter` (slice 7).
- Not predicted, a limitation of the field set (disclosed, not tuned afterwards): the `lambda` iteration stops after 2 passes
  in 340 of 360 cells (`dUsds` is nearly zero, so `lambda` stays near 0); only 20 cells run 3 to 11 passes, so the
  coupling of `lambda` into `thetat` is exercised by 20 cells, and the clamp `lambda in [-0.1, 0.1]` was not checked
  to be reached. `Freattach` lies in (0.946, 1) (`RT` at most 9.7, so `exp(-(RT/20)^4)` hardly varies), and `Fonset` is 0 in 72
  cells, 2 in 16 and in [1, 2) in 272 (never in (0, 1)), so the lower part of the `Fonset2` ramp is not exercised;
  `Pgamma > 0` in 288 cells. The `Tu <= 1.3` branch has 152 cells, `Tu > 1.3` 208;
  `dUsds <= 0` 92, `> 0` 268; `ReThetat` in the four `Flength` bands 42/30/106/182 and above 1870 in 70.

### Fields: changed once, from INPUT coverage only (disclosed)

The first dump used `gammaIntEff = 0.03 + 5.2 s (1 + 0.1 x) + 0.1 z`; `scripts/lm_coverage_stats.py` on the dumped inputs
showed it never fell below 0.1 (0 cells), so the lower side of `clamp(gammaIntEff, 0.1, 1)` was not exercised. It was
changed to `0.02 + 24 s^2 (1 + 0.1 x) + 0.1 z` (72 / 158 / 130 cells below 0.1 / within / above 1), and the reference
re-dumped. No model output had been compared: the Rust side did not exist yet. The predictions were committed before the
first dump with the first polynomial; the README text above names the final one.

### Upstream behaviours noted (reproduced, nothing changed)

1. `Pk` multiplies by the UNCLAMPED `gammaIntEff` (up to 1.98 here; `Pk > Pk_SST` in 130 cells, `<` in 230) while
   `epsilonByk` clamps to [0.1, 1]. `gammaSep` can reach 2 times `Fthetat`, so `Pk` can exceed the SST limiter
   `10 betaStar k omega`.
2. `gammaIntEff_` is a `volScalarField::Internal` (no boundary), zero after construction: in a real run the first
   `correct()` has `Pk = 0` and `epsilonByk = 0.1 betaStar omega`. Not exercised here (the driver sets it first).
3. The `ReThetat0` loop is `do ... while (lambdaErr > lambdaErr_)` with no iteration cap (the `maxLambdaIter` warning is
   after the loop). The port guards at 1e6 passes with a panic. Maximum observed: 11.

### Defects

None found in the port (no earlier port existed). Not ported / not covered: `correct()` (the two solves, `bound`, the
update of `gammaIntEff` between steps), the `ReThetat` and `gammaInt` boundary conditions, `fvOptions`, a non-zero
`omegaInf`/`kInf`, SST `F3`/decay control/`Qsas` (all off in the v2506 defaults), compressible and multiphase
variants, and `Tu` from a freestream (upstream uses the cell `k` and `|U|`, as ported).

# Slice 9: LES deltas `maxDeltaxyz`, `IDDESDelta`, `vanDriest` (and `smooth`) (gh:#1544)

**Process disclosure (a rule slip, stated before anything else).** The four reference dumps
(`ref_LESdelta_{maxDeltaxyz,IDDESDelta,vanDriest,smooth}.txt.gz`) were generated BEFORE these predictions were
committed. They are committed with this text, in the same commit. No dump value was opened and no Rust code for the
deltas existed when the predictions below were written (the run's only visible output was "no compile or run error").
The predictions are therefore not retroactive in content, but they are not timestamped ahead of the run as the
earlier slices are.

Setup: the REAL delta classes of ESI v2506 (`LESdeltas/`) are selected through a `kEqn` LES model
(`delta <name>;` in `turbulenceProperties`, `scripts/run.sh` variants `LESdelta_*`; `kEqn` and the frozen fields are those of
slice 5; `correct()` runs with `dumpSolver`). Dumped: `delta_real` (after `validate()`), `delta_after` (after `correct()`,
which recomputes `vanDriest`), `hmax_real` (`IDDESDelta`'s `maxDeltaxyz`), `wall_n`, `wall_y` (`wallDist`), and for `vanDriest` a
transcription of `calcDelta()` (`vd_*`: per-wall-face `nuw`, `nutw`, `|snGrad(U)|`, `ystar`; the addressing `y`; `ystar` mapped
from the nearest wall face; `y+`; `delta`). Port: `outram_foam_turbulence_lib::les::deltas` (new).

1. **`maxDeltaxyz`** (`deltaCoeff` 2): `deltaCoeff max_f |n_f . (Cf - C)|` with `n_f = Sf/|Sf|`; only `+ - * / sqrt` and
   a max: **360/360 bit-identical**. Confidence 85% (the unit-normal division `Sf/mag(Sf)` is the place to slip).
2. **`IDDESDelta`** (`Cw` 0.15; its `hmax` is a `maxDeltaxyz`, default coefficient 2):
   `min(max(max(Cw y, Cw hmax), faceToFaceMax), hmax)`, with `faceToFaceMax = max_{i,j in cell faces} n_c . (Cf_j - Cf_i)`.
   With ESI `y`, `n` fed: **360/360 identical** for `delta` and for `hmax` from the port's own `maxDeltaxyz`. Confidence 80%.
3. **`vanDriest` per-face `ystar`**, `nuw/sqrt((nuw + nutw) |snGrad U| + VSMALL)`, with `snGrad U = deltaCoeffs (Uw - Uc)`,
   `deltaCoeffs = 1/|Cf - C|` (orthogonal duct): **identical on every wall face**. Confidence 65% (the form of the
   boundary `deltaCoeffs` is the risk).
4. **`vanDriest` `ystar` mapped to cells.** The port picks the nearest wall FACE CENTRE by brute force; upstream
   `wallDistAddressing` uses `meshWave`, which for the SST wall distance (slice 2) differed from the face-centre distance
   in 95 of 360 cells. Predicted: the mapped `ystar` is identical in at least 70% of cells and is wrong (a different
   face) in the rest; the wall-face `ystar` varies little across a wall of this duct, so the difference in `ystar` is much smaller than in `y`.
5. **`vanDriest` `delta`**, with ESI `y` and the ESI-mapped `ystar` fed in: `min(geometric, (kappa/Cdelta)((1 + SMALL) - exp(-ystar/Aplus)) y)`
   is **360/360 identical** (PETIR `exp` equals glibc on every earlier test). Both `min` branches and both sides of the `y+ > 500` cutoff
   are predicted to occur on this duct (not checked before the run).
6. `smooth` (a `FaceCellWave` fixed point whose result depends on the visiting order through the tolerance `1.01`) is ported
   only if time allows; no prediction beyond: the port must reproduce upstream's visiting order, not a "converged" maximum.

## Slice 9 results (2026-11-14, release; same 360-cell duct, ESI `y`, `n`, `hmax` fed where stated)

Port: `outram_foam_turbulence_lib::les::deltas` (new). Tests: `max_delta_xyz_vs_openfoam`, `iddes_delta_vs_openfoam`,
`van_driest_delta_vs_openfoam` (13 rows pinned, `PINS_D_*`).

| row | identical / n | max ulp |
|---|---|---|
| maxDeltaxyz `delta` (after `validate()` and after `correct()`) | 360/360, 360/360 | 0 |
| IDDESDelta `hmax` (own `maxDeltaxyz`, `deltaCoeff` 2) | 360/360 | 0 |
| IDDESDelta `delta` (ESI `hmax`, `n`, `y`) / (own `hmax`, ESI `n`, `y`) | 360/360, 360/360 | 0 |
| vanDriest geometric `delta` (= `cubeRootVol`, from the dump) against `delta_real` | 360/360 | 0 |
| vanDriest `ystar` on the 312 wall faces (port: `nu/sqrt((nu + nut)|deltaCoeffs (Uw - Uc)| + VSMALL)`) | 312/312 | 0 |
| vanDriest `y+` and `delta` from the ESI-mapped `ystar` and ESI `y`, against the transcription and the real `delta` | 360/360 (4 rows) | 0 |
| vanDriest `ystar` mapped with the port's NEAREST wall face | **91/360** | huge (relative difference up to 1.1) |
| vanDriest `delta` with the port's mapped `ystar` | **228/360** (max relative difference 0.41) | |

Predictions: 1 (maxDeltaxyz identical), 2 (IDDESDelta identical), 3 (`ystar` on wall faces identical), 5 (`delta` from ESI inputs
identical) held. **Prediction 4 was wrong**: I predicted at least 70% of cells map to the same `ystar`; only 91 do. Cause below.

### Finding in ESI v2506: `wallDistAddressing::map` does not take the value from the nearest wall face (not fixed, deferred)

The driver maps a test field whose wall-face values are the face's index within the wall patch through the real
`wallDistAddressing::map` (`vd_idx_mapped`). In **263 of 360 cells the face it takes the value from is not the wall face
nearest to the cell**, although `wallDistAddressing::y()` (the distance) is the true nearest distance: e.g. cell 1, 0.022 m above
the wall, takes the value of the wall face 0.334 m away (index 6 for cells 1..5, then 73, 93, 113, ...). The real
`vanDriestDelta` therefore uses `ystar` of an unrelated face in most cells of this duct, and the driver's transcription of
`calcDelta()` (which calls the same `map`) reproduces the real `delta` 360/360, so this is the behaviour of the library
call, not of the driver's arrangement of it. Not established: whether this is an upstream defect in v2506
(wrong index order in the map's addressing) or a consequence of this case (serial run, one `wall` patch listed after `inlet` and
`outlet`, wall faces numbered z-fastest). Not reproduced in the port: the port maps `ystar` from the nearest
wall-face centre, which is what the class intends; bit-identity with ESI for `vanDriest` therefore needs the mapped `ystar`
as an input ([`van_driest_yplus`] takes it), and the two rows above are pinned as the measured disagreement. **Decision for the
maintainer (deferred):** follow the intent (nearest face, as now) or reproduce the library; and whether to report it upstream.
Not checked: the Foundation (dev) `vanDriestDelta`, which uses a different addressing code.

### `smooth` not ported

The real `smooth` delta with the case's `maxDeltaRatio 1.15` returns `cubeRootVol` unchanged on this duct (max relative
difference from `cbrt(V)` 5.5e-16, 0 cells beyond 1e-12): the neighbouring cell volumes differ by too little for the wave
to start. The dump (`ref_LESdelta_smooth.txt.gz`) therefore cannot test a port. The port would also need upstream's
`FaceCellWave` visiting order (the tolerance 1.01 in `deltaData::update` makes the fixed point order-dependent). Deferred; a
useful case needs `maxDeltaRatio` of about 1.02 on this mesh (chosen from the graded cell sizes, not yet run).

### Not covered

Coupled patches, non-orthogonal meshes (`vanDriest`'s boundary `deltaCoeffs` is `1/|Cf - C|` only for orthogonal faces), 2D cases
(`nGeometricD` warning branch), `PrandtlDelta`, `SLADelta`, `DeltaOmegaTildeDelta`, `maxDeltaxyzCubeRootLESDelta`, `cubeRootVol` itself
(the earlier slices use it as `1.0 cbrt(V)`).
