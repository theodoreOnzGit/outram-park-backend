# HTR-10 k vs height, 10 000 × [5 + 135] on the bounded majorant: run parameters (2026-10-07)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** One seed per point. Research, education
> and V&V only; not for any operational use. AI-assisted run (Claude Opus 5.5
> in Claude Code), not yet reviewed by the maintainer.

Only the items **marked "differs"** change from
[`../htr10_seker_2026_10_01_10k/RUN_PARAMETERS.md`](../htr10_seker_2026_10_01_10k/RUN_PARAMETERS.md);
geometry, materials, nuclear data and references (its sections 5 to 7) are
unchanged.

## 1. Code and data versions

| Item | Value |
|---|---|
| Commit the runs were built from | `develop` at `dbb9e26eb10a665735d179359af985d5fcdbd4fa` (`logs/commit.txt`) (**differs**: `f888cfd98e`; contains the #589 majorant fix `b208cecc7`) |
| Submodule `reference-data/ace` | `6440b6dfe07a5f861101df16424e75aa47d34bce` (same) |
| Rust toolchain | `rustc 1.98.1 (48a229cea 2026-09-01)`, `cargo 1.98.1` (**differs**: 1.98.0) |
| Build profile | `--release` |
| Example | `crates/nee_soon/examples/htr10_rmc_keff.rs` (same) |
| Hardware | Intel Xeon W-2195 @ 2.30 GHz, 18 cores / 36 threads, 62.5 GiB RAM, Linux 5.15.0-190-generic, CPU only (**differs**) |
| Plot/table script | `plot_keff_vs_height.py` (Python 3.10.13, matplotlib 3.8.1), copied from the 2026-10-05 record with labels changed only |

## 2. Reproduce

```bash
git checkout dbb9e26e
git submodule update --init reference-data/ace
cargo build --release -p nee_soon --example htr10_rmc_keff
cargo test  --release -p nee_soon --test htr10_geometry_integrity   # 4 passed

cd crates/nee_soon/verification_and_validation/htr10_seker_2026_10_07_10k
./run_all.sh                       # all 22 runs, resumable
python3 plot_keff_vs_height.py logs .
```

`run_all.sh` calls the shared launcher `../htr10_run_all.sh` at its defaults:
N = 10, 14, 17, 20, 12, 16, 19, 11, 13, 15, 18 in that order, VIII.0 and VII.0
interleaved, **five runs at a time, each pinned to its own 7 logical CPUs**
(`taskset`, CPUs 0-34; one left free). It samples each run's peak RSS (VmHWM)
every 20 s and appends it and the CPU set to the log. Peak RSS was at most
0.68 GB.

**One change to the launcher after these runs** (behaviour of the binary
unchanged): these runs were started from inside this folder, so the binary
wrote its per-run performance reports to `./verification_and_validation/local_perf/`
here, outside the root `.gitignore` entry. They were moved to the
(gitignored) root `verification_and_validation/local_perf/`, and the launcher
now starts each run from the repository root.

Run times: nuclear data 108 to 336 s, transport 4 267 to 8 984 s per run
(the two N = 18 runs were fastest, running after the pool had emptied).
Per-run numbers are in each log and in `results_table.csv`.

## 3. Environment variables (complete list used)

| Variable | Value | Meaning |
|---|---|---|
| `OUTRAM_HTR10_HISTORIES` | `10000` | particles per cycle (same) |
| `OUTRAM_HTR10_INACTIVE` | `5` | discarded cycles (same) |
| `OUTRAM_HTR10_ACTIVE` | `135` | active cycles (same) |
| `OUTRAM_HTR10_RINGS` | `14` | full-radius bed (same) |
| `OUTRAM_HTR10_LAYERS` | `10` to `20` | Şeker layers N (same) |
| `OUTRAM_HTR10_THREADS` | `7` | pinned thread count (**differs**: 5) |
| `OUTRAM_HTR10_ENDF7` | set for the VII.0 arm only | library switch (same) |
| `OUTRAM_HTR10_SEED` | unset → default `20260917` | RNG seed (same) |

Every other `OUTRAM_HTR10_*` knob was unset: every ablation off, including
`OUTRAM_HTR10_MAJORANT_WITHOUT_BREAKPOINTS`, so the bounded majorant (#589)
is in use. The default `Htr10CoreDesign` (rods withdrawn) applies.

## 4. Transport settings

| Setting | Value |
|---|---|
| Cycles | 140 = 5 inactive + 135 active (same) |
| Histories | 1 400 000 simulated, 1 350 000 active, per point (same) |
| Seed | 20260917 (same; streams differ from the record's since the majorant changes every flight) |
| Lost locates | 0 in every run |
| Everything else | 300.15 K, hybrid tracking (delta in the bed), within-run 1σ (same) |
