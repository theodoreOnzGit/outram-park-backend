# AGENTS.md

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

## Never declare modules through a macro (HARD RULE, 2026-11-04)

**Maintainer direction, 2026-11-04**, the workspace rule of the same name in
the root `CLAUDE.md`, restated here because this crate is where it bit. Every
`mod` is a plain `mod name;` line: never `mod $m;` inside a `macro_rules!`
(`rung_table!`, removed 2026-11-05, was one). kovan reads `mod` lines without expanding macros, so
the HTR-10 web demo and nine other `monte_carlo_web` rungs had no
`kovan.toml` and were missing from review until their `mod` lines moved into
`main.rs`. A macro may still generate the enums and dispatch over modules
declared plainly. Pinned by `crates/kovan/tests/no_macro_declared_modules.rs`.

## Web demos are mobile-first (HARD RULE, 2026-10-04)

Every demo here follows
[`docs/claude-md/mobile-first-tutorials-and-demos.md`](../../docs/claude-md/mobile-first-tutorials-and-demos.md):
a full-page main view with on-screen zoom in, zoom out and reset buttons, and a
collapsible side panel holding every control, folded by default on a narrow
screen. `examples/monte_carlo_web/` (~~`examples/triso_pebble_web/`~~, renamed
2026-10-04) is the reference implementation; **since gh:#1763 its app,
engine and rungs are the library's `src/mc_demo/` (feature `mc_demo`)**, and
the example keeps only its `main` functions.

## Web demos: how a new track app and a new rung plug in

