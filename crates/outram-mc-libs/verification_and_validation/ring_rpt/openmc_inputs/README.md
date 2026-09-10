# OpenMC input scripts — ring-RPT V&V reference

Verbatim snapshot of the maintainer's OpenMC decks that produce the reference
numbers for `../ring_rpt_vs_openmc.md`. Kept here (committed) so the reference
side of the code-to-code comparison is reproducible — per the workspace
`crates/outram-mc-libs/CLAUDE.md`, "Code-to-code verification: commit the OpenMC
input scripts".

| File | Role |
|---|---|
| `pipeline_triso_to_rpt.py` | driver — builds both pebbles, runs, six factors |
| `triso.py` | TRISO particle (5 layers) + FHR materials |
| `triso_pebble.py` | explicit-TRISO pebble assembly |
| `rpt_pebble.py` | ring-RPT homogenised-shell pebble + `get_mixed_triso_fuel_material` |
| `reactor_physics_preamble.py` | 3-group six-factor + lethargy-spectrum reader (mirrors `physics::reactor_physics`) |

**Provenance.** `github.com` ↔ GitLab `theodore_ong/openmc_fuel_perf_project`,
BSD-3-Clause © 2024 theodoreOnzGit (see `LICENSE`). Snapshotted 2026-09-10 from
that working tree at base commit `ff92278` with the three uncommitted
`pebble_factory/*.py` edits and the two untracked driver files as they stood
that day. Own work, GPL-3-compatible, notice carried.

**OpenMC / data.** OpenMC 0.15.3-dev (op-mzvp.1, commit `09ee8308d`),
ENDF/B-VIII.0 HDF5, 600 K, 20 000 particles × 150 batches (50 inactive).
