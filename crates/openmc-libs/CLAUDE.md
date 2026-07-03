# CLAUDE.md — openmc-libs

Pure-Rust port of the OpenMC Monte Carlo neutron transport kernels.

The reference C++ source lives at:
`/home/teddy0/Documents/research/openmc/`

**Upstream license:** OpenMC is MIT-licensed. This Rust port is GPL-3.0-only
per the workspace default; the port constitutes new copyrightable expression.

---

## Porting rule (mandatory) — mirror the canonical source, do not reinvent

**Every transport / physics / geometry behaviour in this crate must be ported
from the canonical OpenMC C++ source at `/home/teddy0/Documents/research/openmc/`
(`src/*.cpp`, `include/openmc/*.h`).** Before implementing anything, grep the
OpenMC source for the corresponding function and mirror its logic — do not
re-derive or reinvent physics that already exists upstream. Cite the reference
`file:line` in the Rust doc comment so a reader can diff against the original.

**Only when a behaviour is genuinely absent upstream** (e.g. the pebble-bed
`delta_tracking` / `stochastic_media` specialization) do you scaffold new parts
and build them out — and mark them clearly as new work, not a port.

Rationale: the crate's entire value is *fidelity* to OpenMC. Reinvented logic
silently drifts from the reference. The C++ is the source of truth; this crate is
a translation of it.

---

## Scope

### In scope
| Module | C++ source | What it does |
|---|---|---|
| RNG | `src/random_lcg.cpp` | LCG with O(log n) jump-ahead for particle splitting |
| Distributions | `src/random_dist.cpp` | Maxwell, Watt, tabulated samplers |
| Geometry / position | `include/openmc/position.h` | 3-D position and direction vectors (cm) |
| Geometry / surfaces | `src/surface.cpp` | Quadric CSG surfaces + distance/sense |
| Geometry / cells | `src/cell.cpp` | Boolean RPN region evaluation |
| Geometry / universes | `src/universe.cpp` | Universe nesting hierarchy |
| Geometry / lattices | `src/lattice.cpp` | Rect + hex lattice indexing |
| Geometry / geometry | `src/geometry.cpp` | `locate_particle`, `distance_to_boundary` |
| Particle state | `src/particle.cpp` | Phase-space state (r, u, E, wgt, seed, …) |
| Particle bank | `src/bank.cpp` | Fission site banking for k-eigenvalue |
| Material | `src/material.cpp` | Nuclide mixture, macroscopic XS |
| Nuclide XS | `src/nuclide.cpp` | Point-energy grid + log-log interpolation |
| Reactions | `src/reaction.cpp` | MT table, Q-value, secondary sampling |
| S(α,β) thermal | `src/thermal.cpp` | Thermal scattering law tables |
| Source sampling | `src/source.cpp` | External source: spatial/energy/angle |
| Tallies | `src/tallies/tally.cpp` | Filter composition + accumulator |
| Tally filters | `src/tallies/filter_*.cpp` | Cell, energy, material, universe, mesh |
| Scoring | `src/tallies/tally_scoring.cpp` | Flux, reaction rate, current accumulation |
| Transport loop | `src/physics.cpp` | `collision()`, `transport_history_based()` |
| Scattering | `src/physics_common.cpp` | Elastic, inelastic, CM-frame kinematics |
| Fission | `src/physics.cpp` | ν sampling, fission bank creation |
| Multigroup | `src/physics_mg.cpp` | Group-averaged cross-section transport |

### Out of scope (will NOT be ported)
- **ENDF nuclear data parsing** — `src/endf.cpp`, `include/openmc/endf.h`
- **HDF5 I/O** — cross-section library loading; data arrives pre-loaded
- **XML configuration parsing** — `src/xml_interface.cpp`
- **Depletion** — `src/chain.cpp`, transmutation matrix
- **CMFD accelerator** — `src/cmfd_solver.cpp`
- **Random ray extension** — `src/random_ray/`
- **Photon/electron transport** — `src/photon.cpp`
- **Python/ctypes C API** — `openmc/lib/` Python package
- **Geometry overlap checker** — `src/geometry_aux.cpp` (overlap detection only; the core intersection logic is in scope)

---

## Design decisions

### Units: raw `f64`, not `uom`
Unlike `openfoam-basic-lib` (which uses `uom` for thermophysics), this crate uses
plain `f64` throughout the inner transport loop.  Monte Carlo simulates billions
of particle histories; a single neutron transport simulation may call
`distance_to_boundary` and `xs_at_energy` O(10⁸) times.  `uom` quantity wrappers
add zero-cost abstraction in principle, but in practice the compile-time overhead
and ergonomic friction in deeply nested loops is not worth it.

Documented unit conventions (enforced by naming, not types):
| Quantity | Unit |
|---|---|
| Length | cm (OpenMC default) |
| Energy | eV |
| Cross-section | barn = 1 × 10⁻²⁴ cm² |
| Macroscopic XS | cm⁻¹ |
| Atom density | atoms / barn·cm |
| Temperature | eV (1 eV ≈ 11604 K) |
| Particle weight | dimensionless (1.0 = fully weighted) |

### No HDF5 dependency in this crate
Cross-section data is loaded externally and passed in by value or reference.
This crate is pure algorithmic: no file I/O, no XML, no HDF5.

### Neutron-only initially
Photon and electron physics (`src/photon.cpp`) are deferred.  The `ParticleType`
enum reserves slots for them, but only `Neutron` transport is implemented.

### Parallelism: per-particle RNG streams
OpenMC's reproducibility guarantee relies on each particle having a completely
independent LCG stream obtained by jump-ahead.  This Rust port preserves that
design: `init_seed(id, offset, master)` derives a unique starting seed for each
particle.  The jump-ahead in `future_seed(n, seed)` is O(log n), implemented in
`src/rng/lcg.rs`.

---

## Port reference (read on demand)

The full Rust-module → OpenMC C++ source map, the bottom-up porting order (with
per-module implementation status), and the prioritised test backlog all live in
**`docs/port-reference.md`**.

## Build and test

**Rule: always use `--release` for builds and tests.** Never run in debug mode.

```bash
cargo check -p openmc-libs --lib
cargo test  -p openmc-libs --lib --release
```

## Porting workflow (mandatory)

After implementing any module, update `src/prelude.rs` with new public items,
then `cargo check -p openmc-libs` to verify.
