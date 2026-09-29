# pyDOSEIA code-to-code verification

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions. Dose results here are research-grade only, never a dose to a real person.

## What this is

A **code-to-code** verification of the Rust port in `src/pydoseia/` against
the **upstream pyDOSEIA Python** (MIT, Copyright (c) 2024 Dr. Biswajit Sadhu;
<https://github.com/BiswajitSadhu/pyDOSEIA>, commit
`dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce`). Every reference number comes from
**executing upstream**. None is re-derived and none is transcribed from the
paper (Sadhu et al., *Health Physics* 130(1) (2026) 94–110,
doi:10.1097/HP.0000000000002014, PMID 40622262).

It answers one question: **does the port compute what pyDOSEIA computes?** It
says nothing about whether pyDOSEIA's model represents reality. There is no
validation against measurement or against an independent code in this pass.

## Methodology

```
verification_and_validation/pydoseia_code_to_code/gen_pydoseia_reference.py
  + tranche2.py (imported by it; second tranche, 2026-09-28)
    imports upstream from vendor/pyDOSEIA, runs it on synthetic inputs
    -> tests/data/pydoseia_reference.csv          1 899 rows, 18 127 values
    -> tests/data/pydoseia_synthetic_*.csv        the inputs, as upstream read them
        -> tests/pydoseia_code_to_code.rs         replays every row through the port
           + tests/pydoseia_c2c/tranche2.rs       (the second tranche's groups)
```

**Inputs are synthetic, and why.** Upstream's coefficient tables are
ICRP-derived (inhalation) or FGR-15 (external), and its met file has no stated
provenance (see [`pydoseia-port-scoping.md`](pydoseia-port-scoping.md)). None of
them may go into the fixture. The generator therefore writes **synthetic**
tables in upstream's exact file layouts into a temporary `library/` directory,
changes into that directory (upstream opens `library/...` relative to the
working directory), and runs upstream against them:

- eight nuclides `SYN-1`…`SYN-8`, with element symbols Co, I, Cs, Kr, F, Sr, Tc
  and H chosen to hit every element branch in the deposition-velocity and
  weathering code;
- an inhalation table with several absorption types per nuclide, an element
  heading row, and a row with a blank type whose huge coefficient would
  dominate every maximum if it were not dropped;
- surface and submersion tables that include the daughters, and a `SYN-9m` row
  that upstream's substring daughter lookup picks up (D4);
- decay chains whose daughters straddle the 1 800 s progeny threshold, plus a
  1e6 s threshold;
- two "years" (30 and 25 days) of hourly met records from a seeded generator,
  with about 2 % blanks per column, direction values on bin edges (0, 11.25,
  348.75 and 360 degrees), speeds on class edges (1.8, 3.0 and 74.5 km/h) and
  one beyond the last class (80 km/h).

The Rust side reads **the tables as upstream read them back**. The generator
writes each committed CSV from `pandas.read_excel` of the file upstream opens,
because the xlsx round trip moved some values by one ulp (the first run showed
3e-16 mismatches in every coefficient group until this was fixed).

**Grid.** sigma_y and sigma_z: 6 classes × 11 distances straddling the 100 m and
1 000 m band edges. Height correction: 6 × 5 release heights (including below
10 m) × 2 measurement heights. Master equations: centreline and offset
receptors. Dilution without met: 2 modes × 5 distances × 3 heights × with and
without mean-speed scaling. Met: raw TJFD, missing-corrected TJFD and calm
factors per year and class, speed means, and the long-term per-sector dilution
factor at 4 distances with and without calm correction, for operating windows
0–24 h and 6–18 h. Half-life parsing: 11 strings covering every unit, `ms`
included. DCF lookups: 5 absorption types × 12 ages (on and around every
bracket edge), and 3 progeny settings × 12 ages for both external tables.
Doses: 2 modes × 4 ages × 2 types × weathering on/off × progeny on/off × 2
dilution factors, for all 8 nuclides and all 3 pathways.

