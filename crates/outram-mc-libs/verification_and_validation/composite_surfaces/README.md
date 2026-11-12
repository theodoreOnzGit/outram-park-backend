# OpenMC composite surfaces against OpenMC's own Python

<!-- vv-unverified-banner -->
> Research, education and V&V only. Not for nuclear facility operation, reactor
> control, safety-critical or licensing decisions (`RESPONSIBLE_USE.md`).

gh:#1389 (child of epic #257). Code: `crates/outram-mc-libs/src/geometry/composite/`.
Test: `crates/outram-mc-libs/tests/composite_surfaces_vs_openmc.rs` (4 tests) plus 8 unit
tests in `src/geometry/composite/tests.rs`.

## Upstream

| | |
|---|---|
| Project | OpenMC, <https://github.com/openmc-dev/openmc> |
| Commit | `8f79d9d0` (2026-11-10) |
| File | `openmc/model/surface_composite.py`; `openmc/surface.py` for `Plane.from_points`, `flip_normal`, `rotate`, `translate`, `Cone`/`Cylinder._get_base_coeffs` |
| Licence | MIT |
| Accessed | 2026-11-12, from the clone at `crates/outram-mc-libs/upstream_source/OpenMC` |

| composite | upstream lines | port |
|---|---|---|
| `CylinderSector` (incl. `from_theta_alpha`) | 71-237 | `CylinderSector` |
| `IsogonalOctagon` (own `__neg__` and `__pos__`) | 240-395 | `IsogonalOctagon` |
| `X/Y/ZConeOneSided` | 737-897 | `ConeOneSided` (axis as a parameter) |
| `Polygon` | 900-1369 | `Polygon` (see the deviation below) |
| `CruciformPrism` | 1372-1454 | `CruciformPrism` |
| `RectangularPrism` (rounded corners) | 1457-1585 | `RectangularPrism` |
| `ConicalFrustum` (with `_rotation_matrix`) | 1752-1869 | `ConicalFrustum` |
| `Vessel` | 1872-1962 | `Vessel` |

