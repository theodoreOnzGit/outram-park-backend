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
NJOY2016.80 binary, pair for pair and TAB1 heads (QM, QI, LR) included. ~~Not
compared: MF=1/2 text and directory, MF=12-14 (the deck's PENDF is the
pointwise content, not a byte copy).~~ **CORRECTED 2026-11-11 (part 2, below):** the
PENDF is now NJOY's tape line for line and is compared that way. Regressions re-run after the fixes: lib
676 passed; `pendf_stages_vs_njoy2016` 9, `heatr_driver_vs_njoy2016` 8,
`ace_five_route_parity` 13, `moder_vs_njoy2016` 1, reconr golden tests all pass.

## What stops the full deck

THERMR and GROUPR have no tape-level driver (their physics is partly ported:
`thermr::on_pendf` yields ACER-facing tables, not a PENDF tape with MF=6/7
sections; `groupr` has no end-to-end GENDF path). The deck driver reads their
cards and reports `DeckRun::blocked`.

## Part 2 (2026-11-11, gh:#1369): the PENDF is NJOY's tape, and ACER/UNRESR/PURR run

`deck::pendf` (translation of `anlyzd`, `emerge`, `recout`; `broadr`'s directory
and MT=152 temperature update) writes MF=1/MT=451 with description and
dictionary, the dummy MF=2/MT=151, MF=2/MT=152, MF=3 heads (`L1/L2`, 0/99 for
redundant sums) and MF=10/12/13/23 on the union grid. `run_deck` now runs
MODER, RECONR, BROADR (several temperatures, `istrap` bootstrap), HEATR, UNRESR
(cards, tape passed through), PURR (cards, tape passed through) and ACER
`iopt = 1` (`newfor = 1`). `njoy-deck <deck> [workdir]` is the CLI;
`examples/ace_files_compare.rs` compares two ACE files word for word.
Harness: `tests/deck_driver_tapes_vs_njoy2016.rs` (upstream's decks run by the
NJOY2016.80 binary and by this crate, every output line compared).

