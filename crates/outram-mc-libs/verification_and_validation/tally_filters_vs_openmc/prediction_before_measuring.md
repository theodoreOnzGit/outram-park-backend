# gh:#261 bin-for-bin tally-filter comparison against OpenMC 0.15.3 -- PREDICTIONS

Written 2026-11-18, **before any comparison was run** (committed first, per the
workspace rule "predictions written before runs").

## What was established from the code before predicting (not from a run)

1. All 16 filters of #261's list have a `FilterKind` variant (`tally/filter.rs:1007`).
   None had an OpenMC cross-code comparison (the existing evidence is hand-computed
   answers and upstream formulas read from source: `tests/filter_extra_vs_upstream.rs`).
2. **Transport wiring.** `FilterEvent` fields `cell_from`, `cell_born`,
   `material_from`, `surface_mu`, `position_born`, `n_collision`, `event_mt`,
   `parent_nuclide`, `secondaries`, and (for the track-length path) `weight`, are set
   by **no code outside `tally/filter*.rs`**: `scoring.rs` builds its `FilterEvent`
   with `..Default::default()`. Only `cell_instance`, `time`, `direction`, `position`,
   `energy` are threaded through `score_track_length`. So a CellFrom / CellBorn /
   MaterialFrom / MuSurface / MeshBorn / MeshSurface / Collision / Weight tally put on
   a real run scores nothing (or bins everything at the default). The reviewed
   transport function `transport_history_vr` is NOT edited; the history-event driver
   that supplies these fields is new code beside it (`tally::history_events`), fed by
   the existing `TraceEvent` observer.
3. **Upstream 0.15.3 (commit 27e38e89) has no `filter_reaction.cpp` and no
   `filter_particle_production.cpp`** (`ls src/tallies/`); REACTION and
   PARTICLE_PRODUCTION cannot be compared with this OpenMC. `ParentNuclideFilter`
   exists but matches only a decay-chain parent index (`filter_parent_nuclide.cpp:42`),
   set only by a decay-photon source.

## Experiment A -- deterministic beams (exact comparison)

Dilute-O16 / void geometry, single mono-directional, mono-energetic neutron per run,
tracks identical in both codes (no collision is sampled at density 1e-10 atoms/b-cm;
a collision would show as a mismatch and is counted). Compared per site, per bin.
Pass criterion: relative difference <= 1e-9 in every bin (round-off only),
absolute 1e-12 where the value is 0.

| filter | prediction |
|---|---|
| DISTRIBCELL, CELL_INSTANCE | agreement in every bin, including the instance numbering (row-major lattice order); unit tests only checked 3 shapes by hand |
| ENERGY_FUNCTION | agreement 1e-9 provided both interpolate linear-linear (OpenMC default) |
| CELLBORN, MATERIALFROM, MESHBORN | agreement, once fed by the new driver |
| CELLFROM | agreement on leaf-cell bins. **Predicted divergence:** OpenMC loops over *all* coordinate levels of the last state (`filter_cellfrom.cpp:11`), so a bin naming the lattice-filled parent cell matches in OpenMC; ours holds one cell index |
| MUSURFACE | agreement of bin assignment for oblique crossings; the sign convention (`p.surface() < 0` flips the normal, `filter_musurface.cpp:19`) is the likely place for a mismatch |
| MESH_SURFACE | agreement of the set of crossed faces, including diagonal beams crossing several faces |
| **MESH_MATERIAL** | **predicted MISMATCH (a miss).** `MeshMaterialFilter::get_bin` bins the segment midpoint only, with no track-length splitting, so any segment that crosses a mesh-element edge inside one material is credited to one element; OpenMC splits it. Exact on segments that stay in one element |

## Experiment B -- statistical, CE O-16 gas, fixed source

Pass: |z| <= 2 per bin with z = (a-b)/sqrt(sa^2+sb^2); report bin count and worst z.
Not tuned; misses are reported.

| filter | prediction |
|---|---|
| COLLISION | agree within |z|<=2 once `n_collision` is fed by the driver |
| WEIGHT | with survival biasing both codes carry weight `(1-Sa/St)^k`; agreement within |z|<=2 |

## Not comparable here (stated before running)

REACTION, PARTICLE_PRODUCTION (absent in OpenMC 0.15.3), PARENT_NUCLIDE (needs a depletion chain and a decay source).
LEGENDRE: attempted only if the scatter-event driver is finished; otherwise reported as not done.
