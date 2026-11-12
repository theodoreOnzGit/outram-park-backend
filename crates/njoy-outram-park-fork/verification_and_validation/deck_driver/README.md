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

## 5. THERMR tape driver, free gas (2026-11-11, gh:#1369)

`src/deck/thermr.rs` translates `thermr.f90`'s main flow, `calcem`'s free-gas path
(`:1857-1916`, `sig` label 200) and `tpend` for `iinc = 1`, no elastic, `iform = 0`,
one temperature. **Predicted before running:** bit-identical tape (the kernel is a
closed form; the only libm exposure is one `exp` per `sig` call, PETIR `r_exp`
vs glibc, so a last-digit difference in a 7-figure rounded value was possible).
**Result:** `tests/deck_driver_tapes_vs_njoy2016.rs::test_01_first_thermr_free_gas_tape_is_identical_line_for_line`:
test 01 up to its first THERMR (carbon free gas, `mtref = 221`, `0 -22 -24`) then
MODER to text: **0 differing lines of 13 056** (9 642 in MF=6), MF=1 dictionary,
MF=3/MT=221 and MF=6/MT=221 included.
~~**Not ported (`NotPorted`):** `iinc = 2` from ENDF-4/5 tapes, `iform = 1`, several temperatures, `iinc = 2`/`icoh` from ENDF-6 tapes (`rdelas`, `iel`).~~
**CORRECTED 2026-11-12 (gh:#1369):** all of these are ported now (sections 6 and 7); what is still refused is listed in the module doc of `src/deck/thermr.rs`.

## 6. THERMR tape driver, ENDF-5 graphite (2026-11-11, gh:#1369)

Test 01's second THERMR (`26 -24 -23 / 1065 1306 8 1 2 1 0 1 229 2`, ENDF/B-V
`t322` as unit 26): `iinc = 2`, `icoh = 1` (graphite), MT 229 and 230.

**Translated** (`thermr.f90`, NJOY2016.80): `gateff`/`gatef2`
(`thermr/effective_temperature.rs`, tables read from the Fortran source by a
script), `sigcoh` for `lat = 1..3` with `form` (`Sigcoh::builtin` in
`thermr/mf3_grid.rs`), `coh` generalised to any `Sigcoh` and to return the
coherent column (`coh_with`), `tpend` MF=3 for `mtref+1`, MF=6 `mtref+1`
(`LIP = -nbragg`, `LAW = 0`, `ncdse = 3` in the dictionary). The MF=7 reader
needed no change: ENDF-4/5 uses the same LIST/TAB2/TAB1 layout as ENDF-6 and
only lacks the trailing `T_eff` TAB1, which the driver replaces by `gateff`'s
value (`thermr.f90:145-157`).

