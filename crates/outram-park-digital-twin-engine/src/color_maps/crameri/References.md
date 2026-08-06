# References — Scientific colour maps

Provenance record for the colour tables vendored in `tables.rs`, per the
workspace `RESEARCH_INTEGRITY_AND_PROVENANCE.md`.

## Source

| | |
|---|---|
| Project | Scientific colour maps |
| Author / copyright | Fabio Crameri |
| Version | 8.0.1 |
| Published | 2023-10-05 |
| DOI | [10.5281/zenodo.1243862](https://doi.org/10.5281/zenodo.1243862) |
| Record | <https://zenodo.org/records/8409685> |
| Home | <https://www.fabiocrameri.ch/colourmaps/> |
| Licence | **MIT**, Copyright (c) 2023, Fabio Crameri |
| Retrieved | 2026-08-06 |
| Archive | `ScientificColourMaps8.zip` (64 MB), official Zenodo release |

## Citation

> Crameri, F. (2018). *Scientific colour maps.* Zenodo.
> <https://doi.org/10.5281/zenodo.1243862>

Supporting publication: Crameri, F. (2018), *Geodynamic diagnostics, scientific
visualisation and StagLab 3.0*, Geoscientific Model Development.

## What was taken, and how

Five of the suite's maps, transcribed from the ASCII `.txt` tables in the
official archive:

| Map | Source file | Kind |
|---|---|---|
| `vik` | `vik/vik.txt` | diverging |
| `roma` | `roma/roma.txt` | diverging |
| `batlow` | `batlow/batlow.txt` | sequential |
| `lajolla` | `lajolla/lajolla.txt` | sequential |
| `romaO` | `romaO/romaO.txt` | cyclic |

Each source file holds 256 lines of space-separated floating-point R G B in
`[0, 1]`. Processing was a single step: multiply each channel by 255 and round
to the nearest integer. No resampling, reordering, smoothing or gamma
adjustment was applied — the values in `tables.rs` are Crameri's own, at 8-bit
display precision.

The 8-bit conversion is not a loss for this use: `egui::Color32` is 8 bits per
channel, so the same quantisation would happen at the display boundary
regardless.

Only these five maps are vendored, not the whole suite, so that what ships is
limited to what is actually used.

## Licence compliance

MIT requires the copyright notice and permission notice to accompany copies and
substantial portions. The full licence ships as `LICENSE.crameri.pdf` at the
crate root (the licence document from the official archive), and the notice is
repeated in the module and table doc comments.

MIT is compatible with this crate's `GPL-3.0-only`.

## Sources considered and rejected

Two convenient wrappers were checked and **not** used:

- **NASA GISS Panoply colorbars** (<https://www.giss.nasa.gov/tools/panoply/colorbars/>)
  — carries no licence, terms of use or redistribution statement, and
  aggregates third-party tables (ColorBrewer, NCL/UCAR, GMT/SOEST, NASA Earth
  Observatory, NOAA, UK Met Office and others) without marking which is which.
  Taking the ColorBrewer-derived tables through it would additionally have
  stripped the attribution ColorBrewer's own licence requires.

- **`github.com/chadagreene/crameri`** — a MATLAB wrapper for these same maps.
  Checked 2026-08-06: no `LICENSE`/`LICENCE`/`COPYING` file, and the GitHub API
  reports no declared licence. Absent a licence grant the code is all rights
  reserved, so translating it would produce a derivative of work we have no
  permission to use — irrespective of the underlying colour data being MIT.

Going to the original release avoids both problems and is the only version whose
licence is unambiguous.

## Not adopted: ColorBrewer

ColorBrewer (Apache-2.0, © 2002 Cynthia Brewer, Mark Harrower, The Pennsylvania
State University) was evaluated as an alternative and not adopted. Its schemes
are designed to distinguish **discrete choropleth classes** and are not
perceptually uniform; stretched across a continuous field they introduce
apparent banding that is not in the data. It remains a reasonable future choice
for *categorical* palettes, where that objection does not apply. Note its
licence requires the acknowledgement *"This product includes color
specifications and designs developed by Cynthia Brewer
(http://colorbrewer.org/)"* in end-user documentation.
