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

## Per-region meshes joined by `cyclicAMI` (2026-11-27, the default)

**Maintainer, 2026-11-27:** "since single region works, cut these
multiregion meshes into individual ones, then mesh each region separately,
then combine them into a multiregion mesh using a AMI interface" (gh:#1858).
`--boundaries ami` is now the default; the conforming route below is
`--boundaries fitted`, kept as the ablation.

### Method

```text
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-cht-mesh \
    --cell 10 --out <case>                          # per region + cyclicAMI (default)
```

1. **One surface per region.** `RegionMap::rz_partition` cuts the R-Z map
   into one-region polygons (the conus slope split along it).
   `meshing::fitted_surface`, keeping one region, revolves that region's
   boundary with 32 segments. An edge to another region `h` becomes the
   patch `<region>_to_<h>`; the outer edges become `top`, `bottom` and
   `side`. Every run is wound with its normal away from the region, so an
   enclosed shell (the solid graphite round the bed and plenums) has
   negative volume and gets a TetGen hole point.
2. **Each region meshed on its own**: TetGen `-pq1.414/3.5a<V>A`, `V` the
   regular tetrahedron of 10 cm, then the polyhedral dual re-wound from the
   tetrahedra (`pipeline::multi_region` with one region). A region that
   grows layers goes through the single-region pipeline, which grows them
   first. A region left as tetrahedra is refused (the pipeline's own dual
   had kept 949 570 tetrahedra on the solid, found by its cell kinds).
3. **Each region snapped** to the true geometry: the morph and face snap of
   step 6 below, then the rims (`snap_rims_to_revolved`, points on an edge
   between two patches go onto their ring) and `split_rim_chords` (a rim
   chord wider than half a facet split at its angular midpoint in every face
   using it), so both sides of a flat interface carry the same rim points.
   Faces are classified by their own segment's normal, so the conus cone is
   snapped too (it was not, on any route, before 2026-11-27).
4. **Joined** (`outram_park_fork_cfmesh::pipeline::region_join`): points and
   cells concatenated, each interface kept as two patches and written as a
   `cyclicAMI` pair with each other as `neighbourPatch` (OpenFOAM ESI's
   convention).
