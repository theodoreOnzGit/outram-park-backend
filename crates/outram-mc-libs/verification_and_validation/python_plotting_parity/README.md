# OpenMC's Python plotting, ported as emitted matplotlib scripts — code-to-code V&V

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is unverified and
> untrusted unless a specific V&V case demonstrates otherwise. This record is one
> such case, for geometry plotting only. Not for nuclear facility operation,
> reactor control, safety-critical or licensing decisions.

Maintainer direction, 2026-09-26: *"exhaustively port over the openmc plotting
functions … for outram mc, the output is matplotlib scripts, which shld then
produce picture identical to the openmc python version. Do code to code
verification."*

OpenMC's C++ plotter (`openmc --plot`, `src/plot.cpp`) was already ported and
verified pixel-for-pixel in [`../geometry_plotting/`](../geometry_plotting/README.md).
This record covers the **Python** plotting functions: the Rust code computes
what OpenMC's C++ library would hand to Python (the id map, the cross sections,
the tracks) and writes a standalone `.py` that runs the transcribed upstream
numpy/matplotlib code on it.

| Upstream Python function | Rust | Record |
|---|---|---|
| `openmc.Model.plot` (`openmc/model/model.py:1345-1543`) | `geometry::plot::ModelPlot::emit` | [`model_plot/`](model_plot/) — this page |
| `Universe.plot`, `Cell.plot`, `Region.plot`, `Geometry.plot` (all delegate to `Model.plot`) | `emit_universe`, `emit_cell`, `emit_region`, `emit` | this page |
| `_id_map_to_rgb`, `SlicePlot.colorize`, `_set_plot_defaults`, `bounding_box` | inside `model_plot` | this page |
| `openmc.plot_xs`, `calculate_cexs`, `Track(s).plot` | see [`xs_and_tracks/`](xs_and_tracks/) | separate README |

## `Model.plot` — result

**17 cases, 546 553 pixels, 0 differing pixels; id maps identical in every
cell and material entry.** Measured 2026-09-26 on x86_64 Linux (glibc), OpenMC
0.16.1.dev25 at commit `d7d3284a1` (C++ library built from source with libpng;
Python 3.12.11), matplotlib 3.11.2, numpy 2.x, both sides run by the same
interpreter.

| Case | What it exercises | Pixels | Differing | Id-map diffs (cell / mat) |
|---|---|---|---|---|
| `godiva_default` | every default: bounding-box origin/width, `pixels=40000` split, `RandomState(1)` colours | 66 822 | 0 | 0 / 0 |
| `lattice_default` | defaults through a rect lattice; bounding box of an intersection | 66 822 | 0 | 0 / 0 |
| `lattice_material_colors_legend` | `color_by='material'`, RGB + SVG-name colours (mixed case), legend | 42 642 | 0 | 0 / 0 |
| `lattice_xz_seed_legend_mm` | x-z basis, `seed=` (`colorize` over `get_all_cells` order), legend, `axis_units='mm'` | 50 181 | 0 | 0 / 0 |
| `lattice_yz_outline` | y-z basis, `outline=True` (`contour`), `pixels` as a total | 37 442 | 0 | 0 / 0 |
| `lattice_outline_only` | `outline='only'` | 23 870 | 0 | 0 / 0 |
| `lattice_imshow_kwargs` | `**kwargs` to `imshow` (`alpha`, `interpolation`) | 23 870 | 0 | 0 / 0 |
| `universe_default` | `Universe.plot`; infinite bounding box → origin 0, width 10 | 16 641 | 0 | 0 / 0 |
| `cell_default` | `Cell.plot`; half-infinite box, `nan_to_num` origin | 10 609 | 0 | 0 / 0 |
| `overlap_show_yellow` | `show_overlaps`, SVG `overlap_color` | 16 641 | 0 | 0 / 0 |
| `overlap_material_seed_legend` | void cell, material `colorize`, `legend_kwargs` override | 16 641 | 0 | 0 / 0 |
| `region_union` | `Region.plot` of a union (auto-id void cell) | 16 641 | 0 | partition identical* |
| `godiva_source_scatter` | `n_samples` scatter, `plane_tolerance`, `source_kwargs` | 23 870 | 0 | 0 / 0 |
| `hex_seed_legend` | y-orientation hex lattice, `colorize` over hex `get_unique_universes` | 37 442 | 0 | 0 / 0 |
| `hex_x_seed_legend` | x-orientation hex lattice | 37 442 | 0 | 0 / 0 |
| `hex3d_xy_upper` | 3-D hex lattice, upper level | 37 442 | 0 | 0 / 0 |
| `hex3d_xz` | 3-D hex lattice, axial slice, material `colorize` | 26 448 | 0 | 0 / 0 |