**Second tranche (added 2026-09-28).** Same rules, more synthetic tables,
written by `tranche2.py` into the same temporary library and exported as
`tests/data/pydoseia_synthetic_{ingestion_dcf,eco_param,gamma_lines,attenuation,screening_*}.csv`:
an ingestion coefficient table (with a duplicate row, so the maximum is
exercised, and `HTO`/`OBT`/`C-14` rows, because upstream branches on those
exact names; the numbers are synthetic), an eco-parameter table with a decoy
`Csx` row (passes upstream's substring pre-filter, must fail its token regex)
and a `Sr Y` row, gamma lines including one below 50 keV, one below 1e-3
probability and a `SYN-1D` line that upstream's substring match hands to
`SYN-1`, synthetic smooth air attenuation curves (not NIST values), six
screening tables with `SCR-*` nuclides (one with an alternate DOE name, one
`_VAPOUR` suffix), and a nomenclature file. The grid:

- `quadpack`, `tplquad`: SciPy's `_quadpack._qagse` called **directly** on
  seven integrands (smooth, end-point singular, logarithmic, oscillatory,
  Runge, divergent `1/x`, a narrow spike) at two tolerances, recording value,
  error estimate, `ier` and evaluation count; and one `tplquad`.
- `ingestion`: ten nuclide lists (plain; with H-3; with C-14; with both; each
  alone; exactly `[H-3, C-14]`; H-3 first; C-14 in the middle; an element-H
  nuclide not named H-3) × both release modes × adult/infant × two `chi/Q`,
  plus eight variations of soil, parameter set (`dosefunc` fallback or input
  generator), tritium and C-14 product lists (including a list that starts
  with meat), climate, vegetable and feed type. Raises and `None` results are
  recorded as such. `dcf_ingestion` (12 ages), `eco_lookup`,
  `ingestion_weathering_unused`, `zeroing`.
- plume shine: `gamma_lines` for five nuclides, `attenuation` at twelve
  energies (below, on and above the table's knots), the three limit functions
  over 6 classes × 4 distances × 3 heights × 2 mean free paths, and
  `plumeshine_dose` itself for three nuclides (two lines, one line, pure beta)
  as a single plume at two receptors, long-term without met at two, and
  long-term with the synthetic met record at one distance (16 sectors);
  `point_source` with upstream's defaults and a custom case, both units.
- `dcf_screening`: three `SCR-*` nuclides × five absorption types × six ages.
- `plume_rise`: defaults and a 48-point grid (neutral) and 12 (stable).
- the driver: `OutputFunc.dose_calculation_script` run as upstream runs it
  (joblib included) for four scenarios (long term without met, 4 nuclides;
  single plume with H-3 and C-14; long term with met and calm correction;
  a user-supplied `chi/Q`), recording the distances, per-distance dilution
  factors and maximum, every (distance, age) dose and ingestion array, the
  report's surface and submersion coefficient pairs, `main.py`'s summed
  summary table, the plant-boundary totals parsed back from the text report,
  and whether an age of 10 raises.

**Pass criterion.** Per group, the relative deviation must be at most the
tolerance below. NaN must match NaN, and an exact 0 must match an exact 0.
**Tolerance 0 means bit-exact**: the port performs upstream's IEEE-754
operations in upstream's order and calls the same libm `pow`. Groups that pass
through numpy's `exp` allow a few ulp, because numpy dispatches `exp` to
CPU-specific SIMD kernels. They measured bit-exact here anyway. `speed_means`
goes through a pandas mean with a different summation order.

**Can it fail?** Ten mutation tests swap in a plausible alternative and assert
that the fixture **rejects** it: the exact 22.5-degree sector width instead of
upstream's `0.39275`; `ln 2` instead of upstream's `0.693`; the corrected calm
correction (D1); an exact-match daughter lookup instead of upstream's substring
match (D4); an age of 1 treated as 1.5 (a bracket-edge slip); and, in the
second tranche, the workspace's existing GSL `qag` (`petir`) in place of the
`dqagse` port; an exact-name gamma-line lookup instead of the substring (D24);
the corrected per-nuclide ingestion driver (D8-D11); `zeroing_ingestion` as
intended instead of pandas 3's no-op (D14); and the summary's ingestion
counted once (D20). All ten are rejected.

The `petir` mutation also records why ~~the integrator had to be ported rather
than reused~~ petir's plain `qag` is not a substitute for `dqagse` (measured
2026-09-28): `qag` agrees with `dqagse` to the last bit
on the smooth integrands, differs by up to 7e-16 on `sin(50x)`, by 4.5e-5 on
`1/sqrt(x)` at `eps = 1.49e-3`, by 1.7e-6 on `ln x`, and fails outright
(`MaxIterations`) on `1/sqrt(x)` at `1.49e-8`, where `dqagse` converges by
extrapolation. **CHANGED 2026-09-28:** that gap was closed in petir rather
than kept as a second integrator here: petir now ports GSL's `qags` (the
extrapolating routine), and plume shine runs on it by default. See "Plume
shine on petir" below.

