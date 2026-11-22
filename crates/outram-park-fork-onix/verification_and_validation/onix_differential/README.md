# ONIX 7328dc6 differential (gh:#1404, epic #1336)

Compares `outram-park-fork-onix` against a **run of the upstream Python**, not against a
reading of it. `PREDICTIONS.md` was committed before the first comparison; `RESULTS.md`
holds the numbers, the defects, and the upstream-gap survey.

## Upstream and how it was run

- ONIX, `https://github.com/jlanversin/ONIX`, commit
  `7328dc6acf705827470819a4367ef2a98c642b74` (MIT), cloned over git into an empty scratch
  directory, never modified.
- Python 3.11 venv; only `numpy 2.4.6 matplotlib networkx uncertainties scipy pandas`
  installed; run as `python -I onix_driver.py --onix <clone> --out <dir>` from outside the
  clone.
- `onix_driver.py` (this directory) imports the upstream modules unchanged. Driver-side
  accommodations, all listed in its docstring: `openmc` replaced by a `MagicMock` (the
  package `__init__` imports it at module level); `CRAM16` run in two forms, because the
  committed function does not run on this numpy (below); a pure-Python replica of this
  crate's pole solve, to localise differences.

## Cases

| # | case | upstream code that produced the reference | reference file |
|---|---|---|---|
| 1 | `CRAM16` on 4 fixed matrices (scalar, 3-nuclide chain, 20-nuclide branching FP chain with a capture loop, 8-nuclide stiff chain with lambda 1e-12..1e4 /s) at dt = 1 s, 1 d, 30 d | `salameche/cram.py` `CRAM16` | `cram_cases.csv` |
| 2 | burnup matrix `A = B*1e-24*flux + C` for a 36-nuclide synthetic passport set with decay branching, isomers, `(n,gamma)X`, `(n,2n)X`, `betanegX`, `Xgamma` and fission yields | `salameche/mat_builder.py` `get_xs_mat`, `get_decay_mat` on real `Passlist`/`Passport` objects | `case2_inputs.csv`, `case2_matrix.csv` |
| 3 | 10 steps of 30 d at constant flux 3e14 | the arithmetic of `burn.py:215-229` `burn_microstep` (`A`, `At = A*dt`, `CRAM16`, `CRAM_density_check`) with the upstream functions; `burn_microstep` itself needs a `Cell` and `Sequence` | `case3_burn.csv` |
| 4 (one-off, not committed) | case 2 on ONIX's own bundled default libraries over `reduced_nucl_set` | same as 2 | `onix_driver.py --real`, then the ignored test |

The inputs of cases 2 and 3 are **synthetic**: numbers invented for this test, of
plausible magnitude, over real nuclide names. They are not ONIX data and not evaluated
nuclear data. The crate bundles no nuclear data (`AGENTS.md`), so case 4 is run from the
upstream clone's libraries and only its summary counts are recorded.

## Data provenance (case 4 only)

`onix/data/default_libs/decay_lib_reduced`, `xs_lib`, `fy_lib_reduced` in the ONIX
repository (MIT). ONIX's own note says the decay and yield libraries are ENDF/B-VIII
derived and `xs_lib` is "a constant one-group cross section library taken from VVER
simulation output with OpenBU coupled version". Accessed 2026-11-13 from the commit
above. Used in memory by the one-off run; nothing from them is committed.

## Reproduce

```bash
python3 -m venv venv && venv/bin/pip install numpy matplotlib networkx uncertainties scipy pandas
git clone https://github.com/jlanversin/ONIX.git onix && git -C onix checkout 7328dc6acf705827470819a4367ef2a98c642b74
venv/bin/python -I onix_driver.py --onix onix --out out          # rewrites the four CSVs
cargo test --release -p outram-park-fork-onix --test onix_differential
# optional one-off on ONIX's libraries:
venv/bin/python -I onix_driver.py --onix onix --out outreal --real
ONIX_REAL_DIR=outreal cargo test --release -p outram-park-fork-onix \
    --test onix_differential real_ -- --ignored --nocapture
```

Pass `ONIX_DIFF_PRINT=1 -- --nocapture` for the per-case tables.
