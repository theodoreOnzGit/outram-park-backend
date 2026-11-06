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

## Results (2026-11-05)

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
| cell-centre dual, corners, **3 layers, cfMesh optimise** | 7 025 | **646 s** | 656 s | 26 | 0 |

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

**Known limitation (found here).** The cell estimate the page shows
(`TetDualSpec::estimated_cells`) is calibrated on the per-corner dual. For the
polyDualMesh duals it overestimates: ≈26k shown against 7 025 actual, for 3
layers. Near the 50k cap this could raise the cell size more than needed.