### Plume shine on petir (2026-09-28)

**Why.** Maintainer: "can you replace QUADPACK integrator with stuff from
petir? i don't want so many duplicate integrators here" … "or rather, include
petir as a dependency to buangkok. quadpack should be used as regression test,
but petir is the main one".

**What changed.** `petir::integration::qags` / `qags_with_status` is a port of
GSL 2.8 `gsl_integration_qags` (`integration/qags.c`, with `qelg.c`,
`qpsrt.c`, `qpsrt2.c`, `util.c`), verified against GSL compiled from the
vendored tree (`crates/petir/tests/gsl_qags_code_to_code.rs`: 21 cases
reaching every exit but `GSL_EFAILED`; statuses, sub-interval and evaluation
counts identical in all 21, results 21/21 bit-identical, error estimates 19/21
bit-identical and 1.4e-6 relative on the other two, a libm `pow` ulp
amplified by QUADPACK's cancelling `errsum`, reproduced by perturbing GSL
itself). `plume_shine::petir_tplquad` nests it exactly as SciPy's `tplquad`
nests `dqagse` (outer `z`, middle `y`, inner `x`; same `epsabs`/`epsrel` at
every level; 50 sub-intervals per level). `PlumeShineIntegrator::Petir` is the
default in `line_integral` and the functions above it, and in
`PyDoseiaConfig::plume_shine_integrator`;
`PlumeShineIntegrator::ScipyQuadpackReference` selects the SciPy port, and the
code-to-code fixture selects it explicitly.

**QAGS or plain QAG? Measured first.** On the fixture's plume-shine cases (the
five geometries, four gamma lines, six classes: 120 triple integrals at
pyDOSEIA's tolerances), nested petir `qag` fails **no** call (0 of 265 812),
all 120 results are within the derived bound below, 106 are bit-identical to
the reference and the worst relative difference is 4.3e-16. So plain `qag`
**does** meet the requested tolerance here, and the measurement alone did not
force QAGS. QAGS was chosen for three reasons: (1) it is the algorithm
upstream runs (`dqagse`), and on these cases it is bit-identical to it, which
keeps the default path's numbers equal to upstream's; (2) the kernel carries a
`1/r^2` point singularity at the receptor, and for a receptor inside the
integration box the inner integrals are sharp Lorentzian peaks and the outer
ones have logarithmic singularities, which is what the extrapolation exists
for (the fixture's receptors are mostly at `z = 0`, below the 1 m floor of the
`z` range, so they do not probe this hard); (3) `qag` discards its estimate on
failure, whereas `qags_with_status` returns it with the status, as
`scipy.integrate.quad` does. On a smooth integrand QAGS makes the same
bisections as QAG, so it costs nothing extra there.

**Regression test** (`tests/plume_shine_petir_vs_quadpack.rs`). Methodology:
the 120 integrals above, petir against the reference, pass criterion derived
from the requested tolerances only: per integrator the nested nominal error is
at most `E = epsabs (Ly Lz + Lz + 1) + 3 epsrel |I|` (positive kernel; `Ly`,
`Lz` the ranges), so two integrators may differ by `2E`. Results
(2026-09-28): **120 of 120 bit-identical, max relative difference 0**;
265 812 QAGS calls against 55 560 for a run with no subdivision anywhere (the
adaptive paths are exercised), none uncertified. The derived bound is
**uninformative** at these tolerances: the smallest `2E / |I|` over the cases
is 716, because pyDOSEIA's `epsabs` (1.49e-2 single plume, 1.49e-3 sector
averaged) is absolute and exceeds the integrals (order 1e-2 and below). That
is a property of upstream's tolerance, reported rather than tightened. The
check that can fail is `petir_plume_shine_also_reproduces_the_fixture` in
`tests/pydoseia_code_to_code.rs`: the petir path through the fixture at the
fixture's own tolerances. Result: **all 120 plume-shine values bit-identical**
(first written as a mutation test expecting rejection; the fixture cannot tell
the two integrators apart, so it pins the agreement instead).

## Results

