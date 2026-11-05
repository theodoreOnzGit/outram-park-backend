# Gruvbox theme across the web demos (2026-11-05)

Maintainer request, 2026-11-05: *"make sure all web demos have consistent
gruvbox themes from bugis"*. Presentation only: no physics, no numbers changed.
AI-drafted; untrusted until a human has looked at the pictures below.

## What changed

- **`bugis::Ink`** (new, `crates/bugis/src/gui.rs`): the colours a GUI *draws*
  itself, read from the ui's `egui::Visuals` and the theme's `gruvbox::Roles`:
  `bg` (main view, `extreme_bg_color`), `card`, `border`, `grid`, `text`,
  `muted`, `warn`, `error`, `ok`, `backing` (translucent, for labels over a
  picture) and `accent(i)` (`Theme::accent`). `Ink::of(&visuals)`,
  `Ink::of_ui(ui)`, `Ink::of_ctx(painter.ctx())`. Pinned by
  `gui::tests::ink_follows_the_theme`.
- **Gruvbox Light's `warn_fg_color`** is now the faded yellow `#b57614`
  (`Roles::warning`); the bright yellow it had was unreadable on
  `light0_hard`. This also changes kovan's and every other simulator's light
  warnings.
- **The shared `web_demo` framework** in bugis draws from `Ink`: the scale bar
  (`view::scale_bar`), the colour bar's frame and labels (`field::colour_bar`),
  the residual plot (`residual::paint`; its series are `accent(i)`, the
  hardcoded `residual::COLOURS` is gone) and the loading card and grid
  (`loading`).
- **All ten demos** (`examples/*_web/`): every demo already applied
  `bugis::Theme::default()` at start-up and had `bugis::selector` in its side
  panel (verified by grep; `fukushima_web` applies it in `main.rs`, on both
  the native and the web path). What changed is the hand-painted part: the
  `const BG = (14, 16, 20)` main views, plot cards, axes, grids, tick labels,
  status lines, warnings, errors and plot series now come from `Ink`, so
  Dark/Light switches every drawn view the same way.
- **Loading screens** (`web/*/index.html`, and `monte_carlo`'s
  `triso-pebble-redirect.html`): Gruvbox Dark0_hard `#1d2021` with light1
  `#ebdbb2` text, dark2 `#504945` borders, bright blue `#83a598` links and
  spinner, bright red `#fb4934` errors, dark0 `#282828` for the References
  panel.

## What was left alone, on purpose

Physics data encodings keep fixed colours in both themes (each is a legend a
reader or a review image relies on):

- material palettes: the HTR-10 slice palette (`nee_soon::htr10_rmc::plots`,
  via `raster.rs` and `RasterInfo::palette`), the TRISO layer colours
  (`delta_tracking_web`, `monte_carlo_web/triso`, `dhshort/model.rs`,
  `triso_atops_web::REGIONS`), the rung materials (godiva, jemima, hst009,
  natu, lct008, ugraphite, lumped);
- colour maps: Crameri in `web_demo::field` (Foam Studio), the helical coil's
  temperature and quality maps (`bugis::colour_maps`), the δ heat map in
  `double_pipe_twin_web::delta_colour`, the dispersion concentration ramp;
- the neutron energy colour scale of the tracks (`anim::energy_colour`) and
  the track/collision markers drawn on the geometry, with their legend;
- marks drawn ON such a picture keep fixed light colours so they read against
  it: the delta-tracking tracks, the Dean arrows ("white arrows"), the δ-map
  dots and rings, the rod-percentage labels on the HTR-10 top slice, the
  Fukushima plant schematic (its building fill encodes the hydrogen state),
  the lumped/lct008/ugraphite/triso labels drawn over their own dark vacuum or
  helium fills, the packing rung's particle categories.

One structural change for the theme: `raster.rs` now colours the OUTSIDE code
transparent (it was `(14, 16, 20)`), so the page's own background shows round
a geometry slice.

Not touched, as asked: `monte_carlo_web/htr10/core/pour.rs` and
`.../htr10/core/screen.rs`. To keep them working, `keffplot`'s named colours
(`LIVE`, `AMBER`, `RED`, `STATUS`, `REFERENCE_COLOUR`, `RECORD_COLOUR`, …)
stay as constants; `keffplot::themed` maps each to its theme colour when the
k plots, text pane and strip draw it (pinned by
`keffplot::tests::named_colours_take_the_theme`), so `screen.rs`'s status
lines already follow the theme. Its own hardcoded colours do not yet.

