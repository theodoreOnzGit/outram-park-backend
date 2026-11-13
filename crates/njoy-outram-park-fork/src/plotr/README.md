# PLOTR — plot-data generator for ENDF/PENDF/GENDF

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §PLOTR); upstream Fortran: `plotr.f90`.

## Theory

PLOTR is the data-selection half of NJOY's plotting pair (PLOTR → VIEWR). It reads
ENDF, PENDF, or GENDF files and produces **VIEWR input** describing the plots:

- conventional **2-D** plots (e.g. cross section *vs* energy) with lin/log axes,
  automatic or user ranges/labels, optional right-hand axis, and one/two titles;
- **3-D** plots (e.g. distributions vs energy and angle/secondary energy).

PLOTR itself does no rendering — it curates the curves and axes; VIEWR turns the
resulting commands into PostScript.

## How the port implements it

~~**Not yet ported.** In a Rust workspace this is the module most worth
*re-thinking* rather than porting verbatim: instead of emitting NJOY's bespoke
VIEWR command stream, a port can expose the selected `(x, y)` series as plain data
for any modern plotting stack (or the workspace's own tooling). Planned: a thin
extractor from the `crate::endf` `Tape` to labelled series; VIEWR-command emission
only if byte-compatibility with NJOY plots is required.~~
**CORRECTED 2026-11-13 (gh:#1369):** translated line for line from `plotr.f90`
(maintainer direction: "perform the actual translation"), and run from a deck
(`deck::plotr`). It writes NJOY's VIEWR command stream.

| file | upstream |
|---|---|
| `cards.rs` | the `read(nsysi,*)` statements of `plotr` (cards 0-13) |
| `stream.rs` | `endf.f90`'s `contio`, `listio`, `tab1io`, `tab2io`, `moreio`, `tpidio`, `tosend`, `tomend`, `findf` on a tape kept as its lines |
| `state.rs` | `plotr`'s declarations (l.362-413) as the fields of `Plotr`; the edit descriptors; the record and getter calls on a unit |
| `run.rs` | `plotr` l.415-1369, 1788-1797: curve loop, ENDF/PENDF 2-d walk with thinning |
| `plot2d.rs` | `plotr` l.1688-1786: input data, the 2-d writer |
| `mf7.rs` | `plotr` l.949-1096 (MF=7) |
| `gendf.rs` | `plotr` l.1464-1686 (GENDF), `gg3d` |
| `plot3d.rs` | `plotr` l.1371-1462, `ad3d`, `ed3d` |
| `getters.rs` | `ascale`, `getz`, `gety4`, `gety6`, `fixl7`, and `gety1` on a tape (through `endf::gety1::Gety1`) |
| `rname.rs` | `rname` |

Reused rather than rewritten: `endf::gety1::Gety1`, `endf::interp::terp1`,
`errorr::weight::terpa` (stateful, flat), `acer::fortran_fmt::fortran_e`,
`viewr::fmt::{f, i}`, `deck::cards::CardReader`, `deck::covr::tape_from_records`
(a blocked-binary GENDF at full precision).

Upstream's locals live for the whole run, so a value one curve sets is what the
next curve reads; they are fields of `Plotr`. A local upstream can read before
anything sets it is an `Option`, and that read is refused by name.

## Testing

`tests/deck_driver_plotr_vs_njoy2016.rs`, against NJOY2016.80 run on the same
deck (2026-11-13):

- upstream decks 06 and 12, whole: every unit identical line for line, and the
  PostScript equal to upstream's committed `referenceTape32` / `referenceTape24`;
- five decks of our own for the other branches: `gety4` and `getz` (ratio plot),
  the automatic reaction loop, MF=7, GENDF with `gg3d`: identical;
- MF=6: refused (below).

Unit tests in each file pin `findf`, paging, `ascale`, `rname`, `getz`, `gety4`,
`gety6`, `fixl7` and the card order.

## Caveats and divergences

- **Refused as undefined upstream:** an MF=6 curve (2-d or `ed3d`). `gety6`'s
  `ylast` is not saved (`plotr.f90:2803`) and its first interpolation reads it
  unset; NJOY2016.80 then plots a stale stack value. Also refused: `lf`, `lep`,
  `l2`, `iauto`, `nr`, card 7 or `sigig` read before any curve set them; `rname`
  outside its table; `fixl7` beyond `egrid(200)`; `gg3d` beyond `ex3(200)`;
  `terp1` of a log law on a non-positive value (a NaN upstream); `ad3d`'s
  `intmu` on an MF=6 tabulated law.
- **Kept as upstream:** `ed3d` writes its view card without `/`; `ad3d` and
  `gg3d` write `-x3`; `gg3d` writes the 3-d header and no matrix data;
  `gety4` returns the coefficient at `E(np-1)` past the last panel; `gety6` ends a
  one-page LIST two panels early; `getz` keeps one `lt` for every tape; MT=443/444
  names swap for `iverf < 6`; `nplt0` is read and not used.
- **Different but equivalent:** `gety1` and `getz` read a TAB1 whole at their
  `x = 0` call where upstream pages it in. The values are the same; the tape
  position differs only until the `tosend` that follows. (If a walk stopped
  before a long TAB1's last page, upstream would read the next record from mid
  TAB1; nothing in PLOTR does that.) The second `getz` tape's record is held
  apart from the first's, where upstream's first page could overlap it when
  `nr > 21`.
- **Not written:** the listing (`nsyso`) and the `mess` lines.
- Card 8a: upstream presets no `iverf2`; it defaults to 0 here.

## References

- NJOY2016 manual §PLOTR (LA-UR-17-20093)
- `plotr.f90` (NJOY2016.80, commit `ac5adf5`); see also `../viewr/README.md`
