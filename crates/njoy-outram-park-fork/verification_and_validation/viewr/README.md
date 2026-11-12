# VIEWR / graph vs NJOY2016.80 (gh:#1369, differential #1366)

`src/viewr/` is a routine-by-routine translation of NJOY2016.80's `viewr.f90` (command reader,
`set2d`, `set3d`, `erbar`, `legndb`, `tagit`, `ascalv`) and `graph.f90` (page, window,
perspective, axes, text, symbols and the PostScript writer). The PostScript it writes is NJOY's
**line for line** on every plot file tried. Upstream: NJOY2016 `ab27c6414a0f` (2016.80);
`graph.f90` SHA-256 `70bd1cd22273aa2ec245f95975115f0ed0d05c1ca8e5065e349ce35085f59bd0`,
`viewr.f90` SHA-256 `89971eac0d0fb03d5f0ac2dd13b14e7ef5678d17b9e80bcc40e6915635f1fb4b`.

Not reproduced: the card echo VIEWR prints on its output listing (`nsyso`); only the PostScript
file (unit `nps`) is. The translation uses PETIR's `log10`/`pow` (gh:#1455); no last-ulp
difference showed up anywhere below.

## 1. Function level (`tests/viewr_code_to_code.rs`)

`extract.sh` compiles `graph.f90` **unmodified** (with `locale`, `phys`, `mainio`, `util`) and
copies `rget`, `iget`, `stripv` (private in `graph.f90`) and `ascalv` (private in `viewr.f90`) out
by line range after checking both files' SHA-256. `viewr_driver.f90` calls them and the public
`graph` routines on fixed inputs and prints every number as hex bits; the PostScript of six
scenes is `tape61..66`. `reference.txt` is that output (gfortran 13.3, `-O3` as NJOY's CMake
release build). `gen_tables.sh` writes `src/viewr/tables.rs` (12 x 32 font widths, 57 colours)
from `graph.f90`, so nothing in it was typed.

**Prediction (before the run):** bit-identical numbers and text; the only libm exposure is
`log10` and the real power in the axis routines.

| family | inputs | result |
|---|---|---|
| `iget`, `rget`, `stripv` | 14 + 14 + 5 | **33 of 33 identical** (`rget` as hex bits) |
| `ascalv` | 28 cases: bad input, zero ends, huge/tiny ratio, close values, `p < 1`, `p >= 1e4`, each 1/2/5 choice, `m = 20` | **28 of 28**, 112 values (z1, z2 bits; major, minor) |
| `txtlen` | 14 strings x 2 fonts: shifts, doubled shifts, `#E #L #H #O #F #M`, defaults, a trailing shift, consecutive instructions | **28 of 28 bit-identical** |
| `trans3`, `xscale`, `yscale`, `xinvrs`, `yinvrs` | 12 points under two perspectives; 4 scale calls | **28 of 28 bit-identical** |
| PostScript, scene 1 | 11 linear axes (labels above/below, ticks both ways, `nonum` 0-3, `numr`, fractions, `*10^n` of both signs, three directions) | **0 of 889 lines differ** |
| scene 2 | 9 log axes (decades, label skipping, `<= -10`, `10^20`, non-decade ends, rotated) | **0 of 1 603** |
| scene 3 | all 25 symbols and the default | **0 of 419** |
| scene 4 | `text3` on the 14 strings, rotated, Symbol font | **0 of 546** |
| scene 5 | 3-d: `init3` x 2, log axis, `grid3` x 3, `curv3`, arrow `vect3` | **0 of 711** |
| scene 6 | log frame, grid, dashes 0-4, symbols, widths, gray shades, hatching, colour shades | **0 of 2 044** |

Both codes run the six scenes in one process: `graph`'s module variables, and `drawh`'s saved
line width and dash, outlive one VIEWR call, so scene 2's first stroke depends on scene 1's last.
`viewr::ViewrState` keeps them for a deck run for the same reason.

## 2. Integration: the upstream decks (`tests/deck_driver_tapes_vs_njoy2016.rs`)

The deck driver reads VIEWR's card (`infile nps`) and runs it on the plot file the previous
module left (`aplots` for decks 13-72, `tplots` for 67-70 and 74).

**Prediction (before the runs):** every PostScript unit identical where the plot file is
identical; and, where ACER's table differs upstream of the plot file (decks 35, 37, 63), NJOY's own plot
file gives identical PostScript.

**Whole deck, both codes** (`viewr_deck`, `test_*_viewr_postscript`): PostScript unit and the plot unit VIEWR read:

