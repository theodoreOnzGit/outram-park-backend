# VIEWR — PostScript plotting back-end

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §VIEWR); upstream Fortran: `viewr.f90` (+ shared `graph.f90`).

## Theory

VIEWR is the rendering half of NJOY's plotting pair. It reads user (or
PLOTR/COVR/DTFR/ACER-generated) commands defining 2-D and 3-D graphs and writes a
high-quality **PostScript** file. Capabilities include conventional 2-D plots
(lin/log axes, auto or user ranges/labels, optional right-hand axis, one/two
titles), curves with various line patterns labelled by tags/arrows or a legend
block, error bars, shading and hatching, and 3-D surface plots. The low-level
drawing primitives are shared with COVR/PLOTR via `graph.f90`.

## How the port implements it

~~**Not yet ported.** … Planned stance: port only if byte-compatible NJOY plots are
explicitly required; otherwise treat VIEWR/PLOTR as a *data* interface …~~
**CORRECTED 2026-11-12 (gh:#1369):** ported whole, routine by routine, because the
upstream regression decks compare VIEWR's PostScript unit (decks 13, 14, 35-42, 63,
67-70, 72, 74). `viewr_postscript(plot_text) -> String` and `ViewrState::viewr`
(state that persists across calls, as the Fortran process's does); the deck driver
runs it from VIEWR's card (`infile nps`).

| file | upstream |
|---|---|
| `driver.rs` | `viewr.f90` `viewr`: the list-directed command reader and page loop |
| `plot2d.rs` | `set2d`, `erbar`, `legndb`, `tagit`, `ascalv` |
| `plot3d.rs` | `set3d` |
| `graph.rs`, `axis.rs`, `curves.rs`, `text.rs`, `symbols.rs` | `graph.f90` |
| `ps.rs`, `fmt.rs` | the PostScript `write` statements and Fortran `Fw.d`/`Iw`/`O3.3` |
| `tables.rs` | font widths and colours, generated from `graph.f90` |

Not reproduced: the card echo VIEWR prints on its output listing.

## Testing

Function level against unmodified `graph.f90` and the extracted `rget`/`iget`/`stripv`/`ascalv`
(`tests/viewr_code_to_code.rs`): 33 + 112 + 28 + 28 values and 6 212 PostScript lines
identical. Integration: 17 upstream decks (1 910 049 PostScript lines) and three
hand-written plot files (feature coverage) identical to NJOY2016.80
(`tests/deck_driver_tapes_vs_njoy2016.rs`, `tests/viewr_vs_njoy2016.rs`). Methodology,
results and the upstream quirks found:
`verification_and_validation/viewr/README.md`.

## Caveats

- Pure visualisation; no nuclear-data content. Research, education and V&V only.
- Upstream quirks kept on purpose (a single-precision `.005e0`, an undefined `l` in
  `txtlen`, instructions that end only at a shift character): see the V&V README, section 4.

## References

- NJOY2016 manual §VIEWR (LA-UR-17-20093)
- `viewr.f90`, `graph.f90` (NJOY2016.80); see `../plotr/README.md`