Also not touched: the HTML "References" button and panel float above the
canvas and stay Gruvbox Dark in the Light theme (they are page HTML, not
egui); the shared password gate
`crates/outram-park-digital-twin-engine/web/gate.js` (fukushima's loading
gate) still uses the old dark palette, because the CIET and HTGR demos of
another crate share it.

## What was checked by eye

Headless Chromium 1194 (`--no-sandbox`, SwiftShader WebGL), each demo built
with its own `build.sh` from this change and served by `python3 -m
http.server`; Dark is the start-up state, Light is one click on "Gruvbox
Light" in the side panel.

| image | demo | width | what I checked |
|---|---|---|---|
| `dem_dark.png`, `dem_light.png` | DEM pour, live | 1280 × 800 | background, plot cards, text and series swap; warning "settling…" readable in both (faded yellow on Light); pebbles grey with background-coloured gaps; vessel wall and valve follow the theme |
| `helical_sg_dark.png`, `helical_sg_light.png` | helical steam generator | 1280 × 800 | three profile plots, legends, dryout marker (error colour); the coil and its colour bar keep the physics maps |
| `helical_sg_phone_dark.png` | same | 390 × 844 | panel folded, plots stacked, readable. The "hot in (falls)" label overlaps the status lines at the top: a layout matter that predates this change |
| `double_pipe_twin_dark.png`, `double_pipe_twin_light.png` | double-pipe twin | 1280 × 800 | readout lines, legend, bands and CFD point legible in both |

~~Not looked at in a browser: delta tracking, dispersion, Foam Studio,
Fukushima, Monte Carlo, nuclear data and TRISO-ATOPS (several need the
published ENDF data or a long run). They compile for wasm32 and their tests
pass (see the hand-off), but nobody has looked at them in Light.~~
**CHECKED 2026-11-05, the same day.** The seven were opened in a browser, with
the same setup as above, at 1280 × 800, after 15 s, then with one click on
"Gruvbox Light". No page errors were logged. Found and fixed in the same
change:

- **Foam Studio's theme selector** was at the bottom of its long side panel,
  so the click missed it. It now sits under the heading, as in every other
  demo.
- **The bar behind a bottom dock's tab** showed the canvas's clear colour,
  near-black even in Light (Foam Studio's "Convergence"). `Dock::toggle` now
  paints the header in the theme's panel colour, which fixes every demo with
  a dock.
- **The HTML References button and panel**, and the page behind the canvas,
  stayed dark in Light. `web_demo::platform::page_theme` (called from
  `Panel::show`) now sets `data-theme` on `<html>` when the theme changes.
  Every `web/*/index.html` has `:root[data-theme="light"]` rules for them.
  Re-checked on TRISO-ATOPS.
- **Fukushima's password gate** (`outram-park-digital-twin-engine/web/gate.js`,
  shared with the CIET and HTGR demos) had its own dark palette. It is now
  Gruvbox Dark. The demo behind the gate was not opened: there is no
  password here.

| image | demo | what I checked |
|---|---|---|
| `delta_tracking_{dark,light}.png` | delta tracking (loading) | panel, processing list, loading card with progress bar and warning text follow the theme |
| `dispersion_{dark,light}.png` | dispersion, plume rung | panel, plume on the theme background; the concentration ramp is the data map and stays; source and arrow follow the theme |
| `foam_studio_{dark,light}.png` | Foam Studio, pipe | the selector at the top, mesh edges, axes, dock tab and bar, plot cards, after the two fixes |
| `fukushima_{dark,light}.png` | Fukushima | only the password gate (old palette in these shots, taken before its fix) |
| `nuclear_data_{dark,light}.png` | nuclear data, RECONR | σ(E) series as accents, grid, axes, legend, panel table |
| `triso_atops_{dark,light}.png` | TRISO-ATOPS, particle | layer colours are the material encoding and stay; legend, scale bar and the References button follow the theme (light shot after the page fix) |
| `monte_carlo_{dark,light}.png` | Monte Carlo, Godiva (loading) | panel, loading card, recorded tracks; the sphere's fill is the material colour and stays |