| deck | PostScript unit | lines | differing |
|---|---|---|---|
| 72 (Be-9) | 54 | 34 325 | **0** |
| 36 (Sn-119) | 54 | 119 126 | **0** |
| 38 (Kr-83) | 54 | 67 929 | **0** |
| 39 (Ni-63) | 54 | 157 626 | **0** |
| 40 (Mn-55) | 54 | 356 031 | **0** |
| 41 (Ge-71) | 54 | 232 343 | **0** |
| 42 (Zn-67) | 54 | 130 615 | **0** |
| 67 (D in 7LiD, `tplots`) | 56 | 5 483 | **0** |
| 68 (H in H2O) | 56 | 4 086 | **0** |
| 69 (Zr in ZrH) | 56 | 4 056 | **0** |
| 70 (Al-27) | 56 | 4 549 | **0** |
| 74 (H in ZrH, 3-d pages) | 56 | 114 347 | **0** |

(1 230 516 PostScript lines in all.) **On NJOY's own plot file** (`viewr_of_njoys_own_plot`:
NJOY runs the deck, our VIEWR runs on NJOY's plot unit): decks 35 (143 580), 37 (240 201), 63
(143 418), 13 (111 182) and 14 (41 152) -- **0 differing lines of 679 533**. Deck 35 whole-deck
also gives an identical PostScript (143 580 lines) although its plot file differs in 5 lines
(PURR words in the ACE, README section 8); decks 37 and 63 differ upstream of VIEWR (ACE words,
`acer_iopt7_2026_11_12.md`), and decks 13 and 14 stop earlier at an ACER `iopt = 7` variant
(`itype != 1`/mcnpx), so those five are gated on NJOY's own plot file.

## 3. Features no upstream deck reaches (`tests/viewr_vs_njoy2016.rs`)

The decks' plot files use a narrow part of VIEWR's input. Three hand-written plot files, run
by the NJOY binary as `viewr / 20 21 / stop` and by `viewr_postscript`, cover the rest:
legend blocks (`ileg = 1`), curve tags with arrows (`ileg = 2`), y and x error bars, hatching
(11, 21, 31) and shading (3, 45), positive and negative symbol counts, all dash types, windows
at a position and rotation (`xll yll ww wh wr`), titles only (`itype = 0`), portrait pages,
`size < 0`, user scales, the alternate right-hand axis (`iplot = -2`), `itype` 1-4, and 3-d
families (`nform = 1`) with `jtype` 1 and 2 and `x3 < 0`. **0 differing lines of 2 564 / 1 919
/ 805.**

## 4. Things found by reading upstream (kept, not fixed)

- **`initp`'s `wbord=.005e0` has no `_kr`: it is single precision** (0.004999999888...), so the
  page border and `gset`'s clip box (`.005e0_kr`) are different widths to `drawh`, which
  emits `0.360 setlinewidth` a second time with the same printed value. The first port
  omitted it; line 23 of deck 72's file differed in 14 places until the literal was
  reproduced. A scan of both files found no other kind-less literal.
- **`ifg = 0` indexes `ifrgb(1,0)`** (out of bounds) after `curv2` fills with shades 1-10 or
  41-80 until the next `frame2`; no deck reaches it and the tests avoid it. The port reads 0 there.
- **`txtlen` passes an undefined `l` to `ssum`** for a string whose first character is `#`.
  An `-O3` build crashed intermittently on it. No generated string starts that way
  (log-axis labels start `10#`/`*10#`); the port starts `l` at 0.
- **`#F` and the other instructions end only at a shift character** (`< > [ ]`), and a shift
  character resets the font, so `#Fi` has no lasting effect; text after an unterminated
  instruction is skipped. Reproduced as is.
- **Labels longer than their string** (`axis3` with `nlabel` above the length of a literal)
  read past it; the port reads blanks. The decks pass 80-column variables.
- **`txtlen` treats a final shift character as an ordinary character** (`i < ntxt` guard);
  `text3` does not. Reproduced.
- **Curves of more than 20 000 points** overrun upstream's arrays; the port refuses.

## 5. Reproduce

```
NJOY2016_ROOT=<NJOY2016.80> OUT=<scratch> verification_and_validation/viewr/extract.sh
NJOY2016_ROOT=<NJOY2016.80> cargo test --release -p njoy-outram-park-fork \
    --test viewr_code_to_code --test viewr_vs_njoy2016 --test deck_driver_tapes_vs_njoy2016 viewr
```

(2026-11-12, rustc 1.99.1, gfortran 13.3, NJOY2016.80.)
