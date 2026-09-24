# OpenMC inputs for the WMP-write cross-code check

Reference side of the code-to-code comparison for this workspace's
`WMP_Library` HDF5 **writer** (gh:#270). Committed, not merely cited, per the
workspace rule that a reader must be able to reproduce the reference.

## Provenance

* **Project** — OpenMC, <https://github.com/openmc-dev/openmc>
* **Author / organisation** — the OpenMC development team (openmc-dev)
* **Version** — `0.1.dev1+gafa7a14ac`, i.e. commit **`afa7a14`**, from the local
  checkout at `/opt/src/openmc`
* **Date accessed** — 2026-09-24
* **Licence** — MIT; upstream notice in `LICENSE.openmc`. Compatible with this
  workspace's GPL-3.0-only.

## Contents

| File | What it is |
|---|---|
| `read_wmp.py` | the driver: reads a `.h5` with `openmc.data.WindowedMultipole.from_hdf5`, prints its structure and evaluated cross sections (new work, GPL-3.0, this workspace) |
| `LICENSE.openmc` | upstream MIT notice |

**There is no committed input file here, deliberately.** The thing under test is
the *writer*, so the `.h5` must be produced fresh by the code as it stands —
committing one would test a snapshot instead. `tests/wmp_h5_vs_openmc.rs` writes
it to a per-process scratch path, runs this script on it, and deletes it.

## Reproducing

```bash
# write a library from the embedded CORE set, then read it with OpenMC
cargo test --release -p njoy-outram-park-fork --test wmp_h5_vs_openmc -- --nocapture

# or by hand, on any WMP_Library-format file
PYTHONPATH=/opt/src/openmc /opt/ompy/bin/python \
    read_wmp.py U238.h5 U238 293.6,600.0,1200.0 1.0,6.67,20.9,100.0,1000.0
```

No cross-section library is needed: `from_hdf5` plus scipy's Faddeeva is the
whole dependency, which is why this cross-code check runs where a
continuous-energy one cannot.

## Data licence note

The WMP data written into the test file is **MIT CRPG** (ENDF/B-VII.1), embedded
in this crate as `src/data/wmp_core.wmpl`. See the crate's `LICENSE-WMP` and
`NOTICE` — both must travel with any file written from it.

Results: `../wmp_h5_write_2026_09_24.md`.
