# V&V: collision tracks and MCPL particle files (gh:#1392)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI draft,
> unreviewed.

## What was ported

| upstream | port |
|---|---|
| OpenMC `src/collision_track.cpp`: `h5_collision_track_banktype`, `write_h5_collision_track` | `njoy_outram_park_fork::hdf5::collision_track` (`collision_track_datatype`, `encode_collision_track_h5`, `write_collision_track_h5`) |
| OpenMC `openmc/source.py`: `read_collision_track_hdf5` | `decode_collision_track_h5` / `read_collision_track_h5` (offsets taken from the file and checked against upstream's struct) |
| OpenMC `src/collision_track.cpp`: `should_record_event`, `collision_track_record`, `collision_track_flush_bank`; `settings.h` `CollisionTrackConfig`; `reaction.cpp` `reaction_mt` | `outram_mc_libs::physics::collision_track` |
| MCPL `mcpl.c` (mcpl-core 2.2.8, Apache-2.0): header, particle (de)serialisation, adaptive-projection and octahedral direction packing, `stat:sum`, unclosed-file recovery, `.gz` input | `outram_mc_libs::source::mcpl` (`decode_mcpl`, `encode_mcpl`, `gunzip`, `gzip`) |
| OpenMC `src/mcpl_interface.cpp`: `mcpl_particle_to_site`, `mcpl_source_sites`, `write_mcpl_source_point`, `write_mcpl_collision_track` | same names in `outram_mc_libs::source::mcpl` |

Pure Rust: no `libhdf5`, no `libmcpl`, no `dlopen`. Upstream loads
`libmcpl` at run time; the port carries the format itself, so it builds for
`wasm32-unknown-unknown` and Android.

**Wiring.** `ExtraScoring::with_collision_track(CollisionTrackConfig)` attaches
a `CollisionTrackBank`; `CsgPowerIteration::step_scored` and `step_par` pass it
to the CSG history kernel, which builds one record per surface-tracked
collision **only when a bank is attached**. `ExtraScoring::flush` keeps an
active generation's records, `discard` drops an inactive one's (upstream's
`current_batch > n_inactive`). `CollisionTrackBank::flush_bank(last_batch)` is
`collision_track_flush_bank` (file names `collision_track.h5`,
`collision_track.<n>.h5`, `.mcpl`), and `write_collision_track_file` writes it
at the run boundary. The transport loop opens no file.

## Methodology

1. **Upstream's HDF5 fixtures** `collision_track/case_1_Reactions` and
   `case_7_all_parameters_used_together` (`collision_track_true.h5`, OpenMC
   `8f79d9d0`, MIT, written by the C HDF5 library; copied to
   `crates/njoy-outram-park-fork/tests/data/openmc_collision_track/`): decoded
   and checked against `h5dump`'s values; decode, encode, decode compared bit
   for bit (`hdf5::collision_track::tests`).
2. **OpenMC's own reader on the port's file.** `tests/collision_track_output.rs`
   runs a two-cell model (HEU sphere, cell 22, material 1, inside a water
   shell, cell 33, material 11, universe 77; CORE-tier U-235, U-238, H-1, O-16;
   300 histories, 2 inactive + 4 active generations) with a 200-record bank
   and writes `collision_track.h5`, `collision_track.mcpl` and a CSV of the
   port's records (shortest round-trip `f64` text).
   [`openmc_inputs/read_back.py`](openmc_inputs/read_back.py) reads them with
   `openmc.read_collision_track_hdf5` and `openmc.read_collision_track_mcpl`
   (OpenMC `8f79d9d0` from the upstream clone, `pip install --no-deps -e`,
   Python 3.13, numpy, h5py, mcpl-python 2.2.8). **No OpenMC binary is run**,
   so there is no OpenMC deck; the script is the input.
3. **Upstream's MCPL fixture**
   `tests/regression_tests/surface_source/surface_source_true.mcpl` (1000
   neutrons, single precision, `stat:sum:openmc_np1 = 10000`, written by
   `libmcpl` from OpenMC 0.15.3-dev149; copied to `tests/data/openmc_mcpl/`):
   decoded, re-encoded, compared byte for byte; gzipped and decoded again;
   taken through OpenMC's conversions (`mcpl_source_sites`, eV/s, then
   `write_mcpl_source_point`, MeV/ms) and compared byte for byte; the port's
   files read by the `mcpl` Python package.
4. **Off by default, no random number drawn.** The same model through
   `step` (no extras), `step_scored` without and with recording, and
   `step_par` without and with recording, in `SeedingMode::Outram` and
   `SeedingMode::OpenMc`. Pass criterion: every generation's `k`, `k_mean`
   and bank size bit-identical. Plus the existing pins
   `stats_move_fingerprints` and `delta_tracking_bit_identity`.

Pass criteria: bit identity for 1, 2 (values), 3 and 4; the HDF5 dtype equal
to upstream's fixture's; MCPL `E` and `time` within 2 f32 ulps (see below).

Prediction, posted on #1392 before the first run: all of the above exact,
the only expected difference the `version` attribute's width; the
same-history differential against OpenMC not feasible in this change.

## Results (2026-11-12, rustc 1.99.1)

