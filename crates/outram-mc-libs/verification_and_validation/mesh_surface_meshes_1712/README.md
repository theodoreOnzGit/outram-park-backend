# Mesh-surface currents on rectilinear, cylindrical and spherical meshes (gh:#1712)

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-drafted; untrusted
> until a human has reviewed it.

## What is ported

OpenMC `8f79d9d0`, `src/mesh.cpp` and `include/openmc/mesh.h`:

| upstream | where it is here |
|---|---|
| `StructuredMesh::raytrace_mesh` + `SurfaceAggregator` (`src/mesh.cpp:1312-1488`) | `tally::mesh::raytrace_surface_bins`, generic over the private `StructuredRaytrace` trait (compile-time, no `dyn`) |
| `StructuredMesh::get_indices` (`:1180`), `get_bin_from_indices` (`:1194`) | `StructuredRaytrace::indices`, `bin_from_indices` |
| `RegularMesh::get_index_in_direction` / `distance_to_grid_boundary` (`:1640`, `:1667`) | unchanged helpers from gh:#1709 |
| `RectilinearMesh::distance_to_grid_boundary` (`:1839`), `get_index_in_direction` (`:1885`), `positive/negative_grid_boundary` (`:1827-1837`) | `rectilinear_*` |
| `CylindricalMesh::get_indices` (`:1980`), `find_r_crossing` (`:2028`), `find_phi_crossing` (`:2074`), `find_z_crossing` (`:2107`), `distance_to_grid_boundary` (`:2128`) | `cylindrical_*` |
| `SphericalMesh::get_indices` (`:2278`), `find_r_crossing` (`:2332`), `find_theta_crossing` (`:2363`), `find_phi_crossing` (`:2427`), `distance_to_grid_boundary` (`:2460`) | `spherical_*` |
| `sanitize_angular_index` (`mesh.h:649`), `MeshDistance` and its `operator<` (`mesh.h:314-325`), `PeriodicStructuredMesh::local_coords` (`mesh.h:480`), `lower_bound_index` (`search.h:14`), `RADIAL_MESH_TOL`, `INFTY` (`constants.h:59`, `:103`) | private helpers / constants in `tally/mesh.rs` |

`raytrace_mesh` computes distances from `local_coords(r0)` (origin subtracted
once, at the track start) but locates elements with
`get_indices(global_r + s u)`, which subtracts the origin itself. That
distinction is carried: for a cylindrical or spherical mesh with a non-zero
origin the index position is `(r0 + s u) - origin`, the distance position
`r0 - origin`.

`MeshSurfaceFilter` now holds a `MeshKind`; `new` accepts the four structured
kinds and refuses `Unstructured`, as upstream `fatal_error`s
(`UnstructuredMesh::surface_bins_crossed`, `src/mesh.cpp:1087`).

## Prediction (written 2026-11-22, before anything was run)

1. **Regular mesh: bit-identical.** The generic raytrace, instantiated on
   `RegularMesh`, performs the same floating-point operations in the same
   order as the gh:#1709 specialised port, so
   `tests/mesh_surface_raytrace_1709.rs` and the four mesh-surface tests in
   `tests/tally_filters_vs_openmc.rs` pass unchanged, and the generic and the
   old routine give identical bin lists on every random track.
