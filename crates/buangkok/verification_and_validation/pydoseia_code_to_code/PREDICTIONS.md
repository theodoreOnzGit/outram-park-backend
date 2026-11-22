# Predictions, written BEFORE the comparison runs (gh:#1429, 2026-11-13)

Committed ahead of the runs so they can fail. Upstream: pyDOSEIA
`BiswajitSadhu/pyDOSEIA` at `dca4cdc3bb0bef7f7e692c8991cf536c91e7a4ce` (MIT), cloned
over git into fresh scratch, run with `python -I` from a venv (Python 3.11.18,
numpy 2.4.6, pandas 3.1.1, scipy 1.17.1, openpyxl 3.1.5, joblib 1.6.0, pyyaml; the
committed fixture was made with numpy 2.5.3, pandas 3.0.6, scipy 1.18.1).

**Disclosure of timing.** Before this file was written I had already observed, by
running upstream only (no Rust run yet):

1. `generator --check` against the committed fixture reports "FIXTURE DIFFERS" in this
   environment (a rows-by-key diff: 103 values differ, all in groups that go through
   `exp` or through a table read: `dilution_no_met`, `ingestion`, `driver`,
   `driver_summary`, `dcf_screening`, `plume_shine_*`, `master_single`);
2. for `speed_means`, a Python-only diagnostic showed that pandas' `Series.mean()` equals
   `np.add.reduce(a) / n` in 12 of 12 cases and a left-to-right sum / n in only 3 of 12.

So P1 is a prediction about the Rust port, made after seeing the Python side.
Everything about the Rust side below was predicted before its first run.

A third observation, made while setting the environment up and also before any Rust run
of the new rows: `np.exp` and glibc's `math.exp` (the one the port calls through Rust's
`f64::exp`) differ by one ulp on 9 190 of 200 000 random inputs in `[-50, 5]` on this
machine (numpy 2.5.3, glibc 2.39), whatever `NPY_DISABLE_CPU_FEATURES` says. So a fixture
regenerated here differs from the committed one in the groups that go through numpy's
`exp`; the committed fixture was generated on a machine where numpy's `exp` agreed with
glibc's.

| # | claim | predicted outcome |
|---|---|---|
| P1 | `speed_means` (4.83e-16, tolerance 2e-15). Cause: `met::speed_distribution` sums with a left-to-right `iter().sum()`; pandas `nanmean` is `values.sum() / count` through numpy's pairwise `add.reduce` (8 accumulators, block 128; the classes hold 29 to 386 values). Fix: use the crate's existing `assessment::numpy_pairwise_sum` | all 12 values bit-identical after the fix; the group tolerance goes from 2e-15 to 0 |
| P2 | `boundary_totals_text` (tolerance 6e-4, measured 3.68e-4) is limited by pandas' 6-significant-figure `to_string` of upstream's report, not by the port. New group `boundary_totals`: the same `df_all` frames captured in memory at full precision (`DataFrame.to_string` wrapped during `output_to_txt`) | 18 cases (3 scenarios: 4 + 8 + 2 nuclide-age frames per scenario, x 5 columns) bit-identical to `assessment::plant_boundary_totals` (total = pandas row sum of 4 columns, ingestion = pandas sum of 3), tolerance 0; the text group is kept, unchanged, as the record of the printed route |
| P3 | The committed fixture is environment-dependent: numpy's `exp` and pandas' xlsx read are not the same across the versions above. The port will therefore NOT be bit-exact against a fixture regenerated here for the exp-dependent groups | observed in 1.; to be quantified by running the port against the regenerated fixture and counting, per group, values that differ and their worst relative difference |

Any observed number outside these is reported as-is with its localisation. None of the
bounds above will be edited after the run.
