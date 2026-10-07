# Architecture: what is ported, what is new

## The problem

A Monte Carlo transport code is a few thousand lines of physics wrapped in tens
of thousands of lines of bookkeeping. Porting one raises two questions a reader
needs answered before any physics: **which parts are a translation of OpenMC, and
which are new?** The answer decides how a disagreement is debugged. In a
translation, the first suspect is something upstream does that the port does not;
in new work there is no upstream to read.

## The rule that governs the port

The crate's own `CLAUDE.md` states it as a hard rule: every transport, physics
or geometry behaviour is ported from the canonical OpenMC C++ source [(Romano et al., 2015)](#ref-romano2015openmc), the
reference `file:line` is cited in the doc comment, and only behaviour that is
**genuinely absent upstream** is written fresh and labelled *NEW WORK*
([`CLAUDE.md`, "Porting rule"](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/CLAUDE.md#L524-L541)).

One exception is stated alongside it: the random-number generator only has to be
**statistically** right, not draw-for-draw identical to OpenMC
([`CLAUDE.md`, "RNG goal"](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/CLAUDE.md#L905-L935)). That
is why a result here is compared with OpenMC *within statistics*, never bit for
bit.

## The map

The full module-to-C++ table is
[`docs/port-reference.md`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/docs/port-reference.md). The
short version:

| Area | Rust | Ported from | Notes |
|---|---|---|---|
| RNG | `rng` | `random_lcg.cpp` | The LCG now lives in `petir` and is re-exported ([`rng/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/rng/mod.rs#L19)) |
| Geometry (CSG, lattices, `locate`, `distance_to_boundary`) | `geometry::*` | `geometry.cpp`, `cell.cpp`, `surface.cpp`, `lattice.cpp` | **Moved to `outram-blender` on 2026-10-02** (#486), re-exported under the old paths |
| Surface crossing, boundary conditions | [`geometry::crossing`](../../api/outram_mc_libs/geometry/crossing/index.html) | `particle.cpp`, `boundary_condition.cpp` | Stayed here: it needs the RNG and the transport state |
| Collision physics | [`physics::scatter`](../../api/outram_mc_libs/physics/scatter/index.html), [`physics::fission`](../../api/outram_mc_libs/physics/fission/index.html) | `physics.cpp`, `physics_common.cpp` | |
| CSG k-eigenvalue loop | [`physics::transport_csg`](../../api/outram_mc_libs/physics/transport_csg/index.html) | `physics.cpp`, `eigenvalue.cpp` | The live history loop |
| Variance reduction | `physics::{variance_reduction, weight_windows, ufs}` | `physics_common.cpp`, `weight_windows.cpp`, `eigenvalue.cpp` | Off by default, on purpose (chapter 5) |
| Multigroup | [`physics::physics_mg`](../../api/outram_mc_libs/physics/physics_mg/index.html) | `physics_mg.cpp`, `mgxs.cpp` | Chapter 7 |
| Delta tracking | [`physics::delta_tracking`](../../api/outram_mc_libs/physics/delta_tracking/index.html) (~~`pebble_beds::delta_tracking`~~ moved 2026-10-06, #718; the old path re-exports it) | — | **NEW WORK**, chapter 3 |
| Depletion | `depletion` | `openmc/deplete` (Python) | New orchestration over ported pieces |

The geometry move is worth noticing: the description of space (surfaces, cells,
lattices) now belongs to the mesh-authoring crate, while everything that needs
a particle's state stays here. The split is recorded in
[`CLAUDE.md`, "Geometry lives in outram-blender"](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/CLAUDE.md#L784-L812),
and a bit-identity test (`tests/stats_move_fingerprints.rs`) guards that moving
the code changed no answer.

## How variation is expressed: enums, not trait objects

The workspace forbids `Box<dyn Trait>`; choices are enums matched in one place.
Two examples you will meet again:

- **Which backend runs the histories.** [`ComputeType`](../../api/outram_mc_libs/physics/compute/enum.ComputeType.html)
  is `CpuSingleThread` (the default, and the bit-reproducible reference),
  `CpuMultiThread` or `Gpu`. [`run_keff`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/keff.rs#L348-L363)
  is a `match` over it.
- **How a particle crosses a region.** `TrackingMethod` is `Surface` (the
  default) or `Delta { majorant }`
  ([`outram-blender/src/csg/cell.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-blender/src/csg/cell.rs#L143-L187)).
  The history loop matches on it at exactly one point (chapter 3).

The benefit for a reader is that every variant of a behaviour is visible in one
`match`, and rust-analyzer's "go to definition" lands on it.

## The drivers

[`physics/mod.rs`](https://github.com/theodoreOnzGit/outram-park-backend/blob/@@COMMIT@@/crates/outram-mc-libs/src/physics/mod.rs#L1-L48) lists what
runs a whole calculation:

- `keff::run_keff` — one bare homogeneous sphere. Small enough to read in one
  sitting; the place to start.
- `transport_csg::run_keff_csg` — the same power iteration over general CSG
  geometry with tallies. **This is the loop the rest of this book follows.**
- `fixed_source::run_fixed_source` — an external source, no eigenvalue.
- `physics_mg::run_keff_mg` — the multigroup twin.
- `pebble_beds::keff_delta` — the delta-tracking driver for pebble beds.

<!-- references:begin -->
## References

<p class="csl-entry" id="ref-romano2015openmc" style="padding-left: 2em; text-indent: -2em;">Romano, P. K., Horelik, N. E., Herman, B. R., Nelson, A. G., Forget, B., & Smith, K. (2015). OpenMC: A state-of-the-art Monte Carlo code for research and development. <span style="font-style: italic;">Annals of Nuclear Energy</span>, <span style="font-style: italic;">82</span>, 90–97.</p>

<!-- references:end -->