Taken **2026-09-28** on x86-64 Linux: upstream `dca4cdc3`, Python 3.14.7, numpy
2.5.3, pandas 3.0.6, scipy 1.18.1, openpyxl 3.1.5. The generator's
`--check` mode regenerates the fixture identically. First tranche: **1 015
cases, 13 422 values, 20 groups, all passing** (unchanged by the second
tranche: the same rows, the same deviations). Second tranche, same day and
versions (joblib 1.6.0): **884 cases, 4 705 values, 23 groups, all passing**.
In total **1 899 cases, 18 127 values, 43 groups**.

| Group | Cases | Values | max rel dev | Tolerance |
|---|---:|---:|---:|---:|
| `sigmay` | 66 | 66 | 0 (exact) | 0 |
| `sigmaz` | 66 | 66 | 0 (exact) | 0 |
| `height_factor` | 60 | 60 | 0 (exact) | 0 |
| `master_single` | 10 | 10 | 0 (exact) | 4e-16 |
| `master_sector` | 6 | 6 | 0 (exact) | 4e-16 |
| `dilution_no_met` | 120 | 420 | 0 (exact) | 4e-16 |
| `tjfd` | 24 | 3 840 | 0 (exact) | 0 |
| `tjfd_missing` | 24 | 3 840 | 0 (exact) | 0 |
| `calm_factors` | 4 | 64 | 0 (exact) | 0 |
| `speed_means` | 2 | 12 | 4.83e-16 | 2e-15 |
| `dilution_met_long_term` | 16 | 256 | 0 (exact) | 1e-15 |
| `half_life` | 22 | 22 | 0 (exact) | 0 |
| `dcf_inhalation` | 60 | 480 | 0 (exact) | 0 |
| `dcf_surface` | 72 | 576 | 0 (exact) | 0 |
| `dcf_submersion` | 72 | 576 | 0 (exact) | 0 |
| `deposition_velocity` | 1 | 8 | 0 (exact) | 0 |
| `effective_lambda` | 6 | 48 | 0 (exact) | 4e-16 |
| `dose_inhalation` | 128 | 1 024 | 0 (exact) | 0 |
| `dose_ground_shine` | 128 | 1 024 | 0 (exact) | 4e-16 |
| `dose_submersion` | 128 | 1 024 | 0 (exact) | 0 |

Second tranche:

| Group | Cases | Values | max rel dev | Tolerance |
|---|---:|---:|---:|---:|
| `quadpack` | 56 | 56 | 0 (exact) | 0 |
| `tplquad` | 1 | 1 | 0 (exact) | 0 |
| `ingestion` | 96 | 768 | 0 (exact) | 0 |
| `dcf_ingestion` | 12 | 72 | 0 (exact) | 0 |
| `eco_lookup` | 7 | 43 | 0 (exact) | 0 |
| `ingestion_weathering_unused` | 2 | 10 | 0 (exact) | 0 |
| `zeroing` | 1 | 6 | 0 (exact) | 0 |
| `gamma_lines` | 10 | 14 | 0 (exact) | 0 |
| `attenuation` | 12 | 48 | 0 (exact) | 0 |
| `limits_single` | 144 | 864 | 0 (exact) | 0 |
| `limits_sector` | 144 | 864 | 0 (exact) | 0 |
| `limits_legacy` | 144 | 864 | 0 (exact) | 0 |
| `plume_shine_single` | 2 | 36 | 0 (exact) | 0 |
| `plume_shine_long_term` | 2 | 36 | 0 (exact) | 0 |
| `plume_shine_met` | 1 | 48 | 0 (exact) | 1e-14 |
| `point_source` | 4 | 26 | 0 (exact) | 0 |
| `dcf_screening` | 90 | 180 | 0 (exact) | 0 |
| `plume_rise` | 62 | 62 | 0 (exact) | 0 |
| `driver` | 53 | 484 | 0 (exact) | 0 |
| `driver_dcf_report` | 10 | 72 | 0 (exact) | 0 |
| `driver_summary` | 12 | 60 | 0 (exact) | 0 |
| `boundary_totals_text` | 18 | 90 | 3.68e-4 | 6e-4 |
| `driver_raises` | 1 | 1 | 0 (exact) | 0 |

