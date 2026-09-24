# Geometry slice plotting — verified against OpenMC's own rasteriser

**Date:** 2026-09-24
**Issue:** [gh:#268](https://github.com/theodoreOnzGit/outram-park-backend/issues/268)
**Code:** `src/geometry/plot.rs` (`Slice`, `ColourBy`, `sample_slice`, `emit_python`)
**Test:** `tests/geometry_plot.rs` — 4 tests.

## Result

**Pixel-for-pixel identical to OpenMC on all 6561 pixels of a Godiva slice**, with
zero disagreements even at the surface.

| | pixels inside | fraction |
|---|---|---|
| this crate (`sample_slice`) | **2733** | 41.66 % |
| OpenMC `--plot` | **2733** | 41.66 % |
| analytic `pi r^2 / (24 cm)^2` | — | **41.67 %** |

81 x 81 pixels, Godiva at `r = 8.7407 cm` (ICSBEP HEU-MET-FAST-001, the radius
this crate already uses) in a 24 x 24 cm window, `xy` basis through the origin.

## Methodology

`openmc --plot` rasterises the same slice of the same geometry and writes a PNG;
`openmc_inputs/plot_godiva_slice.py` reads it and dumps a **0/1 cell mask**
(white = no cell). The Rust test builds the identical sphere, samples the slice
through the crate's own `locate()` path, and compares
`index >= 0` against that mask.

**A mask rather than an image diff**, because OpenMC assigns cell colours
arbitrarily — comparing RGB would compare a palette. What both codes must agree
on is which points are in the model, which is the question a geometry plot
exists to answer.

**A disagreement is forgiven only at the boundary.** A differing pixel counts as
rounding only if its centre lies within half a pixel diagonal (0.2095 cm) of the
sphere surface; anywhere else it is a real geometry difference, and the test
prints the pixel, its coordinates and its distance from the surface. "A few
pixels differ" is exactly the report that hides a systematic offset, so the two
populations are separated rather than summed. **Both came out zero.**

Both sides are additionally checked against the analytic circle fraction, so a
systematic offset common to both would still fail.

**`openmc --plot` needs no cross-section library** — it only locates cells. None
is installed here, which is why this cross-code comparison was runnable when the
continuous-energy ones were not.

## Against #268's acceptance

| item | state |
|---|---|
| a slice of a core model compared against `openmc.Plot` output | **done, and stronger than asked** — pixel-for-pixel rather than visual |
| the emitted script runs on a bare `python3` with only matplotlib + numpy | **done** — `the_emitted_script_runs_on_a_bare_interpreter` runs it and it writes a 26 917-byte PNG. It searches for an interpreter that can import both and **skips** if none has them: `python3` on this machine does not have matplotlib, `/opt/ompy/bin/python` does |
| a regression test that the script is valid Python and the index array matches `locate()` | **done** — `the_index_array_is_what_locate_returns` checks 1681 pixels; running the script is itself the syntax check |

Scope items 1–4 are in (slice sampler, colour by cell/material/universe,
data embedded in the script, metadata header). Item 5, emitting the equivalent
`openmc.Plot` call for the same slice, is referenced in the module but is not
what this comparison used — the reference here is a committed driver script.

## Limitations

1. **One geometry, one slice, one basis.** A sphere is convex and has a single
   surface; it cannot expose a lattice-indexing or nested-universe error. HTR-10
   and the FHR pebble models are the cases that would, and neither is compared
   here — the acceptance text offered a choice of three and this takes the
   simplest.
2. **A cell mask, not cell identity.** With one cell in the model, "inside"
   and "which cell" coincide. A multi-cell comparison would need OpenMC's colour
   assignment decoded, which is a palette-matching problem rather than a
   geometry one.
3. **`ColourBy::Material` and `ColourBy::Universe` are not cross-checked**, only
   `Cell`.
4. **No voxel plots, no native rasteriser, no interactive viewer** — explicitly
   out of scope per the issue.
