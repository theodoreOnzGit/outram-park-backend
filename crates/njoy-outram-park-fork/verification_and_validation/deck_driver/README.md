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
`../viewr/README.md`. ~~**The survey rows were not regenerated** (see `survey.md`).~~ **CORRECTED 2026-11-12:** they were, the same day (`survey.md`, "RE-RUN 2026-11-12"), and decks 14, 50-54, 62 and 71 again for gh:#1490 (section 11).

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
written, and both are now `identical`. Deck 82's first ACER table (unit 26) is identical; ~~the
deck stops at `izaopt = 1` (not ported). Its tape21/22 rows compare the last material NJOY wrote
against the first one this run wrote.~~ **CORRECTED 2026-11-13:** `izaopt` is ported and deck 82
is `identical` on all 18 units (section 12).

## 11. The incident-charged-particle decks re-run (2026-11-12, gh:#1490)

`SURVEY_CASES=14,50,51,52,53,54,62,71` (`tests/deck_driver_survey.rs`, 9 s after the build,
NJOY2016.80 `ac5adf5`, toolchain per `rust-toolchain.toml`): **all eight `identical`**, 6 tapes
each, every unit NJOY wrote as text compared line by line (ACE, check file, both directory
lines, plot file and VIEWR's PostScript); tape21/22 are NJOY's blocked binary. The rows equal
`survey.md`'s, so no row changed. The issue's last bullet (the second `acer` card, `iopt = 7`,
and `viewr` not ported) was stale: both landed under gh:#1492 and gh:#1369 (sections 9 and 10,
`acer_iopt7_2026_11_12.md`). The branches the issue lists are recorded in
`../acer_charged/README.md`, "gh:#1490".

## 12. `izaopt`, LEAPR, ACER `iopt = 4/5`, and four PENDF writer details (2026-11-13, gh:#1369)

Each cause was found by diffing against NJOY2016.80 and reading the upstream
routine that writes the line. A prediction was posted on gh:#1369 before each
change. Gates: `tests/deck_driver_tapes_vs_njoy2016.rs`.

**Search first.** `src/` already had the whole of LEAPR (`leapr::deck::LeaprDeck`,
`leapr::run::run_deck`, `LeaprRun::write_text`) and the photo-atomic and
photo-nuclear ACE builders. Only the deck wiring was missing, so no second
parser was written. ERRORR has `run_mf33` and `ErrorrResult::to_tape`, but
every errorr deck also needs an unported piece: a GROUPR input tape (`ngout`),
MF=31/34/35, or COVR's plot option. GAMINR's numeric engine is not ported.
PLOTR is not ported at all (`src/plotr/mod.rs`). All three are left as large
ports.

