# HTR-10 k vs height, ENDF/B-VIII.0 and VII.0, 10 000 × [5 + 135]: run parameters (2026-10-01)

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** Single seed per point. Research,
> education and V&V only; not for any operational use. AI-assisted run
> (Claude Opus 5.5 in Claude Code), not yet reviewed by the maintainer.

This file lists everything needed to rebuild both runs (VIII.0 arm and VII.0
arm) from scratch. Results are in `README.md` and `results_table.md`.

## 1. Code and data versions

| Item | Value |
|---|---|
| Repository | `github.com/theodoreOnzGit/outram-park-backend` |
| Branch / commit the runs were built from | `develop` at `f888cfd98e` |
| Submodule `reference-data/ace` | `6440b6dfe07a5f861101df16424e75aa47d34bce` (**must** be checked out; it carries the Ni-58/60/61/62/64 VIII.0 tapes) |
| Submodule `crates/kovan-literature/reactor-literature` | `686c94bfb296810de3df68c19930d9ee4a97a8db` (not used by the run) |
| Rust toolchain | `rustc 1.98.0 (88d9e12ae 2026-08-18)`, `cargo 1.98.0` |
| Build profile | `--release` |
| Example | `crates/nee_soon/examples/htr10_rmc_keff.rs` |
| Hardware | Intel i9-13900K, 16 cores available to the process, 62.5 GiB RAM, Linux 7.2.7-arch1-1; transport on CPU (the logs' hardware line also lists an RTX A5000 GPU: detected by host introspection, not used) |
| Plot/table script | `plot_keff_vs_height.py` (Python 3.14.7, matplotlib 3.11.2) |

## 2. Reproduce

```bash
git clone --recurse-submodules https://github.com/theodoreOnzGit/outram-park-backend
cd outram-park-backend
git checkout f888cfd98e
git submodule update --init reference-data/ace    # after any checkout or pull

cargo build --release -p nee_soon --example htr10_rmc_keff
cargo test  --release -p nee_soon --test htr10_geometry_integrity   # 4 passed expected

# One run. LIB = e8 (VIII.0) or e7 (VII.0); N = 10 ... 20.
OUTRAM_HTR10_HISTORIES=10000 \
OUTRAM_HTR10_INACTIVE=5 \
OUTRAM_HTR10_ACTIVE=135 \
OUTRAM_HTR10_RINGS=14 \
OUTRAM_HTR10_LAYERS=$N \
OUTRAM_HTR10_THREADS=5 \
[OUTRAM_HTR10_ENDF7=1]   # VII.0 arm only \
./target/release/examples/htr10_rmc_keff > logs/run_${LIB}_N${N}.log 2>&1

python3 plot_keff_vs_height.py logs .
```

22 runs in all (11 heights x 2 libraries), three at a time, each pinned to
5 threads, so 15 of the machine's 16 available cores were in use and one was
left free. N = 10, 12, 20 ran first (one wave per library); N = 11 and 13 to 19
were added at the maintainer's request and ran in a 3-slot pool afterwards.

**N = 9 is not run.** At equal ball count its matched height falls below
RMC's lowest tabulated point (94.182 cm), so there is no reference to compare
against without extrapolating.

**Caution found on this run.** After `git pull`, `reference-data/ace` was
left at its old pin (`git submodule status` showed `+`), and the first launch
stopped at `SKIP: Ni58: tape ... is not in this checkout`. Run
`git submodule update reference-data/ace` after every pull. Those aborted
logs were deleted, and every number here comes from the relaunch.

## 3. Environment variables (complete list used)

| Variable | Value | Meaning |
|---|---|---|
| `OUTRAM_HTR10_HISTORIES` | `10000` | particles per cycle |
| `OUTRAM_HTR10_INACTIVE` | `5` | discarded (inactive) cycles, as in the reference paper |
| `OUTRAM_HTR10_ACTIVE` | `135` | active cycles (140 cycles in total) |
| `OUTRAM_HTR10_RINGS` | `14` | full-radius bed (hex rings of Şeker tiles) |
| `OUTRAM_HTR10_LAYERS` | `10` to `20` | Şeker layers N; bed height `9.798 N + 6` cm |
| `OUTRAM_HTR10_THREADS` | `5` | pinned thread count |
| `OUTRAM_HTR10_ENDF7` | set (VII.0 arm), unset (VIII.0 arm) | library switch |
| `OUTRAM_HTR10_SEED` | unset → default `20260917` | RNG seed |

Every other `OUTRAM_HTR10_*` knob was unset, so every ablation is off and the
example's defaults hold.

## 4. Transport settings

| Setting | Value |
|---|---|
| Eigenvalue mode | k-eigenvalue power iteration (`KeffSettings`) |
| Particles per cycle | 10 000 |
| Cycles | 140 total = 5 inactive + 135 active |
| Histories | 1 400 000 simulated (the log's `histories` line), 1 350 000 of them in active cycles |
| Seed | 20260917 (one seed per point) |
| Temperature | 300.15 K, all materials |
| Tracking | hybrid: delta tracking in the pebble bed, surface tracking elsewhere |
| Uncertainty quoted | within-run 1σ of the active-cycle mean |

## 5. Geometry and materials

| Item | Value |
|---|---|
| Builder | `assemble_explicit_triso(14, N, 0)` |
| Bed | Şeker & Çolak (2003) 13-ball cell; every pebble whole; wall-crossing balls rejected (gh:#472) |
| Bed heights built | `9.798 N + 6` cm, N = 10 to 20 (103.980 to 201.959 cm); per-run values in `results_table.md` |
| Fuel | explicit TRISO lattice inside each fuel pebble |
| Reflector | TECDOC-1382 explicit zones, withdrawn control rods in their channels, reflector zone 22: C 8.8242e-2, natural B 4.7377e-7 |
| Coolant | natural helium (He-3 1.343e-6 at.), ideal gas, 300.15 K, 101.33 kPa (pressure assumed atmospheric, after Şeker & Çolak 2003 p.267) |
| Carbon | natural carbon (VIII.0: C-12/C-13 at 98.93/1.07 at.%; VII.0: elemental MAT 600) |
| Rod metal | full: real Ni-58/60/61/62/64 and Fe-54/56/57/58, plus Cr, Mn, Ti, Si |
| Sizes printed by the run (N = 12) | 22 974 tiles, 43 445 cells, 1 502 universes |

## 6. Nuclear data

| Item | VIII.0 arm | VII.0 arm |
|---|---|---|
| Fuel, moderator, impurities (U, O, C, Si, B) | ENDF/B-VIII.0 | ENDF/B-VII.0 |
| Helium | ENDF/B-VIII.0 | ENDF/B-VIII.0 (no VII.0 tape in the checkout) |
| Rod metal (Fe, Cr, Ni, Mn, Ti, free Si) | ENDF/B-VIII.0 | ENDF/B-VIII.0 (no VII.0 tapes in the checkout) |
| Graphite S(α,β) | `tsl-reactor-graphite-30P.endf` (MAT 32) | `tsl-graphite-ENDF7.0.endf` (MAT 31), crystalline |
| SiC S(α,β) | C-in-SiC (MAT 44), Si-in-SiC (MAT 43) | not applied: VII.0 has no SiC evaluation |
| UO2 laws | U-in-UO2, O-in-UO2, generated with LEAPR in-run | same |
| Processing | in-run, from the ENDF tapes, by the NJOY port (`njoy-outram-park-fork`); per-nuclide times are in each log | same |

## 7. References compared against

| Curve | Source | Library | Role |
|---|---|---|---|
| RMC | Li, Yu & Wei (2014) | ENDF/B-VII.0 | reference |
| MCNP Table 3 | Li (2014) Table 3 = Şeker & Çolak (2003) vacuum column | ENDF/B-VI, TMCCS graphite | gauge only |
| MCNP Table 4 | Li (2014) Table 4 = Şeker & Çolak (2003) helium column | ENDF/B-VI, TMCCS graphite | gauge only; like-for-like coolant with this model |

The values are in `crates/nee_soon/src/htr10_rmc/mod.rs` (`RMC_KEFF_VS_HEIGHT`,
`MCNP_TABLE3_KEFF_VS_HEIGHT`, `MCNP_TABLE4_KEFF_VS_HEIGHT`); the script reads
them from there. Each point is compared at the height where Şeker's model holds
the same number of balls as the built bed (gh:#472), with linear interpolation
between tabulated heights. The references quote no uncertainty.
