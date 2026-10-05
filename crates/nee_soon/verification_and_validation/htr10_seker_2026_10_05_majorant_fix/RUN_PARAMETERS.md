# HTR-10 k vs height on the bounded majorant: run parameters (2026-10-05)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Single seed per point, reduced
> statistics. Research, education and V&V only; not for any operational use.
> AI-assisted run (Claude Opus 5.5 in Claude Code), not yet reviewed by the
> maintainer.

This file lists everything needed to rebuild these runs. It is a
re-measurement of a subset of
[`../htr10_seker_2026_10_01_10k/`](../htr10_seker_2026_10_01_10k/) after the
delta-tracking majorant fix of GitHub #589 (`b208cecc7`). Only the items
below that are **marked "differs"** change from that record's
`RUN_PARAMETERS.md`. Results are in `README.md` and `results_table.md`.

## 1. Code and data versions

| Item | Value |
|---|---|
| Repository | `github.com/theodoreOnzGit/outram-park-backend` |
| Commit the runs were built from | `develop` at `e09030a3cf` (contains the majorant fix `b208cecc7`) (**differs**: the record used `f888cfd98e`) |
| What changed in the code between the two | `Majorant::over_indices` (called by `nee_soon::htr10_rmc::keff_vs_height::bed_majorant`) now adds every nuclide breakpoint to the 4096-point log grid, reads one ulp either side of each node, and is floored at the old values. The old construction under-bounded `Sigma_t` by 14.04x at 661.2 eV in the UO2 kernel (VIII.0) and 13.79x (VII.0); `tests/majorant_bounds_endf.rs` pins the new one at 0.7693 on that material set. Other commits between `f888cfd98e` and `e09030a3cf` were not audited for effect on this case (see README) |
| Submodule `reference-data/ace` | `6440b6dfe07a5f861101df16424e75aa47d34bce` (same as the record) |
| Rust toolchain | `rustc 1.94.1 (e408947bf 2026-03-25)`, `cargo 1.94.1` (**differs**: the record used 1.98.0) |
| Build profile | `--release` |
| Example | `crates/nee_soon/examples/htr10_rmc_keff.rs` (same) |
| Hardware | Intel Xeon Processor @ 2.80 GHz, 4 cores, 15.7 GiB RAM, Linux 6.18.44, CPU only (**differs**) |
| Plot/table script | `plot_keff_vs_height.py` (Python 3.11.15, matplotlib 3.11.2), adapted from the record's |

## 2. Reproduce

```bash
git checkout e09030a3cf
git submodule update --init reference-data/ace
cargo build --release -p nee_soon --example htr10_rmc_keff

cd crates/nee_soon/verification_and_validation/htr10_seker_2026_10_05_majorant_fix
./run_all.sh                      # sequential, all five runs, resumable
python3 plot_keff_vs_height.py logs .
```

`run_all.sh` is the exact launcher: VIII.0 N = 14, 10, 20, 17, then VII.0
N = 14, one run at a time on cores 0-3 (`taskset -c 0-3`), each with the
environment in section 3. It also samples the process's peak RSS (VmHWM)
every 20 s and appends it to the log.

**Pilot (not a result):** `logs/pilot/pilot_e8_N14_2000x10.log`, VIII.0
N = 14 at 2000 × [2 + 8]: data 269.9 s, transport 146.9 s for 20 000
histories (7.35 ms wall per history on 4 threads). At that rate the record's
full 1.4 M histories cost about 2.9 h per point, which is why the active
cycles were cut (section 4).

## 3. Environment variables (complete list used)

| Variable | Value | Meaning |
|---|---|---|
| `OUTRAM_HTR10_HISTORIES` | `10000` | particles per cycle (same) |
| `OUTRAM_HTR10_INACTIVE` | `5` | discarded cycles (same) |
| `OUTRAM_HTR10_ACTIVE` | `20` | active cycles (**differs**: 135 in the record) |
| `OUTRAM_HTR10_RINGS` | `14` | full-radius bed (same) |
| `OUTRAM_HTR10_LAYERS` | `10`, `14`, `17`, `20` | Şeker layers N (a subset of the record's 10 to 20) |
| `OUTRAM_HTR10_THREADS` | `4` | pinned thread count (**differs**: 5) |
| `OUTRAM_HTR10_ENDF7` | set for the VII.0 run only | library switch (same) |
| `OUTRAM_HTR10_SEED` | unset → default `20260917` | RNG seed (same) |

Every other `OUTRAM_HTR10_*` knob was unset, as in the record.

## 4. Transport settings

| Setting | Value |
|---|---|
| Particles per cycle | 10 000 (same) |
| Cycles | 25 = 5 inactive + 20 active (**differs**: 140 = 5 + 135) |
| Histories | 250 000 simulated, 200 000 active (**differs**: 1.4 M / 1.35 M) |
| Expected σ | about 105 pcm × √(135/20) ≈ 270 pcm per point |
| Seed | 20260917 (same; the streams diverge from the record's anyway, since the majorant changes every flight) |
| Everything else | as in the record: 300.15 K, hybrid tracking (delta in the bed), within-run 1σ |

Geometry, materials, nuclear data and references: unchanged from the
record's sections 5 to 7.
