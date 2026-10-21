# OpenMC inputs for the geometry-plot cross-code check

Reference side for gh:#268 acceptance item 1. Committed, not cited.

## Provenance

* **Project** — OpenMC, <https://github.com/openmc-dev/openmc>
* **Version** — `0.1.dev1+gafa7a14ac` (commit `afa7a14`), local checkout `/opt/src/openmc`
* **Date accessed** — 2026-09-24
* **Licence** — MIT, notice in `LICENSE.openmc`; compatible with GPL-3.0-only
* **Solver** — `/opt/openmc/bin/openmc --plot`

## Contents

| File | What |
|---|---|
| `plot_godiva_slice.py` | driver: builds the Godiva sphere, runs `openmc --plot`, dumps a 0/1 cell mask (new work, GPL-3.0) |
| `openmc_godiva_mask.txt` | the generated 81 x 81 reference mask |
| `LICENSE.openmc` | upstream MIT notice |

## Why a mask rather than an image diff

OpenMC assigns cell colours arbitrarily, so comparing RGB compares a palette.
What both codes must agree on is **which points are in the model** — the geometry
question a plot exists to answer. White in OpenMC's PNG means "no cell here".

## Why this comparison is possible at all

`openmc --plot` needs **no cross-section library**; it only locates cells. No
nuclear data is installed on this machine, so every other cross-code check in
this crate had to build a data pipeline first. This one did not.

## Reproducing

```bash
PYTHONPATH=/opt/src/openmc /opt/ompy/bin/python plot_godiva_slice.py <out_dir>
cargo test --release -p outram-mc-libs --test geometry_plot -- --nocapture
```

Consumed by `tests/geometry_plot.rs::our_godiva_slice_matches_openmcs_own_rasteriser`.
Results: `../geometry_plot_2026_09_24.md`.
