# Predictions, written before any run (gh:#1400, 2026-11-13)

Committed BEFORE any driver was compiled, before the port gained a table
reader or a Marshak boundary, and before any comparison test existed.
Upstream pin: Moltres `3dd2ce7a60f59424d0d8f46748ba4e243d76b671` (a fresh
clone was checked out at `3dd2ce7`, then its `.git` was discarded to save
disk). MOOSE `LinearInterpolation` is taken from the MOOSE commit that
Moltres' submodule pointer names in its HEAD merge message, `d7c4bde`
(short sha, fetched from raw.githubusercontent.com; the full sha was not
resolved, stated here so nobody assumes it was).

## Route (decided from feasibility, not from a result)

* **A real MOOSE + Moltres build was NOT attempted.** The image has no
  micromamba/conda; disk free was 5.8 GB against a 5 GB floor; the Moltres
  clone does not contain MOOSE (a submodule); 4 cores are shared. The
  conda `moose-dev` environment alone is several GB. Nothing was built, so
  nothing about a fresh Moltres run is claimed.
* **Real Moltres numbers that ARE used** are the `gold/*.e` Exodus files that
  ship in the pinned Moltres tree (`tests/*/gold`). They are outputs of real
  Moltres runs, committed by upstream and checked by upstream CI (`Exodiff`,
  default `rel_err` 1e-6, 1e-4 where the test file says so). They are read
  with scipy's NetCDF reader and the numbers copied to
  `moltres_gold_values.txt`. This is "published reference values in the
  Moltres repo", not a run by us; it is also a weaker check than a rerun
  because we cannot vary the input.
* **Kernel level** is a transcription route: standalone C++ with a minimal
  shim; lines marked `UPSTREAM` are copied verbatim from the pinned tree and
  `check_verbatim.sh` greps for each of them.

## Kernel level (per quadrature point / per cell)

N = 4096 deterministic pseudo-random samples per quantity (LCG, fixed seed).
Floats compared as u64 bit patterns. "Port" = the crate function reached by
the test. Moltres kernels evaluated with `_test = 1.0`, `_prec_scale = 1`
(a unit test function does not change rounding; stated so the reader knows
what was *not* varied).

* K1 linear table interpolation (value and slope), MOOSE
  `LinearInterpolation::sample/sampleDerivative`, on the 12 `newt_fuel_*`
  Moltres property files: port reader **100 % bit-identical** after I
  translate the expression in MOOSE's own order
  `y[i] + (y[i+1]-y[i]) * (x-x[i]) / (x[i+1]-x[i])`. A version written as
  `slope*(x-x[i])` would differ; that is the failure mode being tested.
* K2 `InScatter` (both `sss2_input` index layouts): **100 %** (the kernel
  accumulates `-1*gt*phi`; negation is exact, so the port's positive sum is
  the exact negative).
* K3 `DelayedNeutronSource`: **100 %**. K4 `PrecursorDecay`: **100 %**.
* K5 `CoupledFissionKernel`, `account_delayed = true`:
  `(sum -nsf*phi) * ((1-beta)*chi_p) / k`. The port today evaluates
  `chi_p*(1-beta)/k*S`, a different association. BEFORE any fix I predict
  **20-80 % bit-identical, worst <= 2 ulp**; AFTER translating upstream's
  order **100 %**. This is a transcription defect (operation order), round-off
  class, fixed by translating upstream.
* K6 `PrecursorSource`: upstream `sum_j(beta_i*nsf_j*phi_j)/k`; the port
  forms `(beta_i/k) * (sum_j nsf_j phi_j)`. For G >= 2 this is a different
  grouping of the sum and **cannot** be made bit-identical without changing
  `PrecursorDrift`'s public signature (it takes `S_f`, not the group fluxes).
  Predict **10-60 % identical, worst <= 4 ulp**, NOT fixed, recorded as a
  documented API-shape difference inside the 1e-8 allowance and flagged.