\* `Region.plot` builds a cell with an auto-assigned id; the two codes give it
different ids (upstream's next free id, ours one past the largest). The id map
is compared as a partition, which is identical; the picture does not depend on
the id (one domain, first `RandomState(1)` draw).

### Negative controls — the comparison can fail

`tests/python_plot_parity.rs::harness_detects_small_perturbations`:

| Perturbation | Differing pixels |
|---|---|
| `seed=4` instead of 3 (`lattice_xz_seed_legend_mm`) | 24 036 / 50 181 |
| origin moved +0.02 cm, under half a pixel (`lattice_outline_only`) | 1 791 / 23 870 |
| hex rings rotated by one position (`hex_seed_legend`) | 5 638 / 37 442 |

## Methodology

**Reference side.** [`model_plot/openmc_inputs/make_references.py`](model_plot/openmc_inputs/make_references.py)
builds five small CSG models with OpenMC's Python API and calls OpenMC's own
`Model.plot` / `Universe.plot` / `Cell.plot` / `Region.plot`, then
`plt.savefig` with matplotlib defaults. It also saves the id map that
`Model.plot` coloured (`Model.slice_data`, same arguments). Outputs are in
[`model_plot/openmc_reference/`](model_plot/openmc_reference/). The models:
Godiva (one sphere); a 3×3 pin lattice in a slab reflector; two deliberately
overlapping spheres with a void cell; a three-ring hex lattice of three
distinct pin universes (orientation y and x); and the same hex lattice with two
axial levels. The materials carry U-234 as a placeholder nuclide: `openmc.lib`
reads `cross_sections.xml` and opens the file even in `-c` mode, but a plot
reads no data. The HDF5 was converted from the committed NJOY2016 ACE table
(`reference-data/ace/reference-njoy/endf-b-viii.0/293.6K/U234.ace.gz`) with
`openmc.data.IncidentNeutron.from_ace`.

**Our side.** `tests/python_plot_parity.rs` builds the same models with this
crate's types: cells in ascending-id order and materials in list order, which
is how OpenMC indexes them. It emits one script per case with `ModelPlot`, runs
it with the same Python, and compares PNGs with `geometry::plot::decode_png`.
[`model_plot/compare.py`](model_plot/compare.py) repeats the pixel comparison
with matplotlib's reader and compares the id maps.

**Pass criterion, fixed before the first run: 0 differing pixels in every
case.** The script runs upstream's numpy and matplotlib calls on the same id
map, so any difference is a defect.

Reproduce:

```bash
python make_references.py model_plot/openmc_reference     # needs openmc + OPENMC_CROSS_SECTIONS
OUTRAM_PYTHON=$(which python) OUTRAM_PLOT_SCRIPT_OUT=$PWD/model_plot/outram \
  cargo test --release -p outram-mc-libs --test python_plot_parity -- --nocapture
python model_plot/compare.py
```

## What the comparison found (both fixed; the pass criterion was not moved)

The first run matched 12 of 14 cases. The two misses were defects on our side:

1. **Overlap ids through `openmc.lib`.** `Model.plot` gets its id map from
   `openmc_slice_data`, which fills a `RasterData`. Its `set_overlap`
   (`src/plot.cpp:153-164`) writes `OVERLAP - overlap_idx - 1` (-4, -5, …) into
   the **cell** channel and -3 into the material channel. `overlap_idx` numbers
   the distinct overlapping pairs `{universe, cell a, cell b}`
   (`check_cell_overlap`, `src/geometry.cpp:38-90`). We had written -3 in both
   channels (the `IdData` behaviour of `openmc --plot`). Fixed in
   `model_plot::overlap_id_map`. **An upstream quirk, kept on purpose:**
   `_id_map_to_rgb` only colours -3, so with `color_by='cell'` upstream draws
   overlaps **white**, not in `overlap_color`. The port does the same. Only
   `color_by='material'` shows `overlap_color`.
2. **`HexLattice::from_rings` put ring elements on the wrong tiles.** OpenMC
   lists each ring **from the top, clockwise**. Since op-6tz.38 our
   `ring_slots` sorted a ring's tiles by angle **anticlockwise from +x**, and
   its own doc called the start and direction "best-effort, not verified". Any
   hex lattice with non-uniform rings was therefore rotated and mirrored
   relative to OpenMC. The fix ports upstream's own two-step path: the Python
   ring→row conversion (`HexLattice._repr_axial_slice_{x,y}`,
   `openmc/lattice.py:1625-1835`) feeds the already-ported C++
   `fill_lattice_{x,y}` (`src/lattice.cpp:555-660`), which reads that row text.
   Both orientations and the 3-D case now match OpenMC exactly (cases
   `hex_*`). **HTR-10 impact: none on its explicit-TRISO bed.** That bed builds
   the lattice from uniform placeholder rings and writes every tile by its
   `(a, b, level)` index (`nee_soon::htr10_rmc::core_model`). The homogenised
   `core_model::assemble` path and `bed_tile_levels` do feed ring order. Their
   Bresenham fuel/dummy pattern now lands on OpenMC's tile order; before, it
   landed on the rotated one. Existing tests pass unchanged (`lattice` unit
   tests 32/32; `hex_lattice_axial_frame`; notebook `hexagonal_lattice`).

## Scope, and what is not ported

- **`axes=`** (drawing into a caller's axes) is not ported: the script owns its
  figure. A caller can edit the emitted script.
- **`n_samples`.** Upstream samples source particles with OpenMC's own random
  stream through `openmc.lib`. The caller passes positions sampled by this
  crate (`ModelPlot::source_points`). The slab selection, unit scaling and
  `scatter` call are upstream's, and the parity case feeds both sides the same
  60 points.
- **Multi-group mode** (`model.py:1399-1406`) only changes how `openmc.lib` is
  initialised; nothing about the picture.
- **Overlap-pair numbering order.** Upstream numbers pairs in the order its
  OpenMP threads meet them. We use row-major order, which is upstream on one
  thread, and every thread count when only one pair is present (the tested
  case). With several distinct overlapping pairs and several threads, upstream
  itself is not deterministic.
- `ModelPlot::title` is an addition (upstream has no title); it is off by
  default and not used in the parity cases.

## Files

| Path | What |
|---|---|
| `model_plot/openmc_inputs/make_references.py` | reference driver (new work, GPL-3.0-only; OpenMC MIT notice in `../geometry_plotting/openmc_inputs/LICENSE.openmc`) |
| `model_plot/openmc_reference/*.png`, `*_idmap.npy` | what OpenMC's Python drew, and the id map it coloured |
| `model_plot/outram/*.py`, `*.png` | the scripts this crate emitted, and what they draw |
| `model_plot/compare.py` | pixel + id-map comparison |