| deck(s) | was | cause (upstream) | fix | result |
|---|---|---|---|---|
| 82 | `NotPorted`: ACER `izaopt` | `acelod` (`acefc.f90:4951-4961`): metastable ZAID `za+300+100*is`, Am-242's swap | `run_acer` rewrites the ZAID field (`izaopt_zaid`) | 16 ACE/directory units identical |
| 82 | units 36, 38, 56, 58: 1 line each (not predicted) | `aceout` writes `NXS(9) = is`, LISO (`acefc.f90:13019`); the builder wrote 0 | the deck driver sets `NXS(9)` from the PENDF's LISO | identical |
| 82 | tape21: 1 line | `recout` copies every MF=3 section after the last redundant reaction with `tofend`, its head included (`reconr.f90:5355-5365`); Am-242m1's MT=102 head carries AWR `2.399800+2`, MF=1 `2.399801+2` | `deck::pendf::recout_heads` | identical |
| 82 | tape22: 1 063 lines (sequence numbers) | BROADR copies MF=1's closers, MF=2's sections and everything after MF=3 with `tosend`/`tofend`/`tomend` (`broadr.f90:681`, `:868-876`, `:999`), which carry the running count on a coded unit (`endf.f90:1150-1154`) | `deck::pendf::broadr_copied_closers` | identical |
| 09, 22, 23, 33 | `leapr: card reader not yet translated` | none; the cards are read by `LeaprDeck` | `deck::leapr` (`read_leapr`, `run_leapr`); NJOY2016.80's `bk` is CODATA 2018 (`phys.f90:22`) | identical (22: 4 636 lines, 23: 17 306, 33: 2 x 53 151, 09 whole deck) |
| 23 | 57 lines of MF=1/MT=451 | card-20 comments are unquoted, and `read(nsysi,*) text` is list-directed (`leapr.f90:3101`), so a card ends at the first blank (`* BEO ...` reads as `*`) | `LeaprDeck::read_comment` reads the card with `deck::cards::CardReader` | identical |
| 81, 88 | 1 line each | LRF=7: `rdsammy` sets `ascat` to the last elastic channel's `APE` (`samm.f90:1126`), and `recout` writes it in MF=2/MT=151 (`reconr.f90:5200`); the port wrote 0 | `deck::pendf::rml_ascat` | identical |
| 48, 59 | `NotPorted`: ACER `iopt = 4` | none | `deck::acer_photon` binds the units; the directory line follows `phoout` (`acepa.f90:1023`) | directory line identical; the ACE differs in 36 of 17 964 lines, all in the heating block (45 words, worst 8.7e-8). This is the residual already recorded in `../acer_photoatomic_vs_njoy2016.md`. |
| 56, 57, 58, 66, 78 | `NotPorted`: ACER `iopt = 5` | none | as above | the builder refuses by name: MF=4 angular data (56), MF=6 `LCT != 1` (57, 58, 78), MT=18 nubar (66) |
| 43 | `differs`: tape35 320 lines | at `tempef <= 0` BROADR only thins (`thinb`, `broadr.f90:1677-1760`), which is not ported (gh:#619); the driver passed RECONR's grid through | the driver now refuses it (`NotPorted`) | `NotPorted`, honestly |
| 80 | `leapr: card reader not yet translated` | not investigated | none | runs; 17 369 of 91 453 lines of MF=7/MT=4 differ in the last printed figure (`test_80_leapr_whole_deck`, ignored with that reason) |

## 13. GROUPR automatic lists, nuclide production, `getmf6` and `gam102` (2026-11-13, gh:#1369)

The target was the GROUPR blockers of decks 02, 08, 11, 17, 30, 46, 47, 73 and 76.
For each deck the survey row was re-run first, to find its real first blocker.
A prediction was posted on gh:#1369 before the changes.

**Search first.** The search covered `src/groupr/`, `src/deck/groupr.rs` and
`src/deck/modules.rs`. It found:
- `deck::groupr::auto_reactions`, which covered `nextr` only for `mfd <= 3` and
  in part for 16/17.
  - Its MF=18 check read a file 18, which ENDF-6 does not have. Upstream's `mf18`
    is the MF=6 photon list.
- `groupr::mf6_feed::Mf6Feed`, a full `getmf6` for LAW 1/7 that nothing called.

Nothing had ported the following:
- `conver`'s MF=4/MF=6/MF=8 lists;
- the production path (`mfd >= 10 000 000`);
- `getyld` for MF=6/9;
- `gam102`;
- `ngout1`;
- `genflx` with `nsigz > 1`;
- a temperature loop;
- ~~`mfd = 5`~~ (**CORRECTED 2026-11-13:** ported, section 14).

`MT = 257-259` and `452-456` were dropped silently with a note.

| deck(s) | was | cause (upstream) | fix | result |
|---|---|---|---|---|
| 30 | 21 249 of 21 249 GENDF lines differ | `3 259` was skipped. `getsig` (`groupr.f90:6684-6690`, `:6762-6776`) has no PENDF section for 257-259 (`first = 1e-5`, `enext = 1.01 e`, `lrflag = 0`). | `SigFeed::non_cross_section` | MF=3 part identical (2 505 lines) |
| 30, 08, 46, 47, 76 | `groupr auto mode for this mfd` | `conver` (`:10920-11827`) records MF=4 MTs range-compressed (MT=18 never merged; 19 replaces it), and MF=6 MTs with a neutron (`mf6`) or photon (`mf18`) subsection. `nextr` (`:1132-1317`) takes `mf4` while `\|mt\| <= 91`, then `mfd = 8` over `mf6`. `16/` takes `mf12`, `mf13`, then `mfd = 18` over `mf18`. | `deck::groupr_auto::{conver_lists, nextr_all}`. `mfd = 8/18` now use `Mf6Feed` (`PanelFeed::Mf6`). | see below |
| 30 | `getmf6` refuses `disc102` | H-1 MF=6/MT=102's photon is a LAW=2 line. `getmf6` hands it to `gam102` (`:7618-7623`, `:7977`, `:10866-10918`). `gam102` puts the whole photon in the group of `disc102 + E awr/(awr+1)` and leaves `idisc` unassigned. | `njoy_panel::Gam102`; `IDISC_UNSET` keeps `panel`'s `idiscf` | **GENDF identical, 21 249 lines** (`test_30_groupr_gendf_whole_tape`) |
| 46 | 157 of 8 281 lines, threshold groups of MF=6 MT=16/22/28/91; none with `ismooth = 0` | `getmf6`'s `ismooth` lin-lin extension (`:7765-7810`) shifts the record before it reads `tmp(ilo+6+2*ncyc)`, so a two-point record (the threshold table) is extended. `ismooth_linlin`, and `ismooth_histogram` with it, returned early unless three records were present. | guard `6 + 2 ncyc` | **GENDF identical, 8 281 lines** (`test_46_groupr_gendf_moder_copy`). ERRORR follows (not ported). |
| 73 | `auto mode for this mfd` (`10/`) | `conver` label 820 lists MF=8 `(LMF, MT, 10 ZAP, LFS, isomer count)`. `nextr` gives `mfd = 10^7 k + 10 ZAP`. `getsig` reads MF=3 (LMF 3/6/9), or the last PENDF MF=10 subsection with `IZAP, LFS` (LMF 10). `getff` label 100 multiplies by `getyld`'s MF=6 (`ZAP`, `LIP = isom`) or MF=9 yield (`terpa`). The head's C2 is `izam` (`:688-711`, `:843-855`). | `write_production`, `groupr_auto::product_yield`, `PanelFeed::Yield` | **whole deck identical, 1 009 lines** (`test_73_whole_deck`) |
| 08 | `auto mode` | Ni-61 MF=6 MT=51-58 are LAW=3. `getmf6` sends them to `getdis` with `mft = 8` (`:7936-7940`), which is not ported. | ~~none~~ **section 14:** LAW=3 through `getdis` | ~~`NotPorted` (`getmf6` LAW=2..5)~~ **CORRECTED 2026-11-13:** past `6/`; stops at `16/` (MF=12 LO=2, `conver`) |
| 47, 29 | `auto mode` (47); 29 `differs` | `5 18` / `5 455`: `mfd = 5` spectra (`getff` label 200 with `econst`, `getsed`, `dntc`). | ~~none; `mfd = 5` is refused by name instead of skipped~~ **section 14:** `mfd = 5` ported | ~~`NotPorted`~~ **CORRECTED 2026-11-13:** 29 whole deck identical; 47 stops at `6/` MT=18. Deck 29 was `differs` because its `5 455` section was skipped without a word. |
| 76 | `auto mode` | `3 452/455/456` (`getyld` on MF=1) | ~~none; refused by name instead of skipped~~ **section 14:** `getyld` on MF=1 | ~~`NotPorted`~~ **CORRECTED 2026-11-13:** GENDF identical without `6/`; stops at `6/` MT=18 |
| 02, 11 | `several temperatures or sigma-zeros` | `ntemp = 3`, `nsigz = 7`. Needs the temperature loop (`:586-630`, one card-9 list per temperature, which `read_groupr` does not yet read), `genflx` and `getunr`. | none | unchanged |
| 17 | `iwt = 6` | Before GROUPR, its JENDL-3.3 RECONR/BROADR tapes already differ (tape31 298 349 of 305 355 lines) | none | unchanged |

Each fix was found by reading the routine that owns the line, except the
`ismooth` guard. For that one, an ablation (`ismooth = 0` on both sides) showed
which branch carried the whole difference.

**Rows that moved earlier.** Decks 15, 18, 27, 29, 34, 47 and 76 ~~now stop at
GROUPR~~ (**CORRECTED 2026-11-13:** stopped at GROUPR until section 14; 29 is now
identical, 15/18/27/34 stop at ERRORR, 47/76 at `getff` label 200) with a named refusal (`mfd = 5`, or MT=452/455/456). Before, GROUPR skipped
those sections with a note and wrote a GENDF that was missing them, and the
deck carried on (18, 27 and 34 to ERRORR, 29 to a `differs`). The survey count is
**47 of 87 identical**, up from 45. Deck 73 is new. Deck 07's row was stale; it is
identical on this tree, and which change closed it was not examined.

**Tests (release, on develop a704d47c5 plus this work):**

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` | 108 passed, 2 ignored (43, 80), 1 395 s |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `acelf6_ismooth_vs_njoy2016_units`, `delayed_blocks_write_vs_njoy2016` | 3 of 3 and 1 of 1 passed |
| `--lib` | 729 of 729 passed |

## 14. GROUPR `mfd = 5` spectra, MT 452/455/456 and `getdis` with `mft = 8` (2026-11-13, gh:#1369)

The targets were the GROUPR refusals left by section 13: `mfd = 5` (decks 18, 27,
29, 34, 47), MT 452/455/456 (15, 76) and `getdis` with `mft = 8` (08). A
prediction was posted on gh:#1369 before each change.

**Search first.** `groupr::continuum::PhotonSed` already ported `getsed`'s
tabulated `LF = 1` branch, but only on MF=15, summed (`nk = 1`). `SedFeed` had
analytic `LF = 9`. `TwoBodyFeed` had `getdis` for MF=4. Nothing ported `getyld` on
MF=1, `getff` label 260 or `displa` label 400. All three were extended or reused,
not duplicated.

**Upstream, read before the change.**
- `mfd = 5` skips `getsig` and `getflx` (`groupr.f90:745-748`). `getff`'s init
  calls `getsed` on MF=5, whose `enext` is the lowest first `p(E)` energy. One
  more `getff` at that energy returns the spectrum (label 260: `iglo = 1`, `ighi`
  the last nonzero group). The record is written once, as group `ngn`.
- For MT=455 the families stay separate (`nk = ndelg`, `nl = ndelg`). Each
  family's first word is `dntc`, which **only `getyld` on MF=1/MT=455 sets**. It
  holds whatever an earlier `3 455` left, or zero.
- The head's `lrflag` is the previous `getsig`'s, a module variable.
- MT 452/455/456 are `getff` label 100 with `ng = 2`, `ff = (yld, 1)`, and
  `getsig` on MT=18: the ratio form of 251-253.

| deck(s) | was | cause (upstream) | fix | result |
|---|---|---|---|---|
| 18, 27, 34 | `mfd = 5` refused | `5 18`, one `LF = 1` subsection | `PhotonSed::from_tape_mf` on MF=5, `groupr::spectrum::mf5_spectrum` | GENDF identical: 968 (18, unit 91), 1 282 (27) and 1 312 (34, MODER copies) lines |
| 18 | after the above: 948 of 968 lines differ, each MF=3 section holding only its top group | **a defect in the driver:** `emaxx` was the PENDF's second MF=1 record's C2 (`STA`, 1 on this Cf-252 decay tape). `:413-431` reads the third record of `nendf` (`EMAX`). | `emaxx` from `nendf`'s third record | identical (row above). Every earlier GROUPR gate still passes. |
| 29 | `mfd = 5` refused | `5 455` on Pu-241: six **`LF = 5`** subsections (the prediction said `LF = 1`; the first run was refused there). `anased` law 5 (`:12355-12363`) is `intega` of `g(x)` times `theta`. `getsed` extends the lowest delayed bin when `ismooth` is on (`:11942-11965`). | `Lf5` in `PhotonSed`; `extend_lowest_delayed_bin` with `fx = .8409` as a single-precision literal | **whole deck identical**: units 35, 45 and 55, 635 lines each. Ablation: with `fx` as the double `0.8409`, the two `ismooth = 1` units differ in 8 lines each and the `ismooth = 0` unit in none. |
| 15 | MT 452/455/456 refused | `getyld` on MF=1 (`:6521-6644`): the LNU=2 TAB1 by `terpa`, or the LNU=1 polynomial | `groupr::spectrum::NuYield`, `PanelFeed::Nu` | GENDF identical, 1 342 lines (unit 91) |
| 76 | MT 452/455/456 refused | as 15, then `5 18` and `5 455`, whose `dntc` now comes from `3 455` | as above | GENDF identical, 7 203 lines, with `6 /` removed on both sides. As written, the deck stops at `6/` → MF=4/MT=18 (`getff` label 200 with `econst`, not ported), as deck 47 does. |
| 08 | `getmf6` LAW=2..5 refused | Ni-61 MT=51-58 are LAW=3. `getmf6` hands them to `getdis` (`:7956`, `:8096-8099`), and `getfle` with `mft = 8` makes them isotropic in the CM (`iso = 1`, `:9733-9737`). | `TwoBodyFeed` over `File4Angular::isotropic_cm` | With `16/` removed on both sides, MT=51-58 (514 lines) and everything outside MF=6 MT=16/28/91 are identical. **28 of 2 272 lines differ**, all in those three LAW=1 sections (`Mf6Feed`, not changed here), in the 7th figure of words near 1e-8. They are `LANG = 1`, `LEP = 1` with an incident `INT = 12`. Cause not examined (gh:#1517). `16/` itself stops at MF=12 `LO = 2` (gh:#1517). |
| 47 | `mfd = 5` refused | `5 18` now written; then `6/` reaches MF=4/MT=18 | none | stops at `getff` label 200 with `econst` (fission), as predicted |

**A false claim corrected.** `photon_prod.rs` refused MF=12 `LO = 2` with
"(upstream errors too)". In GROUPR, `getgyl` reads `conver`'s output tape, and
`conver` converts a LO = 2 transition array to LO = 1 yields
(`:11190-11420`). NJOY2016 runs deck 08 to the end. The refusal now names
`conver`.

Gates (`tests/deck_driver_tapes_vs_njoy2016.rs`):
- `test_29_whole_deck`;
- `test_18_groupr_gendf_whole_tape`, `test_27_groupr_gendf_moder_copy`, `test_34_groupr_gendf_moder_copy`;
- `test_15_groupr_gendf_whole_tape`;
- `test_76_groupr_gendf_without_auto_mf6`;
- `test_08_groupr_gendf_without_auto_mf16`.

**Survey:** the 21 decks that run GROUPR were re-run (`SURVEY_CASES`, 674 s) on
develop e1aa4acf9 plus this work. **48 of 87 are identical** (47 before); deck 29 is
the new one. Decks 15, 18, 27 and 34 now stop at ERRORR.

**Tests (release, on develop e1aa4acf9 plus this work):**

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` | 115 passed, 2 ignored (43, 80), 1 478 s |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `--lib` | 730 of 730 passed |

**Still refused at GROUPR:**
- `getff` label 200 for fission with `econst` (decks 47, 76);
- `conver`'s MF=12 LO=2 conversion (08);
- the temperature loop with `genflx`/`getunr` (02, 11);
- deck 17's earlier differences.
