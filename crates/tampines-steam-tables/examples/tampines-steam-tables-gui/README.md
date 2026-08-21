# TAMPINES Steam Tables GUI — T-p, p-h, T-s and h-s diagrams

An `eframe`/`egui` tool for **figure generation and interactive inspection** of
the IAPWS-IF97 surface implemented by `tampines-steam-tables`. It is not a
simulator and contains no solver.

Implements [issue #26](https://github.com/theodoreOnzGit/outram-park-backend/issues/26),
plus the maintainer's extension from three tabs to **four** — the issue required
p-h and h-s and listed T-s as optional; T-p was added on top. Renamed from
`steam_table_plotter` to `tampines-steam-tables-gui` per the issue's branding
request.

## Running it

```bash
# interactive window
cargo run --release -p tampines-steam-tables --example tampines-steam-tables-gui

# headless: write every figure and CSV for all four diagrams, then exit
cargo run --release -p tampines-steam-tables --example tampines-steam-tables-gui -- --export-all

# self-checks, including the saturation-curve V&V gate
cargo test --release -p tampines-steam-tables --example tampines-steam-tables-gui
```

`--export-all` needs no display. Options: `--out-dir <DIR>`, `--samples <N>`
(default 400 samples per computed curve), `--help`.

## Top-level tabs

Three tabs across the top of the window (issue #26, 2026-08-21: *"Tab
selection should be on the top, similar to how htgr sim v1 does it"*):

* **Graph** — the original interactive plotter described below.
* **Evaluation** — double-click the plot to drop a state point on the
  current diagram; its full property set (density, specific volume,
  temperature, pressure, enthalpy, entropy, quality, Gibbs and Helmholtz
  free energy) is computed live and listed in the sidebar, copyable as one
  CSV block. Points persist per diagram, so switching diagrams does not lose
  them. A click that cannot be resolved to a valid IAPWS-IF97 state (out of
  range, or a T-p click landing in the degenerate two-phase collapse) is
  skipped with a status-bar warning, never fabricated.
* **Citations** — every literature/attribution source this GUI relies on
  (the tabulated steam-table data, Moody, Zaloudek, Marviken, Edwards-O'Brien,
  and the Gruvbox palette), in one place, read straight from the same
  provenance strings the sidebar tooltips and CSV export use — so it cannot
  drift from them.

## GUI controls (Graph tab)

The sidebar is sectioned as: **Diagram**, **Theme**, **Legend**, **Computed
curves**, **Custom lines**, **Reference / validation data**, **Axes and
resolution**, **Export**, **Status**. Longer explanations sit on a hover
tooltip over the section heading rather than in the sidebar body.

* **Theme** — Gruvbox Dark, Gruvbox Light. (The plain `egui`
  Light/Dark/System options were removed 2026-08-21 at the maintainer's
  request — they were buggy and redundant once Gruvbox covers both a light
  and a dark chrome.) Both variants build custom `egui::Visuals` from the
  palette below and apply immediately, live canvas included. The **exported
  figure** is a separate concern — see Export style below.
* **Legend** — Off / Compact (default) / Full. Compact merges a whole family
  of curves (all isotherms, all quality lines, every reference dataset, …)
  into one legend row; Full gives every curve its own row. The corner hover
  readout (below) shows the full property set regardless of which legend
  mode is active.
* **Isobars / Isotherms / Quality lines** — each has a multi-select dropdown
  (`N/… shown`, plus `all`/`none` shortcuts) restricting the checkbox to a
  chosen subset of the fixed default values, rather than always drawing every
  one at once. The tabulated-data crosses paired with a selected isobar or
  isotherm, and the separate "Tabulated data: IAPWS single-phase table" points
  layer, follow the same selection — enabling only the 100 bar isobar shows
  only its own tabulated rows, not the full 2 334-row table.
* **Hover coordinates** — hovering anywhere over the plot area shows a
  corner-anchored readout with the point's full thermodynamic state: `p`,
  `T`, `h`, `s`, density, specific volume, quality, Gibbs and Helmholtz free
  energy — not just the two axis coordinates. On a log-pressure axis the
  readout is always in bar, never the raw `log10` value the canvas plots
  internally. The Evaluation tab's plot shows the same readout.
* **Custom lines** — add an isobar, isotherm, isentrope, isenthalp, isochore
  or quality line at any value (not just the fixed defaults under Computed
  curves), via a slider plus a numeric input for precise entry. Each is
  computed live through this crate's own flashes (see *Custom-line physics*
  below) and included in CSV export with `custom_line_type` /
  `custom_line_value` / `custom_line_unit` columns. **Clear custom lines**,
  **Clear reference overlays**, **Clear all overlays** and **Reset plot** sit
  alongside the list.
* **Export style** — Light publication (default; white/black/grey — issue
  #26's own fallback choice), Current theme (mirrors whichever GUI theme is
  active, Gruvbox included), Dark, Gruvbox. Only the page background/ink/grid
  change; every plotted curve keeps its own colour in every style.
* **File-browser export** — PNG/PDF/SVG each open a save-file dialog
  pre-filled with `<diagram>.<ext>`; CSV and "all formats" open a directory
  picker (CSV always writes two files, so it has no single-file form). Built
  on [`egui-file-dialog`](https://crates.io/crates/egui-file-dialog) 0.13.0 —
  pure `egui`, no GTK or other native-dialog backend, and the only version
  pinned to `egui ^0.34.0`, matching this workspace's pin exactly.
* **Status bar** — updates on diagram switch, every layer toggle, custom-line
  add/remove/clear, reset-plot, an export finishing (or failing), and a
  warning if a custom line has no evaluable point anywhere in its sweep
  (dropped, never fabricated — same "never invent a value" rule the rest of
  this tool follows).

### Gruvbox provenance and licence

The Gruvbox colour palette is based on
[morhetz/gruvbox](https://github.com/morhetz/gruvbox), licensed under the MIT
License. Only the published hex colour values are reproduced (in
`examples/tampines-steam-tables-gui/theme.rs`); no source code from that
project is used.

### Custom-line physics

Isobar and isotherm reuse the same generators the fixed default set under
*Computed curves* uses. Isentrope and isenthalp sweep pressure through this
crate's own already-verified `(p,s)`/`(p,h)` flashes, so they cannot disagree
with a `(p,s)`/`(p,h)` lookup anywhere else in the crate. Isochore has no such
flash to reuse, so it bisects pressure at each temperature against the
forward single-phase volume dispatcher, then **verifies** the converged point
actually reproduces the requested volume to within 0.1 % before accepting it
— the two-phase dome makes a naive bracket-and-bisect unsound there (see the
doc comment on `curves::isochore` and its bisection helper for the failure
mode a test caught during development). A value with no achievable single-
phase solution anywhere in range is dropped, not approximated.

## Output

Into `crates/tampines-steam-tables/figures/property_validation/` — the directory
issue #26 suggests, resolved from `CARGO_MANIFEST_DIR` so it does not depend on
the working directory. Five files per diagram:

```text
<stem>.png                    raster figure
<stem>.pdf                    vector figure
<stem>.svg                    vector figure
data/<stem>_curves.csv        every computed curve, full state per row
data/<stem>_points.csv        every reference point, full state per row
```

with `<stem>` one of `tp_validation_coverage`, `ph_validation_coverage`,
`ts_validation_coverage`, `mollier_validation_coverage`.

The directory is **gitignored**: all twenty files regenerate in seconds from the
command above, and together they run to tens of megabytes. `git add -f` a
specific figure if it is wanted as a committed publication artifact.

The CSVs carry the **complete** state of every plotted point — pressure,
temperature, enthalpy, entropy, quality, and the reference mass flux where the
source dataset reports one — not just the two coordinates that were on the axes.
A CSV exported from the p-h tab can therefore be replotted as a Mollier diagram
elsewhere without going back to the source.

## What is computed and what is cited

This distinction is the point of the tool.

| | Source | Examples |
|---|---|---|
| **Curves** | computed **live** from this crate's IAPWS-IF97 routines on every rebuild | saturation dome, saturated liquid/vapour lines, quality lines, isobars, isotherms, region boundaries, critical and triple points, and any custom isobar/isotherm/isentrope/isenthalp/isochore added via the Custom lines section |
| **Scattered points** | **cited reference data**, verbatim from this crate's own test fixtures | tabulated single-phase/saturation tables, Moody, Zaloudek, Marviken, Edwards–O'Brien |

Full citations for every one of those datasets are on the **Citations** tab.
The tabulated single-phase and saturation tables are cited as:

> Wagner, W., & Kretzschmar, H. J. (2008). International steam tables:
> Properties of water and steam based on the industrial formulation
> IAPWS-IF97. Berlin, Heidelberg: Springer Berlin Heidelberg.

(labelled "Tabulated data" in the sidebar and legend, not "Wagner" — the
tabulated steam-table data itself predates that textbook, per the
maintainer's 2026-08-21 correction.)

Nothing plotted is invented, and a layer that cannot be honestly drawn is
**disabled with a stated reason** rather than filled in. Ten of the sixty-four
layer/diagram combinations are disabled, in two categories:

* **the data does not exist** — the Edwards–O'Brien GS-1 trace is a measured
  *pressure* history; no enthalpy, entropy or quality was measured, so it
  appears on the T-p diagram only;
* **the curve would be degenerate** — an isobar is a horizontal line on p-h, an
  isotherm is a horizontal line on T-s, and in T-p coordinates the whole
  two-phase region collapses onto the vapour-pressure curve, so the saturated
  liquid line, the saturated vapour line and all five quality lines would be
  five copies of one curve.

## Vapour quality is derived, not validated

Quality comes from the Region-4 lever rule, exactly as issue #26 specifies:

$$x = \frac{h - h_f(p)}{h_g(p) - h_f(p)}$$

Wagner and Kretzschmar do not independently tabulate quality for every $(p, h)$
state, so **quality here is a derived quantity, not an independently validated
property** of this implementation. Every figure that draws quality lines repeats
that in a footnote printed on the figure itself.

## Verification and validation

Run by `cargo test --release -p tampines-steam-tables --example tampines-steam-tables-gui`.
**38 tests, all passing as of 2026-08-21.**

The load-bearing one is `curves::saturation_curve_matches_the_wagner_steam_table`.

* **Methodology.** For each of the 220 rows of Kretzschmar & Wagner's published
  saturation table (carried in `reference_data::wagner`), evaluate the plotted
  saturation state at the tabulated saturation temperature and compare the
  computed $p_{sat}$, $h_f$ and $h_g$ against the tabulated values. Pass
  criterion: 0.5 % relative on $p_{sat}$; `max(0.5 %, 1 kJ/kg)` on $h_f$ and
  $h_g$ — the absolute floor exists because $h_f$ passes through zero at the
  triple point, where a relative tolerance is meaningless. Rows within 1 K of
  $T_c$ are skipped, since the crate documents that its Region 3 backward
  equations lose digits there.
* **Result (2026-08-20).** Passes over 209 checked rows, from 0 °C to within
  1 K of the critical temperature. The dome the tool draws is therefore the
  published saturation line to within those tolerances across the full
  sub-critical range.

The others cover the lever rule and its ordering inside the dome, the critical
point against the crate's published `S_C_KJ_PER_KG_K` constant (0.1 %), the
triple point against the table's 0.01 °C row, the availability table, isobar and
isotherm segmentation, axis transforms, Liang–Barsky clipping, the pen-up
convention, tick generation, glyph bounds, PDF cross-reference offsets, PNG
decoding, byte-reproducibility of the SVG, PDF and CSV exports, the Compact-
legend family groupings, the hover-formatter unit/log-axis round trip, that a
non-default export palette actually reaches the rendered PNG, that every
custom-line type builds a correctly-tagged layer, that isenthalp/isentrope
sweep cleanly through every region and report quality across the dome, and
that the isochore bisection's converged point reproduces the requested
specific volume to within 0.1 %.

## Two library defects this work surfaced

Both are recorded as beads and **worked around in the plotter only** — neither
is fixed in the library here.

* **`op-hidb` (P1).** `x_ph_flash`'s Region-3 saturated-liquid sub-region chain
  selects `v_tp_3y` for $T_{sat}$ in (646.483 K, 647.096 K), where that
  sub-region's IF97 validity band lies above the critical pressure. Measured at
  $T_{sat}$ = 646.503 K, $p_{sat}$ = 21.906 MPa: $h_f$ = −1.108e21 kJ/kg. The
  value is *finite*, so an `is_finite()` filter passes it. The plotter guards
  with an ordering check ($h_f \le h_g$, $s_f \le s_g$, $h_f \ge -1$ kJ/kg) and a
  neighbouring-sub-region fallback.
* **`op-l1tz` (P2).** `region_fwd_eqn_single_phase` classifies compressed liquid
  between 623.15 K and $T_c$ (at $p_{sat} < p < p_{B23}$) as Region 2, so it is
  evaluated through the vapour equations. Not quantified. The plotter stops
  sub-critical isobar liquid branches at 623.15 K, which shows as a deliberate
  break in the curve.

## Design notes

* **Export does not screenshot the canvas.** `egui_plot` draws to a GPU texture;
  a framebuffer grab is raster-only, depends on window size and display scale,
  and cannot run without a display. Instead a backend-independent `Scene` is laid
  out into two primitives — a stroked polyline and a filled polygon — and
  serialised three ways. All three formats are therefore the same figure, and a
  PNG preview is an exact preview of the PDF.
* **One new workspace dependency: `egui-file-dialog`.** PNG encoding uses
  `image`, already a root `[workspace.dependencies]` entry, and PDF/SVG need no
  crate at all — see below. The file-browser export controls (issue #26) are
  the one place this example genuinely needed something new: an in-app
  save-file/directory picker. `rfd` was tried first and rejected — it needs a
  GTK 3 backend on Linux, which is not something this workspace wants to
  require. `egui-file-dialog` 0.13.0 is pure `egui`, adds no native-toolkit
  dependency, and is the only published version pinned to `egui ^0.34.0`,
  matching this workspace's pin.
* **PDF and SVG still need no crate at all.** With every
  glyph reduced to a polyline by the example's own stroke font, a PDF page is a
  plain path stream with no font dictionary, so it is written directly (~80
  lines) and is byte-reproducible. `lopdf` was considered — it is already a
  workspace dependency — but it earns its keep on documents with fonts, images or
  annotations, and this figure has none.
* **One `egui_plot` difference to know about.** `egui_plot` has no log axis, so
  when the log pressure toggle is on the live canvas plots `log10(p / bar)` and
  says so in the axis label, while the exported figure draws a proper
  decade-ticked log axis. The numbers are identical.

## Android

The `egui`/`eframe` stack is Android-hostile, so this example follows
`examples/fhr_sim_v1/main.rs`: an empty `main` under
`#[cfg(target_os = "android")]`, everything else gated off Android. Verified
2026-08-20 with

```bash
cargo check -p tampines-steam-tables --all-targets --target aarch64-linux-android
```

which finishes clean — `--all-targets`, so it covers this example, not just the
library.
