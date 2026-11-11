# NJOY input-deck driver vs NJOY2016.80 (gh:#1369, differential #1366)

Port of `main.f90` (module loop), the list-directed card semantics every
module's input block relies on, and `openz`/`repoz`/`closz` (`util.f90`), as
`njoy_outram_park_fork::deck`. Upstream: NJOY2016 `ab27c6414a0f` (2016.80).

## 1. Function level: card reader vs gfortran

`listdirected.f90` makes 36 single `read(5,*)` statements over
`listdirected.in` and prints every item (reals as hex bits). `listdirected.ref`
is its output (gfortran 13.3, 2026-11-11; rebuild with
`gfortran listdirected.f90 -o ld && ./ld < listdirected.in`).
`tests/deck_driver_code_to_code.rs::card_reader_matches_gfortran_list_directed`
replays the sequence through `deck::cards::CardReader`.

Prediction (before the run): identical on all 36. **Result: 36 of 36 identical**
(every item, every hex bit pattern, both error and end-of-file cases). Covered:
plain, `/` terminators with defaults preserved, null between commas, leading
comma, `r*v`, `r*` nulls, spanning records, excess values discarded, blank
record skipped, reals (`1.e10`, `.005`, `5.`, `d`, `q`, letterless `1.5+3`),
quoted (single, double, doubled quote, empty, longer than the variable, slash
inside), unquoted string with the rest of the record dropped (the `-- comment`
module card), mixed string/int cards, tabs, repeat of a string, and `1.5` for an
integer, a letter for a number, `1.5e`, and EOF as errors. Not covered: a quoted
string spanning records.

## 2. Integration: upstream test 01 (C-nat)

Deck cards: all of `tests/01/input` is read to `stop` (MODER, RECONR, BROADR,
HEATR, THERMR x2, GROUPR, MODER). Execution: `moder; reconr; broadr; heatr`
with upstream's own (blocked-binary) units, `moder` conversions appended to
read the PENDF as text, against the NJOY2016.80 binary on the same deck;
every MF=3 section of the PENDF after BROADR and after HEATR is compared.

**Prediction (before running):** PENDF bit-identical (as `pendf_stages` shows on
ENDF/B-VIII tapes); HEATR tape MF=3 identical.

**Result (2026-11-11), first run: 15 of 29 sections differed after RECONR and
HEATR never returned.** Both were port defects on this ENDF/B-V tape, found by
the deck (gh:#1446, fixed, translating upstream):
1. `parse_endf_float` turned `1.00000- 5` (blank in the exponent) into 0.0; the
   MF=12/MT=102 yield became zeros and `gheat`'s first-energy scan looped on
   `xnext = 0` (upstream reads the blank as null, Fortran BN mode).
2. RECONR took AWI from `rows[2][0]` for every format version; upstream does so
   only for `iverf >= 6`. With TEMP=0 as AWI the `lunion` threshold raise was
   off (MT=51 at 4 812 000 eV against NJOY's 4 812 123; 4 points missing).
3. The `lunion` MF=3 skips applied to the grid only: MT=251-253 reached the PENDF.

**After the fixes:** MF=3 of MAT 1306 after RECONR **29/29**, after BROADR
**29/29**, after HEATR **31/31** (MT=301, 444 included) identical to the
NJOY2016.80 binary, pair for pair and TAB1 heads (QM, QI, LR) included. Not
compared: MF=1/2 text and directory, MF=12-14 (the deck's PENDF is the
pointwise content, not a byte copy). Regressions re-run after the fixes: lib
676 passed; `pendf_stages_vs_njoy2016` 9, `heatr_driver_vs_njoy2016` 8,
`ace_five_route_parity` 13, `moder_vs_njoy2016` 1, reconr golden tests all pass.

## What stops the full deck

THERMR and GROUPR have no tape-level driver (their physics is partly ported:
`thermr::on_pendf` yields ACER-facing tables, not a PENDF tape with MF=6/7
sections; `groupr` has no end-to-end GENDF path). The deck driver reads their
cards and reports `DeckRun::blocked`.
