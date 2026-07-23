# outram-mc-tui

> **Where this lives:** `outram-mc-tui` is a `[[bin]]` target **inside the
> `outram-mc-libs` crate** (`src/bin/outram-mc-tui/`), gated behind the crate's
> non-default **`tui`** feature — it is not a separate crate. Build/run it with
> `--features tui --bin outram-mc-tui` (see commands below). Because the feature
> is off by default, library consumers of `outram-mc-libs` (`tampines`,
> `nee_soon`, …) never pull the `ratatui`/`crossterm` terminal stack.

A mobile-first, touchscreen [`ratatui`](https://ratatui.rs) terminal UI over
[`outram-mc-libs`](..): pick a preconfigured geometry/material,
tune the run settings, launch a k-eigenvalue transport run, watch it converge
live, and inspect the tallied neutron spectrum overlaid with a cross section.

Part of the OUTRAM PARK backend workspace — see the root `README.md` /
`CLAUDE.md` for the project as a whole, and `RESPONSIBLE_USE.md` /
`DATA_POLICY.md` for the compliance rules this crate (like every crate here)
is bound by.

```bash
cargo run -p outram-mc-libs --features tui --bin outram-mc-tui --release
```

## Mobile-first, touch-first design (op-omf)

This crate targets a **phone terminal** (Termux on Android), not a desktop
terminal that happens to be narrow. Concretely:

- **Narrow single-column layout.** Every screen is `[header] [content]
  [footer]` stacked vertically — no side-by-side panes. A phone terminal is
  narrow, not short, so a horizontal split wastes the dimension there is
  plenty of.
- **Large tap targets.** Every tappable element (a geometry card, a stepper
  arrow, the RUN button) is a bordered block at least 3 terminal rows tall —
  a deliberately generous touch target for a fingertip, not a mouse pointer.
- **Touch is primary, keyboard is secondary.** The binary enables terminal
  **mouse capture** (`EnableMouseCapture`), which is how Termux delivers a
  touch tap (`MouseEventKind::Down(Left)` at the cell under the finger) and a
  drag/flick (`Drag` / `ScrollUp` / `ScrollDown`) to a terminal app. Every
  screen's draw function registers `(Rect, action)` hit regions as it lays
  out; a tap or scroll event is resolved against that list
  (`src/app.rs::App::tap` / `App::scroll`). Keyboard shortcuts (arrow keys,
  digits, `Enter`, `Esc`, letters) drive the *same* action enum as a
  convenience for a desktop terminal — there is only one command set, not a
  keyboard path and a separate mouse path that can drift apart.
- **Paginated, not pixel-scrolled, lists.** When the geometry-picker's four
  cards or the settings screen's rows do not all fit a short terminal, the
  list pages by whole item (card/row) rather than clipping mid-item — simpler
  to implement correctly and simpler to read on a small screen. Wheel-scroll
  or a vertical drag pages the list; see `app::App::scroll_active_list`.

### Android/Termux

Unlike `kovan-tui` (which target-gates `ratatui` off Android and stubs to a
CLI redirect on that target), **this crate declares `ratatui.workspace = true`
unconditionally** and runs as a real touch TUI on Android — a terminal
library is in-scope on Android per the workspace `CLAUDE.md` "Android
portability" rule; only windowing GUI (`egui`/`eframe`) is out of scope there.

The one Android-specific wrinkle is `outram-mc-libs`'s GPU compute
(`wgpu`), which *that* crate target-gates off Android
(`[target.'cfg(not(target_os = "android"))'.dependencies]`). This binary
needs no `cfg(target_os = "android")` of its own to handle that:
`outram_mc_libs::gpu::probe()` already has an Android-safe CPU-only shim that
always returns `None`, so every run transparently executes the CPU path on a
phone, exactly as it would on a desktop with no GPU adapter. The CPU
single-/multi-thread compute-backend options are both wired for every preset;
only the **GPU** option has no kernel for these geometries and falls back to
the multi-threaded CPU path — see "Known gaps" below.

Termux usage:

```bash
pkg install rust      # or your preferred Rust toolchain on Termux
cargo run -p outram-mc-libs --features tui --bin outram-mc-tui --release
```

Tap a card to pick a geometry, tap `+`/`-` to adjust a setting, tap `RUN` to
launch. Drag or use the (two-finger, if your terminal app supports it) wheel
gesture to scroll a list that overflows the screen.

## What it does

### 1. Preconfigured geometries (op-dyt)

Four touch-selectable cards (`src/presets.rs`):

| Card | Geometry | Driver |
|---|---|---|
| **Pebble bed** | HEU-metal kernels (r=0.04cm) RSA-packed to pf=0.30 in an H-1 matrix, reflective 1cm cube | delta (Woodcock) tracking |
| **LWR cell** | UO2 fuel / void gap / natural-Zr clad / light-water moderator, 1.26cm square reflective pitch (the openmc `pincell` notebook geometry) | general CSG |
| **TMSR-like pebble bed** | Same kernel dispersion, but a FLiBe-like (Li-7/Be-9/F-19) molten-salt matrix instead of hydrogen | delta tracking |
| **Bare-metal sphere** | Godiva (HEU) or Jezebel (Pu) — tap the small isotope toggle in the card's title bar to switch | single-cell CSG sphere |

Every nuclide is built with `Nuclide::from_core` — the embedded LOW-tier CORE
windowed-multipole + fast-MGXS library in `njoy-outram-park-fork`
(`docs/wmp-nuclide-manifest.md`), no network, no HDF5.

**Documented approximations** (this environment does not have every input a
fully faithful reproduction would need — see each preset's in-app `blurb()`
for the full text):

- **LWR cell** uses **free-gas hydrogen**, not the bound-atom S(alpha,beta)
  thermal-scattering table. `outram-mc-libs`'s own thermal-LWR-pin LIVE test
  (`tests/openmc_notebooks/pincell.rs::pincell_lwr_thermal_pin_benchmark`) is
  itself data-gated on an external ENDF/B-VIII.0 `tsl-HinH2O.endf` file this
  offline environment does not carry (`DATA_POLICY.md`: public data,
  referenced by path, never vendored) — this preset makes the same
  simplification for the same reason.
- **Pebble bed** uses an **H-1 matrix**, not graphite: `outram-mc-libs` has no
  graphite/carbon nuclide + S(alpha,beta) treatment yet (see that crate's
  `src/material/thermal.rs` module doc), and its own
  `examples/triso_delta_tracking.rs` makes the identical simplification.
- **TMSR-like pebble bed**'s FLiBe-like matrix uses **Li-7 only** (natural Li
  is ~92.5% Li-7; Li-6's much larger thermal absorption cross section is
  neglected). Atom densities are computed from a representative Li2BeF4
  formula-unit density (rho=1.94 g/cm3), not taken from a specific published
  salt-loop benchmark.
