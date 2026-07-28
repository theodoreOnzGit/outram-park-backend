# njoy-tui

> **Where this lives:** `njoy-tui` is a `[[bin]]` target **inside the
> `njoy-outram-park-fork` crate** (`src/bin/njoy-tui/`) — it is not a separate
> crate. `ratatui` is an unconditional dependency, so the binary always builds
> with a plain `cargo run -p njoy-outram-park-fork --bin njoy-tui` (no feature
> flags; see commands below).

A **mobile-first, touchscreen** [`ratatui`](https://ratatui.rs) terminal
browser for [`njoy-outram-park-fork`](..) — a
JANIS-like nuclide / cross-section viewer, built for a **phone terminal
(Termux) first, desktop terminal second**.

This is new tooling on top of `njoy-outram-park-fork`, not a port of any
upstream NJOY component. It implements three OUTRAM PARK beads:

| Bead | Feature |
|---|---|
| `op-ixe` | Fuzzy nuclide finder — type-to-filter over the embedded nuclide catalog |
| `op-1lj` | Cross-section log-log plot — sigma(E) vs E, braille chart |
| `op-dzl` | Temperature input in degC/Kelvin -> Doppler-broadened sigma(E,T) |
| `op-omf` | Shared mobile-first / touchscreen interaction layer both of the above sit on |

## Running it

```bash
cargo run -p njoy-outram-park-fork --bin njoy-tui --release
```

On Termux (native `aarch64-linux-android`, no NDK cross-toolchain needed —
see "Android/Termux" below):

```bash
pkg install rust
cargo run -p njoy-outram-park-fork --bin njoy-tui --release
```

Tap a row in the nuclide list to open it, or type to filter first. On the
plot screen, tap the MT buttons / unit toggle / +/- temperature buttons, or
tap "< back" to return to the finder. `Ctrl+C` quits from either screen.

## Mobile-first / touchscreen design (op-omf)

This crate is deliberately **not** built the way `kovan-tui` (this
workspace's other ratatui app) is built. `kovan-tui` is desktop-scope and
compiles to a CLI-redirect stub on Android; **this crate's entire purpose is
to be the touch-driven terminal app that runs on a phone**, so it declares
`ratatui.workspace = true` as a normal (non-Android-gated) dependency and is
checked against `aarch64-linux-android` like any other non-GUI crate in this
workspace — a terminal UI is in-scope for Android per the root `CLAUDE.md`
("Android portability"); only windowing GUI (`egui`/`eframe`) is out of
scope, and this crate has neither.

Concrete design choices, in the order op-omf lists them:

- **Single column, narrow-first layout.** Every screen is one vertical stack
  of full-width bands (title -> controls -> content -> footer). The exact
  layout functions the renderer uses (`ui::finder_layout`, `ui::plot_layout`,
  `ui::mt_button_rects`, `ui::temp_button_rects`) are pure functions of the
  terminal `Rect` — they are called again, unchanged, by the mouse handler to
  hit-test a tap against the same geometry that was just drawn. There is no
  separate "remembered widget rects" cache to go stale.
- **Narrow-screen adaptation.** The MT-channel selector (4 buttons) renders
  as one row of 4 when the terminal is at least `ui::MT_WIDE_MIN_WIDTH` (40)
  columns wide, and drops to 2 rows of 2 below that — so a button never
  shrinks below a tappable width, it just wraps (see
  `ui::tests::mt_buttons_cover_all_four_channels_wide_and_narrow`, which
  exercises 20/40/80-column terminals).
- **Mouse capture is enabled unconditionally** (`crossterm::EnableMouseCapture`
  in `main.rs`) because Termux maps a touchscreen tap/scroll to a crossterm
  mouse event:
  - **Tap** -> `MouseEventKind::Down(MouseButton::Left)` — selects a list row
    (finder) or presses a button (plot screen: back / unit toggle / +/-
    temperature steppers / MT selector).
  - **Two-finger scroll / mouse wheel** -> `ScrollUp`/`ScrollDown` — scrolls
    the finder's result list, or cycles the MT channel on the plot screen.
  - **Held-finger drag** -> a stream of `MouseEventKind::Drag` events — the
    finder turns consecutive drag rows into a scroll delta
    (`ui::finder::handle_mouse`), so a phone user can flick-scroll the result
    list the way any native list view works.
- **Keyboard is a secondary path, not the primary one.** Every control that
  can be tapped can also be reached from the keyboard (arrow keys / Enter /
  Tab / Esc — see `ui::finder::handle_key` and `ui::plot::handle_key`), but
  the touch path never depends on it: the temperature control, for example,
  is a set of tap-able +/-1 / +/-10 steppers precisely so a touch-only
  session (no physical keyboard) can still reach any temperature, not just
  type one.

## Data path — why the embedded CORE set, not a live ENDF-tape reconstruction

`njoy-outram-park-fork` *can* reconstruct pointwise cross sections from a raw
ENDF tape via `njoy_outram_park_fork::interface::NuclearDataLibrary`
(`from_file` -> `reconstruct` -> `broaden`), but that path reads a tape file
from disk and re-runs RECONR's adaptive energy-grid refinement — not
something to redo on every keystroke of an interactive touchscreen session,
and the TUI has no bundled tape to point it at anyway.

Instead, `src/xsdata.rs` wires to the crate's **representative in-crate data
path**, which is fast enough to recompute on every tap:

- **Resonance/thermal range** — the embedded windowed-multipole CORE library,
  [`njoy_outram_park_fork::wmp::WmpLibrary::core`]
  (`njoy-outram-park-fork/src/data/wmp_core.wmpl`; 125 reactor-grade + LFTR
  nuclides, ENDF/B-VII.1, MIT CRPG). `WindowedMultipole::evaluate(e, T)`
  gives the analytic Doppler-broadened sigma(E,T) directly — this is what
  makes `op-dzl`'s temperature control interactive rather than a multi-second
  batch job.
- **Fast range** (above each nuclide's WMP `e_max`, up to 20 MeV) — the
  matching fast-MGXS fallback,
  [`njoy_outram_park_fork::nuclear_data::MgxsLibrary::core`]. This tier is a
  coarse, **non-self-shielded**, temperature-independent 10-group constant
  set (Watt-spectrum-weighted) — a deliberate low-fidelity/high-speed
  fallback per its own doc comment, not a pointwise reconstruction.

`src/xsdata.rs::NuclideXs` combines the two: below a nuclide's WMP `e_max`
the plot shows the real Doppler-broadened evaluation; above it, the coarse
fast-range fallback (still real data, just lower fidelity — see the module
doc comment on `NuclideXs` for the exact trade-off).

### What this means for accuracy

This is a genuine fidelity trade the task brief explicitly allows
("if full reconstruction from a tape is too heavy for an interactive TUI,
use a representative in-crate data path and document the choice"):

- The resonance/thermal region (the part `op-dzl`'s temperature control
  actually demonstrates) is the same physics-faithful windowed-multipole
  evaluation `outram-mc-libs` uses for transport — see
  `njoy-outram-park-fork`'s own Doppler verification test
  (`tests/u238_doppler_verification`) for its validation status.
- The fast region above WMP's range is intentionally coarse (10 groups, no
  self-shielding) — good enough to see the overall shape of sigma(E) on a
  terminal-resolution log-log plot, not for a quantitative fast-flux
  calculation.
- ν̄ is not exposed as a plottable channel at all: the CORE set carries no
  ENDF MF=1/452 tape to derive a real ν̄(E) from, and showing an
  always-zero "nu_fission" channel would be actively misleading (see the
  module doc comment on `xsdata.rs`).

## MT channels

Only the four base channels [`njoy_outram_park_fork::nuclear_data::MicroXs`]
actually carries from this data path: **total** (MT=1), **elastic** (MT=2),
**fission** (MT=18), **capture** (MT=102). A channel that is exactly zero
everywhere for the selected nuclide (e.g. fission for a non-fissile nuclide)
renders as an explicit "no data" message rather than an empty chart.

## Known gaps / not implemented

- **No pan/zoom on the chart.** The plot always shows the nuclide's full
  `[e_min, e_max]` range; touch-drag/pinch panning or zooming into a
  narrow resonance is not implemented in this first pass.
- **No free-text temperature entry via touch.** The temperature stepper
  buttons (+/-1, +/-10) are the touch-only path; typing an exact number is
  not available even from the keyboard in this pass (the keyboard path is
  arrow-key nudges of the same step sizes, plus Tab/`u` to toggle the unit).
- **Sampling is a fixed 400-point log-spaced grid** per redraw
  (`app::PLOT_SAMPLES`), not RECONR's adaptive resonance-aware refinement —
  a very narrow resonance can be visually under-resolved. See the doc
  comment on `xsdata::sample_log_log`.
- **The catalog is the embedded WMP CORE set only** (125 nuclides) — it does
  not reach the `EXTENDED` sibling-crate set or a user-supplied ENDF tape.

## Layout of this crate

```text
src/
  main.rs        entry point: terminal setup (raw mode, alt screen, mouse
                 capture), the event loop, and screen dispatch
  app.rs         App / Screen / FinderState / PlotState — no trait objects,
                 an enum for the two screens
  elements.rs    Z <-> element-symbol table (display/search plumbing only)
  nuclides.rs    NuclideCatalog + the hand-rolled fuzzy matcher (op-ixe)
  temperature.rs TempUnit (degC/K) + conversions (op-dzl)
  xsdata.rs      NuclideXs: WMP + fast-MGXS combined sigma(E,T) (op-1lj/op-dzl)
  ui/
    mod.rs       shared mobile-first layout functions (op-omf)
    finder.rs    finder screen render + touch/keyboard input
    plot.rs      plot screen render + touch/keyboard input
  smoke_test.rs  #[cfg(test)] TestBackend smoke tests (see below)
```

## Testing

`njoy-tui` is a `[[bin]]`-only crate (no library target), so its unit tests
live inline in each module and run via:

```bash
cargo test -p njoy-outram-park-fork --bin njoy-tui --release
```

Because there is no way to drive the touchscreen/mouse loop interactively
from an automated test, `src/smoke_test.rs` renders real frames — through
the exact `draw` function `main`'s event loop calls — into an in-memory
`ratatui::backend::TestBackend` buffer (at both a narrow "phone" size and a
desktop size) and asserts the finder and plot screens actually put the
expected content on screen, including that the narrow-terminal MT-selector
wrap renders without panicking.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
