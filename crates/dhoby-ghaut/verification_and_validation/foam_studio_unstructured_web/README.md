# Foam Studio in the browser: tet-dual meshes with wall boundary layers

> Research, education and V&V only (`RESPONSIBLE_USE.md`). AI-generated, not yet
> reviewed by a human.

**Question.** Can the browser demo (Foam Studio's laminar pipe) build a tet-dual
polyhedral mesh with prism boundary layers on the wall, solve on it, and stay
responsive on desktop and phone? Covers gh:#1202, #1215 and #1240.

## Methodology

- **Build.** `crates/dhoby-ghaut/web/foam_studio/build.sh`: release wasm32 and
  `wasm-bindgen` 0.2.129 (the version `Cargo.lock` pins). Built from
  `claude/outram-foam-8ookor` at `9365a719b` plus merges, served locally.
- **Browser.** Headless Chromium (Playwright), with **software** WebGL
  (SwiftShader). Viewports: desktop 1280×900, phone 390×844.
- **Driving the page.** Every case is set by URL options, then left to run:
  - `mesh=unstructured`, `h` (cell size in mm), `layers`;
  - since 2026-11-05, `dual=point|centre`, `fp=corners` and
    `layerq=cfmesh|snappy`. These are the same as the new side-panel controls.
  - since gh:#1298 (2026-11-05), `route=legacy|upstream`; and the
    unstructured default changed, so the rows recorded before it are
    re-run with the old choices named in full
    (`route=legacy&dual=corner&fp=edges&layerq=backoff`, `LEGACY` in
    `browser_check.js`). `BROWSER_CASES` picks cases, `BROWSER_LOG` names
    the log.
  - `autostart=1` runs the solve as soon as the mesh arrives.
- **Signals read** (`browser_check.js`):
  - the page title, which carries the cell count, the step and "steady";
  - frame intervals from `requestAnimationFrame`, as 2-second windows on the
    UI thread;
  - page errors and console errors;
  - a screenshot while solving and one at the end.
- **Pass criterion.** The mesh is built, the solve reaches steady, there are no
  page errors, and the UI thread keeps drawing. The no-lag rule requires that
  meshing and solving run in the worker.

## Results: the default after the maintainer's gh:#1298 decision, h 5 mm with 3 layers (2026-11-05)

**Maintainer decision on gh:#1298 (2026-11-05):** the default keeps the
gh:#1298 mesher but is **h 5 mm with 3 wall layers** (first 0.3 mm, × 1.2),
because the bare h 4 mm default missed its dp/dz prediction (1.0649 × HP)
while this case measured 0.9394 / 1.0062. Loaded with no size in the URL
(`mesh=unstructured&autostart=1`); `h=4&layers=0` gives the bare case.
Built from `claude/outram-foam-8ookor` at `d73f11f61` plus the working tree
(page stamp `-dirty`), wasm-bindgen 0.2.129. Host: 4 cores, load ~1.6, no
other build running. Log: `browser_check_gh1298_default_h5.txt`.

| case | cells | meshed | steady | steps | UI frames while meshing (median / max) | page errors |
|---|---|---|---|---|---|---|
| **default** (h 5 mm, 3 layers), desktop 1280×900 | 7 049 | 22 s | 29 s | 26 | 16.7 / 183 ms (the page's first frame; ≤ 67 ms after) | 0 |
| **default**, **phone** 390×844 | 7 049 | 22 s | 27 s | 26 | 16.7 / 16.8 ms | 0 |
| bare h 4 mm (`h=4&layers=0`), desktop (re-run; screenshots kept from the first run) | 6 353 | 8 s | 13 s | 27 | 16.7 / 183 ms (first frame) | 0 |

- Predictions (V&V record, gh:#1298, written before the run): 7 049 cells,
  26 steps, meshing 20–25 s, median 16.7 ms, 0 errors; the bare URL 6 353
  cells / 27 steps. **All held.**
- **Phone meshing time: 22 s** for the default (against 8 s for the old
  bare h 4 default). It is above the "under ~20 s" I had taken as
  phone-acceptable for gh:#1298; the UI stays at 60 fps throughout (the
  meshing is in the worker).
- The side panel opens with cell size 5.00 mm, 3 wall layers, 0.30 mm,
  1.200; the estimate reads "≈ 7k cells estimated" with no "cell size
  raised" note (the 50k cap does not touch the default); the layer note
  reads "cfMesh's layer optimiser with upstream's sequence (the default):
  about 6 s natively ... about 22 s in a browser".
- Screenshots: `default_h5_3layers_{desktop,phone}_{solving,final}.png`.
  Checked by eye: the phone view has the panel folded ("Controls »"), the
  +/−/Reset buttons, the solved pipe and the dock with the profile against
  Hagen–Poiseuille and the cross-section (7 049 cells); the desktop panel
  as described above, "7049 cells · max non-orthogonality 45.1° · max
  skewness 1.97", "meshed in 20.88 s".

## Results: the gh:#1298 default (2026-11-05)

> ~~The "new default" rows below are the default~~ **SUPERSEDED the same
> day by the maintainer's decision above:** the "new default" rows are the
> gh:#1298 mesher bare at h 4 mm, now reached by `h=4&layers=0`
> (`browser_check.js` cases `default_gh1298_*` say so since).

**gh:#1298 made the cell-centre dual with corners, cfMesh's optimiser and
upstream's sequence (without the Laplace pass) the unstructured default.**
Built from `claude/outram-foam-8ookor` after `7906d896d` plus the gh:#1298
working tree (page stamp `-dirty`). Host: 4 cores, load ~3, no other build
running. Log: `browser_check_gh1298.txt`.

| case | cells | meshed | steady | steps | UI frames while meshing (median / max) | page errors |
|---|---|---|---|---|---|---|
| ~~**new default**~~ gh:#1298 mesher bare (h 4 mm, no layer), desktop | 6 353 | 7 s | 14 s | 27 | 16.7 / 117 ms | 0 |
| ~~**new default**~~ gh:#1298 mesher bare, **phone** | 6 353 | 8 s | 13 s | 27 | 16.7 / 100 ms | 0 |
| new mesher, h 5 mm, 3 layers, desktop | 7 049 | 23 s | 28 s | 26 | 16.7 / 100 ms | 0 |
| new mesher, h 5 mm, 3 layers, **phone** | 7 049 | 22 s | 26 s | 26 | 16.7 / 233 ms | 0 |
| ~~old default~~ the ablation (`route=legacy&dual=corner&fp=edges&layerq=backoff`), desktop | 6 353 | 2 s | 10 s | 29 | 16.7 / 167 ms | 0 |

- Cells and steps equal the native runs (gh:#1298 section of
  `laminar_pipe_gh1155.md`: 6 353 cells / 27 steps; 7 049 / 26; the
  ablation 29 steps as its pin).
- **Meshing is phone-acceptable for ~~the default~~ the bare h 4 mm case: 8 s on the phone
  viewport** (prediction 3–15 s, held), against 2 s for the old default.
  The 3-layer case takes 22–23 s, where the old route's cfMesh 3-layer case
  took 646 s (row struck below).
- The UI thread draws at 60 fps (median 16.7 ms) all through the meshing;
  the meshing is in the worker. The single long frame of each case is the
  page's first.
- Screenshots: `default_gh1298_{desktop,phone}_{solving,final}.png`,
  `default_gh1298_h5_3layers_{desktop,phone}_{solving,final}.png`,
  `legacy_default_desktop_{solving,final}.png`. Checked by eye: the phone
  view has the panel folded ("Controls »"), the +/−/Reset buttons and the
  solved pipe with the convergence dock; the desktop panel shows the new
  "cfMesh surface + final optimisation (upstream sequence)" box ticked,
  cell-centre, corners only and cfMesh optimise selected, "≈ 6k cells
  estimated", and "meshed in 5.62 s".
- **The estimate is fixed:** "≈ 6k cells estimated" for 6 353 (the
  `polyDualMesh` duals now have their own coefficients, gh:#1298).

## Results: before gh:#1298 (2026-11-05)

The machine has 4 cores, shared with a native agent build/run (gh:#1241). Times
are therefore upper bounds.

| case | cells | meshed | steady | steps | page errors |
|---|---|---|---|---|---|
| structured O-grid (control) | 20 480 | 2 s | 20 s | 34 | 0 |
| tet dual, h 5 mm, 0 layers | 3 113 | 3 s | 8 s | 24 | 0 |
| tet dual, h 5 mm, 1 layer | 10 745 | 3 s | 16 s | 28 | 0 |
| tet dual, h 5 mm, 2 layers, desktop | 18 377 | 4 s | 85 s | 117 | 0 |
| tet dual, h 5 mm, 2 layers, **phone** | 18 377 | 2 s | 66 s | 117 | 0 |
| cell-centre dual, corners, 2 layers, desktop | 5 721 | 2 s | 12 s | 32 | 0 |
| cell-centre dual, corners, 2 layers, **phone** | 5 721 | 2 s | 11 s | 32 | 0 |
| cell-centre dual, corners, **3 layers, cfMesh optimise** (pre-gh:#1298 route) | 7 025 | ~~**646 s**~~ **646 s on the old route; 22–23 s on the gh:#1298 route (7 049 cells)** | 656 s | 26 | 0 |

- **Cell and step counts match the native records.** The O-grid takes 34 steps;
  the per-corner dual has 3 113 / 10 745 / 18 377 cells for 0 / 1 / 2 layers;
  and the cfMesh 3-layer case takes 26 steps, as in gh:#1240. The browser
  therefore builds and solves the same meshes as native code.
- **3 layers.** The mesher reports "100.0 % of wall faces with all 3 layers",
  max non-orthogonality 56.3° and max skewness 1.65. The cross-section in
  `centre_corners_h5_3layers_cfmesh_desktop_final.png` shows the three wall
  rings.
- **The cfMesh optimiser takes minutes in a browser.** It took 646 s here,
  against 140–370 s natively (gh:#1240), with the CPU shared. The page warns
  about this beside the control.
- **UI thread while the worker meshed** (cfMesh case, 10 minutes): median frame
  16.7 ms, i.e. 60 fps, with occasional p95 of 66–150 ms.
- **UI thread while a large mesh solves:** median frames rise to 100–200 ms.
  - With a forced repaint (an orbit drag) at steady state, a redraw costs
    67 ms on the structured mesh and 83–100 ms on the 1- and 2-layer duals.
    The mesh overlay makes no difference.
  - The cost is therefore drawing in a software renderer, about 1.3–1.5× the
    structured mesh's, not computation on the UI thread. The solver stays in
    the worker.
  - It was not measured on a real GPU. That check, and a check on a real phone,
    are still open.

## What this says, and what it does not

**What it shows:**
- The browser demo can do tet-dual meshes with wall boundary layers, at 0–3
  layers.
- It covers all three dual kinds' controls, at both desktop and phone width.
- The worker does the meshing and the solving.

**What it does not show:**
- It does not change the physics record. The accuracy against Hagen–Poiseuille
  is the native V&V record's (`laminar_pipe_gh1155.md`) and is not re-measured
  here.
- The 3-layer back-off still diverges (gh:#1241, open). The page's warning
  says so.

~~**Known limitation (found here).** The cell estimate the page shows
(`TetDualSpec::estimated_cells`) is calibrated on the per-corner dual. For the
polyDualMesh duals it overestimates: ≈26k shown against 7 025 actual, for 3
layers. Near the 50k cap this could raise the cell size more than needed.~~
**FIXED 2026-11-05 (gh:#1298):** `estimated_cells` uses
`MESH_ESTIMATE_POLY_DUAL = (6.5, 2.6)` for the polyDualMesh duals, measured
by `--mesh-sweep`: 7 188 estimated against 7 049 for 3 layers at h 5 mm.
