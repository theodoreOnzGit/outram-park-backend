# outram-mc-libs

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


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

## Quick start

```toml
[dependencies]
outram-mc-libs = "0.1.0"
```

```rust
use outram_mc_libs::prelude::*;
```

## Scope

See `CLAUDE.md` for the full porting-rule and module-scope table (RNG,
geometry, surfaces, particle tracking, k-eigenvalue, pebble-bed delta
tracking). Every transport/physics/geometry behaviour here is ported from
the canonical OpenMC C++ source — see `CLAUDE.md` for the reference-file
discipline.

## License

GPL-3.0-only (see the workspace root `LICENSE`), permitted under the terms
of OpenMC's upstream MIT license — see `TRADEMARKS.md`.
