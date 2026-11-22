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
ported **as is** (porting rule). Each is pinned by a test so a deliberate fix
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

## Code-to-code against OpenMC: PENDING, NOT YET RUN

The deck is `openmc_inputs/beams_mesh_surface.py` (**NOT YET RUN**): 66
deterministic single-neutron beams through a near-transparent 20 cm box,
`current` on a `MeshSurfaceFilter` over a rectilinear, a cylindrical (four
partial-width phi bins on a full circle, origin off zero) and a spherical mesh
(theta edges including pi/2, origin off zero), one run per site, as gh:#261's
Experiment A. Run it on the OpenMC build being prepared in parallel, then add
the Rust side (same tracks through `MeshSurfaceFilter::matches`) and record
OpenMC version/commit, library, hardware and the per-mesh bin agreement here.
Predicted: identical bins on every track, **including** findings A and C if
OpenMC's `libm` rounds `cos(pi/2)` and the discriminant as PETIR's
`RealMath` does; any disagreement on a track that crosses theta = pi/2 is
then a `libm` difference, not a port defect.
