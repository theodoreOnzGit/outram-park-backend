# Surface distance and lattice distance against OpenMC, bit by bit

<!-- vv-unverified-banner -->
> Research, education and V&V only. Not for nuclear facility operation, reactor
> control, safety-critical or licensing decisions (`RESPONSIBLE_USE.md`).

gh:#1386 prio-1 rows 2 and 3 (epic gh:#1336, differential testing). Tests:
`crates/outram-mc-libs/tests/surface_distance_vs_openmc_code_to_code.rs` (16 tests)
and `crates/outram-mc-libs/tests/lattice_distance_vs_openmc_code_to_code.rs` (9 tests).

## Upstream

| | |
|---|---|
| Project | OpenMC, <https://github.com/openmc-dev/openmc> |
| Commit | `8f79d9d02483c90e4134f03a9df4b75ce6b062bf` (2026-11-10, "Prevent Accidental Use of Forward Sources in the Adjoint Solve of FW-CADIS (#4216)") |
| Licence | MIT (the repository `LICENSE`) |
| Accessed | 2026-11-10 |
| Clone | `crates/outram-mc-libs/upstream_source/OpenMC` (shallow, gitignored by `/upstream_source/*/`; recorded in `upstream_source/README.md`) |

OpenMC is **not built** (it needs HDF5, pugixml, fmt). `run.sh` compiles a small
`g++ -O2 -std=c++17 -ffp-contract=off` driver against:

- the routines below, copied **verbatim** by `extract.sh` after it checks the commit and the
  SHA-256 of every source file (and that each extracted function starts on the expected line
  and ends at the first column-0 `}`):
  `Surface::sense`, `axis_aligned_{plane,cylinder,cone}_{evaluate,distance,normal}`, the
  `evaluate`/`distance`/`normal` of all 15 surface classes, `torus_distance`
  (`src/surface.cpp`); `RectLattice::{distance,get_indices,get_local_position}`,
  `HexLattice::{distance,get_indices,get_local_position}` (`src/lattice.cpp`); `isclose`
  (`src/math_functions.cpp:1015`); `coincident` (`include/openmc/geometry.h:68`); four constants
  (`FP_PRECISION`, `FP_COINCIDENT`, `TORUS_TOL`, `INFTY`) from `include/openmc/constants.h`;
- **unmodified** `src/position.cpp` and `src/external/quartic_solver.cpp`, and the real
  `include/openmc/position.h`, `array.h`, `vector.h`, `external/quartic_solver.h`.

### Substitutions (every one)

| id | what | why | effect on compared numbers |
|---|---|---|---|
| S1 | `stubs/fmt/format.h` | fmt is not installed; `position.h` only uses it for a `fmt::formatter<Position>` specialisation | none |
| S2 | `stubs/surface_stub.h`: the 15 surface classes with only their data members and the three virtuals, member names copied from `include/openmc/surface.h` | the real header needs pugixml and HDF5 | none: the bodies are upstream's |
| S3 | `stubs/lattice_stub.h`: `RectLattice` and `HexLattice` with only their data members and the three methods | same | none |
| S4 | the loops of the ray trace and the lattice walk (`ray_trace`, `rect_walk`, `hex_walk` in `driver.cpp`) are written here | upstream's cell and geometry code needs the whole model | only `surface.distance` and the lattice routines are upstream's; the nearest-surface rule is copied from `Region::distance_to_nearest_surface` (`src/cell.cpp:955-995`) |
| S5 | the pseudo-random probes come from a 64-bit LCG in the driver | only chooses inputs; every input is printed | none |

`reference.txt` (25 008 lines, 3.4 MB) is the committed output. `generated/` is gitignored.

## Inputs

**Surfaces:** 15 kinds, 33 parameter sets (planes at 1.5 and -3250.25; general plane x2;
cylinders and spheres, a unit-scale and a tiny one; cones; three quadrics incl. a paraboloid that
exercises the `a == 0` branch; three tori per axis with `b != c` in two of them). For each: 14
generic origins (inside, outside, far, random) x 14 directions (axis-aligned, diagonal, near-axis
`(1, 1e-9, 0)`, random) with `coincident = false`; plus 3 to 6 points that lie **exactly** on the
surface and the same points shifted by (4e-13, 0, -3e-13) (inside the `|f| < 1e-12` band without
being on it), each with the 14 directions, the outward and inward normal and two tangent
(grazing) directions, with `coincident = false` **and** `true`. 25 008 lines in all, of which
~21 000 are surface cases.
Printed per case: origin, direction, flag, `evaluate`, `distance`, `sense`, as hex bits.