2. **Rectilinear mesh with uniformly spaced planes: identical to the regular
   port on random off-tie tracks.** The plane positions differ at round-off
   (`lower_left + i * width` against a stored grid value) and the index lookup
   is a binary search instead of `ceil((r - ll) / w)`, so a track passing
   within ~1e-15 cm of a plane could differ; off ties (no end point within
   1e-6 cm of a plane, as gh:#1709's test) none should.
3. **Cylindrical and spherical: analytic tracks agree exactly in bins.** A
   radial track through the axis crosses each r-shell once going in and once
   going out, with an outward/inward pair at each interior shell and one
   inward (entering) or outward (leaving) at the outer shell. A track that
   passes through the axis/centre in `r` passes from shell 1 back into shell 1
   with **no** `r = 0` crossing (`find_r_crossing` returns `INFTY` for
   `r0 == 0`).
4. **Conservation:** on random tracks, for every element the track enters and
   leaves (start and end outside the element), the number of inward and
   outward currents is equal; exactly so, since every crossing is counted
   once by the walker.
5. **Where OpenMC and this port could differ (code-to-code, not yet run):**
   the cylindrical/spherical crossings use `sqrt`, `cos`, `sin`, `atan2`,
   `acos`, `hypot`. Here they go through `petir::mathf::RealMath`; OpenMC
   uses the platform `libm`. A 1-ulp difference in a crossing distance only
   changes the **bins** when two crossings are within ~1e-15 cm of each other
   or a track ends within that of a surface. Expected: bin lists identical on
   off-tie tracks, so tallies bit-identical except for such ties.

### Upstream behaviour carried deliberately, worth a reviewer's eye

- `find_r_crossing` skips a shell when the **track start** `R` lies within
  `RADIAL_MESH_TOL * (1 + r0)` of it, for the whole track (it uses the
  start position, not the current one). A track starting on a shell that
  later crosses the same shell on the far side does not score that later
  crossing. Ported as is.
- `find_phi_crossing` with a full `[0, 2pi]` phi grid and one phi bin returns
  `INFTY` (no real surface); with a partial grid an out-of-range phi index
  sanitises to 0.

## Results (2026-11-22, commit of this change; tests in `tests/mesh_surface_curvilinear_1712.rs`)

| check | prediction | result |
|---|---|---|
| 1. regular: generic walker vs a frozen copy of the gh:#1709 walker, 20 000 random tracks (15 000 with a start or end exactly on a plane), 113 026 bins | identical | **identical** |
| `mesh_surface_raytrace_1709` (2 tests), the four mesh-surface tests of `tally_filters_vs_openmc` (worst scaled diff 0.000e0, 0 misses, unchanged) | pass unchanged | **pass unchanged** |
| 2. rectilinear with uniform planes vs regular, 20 000 off-tie tracks, 113 644 bins | identical | **identical** |
| 3a. non-uniform rectilinear, three hand-computed tracks | exact | **exact** |
| 3b. radial chord through the axis (cylindrical) / centre (spherical), origin off zero | `[27, 24, 15, 12, 3, 2, 13, 14, 25, 26]` | **`[27, 24, 15, 12, 1, ...]`: prediction wrong in one label, see finding A** |
| 3c. cylindrical phi crossing; 2 pi -> 0 wrap; reverse wrap | `[6, 17]`, `[42, 5]`, `[4, 43]` | `[6, 17]`, `[42, 5]`, **`[4, 41]`** (finding A) |
| 3d. spherical theta = pi/2 crossing, down / up | `[6, 17]`, `[16, 7]` | `[6, 17]`, **`[16, 5]`** (finding A) |
| 4. conservation, 5000 random tracks per mesh: rectilinear (13 890 bins), cylindrical full phi (29 917), cylindrical partial phi with r_min > 0 (9 484), spherical (27 384) | exact on every track | **exact**, after two upstream behaviours were found and excluded from the random set (findings B, C), each pinned by its own test |

`cargo test --release -p outram-mc-libs --test mesh_surface_curvilinear_1712
--test mesh_surface_raytrace_1709 --test every_filter_kind_is_reachable --test
tally_filters_vs_openmc`: 10 + 2 + 11 + 30 passed, 0 failed.
`cargo test --release -p outram-mc-libs --lib`: 677 passed, **1 failed**,
`vv::msre_n1::tests::plenums_and_openings_match_their_sources`, which asserts
that the ENDF tape `n-028_Ni_058-ENDF8.0.endf` is present; this worktree does
not have the reference-data tapes, and the test touches nothing changed here.
`cargo check --release -p outram-mc-libs --lib --no-default-features --target
x86_64-unknown-none`: builds (`no_std` + `alloc`).

### Findings: upstream behaviour the port reproduces (worth a maintainer's eye and possibly an upstream report)

All three were found by the tests, read back to the upstream lines, and are
ported **as is** (porting rule). **Fixed beside the port 2026-11-23 (gh:#1715,
maintainer: "Fix them beside the port"):** see the last section. The port is
unchanged and is reached by `MeshSurfaceFilter::with_openmc_raytrace()`. Each is pinned by a test so a deliberate fix
has to change it. Whether OpenMC itself shows them is what the pending
code-to-code run decides.

- **A. The inward-current face label is inferred from the next exit.**
  `raytrace_mesh` scores the inward current with
  `!distances[k].max_surface` (`src/mesh.cpp:1393`, and `:1427` in the
  outside branch): the face is taken from where the track will **leave** the
  element on axis `k`, not from where it came in. On a Cartesian axis these
  are always opposite faces. On a radial axis, entering the core element
  through r = 1 means leaving through r = 1 again, so the inward current is
  labelled the **min** r face (zero radius). With no further exit on the axis
  (both distances INFTY; `std::min` keeps the first, `max_surface = true`) it
  is labelled min as well, whichever face it entered through (the reverse
  phi wrap and the theta case). The element and the inward/outward sense are
  right, so element-wise conservation holds; the **face** of an inward
  current on a curvilinear axis is not reliable. Tests:
  `cylindrical_radial_chord_crosses_each_shell_in_and_out`,
  `spherical_radial_chord_crosses_each_shell_in_and_out`,
  `cylindrical_phi_crossings_including_the_two_pi_wrap`,
  `spherical_theta_crossing`.
- **B. Partial phi grid: a track starting above the last phi plane scores
  nothing.** `CylindricalMesh::get_indices` sanitises the out-of-range phi
  index `N + 1` to 0 (`sanitize_phi`, `include/openmc/mesh.h:649`), so
  `find_phi_crossing` looks for `phi_grid[0]` instead of `phi_grid[N]`,
  returns INFTY, and the outside branch travels that distance and returns.
  Starting below the first plane is unaffected. In the conservation run
  11 063 random starts above phi_max were excluded to reach 5000 kept tracks.
  Test: `partial_phi_track_starting_above_phi_max_is_lost` (applies to the
  spherical phi axis by the same code; not separately tested).
- **C. A theta edge at exactly pi/2 can be missed by round-off.** The cone
  theta = pi/2 is the plane z = 0; `find_theta_crossing`
  (`src/mesh.cpp:2363`) solves a quadratic whose two roots coincide there, so
  the discriminant is zero in exact arithmetic and its sign is round-off. On
  the pinned track `D = -3.55e-15` (recomputed in Python with the same
  operations), INFTY is returned and the z = 0 crossing is not scored; the
  walker then ends in the wrong theta element. A hemisphere split at pi/2 is
  a natural mesh, so this one matters in practice. The conservation run uses
  1.45 instead of pi/2. Test: `theta_half_pi_cone_crossing_is_lost_to_round_off`.

Prediction 3 was written from the geometry, not from the code's labelling
rule, which is why it missed finding A; predictions 1, 2 and 4 held (4 only on
meshes free of B and C).

## Code-to-code against OpenMC (run 2026-11-22)

~~PENDING, NOT YET RUN~~ **Run 2026-11-22.** The deck
`openmc_inputs/beams_mesh_surface.py` was run unchanged. It had been written
before any OpenMC output existed; only its "NOT YET RUN" banner is now stale,
and that is noted here rather than edited. Test:
`tests/mesh_surface_vs_openmc_1712.rs`. Reference:
`openmc_reference/` (nonzero bins only, `site bin value`; `sites.txt`).

- **OpenMC:** 0.15.3, commit `27e38e89` (built from source for gh:#1711, no
  MPI, GCC 13.3.0, HDF5 1.10.10). **Library:** the gh:#1711 O-16 library
  (`make_o16_lib.py`, NJOY2016 `ac5adf5f`). 66 runs in 5.8 s on a 4-core
  Intel Xeon @ 2.30 GHz, 1 thread, 15 GiB RAM, Linux, CPU only.
- **Rust side:** the same track per site, from the source point to the box
  exit pushed on by `TINY_BIT` (OpenMC scores a leaking track once,
  `particle.cpp:743-748`), through `MeshSurfaceFilter::matches`. The direction
  is renormalised as OpenMC renormalises it when reading the source
  (`u_ref_ /= u_ref_.norm()`, `distribution_multi.cpp:47`).

| mesh | nonzero bins, ours / OpenMC | misses |
|---|---|---|
| rectilinear | 186 / 186 | **0** |
| cylindrical (origin off zero) | 187 / 187 | **0** |
| spherical (theta edge at pi/2, origin off zero) | 109 / 109 | **0** |

**Prediction held.** Every bin equal on every track.

- **Findings A and C are OpenMC's own behaviour.** Finding A (the inward-face
  label) shows on many crossings of the curved axes, and OpenMC labels them the
  same. Finding C shows at site 55: the track runs from z = +3.7 to below the
  mesh's z = 0 plane (theta = pi/2), and OpenMC does not score that crossing
  either.
