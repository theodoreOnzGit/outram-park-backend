# CLAUDE.md

Guidance for Claude Code (and other AI assistants) working in the `dhoby-ghaut` crate.

## Reactor geometry is DRAWN for a human to check before it is trusted (HARD RULE)

**Maintainer direction, 2026-09-25.** Binds this crate. The same rule is in the
`CLAUDE.md` of `outram-mc-libs`, `nee_soon`, every `outram-foam-*` crate and
every crate downstream of them; a crate that newly depends on one of those
takes the rule into its own `CLAUDE.md` (check with `cargo metadata`).

**Whenever you build or change a complex reactor geometry** — CSG cells and
surfaces, lattices, pebble beds, TRISO particles, reflector zones, control-rod
bands, a CFD/FEM mesh, anything a solver will transport or integrate through —
**draw it, as images a human can open (PNG, or JPG/SVG), and hand them over**
before any result computed on it is reported as more than tentative.

- **Draw what the solver sees, not what you meant.** Render from the ASSEMBLED
  geometry (cell / material lookup at each pixel, or the mesh itself), never
  from the named constants. A picture of the constants hides exactly the
  defects this rule exists to catch.
- **Minimum set:** an axial slice (R-Z / x-z) of the whole model; radial (x-y)
  slices at the heights that matter; and, for nested geometry, zoomed slices at
  every level down to the smallest (pebble, TRISO particle). Colour by
  material, with a legend and the key dimensions marked.
- **Commit the images with the change** (beside the V&V record or the
  manuscript package) and point the human at them by path in your summary.
  Regenerate them whenever the geometry changes.
- **Say what you checked in them, and what you could not** — an image nobody
  was told to look at checks nothing.

