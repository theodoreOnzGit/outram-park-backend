# Meshing nexus: drawings of the generated meshes (GitHub #492)

Research, education and V&V only. These images are AI-generated draft
material until a human has looked at them (`RESPONSIBLE_USE.md`).

The geometry-drawing HARD RULE (`crates/outram-blender/CLAUDE.md`) applies to
every mesh a solver will integrate or transport through. These six images are
the first drawings of meshes produced through the neutral
`outram_blender::unstructured::UnstructuredMesh`. Each one was drawn from the
neutral mesh after conversion, not from the mesher's own output.

Regenerate them with:

```bash
cargo run --release -p outram-blender --example meshing_nexus_images \
    --features "block-mesh foam-mesh fem-export"
```

Source: `crates/outram-blender/examples/meshing_nexus_images.rs`. Generated
2026-10-03, in the commit that added this file.

## How the images are drawn

`unstructured::plot::render_mesh_slice` cuts the mesh with an axis-aligned
plane. Each cell's **bounding fan triangles** are the definition of the cell
that Monte Carlo point location also uses. The plane section of those
triangles is filled by even-odd scanline in the colour of the cell's zone.
Every face the plane crosses is drawn as a black line. Axes are in cm (the
mesh is scaled from its own unit). Zones were assigned by the example from
cell centres, so the colour boundary is a staircase of whole cells.

## The images

| file | mesher (feature) | what it shows | cells | volume (neutral) |
|---|---|---|---|---|
| `fv_cfmesh_cylinder_xy.png` | cfMesh tet -> dual -> layers (`foam-mesh`) | x-y slice at z = 52 cm of a cylinder r = 0.5 m, h = 1 m, 32-segment surface, cell size 0.1 m | 24751 | 0.7764105 m^3 |
| `fv_cfmesh_cylinder_xz.png` | same | x-z slice at y = 1.3 cm | 24751 | same |
| `fv_blockmesh_box_xz.png` | blockMesh (`block-mesh`) | two blocks 0.4 m and 0.2 m long, x-grading 2 and 0.5, y = 3 cm | 144 | 0.012 m^3 |
| `fe_farrer_quarter_annulus.png` | farrer-park `quarter_annulus_quad4` (`fem-export`) | 2-D, r in [0.1, 0.2] m, 6 x 12 Quad4 | 72 | 0.02349471 m^2 |
| `fe_farrer_box_tet4_xz.png` | farrer-park `box_tet4` (`fem-export`) | 0.3 x 0.2 x 0.2 m box, 3 x 2 x 2 hexes, 6 Kuhn tets each, y = 4.3 cm | 72 | 0.012 m^3 |
| `one_d_column_xz.png` | the 1-D mesher (core) | 10 Hex8 over 1 m, 0.01 m^2 cross-section, y = 0.1 cm | 10 | 0.01 m^3 |

Cut planes were placed off the mesh's own grid planes on purpose. A cut that
lies exactly in a face plane is degenerate for a section drawing.

## What was checked in the images (by eye, 2026-10-03)

- **cfMesh cylinder.** The x-y section is a closed disc of radius 50 cm with
  no unfilled cell. The dual cells show as the expected polygons, with small
  faces at the corners of the background octree. The x-z section fills the
  0..100 cm height and +-50 cm width with no gap. The inner zone
  (r < 0.3 m by cell centre) is a staircase about r = 30 cm, as expected for
  a whole-cell assignment.
- **blockMesh.** 8 cells in block 1 grow along x (grading 2) and 4 cells in
  block 2 shrink (grading 0.5). There are 6 rows in z, and the shared face at
  x = 40 cm is conforming.
- **Quarter annulus.** 6 radial x 12 circumferential quads between r = 10 and
  20 cm. The straight-sided quads under-fill the arc, so the neutral area
  0.02349471 m^2 is below pi (0.2^2 - 0.1^2) / 4 = 0.02356194 m^2, by 0.29 %.
- **Box tet4.** The section of the six-tet Kuhn subdivision shows the
  expected diagonal traces in each structured cell, and the box edges are at
  0/30 cm and 0/20 cm.
- **1-D column.** 10 equal cells, 1 m long. The section is 10 cm high, which
  is the square side sqrt(0.01 m^2).

## Numbers printed by the run (not tests)

- cfMesh's own report gives total volume 0.776410 m^3 and the neutral mesh
  gives 0.7764105 m^3. The two agree, so the polyMesh-to-neutral geometry
  reproduces cfMesh's.
- The cfMesh volume sits below the faceted 32-gon prism (0.78036 m^3), which
  is consistent with snapping to the surface. This was not investigated
  further.
- **264 of the 24751 cfMesh dual cells (1.1 %) are not star-shaped about
  their centroid.** Their centroid tetrahedral decomposition contains an
  inverted tetrahedron. This was found on the first run, which originally
  rejected such meshes. The neutral mesh now defines a cell by its bounding
  fan triangles (a winding-number inside test) and keeps the decomposition
  only for sampling, with rejection sampling for those 264 cells. The
  blockMesh, farrer-park and 1-D meshes have none.

## What could not be checked

- **The unit tests of the neutral mesh, the converters and the Monte Carlo
  scoring: NOT YET MEASURED.** Testing was deferred by the maintainer on
  2026-10-03, and the tests are written but have not been run.
- No 3-D view and no boundary-patch colouring. The images show cells and
  zones; patches are listed in the API only.
- The cfMesh boundary-layer prisms are about 1 cm thick and are barely
  visible at this resolution. A zoomed slice is needed to inspect them.