- **The input is part of the check.** A first run of this test without the
  renormalisation missed 12 bins, all at site 55. The direction then differed
  from OpenMC's in the last bit, which flipped the round-off at theta = pi/2.
  With the same input there are 0 misses. This is finding C's sensitivity,
  seen directly.
- **Finding B was not exercised.** The deck's meshes have full phi coverage, so
  partial-phi behaviour has still not been checked against OpenMC.

All three findings are upstream defects that the port reproduces, as the
porting rule requires. Whether to report them upstream, or to fix them beside
the port, is for the maintainer.

## gh:#1715: the three defects fixed beside the port (2026-11-23)

**Finding B against OpenMC first.** `openmc_inputs/beams_partial_phi.py` was
written before it was run. It fires 45 beams at a cylindrical and a spherical
mesh with phi in [0, 2.5]:
- three hand-picked tracks start at phi = 3.5 and pass through the mesh;
- two control tracks start at phi = -0.6;
- 40 tracks are random.

OpenMC 0.15.3 (`27e38e89`, same build as above, 45 runs in 3.0 s) scores
**0 bins on all three tracks starting above phi_max**, and scores both controls.
B is OpenMC's own behaviour, like A and C.

**Fixes (`tally/mesh.rs`, `MeshSurfaceRaytrace::Corrected`, the default of
`MeshSurfaceFilter` by the correct-physics-default rule).** This is a separate
walker, `raytrace_surface_bins_corrected`, with its own hooks. No port function
was edited.
- **A.** An inward current is labelled by the face entered: the opposite of the
  face just left, or for an entry from outside, the side the track came from.
