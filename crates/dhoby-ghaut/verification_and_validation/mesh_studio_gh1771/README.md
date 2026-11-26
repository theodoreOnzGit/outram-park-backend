# Mesh Studio review images (gh:#1771, gh:#1796, gh:#1825)

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
- **Tet source (gh:#1796).** Since 2026-11-26 the page meshes **TetGen's
  tetrahedra** by default (`-pq1.414`, `-a` the regular tetrahedron of the
  cell size, `h³/(6√2)`); `?tets=carved` asks for the structured
  carve-and-split route. Each page title names the source.
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

## gh:#1796: TetGen's tetrahedra, every mesh type (2026-11-26)

### The measurement table (native, release)

`mesh_studio --measure --shape <s> --tets <src> --mesh tet|poly --n 0|3
--strategy backoff|cfmesh` (`headless::measure`), the page's defaults
otherwise: cell-centre dual with corner feature points, 3 layers asked
(first 20 mm, ×1.3, 79.8 mm in all). "Layers" is the deepest layer grown
and the median grown column over the asked thickness. Times are wall
clock for the whole build; the `CfMeshOptimise` and dual-kind rows ran up
to four at a time on four cores, so they are upper bounds. Every row is
valid (the pipeline refuses an invalid stage) with **0 negative cells**.

| shape | source | mesh | cells | max non-orth | max skew | layers | time |
|---|---|---|---|---|---|---|---|
| pipe | TetGen | tet | 18020 | 62.6° | 0.99 | n/a | 0.3 s |
| pipe | TetGen | tet + BL, back-off | 18020 | 62.6° | 0.99 | **none** | 0.9 s |
| pipe | TetGen | tet + BL, cfMesh | 18020 | 62.6° | 0.99 | **none** (stage refused) | 117 s |
| pipe | TetGen | tet + BL, **tetMesh** (gh:#1824) | 24353 | 65.8° | 0.90 | 3, 117 % (2111 per layer) | 1.3 s |
| pipe | TetGen | dual | 3562 | 49.5° | 1.37 | n/a | 0.4 s |
| pipe | TetGen | dual + BL, back-off | 6826 | 69.9° | 2.01 | 3, 17 % | 0.5 s |
| pipe | TetGen | dual + BL, cfMesh | 6826 | 54.7° | 2.20 | 3, 75 % | 22 s |
| pipe | carved | tet | 29952 | 65.9° | 0.40 | n/a | 3.5 s |
| pipe | carved | tet + BL, back-off | 39168 | 72.7° | 0.92 | 3, 22 % | 3.5 s |
| pipe | carved | tet + BL, cfMesh | 39168 | 63.8° | 1.08 | 3, 119 % | 17 s |
| pipe | carved | dual | 7153 | 59.3° | 0.94 | n/a | 3.3 s |
| pipe | carved | dual + BL, back-off | 11857 | 66.7° | 2.05 | 3, 9 % | 3.5 s |
| pipe | carved | dual + BL, cfMesh | 11857 | 52.1° | 2.40 | 3, 55 % | 49 s |
| fuel channel | TetGen | tet | 30804 | 67.7° | 1.00 | n/a | 0.5 s |
| fuel channel | TetGen | tet + BL, back-off | 30804 | 67.7° | 1.00 | **none** | 1.9 s |
| fuel channel | TetGen | tet + BL, cfMesh | 30804 | 67.7° | 1.00 | **none** (stage refused) | 451 s |
| fuel channel | TetGen | tet + BL, **tetMesh** (gh:#1824) | 46119 | 73.8° | 1.08 | 3, 65 % (5105 per layer) | 71 s |
| fuel channel | TetGen | dual | 6399 | 60.7° | 1.65 | n/a | 0.7 s |
| fuel channel | TetGen | dual + BL, back-off | 14313 | 74.5° | 2.94 | 3, 12 % | 1.1 s |
| fuel channel | TetGen | dual + BL, cfMesh | 14313 | 63.3° | 2.84 | 3, 45 % | 57 s |
| fuel channel | carved | tet | 46080 | 75.9° | 0.40 | n/a | 0.4 s |
| fuel channel | carved | tet + BL, back-off | 46080 | 75.9° | 0.40 | **none** | 1.6 s |
| fuel channel | carved | tet + BL, cfMesh | 66624 | 74.5° | 0.77 | 3, 85 % | 90 s |
| fuel channel | carved | dual | 11524 | 65.2° | 0.98 | n/a | 0.5 s |
| fuel channel | carved | dual + BL, back-off | 22204 | 84.8° | 2.08 | 3, 6 % | 1.3 s |
| fuel channel | carved | dual + BL, cfMesh | 22204 | 78.2° | 2.36 | 3, 38 % | 240 s |
| pebble | TetGen | tet | 35347 | 72.2° | 1.53 | n/a | 0.7 s |
| pebble | TetGen | tet + BL, back-off | 35347 | 72.2° | 1.53 | **none** | 3.4 s |
| pebble | TetGen | tet + BL, cfMesh | 35347 | 72.2° | 1.53 | **none** (stage refused) | 413 s |
| pebble | TetGen | tet + BL, **tetMesh** (gh:#1824) | 35347 | 72.2° | 1.53 | **none** (refused: 6 inverted cells) | 237 s |
| pebble | TetGen | dual | 7186 | 67.8° | 1.78 | n/a | 1.2 s |
| pebble | TetGen | dual + BL, back-off | 7186 | 67.8° | 1.78 | **none** | 2.7 s |
| pebble | TetGen | dual + BL, cfMesh | 15052 | 68.7° | 3.20 | 3, 31 % | 93 s |
| pebble | carved | tet | 43392 | 78.4° | 0.43 | n/a | 0.4 s |
| pebble | carved | tet + BL, back-off | 43392 | 78.4° | 0.43 | **none** | 1.4 s |
| pebble | carved | tet + BL, cfMesh | 56832 | 74.5° | 0.75 | 3, 83 % | 72 s |
| pebble | carved | dual | 10412 | 79.9° | 1.04 | n/a | 0.7 s |
| pebble | carved | dual + BL, back-off | 10412 | 79.9° | 1.04 | **none** | 2.1 s |
| pebble | carved | dual + BL, cfMesh | 17474 | 78.2° | 2.36 | 3, 37 % | 216 s |

(The carved pipe's tet + BL under cfMesh reports a median column 119 % of
the asked total: `CfMeshOptimise` does not cap a column at the asked
thickness. Not investigated here.)

**What it says.**

- ~~**No layer grows on TetGen's tetrahedra**, with either strategy, on any
  shape: back-off backs off to zero and `CfMeshOptimise`'s result is
  refused by the pipeline's gate. Some wall tetrahedra have all four
  vertices on the boundary (146 on the 1 cm test pipe, none on the carved
  route; `tet_source::tests::diagnose_boundary_tets_of_both_sources`).
  The report and the panel say so. **Use the dual for layers.**~~
  **CORRECTED 2026-11-26 (gh:#1824):** back-off and `CfMeshOptimise`
  still grow none there, but cfMesh's own `tetMesh` layer steps
  (`strategy=tetmesh`, `LayerQualityStrategy::CfMeshTetMesh`) grow all 3
  on every wall face of the **pipe** and the **fuel channel**, with no
  inverted cell (rows above). The **pebble** is still refused: 6 cells
  stay inverted. The all-boundary tetrahedra were not the cause (none of
  the cells the layer inverts is one); back-off is blocked by its
  whole-mesh `minDeterminant 0.001` check, which TetGen's bare tets fail.
  The methodology and the step-by-step measurement are in
  `outram-park-fork-cfmesh/src/pipeline/tet_layers.rs`.
- **The tetMesh route's thickness is upstream's** (half the local wall
  edge, thinned by `optimiseLayer` where a cell would invert), so the
  column medians are 117 % (pipe) and 65 % (fuel channel) of the 79.8 mm
  asked; the requested 20 mm only caps the first layer. Its time is mostly
  the port's `untangleBoundaryLayer`, which on the fuel channel would have
  inverted 1463 cells and is not kept (the module's one non-upstream rule).
- **On TetGen's dual** back-off grows thicker layers than on the carved
  dual (pipe 17 % against 9 %, fuel channel 12 % against 6 %) and
  `CfMeshOptimise` is 2–4× faster than on the carved dual and grows layers
  on every shape.
- **The pebble (the issue's question):** back-off still grows **no** layer
  on the pebble's TetGen dual. `CfMeshOptimise` grows all three (2622 per
  layer, 31 %).
- **20 layers** (`n=20&first=0.002&ratio=1.1`, 114.5 mm asked) on the
  pipe's TetGen dual: `CfMeshOptimise` grows all 20 on every wall face
  (25322 cells, 52 % of the asked thickness, 54.7°, 11 s natively, 41 s in
  the browser); back-off grows none (pinned in
  `engine::tests::twenty_layers_grow_on_the_pipe_with_the_cfmesh_optimiser`).

### The dual kind on TetGen's tetrahedra, chosen by measurement

| dual (feature points) | pipe dual + BL, back-off | fuel channel | pebble | cfMesh layers, time (pipe / fuel / pebble) |
|---|---|---|---|---|
| cell-centre (corners) | 6826, 69.9°, 3 layers, 17 % | 14313, 74.5°, 12 % | none | 75 / 45 / 31 %, 22 / 57 / 93 s |
| cell-centre (on feature edge) | same cells, 69.9°, skew 2.07 | same, skew 2.98 | none | not run |
| median per point (corners) | 6826, 67.3°, 17 % | 14313, 69.1°, 12 % | none | 62 / 40 / 40 %, 151 / 364 / 549 s |
| median per corner | 22561, 83.8°, 17 % | 52344, 84.8°, 8 % | 53248, 88.2°, 2 % | not run |

The **cell-centre dual with corner feature points** stays the reactor
shapes' choice: it ties on validity and layers grown with back-off, is
7–18× faster under `CfMeshOptimise` with equal or thicker layers, and has
a smaller skewness than the feature-edge rule. The per-point median dual
has 2.5–5.4° less non-orthogonality with back-off but is far slower under
`CfMeshOptimise`. The per-corner dual is the only one on which back-off
grows layers on the pebble, but with 4–6× the wall faces, 2 % of the
asked thickness and 80–88° non-orthogonality.

### Images (browser, 2026-11-26; cell counts from the page titles)

| image | view | cells |
|---|---|---|
| `pipe_tetgen_tets.png` | pipe · TetGen tets · z · non-orth · 3× | 18020 |
| `pipe_tetgen_tets_layers_zoom.png` | pipe · TetGen tets + 3 layers, **tetMesh route** (gh:#1824) · 30× at the wall | 24353 (~~18020, no layer~~); 1.65 s in the browser |
| `pipe_tetgen_dual.png` | pipe · TetGen dual · z · non-orth · 3× | 3562 |
| `pipe_tetgen_dual_layers_zoom.png` | pipe · TetGen dual + 3 layers (back-off) · 30× at the wall | 6826 |
| `pipe_tetgen_dual_20_layers_zoom.png` | pipe · TetGen dual + 20 layers (cfMesh) · 20× at the wall | 25322 |
| `fuel_channel_tetgen_tets.png` | fuel channel · TetGen tets · 1.5× | 30804 |
| `fuel_channel_tetgen_tets_layers_zoom.png` | fuel channel · TetGen tets + 3, **tetMesh route** · 20× at the pin | 46119 (~~30804, no layer~~); 107 s in the browser |
| `fuel_channel_tetgen_dual.png` | fuel channel · TetGen dual · 1.5× | 6399 |
| `fuel_channel_tetgen_dual_layers_zoom.png` | fuel channel · TetGen dual + 3 (back-off) · 20× at the pin | 14313 |
| `pebble_tetgen_tets.png` | pebble · TetGen tets · 1.5× | 35200 |
| `pebble_tetgen_tets_layers_zoom.png` | pebble · TetGen tets + 3, **tetMesh route** · 20× at the pebble | 35200 (no layer: refused, 12 cells inverted in the browser's mesh); 305 s in the browser |
| `pebble_tetgen_dual.png` | pebble · TetGen dual · 1.5× | 7158 |
| `pebble_tetgen_dual_layers_zoom.png` | pebble · TetGen dual + 3 (cfMesh) · 20× at the pebble | 14958 |
| `pair_pipe_dual_carved.png` / `pair_pipe_dual_tetgen.png` | pipe dual, carved / TetGen, same 3× view, non-orth | 7153 / 3562 |
| `phone_pebble_tetgen.png` | pebble at 390 × 844, controls folded | 7158 |

**The browser's TetGen pebble is not the native one** (35200 tetrahedra
and 7158 dual cells against 35347 and 7186; 2600 cfMesh cells per layer
against 2622): the port's `acos`/`cos`/`ln`/`powf` come from the
platform's `libm` (gh:#1823). Pipe and fuel channel agree. The pins are
native.

**Checked, by looking at the images:**

- TetGen tets and their dual are **irregular** (5–8-sided sections), not
  the carved route's truncated-octahedron lattice (`pair_*`: the carved
  dual is a square-symmetric pattern of octagons and squares, the TetGen
  dual is not).
- Cell size **grades toward the pebble** (smaller cells round it in
  `pebble_axial_nonorth.png`), but not toward the pipe or channel walls:
  with `-a` from the cell size the interior is near-uniform and only small
  surface features refine. The maintainer's reference pictures grade toward
  every wall; that is not reproduced.
- The pebble is a **hole** with its own boundary (it was not, before
  `hole_points`: TetGen meshed its inside).
- Layers sit on the walls in order 1 → 2 → 3 (green, orange, pink) from
  the wall: on the pipe wall, the fuel pin (growing outward) and the
  pebble (growing outward into the coolant). The 20-layer stack is 20 thin
  bands, thinnest at the wall, ending in polyhedra.
- ~~The tet + layers views show tets against the wall and **no** layer,
  with the report's "boundary layers added none" note.~~ **CORRECTED
  2026-11-26 (gh:#1824, re-rendered with `strategy=tetmesh`):** the pipe
  and fuel-channel views show three prism layers on the wall, green
  (layer 1) at the wall, then orange, then pink, each one cell per wall
  face, under unstructured tets; on the pin they grow outward into the
  coolant. The report: 0 negative cells, max non-orthogonality 65.8° /
  73.7°, skewness 0.90 / 1.08, 2111 / 5105 cells per layer. The pebble
  view shows tets against the pebble and no layer; its report says the
  acceptance gate refused the layer and why (12 inverted cells remain on
  the browser's mesh, 6 on the native one).
- At phone width the controls fold, the zoom buttons and report remain,
  and the pebble hole is centred.

**Not checked:** y-cuts; other cut positions; the TetGen route on the
primitives beyond `box_cross_layers.png`; whether TetGen's dual with
layers suits any flow; the browser's own count differences beyond the
pebble (only the three shapes' default builds were compared).

### No lag (the no-lag rule), measured with Playwright

Chromium headless, the view dragged throughout the build, frame gaps from
`requestAnimationFrame` (`no_lag.mjs` in this folder):

| build | window | frames during the build: median / p99 / max | the same drag with no build |
|---|---|---|---|
| pebble default (the heaviest default: 5.9 s in the worker) | 1100 × 760 | 49.9 / 83.3 / 83.4 ms | 33.4 / 66.8 / 100.1 ms |
| pebble default | 390 × 844 | 16.7 / 33.3 / 49.9 ms | 16.7 / 50.0 / 66.7 ms |
| fuel channel default (2.3 s) | 1100 × 760 | 50.0 / 83.4 / 83.4 ms | not run |
| pipe default (1.2 s) | 1100 × 760 | 50.0 / 83.4 / 83.4 ms | not run |

The frame gaps during a build match those of the same drag with nothing
building: the cost is the software-rendered drawing, not the worker. The
largest gap seen on page load (333 ms) is before the build starts (the
wasm starting).

## gh:#1825: TetGen's tetrahedra graded toward the walls (2026-11-26)

`?grade=on` (panel: "grade toward the walls (TetGen -m)"): TetGen `-m`
sizes half the cell size h at the walls (the layer patches), growing 1.2
per cell to 2h inside, on the PLC points and on a jittered lattice of seed
points in the volume; `-a` derived from 2h. Off by default (the gh:#1796
pins stay). The port of `-m` is byte-identical to upstream's binary
(`outram-park-fork-cfmesh/verification_and_validation/tetgen_sizing_gh1825/`,
with the grading measured and the native raw-mesh images coloured by edge
length).

| shape | grade | tets | dual cells | native tets / dual [s] | browser tets / dual [s] |
|---|---|---|---|---|---|
| pipe | off | 18 020 | 3 562 | 0.33 / 0.68 | — |
| pipe | on | 26 963 | 5 594 | 0.33 / 0.53 | 0.71 / 0.90 |
| fuel channel | off | 30 804 | 6 399 | 0.63 / 0.62 | — |
| fuel channel | on | 65 097 | 13 104 | 0.88 / 1.31 | 1.84 / 2.29 |
| pebble | off | 35 347 | 7 186 | 0.67 / 0.95 | — |
| pebble | on | 45 321 (browser 45 176) | 9 220 (9 180) | 0.74 / 1.11 | 1.24 / 1.86 |

(Native: `mesh_studio --measure --shape <s> --tets tetgen --mesh tet|poly
--n 0 --grade on|off`, one run. Browser: the report's "meshed in" of the
`render.mjs` views, headless Chromium, this container. The pebble differs
between native and browser as gh:#1823 records for the ungraded mesh.)

**Views** (`render.mjs ... _graded_`): `{pipe,fuel_channel,pebble}_graded_tets_{z,x}.png`,
`*_graded_dual_{z,x}.png`, `*_graded_dual_wall_zoom.png`, and
`pipe_ungraded_dual_wall_zoom.png` beside `pipe_graded_dual_wall_zoom.png`.

**Checked, by looking at the images, against the reference description
(gh:#1796: "irregular polyhedra whose size grades down toward the walls"):**
the pipe's x-cut has its smallest polyhedra along both walls and its
largest on the axis, the inlet and outlet (not walls) keep large cells; the
wall zoom shows the polyhedra shrinking toward the wall, where the ungraded
zoom's are the same size throughout; the pebble's finest cells hug the
pebble and the channel wall is finer than mid-gap. **Does not match yet:**
the fuel channel's grading is weak (interior / wall mean edge 1.18): its
0.5 m annulus leaves the size only 0.25 m to grow; a smaller wall size
grades it more at a quadratic cost in cells (a quarter of h: 399 k
tetrahedra). The grading is also gentler than the sizes ask (1.66 against
2.6 on the pipe axis).

## ~~Results (2026-11-26, branch `claude/outram-blender-mggk7u`)~~

**CORRECTED 2026-11-26 (gh:#1796):** the table below described the
**carved (structured) default** of the first round. The images were
regenerated on the new TetGen default; every view of this table except
the three `fuel_channel_stage_*` views (which now ask for `tets=carved`
so the carve, snap and split stages still have cells) now shows TetGen's
tetrahedra and their dual. New counts, from the page titles: pipe views
6826 cells; fuel channel views 14313; pebble views 7158; box 3327 (TetGen,
per-corner dual, 249 core + 3 × 1026); `fuel_channel_stage_*` 22204 (carved,
as before). The struck figures are kept for the carved route.


| image | shape · cut · colour | ~~cells~~ carved cells (first round) | what to look at |
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

### Layer counts and thicknesses, read from each page's report (carved route, first round)

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

## What was checked, and what was not (first round, carved route)

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