* K7 `FissionHeatSource`: the port stores `kappa*Sigma_f` premultiplied
  (`sigma_power`) and applies `power/tot` outside the group sum; upstream
  multiplies per group inside the sum. Not compared (no common function);
  listed as a gap, not a result.
* K8 `GroupDiffusion`, `CoupledScalarAdvection`, `ConvectiveHeatExchanger`,
  `ScalarTransportTimeDerivative`: FE weak-form residuals against FV face
  fluxes / implicit matrices have no pointwise counterpart. Not compared at
  kernel level; covered at solution level or not at all (stated per row in
  RESULTS).

## Solution level (real Moltres numbers from gold files)

* S1 `bcs/vacuum_bc.i` (Marshak), `mark`, `milne` variants: unit square,
  `D = 1e-3`, `Sigma_r = 0`, Dirichlet on the left, vacuum on the right. The
  exact solution is linear with slope -1, so Moltres' right-edge value is
  `alpha*D` (alpha = 2, sqrt(3), 3*0.710446) per the test's own comments.
  The port has **no** Marshak/Mark/Milne boundary (`flux_boundary` is
  `FixedValue`/`ZeroGradient` only), so BEFORE adding one the port's
  "vacuum" (`FixedValue(0)` at the face) gives a right-edge value of exactly
  0, which misses Moltres by `alpha*D` (2e-3 absolute, 100 % of the
  right-edge value). AFTER implementing the boundary as a `MixedField` Robin
  condition, predict: gold right-edge value reproduced to **<= 1e-9
  absolute** (profile exactly linear in FV and FE; limit is the CG
  tolerance 1e-12 times the condition number of a 10-cell line).
* S2 `materials/gmm_none`, `gmm_linear` (2 groups, natural boundaries, so
  the eigenfunction is uniform and `k_eff = k_inf` for both FE and FV):
  port `k_inf` from the same tables (`sss2_input=false` layout,
  `account_delayed=false`) agrees with Moltres' `k_eff` to **<= 1e-8
  relative**. Derivation: Moltres' Newton eigensolve stops at
  `nl_abs_tol = 1e-12` on a residual of flux O(1e2), so k is good to
  ~1e-10; the port's power iteration is run at `k_tolerance = 1e-12`; both
  discretisations are exact for a uniform field; the data path is the same
  table through the same interpolation. A miss larger than 1e-8 means the
  data path (index layout, units, which entry is removal) is wrong.
* S3 `gmm_spline`, `gmm_monotone_cubic`: the port has linear interpolation
  only. Predict `k_port(linear) - k_gold(spline)` equals the linear-vs-spline
  difference already visible between the gold files (about +5.9e-5 and
  +1.8e-5 in k), to within the 1e-8 of S2. Reported as a model difference,
  child issue filed for spline / monotone-cubic.
* S4 issue case 4, temperature feedback, `newt_fuel` tables 800-1050 K,
  five temperatures {825, 875, 922, 975, 1025} K: table = linear
  interpolation of all constants. Reduced model = the port's
  `sigma_removal_at`, all constants frozen at the 900 K row except
  `Sigma_r`, whose slope is Moltres' own `d_remxs_d_temp` at 900 K (the
  most favourable choice, stated). Prediction: **the two have OPPOSITE sign
  of dk/dT**: in these tables `REMXS(thermal)` falls with temperature
  (0.0425 -> 0.0375 over 800-1050 K), so removal-only feedback raises k,
  while `NSF(thermal)` falls and the full table gives a net fall in k.
  Magnitudes: table `|dk/dT|` in 5-40 pcm/K, reduced in 5-40 pcm/K; the
  reduced model's error at 1025 K exceeds 500 pcm.
* S5 precursor tests (`tests/pre/*`) and MSRE-style axisymmetric cases use
  unstructured `.msh` meshes and gmsh geometry the port has no reader for.
  Not run. Listed as a gap.

---

# Round 2 predictions (gh:#1538, 2026-11-13), written before the new code was compiled or run

