# BUGIS

**B**rowser-based **U**ser **G**raphics **I**nteraction **S**ystem (backronym set by the maintainer, 2026-11-02).

> ⚠️ **Research, education and V&V only.** Not for nuclear facility operation,
> reactor control, licensing, safety-critical decisions or emergency response.
> See the workspace `RESPONSIBLE_USE.md`.

The workspace's home for **colour theming in general** (maintainer,
2026-11-02), and the one place the egui simulators get their look from. It
answers
**"what does an OUTRAM PARK simulator look like?"**: the **Gruvbox** palette as
a dark and a light `egui::Visuals`, and a Dark/Light selector (maintainer,
2026-11-02: "give all simulators a gruvbox theme").

```rust,ignore
let mut theme = bugis::Theme::default(); // Gruvbox Dark
theme.apply(ctx);                        // once, at start-up
bugis::selector(ui, &mut theme);         // in the side panel; applies on change
let series = theme.accent(0);            // a plot colour that follows the theme
let ink = bugis::Ink::of_ui(ui);         // drawn chrome: bg, card, border, grid,
painter.rect_filled(rect, 0.0, ink.bg);  //   text, muted, warn, error, ok, backing
```

Since 2026-11-02 it is also **the workspace's central GUI crate**
(maintainer: "the gui portions, eg widget library and such, should then live
in bugis too"; gh:#1112). The GUI code of `outram-park-digital-twin-engine`
and `dhoby-ghaut` moves here, one step at a time, byte for byte, with re-exports at the old
paths so no call site changes. **CORRECTED 2026-11-05** (maintainer:
"widget art can live in bugis, the combination of logic and art can live inside the digital twin engine"): the widget library is split, the art here and the
physics-backed widgets in the engine; see below.

## Features

| Feature | Adds | Moved from | needs `std` |
|---|---|---|---|
| (none) | `Rgb`, `gruvbox`, `colour_maps`: the `no_std` + `alloc` core | | no |
| `egui` (default) | `Theme`, `selector`, `Ink`, `palette`, `gruvbox_visuals`, `colour_maps::color32` (the maps as `egui::Color32`) | `kovan` (visuals); engine `color_maps/` (wrappers) | no |
| `std` | turns off `no_std` (implied by everything below) | | yes |
| `animation` | `animation`: tracer, pebble-handling and control-rod-drive kinematics; uom only, egui-free, builds for Android | engine `animation/` | yes |
| `ascii` | `ascii`: 2-D schematic geometry on a character grid | engine `ascii/` | yes |
| `app_scaffold` | `app_scaffold`: `SharedState`, monitored physics threads, crash modal, CSV display and logging, frame metrics, plot history, real-time pacing. Turns on `egui`; adds `csv`, `web-time`, `thiserror` | engine `app_scaffold/` | yes |
| `web_demo` | `web_demo`: the tutorial tracks' browser-demo framework (main view, zoom buttons, folding panel, docks, worker/thread link, loading, lessons, platform, worker pool, nuclear-data cache; since gh:#1155 `field` (cell fields on mesh sections, Crameri colour bar) and `residual` (convergence history plot)). Turns on `egui`; adds `sha2`, and on wasm32 only `wasm-bindgen`, `wasm-bindgen-futures`, `js-sys`, `web-sys` | `dhoby-ghaut` `web_demo/` | yes |
| ~~`tampines`, `tampines-steam-tables`, `outram-park-fork-dwsim-libs`, `nee_soon`~~ | ~~the physics crate of the same name, as an optional dependency~~ (removed 2026-11-05) | | yes |
| `widgets` | `components`: the widget **art**, one `egui::Widget` per process object drawn from plain numbers. Turns on `egui` and `animation` ~~and all four physics features~~ (none since 2026-11-05) | engine `components/` (all but `excursion`; the physics-backed widgets went back, 2026-11-05) | yes |

~~`widgets` needs all four physics features because the module tree is one
unit and its base widget (`pipe.rs`) reads `tampines` state; splitting it by
physics crate would mean editing the moved files, not moving them. The
engine's `excursion` overlay stays in the engine: it reads the engine's cited
`htr10::design` fuel-temperature limits, which are physics data.~~

**CORRECTED 2026-11-05** (maintainers: "bugis should be standalone";
"widget art can live in bugis, the combination of logic and art can live inside the digital twin engine"): the widget library is split. The art
stays here: every widget that draws from plain numbers, and the drawing half
of each physics-backed widget (`PipeArt`, `CondenserArt`, `CoolingTowerArt`,
`HeatExchangerArt`, `PumpArt`, `SteamGeneratorArt`, `TurbineArt`, `ValveArt`,
`ReactorVesselArt`). The widgets that hold a `tampines`,
`tampines-steam-tables`, DWSIM or `nee_soon` component (`PipeVisual`,
`CondenserVisual`, ..., `PipeComponent`) moved to
`outram_park_digital_twin_engine::components` and draw through that art,
taking the four physics features and the `tuas_boussinesq_solver`
dev-dependency with them. `cargo publish`
resolves optional and dev dependencies, and the physics crates dev-depend on
bugis for their GUI examples' theme, so those edges were dependency cycles in
the lockstep release (`scripts/release-lockstep.sh`). bugis now depends on no
workspace crate except `petir`.

## Status

Implemented: `Theme` (Gruvbox Dark, the default, and Gruvbox Light), `apply`,
`selector`, `accent`, the full `palette`, `gruvbox_visuals`. ~~It depends on
`egui` only (never `eframe`), so it compiles for Android and wasm32.~~
**CHANGED 2026-11-02 (gh:#1112):** with its default features it still
depends on `egui` only (never `eframe`), so it compiles for Android and
wasm32. `--no-default-features` is `no_std`, and
`--no-default-features --features animation` builds for
`aarch64-linux-android` (checked 2026-11-02). ~~The `widgets` feature pulls the
physics crates and is checked on the host only.~~ (Since 2026-11-05 `widgets`
pulls no physics crate.)

Moved in (gh:#1112, all five steps): `animation`, `ascii`, the widget
library `components` (the art only since 2026-11-05), `app_scaffold` (from the engine) and `web_demo` (from
`dhoby-ghaut`). Each old path still works through a re-export. Not checked
here: the wasm32 build of `web_demo` (disk-limited session).

The visuals were moved here from `kovan`'s `src/app/theme.rs`; `kovan` and the
`tampines-steam-tables` GUI now call this crate instead of carrying copies.

~~Not done: the simulators' hard-coded drawing colours (plot series, schematics)
are not all routed through `Theme::accent`; only the egui chrome follows the
theme everywhere.~~ **UPDATED 2026-11-05:** `Ink` (theme-derived colours for
what a GUI draws itself) landed, and the ten `dhoby-ghaut` web demos and the
`web_demo` framework (`view::scale_bar`, `field::colour_bar`, `residual`,
`loading`) draw their chrome and plot series from it
(`crates/dhoby-ghaut/verification_and_validation/gruvbox_web_demos/`).
Physics data encodings (material palettes, colour maps, energy scales) stay
fixed on purpose. Not re-checked: the other egui simulators' drawing colours
(the engine's, `kovan`'s). Gruvbox Light's `warn_fg_color` is the faded yellow
since 2026-11-05 (the bright one was unreadable on `light0_hard`).

## Bookkeeping status

| Axis | Status |
|---|---|
| Human reviewed | ☐ |
| Human V&V | ☐ |

## Licence

AGPL-3.0-only. The Gruvbox palette is MIT (morhetz/gruvbox); see `NOTICE`.