**Since 2026-10-04 (gh:#521) the track-independent half of every browser demo
is in this crate's LIBRARY, `dhoby_ghaut::web_demo`.** Build on it; do not
copy `monte_carlo_web`. It gives, ready-made and tested:

> **MOVED 2026-11-02 (gh:#1112):** the code now lives in the central GUI
> crate, `bugis::web_demo` (`crates/bugis/src/web_demo/`, feature
> `web_demo`). `dhoby_ghaut::web_demo` re-exports it, so every path in the
> table below still works and new demos may use either. Change the framework
> in bugis; `src/web_demo/` here keeps only its kovan review records.

| piece | module | what it does |
|---|---|---|
| main view | `web_demo::view` | `View` (world cm to screen, `fit`, `zoom_about`, `handle_input`: wheel / pinch / drag / double-click; refits on a size change until the reader zooms or pans), `zoom_buttons` + `apply_zoom` (+ / − / Reset, 36 px, top right), `scale_bar` |
| side panel | `web_demo::panel` | `Panel::show(ui, heading, contents)`: folds, "« Hide" inside, opens folded under 700 px, at most 85 % of a phone; `reopen_button` draws "Controls »" on the main view |
| docks | `web_demo::dock` | (2026-10-25) `Dock::new(Edge::Bottom \| Edge::Right, title)`, `split` (right first on the full rect, then bottom on what is left), `toggle`: a results or companion-view panel that folds to a thumb-sized tab; bottom opens at every width, right only where the main view is 860 px or wider; a bottom dock's body keeps `REFS_CLEAR_PX` free along its bottom for the page's References button. Every run's k_eff and entropy go in a bottom dock with the shared ~~`examples/monte_carlo_web/keffplot.rs`~~ **`dhoby_ghaut::mc_web::keffplot` since 2026-11-18 (gh:#1653)** (Godiva's Run mode and the HTR-10 whole core are the users) |
| no-lag plumbing | `web_demo::link` | `Link<Req, Ev>` (UI side, `send` / non-blocking `drain`), `NativeEngine` + `start_native` (a thread), `WorkerEngine` + `worker_main` + `start_web` (a module Web Worker on the same wasm, with the hello handshake), `Message` + `js` helpers, `fetch_start` / `fetch_promise` |
| loading | `web_demo::loading` | `Loading`: per-job progress weighted by cost, `card` on the main view, `grid` in the panel |
| rungs and lessons | `web_demo::lesson` | trait `Rung` (`all`, `name`, `title`, `lesson`), `from_query` (`?rung=`), `lesson_url`, `whats_happening`, `picker` |
| nuclear-data cache | `web_demo::data_cache` | (gh:#818) `Record` (a processed product's exact `f64` words, key, code version, SHA-256), `Miss` (why a lookup missed, with the note the page shows), `Source` (processed now / from this browser's cache, `summarize`), `Summary`, `PutError` (quota), and `idb` (IndexedDB glue on the site's origin, wasm only). The physics side, keys and `DataStore`, is ~~`examples/common/processed_cache.rs`~~ **`dhoby_ghaut::mc_web::processed_cache` since 2026-11-14 (gh:#1572)**; the old path is a re-export |
| browser Monte Carlo pipeline | `mc_web` (feature `mc_web`) | (2026-11-14, gh:#1572) promoted from `monte_carlo_web` so other demos can run it: `tapes` (strip, compress, wire names, LEAPR "tapes"), `processed_cache` (`DataStore`, keys), `keff` (`KeffConfig`, caps, console, summary), `pool` (`DataRelay`: deal jobs, relay products, assemble; `KeffPool<T>`: the k_eff run protocol with epochs, restarts and dead workers; `csg_source` / `csg_chunk`, the worker side of a CSG model; `schedule`, `thin_tracks`, the entropy-mesh words). **Since 2026-11-18 (gh:#1652, #1653, #1654) also the standard k_eff simulator panels**, so every k_eff simulator has the same layout (maintainer: "the same panel patterns okay for any keff simulator"): `keffplot` (k with running mean and reference bands, and Shannon entropy, for the bottom dock), `run_settings` (particles, inactive, active, seed, Defaults, Recorded, for the left panel), `rod_bank` (`RodBank` and `controls_ui` for any number of rods in the left panel; `draw_bank` for the right dock), `raster` (the CSG slicer for the main view) and, since 2026-11-20 (gh:#1677), `sweep` (`Sweep<S>`: k_eff at a list of settings run one after another on the pool, e.g. a rod worth curve, for any pool implementing the `SweepPool` trait; HTR-10's `LiveSweep` wraps it). The HTR-10 rung uses all five; build a new k_eff simulator on them rather than copying the rung. The HTR-10 core's `PoolState` wraps `DataRelay` + `KeffPool`; the examples' `tapes`, `keff` and `processed_cache` modules are re-exports. Users: `monte_carlo_web`, and `outram-park-digital-twin-engine`'s ~~`msre_web` k_eff tab (gh:#1571)~~ library module `msre::web` (the MSRE k_eff workers and pool, moved from its `msre_web` example, gh:#1578). Turn the feature on with a dev-dependency (this crate's own examples depend on themselves with `features = ["mc_web"]`), or, from a library, with `default-features = false, features = ["mc_web"]` (as the twin engine does) |
| the Monte Carlo demo itself | `mc_demo` (feature `mc_demo`) | (2026-11-25, gh:#1763) the whole `monte_carlo_web` app, engine and rungs, moved from the example so another simulator can host it: `McApp::hosted(ctx, query)` opens it on what a URL query would (`rung=htr10&view=core`), and the host draws `McApp::controls` in its panel, `McApp::main_view` in its central view and calls `McApp::pump` each frame; the app then leaves the page's URL, title and theme alone. In the browser `set_worker_js` points its workers at the demo's own `../monte-carlo/worker.js`. `htr10::core::screen::FinishedRun` (via `McApp::core_runs`) records each finished whole-core run for a host's coefficients. First host: `outram-park-digital-twin-engine`'s `htgr_sim_v1` MC page. The example keeps only its `main` functions; the tests moved to `src/mc_demo/tests.rs` |
| fields and residuals | `web_demo::field`, `web_demo::residual` | (gh:#1155) `field::Scale` (range + Crameri map), `polygon_mesh` / `paint` (cell polygons of a mesh section as one triangle mesh), `colour_bar`, `contains` (picking); `residual::History` (bounded, decimating history of named series), `plot_points`, `paint` (log or linear plot with legend). `foam_studio_web` is the first user (since gh:#1156 it also runs the double-pipe heat exchanger: three regions by `ChtMultiRegionFoam`, T on the slices, ε against ε-NTU; since gh:#1157 the helical coil section, with the Dean cross-section in the dock and f against Mishra & Gupta from `tampines`) |
| platform | `web_demo::platform` | `now_s` (no `Instant` on wasm), `set_title`, `query_pairs` (URL query, or `--key value` natively), `set_query`, `autostart`, `keep_canvas_at_device_pixels` (canvas backing store = CSS size × `devicePixelRatio`; `Panel::show` calls it every frame, gh:#556) |

### A new track's demo (nuclear data, dispersion, fuel performance, …)

1. `examples/<track>_web/main.rs`, declared in `Cargo.toml` like
   `monte_carlo_web` (Android stub `main`; on wasm, `worker_main::<YourEngine>()`
   when in a worker, else start eframe on a canvas).
2. `Request` / `Event` enums; implement `web_demo::link::Message` for both on
   wasm (see ~~`monte_carlo_web/engine.rs`~~ `src/mc_demo/engine.rs`, module `web`).
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

0. **The model goes in `nee_soon::mc_rungs::<rung>` (since 2026-11-25,
   gh:#1742; maintainer: "the code for the rungs shld live in nee soon"):**
   geometry and CSG case (`nee_soon::mc_rungs::CsgModel`), materials, tape
   list, bed and recorded values. The example holds only the GUI half and
   the data builder (which goes through the browser cache). Moved so far:
   `godiva`, `jemima`, `hst009`, `natu`, `triso`, `ugraphite`, `lumped`
   and the `htr10` core's bed codes, DEM bed, pour model and recorded bed k
   (`htr10_core`). Still here: `lct008`, `dhshort` and the `htr10` fuel
   zone, which `#[path]` their models from `outram-mc-libs/examples/common/`.
1. A directory `src/mc_demo/<rung>/` (~~`examples/monte_carlo_web/<rung>/`~~
   until gh:#1763, when the demo moved into the library) whose `mod.rs` defines a
   marker type implementing `rungs::McRung`: `INFO` (`name:`, `title:`,
   `lesson:` on one line each, `spectrum:`), `jobs` (tapes), `tier`,
   `job_weights`, `half_extent`, `draw`, `notes`, and optionally
   `run_default` (gives it Run k_eff), `watch_generations`, `reference`,
   `loading_note`, `legend`. Its `Builder` implements `RungBuilder` (one tape
   per `step`; **since 2026-10-09 (gh:#818) `step` takes the shared
   `processed_cache::DataStore`: build every nuclide with `store.nuclide(..)`
   and every thermal law with `store.thermal(..)`, never
   `Nuclide::from_tape_with_speed` directly, so the browser cache and the
   page's "processed now / from cache" line cover the rung**) producing a type implementing `LoadedRung` (`run_next` traced
   history; `keff_start` / `keff_step` / `keff_finished`, or an error for a
   rung without Run). `godiva/` is the worked example with Run k_eff,
   `triso/` without. Optional since 2026-10-05 (gh:#549): a small `k_inf`
   case the reader runs at a parameter of their choice (`McRung::kinf_case`
   with `LoadedRung::kinf_start` / `kinf_step`, one generation per request;
   `lct008/`'s pitch slider, on `transport_csg::CsgPowerIteration`), and
   the σ(E) panel beside the geometry (`LoadedRung::xs_curves`, `xs::curves`).
   Optional since 2026-10-07 (gh:#785): **a demo of its own** for a code walk
   whose idea is not tracks, a power iteration or a sweep
   (`McRung::walk_demo`, the Watch view `demo`): the page side is a variant
   of `walkdemo::WalkDemo` (its own panel and main view, asking the worker
   through `walkdemo::Outgoing`), the worker side `LoadedRung::walk` (flat
   `f64`s in and out, one short computation per message) and, for a slice,
   `LoadedRung::raster`. `dhshort/` and `packing/` are the worked examples.
   Add `render.rs` for the geometry review images (the
   drawing rule above) and wire it into `--render-geometry` in `main.rs`.
   Since 2026-10-13 (gh:#859): a rung whose model is a small CSG geometry
   implements `csgrung::CsgCase` instead (tapes, materials, geometry, source
   box, where a fresh neutron is born) and takes `csgrung::Builder<Self>` as
   its `Builder`: data processing, the Watch chain and a Run k_eff ~~on
   `CsgPowerIteration`~~ come with it (`jemima/` and `hst009/` are the worked
   examples). **CHANGED 2026-10-16 (gh:#905, #907):** Run k_eff runs on a
   pool of workers when the rung returns `Some(mb)` from
   `McRung::run_pool_mb` (one worker's memory, which sizes the pool):
   `benchpool.rs` starts the workers, each loads the rung, and the run is
   the HTR-10 core pool's protocol (`htr10/core/pool.rs`, `PoolState::ready`;
   since gh:#1572 its run half is the library's `mc_web::pool::KeffPool`)
   with each worker serving `LoadedRung::pool_source` / `pool_chunk`
   (`csgrung::pool_source` / `pool_chunk`, i.e. `transport_csg::distributed`,
   bit for bit `run_keff_csg_par` for any number of workers). Watch's
   whole-generations view stays on the serial `CsgPowerIteration`. Since 2026-10-14 the CSG run also reports the source entropy
   (`CsgCase::entropy_box` sets the mesh's box: override it when the case
   fissions outside its source box), and Run mode draws the geometry with
   neutrons born at the run's fission sites: while a run is going, the
   rung's `LoadedRung::run_next` must start its history at a site of that
   run's source (`csgrung::Keff::source_site`). (On a pooled rung, since
   2026-10-16, Run mode draws the run's OWN traced histories instead, the
   first four of each generation, which are counted in `k`.) Any rung can ship **recorded tracks** that play over the
   geometry while its data load: `McRung::recorded_tracks` returning
   `include_bytes!("data/tracks.bin.z")`, written by `--bake-tracks <rung>`
   (`recorded.rs`); re-bake when the rung's geometry or Watch chain changes,
   and update the counts its test pins.
2. ~~**One line** in the `rung_table!` invocation in `main.rs`
   (`<module>: <Marker>,`, in ladder order). The macro declares the module and
   generates the `Rung` enum and all dispatch~~ ~~**CHANGED 2026-11-04:**
   **two lines** in `main.rs`: a plain `mod <module>;` (with the
   `#[cfg(not(target_os = "android"))]` the others carry) and
   `<module>: <Marker>,` in the `rung_table!` invocation, in ladder order.
   The macro generates the `Rung` enum and all dispatch; the engine, app,
   worker and message format need no change.~~ **CHANGED 2026-11-05 (gh:#1252,
   #1253, maintainer: "refactor out the custom macros"):** `rung_table!` is
   gone, written out by hand in ~~`examples/monte_carlo_web/table.rs`~~
   `src/mc_demo/table.rs`. A new rung adds a plain `pub mod <module>;` line
   in ~~`main.rs` (with the `#[cfg(not(target_os = "android"))]` the others
   carry)~~ `src/mc_demo/mod.rs` (gh:#1763: the whole module is gated off
   Android in `lib.rs`), and in `table.rs`
   one `Rung` variant, its entry in `ALL` (ladder order), a variant in
   `Builder` and in `Loaded`, and **one `match` arm in every dispatch method**
   of `Rung`, `Builder` and `Loaded` (the compiler names each `match` that is
   missing the arm); the engine, app, worker and message format need no
   change. **No macro may declare the module (HARD RULE, root `CLAUDE.md`,
   "Never declare modules through a macro"):** kovan reads `mod` lines without
   expanding macros, and until 2026-11-04 every rung but `triso` (which
   `delta_tracking_web` loads by `#[path]`) was missing from kovan, the HTR-10
   rung among them. The same holds for every example and library here: no
   `mod $m;` in a macro.
3. The lesson page `crates/outram-mc-libs/docs/tutorial/src/<rung>.md`, listed
   in that book's `SUMMARY.md` and in the "every page must exist" list of
   `scripts/build-pages.sh`; it links the demo as
   `../../demos/monte-carlo/?rung=<name>&mode=watch` (or `mode=run`). The site
   build fails if the rung's lesson page is missing or a page links to a rung
   not in the table.
4. Any new tape must be in `reference-data/endf/`; `--prepare-web-data`
   publishes every rung's tapes (shared ones once). **A thermal law is
   generated by LEAPR, not read from a `tsl-*.endf` tape** (maintainer
   direction 2026-10-14): give its job the "tape" `tapes::LEAPR_H_IN_H2O`
   (nothing is downloaded or published for it; the engine passes empty
   bytes) and build it with `tapes::leapr_h_in_h2o`, as `hst009/` and
   `lct008/` do.

## Mesh Studio: the cfMesh pipeline, stage by stage (gh:#1765–#1771, 2026-11-26)

`examples/mesh_studio/` is one app with two targets, built on
`web_demo` like the other demos: native (eframe, plus `--headless`) and
the browser (`web/mesh_studio/`, published as `demos/mesh-studio/`). The
engine (`engine.rs`) builds through `outram_park_fork_cfmesh::pipeline::staged`
and answers cuts of any stage; the app only draws.

- **Shapes**: the four primitives (the headless sweep and its fixture use
  only these, `GeomKind::PRIMITIVES`) and three illustrative reactor
  shapes (pipe, fuel pin in a channel, pebble in a channel) built as
  revolved profiles with named patches; layers grow on the wall patches
  only.
- **Review images** are `verification_and_validation/mesh_studio_gh1771/`
  (`render.mjs` regenerates them). Wall layers are 1-2 px wide at the
  fitted zoom: a review set must include zoomed wall views
  (`?at=x,y&zoom=f`), and the report's "Wall layers total" line states the
  grown thickness against the asked one (~~back-off keeps ~10 % on the
  reactor shapes~~ **CORRECTED 2026-11-26, gh:#1796:** ~~back-off keeps~~
  ~~6–9 % on the carved dual and 12–17 % on the TetGen dual of the pipe and
  fuel channel, and grows none on the pebble~~ **CORRECTED again
  2026-11-26, gh:#1829:** layers grown first keep the full asked thickness
  on all three reactor shapes at the page's defaults).
- **Tet source (gh:#1796).** The page's default is TetGen
  (`Design::tetgen`, `?tets=tetgen|carved`, `?q=` radius-edge ratio,
  default 1.414, `?vol=` max tet volume, default the regular tetrahedron
  of the cell size `h³/(6√2)`); ~~the cfMesh crate's default, and so
  `Params::default` and the headless sweep, stay on the carved route~~
  (gh:#1829: TetGen is now the cfMesh crate's default too, so
  `Params::default` and the headless sweep mesh on TetGen). On
  TetGen's tets the carve and snap stages are listed as skipped and cut
  empty. `--measure` (`headless::measure`) prints one row of the
  mesh-type table for any query; the table is in the review README.
  `Design::MAX_LAYERS` is 25: ~~back-off grows none of a 20-layer stack on
  the pipe, `CfMeshOptimise` grows all 20~~ (gh:#1829: layers first grow
  all 20 at the full thickness).
- **Wall grading (gh:#1825).** `?grade=on|off|<wall size m>[,<growth>]`
  (`Design::wall_size`, `wall_growth`; panel: "grade toward the walls"):
  TetGen's `-m` sizes graded from the walls (the layer patches), `on`
  being half the cell size at the walls, growth 1.2, twice the cell size
  inside, and the derived `-a` the regular tetrahedron of that interior
  size. **Off by default**, so the TetGen pins do not move (switching it on
  by default needs the maintainer). Cost: 27 k / 65 k / 45 k tetrahedra
  (pipe, fuel channel, pebble) against 18 k / 31 k / 35 k ungraded; a
  quarter of the cell size at the walls gave the fuel channel 399 k, so the
  wall size is clamped to at least a quarter. `--tetgen-render <dir>
  graded` draws the raw graded meshes coloured by edge length; the
  `*_graded_*` views of `render.mjs` the page's.
- ~~**Tets + layers (gh:#1824)** build through cfMesh's `tetMesh` layer
  steps, `strategy=tetmesh` (`LayerQualityStrategy::CfMeshTetMesh`): the
  only strategy that grows layers on TetGen's tetrahedra (pipe and fuel
  channel; the pebble is refused with 6 inverted cells). The page's
  default strategy stays back-off, so `mesh=tet` without `strategy=tetmesh`
  still grows none; the panel's note says so.~~
- **Layers grown first (gh:#1829, 2026-11-26, the cfMesh crate's HARD
  RULE):** the strategy picker and the `strategy=` key are gone (the key is
  ignored). Prisms are grown from TetGen's wall triangulation and TetGen
  meshes the inside face to face; `mesh=poly` is the dual of both. The
  carved source (`tets=carved`) takes no layers: with `n>0` the build is
  refused and the panel says so. The stage stepper lists `boundary layers`
  before `polyhedral dual`. Measurements and images: the gh:#1829 section
  of `verification_and_validation/mesh_studio_gh1771/README.md`.
- The headless-sweep fixture test fails on HEAD for reasons that predate
  Mesh Studio (gh:#1772); do not re-pin it without that issue's answer.

## The high-fidelity workbench (gh:#561)

**Behind the default `workbench` feature since 2026-11-15 (gh:#1578):** the
`workbench` library module, the `dhoby-ghaut` binary (`required-features`)
and their whole stack (kovan, eframe, nee_soon, outram-blender's GPU tracer,
the foam and DEM ports, kopitiam-pdf) are optional, so a library that wants
only `mc_web` does not build them. A new workbench dependency goes in that
feature's list, `optional = true`, never as an unconditional dependency.

**Maintainer design, 2026-10-05.** `src/bin/dhoby-ghaut/` is the guided
high-fidelity simulator; `src/workbench/` is its library half (catalogue,
steps, recipe; since 2026-10-05 also `reactivity_map`, Step 6's low-fidelity
surrogate on petir's Chebyshev/QR, and `exports`, Step 11's CSV and kovan
report). Rules that bind changes to it:

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
  r-z porous-core TH on `tampines` correlations; ~~prescribed power shape,
  lumped feedback~~ **UPDATED 2026-10-05 (gh:#591):** by default the power
  and k are solved, `spatial.rs`: the GeN-Foam port's `DiffusionNeutronics`
  on Step 7's neutronics polyMesh with Step 8's `nuclearData` per cell,
  Picard-coupled to the march through Step 7's `MeshMapping`s; the
  prescribed shape is the explicit ablation `--prescribed-power`). Every
  gap is in `mp_preset.rs::elements` with its issue (gh:#592, #593, #594,
  #595); keep that list true when the solver changes, and re-run the mesh
  study in `verification_and_validation/htr10_multiphysics_step10/` before
  moving the default mesh. Step 9 is saved as its own kovan artifact
  (`step-9`, `src/workbench/multiphysics.rs`); ~~which also holds the input
  types Steps 7 and 8 hand over~~ its `MultiphysicsInputs` is just
  `MeshSet` + `MgxsSet` (no duplicate types). With the solved shape the
  march's bed is the reactor Steps 1-8 built (`mp_preset::on_built_core`).
- **Ray tracing is the GPU's job** (maintainer, 2026-10-05, gh:#587): the 3D
  view and the slices are traced by `outram_blender::csg::gpu` on eframe's
  wgpu device (`src/bin/dhoby-ghaut/gpu_view.rs`); the UI thread only
  submits GPU work, never waits on it, and the flattening runs on its own
  thread. The CPU plotter on the geometry engine thread is the fallback and
  the reference: PNG exports and `--render-review` always use it. A change to
  either tracer re-runs `tests/gpu_csg_parity.rs` (here and in
  `outram-blender`) and records the mismatch fraction it prints. Since
  2026-10-05 the flattened geometry carries a grid index
  (`outram_blender::csg::gpu::index`: HTR-10's half-sections ~0.36 s ->
  20-60 ms); the parity test draws every view with and without it, and
  also on a DEM-poured bed. Step 1's DEM pour view (`dem.rs`) draws its
  pebbles as GPU sphere impostors (`gpu_view::PebbleGpu`), the egui painter
  being the fallback without a wgpu device.
- **A step that is not built says so** and names its issue
  (`WizardStep::issue`); nothing is simulated behind a placeholder.
- **Steps 7-8 hand off through library types** (2026-10-05, gh:#572/#573):
  `workbench::meshes::MeshSet` (three meshes as GeN-Foam `polyMesh` +
  `cellZones`, the region map, six `MeshMapping`s with upstream `mapTgtToSrc`
  semantics) and `workbench::mgxs::MgxsSet` (a GeN-Foam `nuclearData`, zone
  names = region ids = cellZone names). Steps 9-10 read those; the module
  docs say exactly what is in them. The meshing calls `outram-blender` and the
  cfMesh port; the mesh-to-mesh overlap is
  `outram_blender::unstructured::overlap` (a port of OpenFOAM's
  `tetOverlapVolume` / `cellVolumeWeightMethod`, as GeN-Foam's `meshHandler`
  uses). Nothing meshes or maps in this crate itself.

## Sizes come from the style system (gh:#586)

Every text size in the workbench is a `style::Text` token and every width or
spacing a multiple of `style::em`; never a bare pixel number. The reader's
UI scale (egui's zoom factor, `style::UiScale`) multiplies all of them
together, so a layout that works at 100 % works at 150 %: check both, and
hold side panels to a fraction of the window rather than a fixed width. To
change the look, change the scale in `style.rs`, not individual call sites.

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

## Avoid writing macros (workspace rule, 2026-11-05)

Maintainer direction: macros make code harder to read and review. Do not
write new `macro_rules!` or proc macros in this crate; use functions,
generics, enums and plain `match` arms. Using std and dependency macros
(`vec!`, `format!`, derives) is fine. Full rule: root `CLAUDE.md`, "Avoid
writing macros".