**Ray trace (integration):** six rays through ten surfaces of seven kinds (3 z-cylinders, a
sphere, 2 z-planes, an x-plane, a general plane, an x-cone, an elliptical z-torus, a quadric),
up to 24 crossings each, with the last-crossed surface flagged `coincident`.

**Lattices:** rect 0 (2-D, pitch 2), rect 1 (3-D, pitch ~1), **rect 2 (3-D, pitch 1e5), rect 3
(2-D, pitch 1e6)**; hex 0-3 (orientation y/x, 2-D/3-D, pitch 1.8, off-centre), **hex 4-5 (4
rings, pitch 1e4)**. `distance` on tile-local origins (centre, quarter points, edges, corners,
random) x 12 directions (incl. `(1, 1+-1e-13, 0)`); `get_indices` on points exactly on tile
boundaries, just inside, outside the lattice, hex edge mid-points and vertices; tile centres; a
12-crossing walk of five rays through every lattice.

## Prediction (written before the first run)

- `evaluate`: bit-identical for every surface except where the port wrote the expression in a
  different order (quadric, tori).
- `distance`: bit-identical for the cylinders and the cones (same algebra), **different** for
  planes and the general plane (port: `1e-14` direction threshold and `d > 0`; upstream
  `|f| < 1e-12`), the sphere (port forces `c = 0` when coincident, `1e-10` floor), cones and
  quadric (generic quadratic solver with an epsilon floor), and the tori (a different root
  finder: round-off at best).