**Prediction (written before the run):** bit-identical text, because the
inelastic kernel (`compute_iform0`) is already gated word for word against
NJOY and the new code is arithmetic translated in Fortran operation order; the
only risk was PETIR libm vs glibc on `sin`/`cos`/`exp` in the lattice sums
(accepted as last-ulp by gh:#1455).

**Result:** unit 31 (MODER of the binary output): **0 differing lines of
32 962** (194 in MF=3/MT=229). Test
`deck_driver_tapes_vs_njoy2016::test_01_second_thermr_graphite_tape_is_identical_line_for_line`;
all 9 tests of that file pass.

## 7. THERMR tape driver: several temperatures, ENDF-6 elastic, `iform = 1` (2026-11-12, gh:#1369)

Decks 11, 25, 32, 49, 67, 68, 69, 70 and 74 were blocked in THERMR. What each
deck needs, from reading its cards and the MF=7/MT=2 `LTHR` of its tape:

| blocker | decks |
|---|---|
| several temperatures (`ntemp = 3`; one `tpend` per block of the multi-temperature PENDF) | 11 (free gas, ENDF-IV), 25 (H in H2O) |
| ENDF-6 `nendf`: `T_eff` TAB1 (read by the MF=7 parser, `ii.teff_request_k`) | 25, 32, 49, 67-70, 74 |
| `LTHR = 2`: `rdelas` + `iel` (incoherent elastic MF=3 and MF=6 `LIP = -2`) | 32, 49, 69 (ZrH), 74 (H in ZrH) |
| `LTHR = 1`: `rdelas` + `coh` lat = 10 | 70 (Al) |
| `LTHR = 3`: `coh` at `mtref+1` and `iel` at `mtref+2`, `ncdse` of the last call in both dictionary entries | 67 (D in 7LiD) |
| no MT=2 on the tape: the card `icoh = 1` stands, `coh` runs the built-in graphite lattice | 25 (a deck quirk upstream reproduces; so do we) |
| `iform = 1` (`LAW = 7` MF=6, `ncds`'s `j*(nep+1)` arithmetic) | 32 (second THERMR) |
| `icoh` of the card replaced by `10*LTHR` (deck 69 card says 1) | 69 |

**Translated:** `thermr` main flow's temperature loop (`:253-330`), `rdelas`'s
`LTHR` (`:337-348`; the `S(E)` temperature rule already existed as
`CoherentElastic::s_of_e_rdelas`), `iel`'s MF=6 (`:1334-1424`, `iel_mf6`),
`tpend`'s `ltt = 4/6` branches (`law7_mf6`), the dictionary's `ncdse`. Reused:
`compute_iform0/1`, `Sigcoh`, `coh_with`, `IncoherentElastic::iel_*`,
`thermr_terp`. Function-level tests (`src/deck/thermr.rs`): `plan_elastic`,
`law7_ncds_increment`, `iel_mf6`; the per-routine differential tests are the
per-section differences each deck test reports by `MAT MF MT`.

**Prediction (before the runs):** bit-identical text for every section, since
every kernel was already word-for-word against NJOY2016 and the new code is
bookkeeping; risk: the `ncds`/`ncdse` dictionary counts, which are not line counts.

**Result:** 0 differing lines. Decks 25, 67, 68, 70, 74: whole MODER'd output
tape (995 320, 463 392, 324 387, 213 800, 470 251 lines). Decks 11, 32 (both
THERMRs, iform 0 and 1), 49, 69: THERMR's own sections (MF=3/6 `mtref..` and
their dictionary rows). Gate: `tests/deck_driver_tapes_vs_njoy2016.rs`,
`test_{11,25,32,49,67,68,69,70,74}_thermr_*`.

**Findings outside THERMR:** ~~on the Zr-90 decks (32, 49, 69) the whole tape
differs in 4025's MF=3 MT=52/53/58 (one line each) and MF=10 MT=16/28/112 (29
lines): RECONR/BROADR output, same in all three, not investigated here.~~
**FIXED 2026-11-12 (gh:#1496)**: both came from RECONR, and both were already on
RECONR's own tape. (1) `lunion`'s scratch `nscr1` is binary unless `npend > 0`
(`reconr.f90:155,193`), so with `reconr -21 -22` `emerge` reads a threshold
raised to `sigfig(thr,7,+1)` that still carries sigfig's 1e-13 bias. The port
always rounded it to 11 columns, which is right only for `npend > 0`
(`reconr::reconr_for_deck`). (2) The deck PENDF's MF=10 writer read the raw
subsections, not `lunion`'s threshold-raised copy (`:1913-1938`;
`deck::pendf::other_files`). The whole tapes are now identical (deck 32 units
34/35, deck 49 unit 91, deck 69 unit 60), gated by
`test_{32,49,69}_whole_pendf_tape_is_identical`. Deck 11's
tape lacks UNRESR's MF=2/MT=152 (UNRESR passes the tape through).

## 8. ACER `iopt = 2`, `iopt = 7` on thermal files, the thermal plot (2026-11-12, gh:#1369)

Decks 25, 49, 67, 68, 69, 70 and 74 stopped at ACER `iopt = 2`. Now ACER's cards 8, 8a
and 9 (already read by `read_acer`) drive `src/deck/acer_thermal.rs`.

