# CLAUDE.md — openmc-libs

Pure-Rust port of the OpenMC Monte Carlo neutron transport kernels.

The reference C++ source lives at:
`/home/teddy0/Documents/research/openmc/`

**Upstream license:** OpenMC is MIT-licensed. This Rust port is GPL-3.0-only
per the workspace default; the port constitutes new copyrightable expression.

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

## C++ source reference map

### RNG
| Rust file | C++ source |
|---|---|
| `src/rng/lcg.rs` | `src/random_lcg.cpp`, `include/openmc/random_lcg.h` |
| `src/rng/distributions.rs` | `src/random_dist.cpp`, `src/distribution_energy.cpp`, `src/distribution_angle.cpp` |

### Geometry
| Rust file | C++ source |
|---|---|
| `src/geometry/position.rs` | `include/openmc/position.h` |
| `src/geometry/surface.rs` | `src/surface.cpp` (1422 LOC), `include/openmc/surface.h` |
| `src/geometry/cell.rs` | `src/cell.cpp` (1861 LOC), `include/openmc/cell.h` |
| `src/geometry/universe.rs` | `src/universe.cpp` (217 LOC) |
| `src/geometry/lattice.rs` | `src/lattice.cpp` (1219 LOC) |
| `src/geometry/geometry.rs` | `src/geometry.cpp` (495 LOC), `src/geometry_aux.cpp` |

### Particle
| Rust file | C++ source |
|---|---|
| `src/particle/particle.rs` | `src/particle.cpp` (1044 LOC), `src/particle_data.cpp` |
| `src/particle/bank.rs` | `src/bank.cpp`, `include/openmc/bank.h` |

### Material
| Rust file | C++ source |
|---|---|
| `src/material/material.rs` | `src/material.cpp` (1603 LOC) |
| `src/material/nuclide.rs` | `src/nuclide.cpp` (1229 LOC) |
| `src/material/reaction.rs` | `src/reaction.cpp` (424 LOC), `src/physics_common.cpp` |
| `src/material/thermal.rs` | `src/thermal.cpp` |

### Source
| Rust file | C++ source |
|---|---|
| `src/source/source.rs` | `src/source.cpp` (778 LOC) |
| `src/source/spatial.rs` | `src/distribution_spatial.cpp` |
| `src/source/energy.rs` | `src/distribution_energy.cpp` |
| `src/source/angle.rs` | `src/distribution_angle.cpp` |

### Tallies
| Rust file | C++ source |
|---|---|
| `src/tally/tally.rs` | `src/tallies/tally.cpp` |
| `src/tally/filter.rs` | `src/tallies/filter_*.cpp` (30 files) |
| `src/tally/scoring.rs` | `src/tallies/tally_scoring.cpp` |

### Physics
| Rust file | C++ source |
|---|---|
| `src/physics/transport.rs` | `src/physics.cpp` (1249 LOC) |
| `src/physics/scatter.rs` | `src/physics_common.cpp`, `src/physics.cpp` |
| `src/physics/fission.rs` | `src/physics.cpp` — `fission()`, `create_fission_sites()` |
| `src/physics/physics_mg.rs` | `src/physics_mg.cpp` |

---

## Porting order (bottom-up dependency order)

1. `rng/lcg.rs` — no deps ✅ (implemented)
2. `geometry/position.rs` — no deps ✅ (implemented)
3. `rng/distributions.rs` — depends on lcg ✅ (stubs)
4. `geometry/surface.rs` — depends on position (plane surfaces ✅; others TODO)
5. `geometry/cell.rs` — depends on surface (struct ✅; `contains()` TODO)
6. `geometry/universe.rs` — depends on cell (struct ✅; `find_cell()` TODO)
7. `geometry/lattice.rs` — depends on universe (stubs)
8. `geometry/geometry.rs` — depends on cell/universe/lattice (TODO)
9. `particle/bank.rs` — depends on position ✅ (implemented)
10. `particle/particle.rs` — depends on position ✅ (implemented)
11. `material/nuclide.rs` — depends on ndarray (stub)
12. `material/reaction.rs` — depends on nuclide (stub)
13. `material/thermal.rs` — depends on nuclide (stub)
14. `material/material.rs` — depends on nuclide + reaction (stub)
15. `source/spatial.rs` — depends on position + lcg (point + box ✅; sphere TODO)
16. `source/energy.rs` — depends on distributions (stubs)
17. `source/angle.rs` — depends on position + distributions (stubs)
18. `source/source.rs` — depends on spatial/energy/angle ✅ (implemented)
19. `tally/filter.rs` — no physics deps ✅ (4 filters implemented)
20. `tally/tally.rs` — depends on filter ✅ (implemented)
21. `tally/scoring.rs` — depends on particle + tally (TODO)
22. `physics/scatter.rs` — depends on particle + nuclide (TODO)
23. `physics/fission.rs` — depends on particle + bank + nuclide (TODO)
24. `physics/transport.rs` — depends on all of the above (TODO)
25. `physics/physics_mg.rs` — depends on transport (last)

---

## Test backlog

### P0 — First things to verify
- `rng/lcg.rs`: `future_seed(n, s)` matches n sequential `prn()` calls for n ∈ {1, 100, 10000} ✅
- `geometry/position.rs`: `stream()` correctness, `from_unnormalised()` gives unit vector ✅
- `geometry/surface.rs`: `XPlane/YPlane/ZPlane` evaluate + distance
- `rng/distributions.rs`: `uniform()` stays in [0,1)

### P1 — Geometry correctness
- `Sphere::distance` (quadratic solve) — test at known intersections
- `ZCylinder::distance` — test at known intersections
- `Cell::contains` (RPN evaluator) — simple intersection of two half-spaces
- `Universe::find_cell` — particle locates correct cell in a 3-cell universe

### P2 — Physics correctness
- `Nuclide::xs_at_energy` — log-log interpolation, verify at grid points and midpoints
- Elastic scatter kinematics — energy/angle conservation in CM frame
- Fission ν sampling — mean ν matches tabulated value
- `TallyBin::rel_std_dev` — converges as 1/√N for a score stream

---

## Porting workflow (mandatory)

After implementing any module, update `src/prelude.rs` with new public items,
then `cargo check -p openmc-libs` to verify.