Pins. Moltres `3dd2ce7a60f59424d0d8f46748ba4e243d76b671`; its `moose`
submodule pointer resolves to the full sha
`d7c4bde4025537ac4574cc80e78c2407e23a42cc` (round 1 only had the short sha;
resolved this round from `git ls-tree` of the fetched 3dd2ce7 commit). MOOSE
files fetched at that sha: `SplineInterpolation{.C,.h}`,
`SplineInterpolationBase{.C,.h}`, `MonotoneCubicInterpolation{.C,.h}`,
`BicubicSplineInterpolation{.C,.h}`. **Moltres uses `BicubicSplineInterpolation`,
not `BicubicInterpolation`**, so the latter is not translated. **LSQ is not a
MOOSE class**: it is the two-coefficient file branch of
`GenericMoltresMaterial::leastSquaresConstruct` (`value = c0*T + c1`).

Route unchanged from round 1: no MOOSE build; kernel level = C++
transcription with `check_verbatim.sh`, solution level = Moltres' committed
`gold/*.e` k_eff. **Moltres 3dd2ce7 ships no test input for `bicubic` or
`least_squares` and no gold file**, so those two are kernel-level (C++
transcription vs port) only; no solution-level claim will be made for them.

## Kernel level (K9-K12)
* K9 `SplineInterpolation` value and slope, 12 `newt_fuel_*` columns (200
  temperatures each, including below range, on nodes and above range, i.e.
  extrapolation): **100 % bit-identical** predicted, worst case if not
  identical <= 2 ulp (same operation order, no FMA in rustc; C++ compiled
  `-O2 -ffp-contract=off`).
* K10 `MonotoneCubicInterpolation` value and slope, same grid: **100 %
  bit-identical** predicted. Named risk: `std::pow(x,2)` vs `x*x`; GCC folds
  `pow(x,2)` to `x*x`, so I predict no difference; if one appears it is <= 1 ulp
  and attributable to that.
* K11 `BicubicSplineInterpolation` value and both slopes on a synthetic
  5 x 4 surface (no Moltres data exists for it; a fixed smooth non-polynomial
  function sampled on non-uniform grids), 4096 points including outside the
  grid: 100 % bit-identical predicted, <= 4 ulp worst.
* K12 LSQ: `c0*T + c1` is one multiply-add; **100 %** predicted, and the
  slope is exactly `c0`.

## Solution level (S5-S7)
Bound is the same as round 1 for the same quantity: relative difference of
the port's k_inf against the Moltres gold k_eff `<= 1e-8` (round 1 measured
6.1e-14 / 8.4e-15). Natural boundaries make the FE eigenfunction uniform so
`k_eff = k_inf`; this assumption is carried over unchanged from round 1.
* S5 `gmm_spline`, `gmm_monotone_cubic` (newt_fuel_ tables, T = 922 K, 6 rows
  800-1050 K so 922 is interior): `<= 1e-8` each. Equivalent: the port's
  spline minus linear k_inf is -6.39 pcm and monotone minus linear is -1.79
  pcm, matching the gold differences to 1e-8.
* S6 `mjm_none`, `mjm_linear`, `mjm_spline`, `mjm_monotone_cubic`
  (`MoltresJsonMaterial`, `xsdata.json` 600/900/1200 K, `sss2_input = true`,
  T = 900 for none and 922 otherwise): `<= 1e-8` each. Spline minus linear
  predicted -51.6 pcm and monotone minus linear -17.8 pcm from the gold files
  (three-point tables, so the spline departs from linear far more than in
  S5). `mjm_none` uses `xsdata-900K.json`.
* S7 JSON reader behaviours tested because upstream does them: temperatures
  are truncated to `int` (`get<int>()`); a missing `CHI_D` becomes `[1, 0,
  ...]` for every row; a constant not listed in `group_constants` is zeros.

## What would falsify these
A kernel difference > 4 ulp, or a solution-level miss > 1e-8, would mean a
translation defect (most likely the in-place `_yp` update in the monotone
limiter loop, or the scatter layout in the JSON path) and would be reported
as such, not absorbed into a tolerance.