Every composite expands into surface types that already existed
(`outram_blender::csg::surface`): axis planes, the general `Plane`, axis cylinders and
cones, and the general `Quadric` (the vessel heads; the rotated frustum cone, which is
what OpenMC's Python exports its general `Cone`/`Cylinder` as). No surface was ported
from `src/surface.cpp`. Regions are RPN `RegionToken` streams over the composite's own
surfaces (`CompositeRegion`), evaluated by the existing `Cell::contains`; there is no
region expression type yet (#709 is open).

### Deviations, every one

| what | why | effect |
|---|---|---|
| `Polygon` is triangulated by ear clipping, not qhull's joggled Delaunay (`Delaunay(..., 'QJ')`) | qhull is not ported | The convex pieces, so the internal facets, can differ from upstream. The point set does not (both are unions of triangles that cover exactly the polygon); checked below. `_group_simplices` and `_get_convex_hull_surfs` are upstream's |
| `Polygon` convexity test: "every point within `1e-12 * max|coord|` of a hull facet" stands in for qhull `Qc`'s coplanar count | qhull not ported | grouping only, not the region |
| `dot` is an FMA chain | numpy's `np.dot` calls OpenBLAS `ddot`, which fuses (measured, below) | makes the coefficients bit-identical |
| composite `translate`/`rotate`, periodic boundaries, `Polygon.offset` | not ported | `with_boundary(Periodic)` is refused |

`rz` polygons with an edge on the axis are **refused**, as upstream refuses them
(`openmc.ZCylinder(r=-0.0)` fails its `r > 0` check). Found while building the
reference; the port reproduces the refusal (unit test `upstream_value_errors_are_refused`).

## Methodology

Predictions were posted on gh:#1389 before any run (comment of 2026-11-12).

1. **Reference.** `openmc_inputs/make_reference.py` imports OpenMC's Python package
   from the upstream clone (numpy 2.4.6, scipy 1.17.1, matplotlib 3.11.2, CPython 3.11,
   x86_64 Linux; the C library is not needed) and, for 27 cases, writes
   `reference.txt`: the case's box and seed; the component surfaces' coefficients in
   `_surface_names` order (general `Cone`/`Cylinder` as their `Quadric` coefficients,
   exactly what `to_xml_element` gives the C++ code); and, for 2000 splitmix64 points in
   the box, `point in -composite` (`1`/`0`), or `x` within 1e-9 of any surface the
   region uses. The script checks `+composite` is the complement of `-composite` on every
   kept point. Regenerate with:

   ```bash
   OPENMC_PY=crates/outram-mc-libs/upstream_source/OpenMC \
   OPENMC_FAKE_META=<dir with openmc-0.15.3.dist-info/METADATA> \
   python3 make_reference.py > ../reference.txt
   ```

2. **Membership.** The test regenerates the same points bit for bit and classifies them
   with `CompositeRegion::contains` (the transport's `Cell::contains`), both half-spaces.
   Pass: zero disagreements.
3. **Coefficients.** Every non-polygon surface: same kind, same count, every coefficient
   bit-identical. Pass: all bit-identical.
4. **Stochastic volume.** The existing `VolumeCalculation` port, 10^6 samples per case,
   on the two-cell model (`CompositeRegion::two_cell_geometry`), against the analytic
   volume in the box (area x slab length for prisms; `pi r2 H^3/3` for a cone nap; the
   frustum formula; cylinder + two half-ellipsoids for the vessel; Pappus for `rz`).
   Pass: `|z| <= 4` per case and every sample located in exactly one cell.
5. **Drawing.** `render_material_slice` on the two-cell model, one slice across and one
   along each composite's axis, through the centroid of its inside points; the test
   checks both materials appear in every slice.

## Results (2026-11-12)

- **Membership: 0 of 54 000 points disagree** (27 cases x 2000; no point fell within
  1e-9 of a surface, so none was dropped). `+` is the complement of `-` on all of them.
  Prediction: 0. Met, Polygon included.
- **Coefficients: 366 of 366 bit-identical** (4 of them `+0.0` against `-0.0`).
  Prediction: bit-identical except possibly a few ulp on `ConicalFrustum`. The frustum
  turned out bit-identical; the miss was elsewhere: with a left-to-right `dot`, 365 of 366
  matched and `sector_x`'s `plane2.d` was 1 ulp off (`0.6920903430595147` against
  OpenMC's `...148`). Cause, measured: `np.dot(n, p1)` returns the FMA-chain value
  `fma(n2, p2, fma(n1, p1, n0 p0))`; the left-to-right sum and the exactly rounded sum are
  both `...147`. `np.sum(n * p1)` gives `...147` and `n @ p1` gives `...148`, so it is the
  BLAS path. With `dot` as that FMA chain all 366 match. Note this makes the reference
  depend on numpy's BLAS kernel: a non-FMA OpenBLAS kernel would give `...147` there.
- **Stochastic volume: every case within 1.5 sigma** (largest `|z|` 1.50,
  `sector_z_reflex`), `chi^2 = 14.85` over 27 cases (expected 27 +- 7.3, a little low, not
  significant). Every sample located in exactly one of the two cells.

| case | V (cm^3) | sigma | analytic | z |
|---|---|---|---|---|
| sector_z | 9.79745 | 0.02940 | 9.77384 | +0.80 |
| sector_z_reflex | 39.02331 | 0.04797 | 39.09538 | -1.50 |
| sector_y | 12.55273 | 0.03005 | 12.56637 | -0.45 |
| sector_x | 15.35323 | 0.03258 | 15.35890 | -0.17 |
| octagon_z | 28.83440 | 0.02470 | 28.84063 | -0.25 |
| octagon_y | 28.85960 | 0.02470 | 28.84063 | +0.77 |
| octagon_x | 28.84350 | 0.02470 | 28.84063 | +0.12 |
| cone_x_up | 2.09830 | 0.00843 | 2.09440 | +0.46 |
| cone_y_down | 3.01799 | 0.00998 | 3.01593 | +0.21 |
| cone_z_up | 8.38761 | 0.02150 | 8.37758 | +0.47 |
| rect_z_round | 23.59884 | 0.01967 | 23.57080 | +1.43 |
| rect_y_round | 23.17752 | 0.01975 | 23.15876 | +0.95 |
| rect_x_round | 18.61333 | 0.01799 | 18.60938 | +0.22 |
| rect_z_square | 24.00840 | 0.01959 | 24.00000 | +0.43 |
| cruciform_z_inc | 47.97345 | 0.04899 | 48.00000 | -0.54 |
| cruciform_x_dec | 26.03585 | 0.03459 | 26.00000 | +1.04 |
| frustum_oblique | 17.90596 | 0.03863 | 17.95570 | -1.29 |
| frustum_cylinder_down | 14.12952 | 0.02188 | 14.13717 | -0.35 |
| frustum_z_expand | 21.97750 | 0.04141 | 21.99115 | -0.33 |
| frustum_x | 6.81125 | 0.01675 | 6.80678 | +0.27 |
| vessel_z | 71.30812 | 0.09102 | 71.20943 | +1.08 |
| vessel_x | 25.91239 | 0.04011 | 25.91814 | -0.14 |
| vessel_y | 16.76858 | 0.02784 | 16.75516 | +0.48 |
| polygon_xy_arrow | 25.01006 | 0.03428 | 25.00000 | +0.29 |
| polygon_yz_arrow | 24.96470 | 0.03427 | 25.00000 | -1.03 |
| polygon_xz_l_cw | 6.00856 | 0.00849 | 6.00000 | +1.01 |
| polygon_rz | 59.75864 | 0.09023 | 59.82116 | -0.69 |

**Interpretation.** The expansions reproduce OpenMC's own surfaces bit for bit and its
point classification exactly on these cases, and the regions have the volumes the
geometry says they should, so the expansions are neither mis-signed nor leaky. What this
does **not** show: transport through composites (distance to boundary on the complex
regions is the existing `Cell::distance_to_boundary`, not re-tested here); behaviour of
points within 1e-9 of a surface (none were sampled); `Polygon` pieces matching upstream's
(they are not expected to).

## Images (`images/`)

Two slices per case, `<case>_across.png` and `<case>_along.png`, inside in orange, outside
in blue, cm axes. Regenerate with
`OUTRAM_PLOT_OUT=<this dir>/images cargo test --release -p outram-mc-libs --test composite_surfaces_vs_openmc draw_every_composite`.

Checked by eye on 2026-11-12: `vessel_z_along` (cylinder with two elliptical heads, bottom
1 cm and top 1.5 cm deep), `polygon_rz_along` (the `(r, z)` outline mirrored about the
axis, hole along the axis), `sector_z_reflex_across` (280-degree annulus, gap from 250 to
330 degrees), `polygon_xy_arrow_across` (concave arrow), `rect_x_round_across` (rounded
corners), `cruciform_z_inc_across` (stepped cross), `octagon_y_across` (octagon in x-z),
`frustum_oblique_along`. **Not checked:** the `frustum_oblique` "along" slice is the plane
`y = const` through the centroid, which does not contain the oblique axis, so it shows an
oblique section rather than the trapezoid; the remaining images were generated and
machine-checked (both materials present) but not looked at individually.
