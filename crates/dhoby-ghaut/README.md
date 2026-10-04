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

**The library is a placeholder.** Its only public item is the `EXPANSION`
constant, and it has no `[dependencies]`. Everything here is an **example**:
the two studios, moved here from `outram-blender` on 2026-09-17, and the TRISO
pebble demo, added 2026-10-03:

| Example | What it does |
|---|---|
| `mc_studio` | Author a geometry, set a material and run settings, run a basic `outram-mc` k-eigenvalue calculation through `nee_soon::sim` (~~`outram_blender::sim`~~, moved 2026-10-02, #486), and read `k_eff ± σ` with the per-generation plot |
| `mesh_studio` | Author a surface in `outram-blender`, volume-mesh it through `outram-park-fork-cfmesh`'s tet → dual → boundary-layer pipeline, show the mesh statistics, and export an OpenFOAM `polyMesh` |
| `triso_pebble_web` | Transport **one neutron at a time** through a 2D analogue of an HTR-10 fuel pebble on real ENDF/B-VIII.0 data, and draw each track as it flies. Single-threaded; also runs **in the browser** — see [below](#the-triso-pebble-in-the-browser) |

```bash
cargo run -p dhoby-ghaut --example mc_studio --release
cargo run -p dhoby-ghaut --example mesh_studio --release
```

Each example has a `--headless` mode that prints CSV with no window, checked by
`#[test]`s against a committed fixture under `tests/fixtures/` (four per
studio, six for the pebble):

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

## The TRISO pebble, in the browser

**Live:** <https://theodoreonzgit.github.io/outram-park-backend/demos/triso-pebble/>
(published by the backend Pages site on every push to `develop`; its geometry
images are at `demos/triso-pebble/geometry/`).

`examples/triso_pebble_web/` puts a pebble on screen and sends neutrons into it
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

```bash
cargo run -p dhoby-ghaut --example triso_pebble_web --release                  # native window
cargo run -p dhoby-ghaut --example triso_pebble_web --release -- --headless 40 # CSV, one row per neutron
crates/dhoby-ghaut/web/triso_pebble/build.sh                                   # browser build -> dist/
python3 -m http.server -d crates/dhoby-ghaut/web/triso_pebble/dist 8000
```

**Geometry review images** (the crate's drawing rule), rendered from the
assembled geometry with the OpenMC-parity plotter, are committed under
[`examples/triso_pebble_web/geometry/`](examples/triso_pebble_web/geometry/):
the whole cell, a quadrant, one particle, and two axial slices. Regenerate them
with `-- --render-geometry <dir>` whenever the geometry changes.

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

GPL-3.0. Part of the [OUTRAM PARK](../../README.md) workspace.