**Why.** On 2026-09-24/25 the HTR-10 model carried, at once: a bottom reflector
mirrored from the top and up to 107 cm short; a core cavity that grew with the
bed; pebbles interpenetrating by 1.1 cm with 4.8 % of core carbon clipped away
(gh:#309, #310); and a TRISO lattice holding 8240 particles while reporting
8340 (gh:#316, +353 pcm). Every run completed with green diagnostics. Each was
found by looking at the built geometry, not by the eigenvalue.

**Tools.** ~~`outram_mc_libs::geometry::plot` samples a slice of an assembled
CSG geometry; OpenMC-parity image output (PNG/JPG) is the preferred path once
it lands.~~ **UPDATED 2026-09-25 — it has landed (gh:#268):**
`outram_mc_libs::geometry::plot` is a port of OpenMC's plotter that writes PNG
directly — slices, wireframe and solid ray traces — verified pixel-for-pixel
against `openmc --plot`
(`crates/outram-mc-libs/verification_and_validation/geometry_plotting/`).
`render_material_slice` draws a material-coloured slice with a legend and cm
axes in one call; `crates/nee_soon/examples/htr10_geometry_images.rs` is the
worked example. For meshes, plot the mesh itself (cells, patches, zones).

## Web demos are mobile-first (HARD RULE, 2026-10-04)

Every demo here follows
[`docs/claude-md/mobile-first-tutorials-and-demos.md`](../../docs/claude-md/mobile-first-tutorials-and-demos.md):
a full-page main view with on-screen zoom in, zoom out and reset buttons, and a
collapsible side panel holding every control, folded by default on a narrow
screen. `examples/monte_carlo_web/` (~~`examples/triso_pebble_web/`~~, renamed
2026-10-04) is the reference implementation.

## Web demos: how a new track app and a new rung plug in

**Since 2026-10-04 (gh:#521) the track-independent half of every browser demo
is in this crate's LIBRARY, `dhoby_ghaut::web_demo`.** Build on it; do not
copy `monte_carlo_web`. It gives, ready-made and tested:

| piece | module | what it does |
|---|---|---|
| main view | `web_demo::view` | `View` (world cm to screen, `fit`, `zoom_about`, `handle_input`: wheel / pinch / drag / double-click; refits on a size change until the reader zooms or pans), `zoom_buttons` + `apply_zoom` (+ / − / Reset, 36 px, top right), `scale_bar` |
| side panel | `web_demo::panel` | `Panel::show(ui, heading, contents)`: folds, "« Hide" inside, opens folded under 700 px, at most 85 % of a phone; `reopen_button` draws "Controls »" on the main view |
| no-lag plumbing | `web_demo::link` | `Link<Req, Ev>` (UI side, `send` / non-blocking `drain`), `NativeEngine` + `start_native` (a thread), `WorkerEngine` + `worker_main` + `start_web` (a module Web Worker on the same wasm, with the hello handshake), `Message` + `js` helpers, `fetch_start` / `fetch_promise` |
| loading | `web_demo::loading` | `Loading`: per-job progress weighted by cost, `card` on the main view, `grid` in the panel |
| rungs and lessons | `web_demo::lesson` | trait `Rung` (`all`, `name`, `title`, `lesson`), `from_query` (`?rung=`), `lesson_url`, `whats_happening`, `picker` |
| platform | `web_demo::platform` | `now_s` (no `Instant` on wasm), `set_title`, `query_pairs` (URL query, or `--key value` natively), `set_query`, `autostart`, `keep_canvas_at_device_pixels` (canvas backing store = CSS size × `devicePixelRatio`; `Panel::show` calls it every frame, gh:#556) |

### A new track's demo (nuclear data, dispersion, fuel performance, …)

1. `examples/<track>_web/main.rs`, declared in `Cargo.toml` like
   `monte_carlo_web` (Android stub `main`; on wasm, `worker_main::<YourEngine>()`
   when in a worker, else start eframe on a canvas).
2. `Request` / `Event` enums; implement `web_demo::link::Message` for both on
   wasm (see `monte_carlo_web/engine.rs`, module `web`).
3. One engine type implementing `NativeEngine` and `WorkerEngine`. **Each
   request must be short**: split long work into steps the UI asks for (a
   generation, a batch) or yield between steps in an async task. Never hold a
   lock across a computation.
4. An `eframe::App` with a `Link`, a `Panel`, a `View` and (while loading) a
   `Loading`; each frame `drain`, handle events, `panel.show(..)`, draw, then
   `zoom_buttons` and `panel.reopen_button`.
5. A rung enum implementing `web_demo::lesson::Rung` if the track has rungs.
6. `web/<track>/{index.html, worker.js, build.sh}` copied from
   `web/monte_carlo/` with the module renamed, and the build added to
   `scripts/build-pages.sh` (publish, every-page list, rung check).
7. Check at phone width AND that the UI stays live during the heaviest
   computation (HARD RULES), and say how in the report.

### A new Monte Carlo rung (additive: no one else's file changes)

1. A directory `examples/monte_carlo_web/<rung>/` whose `mod.rs` defines a
   marker type implementing `rungs::McRung`: `INFO` (`name:`, `title:`,
   `lesson:` on one line each, `spectrum:`), `jobs` (tapes), `tier`,
   `job_weights`, `half_extent`, `draw`, `notes`, and optionally
   `run_default` (gives it Run k_eff), `watch_generations`, `reference`,
   `loading_note`, `legend`. Its `Builder` implements `RungBuilder` (one tape
   per `step`) producing a type implementing `LoadedRung` (`run_next` traced
   history; `keff_start` / `keff_step` / `keff_finished`, or an error for a
   rung without Run). `godiva/` is the worked example with Run k_eff,
   `triso/` without. Optional since 2026-10-05 (gh:#549): a small `k_inf`
   case the reader runs at a parameter of their choice (`McRung::kinf_case`
   with `LoadedRung::kinf_start` / `kinf_step`, one generation per request;
   `lct008/`'s pitch slider, on `transport_csg::CsgPowerIteration`), and
   the σ(E) panel beside the geometry (`LoadedRung::xs_curves`, `xs::curves`).
   Add `render.rs` for the geometry review images (the
   drawing rule above) and wire it into `--render-geometry` in `main.rs`.
2. **One line** in the `rung_table!` invocation in `main.rs`
   (`<module>: <Marker>,`, in ladder order). The macro declares the module and
   generates the `Rung` enum and all dispatch; the engine, app, worker and
   message format need no change.
3. The lesson page `crates/outram-mc-libs/docs/tutorial/src/<rung>.md`, listed
   in that book's `SUMMARY.md` and in the "every page must exist" list of
   `scripts/build-pages.sh`; it links the demo as
   `../../demos/monte-carlo/?rung=<name>&mode=watch` (or `mode=run`). The site
   build fails if the rung's lesson page is missing or a page links to a rung
   not in the table.
4. Any new tape must be in `reference-data/endf/`; `--prepare-web-data`
   publishes every rung's tapes (shared ones once).

## The high-fidelity workbench (gh:#561)

**Maintainer design, 2026-10-05.** `src/bin/dhoby-ghaut/` is the guided
high-fidelity simulator; `src/workbench/` is its library half (catalogue,
steps, recipe). Rules that bind changes to it:

- **Recipes are kovan markdown, read and written through `kovan::artifact`.**
  Do not write a second markdown or TOML-in-markdown parser. Plain TOML is
  for light data transfer only (the reactivity map, gh:#571; #576).
- **Every input and output file is kovan-compatible, and this crate depends
  on `kovan` directly**, which is why it is AGPL-3.0-only (`NOTICE`). A crate
  that depends on `dhoby-ghaut` as a library inherits that: confirm with the
  maintainer first.
- **Prefill from the model's own constants wherever it exports them**
  (`preset.rs` reads `nee_soon::htr10_rmc`); a value it does not export is
  marked at the line that types it. Cite each value; give a page only where a
  source states one.
- **State what the model lacks.** Every part of the reactor is *in model*,
  *simplified* or *NOT in model*, with an issue. The pre-built card shows the
  preset's V&V status from its record, never "validated"; an edited recipe is
  "derived", with no V&V standing.
- **The review gate stays in front of Monte Carlo.** It draws the drawing
  rule's minimum set from the assembled geometry.
- **The UI thread only draws.** ~~Two engine threads (geometry, physics) share
  the assembled core as an `Arc`.~~ **CORRECTED 2026-10-05:** four engine
  threads: geometry and physics (which share the assembled core as an
  `Arc`), the Step 1 DEM pour (`dem.rs`) and the Step 10 coupled run
  (`coupled.rs`).
- **Steps 9–10 (gh:#574) run a SIMPLIFIED coupled case** (`porous_core.rs`:
  r-z porous-core TH on `tampines` correlations, prescribed power shape,
  lumped feedback). Every gap is in `mp_preset.rs::elements` with its issue
  (gh:#591, #592, #593); keep that list true when the solver changes, and
  re-run the mesh study in
  `verification_and_validation/htr10_multiphysics_step10/` before moving the
  default mesh. Step 9 is saved as its own kovan artifact (`step-9`,
  `src/workbench/multiphysics.rs`), which also holds the input types Steps 7
  and 8 hand over.
- **A step that is not built says so** and names its issue
  (`WizardStep::issue`); nothing is simulated behind a placeholder.

## Files and folders are chosen with a file picker (HARD RULE)

**Maintainer direction, 2026-10-05. Binds `dhoby-ghaut`, `kovan` and `dover`.**
Whenever the program needs the user to open, save or choose a file or a
folder (an ENDF library, a kovan root, a recipe or input deck, an export
folder, an image, a PDF), it offers a **file picker**: `egui-file-dialog` in
the egui apps (the workspace dependency; one shared `FileDialog` routed by
what asked for it, as `kovan/src/app/mod.rs` `FileDialogTarget` and
`dhoby-ghaut/src/bin/dhoby-ghaut/app.rs` `Pick` do), a file-path argument in
a CLI. A typed path box is never the only way in; the picker opens where the
current value is. Show the chosen path as text so the user can see it.

**Why.** A typed path is error-prone and undiscoverable: the user has to know
the exact folder and spell it. The maintainer asked for pickers everywhere
after the workbench's first build had typed ENDF and kovan-root fields.