| check | result |
|---|---|
| upstream fixtures decode to `h5dump`'s values (case 1: MT 18 on U-238 = PDG 1000922380, cell 22, universe 77, parent 13, progeny 8) | pass |
| decode, encode, decode, both fixtures | bit-identical |
| `openmc.read_collision_track_hdf5` on the port's file | 200 records; **dtype identical** to upstream's fixture (128 bytes, nested `r`/`u`); **4000 of 4000 values bit-identical** to the port's records; MTs {2: 169, 18: 15, 91: 14, 101: 2} |
| `version` attribute | `[1, 2]` as **int64** (upstream int32; `hdf5-pure` 0.20.1 has no i32 array attribute; OpenMC's reader does not read it) |
| `openmc.read_collision_track_mcpl` on the port's `.mcpl` | 200 records; ids, MTs, counts equal; positions equal at f32; `E` worst 5.5e-8, `time` worst 1.24e-7 relative (f32 storage, then the reader's own f32 product under NumPy 2 promotion: two f32 roundings); `dE` worst 4.2e-6 (upstream's blob prints 6 figures) |
| `surface_source_true.mcpl`, decode then encode | **0 of 36 120 bytes differ** |
| through `mcpl_source_sites` then `write_mcpl_source_point` (srcname set to the fixture's) | **0 bytes differ** |
| gzip of the re-encoding | `mcpl` reads it identical to the fixture; inflates to the fixture's bytes |
| `mcpl` 2.2.8 on the port's plain, gzipped and double-precision files | identical to the fixture's particles; `stat:sum` 10000 |
| k per generation, recording on vs off (`step`, `step_scored`, `step_par`; Outram and OpenMc seeding) | **bit-identical**; 6551 (Outram) and 7025 (OpenMc) records over 4 active generations |
| `step` vs `step_par` under OpenMc seeding | bit-identical k and **identical records** (parallel chunks merge in history order) |
| `stats_move_fingerprints` (9), `delta_tracking_bit_identity` (3) | pass, unchanged |

Every prediction held. No difference needs explaining beyond the two named
roundings (MCPL's f32 and upstream's 6-figure `dE` blob), which are upstream's
format, not the port's.

## Where a record differs from OpenMC's, and why

This crate's kernel samples reactions by group, not OpenMC's MT-by-MT walk, so
a record cannot be compared with OpenMC's history by history. Field by field
(also in `physics::collision_track`'s module doc):

- `event_mt`: 18 fission, 101 capture, 2 elastic (incl. S(α,β)), the level's
  MT for a discrete level, 91 continuum, 16/17/5, the channel's MT otherwise.
  OpenMC writes the sampled fission reaction's MT (18 on these evaluations).
- `n_collision`: since the history began; OpenMC gives a secondary its
  parent's count at banking. Same for histories without (n,xn) or splits.
- `progeny_id`: fission sites + secondaries + split daughters so far.
- `parent_id`: 1-based history index in the generation (OpenMC's id on one
  rank).
- Not recorded: collisions in a delta-tracked region (extras are not scored
  there), and a collision rouletted by survival biasing before its reaction
  is drawn.

## Not done

- **Same-history differential against OpenMC's collision-track output: not
  run.** Blocked as recorded on #1386: OpenMC is not built in this container
  and there is no shared nuclear-data library (no ACE-to-HDF5 path for the
  port's nuclides), and the CSG kernel's reaction sampling is not OpenMC's
  draw for draw even under `SeedingMode::OpenMc`.
- No XML `<collision_track>` reader (the crate reads no XML); the config is
  built in Rust, with `with_reactions` doing `settings.cpp`'s name lookup.
- Surface-source writing as MCPL is available as a function
  (`write_mcpl_source_point`, from `ParticleSite`s a caller builds from its
  `SurfaceSource` crossings) but no driver
  writes it automatically.
- Gzip *output* is offered (`gzip`) but OpenMC writes plain MCPL, so no
  writer path uses it.

## Gates run (2026-11-12)

`CARGO_TARGET_DIR` set to a scratch directory, one cargo process at a time:

| command | result |
|---|---|
| `cargo test --release -p njoy-outram-park-fork --lib` (under the 12 GB `ulimit -v`) | 723 passed, 0 failed |
| `cargo test --release -p outram-mc-libs --lib` | 528 passed, 1 failed, 4 ignored. The failure, `vv::msre_n1::tests::plenums_and_openings_match_their_sources`, asserts that `reference-data/ace/endf/.../n-028_Ni_058-ENDF8.0.endf` exists; that submodule is not checked out in this worktree. Unrelated to this change (it reads no code touched here). |
| `cargo test --release -p outram-mc-libs --test collision_track_output` | 4 passed |
| `cargo test --release -p outram-mc-libs --test stats_move_fingerprints` | 9 passed (pins unchanged) |
| `cargo test --release -p outram-mc-libs --test delta_tracking_bit_identity` | 3 passed (pins unchanged) |
| `cargo check --release -p outram-mc-libs --lib --target wasm32-unknown-unknown` | builds, no new warnings |
| `OUTRAM_COLLISION_TRACK_OUT=<dir> cargo test ... --test collision_track_output`, then `python openmc_inputs/read_back.py <dir> <OpenMC clone>` | ALL CHECKS PASSED |