- **Jezebel** omits the real assembly's ~1 at% Ga alloying addition (Ga-69/71
  live in `njoy-outram-park-fork`'s EXTENDED nuclide set, not the embedded
  CORE library this crate reads from) and its Pu-239/240/241 atom densities +
  radius are reproduced from commonly published reactor-physics literature
  values that were **not independently re-verified against the primary
  ICSBEP handbook in this session** — treat Jezebel as illustrative, not
  benchmark-grade, unlike Godiva (whose densities mirror this crate's own
  Godiva verification tests in `outram-mc-libs`).

None of these approximations are hidden: every one is stated on the relevant
preset's card in-app (`GeometryPreset::blurb`) as well as here.

### 2. Run settings (op-4h8)

The Settings screen (`src/settings.rs`, `src/ui.rs::draw_settings`) exposes
exactly what the bead asked for: compute backend (CPU-single / CPU-multi /
GPU), histories-per-generation, inactive generations, active generations
("batches"), and the RNG seed, each as a tap `+`/`-` stepper. `GPU` queries
`outram_mc_libs::gpu::probe()` live and shows whether an adapter was found;
if not, the crate's own documented contract is that it transparently falls
back to CPU (never an error).

**The default histories/generation counts are deliberately tiny** (30
histories x [1 inactive + 2 active] generations — see
`RunSettings::default`'s doc comment). This is sized to the *slowest* preset
this crate ships, not the fastest: the **LWR cell** preset's free-gas
hydrogen (no thermal cutoff — see its documented approximation above) makes
a neutron's slowing-down random walk take many thousands of collisions per
history in a zero-leakage reflective cell, measured at roughly 50ms per
(history x generation) on the development machine used to build this crate —
over 90 seconds at `outram-mc-libs`'s own 400x45-generation default scale.
Raising the steppers gives a tighter (much slower) estimate; the bare-sphere
and pebble-bed presets tolerate far larger settings quickly, since they do
not share the LWR cell's moderator physics.

**Compute-backend dispatch (op-fla — CPU wired, GPU falls back).** All three
of `outram-mc-libs`'s transport entry points now dispatch on `ComputeType`.
Every preset in this TUI runs through `run_keff_csg` (bare sphere, LWR cell —
chosen so those two can carry the op-iom spectrum tally) or `run_keff_delta`
(pebble beds), and both now honour the selector:

- **CPU (single-thread)** — the deterministic, bit-reproducible reference path.
- **CPU (multi-thread)** — genuinely transports each generation's histories in
  parallel across all cores with `rayon` (dedicated pool, per-history jump-ahead
  seeding so the result is reproducible independent of thread count). This is a
  real, selectable speed-up. Backend agreement is covered by V&V tests
  (`csg_multithread_agrees_with_single_thread` and
  `delta_multithread_agrees_with_single_thread`): the parallel result is
  thread-count-invariant to the bit and agrees with the single-thread reference
  within combined statistical uncertainty.
- **GPU** — the crate's GPU transport kernel exists only for the bare-sphere
  driver (`physics::keff::run_keff`), which no preset here routes through. For
  CSG and delta-tracked geometry there is **no GPU kernel yet**, so selecting
  GPU transparently runs the **multi-threaded CPU** path instead of erroring
  (an honest fallback, logged via `log::debug!`). The Settings screen says so
  directly under the compute row (`App::compute_dispatch_note`) and the live
  GPU-availability line. Wiring a genuine GPU Sigma_t lookup into CSG/delta
  transport is the remaining follow-up tracked as bead **op-fla**.

### 3. Live convergence (op-4h8)

Tapping RUN launches the transport run on a **background thread**
(`src/transport.rs::RunHandle`), so the UI keeps polling touch/keyboard input
and redrawing a busy spinner + elapsed-time clock for the whole run — it never
freezes. Once the thread finishes, the run screen **animates revealing** the
already-computed per-generation k trace a few points per redraw tick onto a
`ratatui` `Chart`, rather than popping the whole curve in at once.

Read this as what it honestly is: none of `run_keff_csg`, `run_keff_delta`,
or `run_keff` expose a per-generation progress callback in this
`outram-mc-libs` version — a whole run is one blocking call that returns
`KeffResult::k_by_generation` only at completion. So "live" here means (1) a
genuinely non-blocking UI during the run, and (2) an honest reveal-animation
of real, already-computed data — not continuous per-generation streaming from
the transport loop itself. See `src/transport.rs`'s module doc for the full
statement.

### 4. Neutron spectrum + cross-section overlay (op-iom)

The Spectrum screen (`src/spectrum.rs`, `src/ui.rs::draw_overlay_chart`)
tallies a 50-bin log-energy (1e-3 eV .. 20 MeV) track-length flux spectrum
with `outram_mc_libs::tally::filter::EnergyFilter` + `Tally` — the same
construction as `outram-mc-libs`'s own
`tests/openmc_notebooks/flux_spectrum.rs` — and samples the primary
material's macroscopic total cross section (`Material::macro_xs_total`) at
each bin's midpoint energy, so flux and Sigma_t share one log-E x-axis. Both
series are drawn on a `ratatui` `Chart` (own implementation — the njoy XS
log-log widget from the parallel op-1lj effort is a different crate and
cannot be imported cross-crate), each normalized to its own peak so they
overlay legibly despite having unrelated units and magnitudes.

**Availability is preset-dependent, and this is disclosed, not silent:**
`run_keff_csg` is the only driver in this crate version that accepts an
optional `Tally`, so only the two CSG-backed presets — **Bare sphere** and
**LWR cell** — carry a spectrum. `run_keff_delta` (the two pebble-bed
presets) has no tally parameter at all; the Spectrum screen for those shows
an explanatory message instead of a chart (`GeometryPreset::supports_spectrum`
gates this, checked before showing the nav button and again on the screen
itself).

## Architecture

```
src/
  main.rs        entry point: terminal init/restore incl. mouse capture, crossterm event loop
  lib.rs         re-exports the modules below (so tests/smoke.rs can drive them headlessly)
  app.rs         App state machine, Screen enum, UiAction enum, hit-region dispatch
  presets.rs     GeometryPreset enum -> concrete Geometry/Material/Nuclide (op-dyt)
  settings.rs    RunSettings + stepper increment/clamp logic (op-4h8)
  transport.rs   RunOutcome, background-thread RunHandle, run_case driver (op-4h8)
  spectrum.rs    energy-grid tally + cross-section sampling (op-iom)
  ui.rs          one draw function per screen; the only place ratatui widgets are built
tests/
  smoke.rs       headless TestBackend render + tiny (few-history) transport-run checks
```

No `Box<dyn Trait>` dispatch and no lifetime parameters anywhere in this
crate, per the workspace `CLAUDE.md` Rust design rules — screens are plain
functions over `&mut App`/`Frame`, presets are a closed enum, and the only
`Box` in sight (`Box<dyn Filter>` inside a `Tally`) is `outram-mc-libs`'s own
pre-existing type, used as-is, not introduced by this crate.

## Testing

```bash
cargo test -p outram-mc-libs --features tui --bin outram-mc-tui --release
```

`tests/smoke.rs` is a headless smoke test built on `ratatui::backend::TestBackend`
(there is no interactive terminal to click through in CI): it renders each
screen and asserts the expected text/tap-targets appear, and runs every
preset once with a tiny history count (20 histories x [1 inactive + 2 active]
generations) to check the build+run path doesn't panic and returns a finite,
positive k. It is **not** a physics verification suite — that lives in
`outram-mc-libs/tests/openmc_notebooks/`, which this crate's preset atom
densities/geometry deliberately mirror.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
