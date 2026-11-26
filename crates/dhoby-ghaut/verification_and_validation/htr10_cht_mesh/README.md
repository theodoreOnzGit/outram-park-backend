# HTR-10 conjugate mesh: body-fitted region boundaries (TENTATIVE)

**Research, education and V&V only.** This is a verification record of a
mesh built on the TENTATIVE `nee_soon::htr10_rmc` HTR-10 R-Z map. Nothing
here is validated. Never use it for an operational purpose.

**Untrusted AI-assisted draft pending human review.**

## What was asked

One mesh of the whole HTR-10 (maintainer, 2026-11-26) holding **solid
graphite**, a **fluid upper plenum** and a **fluid lower plenum**, with a
**porous pebble bed** in the middle. It is to be built tet -> polyhedral dual
with TetGen tetrahedra, and the **internal boundaries are to be fitted**:
every region interface must be a set of mesh faces, never a stair-step.

Regions are `RegionMap::three_class_cht` (`src/workbench/meshes.rs`):

| region | class | made of |
|---|---|---|
| `porous_bed` | porous (pebble bed) | the bed above the conus |
| `fluid_upper_plenum` | fluid | the cavity above the bed + the cold-gas plenum (TECDOC-1382 Fig. 4.10 zone 3) |
| `fluid_lower_plenum` | fluid | the hot-gas plenum (zones 14, 15) |
| `solid_graphite` | solid | everything else inside r = 190 cm: the conus, the discharge tube and the channel bands |

The plenum zones still hold graphite in Fig. 4.10 (16 % of zone 3, and 61 %
and 38 % of zones 14 and 15 by carbon density). Here they are taken as all
gas. That is an idealisation, stated in the code.

## Method

```text
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-cht-mesh \
    --cell 10 --out <case>                          # fitted (default)
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-cht-mesh \
    --cell 10 --boundaries centroid --out <case>    # the earlier route
```

The fitted route takes five steps.

1. **R-Z tiling** (`RegionMap::rz_tiling`). The tensor grid of every `r` and
   `z` at which a part or a box begins or ends. Each rectangle gets the
   region at its centre, and is refused if a probe at any of its four
   corners (inset by 1e-4 of the cell) finds another region or none. So the
   region boundaries are grid edges, checked rather than assumed. The
   Basic map, whose conus and conus graphite are different regions across
   the slope, is refused, as it must be.
2. **Multi-region surface** (`meshing::fitted_cht_surface`). Every grid edge
   between two regions, and the outer top, bottom and side, is revolved with
   32 segments. Collinear edges with the same two regions are merged first,
   where nothing else meets them. Each interface is a patch marker
   `<a>_to_<b>`. Each connected component of each region gets one seed
   point.