5. **AMI weights** on every pair: `outram_foam_basic_lib::mesh::face_area_weight_ami`,
   a port of ESI's `faceAreaWeightAMI` bit-identical to the upstream library
   on 17 cases (gh:#1859).

**Pass criteria** (`the_ami_cht_mesh_joins_snapped_regions`, 40 cm, 16
segments): every region has cells and is within 1 % of its analytic round
volume; no cell is a tetrahedron; the outer patches are `top`, `bottom`,
`side`; every interface is a pair named after its two regions and written
as `cyclicAMI`; the polyMesh and cellZones read back.

### Results (2026-11-27, 10 cm, 32 segments)

Same container as below; built from `b73f1ad43`. Report: `report_ami.md`.

| | per region + AMI | conforming (`--boundaries fitted`, 2026-11-26) |
|---|---|---|
| cells (all polyhedra) | 184 204 | 185 798 |
| wall time | 72 s | 50–53 s |
| max non-orthogonality | **71.0°** | 86.4° |
| max skewness | **2.50** | 12.5 |

| region | per region + AMI | conforming, snapped |
|---|---|---|
| `porous_bed` | **−0.03 %** | −0.02 % |
| `fluid_upper_plenum` | **−0.03 %** | −0.03 % |
| `fluid_lower_plenum` | **+0.00 %** | +0.01 % |
| `solid_graphite` | **−0.00 %** | −0.00 % |

The region volumes are as good as the conforming route's, and the quality
is much better: the conforming route's worst cells were the interface
cells at the corner ring where the bed's bottom edge meets r = 90 cm, and
here no cell sits on an interface. The worst cell is now at the outer
wall (r 188 cm, z 92 cm).

**The interfaces** (side areas, and AMI weight sums min / mean / max):

| pair | b / a − 1 | source sums | target sums |
|---|---|---|---|
| `porous_bed` / `fluid_upper_plenum` (flat) | +4.4e-16 | 0.9907 / 0.999977 / 1.0000 | 0.9917 / 0.999983 / 1.0000 |
| `porous_bed` / `solid_graphite` | +1.75e-5 | 0.9999 / 1.000057 / 1.0004 | 0.9901 / 1.000022 / 1.0054 |
| `fluid_upper_plenum` / `solid_graphite` | +2.60e-6 | 0.9906 / 1.000016 / 1.0083 | 0.9907 / 1.000030 / 1.0003 |
| `fluid_lower_plenum` / `solid_graphite` | +8.14e-6 | 0.9852 / 1.000063 / 1.0244 | 0.9999 / 1.000048 / 1.0007 |

A weight sum above 1 is where curved facets overlap after the projection
upstream uses; below 1 is a face partly uncovered (at a rim). The means are
within 6.3e-5 of 1 on every side.

**Images** (`images/ami/`, drawn from the joined mesh). Checked: every
interface is a straight line in X-Z and a circle in X-Y; the cold-gas
plenum is one continuous band; the hot plenum is the annulus with the solid
column inside it; nothing is white inside the cylinder. Not checked:
anything off the drawn planes, and the AMI coupling itself (the images show
cells, not weights).

**Not yet done:** the solver does not read the `cyclicAMI` pairs back as
coupled yet (gh:#1860): `read_poly_mesh` takes them as plain patches.

## The conforming route (`--boundaries fitted`, the default until 2026-11-27)

## Method

```text
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-cht-mesh \
    --cell 10 --out <case>                          # fitted (default)
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-cht-mesh \
    --cell 10 --boundaries centroid --out <case>    # the earlier route
```

The fitted route takes five steps.

1. **R-Z tiling** (~~`RegionMap::rz_tiling`~~ `RegionMap::rz_partition`
   since 2026-11-26, which also splits the rectangles the conus slope
   crosses, so the Basic map is no longer refused). The tensor grid of every `r` and
   `z` at which a part or a box begins or ends. Each rectangle gets the
   region at its centre, and is refused if a probe at any of its four
   corners (inset by 1e-4 of the cell) finds another region or none. So the
   region boundaries are grid edges, checked rather than assumed. ~~The
   Basic map, whose conus and conus graphite are different regions across
   the slope, is refused, as it must be.~~ (CORRECTED 2026-11-26: the slope
   is now split, see above.)
2. **Multi-region surface** (~~`meshing::fitted_cht_surface`~~
   `meshing::fitted_surface` since 2026-11-26). Every grid edge
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

6. **Snap to the true geometry** (maintainer, 2026-11-26: "snap
   properly"; default on, `--no-snap` is the ablation). The surface reached
   TetGen as 32 flat facets per cylinder, so every region was short by the
   32-gon's −0.64 %. Two steps, in `meshing.rs`. `morph_to_revolved` moves
   every point within one cell size of an interface or outer cylinder
   radially, by a share of the facet's chord-to-circle move: the full move
   on the facet, falling linearly to none one cell away (and beyond the
   cylinder's ends). `snap_faces_to_revolved` then takes every surface face
   whose normal is mostly radial (outer patches, and internal faces between
   two regions), finds its cylinder from its radius and height, and puts all
   its points exactly on the true radius. Disc faces keep their points. A
   cell with non-positive volume is refused. Snapping TetGen's own vertices
   before the dual was tried and rejected: the cells away from the
   interfaces went from 63.8° to 87.4° and skewness 1.3 to 14.9.

**Pass criteria.** Unsnapped (`--no-snap`), each region's meshed volume
equals the exact volume of its **faceted** body (32-gon rings, `n/2 sin(2 pi/n)` in place of `pi`) to
1e-9 relative. Every cell's vertices lie in its own region's rectangles
(the inner radius taken at the facets' chord). The only patches are `top`,
`bottom` and `side`. The polyMesh and cellZones read back through the
GeN-Foam port. These are gated at 40 cm by
`the_fitted_cht_mesh_has_exact_region_volumes` (`src/bin/dhoby-ghaut/main.rs`).
Snapped (the default), every `side` point is on `r = 190 cm` to 1e-9 m, every
cell has positive volume, and every region is at least 4× closer to its
analytic round volume than the faceted body
(`the_snapped_cht_mesh_is_on_the_true_cylinders`, 40 cm, 16 segments),
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

**The reference volume.** ~~"Exact R-Z volume" was `exact_in`'s 0.5 cm
midpoint quadrature.~~ **CORRECTED 2026-11-26:** that quadrature is off by
up to about 0.4 % where a region edge is not on its grid (the bed's top
and bottom). The report now uses the R-Z tiling's analytic volume (each
rectangle revolved exactly). Against it, the unsnapped fitted mesh is short
by exactly the 32-gon's −0.64 % in every region.

**Region volumes against the analytic round volume** (the numbers to
quote; the snapped mesh is the default):

| region | fitted, snapped | fitted, `--no-snap` |
|---|---|---|
| `porous_bed` | **−0.02 %** | −0.64 % |
| `fluid_upper_plenum` | **−0.03 %** | −0.64 % |
| `fluid_lower_plenum` | **+0.01 %** | −0.64 % |
| `solid_graphite` | **−0.00 %** | −0.64 % |

Quality, snapped: max non-orthogonality 86.4°, max skewness 12.5. Both are
at one corner ring (r ≈ 89 cm, z ≈ −63 cm), where the bed's bottom edge
meets the r = 90 interface. The interface ends there, so the morph has to
fade below it. Unsnapped, that same cell is the worst too, at 83.6° and
6.36; with the surface snapped but no morph (`DHOBY_SNAP_MORPH=0`) it is
88.7° and 32.9. The cells away from the interfaces reach 63.8° and 1.42 in
every variant.

Unsnapped, against the exact faceted volume (what that surface encloses):

| region | fitted | centroid |
|---|---|---|
| `porous_bed` | −3.3e-15 | +9.6e-3 |
| `fluid_upper_plenum` | −6.5e-15 | +2.7e-3 |
| `fluid_lower_plenum` | −7.8e-16 | +2.5e-4 |
| `solid_graphite` | +9.2e-15 | −6.0e-4 |

Against the exact **round** R-Z volume, the unsnapped fitted mesh is short
by ~~the faceting of each region's own radii (bed −0.98 %, upper plenum
−0.42 %, lower plenum −0.31 %, solid −0.64 %)~~ **−0.64 % in every
region** (CORRECTED 2026-11-26: the spread was the quadrature reference's
error, not faceting). The centroid route's bed agreement with the
quadrature reference is a coincidence of errors and should not be quoted.

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
| `report_ami.md` (per region + `cyclicAMI`, the default since 2026-11-27) | the command's own report |
| `report_fitted.md` (snapped, ~~default~~ the conforming ablation), `report_fitted_no_snap.md`, `report_centroid.md` | the command's own reports |
| `images/ami/*.png` | the per-region route's slices |
| `images/fitted/*.png`, `images/centroid/*.png` | the slices above |