- **B.** A partial phi gap is one region outside the mesh, bounded by phi_min
  going up and phi_max going down. A partial theta range keeps 0 and n + 1 as
  they are; theta does not wrap.
- **C.** A theta edge with |cos theta| < FP_PRECISION is crossed as the plane
  z = 0.

**Tests** (`tests/mesh_surface_corrected_1715.rs`, predictions in its module
docs written before the first run):

| check | corrected | port |
|---|---|---|
| regular and rectilinear meshes, 20 000 random tracks each | identical to the port, bin for bin | (reference) |
| geometry: elements entered, read from the bins, against `MeshKind::bin` sampled every 1e-4 cm (4 curvilinear meshes x 400 tracks) | **0 mismatches** on every mesh | 0 / 41 / 53 / 53 mismatches (full-phi cylinder / partial-phi cylinder / sphere with pi/2 edge / partial theta+phi sphere) |
| face pairs: an outward bin on face m followed by an inward bin on face !m between neighbours | **0 violations** | 199 / 39 / 342 / 9 |
| theta = pi/2 crossing scored | yes | lost (finding C) |
| partial-phi deck against OpenMC | scores the three tracks OpenMC loses (8 bins on the cylinder, 10 on the sphere, each) | **0 misses** against OpenMC (36 / 40 nonzero bins) |

One amendment, made before any result was judged: the first pair check also
counted a track that leaves the mesh through a theta cone and re-enters through
the same cone in another phi sector. That is real geometry, confirmed by the
geometry check, not a pair. The check now requires the two elements to be
neighbours along the axis crossed. The code was not changed.

**Default changed:** `MeshSurfaceFilter::new` now bins curvilinear meshes with
the corrected route. Regular and rectilinear results do not move. The
code-to-code tests (`mesh_surface_vs_openmc_1712.rs`, and the curvilinear tests
that pin upstream's labels) select the port by name.
