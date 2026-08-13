# outram-mc-libs — C++ source map, porting order & test backlog

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
| `src/physics/transport.rs` | `src/physics.cpp` (1249 LOC) — history-based loop notes (still a stub; the live loop is `transport_csg.rs`) |
| `src/physics/transport_csg.rs` | `src/physics.cpp`, `src/geometry.cpp` — the **live** CSG k-eigenvalue transport loop (`run_keff_csg`); its `transport_history` is the per-history kernel reused by the fixed-source driver |
| `src/physics/fixed_source.rs` | new orchestration (not a direct port) over `transport_csg::transport_history` — **fixed-source** driver (`run_fixed_source`): external point/box source, sub-critical multiplication, no `k_eff`; analytic void-streaming V&V |
| `src/physics/scatter.rs` | `src/physics_common.cpp`, `src/physics.cpp` |
| `src/physics/fission.rs` | `src/physics.cpp` — `fission()`, `create_fission_sites()` |
| `src/physics/physics_mg.rs` | `src/physics_mg.cpp` |

### Depletion (new work — not a direct transport-module port)
| Rust file | C++ / Python source |
|---|---|
| `src/depletion/chain.rs` | `src/chain.cpp`, `openmc/deplete/chain.py` — decay/transmutation chain |
| `src/depletion/cram.rs` | `openmc/deplete/cram.py` — CRAM `exp(A·dt)` matrix-exponential solver |
| `src/depletion/matrix.rs` | `openmc/deplete/` — sparse burnup matrix assembly |
| `src/depletion/operator.rs` | `openmc/deplete/coupled_operator.py` — one-group depletion operator |

### Pebble beds (new work — doubly-heterogeneous specialization, absent upstream)
| Rust file | C++ source |
|---|---|
| `src/pebble_beds/` | none upstream — `delta_tracking` (Woodcock) + `stochastic_media` sphere packing for TRISO / pebble-bed cores |

---

## Porting order (bottom-up dependency order)

1. `rng/lcg.rs` — no deps ✅ (implemented)
2. `geometry/position.rs` — no deps ✅ (implemented)
3. `rng/distributions.rs` — depends on lcg ✅ (stubs)
4. `geometry/surface.rs` — depends on position. **Full OpenMC surface set ✅**: axis planes (`XPlane`/`YPlane`/`ZPlane`) + general `Plane`, `Sphere`, `XCylinder`/`YCylinder`/`ZCylinder`, `XCone`/`YCone`/`ZCone`, general `Quadric` (op-ah7), and `XTorus`/`YTorus`/`ZTorus` (op-e5k, quartic ray intersection via an in-file degree-≤4 real-root solver) — evaluate/sense/distance/normal/reflect, all unit-tested.
5. `geometry/cell.rs` — depends on surface (struct ✅; `contains()` RPN region eval ✅)
6. `geometry/universe.rs` — depends on cell (struct ✅; `find_cell()` ✅)
7. `geometry/lattice.rs` — depends on universe (`RectLattice` ✅; `HexLattice` ring construction + indexing ✅)
8. `geometry/geometry.rs` — depends on cell/universe/lattice (`locate()` nested lattice descent ✅)
9. `particle/bank.rs` — depends on position ✅ (implemented)
10. `particle/particle.rs` — depends on position ✅ (implemented)
11. `material/nuclide.rs` — depends on ndarray (✅; point/WMP grid + `xs_at_energy`, `from_core`)
12. `material/reaction.rs` — depends on nuclide (✅)
13. `material/thermal.rs` — depends on nuclide (✅; S(α,β) thermal-scatter tables)
14. `material/material.rs` — depends on nuclide + reaction (✅; nuclide mixture + macroscopic XS)
15. `source/spatial.rs` — depends on position + lcg (point + box ✅; sphere TODO)
16. `source/energy.rs` — depends on distributions (stubs)
17. `source/angle.rs` — depends on position + distributions (stubs)
18. `source/source.rs` — depends on spatial/energy/angle ✅ (implemented)
19. `tally/filter.rs` — no physics deps ✅ (4 filters implemented)
20. `tally/tally.rs` — depends on filter ✅ (implemented)
21. `tally/scoring.rs` — depends on particle + tally (✅; flux/reaction-rate scoring)
22. `physics/scatter.rs` — depends on particle + nuclide (✅; elastic + CM-frame kinematics)
23. `physics/fission.rs` — depends on particle + bank + nuclide (✅; ν sampling + fission-site banking)
24. `physics/transport_csg.rs` — depends on all of the above (✅; live CSG k-eigenvalue loop, `run_keff_csg`). The generic history-based `physics/transport.rs` variant is still a stub.
25. `physics/physics_mg.rs` — depends on transport (still a stub — last; multigroup mode pending)

---

## Test backlog

### P0 — First things to verify
- `rng/lcg.rs`: `future_seed(n, s)` matches n sequential `prn()` calls for n ∈ {1, 100, 10000} ✅
- `geometry/position.rs`: `stream()` correctness, `from_unnormalised()` gives unit vector ✅
- `geometry/surface.rs`: `XPlane/YPlane/ZPlane` evaluate + distance
- `rng/distributions.rs`: `uniform()` stays in [0,1)

### P1 — Geometry correctness
- `Sphere::distance` (quadratic solve) — test at known intersections ✅
- `ZCylinder::distance` — test at known intersections ✅
- `Cell::contains` (RPN evaluator) — simple intersection of two half-spaces ✅
- `Universe::find_cell` — particle locates correct cell in a 3-cell universe ✅

### P2 — Physics correctness
- `Nuclide::xs_at_energy` — log-log interpolation, verify at grid points and midpoints ✅
- Elastic scatter kinematics — energy/angle conservation in CM frame ✅
- Fission ν sampling — mean ν matches tabulated value ✅
- `TallyBin::rel_std_dev` — converges as 1/√N for a score stream

---

## End-to-end status

The full CSG k-eigenvalue path is **live**: `physics::transport_csg::run_keff_csg`
navigates surfaces/cells/universes/lattices (rect **and** hex), samples collisions,
scatters, and banks fission sites. It is validated against the **Godiva bare-sphere**
benchmark (ICSBEP HEU-MET-FAST-001), see `docs/validation.md`. The
`hexagonal-lattice` and `triso` notebook harnesses run this loop live. Still
pending: the generic history-based `transport.rs`, multigroup (`physics_mg.rs`),
DAGMC/unstructured mesh, photon transport, and the C-API.

**Godiva k_eff figures — status after `op-jis` (2026-08-06).** `rng::lcg::prn`
gained OpenMC's PCG-RXS-M-XS output permutation; the LCG **state recurrence is
unchanged**, so nothing in the source map or porting order above is affected, but
sampled k values moved.

- **HIGH-fidelity ENDF/B-VII.1: k_eff = 1.00094 ± 0.00198 (+94 pcm) —
  SUPERSEDED, PENDING A RE-RUN.** Measured 2026-07-07 with the pre-`op-jis`
  output function (raw top-52 state bits). The ENDF path needs
  `--features net-fetch` and has no local tape cache on this machine, so it could
  not be re-run and **no replacement value was measured or invented**.
- **LOW-tier / offline (`Nuclide::from_core`): k_eff = 1.01042 ± 0.00174
  (+1042 pcm vs ICSBEP 1.0000 ± 0.0010)** — re-measured **2026-08-06** from
  `examples/godiva_keff`. *Supersedes* the pre-`op-jis` 1.01022 ± 0.00177
  (+1022 pcm).

---