Tolerances of the second tranche: 0 everywhere the port performs upstream's
operations in upstream's order. `plume_shine_met` allows 1e-14 because numpy's
`einsum` sums 54 products per sector in an order it chooses; it measured
bit-exact. `boundary_totals_text` is read back from upstream's **text** report,
which prints four significant figures, so its tolerance is half a unit in the
fourth figure (5e-4 for a leading 1, with margin); the unrounded numbers it
prints are the `driver` and `driver_summary` groups, which are exact. In the
`ingestion` group 28 cases raise (D8, D11b), 8 return `None` (D9), and the
rest are arrays of three shapes, including the transposed one (D9); the port
reproduces each outcome and each shape.

**Interpretation.** Of the 43 groups, 41 are bit-exact; `speed_means` is 2 ulp
from a pandas reduction and `boundary_totals_text` is limited by the report's
printing. The plume-shine groups being bit-exact means that the QUADPACK
port subdivides exactly as SciPy does (the adaptive quadrature would amplify
any difference in the integrand or the error estimate into a different
subdivision). The driver groups compare against upstream's own joblib run, so
replacing joblib by a loop is verified too. The port reproduces upstream's arithmetic
exactly, including its NaN results (a nuclide with no row of the requested
absorption type gives a NaN coefficient and a NaN inhalation dose upstream;
the fixture contains 112 such rows and the port matches each one). Being
bit-exact means the translation of every ported path is verified. It does not
show that the model is right: a wrong formula carried over faithfully would
pass too, which is why the defects below were looked for separately, by
reading the code.

Rerun:

```bash
python3 crates/buangkok/verification_and_validation/pydoseia_code_to_code/gen_pydoseia_reference.py --check
cargo test --release -p buangkok --test pydoseia_code_to_code -- --nocapture
```

(`--check` regenerates the synthetic input CSVs, which is deterministic, and
diffs the fixture only.)

## Upstream defects found while porting

The port is **faithful**: every defect below is reproduced, except those in
code that cannot run or is never called. Where a corrected variant exists, it
is a separate, labelled **divergence**, and the default follows upstream.

