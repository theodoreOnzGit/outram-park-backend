# Results: outram-park-fork-moltres vs Moltres 3dd2ce7 (gh:#1400), 2026-11-13

Untrusted AI-assisted draft pending human review. Research, education and V&V
only; not for facility operation, licensing or safety decisions.
Predictions: `PREDICTIONS.md` (committed before any driver or run; unchanged).

## Route actually taken

* MOOSE + Moltres were **not built** and no fresh Moltres run exists. Reasons
  (decided before measuring, see PREDICTIONS.md): no micromamba/conda in the
  image, 5.8 GB free against a 5 GB floor, MOOSE is a submodule absent from
  the Moltres clone, 4 shared cores. I did not probe the conda channel.
* Kernel level: C++ transcription (`dump_moltres_kernels.cpp`) of
  `InScatter`, `CoupledFissionKernel`, `PrecursorSource`,
  `DelayedNeutronSource`, `PrecursorDecay`, `ScalarTransportBase` and MOOSE
  `LinearInterpolation::sample/sampleDerivative`, plus the property-file
  reader. `check_verbatim.sh`: **31 UPSTREAM_MOLTRES lines, 0 not found;
  14 UPSTREAM_MOOSE lines, 0 not found** (`check_verbatim_output.txt`).
  Shim: plain vectors for MOOSE's material properties, `_test = 1`; no
  quadrature, shape functions, libMesh or PETSc.
* Solution level: numbers read from Moltres' **committed gold Exodus files**
  (`moltres_gold_values.txt`, via `gold_extract.py`). These are real Moltres
  outputs at the pinned commit but not runs by us.

## Kernel level (`tests/moltres_kernel_differential_gh1400.rs`, bits)

| ID | Quantity | N | Bit-identical | Worst | Predicted |
|---|---|---|---|---|---|
| K1 | table value, 28 columns of 10 `newt_fuel_*` files, 200 T each (below range, on node, above range) | 5600 | **5600 (100 %)** | 0 ulp | 100 % |
| K1 | table slope (`d_*_d_temp`) | 5600 | **5600 (100 %)** | 0 ulp | 100 % |
| K2 | `InScatter`, `sss2_input` true, G=3 | 4096 | **100 %** | 0 | 100 % |
| K2 | `InScatter`, `sss2_input` false, G=3 | 4096 | **100 %** | 0 | 100 % |
| K3 | `DelayedNeutronSource`, 6 families | 4096 | **100 %** | 0 | 100 % |
| K4 | `PrecursorDecay` | 4096 | **100 %** | 0 | 100 % |
| K5 | prompt fission, G=2, BEFORE (legacy `chi_p*(1-beta)/k*S`) | 4096 | 2669 (65.2 %) | 2 ulp | 20-80 %, <= 2 ulp: **hit** |
| K5 | prompt fission, G=3, BEFORE | 4096 | 2678 (65.4 %) | 2 ulp | same: **hit** |
| K5 | prompt fission, G=2 and G=3, AFTER translating upstream order | 8192 | **100 %** | 0 | 100 % |
| K6 | `PrecursorSource`, G=2 | 4096 | 1915 (46.8 %) | 3 ulp | 10-60 %, <= 4 ulp: **hit** |
| K6 | `PrecursorSource`, G=3 | 4096 | 1934 (47.2 %) | 3 ulp | same: **hit** |

K6 is **not fixed** and is flagged for the maintainer: upstream forms
`sum_j(beta_i nsf_j phi_j)/k`, the port `(beta_i/k)(sum_j nsf_j phi_j)`
because `PrecursorDrift` takes `S_f`, not the group fluxes. Round-off class
(<= 3 ulp, ~4e-16 relative, well inside the 1e-8 allowance); closing it
needs a public-signature change.

Not compared, and why: `GroupDiffusion`, `CoupledScalarAdvection`,
`ScalarTransportTimeDerivative`, `ConvectiveHeatExchanger` (FE weak-form
residuals / implicit FV matrices, no pointwise counterpart);
`FissionHeatSource` (the port stores `kappa*Sigma_f` premultiplied and scales
by `power/tot` outside the group sum: a data-layout difference, left as a
gap).

## Solution level (`tests/moltres_solution_differential_gh1400.rs`)

**S1 vacuum boundary** (`bcs/vacuum_bc.i`; unit length, `D = 1e-3`, no
removal, Dirichlet left; port: 20 cells, `MixedField` Robin from the new
`vacuum_patch_field`):

