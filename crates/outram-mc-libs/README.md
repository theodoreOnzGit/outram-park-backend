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


> **This is OUTRAM PARK's independent Rust translation of selected OpenMC
> algorithms.** It is not the official OpenMC software and is not
> affiliated with, endorsed by, or sanctioned by MIT or Argonne National
> Laboratory. See [`TRADEMARKS.md`](./TRADEMARKS.md) for the full
> attribution and non-affiliation notice. Translated from
> [`openmc-dev/openmc`](https://github.com/openmc-dev/openmc), `develop`
> branch — no commit is pinned (translation was done by reading the
> C++/Python source directly, not from an ongoing codegen-from-clone
> pipeline); see `upstream_source/README.md` for the full provenance record.

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
as this limitation until #466 lands a shared, process-wide GPU context for the
tests. If it bites, re-run with `--test-threads=1`. Transport itself uses one
device per run and is not affected.

## Quick start

```toml
[dependencies]
outram-mc-libs = "0.1.0"
```

```rust
use outram_mc_libs::prelude::*;
```

### `outram-mc-tui` — terminal transport UI

This crate also ships a **mobile-first, touchscreen** terminal UI: pick a preset
geometry (pebble bed / LWR cell / TMSR-like pebble bed / bare metal sphere),
tune the run settings (CPU single/multi/GPU, histories, batches, seed), and
watch the k-eigenvalue converge with a neutron-spectrum / cross-section overlay.
It is a `[[bin]]` **inside this crate** (not a separate crate); `ratatui` is an
unconditional dependency, so it always builds with a plain `cargo build` — no
feature flags.

```bash
# run from a checkout
cargo run -p outram-mc-libs --bin outram-mc-tui --release
# install the standalone binary (also works on Termux/Android)
cargo install --path crates/outram-mc-libs
```

Full design notes and Termux usage live in
[`docs/outram-mc-tui.md`](docs/outram-mc-tui.md).

## Scope

See `CLAUDE.md` for the full porting-rule and module-scope table (RNG,
geometry, surfaces, particle tracking, k-eigenvalue, pebble-bed delta
tracking). ~~Every transport/physics/geometry behaviour here is ported from
the canonical OpenMC C++ source~~ **CORRECTED 2026-10-15:** transport,
physics and geometry behaviour is ported from the canonical OpenMC C++
source, except for the deliberate deviations listed below. See `CLAUDE.md`
for the reference-file discipline.

## Port deviations from OpenMC

These are places where the port **deliberately does not follow upstream**.
Each one is listed so that "ported from OpenMC" is never read as covering
it.

| Area | OpenMC | This crate | Why | Known consequence |
|---|---|---|---|---|
| **RNG seeding and streams** | One run-wide particle counter, `(total_gen + gen − 1) × n_particles + id`, times the stride (`simulation.cpp` `compute_transport_seed`). **Four streams per particle** (tracking, source, URR tables, volume), each started from its own master seed, `seeds[i] = M·(master + i) + C` (`random_lcg.cpp` `init_particle_seeds`). | A two-level jump: `gen × 2⁴⁰`, then `hist × 152 917` ([`transport_csg.rs`](src/physics/transport_csg.rs) `HIST_STRIDE`, `GEN_STRIDE`). **One stream per history.** The delayed-group, URR-table and split-child streams are jumps of 3, 5 and 1 × 152 917 draws **inside that stream** (`DELAYED_STRIDE`, `URR_STRIDE`, `SPLIT_STRIDE`; also `keff.rs`, `keff_delta.rs`). | The goal is statistical correctness and repeatability, not draw-for-draw parity (maintainer decision 2026-08-06, `CLAUDE.md` "RNG goal"). Histories here run in a different order and sample their source differently, so parity was never reachable. A run is repeatable for a given seed at any thread count. | **gh:#700 (open, P2):** each auxiliary stream starts where another history's tracking stream starts. History *i*'s URR draws are history *i+5*'s, its delayed-group draws are *i+3*'s, and a split child's are *i+1*'s. Each history still sees uniform draws, so mean `k` is unbiased, but neighbouring histories are not fully independent, which affects the quoted σ. Its size is not measured. |
| Generator itself | PCG-RXS-M-XS output on a 64-bit LCG | **Identical**: constants, output permutation, jump ahead ([`petir::rng::lcg`](../petir/src/rng/lcg.rs)) | — | Not a deviation; listed so the row above is not over-read |
| MG (n,xn) scattering multiplicity | `wgt *= mult[g][g']` (`scattdata.cpp:367`) | Weights by the row multiplicity `nu_row / s_row` | Upstream samples g′ from the nu-scatter row, so its weighting is exact only for a row-uniform multiplicity | On a two-group set this port gives 1.516 ± 0.003 against the analytic 1.517; upstream's rule gives 1.578 (`physics_mg::tests::multiplicity_weights_by_the_row_and_matches_analytic_kinf`) |

Two other categories are not deviations: behaviour that is **NEW WORK** (absent
upstream, e.g. delta tracking; see `CLAUDE.md`) and what is **out of scope**
(`CLAUDE.md`, "Out of scope").

## License

GPL-3.0-only (see the workspace root `LICENSE`), permitted under the terms
of OpenMC's upstream MIT license — see `TRADEMARKS.md`.

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