| case | what is compared | result |
|---|---|---|
| 01 C-nat ENDF/B-V (`moder reconr broadr heatr`) | units 28, 29, 30, every line | **0 differing lines** of 2 848 / 2 734 / 3 358 |
| 07 U-235 ENDF/B-V | unit 28 after HEATR | 56 of 17 897 lines differ, all MF=3 MT=18/19/102/301 and MF=2/MT=152 in the unresolved range (gh:#1448, `csunr1` not ported) |
| 10 Pu-238 ENDF/B-IV, 3 temperatures, bootstrap | unit 29 after BROADR | 6 lines, all MF=2/MT=152 (gh:#1448) |
| 10, whole deck through UNRESR, PURR, ACER | ACE unit 26 | **identical in every word** (NXS, JXS, 38 713 XSS words, PURR bands included) |
| 19 Pu-241 ENDF/B-VI.3 | unit 29 after BROADR | 6 lines, all MF=2/MT=152 |
| 19, whole deck | ACE unit 27 | every block identical **except LUNR: 760 of 4 604 words** (gh:#1450) |

**Predictions (before the runs):** the PENDF identical except where RECONR's
unresolved evaluation enters; the ACE identical where ACER's inputs are. **Not
fully borne out:** the first runs found the port defects below, each fixed by
translating upstream (gh:#1447):
1. `csslbw` rounds the running elastic sum to 8 figures after each l-state
   (`reconr.f90:2849-2851`); the port did not: 396 differing lines on test 07.
2. `emerge`'s `sn=small` floor on elastic (1e-8): Pu-238 wrote 0.0 (99 lines).
3. `lrp = 3` with an unresolved range; BROADR rewrites MT=152's temperature.
4. A redundant sum starts at the final-grid point before its first non-zero
   one (`mtrt`, `emerge` :4888-4891): MT=4 of Pu-241 lacked its leading point.
5. BROADR `istrap`: the next temperature starts from the **unrounded** scratch
   rows, not the `sigfig`'d tape values (400 MT=18 lines per temperature).
6. ACER: ENDF-4/5 MF=4 `LVT /= 0` carries a LIST where the CONT is
   (`acefc.f90:5811-5819`); and an MF=4 on MT=18 gives a fission AND segment
   (Pu-238: AND 12 889 words against 12 910).

**Not done in part 2 (and why):** the THERMR and GROUPR tape drivers (see the
issue comment: they need MF=6/MF=7 PENDF writing and a thermal-aware GROUPR
reading it, neither of which exists); UNRESR's MF=2/MT=152 and PURR's
MF=2/MT=153 are **not written** (`DeckRun::notes`), so the PENDF after those
modules is not compared; ACER `newfor = 0` (tests 07, 08; gh:#1449); test 21
needs RECONR `errmax`/`errint`/`enode` and BROADR `thnmx`/`errmax` (refused with
`NotPorted`); test 13 needs GASPR's tape driver and ACER `iopt = 7`; the ACER
directory unit is not written.

## 4. Part 3 (2026-11-11): `csunr`, PURR heating, ACER `newfor = 0` (gh:#1448, #1450, #1449)

Gate: `tests/deck_driver_tapes_vs_njoy2016.rs` (6 tests, ~21 s); function level for
`csunr1`/`csunr2`/`unfac`/`gnrl`: `upstream_units/reconr_csunr/README.md`.

| deck | what is compared | before | after |
|---|---|---|---|
| 07 (U-235 ENDF/B-V) | PENDF after HEATR, 17 897 lines | 56 lines differ | **0** |
| 10 (Pu-238 ENDF/B-IV) | PENDF after BROADR (3 T), 3 lines per T | 6 lines differ (MT=152) | **0**; ACE unchanged (identical) |
| 19 (Pu-241 ENDF/B-VI.3) | PENDF after BROADR, 3 T | 6 lines (MT=152, MT=1 at 2.5e4 eV) | **0** |
| 19 | ACE, 67 740 words | LUNR 760 words differ | **identical in every word** |
| 07 | ACE, `newfor = 0` | refused | all blocks identical except GPD (GROUPR `gamout`, not available) and 1 ESZ word |
| 08 | ACE, `newfor = 0` | refused | AND identical (2 943 words); ESZ/SIG/MTR... differ (see below) |

Causes found, each by reading upstream:

- **`csunr1`/`csunr2` not ported (#1448).** `reconr::csunr` is the literal
  translation, panel state (`ilast`, `enxt`, `elst`, `slst`, `snxt`) kept, with the
  `unfac` `l >= 3` fall-through and `csunr1`'s dead `ie.eq.ne` test (`ne` is 0 at that
  point). Fe-58's MT=152 now reproduces NJOY's physically absurd values on purpose
  (decision recorded in `reconr/urr.rs`).
- **MT=1 at 2.5e4 eV (test 19).** `rebuild_total_as_sum_of_parts` summed Pu-241's
  MT=51 below its threshold: `eval_lin_lin` returns the section's first value (2.08e-7 b)
  for a lower energy, where `emerge` skips a non-resonance section below its
  threshold (`thresh-eg.gt.test*thresh`, `test = 1e-10`).
- **PURR heating (#1450), not the probability bands.** Test 19's HEATR card asks for
  `302 318 402`, so PURR's `rdheat` finds partial kermas (`ihave = 2`) and
  self-shields the heating per band (`purr.f90:533-558`); the port divided MT=301 by
  the total for every band (96.03388 against NJOY's 38.636...). With the cards' units
  blocked binary, MT=301 and MT=153 are read as doubles, not through `a11` text.
  `AceDeck::with_heatr_partials`, `with_purr_binary`, `with_heatr_binary`;
  `PurrHeating`; `heatr_kerma_with_partials`. ESZ keeps reading MT=301 as printed
  (that is what test 07 needs).
- **ACER `newfor = 0` (#1449).** `topfil` turns every MF=4 incident-energy
  distribution into 32 equally probable bins (`ptleg` for Legendre, `pttab` for
  tabulated), `acensd` stores the 33 edges `sigfig(.,6)` at a positive locator and
  locator 0 when the law is isotropic (`n <= 2`). Both converters already existed for
  photons (`acer::photon_angular`); `acer::acensd::parse_mf4_for_acer_newfor` and
  `EnergyAngular::is_equiprobable` carry it to the AND writer.

Open after this part: test 07's GPD (needs GROUPR's photon-production GENDF, `gamout`)
and one ESZ heating word at 81.99999 eV (75.45917 against 75.45914; HEATR's own PENDF
is identical, cause not found); test 08 needs ACER to read the deck's HEATR-extended
PENDF (MT=443/444 in the reaction list and union grid) and the old-format MF=6 laws
(`acelf6` `newfor = 0`, `topfil` MF=6 conversion); tests 21 and 13 not started.
