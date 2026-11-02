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
```

Since 2026-11-02 it is also **the workspace's central GUI crate**
(maintainer: "the gui portions, eg widget library and such, should then live
in bugis too"; gh:#1112). The GUI code of `outram-park-digital-twin-engine`
moves here, one step at a time, byte for byte, with re-exports at the old
paths so no call site changes.

## Features

| Feature | Adds | Moved from | needs `std` |
|---|---|---|---|
| (none) | `Rgb`, `gruvbox`, `colour_maps`: the `no_std` + `alloc` core | | no |
| `egui` (default) | `Theme`, `selector`, `palette`, `gruvbox_visuals`, `colour_maps::color32` (the maps as `egui::Color32`) | `kovan` (visuals); engine `color_maps/` (wrappers) | no |
| `std` | turns off `no_std` (implied by everything below) | | yes |
| `animation` | `animation`: tracer, pebble-handling and control-rod-drive kinematics; uom only, egui-free, builds for Android | engine `animation/` | yes |
| `ascii` | `ascii`: 2-D schematic geometry on a character grid | engine `ascii/` | yes |
| `tampines`, `tampines-steam-tables`, `outram-park-fork-dwsim-libs`, `nee_soon` | the physics crate of the same name, as an optional dependency | | yes |
| `widgets` | `components`: the widget library, one `egui::Widget` per process object. Turns on `egui`, `animation` and all four physics features | engine `components/` (all but `excursion`) | yes |

`widgets` needs all four physics features because the module tree is one
unit and its base widget (`pipe.rs`) reads `tampines` state; splitting it by
physics crate would mean editing the moved files, not moving them. The
engine's `excursion` overlay stays in the engine: it reads the engine's cited
`htr10::design` fuel-temperature limits, which are physics data.

## Status

Implemented: `Theme` (Gruvbox Dark, the default, and Gruvbox Light), `apply`,
`selector`, `accent`, the full `palette`, `gruvbox_visuals`. ~~It depends on
`egui` only (never `eframe`), so it compiles for Android and wasm32.~~
**CHANGED 2026-11-02 (gh:#1112):** with its default features it still
depends on `egui` only (never `eframe`), so it compiles for Android and
wasm32. `--no-default-features` is `no_std`, and
`--no-default-features --features animation` builds for
`aarch64-linux-android` (checked 2026-11-02). The `widgets` feature pulls the
physics crates and is checked on the host only.

Moved in so far (gh:#1112): `animation`, `ascii`, and the widget library
`components` (step 3). Still to come: the engine's `app_scaffold/` and
`dhoby-ghaut`'s `web_demo/`.

The visuals were moved here from `kovan`'s `src/app/theme.rs`; `kovan` and the
`tampines-steam-tables` GUI now call this crate instead of carrying copies.

Not done: the simulators' hard-coded drawing colours (plot series, schematics)
are not all routed through `Theme::accent`; only the egui chrome follows the
theme everywhere.

## Bookkeeping status

| Axis | Status |
|---|---|
| Human reviewed | ☐ |
| Human V&V | ☐ |

## Licence

AGPL-3.0-only. The Gruvbox palette is MIT (morhetz/gruvbox); see `NOTICE`.
