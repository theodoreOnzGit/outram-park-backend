# outram-mc-libs

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Monte Carlo deep dive:** <https://theodoreonzgit.github.io/outram-park-backend/deep-dives/monte-carlo/>,
how this crate is built and why, lesson beside code (sources in
[`docs/lessons/`](./docs/lessons/); [API reference](https://theodoreonzgit.github.io/outram-park-backend/api/outram_mc_libs/)).
First draft 2026-10-03, not yet human-reviewed.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

What this crate makes studyable and challengeable is how a Monte Carlo
neutron-transport code works, routine by routine, beside the OpenMC C++ it was
translated from: random-number streams, CSG geometry and surface crossing,
collision physics, power iteration for `k_eff`, and Woodcock (delta) tracking,
which OpenMC does not have, for pebble beds. A reader can follow a deep dive
that sets lessons beside the code, run tutorials from Godiva to an HTR-10
pebble bed, and inspect benchmark residuals that are kept when they move the
wrong way, such as Jemima moving from −253 to −395 pcm when URR
self-shielding was turned on by default.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | OpenMC (`openmc-dev/openmc`, MIT): translated from `develop` with no pinned commit, and compared since 2026-11-10 against commit `8f79d9d0` cloned and built in `upstream_source/` (`upstream_source/README.md`); Liang Jingang et al.'s `virtual_lattice` fork (`liangjg/openmc` commit `be04e280`, MIT) for `src/geometry/virtual_lattice/`; LIGGGHTS through `outram-park-fork-liggghts` for the DEM pebble bed. Benchmarks: ICSBEP HEU-MET-FAST-001 (Godiva), IEU-MET-FAST-002 (Jemima), HST-009, LEU-COMP-THERM-008 (`verification_and_validation/icsbep/`). |
| Mathematical equations | `docs/lessons/` (the Monte Carlo deep dive: surface tracking, delta tracking, collisions and fission, power iteration, multigroup, correct physics as default), `docs/port-reference.md`, `docs/temperatures.md`, and module docs under `src/physics/`. |
| Source code | `src/rng/`, `src/geometry/` (CSG, lattices, `virtual_lattice`, TRISO particles), `src/material/`, `src/particle/`, `src/physics/` (`transport_csg.rs`, `keff.rs`, `fixed_source.rs`, `delta_tracking/`, `physics_mg.rs`, `kinetics.rs`, `variance_reduction.rs`, `weight_windows.rs`), `src/tally/`, `src/source/`, `src/depletion/`, `src/pebble_beds/`, `src/stochastic/`, `src/plotter/`, `src/model/`, `src/gpu/`. |
| Interactive simulations | `outram-mc-tui`, a touchscreen terminal UI (`src/bin/outram-mc-tui/`, `docs/outram-mc-tui.md`); the tutorial book `docs/tutorial/` with its widgets and the `tutorial_*` examples; about 90 headless examples under `examples/`. The `dhoby-ghaut` Monte Carlo web demo runs on this crate. |
| Visualisations | `src/plotter/` (geometry plots and plot scripts), records in `verification_and_validation/geometry_plot/`, `geometry_plotting/` and `python_plotting_parity/`; about 200 committed images in the crate. |
| Verification and validation evidence | 61 record folders under `verification_and_validation/`. ICSBEP (`icsbep/README.md`): Godiva re-measured at `−10 ± 9 pcm` over 400 seeds on 2026-09-20 with URR and DBRC on, later found to have been taken on a defective PURR grid (gh:#325), so its movement is "not yet measured"; after the OpenMC-parity audit (2026-09-30, gh:#407) every case and route agreed with OpenMC + NJOY2016 within 2 sigma except LCT-008 on route 3, at −18 ± 8 pcm (96 seeds a side). Re-measurement on the 2026-11-12 defaults is in `icsbep_remeasure_2026_11_12/`. Code to code against built OpenMC: `upstream_openmc_surfaces/`, `upstream_openmc_samplers/`, `upstream_openmc_seeding/`, `upstream_openmc_reaction_walk/`, `openmc_godiva_cross_code/`. `tests/correct_physics_is_default.rs` pins URR and DBRC on by default. `docs/validation.md` is a superseded 2026-07-07 record. |
| Human reviews and corrections | 19 functions carry a kovan human-review stamp (among them `physics/transport_csg.rs::transport_history_vr`, `physics/delta_tracking/majorant.rs::Majorant` and `rng/distributions.rs::watt`). No entry in `docs/human-corrections-to-ai-work.md` names this crate. The URR/DBRC-default correction (2026-09-20) is recorded in the root `CLAUDE.md` and at the top of `verification_and_validation/icsbep/README.md`. |

**Status in one sentence:** transport has code-to-code evidence against a
built OpenMC and recorded ICSBEP residuals; no residual has been human-reviewed,
several await re-measurement on the current defaults, and nothing is validated
for any engineering use.


> **This is OUTRAM PARK's independent Rust translation of selected OpenMC
> algorithms.** It is not the official OpenMC software and is not
> affiliated with, endorsed by, or sanctioned by MIT or Argonne National
> Laboratory. See [`TRADEMARKS.md`](./TRADEMARKS.md) for the full
> attribution and non-affiliation notice. Translated from
> [`openmc-dev/openmc`](https://github.com/openmc-dev/openmc), `develop`
> branch — ~~no commit is pinned~~ **CORRECTED 2026-11-27** — no commit was
> pinned for the translation itself (done by reading the C++/Python source
> directly, not from an ongoing codegen-from-clone pipeline), but since
> 2026-11-10 the differential tests use commit `8f79d9d0`, cloned and built
> under `upstream_source/` (checked in `upstream_source/README.md`, "Commit at
> last sync"); see that file for the full provenance record.

Pure-Rust port of selected [OpenMC](https://openmc.org) Monte Carlo
neutron-transport kernels — RNG, geometry/CSG, particle tracking,
k-eigenvalue (criticality) and fixed-source (external-source / shielding)
drivers, and delta (Woodcock) tracking for doubly heterogeneous media
(e.g. pebble-bed cores).

Both canonical Monte Carlo modes are available: the **k-eigenvalue** driver
(`run_keff` / `run_keff_csg`) for criticality, and the **fixed-source** driver
(`run_fixed_source`) for an external neutron source (point or box) driving a
sub-critical or non-multiplying system — the shielding / detector-response
case, scoring track-length tallies with no power iteration.

Data-free: all cross sections come from `njoy-outram-park-fork`'s
`XsProvider` surface, not from any data bundled in this crate. See
`NUCLEAR_DATA.md` for how nuclear-data distribution is planned to work
(runtime downloader + cache vs. embedded curated subsets).

## `no_std` + `alloc` (gh:#1605)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-mc-libs --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). What stays: geometry, materials, sources,
tallies, the transport and k-eigenvalue drivers, delta tracking, the
`model` API on in-memory or embedded data, and statistics. What `std` adds:

- threads (`rayon`; without it the parallel drivers run serially through
  `wasm_par`, which the drivers' per-history seeding makes exact, as on wasm);
- files: ENDF/ACE/HDF5 loading from disk, MCPL and `collision_track.h5`
  output, reports, `perf_report`, `run_diagnostics`, `vv`;
- clocks, printing and environment variables (without `std` every
  `OUTRAM_MC_*` switch reads as unset, so it takes its default);
- the GPU path (`wgpu`; without `std` the CPU path is used, as on Android);
- S(α,β) thermal laws, which NJOY's LEAPR generates (without `std`,
  `NuclearData::load_nuclide` with a law returns an error saying so);
- the DEM pebble bed (`pebble_beds::dem_bed`, LIGGGHTS), the OpenMC decay-data
  tables of `depletion::chain`, and the `outram-mc-tui` binary.

With `std` on (the default) nothing changes: every switch above re-exports
the `std` item it replaces.

## Compute backend & GPU tradeoff (opt-in)

The k-eigenvalue driver takes a `ComputeType` (set on `KeffSettings`), selecting
the transport backend:

- **`CpuSingleThread`** — scalar `f64`, the **bit-reproducible trusted reference**.
- **`CpuMultiThread(ThreadCount)`** — rayon-parallel over histories; a dedicated
  pool auto-sized to the machine (`available_parallelism`) — more threads on a
  desktop, fewer on a phone. Reproducible independent of thread count. **~7× over
  single-thread** on a 12-core box.
- **`Gpu`** — GPU-accelerated (`f32`), **desktop only** (`wgpu` is target-gated
  off Android), with a **graceful CPU fallback** (debug message when no adapter;
  no GPU code compiled on Android).

**The tradeoff, stated plainly (accepted design choice):** GPU uses `f32` for
speed; CPU uses `f64` and stays the trusted / V&V / publication reference. And
importantly — **Monte Carlo transport is memory-bound and branch-divergent, so
the GPU path does *not* currently beat `CpuMultiThread`** (measured on an RTX
3050; it's launch/transfer bound, see beads `op-u6s.7`/`op-u6s.8`). GPU compute
*does* win on the *compute-bound* nuclear-data kernels in `njoy-outram-park-fork`
(the Faddeeva pole-sum, up to ~60×). Rule of thumb: **GPU helps where the work is
arithmetic-dense, not where it's memory-random** — pick the backend accordingly;
CPU remains the trusted result either way.

**Known limitation: concurrent GPU device creation can crash the test process
(GitHub #466).** `tests/gpu_geometry_helpers_directly.rs` exits with SIGSEGV
under `--test-threads=3` and passes 10/10 at `--test-threads=1`. The library's
own test binary SIGSEGV'd once under 14 threads on 2026-10-02 and then passed
five reruns. Each GPU test creates its own `wgpu` device and queue, and
several threads doing that at once in one process appear to race inside the
Vulkan driver or loader. The crash is below this crate, which is safe Rust,
and it is a race, so it is intermittent. Treat a SIGSEGV in a GPU test binary
as this limitation ~~until #466 lands a shared, process-wide GPU context for the
tests. If it bites, re-run with `--test-threads=1`.~~ **CHANGED 2026-11-17
(maintainer decision on #466):** the GPU tests now take one process-wide device
from `gpu::shared_probe()` (created once, reused by every test), so the
concurrent creation can no longer happen in the test binaries. On Mesa lavapipe
the crash did not reproduce before the change either (0 in 180 runs at 4 and 14
test threads), so the fix is by construction, not shown by a crash count going
from non-zero to zero. A SIGSEGV that still appears should be reported.
Transport itself uses one device per run and is not affected.

## Quick start

```toml
[dependencies]
outram-mc-libs = "0.0.8"
```

```rust
use outram_mc_libs::prelude::*;
```

(The version above read ~~`"0.1.0"`~~ until **CORRECTED 2026-11-27** to the
crate's `Cargo.toml` version, `0.0.8`.)

### `outram-mc-tui` — terminal transport UI

This crate also ships a **mobile-first, touchscreen** terminal UI: pick a preset
geometry (pebble bed / LWR cell / TMSR-like pebble bed / bare metal sphere),
tune the run settings (CPU single/multi/GPU, histories, batches, seed), and
watch the k-eigenvalue converge with a neutron-spectrum / cross-section overlay.
It is a `[[bin]]` **inside this crate** (not a separate crate); ~~`ratatui` is an
unconditional dependency, so it always builds with a plain `cargo build` — no
feature flags.~~ **CORRECTED 2026-11-27** — `ratatui` is optional, enabled by
the default `std` feature on non-wasm targets, and the binary has
`required-features = ["std"]` (checked in `Cargo.toml`). A plain
`cargo build --release` still builds it, because `std` is on by default.

```bash
# run from a checkout
cargo run -p outram-mc-libs --bin outram-mc-tui --release
# install the standalone binary (also works on Termux/Android)
cargo install --path crates/outram-mc-libs
```

Full design notes and Termux usage live in
[`docs/outram-mc-tui.md`](docs/outram-mc-tui.md).

## Scope

See `AGENTS.md` for the full porting-rule and module-scope table (RNG,
geometry, surfaces, particle tracking, k-eigenvalue, pebble-bed delta
tracking). ~~Every transport/physics/geometry behaviour here is ported from
the canonical OpenMC C++ source~~ **CORRECTED 2026-10-15:** transport,
physics and geometry behaviour is ported from the canonical OpenMC C++
source, except for the deliberate deviations listed below. See `AGENTS.md`
for the reference-file discipline.

## Port deviations from OpenMC

These are places where the port **deliberately does not follow upstream**.
Each one is listed so that "ported from OpenMC" is never read as covering
it.

| Area | OpenMC | This crate | Why | Known consequence |
|---|---|---|---|---|
| **RNG seeding and streams** | One run-wide particle counter, `(total_gen + gen − 1) × n_particles + id`, times the stride (`simulation.cpp` `compute_transport_seed`). **Four streams per particle** (tracking, source, URR tables, volume), each started from its own master seed, `seeds[i] = M·(master + i) + C` (`random_lcg.cpp` `init_particle_seeds`). | A two-level jump: `gen × 2⁴⁰`, then `hist × 152 917` ([`transport_csg.rs`](src/physics/transport_csg.rs) `HIST_STRIDE`, `GEN_STRIDE`). **One stream per history.** The delayed-group, URR-table and split-child streams are jumps of 3, 5 and 1 × 152 917 draws **inside that stream** (`DELAYED_STRIDE`, `URR_STRIDE`, `SPLIT_STRIDE`; also `keff.rs`, `keff_delta.rs`). | The goal is statistical correctness and repeatability, not draw-for-draw parity (maintainer decision 2026-08-06, `AGENTS.md` "RNG goal"). Histories here run in a different order and sample their source differently, so parity was never reachable. A run is repeatable for a given seed at any thread count. | **gh:#700 (open, P2):** each auxiliary stream starts where another history's tracking stream starts. History *i*'s URR draws are history *i+5*'s, its delayed-group draws are *i+3*'s, and a split child's are *i+1*'s. Each history still sees uniform draws, so mean `k` is unbiased, but neighbouring histories are not fully independent, which affects the quoted σ. Its size is not measured. |
| Generator itself | PCG-RXS-M-XS output on a 64-bit LCG | **Identical**: constants, output permutation, jump ahead ([`petir::rng::lcg`](../petir/src/rng/lcg.rs)) | — | Not a deviation; listed so the row above is not over-read |
| MG (n,xn) scattering multiplicity | `wgt *= mult[g][g']` (`scattdata.cpp:367`) | Weights by the row multiplicity `nu_row / s_row` | Upstream samples g′ from the nu-scatter row, so its weighting is exact only for a row-uniform multiplicity | On a two-group set this port gives 1.516 ± 0.003 against the analytic 1.517; upstream's rule gives 1.578 (`physics_mg::tests::multiplicity_weights_by_the_row_and_matches_analytic_kinf`) |

Two other categories are not deviations: behaviour that is **NEW WORK** (absent
upstream, e.g. delta tracking; see `AGENTS.md`) and what is **out of scope**
(`AGENTS.md`, "Out of scope").

## License

~~GPL-3.0-only~~ **AGPL-3.0-only** (relicensed 2026-10-20, gh:#973; see `LICENSE`), permitted under the terms
of OpenMC's upstream MIT license — see `TRADEMARKS.md`.

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
