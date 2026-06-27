# openmc-libs — C++ source map, porting order & test backlog

Reference material mapping each Rust module to its OpenMC C++ source, the
bottom-up porting order (with per-module implementation status), and the
prioritised test backlog. Consulted on demand when porting a module — not
per-turn guidance. The crate scope and design decisions live in CLAUDE.md.

OpenMC reference C++ source tree: `/home/teddy0/Documents/research/openmc/`

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