**No second builder.** ACER's `acesix` reads THERMR's PENDF back. The deck driver records
each THERMR run on the PENDF it writes (`PendfMeta::thermr`: the cards and the thermal ENDF
tape). `run_acer_thermal` finds the run for `mti`/`matd`, reads the incident MT=2 and `aw0`
from the PENDF block within 1 K of `tempd`, parses MF=7 at the block's temperature, and calls
`thermr_tables` + `AceTable::thermal_from_thermr_elastic`, the builder gated word for word by
`tests/acer_thermal_word_parity.rs`. Card 4's `(iz, aw)` pairs reach the header (`izn/awn`;
`AceTable::aw` is new); card 8a then overwrites `izn(1..nza)`.

**Prediction (before the runs):** identical ACE and directory units on every deck, because
the builder was already NJOY's. **What the first runs showed** (each cause found by reading
upstream):

| finding | decks | cause |
|---|---|---|
| ACE differs in the 12th figure of ITIX/ITXE; ITIX 0 at the first energy | 49 (binary `-25`) | every earlier gate had positive (text) units. With negative units THERMR's tape and `calcem`'s scratch tape are binary: no `a11` rounding, so `pendf_text` is the identity (`acesix::with_binary_units`, per thread, set by `npend < 0`). `calcem` rounds each incident energy `sigfig(.,8,0)` (`thermr.f90:1979`), so the ITIX query energy does too; and `gety1` shades a non-zero first abscissa to `0.999999*x0` (`endf.f90:1967`), which `itix_at` lacked |
| `NXS(5) = 3, NXS(6) = 19` against NJOY's `4, -1` | 49 | card 9 `mte = 228, ielas = 0` reads `iel`'s MF=3 as **Bragg data** (`aceth.f90:134-189`): `ElasticRead::BraggFromIncoherentMf3`. `mte = 0` is `ElasticRead::NoElastic` |
| ITCX half NJOY's | 67 (`natom = 2`, mixed elastic) | the builder divided ITCX by `natom`; `thrlod` divides by `nmix` (`aceth.f90:735`). Every earlier case had `natom = 1`. **Defect in the port, fixed** |
| 22 of 12 356 ACE lines differ, in ITIX only | 25 (`icoh = 1` on a tape with no MF=7/MT=2) | THERMR runs `sigcoh`'s built-in graphite lattice and `coh` merges its Bragg-edge points into the inelastic MF=3 grid even though ACER (`mte = 0`) reads no elastic; ITIX interpolates on that grid. `thermr_tables_lattice` does the same merge |

**`iopt = 7` on a thermal file** is `acer::thrfix` (`aceth.f90:550-637`: re-print through the
thermal class's own integer/real walk, the comment, and `throut`'s directory line,
`:2420-2429`). **The plot file** is `acer::tplots` (`aceth.f90:1073-2106`: log-log cross
sections, mubar, ebar, and for `IFENG = 2` the four 3-D pages and the angle-energy pages).
`aceth` has its own `ascll` without the "make sure amin and amax differ" step, so `tplots`
has its own copy. Not ported (`NotPorted`): a ZAID suffix and `nxtra != 0` on a thermal
file, `nmix > 1`, an ENDF-4/5 thermal tape, mcnpx formats, Type 2.

**Result** (`tests/deck_driver_tapes_vs_njoy2016.rs`,
`test_{25,49,67,68,69,70,74}_thermal_ace_and_directory`): 0 differing lines on every unit
compared. Deck 25: ACE 70-72 and directory 75. Deck 49: ACE 71 and directory 61. Decks 67-70
and 74: ACE 70, directory 75, and for the second ACER the check file 71, its directory 76 and
the plot file 55 (`viewr` input text). Deck 74 covers the continuous-form 3-D and
angle-energy pages, deck 70 the coherent pages, deck 67 the mixed-elastic pages. `viewr`
itself (unit 56) is not ported. Survey rows: `survey.md`.

