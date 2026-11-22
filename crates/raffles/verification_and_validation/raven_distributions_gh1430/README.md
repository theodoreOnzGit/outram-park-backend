# raffles vs RAVEN: distributions, BasicStatistics, Grid (gh:#1430)

Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-assisted draft
material; no human or crate-owner review. Verification only: it asks whether the
port reproduces its upstream, not whether any model is physically right.

| file | what |
|---|---|
| `PREDICTIONS.md` | predictions, committed before each run |
| `RESULTS.md` | measured results, scorecard, first divergent operation per mismatch |
| `generate_references.py` | RAVEN `Distributions1D` on fixed grids -> `references/distributions.json` |
| `generate_statistics_reference.py` | RAVEN `BasicStatistics._runLocal` on a fixed dataset -> `references/statistics.json` |
| `generate_grid_reference.py` | RAVEN `GridEntity` levels -> `references/grid.json` |
| `ravenimport.py` | standalone import of RAVEN modules (copy of the gh:#1193 one) |
| `../../tests/raven_{distributions,statistics,grid}_gh1430.rs` | the comparisons, with count/bound pins |

## Reproduce

```bash
python3.11 -m venv V && V/bin/pip install numpy==1.26.4 scipy==1.14.1 scikit-learn==1.3.2 xarray==2024.6.0 pandas==2.2.2
git clone --depth 1 --filter=blob:none --sparse https://github.com/idaholab/raven.git R
(cd R && git sparse-checkout set ravenframework)
for g in generate_references generate_statistics_reference generate_grid_reference; do
  RAVEN_DIR=R V/bin/python -I crates/raffles/verification_and_validation/raven_distributions_gh1430/$g.py
done
cargo test --release -p raffles --test raven_distributions_gh1430 --test raven_statistics_gh1430 --test raven_grid_gh1430 -- --nocapture
```

References were generated against RAVEN `devel` @
`dcb75beaae888500402aa7edd0ba0d5d0f64ad05` (2026-11-11), accessed 2026-11-13;
the crate's port reference commit is `0121693796` (2026-07-14) and the
gh:#1193 routes used `8baaae3a97`. RAVEN is Apache-2.0; only its outputs are
committed, not its code. Nothing was run from the compiled `crow` layer
(retired from RAVEN's default path).

## Headline

* **Distributions**: no port defect. Closed forms agree to a few ulp
  (Uniform bit-identical); Normal/LogNormal/Gamma/Beta differ because scipy and
  raffles use different special-function code (worst 5.7e-15 pdf, 3.1e-15
  absolute cdf, ppf within 26 units of 2^-53 in probability). **RAVEN has a
  defect**: `LogNormal.std()` adds `low`.
* **BasicStatistics**: mean, variance, sigma, covariance, Pearson, Spearman
  agree to <= 5.4e-15; the whole difference is numpy's pairwise summation
  (reproducing it gives 3/3 bit-identical). Skewness, kurtosis, median,
  percentile, min and max do not exist in raffles (gap).
* **Grid**: RAVEN's `equal` levels are not `linspace`'s (14-decimal rounded
  step plus `arange`); the doc claim saying so is corrected, and
  `equally_spaced_as_raven` reproduces all 1220 levels bit for bit. RAVEN
  rejects two of four `custom` grids that raffles accepts.