- Lattices: indices and hex distance bit-identical; `RectLattice::distance` value identical but
  the **translation vector wrong for large pitch** (the port predates upstream #3853).

What happened: the cylinders were **not** identical (the port used `a = u^2 + v^2` where upstream
has `a = 1 - u_axis^2`), and the planes looked identical on the first (exact-point) grid because
the probes were not near enough to a surface; the 4e-13 shifted points were added after that run
and the "before" numbers below were re-measured on the final input set. The tori `evaluate` also
differed, for the reason in the next section. Everything else was as predicted.

## Results

Counts are `evaluate` bit-identical / `distance` bit-identical or both "no hit" / `sense` equal,
out of the cases of that kind. "No hit" is `DBL_MAX` upstream and `f64::INFINITY` in the port;
the tests count that pair as agreeing (a representation difference, both mean "never crosses";
the port keeps `INFINITY` because callers test `is_finite`).

**Before (port at `469de110e`, final input set, 2026-11-10) and after (this change):**

| kind | cases | evaluate before | distance before | sense before | all three after |
|---|---|---|---|---|---|
| xplane / yplane / zplane | 680 each | 680 | 656 / 680 / 648 (24 / 0 / 32 hit-vs-miss) | 680 | 680 / 680 / 680 |
| plane | 680 | 680 | 660 (20) | 680 | 680 |
| xcyl / ycyl / zcyl | 680 each | 680 | 589 / 601 / 639 (17 / 0 / 0 hit-vs-miss; rest last-bit, worst 1.3e-8) | 678 / 676 / 678 | 680 |
| sphere | 752 | 752 | 698 (15) | 750 | 752 |
| xcone / ycone / zcone | 744 each | 744 | 661 / 631 / 642 (37 / 30 / 27) | 744 / 744 / 742 | 744 |
| quadric | 1236 | 1074 | 1063 (39) | 1236 | 1236 |
| xtorus / ytorus / ztorus | 1596 each | 628 / 642 / 628 | 948 / 937 / 887 (234 / 235 / 239 hit-vs-miss) | 1304 / 1271 / 1294 | 1596 |
| ray trace, 6 rays | 84 crossings | | 9 identical before the first divergence | | 86 of 86 |

"After" is bit-identical in every column; the two "(hit-vs-miss)" counts are cases where one
code found a crossing and the other did not. The 25 tests pass: `cargo test --release -p
outram-mc-libs --test surface_distance_vs_openmc_code_to_code --test
lattice_distance_vs_openmc_code_to_code` (16 + 9 passed, 0 ignored; g++ 13.3, rustc 1.99.1,
x86-64, glibc libm, 2026-11-10).

**Lattices** (before = tile-local convention for the hex walk, `RectLattice::distance` of the
earlier port; after = this change):

| quantity | cases | before | after |
|---|---|---|---|
| `RectLattice::distance`, pitch ~1-2 | 360 | value 360, translation **330** | 360 / 360 |
| `RectLattice::distance`, pitch 1e5-1e6 (#3853) | 360 | value 360, translation **320** | 360 / 360 |
| `RectLattice::get_indices` | 2760 | 2760 | 2760 |
| `HexLattice::distance` small / large pitch | 2064 / 480 | 2064 / 480 | 2064 / 480 |
| `HexLattice::get_indices` | 4416 | 4416 | 4416 |
| hex tile centre | 20 | 20 | 20 |
| rect walk (260 steps), hex walk (390 steps) | | 230, **363** | 260, 390 |

## What was wrong, and what this change did (the port now follows upstream)

1. **Surface kinds** (`crates/outram-blender/src/csg/surface/`): the planes, general plane,
   sphere, cones and quadric used their own thresholds and a generic quadratic solver; the
   cylinders used `a = u^2 + v^2`. All now translate `src/surface.cpp` (`axis_aligned_*`
   helpers, same operation order). The quadric `evaluate` uses upstream's grouping.
   `Surface::sense`'s coincident branch dotted `u` with the **normalised** normal; upstream
   uses the raw one and a grazing dot product of zero up to round-off flips sign
   (`SurfaceKind::normal_unnormalised`).
2. **Tori, gh:#701:** `b` and `c` were swapped (`b` radial in the port, axial upstream; the
   plotter's `bounds.rs` already used OpenMC's meaning, so the crate disagreed with itself), and
   the root finder was derivative bracketing + bisection. Fixed: OpenMC's field meaning
   everywhere (the GPU encoder swaps when packing, since the f32 twin keeps the old slots), the
   torus quartic's coefficients, coincident rule and interior-root check are upstream's, and
   `oqs::quartic_solver` is ported (`quartic_oqs.rs`, a line-by-line translation of the 575-line
   C++). Complex-coefficient branches use a private complex type whose `sqrt` and `/` are the
   textbook formulas, not `libstdc++`'s; they only produce non-real roots, which `torus_distance`
   discards, so no compared number depends on them.
3. **`RectLattice::distance` (upstream #3853):** the port rebuilt the crossing position and
   compared it with the edge to an absolute 1e-12, which fails at large pitch (and, as measured,
   at 1e-13-level near-ties even at pitch 2). Now upstream's per-axis distances and `isclose`.
   `std::min({dx, dy, dz})` is reproduced by first-smallest, so `-0.0` is returned as upstream does.
4. **`HexLattice::distance` calling convention** (not changed, recorded): the port takes the
   *tile-local* position and rebuilds the lattice-frame one by adding the tile centre; upstream
   is handed the lattice-frame position (`geometry.cpp:459-467`). `(r - off) + off` is not `r`
   in floating point: **27 of 390 walk steps differ in the last bits** through `distance()`.
   `distance_in_lattice_frame()` is the faithful routine (390 of 390 identical) and `distance()`
   is a wrapper over it. The transport path still goes through the wrapper.

## Upstream oddities (recorded, not copied)

- `HexLattice::distance` leaves `lattice_trans` **uninitialised** when no boundary is hit (a
  2-D lattice and `u = (0, 0, +-1)`): `d = INFTY` with a garbage translation (the reference
  holds values like 1629860000). The port returns `[0, 0, 0]`; the test does not compare the
  translation when upstream's distance is `INFTY`.
- `Surface::sense` and the non-torus `normal` are not unit length upstream; the port's `normal`
  stays normalised (reflection and white boundaries need it) and `sense` now uses the raw vector.
- Upstream "no hit" is `DBL_MAX`; the port's is `INFINITY` (kept on purpose).

## Not compared here (gh:#1386)

- **`init_particle_seeds`** (`src/random_lcg.cpp:108-115`) is not ported and no OpenMC-compatible
  seeding option exists. `crates/petir/tests/lcg_vs_openmc_code_to_code.rs` already compares
  `prn`/`future_seed`/`init_seed` bit for bit; its harness pattern is
  `crates/petir/verification_and_validation/upstream_openmc_lcg/`. Porting the seeding option
  is port work and was not done here.
- `Surface::normal` and `reflect`; `find_cell`, `cross_lattice`; the secondary samplers.

## Reproduce

```bash
git clone --depth 1 https://github.com/openmc-dev/openmc.git crates/outram-mc-libs/upstream_source/OpenMC
git -C crates/outram-mc-libs/upstream_source/OpenMC fetch --depth 1 origin 8f79d9d02483c90e4134f03a9df4b75ce6b062bf
git -C crates/outram-mc-libs/upstream_source/OpenMC checkout 8f79d9d02483c90e4134f03a9df4b75ce6b062bf
crates/outram-mc-libs/verification_and_validation/upstream_openmc_surfaces/run.sh   # rewrites reference.txt
cargo test --release -p outram-mc-libs --test surface_distance_vs_openmc_code_to_code \
  --test lattice_distance_vs_openmc_code_to_code
```