3. **TetGen with region attributes**
   (`outram_park_fork_cfmesh::pipeline::multi_region`, NOT a port). The
   switches are `-pq1.414/3.5a<V>A`, with `V = h^3/(6 sqrt 2)` at
   h = 10 cm (Mesh Studio's defaults). TetGen recovers every interface
   triangle as a subface, so no tetrahedron crosses one, and `-A` gives
   each tetrahedron its region.
4. **Dual per region, then stitch.** Each region's tetrahedra are dualled
   on their own (`polyhedral_dual_min_faces`, falling back to
   `polyhedral_dual`). A vertex on an interface therefore gets one dual
   cell on each side. Both sides tile the interface with the per-corner
   quads of the same primal triangles. Those faces are paired by their
   points (merged within 1e-9 of the bounding-box diagonal) and become
   internal faces. An unpaired interface face is an error, never dropped.
5. **Re-winding from the primal geometry** (`rewind_dual`). The existing
   dual winds each face from its cell's vertex average. That left a cell
   open at a re-entrant edge, found by the nested-cube test. Here it is
   replaced by two exact rules. An internal face's pieces lie in the planes
   `lambda_o = lambda_n` of their tetrahedra, so the face is wound from cell
   `o` towards `n`. A boundary face lies in one primal boundary triangle,
   and takes that triangle's winding. Every cell must then be closed
   (`validate`) and have **positive volume** (a new gate: a cell wound
   inside out is still closed).

**Pass criteria.** Each region's meshed volume equals the exact volume of
its **faceted** body (32-gon rings, `n/2 sin(2 pi/n)` in place of `pi`) to
1e-9 relative. Every cell's vertices lie in its own region's rectangles
(the inner radius taken at the facets' chord). The only patches are `top`,
`bottom` and `side`. The polyMesh and cellZones read back through the
GeN-Foam port. These are gated at 40 cm by
`the_fitted_cht_mesh_has_exact_region_volumes` (`src/bin/dhoby-ghaut/main.rs`),
and the cfMesh module by four tests in `pipeline/multi_region.rs` (two
cubes side by side, a cube inside a cube, both routes, refusals).

## Results (2026-11-26, 10 cm, 32 segments)

Hardware: cloud container, 4 vCPU Intel Xeon @ 2.10 GHz, 15 GB RAM. One
thread (the TetGen port and the dual are serial). Built from `d3451321`
plus this change.

| | fitted | centroid (earlier) |
|---|---|---|
| cells (all polyhedra) | 185 798 | 177 789 |
| TetGen tetrahedra | 1 053 587 | 1 046 314 |
| wall time | 50–53 s | 58–67 s |
| interface faces | 41 490 | none (cells straddle) |
| max non-orthogonality | 83.6° | 69.0° |
| max skewness | 6.36 | 2.54 |
| cells not star-shaped about their centroid | 8 167 | 6 282 |

**Region volumes**, meshed against the exact faceted volume (the number to
quote, since it is what the surface encloses):

| region | fitted | centroid |
|---|---|---|
| `porous_bed` | −3.3e-15 | +9.6e-3 |
| `fluid_upper_plenum` | −6.5e-15 | +2.7e-3 |
| `fluid_lower_plenum` | −7.8e-16 | +2.5e-4 |
| `solid_graphite` | +9.2e-15 | −6.0e-4 |

Against the exact **round** R-Z volume, the fitted mesh is short by the
faceting of each region's own radii (bed −0.98 %, upper plenum −0.42 %,
lower plenum −0.31 %, solid −0.64 %). The centroid route's bed reads
−0.03 % only because its +0.96 % stair-step cancels the −0.98 % faceting.
That agreement is a coincidence of two errors and should not be quoted.

**Quality, by place** (fitted): the 13 840 cells on an interface carry the
worst values (83.6°, 6.36). The other 171 958 cells reach 63.8° and 1.41,
better than the whole centroid mesh. The cause is known. An interface face
is made of per-corner boundary quads (`DualKind::MedianPerCorner`), which
`AGENTS.md` (gh:#1215) records at a median 69° to the owner on a wall.
`MedianPerPoint` or `CellCentre` on the interfaces would be the next step;
the stitch would need those faces to match from both sides, which is not
yet checked.

## Images (`images/`)

Drawn from the assembled mesh (`render_mesh_slice`, cells filled by zone).
`fitted/` and `centroid/` each hold `cht_regions_rz` (the region map
itself), `cht_mesh_xz` (through the axis), and `cht_mesh_xy_bed`,
`cht_mesh_xy_upper_plenum` and `cht_mesh_xy_lower_plenum` (X-Y at the
middle of each non-solid region).

**Checked in them (fitted):**

- Every interface is a straight line in X-Z and a clean circle in X-Y.
- The cold-gas plenum is one continuous band between the top reflector
  blocks. On the centroid mesh it is 1–2 cells thick and broken.
- The hot plenum is the annulus r 41.75–90 cm with zone 12's solid column
  inside it.
- Nothing is left white inside the cylinder.

**Not checked in them:** the conus (all solid here, so there is nothing to
see), and anything off the drawn planes.

**A plotter defect found and fixed on the way.** The first fitted images
had white cells beside the interfaces. A probe found each white point
inside exactly one cell, and every cell had positive volume, so the mesh
was complete. The fault was in `outram_blender::unstructured::plot::plane_cut`.
It pushed a vertex lying on the cut plane as well as the crossings, so a
triangle got three points and the even-odd fill paired the wrong
crossings. The cut planes here hit mesh vertices exactly: y = 0 holds the
facet rings' angle-0 vertices, and the X-Y planes are box mid-heights,
where TetGen splits the interface segments. An on-plane vertex now counts
as above the plane (pinned by `a_cut_through_vertices_fills_every_cell`).

## Files

| file | what |
|---|---|
| `report_fitted.md`, `report_centroid.md` | the command's own reports |
| `images/fitted/*.png`, `images/centroid/*.png` | the slices above |