| # | Where (upstream `dca4cdc3`) | Defect | Effect | In the port |
|---|---|---|---|---|
| D1 | `metfunc.py` `calm_correction_factor_calc` | `N_L` is `TJFD.reshape(6,10,16).sum(axis=1).sum(axis=0)[1]`, the total over all speed classes **in direction sector 1**. The formula `1 + N_0 N_JL / (N_L N_J)` needs the total in the **lowest non-calm speed class** over all sectors | Every calm-correction factor uses the wrong denominator. On the synthetic met data the corrected factors differ from upstream's, and the fixture rejects them (mutation test `mutation_corrected_calm_correction_is_rejected`) | Reproduced in `met::calm_correction_factors` (`CalmCorrection::Upstream`). Corrected in `met::calm_correction_factors_lowest_speed_class` (`CalmCorrection::LowestSpeedClassTotal`), **not checked against the Hukkoo–Bapat manual**, which was not available |
| D2 | `metfunc.py` `dilution_per_sector`, `dosefunc.py` `inhalation_dose` | `dilution_factor_sectorwise.T[0].max()` on a one-dimensional array of six classes is the **first element** (class A), not the maximum | `max_dilution_factor` after the no-met modes, and the inhalation dose when called without an explicit `chi/Q`, use class A's value. Upstream's own driver avoids it by recomputing the maximum | Reproduced as `dispersion::upstream_internal_max_dilution_factor` (in the fixture). `dispersion::max_dilution_factor` is the real maximum, matching the driver |
| D3 | `metfunc.py` `dilution_per_sector`, single plume with met data | `dilution / mean_speeds[:, None]` broadcasts a `(6,)` array to `(6, 6)`, and the function's own `assert shape == (6,)` then fails. The means are also in **km/h** (the met column), while the dilution factor is per 1 m/s | This release mode **cannot run**. If the shape were fixed, the result would be too small by a factor of 3.6 | ~~Not ported: there is nothing to verify against~~ **2026-09-28:** still unverifiable against upstream; a corrected version (each class divided by its mean speed in m/s) is provided as a labelled divergence, `dispersion::dilution_single_plume_with_met_speeds`, with a unit test. The driver keeps upstream's behaviour and reports `AssessmentError::SinglePlumeWithMetCannotRun` |
| D4 | `raddcffunc.py` `dcf_list_ecerman_*_include_progeny` | The daughter coefficient is looked up with `str.contains` (substring, regex), so `Y-90` also matches `Y-90m` and the larger value wins | A daughter can get a different nuclide's coefficient | Reproduced in `dcf::ExternalDcfTable::lookup_contains`, and in the fixture through `SYN-9` / `SYN-9m` |
| D5 | `metfunc.py` `building_wake_effect_gifford` | Uses `self.sigmay` and `self.sigmaz`, which are **bound methods**, as numbers (a `TypeError` if called), and multiplies by `U` where Gifford's formula divides | Unusable; marked "TO-DO" and never called | ~~Not ported~~ **2026-09-28:** a corrected version (sigmas as arguments, divide by `U`, upstream's one-third floor) is a labelled divergence, `plume_rise::building_wake_gifford`; not checked against the AERB guide or TECDOC-379 |
| D6 | `metfunc.py` `compute_plume_rise_stable_cat` | `S` is assigned for class E and then overwritten for F; `Dh` is computed by one formula and then overwritten by another | Always class F, second formula; marked "TO-DO" and never called (it does run) | ~~Not ported~~ **2026-09-28:** ported faithfully, `plume_rise::plume_rise_stable_upstream` (`plume_rise` group); the divergence `plume_rise_stable_both_formulas` takes the class's `S` and returns both formulas, leaving the choice to the caller |
| D7 | `outputfunc.py` `agewise_dcfs_inh_gs_submersion` vs `dosefunc.py` `inhalation_dose` | The DCF **reported** comes from the multi-source maximum (`get_dcfs_for_radionuclides`), but the dose uses `inhalation_dcf_list` on `RadioToxicityMaster.xls` | The report's inhalation coefficient need not be the one used in the dose. Found by reading the code; the disagreement itself is **not demonstrated** in the fixture (its `SYN-*` nuclides are not in the synthetic screening tables) | ~~The port has only the path the dose uses~~ **2026-09-28:** both paths are ported and verified separately (`dcf_inhalation`; `dcf_screening`, `driver_dcf_report`); the report uses `assessment::dcf_report`, the dose `dcf::InhalationDcfTable::lookup`, as upstream |
| D8 | `dosefunc.py` `ingestion_dose`, `raddcffunc.py` `fv_list_ecerman_ingestion`, `ingestion_weathering_correction_real` | The per-element lists (transfer factors, removal rates, concentrations) **skip** H and C but are **indexed by the position in the full nuclide list**; the dose loop skips by *name* (`H-3`, `C-14`), the lists by *element* (`H`, `C`) | Correct only when every H and C nuclide comes after all the others and they are named `H-3`/`C-14`. Otherwise every later nuclide reads another's entry or, as in every case in the fixture (`h3_first`, `c14_middle`, `h_named_syn8`), upstream raises `IndexError` | Reproduced (`ingestion::upstream::ingestion_dose_upstream`, `UpstreamIngestionError::IndexError`). Corrected in `ingestion::corrected::ingestion_dose_per_nuclide` (the caller names the model per nuclide) |
| D9 | `dosefunc.py` `ingestion_dose` (output assembly), `outputfunc.py` `dose_calculation_script` | With H-3, C-14 and other nuclides the stacked array is **transposed** to 3 × n; with exactly `[H-3, C-14]` no branch matches and the function returns the previous call's result (`None` on a fresh object). The driver then reshapes every cell to n × 3 in C order | For the transposed case the driver **scrambles** routes across nuclides (in the `single_plume_h3_c14` scenario the infant's reported ingestion for SYN-3 at the boundary is 11 860 mSv, almost all of it C-14's vegetable dose). The `None` case crashes the driver | Reproduced, including the scrambling (`assessment::driver_ingestion_matrix`). The corrected driver gives one row per nuclide |
| D10 | `dosefunc.py` `ingestion_dose`, C-14 branch | `C_air = discharge * chi/Q` with the discharge in **Bq/y** for a long-term release; the H-3 branch divides by `365 * 24 * 3600`, C-14 does not | For a long-term release the C-14 plant concentration, and so every C-14 ingestion dose, is **3.15e7 times** too large (Bq s/(y m^3) used as Bq/m^3) | Reproduced; corrected in the corrected driver |
| D11 | `dosefunc.py` `ingestion_dose`, C-14 and H-3 animal loops | (a) C-14: the milk and meat doses are added for **each** product whenever the product *list* contains any milk (resp. meat), so `[cow_milk, goat_meat]` adds both products to both routes. (b) H-3: a debug `print` of `sum_hto_obt_animal_milk_tritium` runs after every product, so a list whose first product is not a milk raises `NameError`. (c) `cow_meat` is in the meat list but not in the ratio tables (`KeyError`) | (a) C-14 milk and meat doses wrong for mixed lists; (b) the `meat_first` cases raise in the fixture | Reproduced; corrected in the corrected driver (per-product routes, no `NameError`); `cow_meat` is not offered (`AnimalProduct` has no such variant) |
| D12 | `dosefunc.py` `ingestion_dose` fallback diet | When the config has no `inges_param_dict_adult`/`_infant`, the fallback values (76.7 kg vegetables, 182.5 L milk, ...) are **annual** amounts, and the pathway multiplies them by 365 as if they were daily (the input generator's defaults, 1.05 kg/d, are daily) | A 365-fold overestimate of ingestion with the fallback diet | Reproduced (`DietaryIntake::DOSEFUNC_FALLBACK_*`, used in half of the `ingestion` cases); the input-generator values are separate constants |
| D13 | `outputfunc.py` `agewise_ingestion_dose` | Only `age > 17` (adult) and `age == 1` (infant) have a receiver; any other age leaves `ingestion_dose_values` unbound | A run with, e.g., `age_group: [1, 10, 18]` raises `UnboundLocalError` (fixture: `driver_raises`) | Reproduced as `IngestionFailure::AgeHasNoReceiver` for that cell; the other cells are still computed |
| D14 | `dosefunc.py` `zeroing_ingestion` | `df.loc[rad][1:] = 0` is a chained assignment; under pandas copy-on-write (pandas 3) it changes a copy | The report's milk/meat doses of elements without transfer factors are **not** zeroed with pandas 3 (fixture: `zeroing`); with pandas < 3 they were. The effect is small (those doses are 0 anyway unless NaN) | The fixture's behaviour is `assessment::Zeroing::ChainedAssignmentNoOp`; the intent is `Zeroing::ZeroMilkAndMeat` |
| D15 | `raddcffunc.py` `RaddcfFunc.point_source_dose` (method) | Calls `self.rads_list()` (a list), `.append`s to a dict, unpacks three of `gamma_energy_abundaces`'s four return values | Cannot run | Not ported; the module-level `point_source_dose`, which runs, is (`point_source`) |
| D16 | `dosefunc.py` `plumeshine_dose`, long-term with met data | The per-class integrals are weighted by the TJFD **hour counts** (divided by speed and height factor) and summed, with no division by the hours of data (`hours_without_calm` is computed and unused; a `time_unit` factor is commented out). Comments call the result µSv/h; the report heads it "microSv per year" | The met-data plume shine scales with the length of the met record, and its unit is not stated consistently | Reproduced (`plume_shine::per_sector_with_met`); the port types plume shine as plain `f64` "upstream values", not as a dose |
| D17 | `raddcffunc.py` `gamma_energy_abundaces` | The cut is `E/1000 < 0.05` (**50 keV**); the comment says 5 keV | Lines between 5 and 50 keV are dropped | Reproduced (a 30 keV line in the fixture is dropped) |
| D18 | `raddcffunc.py` `screen_Annex_H_ICRP119_...` | `pd.read_csv(self, file_path, ...)` passes the object as the path; the `except` returns an error string | ICRP 119 Annex H (soluble/reactive gases) never contributes to the screening | Reproduced by omission: `dcf_screening` has no Annex H input |
| D19 | `raddcffunc.py` `screen_Table_A2_DOE_STD_1196_2011_...` | Its renamed columns are `inh_5_year`, `inh_10_year`, `inh_15_year` (singular) where every other table has `..._years` | DOE-STD-1196 values are **never** seen for ages 2-17; only infant, 1-year and adult | Reproduced (`ScreeningSource::age_columns`; visible in `dcf_screening` for `SCR-1`/`SCR-2`) |
| D20 | `main.py` `reshape_dose_data` | The ingestion total is a group sum over `reshape_ingestion_dose_data`'s rows, which include its own `SUM` row | `summary_summed_inh_gs_sub_dose.csv` counts ingestion **twice** (its per-nuclide text report does not) | Reproduced (`SummaryIngestion::UpstreamDoubleCounted`, `driver_summary`); `SummaryIngestion::PerNuclideRowsOnly` is the divergence |
| D21 | `main.py` `reshape_dose_data` | The detailed rows are built `[dist, ag, ...]` under the columns `['Age (y)', 'Distance (m)', ...]` | `summary_detailed_inh_gs_sub_dose.csv` has its age and distance **labels swapped**. Found by reading; not executed | Not reproduced (the port returns typed fields, not a labelled table) |
| D22 | `outputfunc.py` `agewise_dcfs_inh_gs_submersion` | Calls the ground-surface look-up with `consider_progeny=True` explicitly | The **reported** ground-surface coefficient always includes progeny, whatever the configuration (the dose follows the configuration) | Reproduced (`assessment::dcf_report`, `driver_dcf_report`) |
| D23 | `outputfunc.py` `output_to_txt` | The guard of the Hukkoo-Bapat self-test reads the key `'like_to_scale_with_mean_speed:'` (trailing colon) | Whenever the other conditions hold, `KeyError`: the self-test never runs. Found by reading; not executed | The table is ported as a Rust test and reproduced exactly |
| D24 | `raddcffunc.py` `gamma_energy_abundaces` | Nuclide rows are matched with `str.contains` (substring, regex), like D4 | `Co-6` would take `Co-60`'s lines; in the fixture `SYN-1` takes `SYN-1D`'s 520 keV line | Reproduced (`GammaLineTable::lines_for`); the exact-match alternative is rejected by a mutation test |
| D25 | `dosefunc.py` `ingestion_dose`, H-3 branch; `configs/input.yaml` | Iterates `[self.config['veg_type_list']]`, so the value must be a single string; upstream's own sample `configs/input.yaml` gives a list | `TypeError: unhashable type: 'list'` for that sample whenever H-3 is present. Found by reading; not executed | The port takes one `VegetationType` |
| D26 | `auto_input_generator_funcs_class.py` | `get_climate_h3c14` offers `'arctic'` and `'meritime'`, which the pathway's dictionary spells `'Arctic'`, `'Maritime'`; `get_inges_param_dict`'s custom branch assigns an undefined `V` | A generated config with those climates raises `KeyError` in ingestion; custom ingestion parameters cannot be entered (the loop re-prompts). Found by reading; not executed | The port's `Climate` enum has the four pathway entries; the prompts are not ported |

### Upstream behaviours that are kept, stated so they are not mistaken for port errors

Second tranche:

- For H-3 the air concentration is divided by a year's seconds **also** for a
  single plume, whose release is in Bq, not Bq/y.
- The diet is per day: times 365 for a long-term release, times 1 for a
  single plume (`consumption_time_food` is never read).
- The plant-boundary total and `main.py`'s total **exclude plume shine**.
- Plume shine for a pure beta emitter (or a nuclide whose lines are all cut)
  integrates a zero-energy placeholder line and multiplies by zero. The port
  skips the integral and returns the same zero.
- The single-plume integral's tolerance is 1.49e-2, the sector-averaged one's
  1.49e-3 (relative and absolute): loose, but upstream's.
- Integration limits floor `z` and `x` at 1 m and use `3.14`, not pi, for the
  sector's half-width.

First tranche:

- A record with a missing **speed** (999) but a valid direction and class
  counts as valid in the missing correction, yet falls outside every speed
  class and is dropped from the TJFD.
- The missing correction truncates counts to integers (`astype(int)`).
- The operating hours are `|start - end|`, while the hour filter keeps both
  ends inclusive. For a 6–18 h window, 13 hourly values per day are kept, but
  12 hours per day are counted in the non-calm-hours denominator.
- `sigma_z`'s three band fits meet only approximately at 100 m and 1 000 m,
  with jumps of at most 0.84 % (class E at 1 000 m).
- The sector width is `0.39275` rad, not 0.392699; `ln 2` is `0.693`.
- There are two year lengths: 31 556 952 s in the primary half-life parser, and
  365 days in the progeny parser, the ground-shine build-up and the dose
  pathways.
- Every age above 1 year breathes the adult rate (8 400 m^3/y).
- Iodine gets the aerosol deposition velocity (1 000 m/d), not the 0.1 m/s
  given to F, Cl and Br.
- A release height below 10 m is raised to 10 m in the wind-speed height
  correction only.
- The sampling-time correction to `sigma_y` is computed but never applied.
- For an instantaneous release, ground shine uses the long-term formula, with
  `Q` spread over a year.
- The progeny chain file is read even when progeny are off, so a nuclide
  missing from it crashes upstream. The port only reads the chains when
  progeny are on (a difference in error behaviour only).
