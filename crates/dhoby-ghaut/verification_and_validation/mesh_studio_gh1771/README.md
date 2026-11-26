# Mesh Studio review images (gh:#1771)

> **Research, education and V&V only.** Not for reactor design, licensing,
> operation or safety decisions. AI-assisted draft: untrusted until a human
> has reviewed it.

These are the drawing rule's images for Mesh Studio
(`crates/dhoby-ghaut/examples/mesh_studio/`, published as
`demos/mesh-studio/`). Mesh Studio runs the cfMesh port's tet → dual →
layers pipeline (`outram-park-fork-cfmesh`) and shows a cut through every
stage, coloured by boundary-layer index or cell quality.

## Methodology

- **What is drawn.** Each PNG is a screenshot of the real browser page (the
  wasm build, with the mesh built in its Web Worker). The page draws the
  **assembled** `VolumeMesh` the pipeline returned, not the shape's
  constants. The chain is: `VolumeMesh` → `outram_blender` neutral mesh →
  `PolyMesh` → `outram_foam_mesh::section::plane_section`, one polygon per
  cut cell.
- **Cell order.** The polygons keep the pipeline's cell order, so a
  per-cell value lands on the right polygon. This is checked by
  `section::tests::the_conversion_keeps_the_cell_order`: per-cell vertex
  means agree within 1e-12 m on the finished box mesh.
- **Colours.**
  - *Boundary layer*: 0 for a core cell, k for the k-th layer out from the
    wall (`pipeline::staged::boundary_layer_index`).
  - *Non-orthogonality* \[deg\] and *skewness*: each cell's worst internal
    face (`checks::cell_quality`). Their maxima equal `checkMesh`'s report
    bit for bit (`checks::tests::cell_quality_maxima_equal_the_report`).
- **Polygons of non-convex cells.** The polygon of a non-convex dual cell is
  the convex hull of its true section. Neighbouring polygons can therefore
  overlap slightly. This affects the drawing only; no number below comes
  from the polygons.
- **Inputs.** The defaults for each shape: size 2 m, 3 layers, first layer
  0.020 m, growth 1.3, back-off strategy.
  - Pipe and fuel channel: cell-centre dual with `Corners` feature points.
  - Box: per-corner median dual.
  - The shapes have illustrative proportions, not any particular reactor's:
    - pipe: radius 1 m, length 6 m;
    - fuel channel: pin radius 0.5 m inside a channel of radius 1 m, height
      4 m;
    - pebble: radius 0.6 m, at mid-height of a channel of radius 1 m, height
      3 m.
- **How to regenerate.** Run `render.mjs`; the commands are in its header.
  It prints each page title (shape and cell count), so a re-run can be
  compared with the table below.

## Results (2026-11-26, branch `claude/outram-blender-mggk7u`)

| image | shape · cut · colour | cells | what to look at |
|---|---|---|---|
| `pipe_cross_layers.png` | pipe · z · layer | 11857 | whole cross-section; layers are too thin to see at this zoom (see below) |
| `pipe_wall_layers_zoom.png` | pipe · z · layer · 30× at the wall | 11857 | three layer bands (1 green, 2 orange, 3 pink) against the wall |
| `pipe_axial_layers.png` | pipe · x · layer | 11857 | the bands run the full length; inlet and outlet carry none |
| `pipe_cross_nonorth.png` | pipe · z · non-orthogonality | 11857 | worst cells at the wall |
| `fuel_channel_cross_layers.png` | fuel channel · z · layer | 22204 | the annulus between the pin and the channel |
| `fuel_channel_pin_wall_layers_zoom.png` | fuel channel · z · layer · 20× at the pin | 22204 | layers on the pin surface, growing outward into the coolant |
| `fuel_channel_outer_wall_layers_zoom.png` | fuel channel · z · layer · 20× at the channel wall | 22204 | layers on the channel wall, growing inward |
| `fuel_channel_axial_layers.png` | fuel channel · x · layer | 22204 | both walls layered along the height |
| `fuel_channel_growth_1_of_3.png` / `_zoom.png` | fuel channel · growth slider at 1 | 22204 | layer 1 coloured; layers 2 and 3 grey (not yet grown) |
| `fuel_channel_stage_1_carve.png` | stage 1, carve | 1920 at this stage | hexahedral carve before snapping |
| `fuel_channel_stage_3_tets.png` | stage 3, tetrahedralise | 46080 at this stage | tetrahedra with their quality |
| `fuel_channel_stage_5_dual.png` | stage 5, polyhedral dual | (in its report) | polyhedra replace the tets |
| `pebble_axial_nonorth.png` | pebble · x · non-orthogonality | 10412 | the sphere hole; worst cells around it |
| `pebble_cross_skew.png` | pebble · z · skewness | 10412 | |
| `box_cross_layers.png` | box · z · layer | 3885 | layers on the four faces the cut crosses |