## 9. VIEWR (2026-11-12, gh:#1369)

`src/viewr/` translates `viewr.f90` and `graph.f90`; the deck driver reads VIEWR's card
(`infile nps`) and runs it on the plot file the previous module left (`aplots`, `tplots`),
writing the PostScript to `nps`. **Prediction (before the runs):** identical PostScript wherever
the plot file is identical. **Result:** unit 54/56 identical, line by line, on decks 72, 36, 38-42,
67-70 and 74 whole-deck (1 230 516 lines), and on NJOY's own plot file for decks 13, 14, 35, 37
and 63 (679 533 lines). Deck 35 whole-deck is identical too although its plot file differs in 5
PURR-word lines; ~~decks 37 and 63 differ upstream of VIEWR (ACE words); decks 13 and 14 stop at an
ACER `iopt = 7` variant.~~ **CORRECTED 2026-11-12:** decks 37 and 63 are identical whole-deck since
gh:#1498, and decks 13 and 14 since section 10. Function level (against unmodified `graph.f90`), hand-written plot files
(legend, tags, error bars, hatching, alternate axis, portrait, 3-d) and the upstream quirks found:
`../viewr/README.md`. **The survey rows were not regenerated** (see `survey.md`).

## 10. Decks 13, 14, 24 and 28 (2026-11-12, gh:#1369)

Each cause was found by diffing the decks hex-exact against NJOY2016.80 and reading the routine
that writes the differing lines. A prediction was posted on gh:#1369 before each change.
**Result:** all four decks are `identical` whole-deck (`tests/deck_driver_tapes_vs_njoy2016.rs`,
`test_{13,14,24,28}_whole_deck`). Deck 13: 7 units, deck 14: 6, deck 24: 9, deck 28: 18.
0 differing lines on every unit. Every prediction held on the first run.

| deck | was | cause (upstream) | fix |
|---|---|---|---|
| 13, 14 | `NotPorted`: ACER `iopt = 7` with `itype != 1` | card 2 reads `7 1 2`, but `acer.f90:486` sets `itype=iopt-6`, so `iopt = 7` reads and writes Type 1. The card's `itype` is only echoed (`:313`) | `run_acer_fix` tests `iopt - 6` and refuses only `iopt = 8` and mcnpx |
| 14 | tape31 5 of 17 855 lines | integer words printed as reals: `aceout` prints XSS through `change` (`acefc.f90:13024`), and the charged-particle branch of the deck driver used `to_type1_text` | that branch writes through `acefix_change::type1_text`, as the neutron branch has done since gh:#1498 |
| 24 | `viewr: unit 26 holds no plot-command file` | HEATR's `nplot` unit (`heatr.f90:157`, plot block from `:6035`); `heatr::heatr` made the file and `run_heatr` dropped it | `run_heatr` writes it on `nplot` |
| 28 | `NotPorted`: ACER `iopp`/`ismooth` other than the defaults | card 6 `0 0 0/`. `iopp = 0`: `first` counts no MF=12/13/15 (`acefc.f90:390-402`), so no `gamsum` (`:209`), and no GPD or photon blocks in `acelod` (`:6256`, `:6304`). `convr` still runs, so the grid is unchanged. `ismooth = 0`: no MT=18 tail (`:6816`), no delayed `sqrt(E)` bin (`:6181`), no LANG=2 extension (`:7555`, `:7596`) | `AceDeck::with_iopp`, `with_ismooth`, `energy::build_emissions_smooth`; the defaults are unchanged |

NJOY's listing for deck 28 shows that `ismooth = 0` is exercised. The first two ACERs print
"extending lowest delayed bin using sqrt(E)" and the third does not. The table now matches with
and without it.

The same changes closed decks 26 and 72. Their HEATR plot units (50 and 26) had not been
written, and both are now `identical`. Deck 82's first ACER table (unit 26) is identical; the
deck stops at `izaopt = 1` (not ported). Its tape21/22 rows compare the last material NJOY wrote
against the first one this run wrote.