| Type | alpha | Moltres gold right edge | Port | abs. diff | Former port "vacuum" (`FixedValue(0)`) |
|---|---|---|---|---|---|
| Marshak | 2 | 1.999999999983953e-3 | 2.000000000000000e-3 | 1.6e-14 | 0 (miss 2.0e-3) |
| Mark | sqrt 3 | 1.732050807570573e-3 | 1.732050807555801e-3 | 1.5e-14 | 0 (miss 1.7e-3) |
| Milne | 3*0.710446 | 2.130488794015986e-3 | 2.130488793962832e-3 | 5.3e-14 | 0 (miss 2.1e-3) |

Predicted `<= 1e-9`: **hit** (by 4 orders). Port against the closed form
`phi_b = alpha D phi_l/(1 + alpha D)`: <= 1e-12. (My PREDICTIONS wrote the
right edge as `alpha*D`, which is only the leading term; the closed form is
exact and the gold agrees with it, so Milne's stale test comment `0.710446`
vs the `3 * 0.710446` in `VacuumConcBC.h` is a comment issue, not a code one.)

**S2 k_eff = k_inf** (2 groups, natural boundaries, same tables, T = 922 K):

| Case | Moltres gold k_eff | Port k_inf | rel. diff |
|---|---|---|---|
| `gmm_none` (`inf_crit_small_two_group_`, P=8) | 0.995186254639 | 0.995186254639 | 6.1e-14 |
| `gmm_linear` (`newt_fuel_`, P=6) | 0.996059797139 | 0.996059797139 | 8.4e-15 |

Predicted `<= 1e-8`: **hit**. Methodological limit: the eigenfunction is
uniform, so this checks the data path (file layout, index transposition,
cm -> m, which entry is removal, interpolation) and the eigenvalue algebra,
and says nothing about spatial discretisation. A spatially varying
mesh-converged FE-vs-FV eigenvalue comparison (issue cases 1-3) was **not**
run (needs a real Moltres run or a gmsh reader).

**S3 spline / monotone-cubic** (~~not translated~~ **translated in round 2, see S5 below**): port(linear) minus gold =
+6.3937e-5 (spline) and +1.7851e-5 (monotone cubic); identical to Moltres'
own linear-minus-spline difference to 1e-8, as predicted. My PREDICTIONS
quoted "about +5.9e-5" for the spline difference; that was an arithmetic
slip (the gold files give 6.39e-5), the equality claim stands.

**S4 reduced feedback vs table** (issue case 4; `newt_fuel_` 800-1050 K;
reduced = all constants at the 900 K row, `Sigma_r` slope = Moltres'
`d_remxs_d_temp` at 900 K, applied through `XsFields::sigma_removal_at`):

| T (K) | k table | k reduced | reduced - table |
|---|---|---|---|
| 825 | 0.99597912 | 0.99633674 | +35.8 pcm |
| 875 | 0.99571835 | 0.99593123 | +21.3 pcm |
| 922 | 0.99605980 | 0.99580113 | -25.9 pcm |
| 975 | 0.99714014 | 0.99596276 | -117.7 pcm |
| 1025 | 0.99926088 | 0.99643265 | -282.8 pcm |

dk/dT (825 to 1025 K): table +1.64 pcm/K, reduced +0.05 pcm/K.
**PREDICTION MISSED** (PREDICTIONS S4: opposite signs, each 5-40 pcm/K, error
> 500 pcm at 1025 K). Measured: same sign, both far below 5 pcm/K, error 283
pcm at 1025 K. The thermal-removal fall I reasoned from is nearly cancelled
by the other constants in these tables, and the table k(T) is not monotone.
The test pins the measured numbers as a regression record (to 0.2 pcm), not
as a claim about expected physics. The finding stands: the reduced model
carries ~3 % of the table's dk/dT and is wrong by up to 283 pcm over +/-125 K
of its reference. These are k_inf of a Serpent/NEWT dataset, not the MSRE.

## Defects found and fixed (translated from upstream)

