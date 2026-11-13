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
MF=31/34/35, or COVR's plot option (~~all~~ **CORRECTED 2026-11-13:** MF=35 from a GENDF,
section 18, and COVR's plot option and MF=33 from a deck, section 19, are ported now).
GAMINR's numeric engine is not ported.
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
| 76 | MT 452/455/456 refused | as 15, then `5 18` and `5 455`, whose `dntc` now comes from `3 455` | as above | GENDF identical, 7 203 lines, with `6 /` removed on both sides. ~~As written, the deck stops at `6/` → MF=4/MT=18 (`getff` label 200 with `econst`, not ported), as deck 47 does.~~ **CORRECTED 2026-11-13 (section 15):** ported; the GENDF as written is identical. |
| 08 | `getmf6` LAW=2..5 refused | Ni-61 MT=51-58 are LAW=3. `getmf6` hands them to `getdis` (`:7956`, `:8096-8099`), and `getfle` with `mft = 8` makes them isotropic in the CM (`iso = 1`, `:9733-9737`). | `TwoBodyFeed` over `File4Angular::isotropic_cm` | With `16/` removed on both sides, MT=51-58 (514 lines) and everything outside MF=6 MT=16/28/91 are identical. **28 of 2 272 lines differ**, all in those three LAW=1 sections (`Mf6Feed`, not changed here), in the 7th figure of words near 1e-8. They are `LANG = 1`, `LEP = 1` with an incident `INT = 12`. ~~Cause not examined (gh:#1517). `16/` itself stops at MF=12 `LO = 2` (gh:#1517).~~ **CORRECTED 2026-11-13 (section 15):** the cause was `getmf6`'s `ismooth` histogram `fx`, not `INT = 12`; `16/` runs through `conver`'s `LO = 2` conversion; the GENDF is identical. |
| 47 | `mfd = 5` refused | `5 18` now written; then `6/` reaches MF=4/MT=18 | none | ~~stops at `getff` label 200 with `econst` (fission), as predicted~~ **CORRECTED 2026-11-13 (section 15):** the first GROUPR is identical; stops at the second (`nsigz = 2`) |

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
- ~~`getff` label 200 for fission with `econst` (decks 47, 76);~~ (**CORRECTED 2026-11-13:** ported, section 15)
- ~~`conver`'s MF=12 LO=2 conversion (08);~~ (**CORRECTED 2026-11-13:** ported, section 15)
- the temperature loop with `genflx`/`getunr` (02, 11) (still refused; section 15);
- deck 17's earlier differences.

## 15. GROUPR fission `econst`, `getmf6`'s histogram `fx`, `conver`'s MF=12 `LO = 2`, card 9 per temperature (2026-11-13, gh:#1369, #1517)

The targets were the GROUPR refusals left by section 14, in this order:
1. `getff` label 200 with `econst` (decks 47, 76).
2. Deck 08 (gh:#1517): `conver`'s MF=12 `LO = 2`, and the 28-line LAW=1 residual.
3. The temperature loop (decks 02, 11).

A prediction was posted on gh:#1369 (or gh:#1517) before each change.

**Search first.**
- `photon_prod::PhotonFeed` already had the `econst`/`jconst` branch for `mfd = 16`. `njoy_panel::group_general` already wrote its `ig = 0` and `ig <= jconst` records, but for `PanelFeed::Photon` only.
- `continuum::PhotonSed::from_tape_mf` is `getsed` on MF=5 with the `econst` search.
- `spectrum::NuYield` is `getyld` on MF=1.
- `acer::photon_blocks::Lo2Cascade` is ACER's `convr`, the same algebra as GROUPR's `conver`.
- `groupr::unresolved` and `urr_pendf` port `getunr` and `stounr`. Nothing ports `genflx` or the `nz > 1` paths of `panel` and `displa`.

All of these were reused. None was duplicated.

| deck(s) | was | cause (upstream) | fix | result |
|---|---|---|---|---|
| 76, 47 | `6/` → MF=4/MT=18 refused | `getff` label 200 (`groupr.f90:7132-7232`). `econst = ebig` for MT 18/19 (`:5831-5836`), lowered by `getsed`, snapped to a group bound by `jconst` with `egn(ig)*(1+small)`. First call (label 505, at the init `en`): `getyld`/`getfle` skipped, label 260 returns the bare `sede` (MT=18: `iglo = 1`, every group). Below `econst`: label 230, `ff = yld`, `enext = econst`, `idisc = 1`. Above: prompt nu (MF=1/MT=456) × `sede` × `fle`. | `ContinuumFeed` carries `getyld`, `PhotonSed` on MF=5 and `econst`/`jconst`; `group_general` takes the constant range from it too | **76: GENDF identical as written, 10 012 lines** (`test_76_groupr_gendf_whole_tape`). **47: first GROUPR's GENDF identical, 2 820 lines** (`test_47_groupr_gendfs_moder_copies`) |
| 08 | 28 of 2 272 lines in MF=6 MT=16/28/91 (7th figure) | **Not `INT = 12`**: `f6lab`'s corresponding-point branch (`:9101-9298`) was compared line by line with `kinematics::lab1` and matches. These are `LEP = 1` neutron spectra with `ismooth` on, so `getmf6`'s histogram sqrt(E) extension (`:7722-7763`) runs. Its `fx=.8409` is a default-real literal assigned to `real(kr)::fx` (`:7547`, `:7723`): `0.8409f32` widened. The port used the double. Deck 46's lin-lin branch has `fx = 0.50`, exact either way. | `ismooth_histogram`'s `FX = 0.8409f32 as f64` | **identical, 2 272 lines** without `16/`; the gate is now the whole tape |
| 08 | `16/` → MF=12 `LO = 2` refused | `conver` (`:10994-11007`, `:11188-11470`) writes `nend2`, which GROUPR's `getgyl`/`getgfl` read. It is text when `nendf > 0` (`:402`). Each `LO = 2` section becomes a `LO = 1` HEAD (`NK = l`), a total TAB1 constant at `ysum` over `[elow, emaxx]` when `l > 1`, and one TAB1 per line (`LF = 2`). `elow` is the MT's MF=3 threshold, `gety1(0)` on the ENDF tape. Once any `LO = 2` is seen, each level-series MF=14 becomes an isotropic HEAD with `NK = ngam(mt)`. | `groupr::conver::lo2_converted` (over `Lo2Cascade`), handed to `PhotonFeed` | **GENDF identical with `16/`, 2 972 lines** (`test_08_groupr_gendf_whole_tape`) |
| 02, 11 (and 47's second GROUPR) | `several ... temperatures or sigma-zeros` | `read_groupr` read one card-9 list. Upstream reads one per temperature (label 590 loops to label 360, `:586-630`), so deck 02's second list was read as card 10. | one list per temperature (`GrouprJob::later_temperatures`) | cards read. The run is still refused: `nsigz > 1` (`genflx` `:5309-5684`, `getflx` `:6457-6520`, `getsig`'s `getunr` `:6787-6796`, `nz > 1` in `panel`/`displa`) and the temperature loop are not translated |
| 17 | RECONR/BROADR differences, `iwt = 6` | not examined this round | none | unchanged |

**Predictions.** Every one posted before a change held. The issue's own first
pointer for deck 08 (`INT = 12`) was not the cause; reading `f6lab` first ruled it out,
and the prediction named the `fx` literal instead. The survey outcomes (08 identical,
76 to CCCCR, 47 to its second GROUPR, 02/11 at `nsigz > 1`) were predicted too.

**Survey:** the 21 GROUPR decks were re-run (`SURVEY_CASES`, 657 s) on develop
9d2e02800 plus this work. **49 of 87 are identical** (48 before); deck 08 is the new
one.

**Gates** (`tests/deck_driver_tapes_vs_njoy2016.rs`):
- `test_76_groupr_gendf_whole_tape`;
- `test_47_groupr_gendfs_moder_copies`;
- `test_08_groupr_gendf_whole_tape`;
- `test_08_groupr_gendf_without_auto_mf16`, now held to every line;
- `deck::modules::tests::groupr_reads_one_card9_list_per_temperature`.

**Tests (release, on develop 9d2e02800 plus this work):**

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` | 117 passed, 1 failed, 2 ignored (43, 80), 1 280 s. The failure was `test_47_...` in its first form, which also compared the second GROUPR (`nsigz = 2`, refused). It now gates the first GROUPR, and it passes alone (0 of 2 820 lines). |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `--lib` | 731 of 731 passed |

**Still refused at GROUPR:**
- ~~`nsigz > 1` (`genflx`, `getflx`, `getunr`, `nz > 1` in `panel`/`displa`): decks 02, 11, and 47's second GROUPR;~~ **CORRECTED 2026-11-13:** translated (section 16); 47's second GROUPR is identical;
- ~~the temperature loop: decks 02, 11;~~ **CORRECTED 2026-11-13:** translated (section 16); decks 02 and 11 now stop at `getwtf` `iwt = 4` and at `getsed` `LF = 7`;
- deck 17's earlier RECONR/BROADR differences and `iwt = 6`.

## 16. GROUPR `nsigz > 1`, the temperature loop, UNRESR's MT=152, `getco` CM to lab, `conver`'s MT=456 (2026-11-13, gh:#1369)

The targets were the ones section 15 left: `nsigz > 1` (decks 02, 11, 47's second
GROUPR), the temperature loop (02, 11), then deck 17 if time remained (it did not).
A prediction was posted on gh:#1369 before any code changed.

**Search first.**
- `groupr::unresolved::UnresolvedTable::shield` (`getunr`, `iovl = 0`), `overlap_context`
  and `urr_pendf::read_urr_table` (`stounr`) were reused. Only `getunr`'s `xtot`/`iovl`
  word tests were added around them (`flux_table::getunr`).
- `unresolved::genflx_bondarenko*` and the older `groupr::panel`/`matrix_panel` engine
  (golden-validated at 1e-5 on 6 sigma-zeros, `groupr/README.md`) were checked and not
  used. They evaluate the Bondarenko formula on a grid the caller gives. `genflx`'s
  table has its own nodes: `getwtf` steps, MT=1 breaks and `getunr` energies, plus the
  MT=261 P1 column. `getflx` interpolates between those nodes.
- `njoy_panel::{panel, group_general}` and `displa` were generalised to `(il, iz)`.
  `group_general` is now a wrapper over `group_general_nz` with `nz = 1`, so the
  existing gates check it.
- The multi-temperature PENDF is split into per-temperature blocks, as
  `deck::thermr::thermr_tape` does.
- `unresr::unresolved_cross_sections` (`unresl`) was reused for UNRESR's table. PURR's
  `urr_energy_grid` is `rdf2un`, not `rdunf2`, so it was not reused (see below).

| deck(s) | was | cause (upstream) | fix | result |
|---|---|---|---|---|
| 47 (second GROUPR), 02, 11 | refused: `nsigz > 1` | `genflx` Bondarenko branch (`:5380-5395`, `:5629-5671`), `getflx` `nsigz > 1` (`:6476-6510`, with the `ip` re-read after `scana`), `getsig` → `getunr` per `il`, MT=261 for MT=1's `il = 2` (`:6777-6797`), `init`'s `nz`/`nl` (`:5815-5838`), `panel` over `iz`, `displa` label 310 for every `nz > 1` | `groupr::flux_table` (`genflx`, `FluxCursor`, `getunr`, `stounr`); `njoy_panel::{FluxSource, group_general_nz}`, `SigFeed::get_nz`; `displa310` general in `(il, iz)`; the label 460 record rounds `iz = 1` only | first run of 47's unit 55: 65 of 2 980 lines differ, all in the URR groups (next row) |
| 47 | UNRESR passed its input through | `unresr` (`:136-330`) rewrites each temperature's MF=2/MT=152: `rdunf2`'s grid (`:426-748`: primer 1 MeV, `egridu` with `enut > 1.01 et`, a `sigfig(.,7,2)` thinning pass, overlap sign), `rdunf3`'s File-3 background (`:783-840`), `unresl` at each sigma-zero, `sigfig(.,7,0)`. Deck 47's MT=152 was RECONR's one sigma-zero table, so `getunr` returned at `nsig0 <= 1`. | `unresr::tape` (`unresr_grid`, `background`, `mt152_rows`, `unresr_tape`), called by the deck driver's UNRESR; `new = 1` (no MT=152 yet) is refused by name | **UNRESR's PENDF identical, 150 428 lines** (`test_47_unresr_pendf_with_mt152`); **47's unit 55 identical, 2 980 lines** |
| 02, 11 | refused: temperature loop | `:520-630`: per temperature, find the PENDF block (`1 + T/1e4` K), `stounr`, `genflx`, MF=1 head, that temperature's card-9 list with `mtd = 1`, `amend`; `atend` once | `run_groupr`'s temperature loop over `temperature_blocks` | deck 11 (next rows) |
| 11 | `getfle`: MF=4/MT=16 lab data refused | `getff` label 200 asks `getfle` for `lcd = 1` (lab, `:7152`). Lab data needs nothing. CM data goes through `getco`'s 64-point quadrature with `xn = (1+awr x)/sqrt(1+awr^2+2awr x)` (`:10550`, `:10684-10686`). Pu-238's MT=91 is a CM-isotropic table, which becomes four lab moments. | `File4Angular::from_tape_lab` (`File4Options::cm_to_lab`, `getco_legendre_converted`) | first run without it: 31 lines (MF=6 MT=91 written isotropic) |
| 11 | MF=1/MT=456 not found | `conver` adds MT=456 = MT=452 when MF=1 has neither 455 nor 456 (`:11473-11550`); `getyld` reads it from `nend3` | `groupr::conver::with_mt456` | **deck 11's GENDF without `6 18` identical, 10 019 lines** (3 temperatures × 7 sigma-zeros, `lord = 3`, MF=6 MT=221) (`test_11_groupr_gendf_without_mf6_mt18`) |
| 11 | -- | `6 18`: Pu-238's MF=5/MT=18 is analytic `LF = 7`, `getsed` → `anased`/`hnab` (`:12281-12627`) with the `econst` search | not translated (refused by name) | deck 11 stops at GROUPR `6 18` |
| 02 | -- | card 6 `iwt = 4` (thermal + 1/E + fission with `eb, tb, ec, tc`), `getwtf` | not translated (refused by name) | deck 02 stops at GROUPR `genflx` (`getwtf iwt = 4`); it also has `6 18` with `LF = 7` |
| 17 | RECONR/BROADR differences, `iwt = 6` | not examined | none | unchanged |

**Predictions.**
- Deck 47's unit 55 identical: held, but only after the UNRESR writer. The posted
  prediction placed a difference, if any, "inside the URR, from `getunr`'s node
  insertion or `terpu`". The cause was upstream of GROUPR: the URR table on the tape
  had one sigma-zero.
- Deck 11 runs and is identical "unless the THERMR MT=221 matrix exposes something
  new": it did not run at first. Three gaps outside `nsigz`/temperature were found:
  `getco` CM to lab, `conver`'s MT=456 and `anased`. Two are closed. With `6 18`
  removed, the GENDF is identical, and MT=221 is identical too.
- Deck 02 stops at `getwtf iwt = 4`: held.

**Survey:** the 21 GROUPR decks were re-run (`SURVEY_CASES`, 689 s) on develop b875f0ca0 plus this work. **49 of 87 are identical** (unchanged): no whole deck closed. Rows 02, 11, 47 and 86 changed. 47 now stops at ERRORR with both GENDFs identical; 11 stops at `6 18` (`anased`); 02 stops at `getwtf iwt = 4`; 86's tape25 went from 28 to 27 differing lines of 52 (not examined).

**Gates** (`tests/deck_driver_tapes_vs_njoy2016.rs`):
- `test_47_groupr_gendfs_moder_copies`, now always units 45 and 55 (`DECK47_BOTH` removed);
- `test_47_unresr_pendf_with_mt152`;
- `test_11_groupr_gendf_without_mf6_mt18`.

Unit tests:
- `flux_table::tests` (`getflx` interpolation, `getunr` without a table);
- `unresr::tape::tests` (the `rdunf2` grid);
- `conver::mt456_tests`;
- `file4::tests::cm_isotropic_converts_to_forward_lab_moments`.

**Tests (release, on develop b875f0ca0 plus this work):**

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 120 passed, 0 failed, 2 ignored (43, 80), 1 689 s |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `--lib` | 736 of 736 passed |

**Still refused at GROUPR:**
- ~~`getsed` analytic spectra (`anased`/`hnab`, `LF = 7` here): decks 02, 11 (`6 18`);~~ **CORRECTED 2026-11-13:** laws 7 and 9 translated (section 17); laws 11 and 12 remain, no deck seen;
- ~~`getwtf` `iwt = 4`: deck 02;~~ **CORRECTED 2026-11-13:** translated (section 17);
- `genflx`'s slowing-down branch (`iwt < 0`) and `iwt = 0` (`getfwt`): no deck seen;
- UNRESR on a PENDF with no MT=152 (`new = 1`): no deck seen;
- ~~deck 17's earlier RECONR/BROADR differences and `iwt = 6`.~~ **CORRECTED 2026-11-13:** both closed (section 17).

## 17. GROUPR `anased` laws 7 and 9, `getwtf` `iwt = 4` and `6`, RECONR's seam between resolved ranges, MODER `nin = 2` (2026-11-13, gh:#1369)

The targets were the ones section 16 left: `anased` `LF = 7` (decks 02 and 11 at `6 18`),
`getwtf` `iwt = 4` (deck 02), then deck 17 (RECONR/BROADR, then `iwt = 6`), then ERRORR
if time remained. A prediction was posted on gh:#1369 before each change.

**Search first.**
- `groupr::continuum::PhotonSed` was already `getsed` on MF=5/15 with the `econst`
  search and an analytic `LF = 5` subsection. It was generalised to `anased`'s laws 5, 7
  and 9 (the `theta` TAB1 alone for 7 and 9). `SedFeed` (`anased9`) stays the
  non-fission `mfd = 6` route. A unit test pins that the two give the same law-9 values,
  bit for bit.
- `erfc` (SLATEC `derfc`, `mathm.f90:447-634`) was already ported in `broadr::joint`. It
  was made `pub(crate)` and reused rather than copied.
- `groupr::weights::ThermalFissionParams` (`genwtf`'s `ab`, `ac`) and
  `thermal_fission_fusion` (the `iwt = 6` value) were reused. Only `getwtf`'s `enext`
  ladders were new.
- `reconr`'s `refine_panel`/`test_panel` (`resxs`'s panel test and step-increase rule)
  were reused for the seam. The SLBW/MLBW and RM per-energy closures moved, unchanged,
  into `reconr::seam::{slbw_delta, rm_delta}`, so the range rebuild and the seam call one
  function.

| deck(s) | was | cause (upstream) | fix | result |
|---|---|---|---|---|
| 11, 02 | `6 18` refused: `getsed` `LF = 7` | `getsed` reads `theta` and lowers `econst` (`:11902-11924`). `anased` law 7 (`:12364-12374`): `bot = rp4 (1-erfc(sqrt xc)) - sqrt(xc) exp(-xc)` on a `new` call, `top = rl e^-xlo - rh e^-xhi - rp4 (erfc rh - erfc rl)`. Pu-238 B-IV MT=18: `U = -3e7`, constant `theta = 1.3333e6`. | `PhotonSed` analytic laws 7 and 9 (`continuum::anased7`); fission law 9 goes through `PhotonSed` too | **deck 11's GENDF as written identical, 10 379 lines** (`test_11_groupr_gendf_whole_tape`, which replaces `test_11_groupr_gendf_without_mf6_mt18`) |
| 02 | `genflx` refused: `getwtf iwt = 4` | `genwtf` stores `eb, tb, ab, ec, tc, ac` (`:5036-5052`); `getwtf` (`:5206-5220`): three branches, `enext = 1.01 e` stopped at `eb` and `ec` | `njoy_panel::genwtf_iwt4`, `weights::getwtf_iwt4` | **GENDF identical, 6 213 lines, and UNRESR's PENDF, 13 873** (`test_02_groupr_gendf_and_unresr_pendf`); deck stops at CCCCR |
| 17 | RECONR: U-238 (tape41) 12 lines short, Pu-239 (tape43) 60 | `resxs` (`:2240-2569`) walks every resolved range in one pass, and `sigma` takes each on `[el, eh)`. The panel from `sigfig(eh,7,-1)` to `sigfig(el,7,+1)` at a boundary between two resolved ranges (JENDL-3.3 U-238 has 11, Pu-239 three) is tested, and its midpoint belongs to the upper range. The port refined each range on its own grid. | `reconr::seam::refine_resolved_seams` | first run: Pu-239 +48 of 60 lines, U-238 +0 |
| 17 | same | `resxs`'s step-increase state (`in`, `res(1)`, `:2419-2421`) carries over the boundary, so the upper range's first panels can be split (NJOY: 1000.003 … 1000.019 eV on Pu-239). The port restarted it in each range (`in = 0`). | `seam::rewalk_from` re-tests the upper range's first panels with the carried state, until it coincides with the per-range walk's | **tape41 (497 335 lines) and tape43 (303 613) byte-identical**; BROADR's 31 and 33 follow |
| 17 | GROUPR refused: `iwt = 6` | `getwtf` (`:5222-5245`): `enext = 1.01 e`, `1.005 e` within 1.6 MeV and `1.002 e` within 0.33 MeV of 14.07 MeV | `weights::getwtf_iwt6` | **tapes 91-93 identical** |
| 17 | MODER `2 99`: label only | `nin = 2` (`moder.f90:110-205`, `:289-330`): the card-2 label with `MAT = 1`, each pick's GENDF records through MEND (every temperature), TEND | `deck::run::select_gendf_text` on the deck driver's text GENDF units | **tape99 identical, 3 548 lines**; the deck stops at ERRORR (`test_17_through_moder_merge`, ten tapes) |

**Predictions.**
- `LF = 7`, deck 11 identical with `6 18`: held. So did deck 02 identical through GROUPR,
  the risk named being MT 251-253/259 under `nsigz = 7`.
- Deck 17 RECONR: "the seam adds the 1e3 points, the step-increase state is the next
  suspect". Both held, in that order. U-238's line count did not move with the seam
  alone, because its missing points were all step-increase splits.
- `iwt = 6`: tapes 91-93 identical. This held. The prediction named MODER's merge or
  ERRORR as the next stop: MODER's `nin = 2` was refused first (it wrote the label alone),
  and once it was translated the deck stops at ERRORR.

**ERRORR (target 4) was not started.** Each candidate deck's ERRORR card was read:
- 15: `ngout = 91`, MF=31 (`ifissp = -1`), then MF=34;
- 18: MF=35 with a 71-group user structure and `efmean`;
- 27 and 47: MF=35 twice;
- 34: MF=35;
- 46: MF=34 with a 238-group structure.

All read an input GENDF (`ngout`). None is the plain MF=33-from-PENDF path that
`errorr::driver`/`run_mf33` cover. The cheapest is deck 34 (one ERRORR, MF=35, binary
`ngout`): it needs the deck-driver card reader, `ngout` input, and the MF=35 (`covcal`
for spectra) path.

**Survey:** the 21 GROUPR decks were re-run (`SURVEY_CASES`, 649 s) on develop 6834385cb
plus this work. **49 of 87 are identical** (unchanged): no whole deck closed, because each
deck now stops at a module that has no driver. Rows 02, 11 and 17 changed:
- 02 stops at CCCCR;
- 11 stops at WIMSR;
- 17 has ten identical tapes (four of them differed before) and stops at ERRORR.

**Tests (release, on develop 6834385cb plus this work, after the last code or test change):**

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 122 passed, 0 failed, 2 ignored (43, 80), 1 651 s |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `--lib` | 741 of 741 passed |

**Gates** (`tests/deck_driver_tapes_vs_njoy2016.rs`):
- `test_11_groupr_gendf_whole_tape` (replaces the `6 18` exemption);
- `test_02_groupr_gendf_and_unresr_pendf`;
- `test_17_through_moder_merge`.

Unit tests:
- `groupr::continuum::analytic_tests`: law 9 through `PhotonSed` against `SedFeed`, bit
  for bit; law 7 against a 20 000-panel Simpson integral of the Maxwellian, `1e-10`;
  `getwtf` `iwt = 4`;
- `reconr::seam::tests` (`sigma`'s `[el, eh)`);
- `deck::run::moder_gendf_tests`.

## 18. ERRORR `mfcov = 35` from an input GENDF (2026-11-13, gh:#1369)

Target: the ERRORR decks that section 17 left, deck 34 first. A prediction was posted on
gh:#1369 before any code changed.

**Search first.** `src/errorr/` had the MF=33-from-PENDF chain (`gridd`, `merge`,
`uniong`, `grpav`, `covcal` LB 0-6/8, `sigc`/`covout`, `resprx`, `run_mf33`) and a card
model (`driver::ErrorrInput`) whose `run` is a `NotPorted` skeleton. Nothing called ERRORR
from a deck. `ngchk`, `colaps`, `rdgout`/`rdchi`, `sumchk`/`spcint`/`covbin`, `fssigc`,
`covcal`'s LB=7 branch and `covout`'s MF=35 form were not anywhere in `src/`. Reused:
`gridd::{merge, uniong}`, `groups::neutron_group_structure`, `mixr::mix::sigfig`,
`endf::interp::{intega, terpa}` and `deck::groupr::GendfText`'s writers.

**New code.**
- `errorr::gendf`: a GENDF as records (`GendfRecord`), read from text the way
  `contio`/`listio` read it, and `ngchk` (`errorr.f90:10865-11152`).
- `deck::groupr::GendfText` now also keeps every record at full precision, and
  `TapeUnits::put_gendf`/`gendf` hand ERRORR GROUPR's words for a **binary** unit (what
  NJOY's blocked-binary GENDF holds) and the parsed text for a coded one.
- `errorr::colaps` (`:9097-9533`): MF=3 onto the union grid, chi from MF=5 (`mffis = 5`) or
  from a GROUPR MF=6 matrix (`mffis = 6`, `:9300-9440`). The chi translation tests
  `er <= ea3` with the values the last MF=3 section left (`:9461`), as upstream does.
- `errorr::mf35` (`run_mf35`): the subsection scan (`:571-640`), `gridd`'s MF=35 energies,
  `sumchk`/`spcint`/`covbin`, the LB=7 rows, `fssigc`, `covout` and the `nout` tape.
- `deck::errorr`: the cards (1, 2, 3, 7, 12, 13b) and the run. Anything but `mfcov = 35`
  is refused by name after the cards are read.

| deck | what it exercises | result |
|---|---|---|
| 34 | Pu-240 B-VIII.0, binary GENDF, `ifissp = 1`, one subsection, 301-group union grid | **covariance tape (MODER copy 50) identical, 175 lines**, first run |
| 18 | Cf-252 decay data, **text** GENDF unit 91, 71 user groups (`ign = 1`), `efmean = 1.23e6` | **tape 28 identical, 961 lines**, first run |
| 27 | Pu-239 B-VIII.0 beta 6, four subsections, `ifissp = 2`, then `-1` with `efmean = 5.75e6` | first run: tape 56 differed in 184 of 199 lines (row `ig = 1` missing upstream); after the fix below **56 and 57 identical, 199 lines each** |
| 47 | Pu-239, GENDF with MF=6 (`mffis = 6`), two GROUPR runs | **48 and 58 identical, 199 lines each**, first run after `mffis = 6` was translated |

**The deck 27 difference was upstream's record handling, and it is kept.** `covout` skips
a row of another subsection with one `listio`, without reading its continuation records.
When the last row of subsection 1 (`jg = 601`) has a continuation record, the next
`listio` reads that record as a row head: `listio` recomputes `nb = 6 + nint(a(5)) - nw`
(`endf.f90:174`), here `-289`, so the "skip this record" loop reads the **next** record,
row `jg = 1` of subsection 2, as continuation and drops it. Found with an instrumented
`errorr.f90` (scratch copy, one `write` after `covout`'s `listio`, relinked against the
build's other objects), then reproduced in `mf35::covout_rows`, which walks the scratch
tape record by record (`npage = 306`, gfortran's `nint` as `lround` truncated to 32 bits,
measured). The first hypothesis, garbage `jg` from skipped continuation records, was
tested and did not change the result.

**Predictions** (posted before the change): deck 34's covariance tape identical — held;
chi equal to GROUPR's word for word — held; the deck then stops at COVR — held.

**Gates** (`tests/deck_driver_tapes_vs_njoy2016.rs`):
- `test_34_errorr_mf35_covariance_tape` (unit 50);
- `test_18_27_47_errorr_mf35_covariance_tapes` (COVR and VIEWR removed from the decks,
  which write no tape another module reads; units 28, 56, 57, 48, 58).

Unit tests: `errorr::gendf::tests`, `errorr::colaps::tests`, `errorr::mf35::tests`,
`deck::errorr::tests`.

**Not done (refused by name):** ERRORR ~~`mfcov = 31`, ~~`33`~~ and `34`~~ from a deck (decks 04,
05, 15, 16, 17, 20, 46, 65: ~~`run_mf33` exists for MF=33 from a PENDF but is not wired to
the deck yet~~ **CORRECTED 2026-11-13:** MF=33 from a PENDF is wired, section 19; MF=31 and
MF=34 are wired, section 20), `iread = 1/2`, `nstan`, ~~`nin`, `nendf = 999` (`covadd`)~~ (wired,
section 20), `iwt = 1`, an MF=35
MT other than 18, MF=5/MT=18 with more than one subsection or `LF /= 1` in `spcint`, and
~~COVR (decks 18, 27, 34, 47 now stop there)~~ **CORRECTED 2026-11-13:** COVR's plot option
runs and those four decks are identical whole, section 19.

**Survey:** the ERRORR decks 04, 05, 15, 16, 18, 20, 27, 34, 46, 47 and 65 were re-run
(`SURVEY_CASES`, 320 s) on develop cc488ffed plus this work. **49 of 87 are identical**
(unchanged). 18, 27, 34 and 47 stop at COVR; 04, 05, 15, 16, 46 and 65 at the named
refusal `mfcov other than 35`; 20 at `nendf = 999`.

**Tests (release, on develop cc488ffed plus this work, after the last code or test change;
only doc comments and Markdown changed after these runs):**

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 124 passed, 0 failed, 2 ignored (43, 80), 1 547 s |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `--lib` | 745 of 745 passed |

## 19. COVR's plot option, ERRORR `mfcov = 33` from a deck (2026-11-13, gh:#1369)

Targets: COVR, which decks 18, 27, 34 and 47 stopped at after section 18, then ERRORR
MF=33 from a deck (deck 05 first). A prediction was posted on gh:#1369 before each change.

**Search first.** `src/covr/` had the card model, the ERRORR-tape reader
(`ErrorrTapeKind`, `read_group_structure`, `read_vector` with `einc`,
`read_covariance_rows`, `present_mts`), `expand_mt_pairs`, `covard`'s numeric half
(`to_dense`) and the library option, byte-identical to NJOY on nine tapes. The plot
option was refused (`NotPorted("covr::plot")`), and the deck driver had no COVR card
reader. VIEWR already ran (section 9). Run on NJOY's own COVR plot files, it gave
PostScript identical to NJOY's on deck 18 and deck 47's first file. Deck 34 and deck 27's
first file differed only in the sign of a zero in a rotated font matrix: deck 47's committed
reference has our sign for the same plot text, so the committed reference tapes come from
different builds. The gate is the local NJOY2016.80 binary. `errorr::run_mf33` and
`ErrorrResult::to_tape` existed, checked by value and through COVR's BOXER library but never
as text, and no deck called them. No `review.md` in `src/covr`, `src/deck`, `src/errorr` or
`src/viewr` has a `kind = "review"` block.

**New code.**
- `covr::plot` (`run_plot`): the plot option of `covr` (`covr.f90:150-505`). It includes
  `corr`'s plot branch with the `epmin` doubling search that raises the module `epmin`,
  `covard` (keeping upstream's stale `izero` for a lumped placeholder), `truncg`, `plotit`,
  `matshd`'s pattern flood and legend, `level` and `patlev` (whose `10./(nlev-1)` is
  single precision, as upstream's).
- `covr::plot_label`: `smilab`, `matmes`, `elem`, `mtno`. These include the `character`
  cuts upstream makes: `l2 = 35` of a 36-character `str2` drops a blank.
- `deck::covr`: COVR's cards. A binary ERRORR unit is read from ERRORR's records at full
  precision (`tape_from_records`), a coded unit from its text. The library option is refused
  by name from a deck.
- `deck::errorr`: `mfcov = 33` with `ngout = 0` runs `run_mf33`. Card 13b (`iwt = 4`) is
  kept now (it was read and dropped). `mf33_text` writes `to_tape`'s sections as `contio`
  and `listio` records with SEND, FEND, MEND and TEND.

| deck | what it exercises | result |
|---|---|---|
| 34 | Pu-240, binary ERRORR unit, `epmin` from the doubling search, `icolor = 1` | plot unit 33 (1 150 lines) and PostScript 43 (3 907) **identical**, first run |
| 18 | Cf-252, text ERRORR unit 28, `epmin = 1e3`, 71 groups | 38 (8 270) and 39 (17 461) **identical**, first run |
| 27 | Pu-239, two ERRORR/COVR/VIEWR runs; the second VIEWR inherits the first's state | 36, 37 (1 565 each), 46, 47 (4 803, 4 802) **identical**, first run |
| 47 | Pu-239, chi from GROUPR MF=6, two runs | 46, 56 (1 565 each), 47, 57 **identical**, first run |
| 05 | `t511` MAT 1306, ERRORR `mfcov = 33` from a PENDF, `ign = -1`, 101-group union grid; COVR over every MT pair, cross pairs included | ERRORR unit 33 (21 471 lines), plot 34 (78 796) and PostScript 35 (199 652) **identical**, first run |

**Predictions** (posted before the changes). The COVR plot files would be identical, so the
four `mfcov = 35` decks would close whole: held. Deck 05's ERRORR tape would be identical
except for a few last-figure words in MF=33, which had only been gated by a tolerance: held
better than predicted, with no word differing.

**Gates:** `tests/deck_driver_covr_vs_njoy2016.rs`. Each test runs the whole upstream deck
in NJOY2016.80 and here and compares every listed unit line by line:
- `test_34_covr_plot_and_viewr_whole_deck`;
- `test_18_...`, `test_27_...`, `test_47_...`;
- `test_05_errorr_mf33_covr_viewr_whole_deck`.

Unit tests: `covr::plot::tests`, `covr::plot_label::tests`, `deck::covr::tests`.

**Not done (refused by name):** ERRORR ~~`mfcov = 31` and `34`, MF=33 from an input GENDF
(`colaps` for MF=33, deck 65), `nendf = 999` (deck 20) and~~ `iread = 2` (deck 17).
**CORRECTED 2026-11-13:** MF=31, MF=34, MF=33 from a GENDF and `nendf = 999` run, section
20. (Deck 65 needed MF=34 from a GENDF, not MF=33.) COVR's
library option from a deck. The plot of an MF=40 tape. `corr`'s restart when a second
material has more groups.

**Survey:** the ERRORR decks 04, 05, 15, 16, 18, 20, 27, 34, 46, 47 and 65 were re-run
(`SURVEY_CASES`) on develop c8bbe9dfa plus this work. **54 of 87 are identical** (49
before): 05, 18, 27, 34 and 47 closed. Decks 04 and 16 now pass their first ERRORR
(`mfcov = 33`, tapes 23 and 26 identical) and stop at `mfcov = 31` and `34`.

## 20. ERRORR `mfcov = 34`, `mfcov = 31`, MF=33 from a GENDF, `covadd` (2026-11-13, gh:#1369)

Targets, in order: MF=34 (decks 46, 65, 16, 15), MF=31 (decks 04, 15), then `covadd` with
`nendf = 999` (deck 20). A prediction was posted on gh:#1369 before each change, with one
exception: **deck 04 (MF=31) was coded and run before its prediction was posted.** That is
recorded on the issue. Deck 04's result is therefore not a tested prediction.

**Search first.** `src/errorr/` had nothing for MF=34 (`grpav4`, `egtlgc`, `musigc`, `rdlgnd`,
`alsigc`, `matrixej`) and nothing for MF=31. It did have the MF=33 chain (`gridd`, `lumpmt`,
`covcal`, `sigc`, `covout`), and `ngchk`/`colaps` from the MF=35 path. `errorr::covadd` existed,
but it works on a parsed `Tape`; upstream's `covadd` is a line-by-line text copy. No `review.md`
in `src/errorr`, `src/deck` or `src/covr` has a `kind = "review"` block.

**New and changed code.**
- `errorr::mf34` translates the MF=34 path:
  - `gridd`'s MF=34 branch: `ltt4`/`lct4`/`eltt4`, and energies from sub-subsections with
    `L, L1 <= legord`;
  - `grpav4`'s GENDF branch, which collapses MT=251 mubar weighted by `dE*sigma_el` and
    applies the `oneeps` tests;
  - `covcal` for the `(legord, ifissp)` sub-subsection, as `F*alp1*alp2`, with rows times
    `(sig flx)(sig flx)`;
  - `musigc`, `rdlgnd`, `alsigc`, and `covout`'s MF=34 form.

  It has two entry points: `run_mf34_gendf` and `run_mf34_pendf`.
- `errorr::grpav4` translates `grpav4`'s ENDF branch:
  - `egtlgc`, which steps one Legendre record per call and returns zeros past the last;
  - `epanel` with `mfcov = 34`;
  - row 2 of `matrixej` (`u1lele`), which is all upstream uses;
  - `plele`, and the `lct4 = 2` scaling.
- `errorr::mf31::run_cov_gendf` runs MF=31 and MF=33 from a GENDF. It reuses the MF=33 chain:
  `CovarianceReactions::mf()` selects MF=31 in `gridd`, `lumpmt` and `covcal`, and
  `scan_reactions_mf` builds the reaction list. `union_group_xs` reads `colaps`'s tape the way
  `rdsig` does: word 2, which is the nubar for MT 452/455/456.
- `covcal::contribution` now multiplies left to right, `F*sig(jg)*sig1(jh)`, as upstream
  writes it. Before, it computed `F*(sig*sig1)`.
- `deck::errorr` gained:
  - the MF=31, MF=34 and MF=33-from-GENDF runs;
  - `copy_nin_ahead`, which copies `nin` ahead of the output (`covout`, `:7144-7158`); every
    copied line is renumbered, and the next MF=1 continues the count;
  - `covadd_text` for `nendf = 999` (`:6907-6967`), whose text goes on the unit through
    `TapeUnits::put_endf_text` so that MF=32 is not re-rendered;
  - `covout_first_seq` (see deck 20 below).
- `deck::pendf` (RECONR's MF=2/MT=151 stub) now takes `spin` for an LRF=7 range as
  `rdsammy` does: the `IB` of the last particle pair with MT=2 (`samm.f90:1084`).

| deck | what it exercises | result |
|---|---|---|
| 46 | Fe-56 JEFF-3.3, 238 groups, MF=34 LB=5/1/5, binary GENDF | unit 34 (8 002 lines) identical, first run |
| 65 | U-235 B-VIII.0, MF=34 with six sub-subsections (only `(1,1)` kept), COVR, VIEWR | 41, 51, 61 identical, first run |
| 16 | U-238 JENDL-3.3, MF=33 and MF=34 from a PENDF (`grpav4` from MF=4 LTT=1, `u1lele`), two COVR/VIEWR | 26, 27, 36, 37, 46, 47 identical, first run |
| 04 | `t511` U-235: MF=33 from a PENDF, GROUPR nubar, MF=31 from the GENDF with `nin = 23` copied ahead | 23, 24, 25 identical, first run (no prediction posted first) |
| 15 | U-238 JENDL-3.3, text GENDF: MF=31, MF=33 (with MF=32 `resprx`) and MF=34 from the GENDF, three COVR/VIEWR | 25-27, 35-37, 45-47, 91 identical, first run |
| 20 | Cl-35 R-matrix limited: `covadd`, RECONR, BROADR, GROUPR, MF=33 from the GENDF with LRF=7 `resprx`, COVR, VIEWR | first run: 22 and 23 differed in 1 line, 25 in 13; after two fixes, 21-23, 91, 25-27 identical |

**Deck 20's two differences were both upstream behaviour, read from the source.**
- **PENDF line 177 (MF=2/MT=151, record 4):** NJOY wrote `spin = 1.5`, and we wrote 0.
  `recout` writes RECONR's module `spin` (`reconr.f90:5199`). For LRF=7, `rdsammy` sets that
  variable to the elastic pair's target spin (`samm.f90:1084`). Decks 81 and 88 have a target
  spin of 0, so they could not show this.
- **Unit 25, MF=33/MT=1:** that section's sequence numbers start at 324 rather than 1. The
  cause:
  - With `nendf > 0`, `resprx` opens a **coded** scratch unit `nscr6` and `rdumrd2` copies
    MF=2/MT=151 there, 323 lines (`errorr.f90:3062-3066`, `:5091-5225`).
  - NJOY's `nsh` sequence counter is one variable shared by every unit, and nothing resets it
    before `covout`'s first `contio`.
  - The count was checked against the tape's MF=2 section, which has 323 lines.
  - Decks 15 and 16 have `nendf < 0` (a binary scratch unit), so the counter does not move
    there.

**Gates** (`tests/deck_driver_covr_vs_njoy2016.rs`, whole decks against the local NJOY2016.80):
`test_46_...`, `test_65_...`, `test_16_...`, `test_04_...`, `test_15_...`, `test_20_...`.
Unit tests are in `errorr::mf34::tests`, `errorr::grpav4::tests` and `errorr::mf31::tests`.

**Not done (refused by name):**
- MF=34 with more than one MT=2 subsection, `MAT1 /= 0` or `MT1 /= 2`, a sub-subsection with
  `NI = 0`, or `legord > 9`;
- `grpav4` with `LVT = 1`, or with a second Legendre energy above the union grid;
- MF=40;
- `iread = 1/2` (deck 17);
- `nstan`.

**Survey:** the ERRORR decks 04, 05, 15, 16, 17, 18, 20, 27, 34, 46, 47 and 65 were re-run
(`SURVEY_CASES`) on develop 588aef22e plus this work. **60 of 87 are identical** (54 before):
04, 15, 16, 20, 46 and 65 closed. Deck 17 still stops at `iread = 2`.

**Tests (release, on develop 588aef22e plus this work; only doc comments in `src/errorr/mod.rs`
and Markdown changed after the deck suite was built):**

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 124 passed, 0 failed, 2 ignored (43, 80), 1 862 s |
| `deck_driver_covr_vs_njoy2016 --test-threads=2` | 11 of 11 passed |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `errorr_mf33_golden`, `errorr_mf32_ar37_golden`, `covr_boxer_golden` | 12, 4, 18 passed |
| `--lib` | 755 of 755 passed |
