# HTR-10 k_eff on the DEM random bed: run parameters (2026-10-08)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** One seed, one pour. Research, education
> and V&V only; not for any operational use. AI-assisted run (Claude Opus 5.5
> in Claude Code), not yet reviewed by the maintainer.

The items **marked "differs"** change from the lattice record,
[`../htr10_seker_2026_10_07_10k/RUN_PARAMETERS.md`](../htr10_seker_2026_10_07_10k/RUN_PARAMETERS.md).
That record's sections 5 to 7 (materials, nuclear data, references) still
apply. The one new item is the bed.

## 1. Code and data versions

| Item | Value |
|---|---|
| Commit the run was built from | `713cd5bb60adbdc99dadbb2405bc3aa73a5eff8f` (`logs/commit.txt`), a worktree branch off `develop` at `ac0adb5084`. **Differs**: the record used `dbb9e26e`. The only transport-side change between them is `7ee55f283a` (#784 tracking events), which draws no random number and is pinned bit-identical. See the rebase note below the table. |
| Submodule `reference-data/ace` | `6440b6dfe07a5f861101df16424e75aa47d34bce` (same) |
| DEM bed | `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv`, sha256 `43684afaf4247b04261f85ed5bf8b69e831b3d6573e628fbaa5c061daff1e553`. The gh:#216 `GranularSystem` bed at µ = 0.1, µ_r = 0. Provenance: `crates/dhoby-ghaut/verification_and_validation/htr10_dem_bed_bake/README.md`. **New.** |
| Rust toolchain | `rustc 1.98.0 (88d9e12ae 2026-08-18)`, `cargo 1.98.0`. **Differs**: the record used 1.98.1. |
| Build profile | `--release`, `-j 5` |
| Example | `crates/nee_soon/examples/htr10_rmc_keff.rs` (same), with the new `OUTRAM_HTR10_DEM_BED` knob |
| Hardware | Intel Core i9-13900K, 16 logical CPUs visible, 62.5 GiB RAM, Linux 7.2.7-arch1-1, CPU only. Shared with other agents' jobs; load average 4–16 during the run. **Differs**: the record ran on a Xeon W-2195. |

**Rebase note.** After the run, the branch was rebased onto `develop` at `d74cc652e7`, so the commit the run was built from is no longer on the branch; the run's code changes are now commit `eed51a2f9c` (the driver knob, the reader and the cut; rebased from `b10afbbb12`). The rebase also brought in #786's library changes:
- `Nuclide::from_tape` split into `process_evaluation` + `from_processed`;
- a new distributed power iteration, which this driver does not call.

#786 pins both as bit for bit the old routes (`tests/processed_evaluation_round_trip.rs`; the distributed module's own tests). **That this run's k is reproduced bit for bit on the rebased commit has not been checked** (HTR-10 transport was not to be re-run). To rebuild the exact binary, check out `ac0adb5084` and cherry-pick `eed51a2f9c`.

## 2. Reproduce

```bash
git checkout ac0adb5084 && git cherry-pick eed51a2f9c   # the run's 713cd5bb60 (see section 1)
git submodule update --init reference-data/ace
cargo build --release -j 5 -p nee_soon --example htr10_rmc_keff
cargo test  --release -j 5 -p nee_soon --lib dem_   # the reader, the cut, the beds-view identity
crates/nee_soon/verification_and_validation/htr10_dem_bed_keff_2026_10_08/run.sh
```

`run.sh` is the exact launch. It runs one process with 5 threads, not
pinned, started from the repository root, and writes
`logs/run_dem_e8_N12.log`, `logs/start_utc.txt` and `logs/end_utc.txt`.

## 3. Environment variables (complete list used)

| Variable | Value | Meaning |
|---|---|---|
| `OUTRAM_HTR10_DEM_BED` | `reference-data/liggghts/htr10_conus_presettled_mu10_mur00.csv` | **new**: build the core on this bed |
| `OUTRAM_HTR10_DEM_CORE_BALLS` | unset → the lattice's count at these layers, 16 681 | **new**: the cut |
| `OUTRAM_HTR10_HISTORIES` | `10000` | particles per cycle (same) |
| `OUTRAM_HTR10_INACTIVE` | `5` | discarded cycles (same) |
| `OUTRAM_HTR10_ACTIVE` | `135` | active cycles (same) |
| `OUTRAM_HTR10_RINGS` | `14` | full-radius bed (same) |
| `OUTRAM_HTR10_LAYERS` | `12` | the lattice matched; on the DEM path it sets only the ball count |
| `OUTRAM_HTR10_THREADS` | `5` | **differs**: the record used 7 |
| `OUTRAM_HTR10_SEED` | unset → `20260917` | same |

Every other `OUTRAM_HTR10_*` knob was unset. Every ablation is therefore off:
URR, DBRC and S(α,β) are on as defaults, graphite is 30P, carbon is natural,
the coolant is helium, the rods are explicit and withdrawn, and the majorant
is the bounded one of #589. `OUTRAM_HTR10_ENDF7` was unset, so this is the
ENDF/B-VIII.0 arm.

## 4. The bed as built

These figures come from the run log and from
`explicit_bed::tests::the_dem_bed_cut_to_the_lattice_ball_count_is_the_beds_view_cut`.

| Item | Value |
|---|---|
| balls kept | 19 138 of 27 554: every one at or below the floor, plus the lowest 16 681 above it |
| core balls / fuel | 16 681 / 9 508 (`paper_fuel_assignment`, 57:43) |
| conus / tube | 2 219 DEM balls in the conus; 238 DEM balls plus 1 721 Şeker filler balls in the tube, with a 0.620 cm gap at the junction |
| bed top (highest ball) / p99 surface | 125.085 cm / 123.692 cm above the floor |
| lens pairs / volume fraction | 51 781 / 1.272e-5 |
| tiles / cells / universes | 22 427 / 88 090 / 2 285 |
| reference height (equal ball count) | 122.091 cm, where RMC reads 0.999419 |

## 5. Transport settings

| Setting | Value |
|---|---|
| Cycles | 140 = 5 inactive + 135 active (same) |
| Histories | 1 400 000 simulated, 1 350 000 active (same) |
| Seed | 20260917 (same seed; the streams differ from the lattice's, since the geometry differs) |
| Everything else | 300.15 K, hybrid tracking (delta tracking in the bed), within-run 1σ (same) |
