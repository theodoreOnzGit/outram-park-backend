# DHOBY GHAUT

**D**igital **H**igh-fidelity **O**rchestration **by** **G**UI for a
**H**uman-friendly **A**utomated **U**nified **T**oolkit.

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`. The studios are offline
> demonstrations: never connect one to a live operational, plant,
> safety-critical or restricted system.

The home for OUTRAM PARK's graphical studios, the ones that drive physics
rather than only display it. Windowing GUIs (`egui`/`eframe`) cannot build for
Android/Termux, which every non-GUI library in the workspace must. This crate
is where the windowing stack lives, so that `nee_soon`, `outram-blender` and
the solver crates never have to carry it. It adds no physics of its own: a
studio that needs a quantity the solver crates do not expose gets it added
there.

Its **low-fidelity counterpart is [DOVER](../dover/README.md)** (maintainer,
2026-09-25), which fills the same role over low-fidelity models. ~~DOVER is an
empty skeleton for now.~~ **CORRECTED 2026-09-30**: its first model, a
deck-driven steam-methane-reforming CSTR, is merged.

## What is here

~~**The library is a placeholder.** Its only public item is the `EXPANSION`
constant, and it has no `[dependencies]`.~~ **CORRECTED 2026-10-04:** the
library holds `web_demo`, the framework every tutorial track's browser demo is
built on (mobile-first main view and panel, the worker/thread plumbing of the
no-lagging rule, loading card, rung table and lesson links), so it depends on
`egui` (off Android) and, on wasm32, the wasm-bindgen family. **Since
2026-10-05** it also holds `workbench` (off wasm32): the guided high-fidelity
workbench's reactor catalogue, wizard steps and the **recipe** format (input
decks as kovan markdown + TOML), which is why it depends on `kovan`, `serde`,
`toml` and `uom` and is AGPL-3.0-only (see [`NOTICE`](NOTICE)). The GUIs and
demos themselves are **examples**:
the two studios, moved here from `outram-blender` on 2026-09-17, ~~and the Monte
Carlo web demo (the TRISO pebble demo of 2026-10-03, made multi-rung on
2026-10-04)~~ **CORRECTED 2026-10-05:** and one web demo per tutorial track,
each built on `web_demo` (Monte Carlo, dispersion, nuclear data, TRISO-ATOPS):

| Example | What it does |
|---|---|
| `dhoby-ghaut` (**binary**) | **The guided high-fidelity workbench, first slice** (gh:#561): pick a reactor type by generation, Basic or Advanced, the HTGR core, then Steps 0–11 with every value prefilled and cited, a kovan literature pane beside the model, and recipes saved and loaded as kovan markdown. Steps 0–5 work for HTGR → Basic → pebble bed (HTR-10, the TENTATIVE `nee_soon::htr10_rmc` model); Steps 6–11 are shown with the issue that builds each. See [below](#the-high-fidelity-workbench) |
| `mc_studio` | Author a geometry, set a material and run settings, run a basic `outram-mc` k-eigenvalue calculation through `nee_soon::sim` (~~`outram_blender::sim`~~, moved 2026-10-02, #486), and read `k_eff ± σ` with the per-generation plot |
| `mesh_studio` | Author a surface in `outram-blender`, volume-mesh it through `outram-park-fork-cfmesh`'s tet → dual → boundary-layer pipeline, show the mesh statistics, and export an OpenFOAM `polyMesh` |
| `monte_carlo_web` (~~`triso_pebble_web`~~, renamed 2026-10-04) | **One demo, a rung of the Monte Carlo tutorial at a time** (gh:#520, #521): `godiva`, a bare uranium sphere, with a Watch mode and a true **Run k_eff** mode (a live power iteration with an `openmc.run()`-style console), and `triso`, one neutron at a time through a 2D HTR-10 pebble. Real ENDF/B-VIII.0 data; single-threaded; also runs **in the browser** — see [below](#the-monte-carlo-demo-in-the-browser) |
| `dispersion_web` | **The dispersion track's demo** (gh:#530): seven rungs, buangkok's Gaussian plume and changi's puff train computed in a Web Worker; the capstone recorded. Published at `demos/dispersion/` |
| `nuclear_data_web` | **The nuclear data track's demo** (gh:#529): real ENDF/B-VIII.0 processed by `njoy-outram-park-fork` in the browser, one NJOY module per rung. Published at `demos/nuclear-data/` |
| `triso_atops_web` | **The TRISO-ATOPS and fuel failure track's demo** (gh:#540): eight rungs, every number a call into `boon-lay` (the slice from region lookups, decay chains, the CRP-6 Case 1 walk against Crank, D(T) per layer, PANAMA-I fuel failure, chemistry, release into the coolant pools); the source term recorded. Published at `demos/triso-atops/` |

```bash
cargo run -p dhoby-ghaut --example mc_studio --release
cargo run -p dhoby-ghaut --example mesh_studio --release
```

~~Each example has a `--headless` mode~~ **CORRECTED 2026-10-05:** the two
studios, `monte_carlo_web`, `nuclear_data_web` and `triso_atops_web` have a
`--headless` mode that prints CSV with no window; the studios' and the Monte
Carlo demo's output is checked by
`#[test]`s against a committed fixture under `tests/fixtures/` (four per
studio; the web demo's tests cover both rungs, and the TRISO trace is pinned
by `tests/fixtures/triso_pebble_web_headless.csv`). `dispersion_web` has no
window-free mode. Every track demo's engine, which does all the computing, is
also exercised without a window by the example's own `#[test]`s:

```bash
cargo run  -p dhoby-ghaut --example mc_studio --release -- --headless
cargo test -p dhoby-ghaut --examples --release
```

The GUI dependencies (`eframe`, `egui`, `egui_plot`, `env_logger`,
`outram-blender` with its `foam-mesh` feature (~~and `mc-export`~~, retired
2026-10-02), and `nee_soon` for the Monte Carlo backend) are
dev-dependencies gated off Android. On Android each example compiles to an
empty `main`. **Since 2026-10-03** the studios' own stack (`egui_plot`,
`env_logger`, `outram-blender`, `nee_soon`) is also gated off `wasm32`, because
dev-dependencies resolve for every example and the pebble demo builds for the
browser; it adds `outram-mc-libs`, `njoy-outram-park-fork` and `miniz_oxide`
(native and wasm), and `wasm-bindgen` and friends (wasm only).

~~The non-GUI halves of both bridges still live in `outram-blender`
(`src/sim.rs`, `src/foam_mesh.rs`, `src/export.rs`). `src/lib.rs` records the
intent to move the headless Monte Carlo bridge into `nee_soon` as ungated code;
that has not happened.~~ **CORRECTED 2026-10-02 (GitHub #486):** the headless
Monte Carlo bridge moved to `nee_soon` (`nee_soon::sim`,
`nee_soon::blender_bridge`), ungated. The volume-meshing bridge stays in
`outram-blender` (`src/foam_mesh.rs`, feature `foam-mesh`).

## The high-fidelity workbench

```bash
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut
KOVAN_ROOT=~/your-kovan-library cargo run --release -p dhoby-ghaut --bin dhoby-ghaut
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-geometry
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --render-review out_dir
cargo run --release -p dhoby-ghaut --bin dhoby-ghaut -- --headless-keff --particles 500 --inactive 10 --active 20
```

The design was agreed with the maintainer on 2026-10-05 (gh:#561). A native
window walks a guided build:

- **Screen 1** lists reactor types by generation (Gen II, Gen III+ large, Gen
  III+ SMRs, Gen IV with MSR split into solid and liquid fuel). Each card has
  an honest status. Only HTGR is in the wizard; PWR and both MSRs are
  "partial" (models exist elsewhere in the workspace); the rest say "not yet".
- **Basic or Advanced.** Advanced is shown and not built yet.
- **HTGR core.** Pebble bed loads the HTR-10 preset and shows its V&V status:
  TENTATIVE, code-to-code against RMC, with the open residual its record
  states. Prismatic (HTTR) waits for its literature.
- **The wizard.** A top bar shows `Step N`, with Back and Next. The kovan
  literature pane is on the left, the reactor in the middle, and the
  prefilled settings on the right. The steps:
  - Step 0: ENDF library scan.
  - Step 1: pebble bed and pebble-type mix.
  - Step 2: pebble designs.
  - Step 3: reflector and internals, each part marked *in model*,
    *simplified* or *NOT in model*.
  - Step 4: inserts.
  - The **geometry review gate**. Every minimum view of the crate's drawing
    rule must be drawn before Monte Carlo is unlocked.
  - Step 5: Monte Carlo. Per-run state, nuclide-by-nuclide data progress, the
    console, k by generation, the lethargy-normalised spectrum and a table of
    runs.
  - Steps 6–11: placeholders naming their issues.
- **The main view** is a slice of the *assembled* geometry, from the solver's
  own cell lookups, re-rendered at screen resolution for whatever window is in
  view. It works from the whole reactor down to one TRISO particle. Slices
  export as PNG with legend and axes.
- **Recipes** (`Save recipe`, `--recipe file`) are kovan markdown. Each
  section is a kovan note artifact read by kovan's own parser, and its
  settings sit in a ` ```toml ` block in the body
  (`src/workbench/recipe.rs`).

What was checked on 2026-10-05:

- `--headless-geometry` is pinned by `tests/fixtures/dhoby_ghaut_geometry.csv`:
  43 445 cells, 22 974 tiles and 16 681 balls at 14 rings × 12 layers.
- The review images were inspected for whole pebbles and five TRISO layers.
- A preview run, 500 × [10 + 20] with 14 threads, took 87 s of nuclear data
  and 15 s of transport. It gave k = 0.99999 ± 0.01264, the same in the GUI as
  headless. That is a preview σ, not a benchmark comparison.
- The spectrum has its thermal peak near 0.05 eV and is flat per unit lethargy
  through the slowing-down range.

**Added later on 2026-10-05, at the maintainer's request:**

- **A Blender-like 3D viewport for Steps 1–3**, with the 2D slices one click
  away. It is a ray trace through the solver's own geometry (the OpenMC-port
  solid and wireframe ray tracers in `outram-blender`), not a mesh: there is
  no CSG tessellator in the workspace, and the trace shows what the solver
  sees.
  - Navigation: orbit (left or middle drag), pan (right drag, or Shift +
    middle), zoom (wheel, + / −), and Front / Right / Top / Iso / Frame all,
    with Persp/Ortho and Solid/X-ray toggles.
  - An axis gizmo and an outliner of materials to show or hide.
  - A **section cut** (off, X, Y or Z, flip, offset). It uses a clip plane
    added to `SolidRayTracePlot` as a flagged extension of the port (OpenMC
    has none), and the cut face is painted in its material's colour.
  - Each step opens on its own view: Step 1 the reactor in half-section with
    the bed; Step 2 one fuel pebble cut through its centre (TRISO in the cut
    face); Step 3 the reflector in half-section.
  - A quarter-resolution preview while moving, the full trace once still. The
    HTR-10 half-section takes 5–7 s at full resolution on the 16-core
    development machine, under a software-rendered display.
- ~~**Every font is twice egui's default** (`FONT_SCALE` in `app.rs`).~~
  **REVERTED the same day** at the maintainer's request, pending a systematic
  style settlement (#586); `FONT_SCALE` is 1.0 and is the hook for it.
- **Files and folders are chosen with a file picker**: the ENDF folder, the
  kovan root, the PNG folder, and recipe open / save / save-as. This is a hard
  rule in this crate, kovan and dover (`CLAUDE.md`).
- **The k_eff console fills live**, one line per generation as
  `openmc.run()` prints them, in a scroll area that follows the newest line.
  Generations stream through `outram-mc-libs`'
  `run_keff_csg_hybrid_with_progress` (#579). A test pins that the callback
  changes nothing about the run.

**Fresh DEM pour in Step 1 (2026-10-05).** Step 1's bed source is either the
preset lattice (Şeker's 13-ball cell) or a **fresh DEM pour**:
`outram-park-fork-liggghts`' `htr10_fill` pours N pebbles (default 27 000,
the full core the quoted 0.61 refers to) with the publications package's
contact settings, on its own thread, drawn live. Once a pour finishes, "Build
the Monte Carlo core from this pour" assembles it with `nee_soon`'s explicit
bed (`assemble_explicit_triso_from_centres`):
- pebbles are kept whole;
- soft-sphere overlaps are split by the bisector plane;
- the tube below the DEM column is filled with the lattice model's dummy
  balls;
- fuel is 57:43 above the floor, with the conus and tube all dummy.

Steps 2–5 then run on that core; until a pour finishes they use the lattice.
Headless: `--headless-geometry --centres bed.csv` and
`--render-review DIR --centres bed.csv`. A core built from a pour has no
validated k: it is a new bed, not the reference's.

Limits, each with an issue:

- ~~The k_eff console fills at the end of a run, not live (#579).~~
  **FIXED 2026-10-05**: it fills live (above).
- ~~Rod insertion is not modelled (#580).~~ **DONE 2026-10-05**: Step 5's
  slider moves all ten explicit rods (0 withdrawn to 1 fully inserted) and
  re-assembles the core; drawings and a TENTATIVE rods-out/in k in
  `verification_and_validation/htr10_rod_insertion/`. One rod alone is not
  representable.
- ~~A custom ENDF folder is scanned but not used for the load (#581).~~
  **DONE 2026-10-05**: the k_eff load reads every tape from the Step 0 folder
  (flat or an extracted library), found by MAT and NSUB; a tape the folder
  lacks stops the run with its name, never falling back.
- ~~Pebble designs and the pebble-type mix are recorded, not rebuilt
  (#566).~~ **DONE 2026-10-05**: the fuel fraction, fuel-zone radius, TRISO
  radii and particle count are rebuilt into the core; what the builder cannot
  represent (ball radius, materials, enrichment, fertile and poison pebbles,
  Steps 3-4 hardware) is listed in red as NOT in the built model.
- The refuelling chute, absorber spheres and irradiation-channel pebbles are
  not in the HTR-10 model (#570).

## The Monte Carlo demo, in the browser

**Live:** <https://theodoreonzgit.github.io/outram-park-backend/demos/monte-carlo/>
(published by the backend Pages site on every push to `develop`; geometry
images at `demos/monte-carlo/geometry/`). `?rung=godiva&mode=run` opens the
Godiva Run k_eff; `?rung=triso` the pebble. The old
`demos/triso-pebble/` URL redirects to `?rung=triso`. The rung table, and the
lesson page each rung links to, is `examples/monte_carlo_web/rungs.rs`.

**Godiva (rung 1, gh:#521).** ICSBEP HEU-MET-FAST-001 from
`outram_mc_libs::vv::godiva`, the model of the recorded result. *Watch* shows
real histories one at a time (illustration) and then whole generations of the
real power iteration, started from a point at the centre, with the source
entropy rising and flattening. *Run k_eff* runs outram-mc-libs'
`PowerIteration` (the `run_keff` reference backend, one generation at a time)
at the record's settings, printing one console line per generation, plotting
`k` and the running mean, counting leaked / captured / fissioned neutrons, and
comparing the result with the experiment and the record. It processes ENDF at
NJOY's tolerance 0.001 so the `k` is comparable. Natively (i9-13900K, one
thread, 2026-10-04) the data took 62 s and the 800 000 histories 2.9 s, giving
k = 0.99942 ± 0.00183 (seed 1).

**TRISO pebble.** `examples/monte_carlo_web/triso/` puts a pebble on screen and sends neutrons into it
**one at a time**, drawing each track coloured by energy as the neutron slows
down in the graphite, until it is captured or causes a fission. Start and stop
it, step one neutron, zoom in until the five TRISO coating layers are visible.

- **Transport is `outram-mc-libs`, unmodified.** Each neutron is one call to
  its continuous-energy `run_fixed_source_traced` with one source particle and
  fission progeny off, so every track drawn is one real history. When a neutron
  causes a fission, the next one is born at that site.
- **The nuclear data are real ENDF/B-VIII.0**, from `reference-data/endf/`, and
  are processed **in the browser tab**, by this workspace's own NJOY port
  (RECONR + BROADR to 296 K, plus the crystalline-graphite thermal-scattering
  law). This is the expensive part: measured natively, U-235 takes 17 s, U-238
  13 s and graphite S(alpha, beta) 15 s, about 47 s in all; the browser is slower
  (about 90 s in headless Chromium). The processing and the neutrons run in a
  **Web Worker** (a thread natively), not on the page's thread, so the page
  stays responsive throughout; only finished tracks cross to the page.
- **On a phone** the control panel starts folded away to the left so the pebble
  gets the screen; a button on the canvas brings it back.
- **Covariance data are stripped before download** — transport never reads
  them, and they are most of the bytes: 116 MB of tapes become an 11 MB
  download. A test proves the stripped tapes give **bit-identical** cross
  sections (2001 energies) and fission-spectrum samples.
- **Low fidelity on purpose**: reconstruction at the `VeryFast` tolerance 0.01,
  not NJOY's 0.001.
- **It is a 2D analogue, not HTR-10.** The dimensions are IAEA-TECDOC-1382's,
  but in 2D the TRISO particles are infinitely long rods. There are 152 of them,
  so the fuel fraction matches the real pebble (5.0 %); the cell pitch matches
  the core's 0.61 filling fraction. Rods self-shield differently from spheres,
  so read it as a picture of how neutrons move, not as a model of the reactor.

- **Animation speed follows the neutron's real speed** (classical kinetic
  energy, v ∝ √E). The slider is the speed at 1 eV on every rung; the default
  is 7 cm/s at 1 eV for thermal rungs and 0.007 cm/s at 1 eV (= 7 cm/s at
  1 MeV) for fast ones.

```bash
cargo run -p dhoby-ghaut --example monte_carlo_web --release                        # native window (Godiva Watch)
cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --rung triso        # the pebble
cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless 40       # TRISO CSV, one row per neutron
cargo run -p dhoby-ghaut --example monte_carlo_web --release -- --headless-keff     # Godiva Run k_eff, as a console
crates/dhoby-ghaut/web/monte_carlo/build.sh                                         # browser build -> dist/
python3 -m http.server -d crates/dhoby-ghaut/web/monte_carlo/dist 8000
```

### Adding a rung, or a new track's demo

Both are written down once, in this crate's
[`CLAUDE.md`](CLAUDE.md#web-demos-how-a-new-track-app-and-a-new-rung-plug-in).
In short: a **Monte Carlo rung** is a directory
`examples/monte_carlo_web/<rung>/` implementing `rungs::McRung`, plus one
line in the `rung_table!` in `main.rs`, plus its lesson page; a **new track's
demo** is a new example built on the library's `dhoby_ghaut::web_demo`
(main view and zoom buttons, folding panel, worker/thread plumbing, loading
card, rung table and lesson links), whose module docs walk through it.

**Geometry review images** (the crate's drawing rule), rendered from the
assembled geometry with the OpenMC-parity plotter, are committed under
[`examples/monte_carlo_web/geometry/`](examples/monte_carlo_web/geometry/):
the whole TRISO cell, a quadrant, one particle, two axial slices, and the
Godiva sphere in x-y and x-z. Regenerate them with `-- --render-geometry <dir>`
whenever the geometry changes.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.

## License

~~GPL-3.0.~~ **AGPL-3.0-only since 2026-10-05** (maintainer-directed, gh:#562),
so that the high-fidelity workbench can depend on `kovan` (AGPL-3.0-only) and
read and write kovan-compatible files. See [`NOTICE`](NOTICE). Part of the
[OUTRAM PARK](../../README.md) workspace.
