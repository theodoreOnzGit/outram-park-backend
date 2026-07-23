# outram-mc-libs

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

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
k-eigenvalue, and delta (Woodcock) tracking for doubly heterogeneous media
(e.g. pebble-bed cores).

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

## Quick start

```toml
[dependencies]
outram-mc-libs = "0.1.0"
```

```rust
use outram_mc_libs::prelude::*;
```

### `outram-mc-tui` — terminal transport UI (opt-in binary)

This crate also ships an optional **mobile-first, touchscreen** terminal UI:
pick a preset geometry (pebble bed / LWR cell / TMSR-like pebble bed / bare
metal sphere), tune the run settings (CPU single/multi/GPU, histories, batches,
seed), and watch the k-eigenvalue converge with a neutron-spectrum /
cross-section overlay. It is a `[[bin]]` **inside this crate**, gated behind the
non-default **`tui`** feature, so library consumers (`tampines`, `nee_soon`, …)
never inherit the `ratatui`/`crossterm` terminal stack — only a build that asks
for the binary does.

```bash
# run from a checkout
cargo run    -p outram-mc-libs --features tui --bin outram-mc-tui --release
# install the standalone binary (also works on Termux/Android)
cargo install --path crates/outram-mc-libs --features tui
```

Full design notes and Termux usage live in
[`docs/outram-mc-tui.md`](docs/outram-mc-tui.md).

## Scope

See `CLAUDE.md` for the full porting-rule and module-scope table (RNG,
geometry, surfaces, particle tracking, k-eigenvalue, pebble-bed delta
tracking). Every transport/physics/geometry behaviour here is ported from
the canonical OpenMC C++ source — see `CLAUDE.md` for the reference-file
discipline.

## License

GPL-3.0-only (see the workspace root `LICENSE`), permitted under the terms
of OpenMC's upstream MIT license — see `TRADEMARKS.md`.