1. **No Marshak/Mark/Milne vacuum condition** (`VacuumConcBC`). The only
   "vacuum" was zero flux at the face; the #1401 row said "ported (check)".
   Added `diffusion::VacuumBoundary`, `vacuum_patch_field`,
   `StaticDiffusion::set_vacuum_boundary` (FV Robin; the weight uses the same
   `delta` as the Laplacian). Effect on a 1 m one-group slab (new unit test
   `marshak_vacuum_slab_matches_analytic_robin_mode`): k 1.03869264 (zero-flux
   face) -> 1.06095378 (Marshak), exact 1.06094742 (rel 6.0e-6): the old
   boundary understated k by 2226 pcm against the Moltres condition.
2. **Operation order of the prompt-fission term** (`CoupledFissionKernel`):
   34.8 % of samples differed by up to 2 ulp; now `terms::prompt_fission_source`
   translates `sf * ((1-beta) chi_p) / k` and is 100 % identical.
3. **No table reader / interpolation** (`GenericMoltresMaterial`): new
   `tables` module (`none`, `linear`, both file layouts, cm -> m, MeV -> J with
   Moltres' `1e6 * 1.6e-19`).

## Moved numbers

Existing gates are unchanged (26 lib tests, `msre_n4_precursor_drift_tm1626`
pass at their old tolerances). The prompt-fission rewrite moves
`circulating` results at the 1e-16 relative level only; I did not re-run and
re-record those digits (not re-measured).

## Data provenance

`data/*.txt`: Moltres `property_file_dir/{newt_fuel_*,inf_crit_small_two_group_*}`
at `3dd2ce7` (arfc/moltres, LGPL-2.1), copied 2026-11-13, unmodified; open
Serpent/NEWT group constants, no restricted data. `moltres_gold_values.txt`:
extracted from `tests/{bcs,materials}/gold/*.e` at the same commit.
`reference_kernels.txt`: output of `dump_moltres_kernels.cpp`
(`g++ -O2 -ffp-contract=off`, g++ in this image). To regenerate:
`g++ -O2 -ffp-contract=off -o dump dump_moltres_kernels.cpp && ./dump data/newt_fuel_`.

## What remains

See the gap list on gh:#1401 and the child issues filed from this work.

---

# Round 2 (gh:#1538, 2026-11-13): spline, monotone cubic, bicubic, LSQ, JSON material

Predictions: `PREDICTIONS.md`, "Round 2" section, committed (`2730f1b9b`)
before the new code was compiled. Pins: Moltres 3dd2ce7, MOOSE
`d7c4bde4025537ac4574cc80e78c2407e23a42cc` (full sha resolved from the
Moltres tree). Same route as round 1 (no MOOSE build; C++ transcription for
kernels, committed gold `.e` files for solutions). `check_verbatim.sh`: 35
`UPSTREAM_MOLTRES` and 153 `UPSTREAM_MOOSE` marked lines, 0 not found
(`check_verbatim_output.txt`). `reference_kernels.txt` was regenerated; every
round-1 record kind (`TBL`, `INSC`, `FISS`, `PSRC`, `DNS`, `DEC`) is
byte-identical to the previous file.

## Kernel level (bits; `tests/moltres_kernel_differential_gh1400.rs`)

| id | what | n | identical | worst | predicted |
|---|---|---|---|---|---|
| K9 | `SplineInterpolation` value / slope, 12 `newt_fuel_` columns, 200 T each incl. extrapolation | 5600 / 5600 | 5600 / 5600 | 0 ulp | 100 % |
| K10 | `MonotoneCubicInterpolation` value / slope, same grid | 5600 / 5600 | 5600 / 5600 | 0 ulp | 100 % |
| K11 | `BicubicSplineInterpolation` value, d/dx1, d/dx2, synthetic 5x4 grid, 4096 points incl. outside | 3 x 4096 | 3 x 4096 | 0 ulp | 100 %, <= 4 ulp |
| K12 | least-squares `c0 T + c1`, `(c0 T + c1) 1e6 1.6e-19`, slopes | 2 x 4096 | all | 0 ulp | 100 % |
| K13 | MeV -> J through `sigma_power` (**not predicted**, see defect 1) | 4096 | 2950 before the fix, 4096 after | 1 ulp before | n/a |

The `std::pow(x, 2)` -> `x * x` risk named in the prediction did not
materialise (0 differences). Bicubic and least-squares have **no Moltres
test input and no gold file** at 3dd2ce7, so K11 and K12 are the only
verification they have: there is no solution-level claim for them and no
comparison against any real Moltres input file.

## Solution level (`tests/moltres_solution_differential_gh1400.rs`)

Bound `<= 1e-8` relative, written before the run. Measured:

| case | gold k_eff | port k_inf | relative difference |
|---|---|---|---|
| S5 `gmm_spline` (newt_fuel_, 922 K) | 0.99599586036298 | 0.99599586036297 | 3.3e-15 |
| S5 `gmm_monotone_cubic` | 0.99604194590814 | 0.99604194590813 | 1.0e-14 |
| S6 `mjm_none` (xsdata-900K.json, 900 K) | 1.14679014198082 | 1.14679014198085 | 3.1e-14 |
| S6 `mjm_linear` (xsdata.json, 922 K) | 1.14696105184326 | 1.14696105184327 | 1.2e-14 |
| S6 `mjm_spline` | 1.14644507369501 | 1.14644507369503 | 1.2e-14 |
| S6 `mjm_monotone_cubic` | 1.14678314424103 | 1.14678314424104 | 1.1e-14 |

Port spline-minus-linear and monotone-minus-linear on the `gmm` tables are
-6.3937 pcm and -1.7851 pcm, equal to the gold gaps to the digits printed.
On the three-row JSON tables the spline departs from linear by -51.6 pcm and
monotone by -17.8 pcm, as predicted from the gold files. All predictions held.
The round-1 assumption (natural boundaries make `k_eff = k_inf`) is carried
over unchanged and is not independently tested here.
S7 (not gold-backed) pins the JSON reader quirks read from
`MoltresJsonMaterial.C`: `int` truncation of temperatures, missing `CHI_D`
-> `[1, 0, ...]`, unlisted constants -> zeros, the error paths.

## Defects and findings

1. **MeV -> J rounding (port defect, fixed).** Moltres computes `X * 1e6 *
   1.6e-19` (two roundings); the port multiplied by the folded constant
   `1e6 * 1.6e-19`. K13: 2950 of 4096 bit-identical, worst 1 ulp. `tables::
   material_from_samples` now applies the factors in upstream order (4096 of
   4096). Moved numbers: `sigma_power` changes by <= 1 ulp for the table path;
   no recorded k_eff depends on it (S1-S6 are unchanged to the digits above,
   S4's pinned pcm values are unchanged).
2. **Upstream MOOSE defect, reproduced, not corrected.**
   `MonotoneCubicInterpolation::initialize_derivs` (d7c4bde) divides the
   interior three-point numerator by `h[i-1] * h[i] * (h[i-1] * h[i])`; the
   end-knot formulas, and the correct derivative, use `(h[i-1] + h[i])`. For
   y = x^2 on unit spacing upstream gives interior slopes 4 and 8 (true 2 and
   4), 3 and 8 after the limiter; at the 50 K spacing of `newt_fuel_` the
   interior slope is about 25 x too small. The translation keeps it because
   Moltres' gold k_eff were produced by it (S5/S6 match to 1e-14). Pinned in
   `moose_interpolation::tests::monotone_cubic_keeps_moose_interior_slope_formula`.
   Effect on k_eff of the bug was **not measured**. Whether to report it to
   idaholab/moose or to Moltres is a decision for the maintainer.
3. **Harness defects (mine, round 1), fixed:** the kernel test's `lines()`
   used `starts_with`, so a `TBL` query would also match `TBLS`/`TBLM`; and
   `Stat::add` computed `a - b` on `i64` bit patterns, which overflows (and
   panics, release overflow checks being on) when values of opposite sign are
   compared. Neither affected a round-1 result.

## Deliberate deviations from upstream

* `LsqTables`: a missing or short least-squares file is an error; upstream
  leaves coefficients zero / stale.
* `PropertyTable::parse` still rejects equal consecutive temperatures at read
  time (round 1); upstream's spline/monotone `errorCheck` also rejects them.
* Only `sample` and `sampleDerivative` are translated; `sample2ndDerivative`,
  `ADReal` overloads and CSV dumps are not (Moltres calls none of them).
* `BicubicInterpolation` (not `BicubicSplineInterpolation`) is not translated:
  Moltres 3dd2ce7 does not use it.

## Not done

* `peak_power_density` controller on `remxs` (the third item of #1538): small,
  but it is a postprocessor-coupled feedback inside the material and needs a
  place in the port's driver to be exercised; not started.
* `MoltresSNMaterial`, `DiffusionRodMaterial` (spline/monotone/linear copies):
  not touched.
* No solution-level test for bicubic or least squares: Moltres has no input
  for them.
