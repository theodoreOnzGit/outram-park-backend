# Hand derivations

Derivations written by people, kept with the crate whose physics they
describe.

## `ray/` — fission-product diffusion in TRISO fuel particles

**Author: Ray W.** ([@weewzr](https://github.com/weewzr)), submitted
2026-09-29 (`notes.tex`, and `notes.md`, the same derivation in markdown).
Moved here from the repository root's `derivations/ray/` on 2026-11-22 at
the maintainer's direction ("Ray's triso derivation belongs in boon lay
docs"); the files are unchanged.

What it covers, in the author's words: the physical system, the governing
diffusion equation and its assumptions, the homogeneous single-sphere
analytical solution through the raw Sturm-Liouville eigencondition with a
Robin surface, and its FTCS discretisation. It is the hand-derived
counterpart of boon-lay's TRISO diffusion models (the single-sphere release
fraction, and the multilayer finite-volume diffusion of gh:#1209). It is
not yet cross-checked against that code.