### Layer counts and thicknesses, read from each page's report

| shape | core | layer 1 / 2 / 3 | total thickness per column, least / median / most | asked for |
|---|---|---|---|---|
| pipe | 7153 | 1568 / 1568 / 1568 | 3.54 / 7.50 / 11.52 mm | 79.80 mm |
| fuel channel | 11524 | 3567 / 3584 / 3529 | 0.00 / 5.00 / 7.91 mm | 79.80 mm |
| pebble | 10412 | none | n/a | 79.80 mm |
| box | 429 | 1152 / 1152 / 1152 | 41.67 / 65.88 / 79.80 mm | 79.80 mm |

### Interpretation

- **The layers on the reactor shapes are about 10–20× thinner than asked
  for.** Back-off shrinks every column until the quality limits hold. On the
  pipe it kept a median of 7.5 mm of the 79.8 mm asked; on the fuel channel,
  5.0 mm.
  - Some fuel-channel columns that count a layer have zero total thickness
    ("least 0.00 mm"). These are collapsed columns, not missing ones.
  - The report line "Wall layers total …" was added because the first
    review images did not show this.
- **On the pebble, back-off grows no layer at all.** The page's stage notes
  say why (it backed off to zero thickness). They also say that feature
  capture left 1312 boundary points unmapped, because `correct_edges` is
  off.
  - The cfMesh optimiser strategy does grow the pebble's layers: 2354 cells
    per layer, measured natively at 77 s. The panel's strategy note says
    this.
- **Defect found in the first round of these images (fixed, not hidden).**
  The layers were invisible in every whole-mesh view.
  - Checked first: the cut data was right. The fuel-channel z-cut has 700
    polygons with layer counts [268, 144, 144, 144]. Drawn outside the app
    without outlines, the bands were there.
  - Cause: the bands are 1–2 px wide at the fitted zoom. The 0.6 px black
    outline on each side of every cell covered them.
  - Fix: a cell narrower than 5 px on screen gets no outline
    (`app::min_screen_width`, tested). The zoomed views were added, and the
    page now takes `?at=x,y&zoom=f` so they can be reproduced.
  - The axial views were also centred on the origin rather than on the mesh,
    which cut off half of each one. The view now centres on the mesh in
    every cut plane (`app::in_plane_centre`, tested).

## What was checked, and what was not

**Checked, by looking at the images:**

- On the pipe and both fuel-channel walls:
  - layers sit against the walls and in order 1 → 2 → 3 outward from the
    wall;
  - none on inlet or outlet;
  - no core cell between a layer and its wall.
- The growth slider greys exactly the layers above its value.
- The stage views change as the pipeline does: carve hexahedra, then tets,
  then polyhedra.
- The pebble hole is round and centred.
- The box's layers line the four faces its z-cut crosses.

**Not checked:**

- Any cut plane other than the default position (0.55 of the extent).
- The y-cuts.
- The cfMesh-optimiser and snappy strategies in the browser; their numbers
  come from native runs.
- Whether 5–8 mm layers are adequate for any flow. These shapes are not
  models of a real component.
- The fidelity of the non-convex dual polygons. They are hulls, as stated
  above.
- The workbench HTR-10 Step 7 meshes (gh:#1769) are not in Mesh Studio yet.

**Known failing test, pre-existing and not caused by this work:**
`tests::the_headless_sweep_matches_the_committed_fixture` (gh:#1772).
Untouched HEAD gives the same sweep byte for byte. It differs from the
fixture because skewness was redefined in gh:#1202 and because the
subdivided-cube dual is now refused. The fixture has not been re-pinned.
