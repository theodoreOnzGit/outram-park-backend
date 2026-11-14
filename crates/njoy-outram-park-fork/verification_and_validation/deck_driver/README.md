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
script; **CORRECTED 2026-11-14:** `gatef2` was translated but called by nothing, and
the SCT kernel had no second-atom term, until §36), `sigcoh` for `lat = 1..3` with `form` (`Sigcoh::builtin` in
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
~~GAMINR's numeric engine is not ported.~~ **CORRECTED 2026-11-13 (§27):** the engine was
ported (`src/gaminr/matrix.rs`); its tape control flow is translated now and decks 03 and 30 are identical.
~~PLOTR is not ported at all (`src/plotr/mod.rs`).~~ **CORRECTED 2026-11-13 (§26):**
PLOTR is translated (`src/plotr/`) and decks 06 and 12 are identical. ~~All three are left as large
ports.~~

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
| 56, 57, 58, 66, 78 | `NotPorted`: ACER `iopt = 5` | none | as above | ~~the builder refuses by name: MF=4 angular data (56), MF=6 `LCT != 1` (57, 58, 78), MT=18 nubar (66)~~ **CORRECTED 2026-11-13:** 57, 58, 66 and 78 build, ACE identical; ~~56 still stops at MF=4 (section 21)~~ **CORRECTED 2026-11-13:** all five are identical whole decks (section 22) |
| 43 | `differs`: tape35 320 lines | at `tempef <= 0` BROADR only thins (`thinb`, `broadr.f90:1677-1760`), which is not ported (gh:#619); the driver passed RECONR's grid through | the driver now refuses it (`NotPorted`) | `NotPorted`, honestly |
| 80 | `leapr: card reader not yet translated` | not investigated | none | ~~runs; 17 369 of 91 453 lines of MF=7/MT=4 differ in the last printed figure (`test_80_leapr_whole_deck`, ignored with that reason)~~ **CORRECTED 2026-11-13 (§25):** 24 264 values differed, most by more than 3e-5, all low (LEAPR `sbfill`'s persistent `sb`); now identical, gate un-ignored |

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

## 21. Photo-nuclear: RECONR on a photo-nuclear tape, `acephn` translated, `phnfix` (2026-11-13, gh:#1369)

Targets: decks 56, 57, 58, 66 and 78 (photo-nuclear ACER, `iopt = 5`, then two `iopt = 7`
check passes and VIEWR). A prediction was posted on gh:#1369 before each of the three
changes, and each held on the first run.

**Search first.** `src/acer/photonuclear/build.rs` already built the LANL-style path
byte for byte on the synthetic Z6 tape. `acer::acensd::{ptleg2, pttab2}`,
`acer::acelf6::bachaa` and `acer::acelcp::eavl` (made `pub(crate)`) are reused, not
rewritten. Nothing ported `phnfix` or `phnplo`. No `review.md` under `src/acer`,
`src/reconr` or `src/deck` has a `kind = "review"` block.

**1. RECONR on a tape with no MF=2 and no MT=1** (`reconr.f90`):
- `anlyzd` makes MT=1 redundant as soon as there is any MF=3 (`:550-556`), and seeds
  `mfs(2)=2, mts(2)=151` unless the tape has MF=23 (`:648-657`). `reconr::seed_redundant_total`
  adds the MT=1 to be summed; `deck::pendf::default_mf2_stub` writes the stub with
  `spin = ascat = 0` and `EH = ehigh = 2e7` (`LRP = -1`, `:301-317`).
- `recout` writes the total as `mth = 3` when `awin = 0` (`:5283-5286`, dictionary
  `:5103-5107`), closed by `asend` like any redundant sum
  (`deck::pendf::relabel_photonuclear_total`).
- Result: tape90 (RECONR's PENDF through MODER) identical on decks 58 (4 668 lines),
  66 (124) and 78 (57). It had differed on nearly every line.

**2. `acephn` translated line for line.** The old builder's passes were built around one
MF=6 shape (`LCT = 1`, `LANG = 1`, `NA = 0`) and refused the rest. Its internals are now a
line-for-line translation of `acepn.f90:27-1853` (`build.rs`, with the record helpers in
`records.rs`): XSS 1-based and zero-filled, written at upstream's offsets in upstream's
order with its `sigfig` calls; the integer/real split still comes from `layout::walk`.
New on the way:
- reactions are read from the **PENDF** (after the grid section, or from MT=5 when it is
  first), with the MT=103-107 skips;
- `LCT /= 1` → `TYRP = -1`; `LANG = 2` → law 44 (`bachaa` with `izai = 0`, `eavl`);
  `NA > 0` → `ptleg2`;
- MF=6 `LAW = 2` → ANDP tables (`ptleg2`/`pttab2`), law 33, and its two-body heating;
- LAW=1 recoil heating (`nr6`), MT=102 `e+q` heating (`icapt`), `copynubar` for MT=18,
  `NXS(9) = LISO`.

Still refused by name: `ielas = 1`, ~~MF=4, MF=5 for a production reaction,~~ MF=6 `LAW = 4`
(**CORRECTED 2026-11-13:** MF=4/MF=5 translated, section 22; `LAW = 4`'s recoil heating,
section 23).
- Result: ACE unit 31 and directory 32 identical on decks 57 (57 803 lines), 58 (132 970),
  66 (259 515) and 78 (3 488). The synthetic Z6 gate still holds byte for byte.
- One deliberate divergence: a photo-atomic tape (`NSUB = 3`) passes upstream's
  `NSUB/10 = 0` test; it is refused here as the wrong sublibrary.

**3. `phnfix`** (`acer::phnfix::phnfix_type1`, `acepn.f90:1855-1959`): the table is read
with `read::parse_type1`, `hk` replaced, and re-printed through `to_type1_string` (which is
`phnout`'s layout); the directory line is `phnout`'s. ~~`phnplo` (`:2830-3573`) is **not
ported**; the deck driver writes the table and directory and then stops by name.~~
**CORRECTED 2026-11-13:** `phnplo` is ported (section 22).
- Result: units 34 and 35 identical on 57, 58, 66 and 78.

**Gates** (`tests/deck_driver_photonuclear_vs_njoy2016.rs`, decks run as written against the
local NJOY2016.80): units 90/31/32/34/35 have zero differing lines, and the run must stop at
`phnplo` and nothing earlier. **SUPERSEDED 2026-11-13:** the gates are whole decks now (section 22).

**Not done:** (**CORRECTED 2026-11-13:** the first two are done, section 22)
- ~~`phnplo`, the `iopt = 7` plot file (units 33 and 36), so the second check pass (37, 38)
  and VIEWR (43, 46) are not reached. It is the last step for decks 57, 58, 66 and 78.~~
- ~~Deck 56 (U-235 ENDF/B-VIII.0): MF=4/MT=18 and MF=5/MT=18, `acephn`'s MF=4/5 neutron
  branches (`:742-818`, `:1166-1425`). Their multiplicities are keyed on `mth`, a header
  variable left over from the last record read.~~
- MF=6 `LAW = 4` (**CORRECTED 2026-11-13:** its recoil heating is translated, section 23),
  `ielas = 1`.

**Survey:** decks 56, 57, 58, 66 and 78 were re-run alone (`SURVEY_CASES`) on develop
7a3fd98b1 plus this work. **60 of 87 are identical** (unchanged): no whole deck closed.
57, 58, 66 and 78 now have 4 or 5 identical tapes each and stop at `phnplo`; 56 stops at MF=4.

**Tests** (release, on develop 7a3fd98b1 plus this work, run after the last code or test
change; only Markdown changed afterwards):

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 124 passed, 0 failed, 2 ignored (43, 80), 1 518 s |
| `deck_driver_photonuclear_vs_njoy2016` | 4 of 4 passed |
| `deck_driver_covr_vs_njoy2016` | 11 of 11 passed |
| `acer_photonuclear_vs_njoy2016` | 5 of 5 passed |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `--lib` | 756 of 756 passed |

## 22. Photo-nuclear decks whole: `phnplo`, and `acephn`'s MF=4/MF=5 neutron branches (2026-11-13, gh:#1369)

Targets: decks 57, 58, 66 and 78 (the last step was `phnplo`) and deck 56 (U-235, MF=4/MF=5).
A prediction was posted on gh:#1369 before each change; both held on the first run.

**1. `phnplo`** (`acer::phnplo`, `acepn.f90:2830-3573`): the viewr input text of a
photo-nuclear table, written to `ngend` by `phnfix_type1`. Pages in upstream's order:
principal, partial (`NTR > 1`), heating, particle heating (when non-zero), particle
production (photons /5), then per production type and reaction an angular page
(`LANDP > 0`, split at 20 MeV) and a 3-D emission page (laws 4, 44, 61).
- **Reused, not rewritten:** `aplots::fmt::{Plot, fe, fi, nint, pad70, hk_it}`,
  `aplots::scale::{ascle, mtname}`, and `tplots`' private `ascll` (now `pub(crate)`).
  `diff` of upstream's three copies: `acepn.f90`'s `ascle` is `acefc.f90`'s, and its `ascll`
  is `aceth.f90`'s, both lacking `acefc`'s "make sure amin and amax differ" step.
- **Quirks kept:** `mtname(mt,name,0)` with `name(2:2)='g'`; the partial page writes all ten
  characters of `name` and reads `xss(iaa+j-1)` with no `ESZ` offset; the angular page's
  `it` loop does not reset `it` and writes no title for photons; the 3-D page's
  `zmin = zmax/10000`, `go to 100` when `zmax = 0`, discrete lines `100*pd` bracketed at
  `ep ± 0.01`.
- Result: unit 33 identical on 78 (5 637 lines), 57 (59 125), 66 (33 524) and 58 (59 951).
  With it, the second check pass (36, 37, 38) and VIEWR (43, 46) are identical too.

**2. `acephn`'s MF=4/MF=5 neutron branches** (`acer::photonuclear::mf45`; `build.rs` only calls
it, in upstream's order; its MF=4 and MF=5 refusals are removed):
- pass 1 (`:348-366`): an MF=4 section counts one neutron reaction and feeds `tneut`;
- production (`:742-833`): MF=4 reactions first in MTRP, `TYRP = 1`, `y*sigma` into PXS,
  `e+q` heating, an MF=12-form yield record (MT=18: the MF=1 nubar, the last of 452/456 read);
- angular: `LANDP = 0` (isotropic, MF=4's own data never read, `:956-962`);
- DLW: ACE law 33 for MF=4 MT=50-90 (`:1166-1212`); MF=5 `LF = 1` → law 4, `LF = 7/9` →
  law 7/9 with `ν Ē σ` moved from the local to the neutron heating (`:1214-1425`), `Ē` through
  `mathm`'s `erfc` (`broadr::joint::erfc`, reused) called on `x`, not `sqrt(x)`, as upstream.
- **The leftover `mth`, upstream behaviour.** The multiplicity table is indexed by `mth`, the
  MT of the last record read. In the MF=4 branch that is pass 1's MEND, so `mth = 0` and every
  MF=4 reaction but MT=18 gets `y = 1`, an (n,2n) included (`MTH_AFTER_PASS_1`, unit test
  `multiplicity_table_and_the_leftover_mth`). In the MF=5 branch `findf`/`contio` have just
  read MF=5/MT=`mt`, so `mth = mt`. Deck 56 does not reach the MF=4 quirk: its only MF=4
  section is MT=18, whose `y` is the nubar.
- Result: deck 56's units 31 (36 530 lines), 32, 33-38 (32 812 / 36 530 / 1 each pass) and
  43/46 (39 522 / 39 521) identical.

**Gates:** `tests/deck_driver_photonuclear_vs_njoy2016.rs` now runs decks 56, 57, 58, 66 and
78 **whole**, as written, and requires every text tape NJOY2016.80 writes (all but the
unchanged input and MODER's blocked-binary unit 30) to have zero differing lines, and the run
to reach `stop`.

**Not done:** MF=6 `LAW = 4` and `ielas = 1` are still refused by name. ~~No upstream deck
reaches either: the five photo-nuclear decks are the only `iopt = 5` decks, and all five are
identical.~~ **CORRECTED 2026-11-13 (gh:#1369):** deck 64 (Ra-226 TENDL-2019) is a sixth
`iopt = 5` deck, and it reaches `LAW = 4`. That recoil heating is translated now and deck 64 is
identical (section 23). `LAW = 4` for an emitted light particle and `ielas = 1` are still
refused, and no upstream deck reaches them.

**Survey:** decks 56, 57, 58, 66 and 78 re-run alone (`SURVEY_CASES`) on develop 43eadde45 plus
this work: all five `identical`. **65 of 87 identical** (60 before).

**Tests** (release, on develop 43eadde45 plus this work, run after the last code or test
change; only Markdown changed afterwards):

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 124 passed, 0 failed, 2 ignored (43, 80), 1 849 s |
| `deck_driver_photonuclear_vs_njoy2016` (whole decks 56, 57, 58, 66, 78) | 5 of 5 passed |
| `deck_driver_covr_vs_njoy2016` | 11 of 11 passed |
| `acer_photonuclear_vs_njoy2016` | 5 of 5 passed |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `--lib` | 759 of 759 passed |

## 23. The remaining decks ranked; 61, 86, 21, 75, 10, 19, 60 and 64 closed (2026-11-13, gh:#1369)

**Re-run first.** The 22 rows that were not `identical` were re-run alone (`SURVEY_CASES`) on
develop `fe6a8230d`. Decks **31 and 87 were already `identical`**: their rows predated
recent merges. That made 67 of 87.

**The ranking.** It was posted on gh:#1369 before any new code. Each deck's first blocker
was found by running NJOY2016.80 and `njoy-deck` side by side and diffing tapes.
- PURR's writer for several T/σ0 and for HEATR input: 10, 19, 21.
- CCCCR: 02, 76.
- PLOTR: 06, 12.
- Photo-atomic heating residual: 48, 59.
- GAMINR + MATXSR: 03, 30.
- One deck each: 86 (GROUPR card 9a), 61 (`newsuff`), 75 (coded numbering, then GPD), 60
  (RECONR), 11 (WIMSR cards), 17 (ERRORR `iread = 2`), 43 (`thinb`), 64 (`acephn`) and
  80 (LEAPR).

A prediction was posted before each run, with one exception: **deck 61 ran before its
prediction** (recorded on the issue).

| deck | cause, read upstream | fix | result |
|---|---|---|---|
| 61 | ACER `iopt = 7` read only text written earlier in the run; `thrfix` refused `suff >= 0` | `run_acer_fix` reads `tape61` (`text_or_file`); `acer::newsuff` (`acecm.f90:619-684`) | 71, 72 identical; whole deck |
| 86 | card 9a (`mfd = -1`) was read, then dropped | down-converted as `groupr.f90:655-676`, processed in card order, C2 = `fzam` (`:871-878`): `Card9Item`, `card9a_reaction`, `AutoReaction::head_c2` | 25 identical (52 lines); whole deck |
| 21 | HEATR's coded tape counts 1 to its MEND unbroken (`hout` copies every closer, `heatr.f90:5961-6029`); PURR's breaks only at MT=152/153's SENDs and MF=2's FEND (`purr.f90:327-570`); PURR's writer refused HEATR input | `heatr_copied_closers`, `purr_copied_closers`; PURR's tables from `acer::deck_modules` (`purr_tables_after_heatr`) | 21-26 identical; whole deck |
| 75 | BROADR's second temperature goes on counting after its copied MEND; GPD's MF=3 comes from the **next** temperature block | `Tape::write`: same MAT after a copied MEND keeps the count (a new MAT restarts: RECONR, test 03); `gamsum_block` (below) | 21, 22, 26, 27 identical; whole deck |
| 10 | PURR at 3 T and 7 σ0 passed the tape through | `purr::tables::multi`: one `unrest` call per energy over every T and σ0 (`purr.f90:300-302`), MT=152 with every `sigu(ix,is,it)`, MT=153 divided by temperature 1's `sigu(·,1,1)` (`:516-523`); `purr_tape_multi`; ENDF-4 TEMP `N2` copied (`:343-349`) | first run: 3 lines (that `N2`); then 26-28 identical |
| 19 | as 10, with HEATR's partials on the input | `rdheat` per temperature (`:1237-1289`; `ihave` from the last temperature), the `ihave = 1/2` heating column (`:525-560`) | 27-29 identical, first run |
| 60 | IRDFF-II has **no MF=3**: MF=10 was evaluated on MT=1's (empty) energies; `recout`'s dummy MF=3/MT=1 (`L2 = 99`) and its dictionary, which drops the last real entry (`reconr.f90:5017-5026`, `:5091-5169`, `:5231-5255`); BROADR's `temp1` (`broadr.f90:334-412`, `:605-610`) | `reconr::union_grid_without_mf3`, `dummy_mt1_directory`, BROADR `temp1 != 0` | 31, 32 identical; the BROADR step was not foreseen |
| 64 | MF=6/MT=11's photon yield is zero everywhere; MF=6 `LAW = 4` recoil heating refused | `LAW = 4` heating `awp*(e+q)/(awr-awp)` on the first subsection's LISTs (`acepn.f90:507-584`); an all-zero yield gives no threshold (below) | 31-38, 43, 46 identical; the `LAW = 4` step was not foreseen |

**`gamsum`'s block (deck 75), established by measurement and then traced.**
- NJOY2016's 293.6 K GPD differed from ours in all 50 590 words, by about 1.3e-4.
- With BROADR at 293.6 K **only**, NJOY2016's ACE equals ours word for word (612 558 words).
- The mechanism: `unionx` leaves `npend` just past MF=3's FEND. Ag-109's blocks end at MF=3, so `gamsum`'s first `findf` (`acefc.f90:3652`) reads the MEND, skips it (`math = 0`, `endf.f90:1504`), meets the next block's MF=1 and searches forward (`:1516`).
- With a file after MF=3, or no further block, it searches backward into its own block.
- `deck::run::gamsum_block` codes exactly that. Both branches are checked:
  - deck 75 (forward);
  - deck 19, whose blocks carry MF=12/13 after MF=3 (backward). An unconditional "next block" broke deck 19's ACE, which is how the condition was found.

**Deliberate divergence on undefined behaviour (deck 64).** `acephn`'s scan for the first positive yield (`acepn.f90:417-424`) has no bound. On an all-zero yield it reads stale `scr` words. Its result only enters a `min()` of thresholds, so an all-zero yield is taken as giving none. The deck matches NJOY2016.80 on every unit. The divergence is commented at the site.

**Gates:**
- `tests/deck_driver_tapes_vs_njoy2016.rs`: `test_61_...`, `test_86_...`, `test_21_heatr_purr_coded_whole_deck`, `test_75_broadr_two_temperatures_gamsum_whole_deck`, `test_10_19_purr_several_temperatures_whole_decks`, `test_60_irdff_no_mf3_whole_deck`;
- `tests/deck_driver_photonuclear_vs_njoy2016.rs`: `test_64_photonuclear_whole_deck`;
- unit tests in `acer::newsuff`.

**Not done.** The order is the ranking above:
- ~~CCCCR (02, 76) and PLOTR (06, 12): large ports with nothing in `src/`.~~ **CORRECTED
  2026-11-13:** both translated and closed (§25, §26).
- ~~03 and 30: GAMINR's driver and MATXSR. Deck 03 first needs RECONR on DLC-7e's old photon format; deck 30 needs a genuine `ith = 0` on its photo-atomic MF=23.~~ **CORRECTED 2026-11-13 (§27):** closed. Deck 30's `ith = 0` was not genuine: the deck PENDF writer built its union grid without MF=23, so every MF=23 section was empty.
- ~~11: WIMSR's card reader. The engine is byte-identical on U-238.~~
- ~~17: ERRORR `iread = 2`.~~
- ~~43: BROADR `thinb`, gh:#619.~~
- ~~48 and 59: the photo-atomic `iheat` residual.~~
- 80: LEAPR, last figure.

**CORRECTED 2026-11-13:** 11, 17, 43, 48 and 59 are closed (§24). Deck 48/59's
"residual" was a port defect (two single-precision literals), not upstream's
convergence.

**Survey.** Rows 10, 19, 21, 30, 31, 60, 61, 64, 75, 86 and 87 were re-run alone
(`SURVEY_CASES`, 184 s) on develop a4a799b8d plus this work. **75 of 87 are identical** (65
before; 67 once the stale rows of 31 and 87 were re-run). Remaining: 02, 03, 06, 11, 12, 17,
30, 43, 48, 59, 76, 80.

**Tests** (release, on develop a4a799b8d plus this work). Run after the last code or test
change, with one exception: doc comments in `src/acer/photonuclear/{build,mod}.rs` and
`src/deck/run.rs`, and Markdown, changed while the deck suite was running. Every other suite
below was built after those edits.

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 130 passed, 0 failed, 2 ignored (43, 80), 1 642 s |
| `deck_driver_photonuclear_vs_njoy2016` (whole decks 56, 57, 58, 64, 66, 78) | 6 of 6 passed |
| `deck_driver_covr_vs_njoy2016` | 11 of 11 passed |
| `acer_photonuclear_vs_njoy2016` | 5 of 5 passed |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `--lib` | 761 of 761 passed |

## 24. Decks 11, 17, 43, 48 and 59 closed: WIMSR's cards, ERRORR `iread = 2`, BROADR `thinb`, `iheat`'s literals (2026-11-13, gh:#1369)

The smaller one-deck blockers of §23's ranking, in order. For each, the existing
code was searched first, upstream was read, and a prediction was posted on gh:#1369
**before** any new code ran. All five held on the first run. No folder touched has
a kovan `kind = "review"` block.

| deck | cause, read upstream | fix | result |
|---|---|---|---|
| 11 | WIMSR's engine was ported (`src/wimsr/`) but had no card reader, and read only a coded GENDF | `deck::wimsr::{read_wimsr, run_wimsr}` (`wimsr.f90:165-268`: presets, `igroup = 0` → `69 14 13 14`, `nfid = nint(rdfid)`, `nout = iabs(nout)`); `wimsr::gendf::read_material_records` and `run_gendf_records`, GROUPR's full-precision records for binary unit -26 | 27 (1 169 lines) and 28 identical; first comparison of `ntemp = 3`, `ires = 3` × 7 σ0, `ip1opt = 1`, `isof = 0`, the 69-group default |
| 17 | ERRORR `iread = 2` refused: card 10's cross-material pairs | card 10 read (`:907-919`); `ngchk`/`colaps` per material (`:11113-11135`, `:9097-9532`); `gridd`'s `iread = 2` subsection rule and the pairs after `nmt` (`:1202-1226`); `rdsig(mat1,…)` in `covcal` and `sigc` (`:2071`, `:7893`); `covout`'s `ix <= nmt`, `ixp <= nmt1` (`:7068`, `:7225`, `:7254`). New `errorr::mf31::run_cov_gendf_card10` beside `run_cov_gendf` | every tape identical: 41-43, 31-33, 91-93, 99 and ERRORR's 26 (6 420 lines) |
| 43 | BROADR at `tempef <= 0` only thins (`thinb`), not ported (gh:#619) | `broadr::joint::broadr_joint_thinb` (`broadr.f90:1218-1222`, `:1677-1760`), beside `broadr_joint`, which is unchanged | 35 identical (151 lines) |
| 48, 59 | the photo-atomic heating residual: `iheat` compares against `0.99999` and `-.9999`, **default-real** literals (NJOY2016 has no `-fdefault-real-8`); the port used doubles, so a panel loop could end one panel apart | `incoherent_heating::{UNEXT_LIMIT, UNOW_LIMIT}` = the `f32` literals promoted | 27 identical (17 964 lines), and 28, 40, 41 |

**Two places where upstream's behaviour is undefined (deck 43's `thinb`), refused by
name and not guessed.** Neither occurs on deck 43:
- `thinb`'s `tt` has no `save` (`:1691`). On every page after the first, it is read
  before it is written (`:1712-1717`). So a table longer than one page
  (`mpage = 1.5e7/(4(nreac+1))` points) thins from whatever that stack slot holds.
- A walk that ends below `thnmax` never writes its last point, and `bfile3` never
  sets `nj < 0`, so its paging does not end.

`istrap` with a thinned pass is refused too; upstream's bootstrap tape for it was not
read.

**The photo-atomic residual (decks 48, 59) was misattributed.**
`acer_photoatomic_vs_njoy2016.md` blamed the `pnow/2` panel rule's truncation above
~300 MeV. The truncation measurement stands. But truncation is common to two copies
of one rule, so it cannot separate them. The record's own conditioning test
(1.4e-16 per ulp) already pointed to a **branch** taken differently. Corrected in
place, with strike-through, in that record, AGENTS.md and the docs that quoted
"71 781 of 71 807 words".

**Gates:**
- `tests/deck_driver_tapes_vs_njoy2016.rs`:
  - `test_11_wimsr_whole_deck`;
  - `test_17_errorr_iread2_whole_deck` (behind `long-tests`; 235 s with NJOY2016);
  - `test_43_whole_deck` (no longer `#[ignore]`d);
  - `test_48_photoatomic_whole_deck` (now including the ACE, unit 27);
  - `test_59_photoatomic_whole_deck`;
- unit tests in `broadr::joint::thinb` (a hand-worked line, and the refusal of a walk below `thnmax`).

**Not done.** The large ports (CCCCR: 02, 76; PLOTR: 06, 12; GAMINR/MATXSR: 03, 30)
were out of scope this round. **UPDATE 2026-11-13 (§26):** PLOTR (06, 12) is closed. Deck 80 (LEAPR, the last printed figure of MF=7/MT=4)
was not attempted. **UPDATE 2026-11-13 (§25):** CCCCR (02, 76) and deck 80 are closed; ~~"the last printed figure"~~ was a mis-description (most values differed by more than 3e-5).

**Survey.** Rows 11, 17, 43, 48 and 59 were re-run alone (`SURVEY_CASES`, 230 s) on
develop 16330e9f6 plus this work. All five are `identical`. **80 of 87 are identical**
(75 before). Remaining: 02, 03, 06, 12, 30, 76, 80.

**Tests** (release, on develop 16330e9f6 plus this work, all run after the last code
or test change):

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 134 passed, 0 failed, 1 ignored (80), 1 886 s |
| `deck_driver_covr_vs_njoy2016` | 11 of 11 passed |
| `deck_driver_photonuclear_vs_njoy2016` | 6 of 6 passed |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `acer_photoatomic_vs_njoy2016` | 4 of 4 passed |
| `--lib` | 763 of 763 passed |

## 25. Decks 80, 02 and 76 closed: LEAPR's persistent `sb`, and CCCCR translated (2026-11-13, gh:#1369)

The last of §23's one-deck items (80) and the CCCCR port (02, 76). I searched the
existing code first and read upstream. For each, a prediction was posted on
gh:#1369 **before** any new code ran, and each held on the first run. No folder
I touched has a kovan `kind = "review"` block.

| deck | cause, read upstream | fix | result |
|---|---|---|---|
| 80 | ~~"17 369 lines differ in the last printed figure"~~ **CORRECTED:** 24 264 values differed, and 19 522 of them by more than 3e-5. In **every** one, NJOY's was the larger. `trans` allocates `sb(ndmax)` once per call and keeps it across the alpha and beta loops (`leapr.f90:883`). When `sbfill`'s `bet += delta` rounding stops it one entry short of `2*nbt-1`, the convolution reads (`:935-937`) the value an earlier `sbfill` left there. The port built a fresh array per call and padded it with zero | `leapr::translation::sbfill` writes one persistent buffer in place, and never-written entries read zero. Also `s=s*delta/3` is now `(s*delta)/3` (`:939`), where the port had `s*(delta/3)` | tape24 identical, 91 453 lines |
| 02 | CCCCR not ported (`ccccr.f90`, 3 526 lines) | `src/ccccr/`, a line-for-line translation: `core`, `state`, `isotxs`, `brkoxs`, `dlayxs`; `deck::ccccr` for the cards; `TapeUnits::put_bytes` | ISOTXS (26, 42 128 bytes) and BRKOXS (27, 23 164 bytes) byte-identical; 28 and 29 identical |
| 76 | the same | the same (DLAYXS from a coded GENDF) | DLAYXS (30, 3 732 bytes) byte-identical; 25 identical |

**Deck 80, the stale record.** The `#[ignore]` reason on `test_80_leapr_whole_deck`
said every difference was one unit in the sixth figure. That was never measured
value by value. Counting values shows otherwise: 4 742 differed in the last figure,
and 19 522 by 3e-5 or more (`6.87804e-17` against `6.87827e-17`), with a few tail
values `0` where NJOY has `5.7e-58`. The fact that every difference had one sign
is what pointed at a missing term. Corrected with strike-through in the test doc.
Decks 09, 22, 23 and 33 also run `trans`, and they stay identical.

**CCCCR: how byte identity was reached.**
- The port keeps upstream's equivalenced container as 100 000 four-byte words,
  indexed 1-based as upstream indexes it.
- It keeps the mixed precision. `a` is `real(4)`, so `a(locn)/a(locd)`,
  `a(locsg)/a(locsg0)`, `abs(a(noloc)-1.0)` and `a(mdex)-273.16` are single
  precision.
- It writes gfortran's 4-byte record markers.

Record: `src/ccccr/README.md`.

**Divergences and undefined behaviour, by name.**
- LEAPR `sbfill`: an `sb` entry that no earlier call wrote is **undefined** in
  Fortran. The port reads zero, which is what gfortran with glibc gives for a fresh
  8 MB `allocate`. A stale entry that an earlier call wrote is defined (an
  allocatable keeps its values), and the port reproduces it.
- CCCCR `prinxs`: with MT=455 before any MT=18, it indexes `dnorm(0)`, which is
  undefined. The port refuses this.
- CCCCR `ruinis` (`ichix = 1`): `spec` is read through `real(8)` and rounded,
  where upstream converts the decimal straight to `real(4)`. These can differ only
  at an `f32` midpoint. No deck reads `spec`.
- Refused by name, and run by no upstream deck:
  - ISOTXS `ifopt = 2`;
  - `nsblok = ngroup`;
  - a chi matrix (`ichix > 1`);
  - BRKOXS `nsblk = nreact`.

  The listing (`lprint = 1`) is not written.

**Survey.** The survey now counts a binary file of ours that differs from NJOY's
as `differs`. Before, it was listed as "binary" and could not fail.

**Gates** (`tests/deck_driver_tapes_vs_njoy2016.rs`):
- `test_80_leapr_whole_deck` (no longer `#[ignore]`d);
- `test_02_ccccr_whole_deck`;
- `test_76_ccccr_whole_deck`;
- `ccccr::core::tests`;
- `deck::tapes::tests::only_output_units_are_written_back` (raw bytes).

**Survey.** Rows 02, 76 and 80 were re-run alone (`SURVEY_CASES`, 537 s) on
develop 60675b0f0 plus this work, and all three are `identical`. **83 of 87 are
identical** (80 before). Remaining: ~~06 and 12 (PLOTR),~~ ~~03 and 30 (GAMINR, MATXSR)~~ (closed, §27).
**UPDATE 2026-11-13:** 06 and 12 are closed in §26.

**Tests** (release, on develop 60675b0f0 plus this work, all run after the last
code or test change):

| suite | result |
|---|---|
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` (`ulimit -v 12582912`) | 137 passed, 0 failed, 0 ignored, 2 036 s |
| `deck_driver_covr_vs_njoy2016` | 11 of 11 passed |
| `deck_driver_photonuclear_vs_njoy2016` | 6 of 6 passed |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `ace_five_route_parity` | 13 of 13 passed |
| `acer_photoatomic_vs_njoy2016` | 4 of 4 passed |
| the 14 `leapr_*` oracle test files | 48 passed, 0 failed, 1 ignored (`leapr_coher_general_self_consistency`, ignored before this work) |
| `--lib` | 764 of 764 passed |

## 26. Decks 06 and 12 closed: PLOTR translated (2026-11-13, gh:#1369)

PLOTR was a stub (`src/plotr/mod.rs`). It is now translated line for line from
`plotr.f90` (3 149 lines) into `src/plotr/`, and wired into the deck driver
(`deck::plotr`). Record of the translation, its reuse and its divergences:
`src/plotr/README.md`.

**What I checked first.** VIEWR (`src/viewr/`) was already translated, and on
NJOY2016.80's own plot files of decks 06 (tape31) and 12 (tape23) it gives
NJOY's PostScript byte for byte. So the decks only needed PLOTR's plot-command
file. I reused, rather than wrote: `endf::gety1::Gety1`, `endf::interp::terp1`,
`errorr::weight::terpa` (flat, stateful), `acer::fortran_fmt::fortran_e`,
`viewr::fmt`, `deck::cards::CardReader` and `deck::covr::tape_from_records`.
PLOTR walks its tapes card by card and positions them with `findf`, which reads
up as well as down. No line-level ENDF reader existed in the crate (the `Tape`
type is parsed sections), so `plotr::stream` holds a tape as its lines rebuilt
from `Tape` with SEND/FEND/MEND/TEND, and translates `contio` ... `findf` with
`npage` paging. No folder I touched has a kovan `kind = "review"` block (`src/plotr`,
`src/deck`, `src/endf`, `src/errorr`, `src/acer`, `src/viewr`, `src/lib.rs`'s folder).

**Prediction** (posted on gh:#1369 before the first run): both decks identical;
the named risk was a 1-ulp step in the walk's thinning, or a last figure of
`1p,2e16.8`.

| deck | first run | cause, read upstream | fix | result |
|---|---|---|---|---|
| 12 | identical | — | — | units 21, 22, 23 (5 278 lines), 24 (6 467) identical; 22 and 24 are upstream's `referenceTape22`/`referenceTape24` |
| 06 | stopped at curve 2 (MT=107, "illegal TAB1, nr<=0") | after the frame search (x axis automatic), `findf` searches up from the SEND. At its label 300 it backs up a card at a time until the card before the HEAD (`if (mth.gt.0) go to 300`, `endf.f90:1575-1577`). The port backed up once | `stream::EndfStream::found` loops as upstream | units 31 (17 344 lines) and 32 (22 422) identical; 32 is `referenceTape32` |

So the prediction held for 12 and not for 06. The miss was control flow (a missing
loop in a positioning routine), not arithmetic.

**Branches the two decks do not reach.** Upstream has no other PLOTR deck, so I
wrote five more and judged them against NJOY2016.80 run on the same deck. **They
were run before a prediction was posted for them**; that is said on gh:#1369.

| deck (in `tests/deck_driver_plotr_vs_njoy2016.rs`) | reaches | result |
|---|---|---|
| `DECK_GETY4_GETZ` (Fe-56 JEFF-3.3) | `gety4`; ratio plot from two units (`getz`) | plot 9 241 lines, PostScript 10 275: identical |
| `DECK_AUTO` (C-12 ENDF/B-V, `mtd = 0`) | automatic loop, passes, `rname`, `findf` up | plot 6 591, PostScript 28 075: identical |
| `DECK_MF7` (H in H2O) | MF=7 against alpha and beta | plot 153: identical. VIEWR not compared: this port's VIEWR panics on NJOY's own plot file of it, gh:#1543 |
| deck 01 + `DECK_GENDF_TAIL` | GENDF MF=3 (coded and binary unit), flux per lethargy, MF=6 spectrum, `gg3d` | plot 172, MODER copy 1 330: identical |
| `DECK_MF6` (Al-27 ENDF/B-VIII.0 MT=16) | `gety6` | **refused**: undefined upstream (below) |

**Undefined upstream, refused by name.** `gety6`'s `ylast` is not in its `save`
list (`plotr.f90:2803`), and the first interpolation of every MF=6 curve reads it
unset. NJOY2016.80 plots `8.245559e-8` there, a value of an earlier LIST, where
the panel's own value is `5.462661e-8`. The port stops at PLOTR with the reason.
The other refusals (reads of `lf`, `lep`, `l2`, `iauto`, `nr`, card 7, `sigig`
before anything set them; `rname` past its table, MT=229-236 among them;
`egrid(200)`/`ex3(200)` overflows; `ad3d`'s `intmu` on MF=6) are listed in
`src/plotr/README.md`.

**Not written:** PLOTR's listing (`nsyso`) and `mess` lines.

**Gates:** `tests/deck_driver_plotr_vs_njoy2016.rs` (7 tests); unit tests in
`plotr::{stream, cards, getters, rname}`.

**Survey.** Rows 06 and 12 were re-run alone (`SURVEY_CASES=06,12`) on develop
0ad98bc5d plus this work: both `identical` (06: 2 tapes, 12: 4). **85 of 87 are
identical** (83 before). Remaining: ~~03 and 30 (GAMINR, MATXSR)~~ none (§27).

**Tests** (release, `ulimit -v 12582912`, all run after the last code change):

| suite | result |
|---|---|
| `deck_driver_plotr_vs_njoy2016 --test-threads=2` | 7 of 7 passed, 3.2 s |
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` | 137 passed, 0 failed, 0 ignored, 2 134 s |
| `deck_driver_covr_vs_njoy2016` | 11 of 11 passed |
| `deck_driver_photonuclear_vs_njoy2016` | 6 of 6 passed |
| `deck_driver_survey` (`SURVEY_CASES=06,12`, `--ignored`) | 06 and 12 `identical` |
| `--lib` | 772 of 772 passed |

## 27. Decks 03 and 30 closed: photo-atomic RECONR, GAMINR's tape driver, DTFR from a deck, MATXSR translated (2026-11-13, gh:#1369)

The last two of 87. Both decks now run to `stop` and every coded unit NJOY2016.80
writes is identical line for line. Gate: `tests/deck_driver_photon_vs_njoy2016.rs`.

**Method.** Each deck is run whole by `NJOY2016_ROOT/build/njoy` and by `run_deck`
on the same tapes (03: `gam23` on 30, `gam27` on 32; 30: the H-1 neutron and
photo-atomic ENDF/B-VIII.0 tapes on 20 and 40). Every coded unit is compared as a
string, line for line, and the units upstream commits are compared byte for byte.
Pass: no differing line, no differing byte, the run reaches `stop`. A prediction
was posted on gh:#1369 before each first run (photo-atomic RECONR, GAMINR, DTFR,
MATXSR, and the two decks of our own).

| deck | unit | module | lines | result |
|---|---|---|---|---|
| 03 | 31 | RECONR (H, U; ENDF-4) | 2 180 | identical |
| 03 | 33 | GAMINR (LANL 12) | 604 | identical |
| 03 | 34 | DTFR tables | 340 | identical |
| 03 | 35 | MATXSR | 211 | identical |
| 03 | 36 | DTFR plot commands | 1 711 | identical |
| 03 | 37 | VIEWR | 9 274 | identical; `referenceTape37` byte for byte |
| 30 | 31 | GROUPR | 21 249 | identical (as before) |
| 30 | 43 | GAMINR (VITAMIN-J 42) | 1 477 | identical; `referenceTape43` byte for byte |
| 30 | 44 | MATXSR (`nscat`, `ng`, `gscat`) | 18 924 | identical; `referenceTape44` byte for byte |
| 30 | 45 | MODER of the photo-atomic PENDF | 4 408 | identical; `referenceTape45` byte for byte |
| X1 | 50, 51, 52 | DTFR photon table, `ifilm = 2`; VIEWR | 3 858, 35 730, 40 988 | identical |
| X2 | 43, 44 | GAMINR `igg = 1`, `iwt = 2`; MATXSR | 39, 20 | identical (MT=501 listed first, below) |

**What was missing, read upstream:**
- **Photo-atomic RECONR** (`deck/pendf_photon.rs`). The deck PENDF writer built
  its union grid from MF=3/10/12/13 only, so on a tape with only MF=23 the grid
  was empty and every section had no point. Deck 30's "MF=10-23 section with no
  positive point" was that, not a genuine `ith = 0`; deck 03's tape31 differed
  in all 2180 lines. Translated: `anlyzd`'s MT=501 (and MT=522 when 534-572 are
  present) as redundant reactions and no MF=2 (`reconr.f90:599-657`), `lunion`'s
  MF=23 skips and no threshold raise (`:1866-1914`), `emerge`'s MF=23 sums
  (`:4896-4915`) and `recout`'s order and heads (`:5222-5345`; the first
  redundant head keeps `scr(5:6)` of the first scratch head, the ENDF-4 TEMP
  record carries the entry count, because `scr(6)` is set again just before
  `hdatio`).
- **GAMINR's control flow** (`gaminr/tape.rs`): the ENDF-format probe from
  `nendf`, the gamout MF=1 head, the standard or listed reactions, the
  total-heating edit and the `akn` cache (`gaminr.f90:133-536`). The head's title
  word is `rt(1)` after `tt(1) = ''` through the `tt`/`rt` equivalence: four
  blanks and columns 5-8 of the PENDF's first card, printed `1.7514+190` (03) and
  `6.0135-154` (30). Two engine defects were fixed: `gtsig` used a local lookup
  instead of `gety1` (the threshold differs), and `EPAIR` was not upstream's
  expression.
- **DTFR from a deck** (`dtfr/deck.rs`, `dtfr/plots.rs`): `dtfr`, `ruin`,
  `dtfout`, `ploted`, `histod`, `dpend`, `plotnn`, `plotnp`. The plots had been
  called "permanently out of scope"; deck 03 compares them.
- **MATXSR** (`src/matxsr/`): every routine of `matxsr.f90`. The container is
  `real(k4)`, so every number on the file is an `f32`; `hname` hard-wires
  `iverf = 6`, so ENDF-4 MT=602/621 are `p02`/`p21`; a list that ends exactly at
  a `/` in a format writes an empty line (checked with gfortran 13).

**Refused by name (undefined or not ported):**
- RECONR: an MF=23 section with no positive point (`istart = 0` reads an undefined
  buffer word); photo-atomic and other materials in one run; MF=3/10/12/13 on a
  photo-atomic material.
- GAMINR: `ngam1 /= 0` (old-tape merge), `ngam2 = 0`, `iwt = 1`, a PENDF with no
  description card (`rt(1)` is then a stale scratch word), `mfd/mtd` outside
  MF=23 and MF=26 MT=502/504/516, a heating edit with `mfd /= 23`.
- MATXSR: a binary MATXS (`nmatx < 0`); `shufl` when one band does not fit
  (`imax` undefined); a vector MT=1/501 (or charged-particle MT=2) after another
  vector reaction.

**A prediction that failed, and a check that hid it.** X2 as first written listed
23/501 after 23/502. `vector` names the first section `hvps(1)`, then MT=501
writes `hvps(1:2)` unconditionally and moves `n1i` past a slot it never set, so
NJOY prints eight zero bytes as the third name (unit 44, line 11) and the 502 data
is overwritten. A first check with `diff` reported 0 differing lines: `diff` treats
a file with NUL bytes as binary and printed no `>` lines. The gate's string
comparison found it. The port now refuses that order
(`x2_mt501_after_another_reaction_is_refused`); with MT=501 first X2 is identical.
Every comparison in this section was then re-made by the gate's string comparison.

**Not exercised by any deck:** MATXSR's reference subtraction (`iref`: several
temperatures or sigma-zeros), its constant sub-blocks (`jconst`) and charged
particles; DTFR `iedit = 1` (CLAW, dated header), thermal corrections and the
fission branches through a deck; GAMINR with several materials of decreasing Z
other than 03's H then U.

**Survey.** Rows 03 and 30 were re-run alone (`SURVEY_CASES=03,30`) on develop
1a81b9422 plus this work: both `identical` (03: 6 tapes, 30: 4). **87 of 87 are
identical** (85 before). Remaining: none.

**Tests** (release, all run after the last code change):

| suite | result |
|---|---|
| `deck_driver_photon_vs_njoy2016` (new) | 5 of 5 passed, 6.7 s |
| `deck_driver_tapes_vs_njoy2016 --test-threads=2` | 137 passed, 0 failed, 0 ignored, 1 964 s |
| `deck_driver_covr_vs_njoy2016` | 11 of 11 passed, 221 s |
| `deck_driver_photonuclear_vs_njoy2016` | 6 of 6 passed |
| `deck_driver_plotr_vs_njoy2016` | 7 of 7 passed |
| `pendf_stages_vs_njoy2016` | 9 of 9 passed |
| `gaminr_vs_njoy2016` | 2 of 2 passed |
| `deck_driver_survey` (`SURVEY_CASES=03,30`, `--ignored`) | 03 and 30 `identical` |
| `--lib` | 774 of 774 passed |

## 28. GROUPR `getwtf` for `iwt = 1, 7, 10, 11, 12`, and `mfchk` (2026-11-13, gh:#1370)

No upstream deck uses these weights (the 87 decks use `iwt` 2-6 and 9), so the tape
driver refused them. Translated from `groupr.f90:5011-5025` (`genwtf`, card 8b) and
`:5186-5300` (`getwtf`):
- `groupr::genwtf`: card 8b's TAB1 (`genwtf_iwt1`), `temp(jtemp)`
  (`getwtf_set_temperature`, set per temperature from `jtemp = itemp`, `:497`), the
  tabulated branch with its saved `ip, ir, ipl, step`, and `iwt = 10`;
- `groupr::weights::getwtf_iwt67` and `getwtf_iwt1112`, beside `getwtf_iwt6` and
  `vitamin_e` (kept), because upstream writes `exp(two)` and `e**half` (a real
  power) where those take `e**2` and `sqrt`.

**Refused by name:** a card 8b that gives fewer words than its own `6 + 2 nr + 2 np`
(upstream reads unset words of an `allocate`d `tmp`; the deck reader marks unread
words `NaN`); `iwt = 0` in `getwtf` (no branch sets `wtf`).

**Gates** (`tests/deck_driver_groupr_weights_vs_njoy2016.rs`): decks of our own on
H-2 ENDF/B-VIII.0 (MODER, RECONR, BROADR to 293.6 K, GROUPR LANL-30 with `3/` and
`6 2/`). The prediction was posted on gh:#1370 before the first run: every line of
the GENDF identical. **Result: identical in all seven, 321 of 321 lines each,
compared as strings,** for `iwt = 1` (a two-region TAB1), 7, 8, 10, 11, 12, and
`iwt = -3` with one sigma-zero (upstream then takes `getwtf` with `|iwt|`,
`:6505-6512`). Control: NJOY's own GENDFs for these decks differ from one another
(3 090 bytes between `iwt = 7` and `8`, 59 between 11 and 12), so the gate can fail.

**`mfchk`** (`groupr.f90:1526-1573`): `deck::groupr_auto::mfchk` now runs on every
non-automatic card-9 request, as `:716` does. `16 mt` with `mt` in MF=13 becomes
`mfd = 17`, and `13 mt` with `mt` in MF=12 becomes 12. In a `-m` range the switched
`mfd` carries on to the range's later MTs, because upstream rewrites `mfd` in place.
`mfchk2` (`:1575-1597`, `mfd = 21-26`) is not translated, because the tape driver
refuses `mfd = 21-36`.
- The first deck, on B-10, stopped at `getgyl`'s refusal of several MF=13 photons
  (`NK = 16` and 9), the refusal its prediction named. It was replaced by `MF2`.
- `MF2`: Pu-240 ENDF/B-VIII.0, card 9 `16 3`, `16 102`. **The GENDF is identical,
  170 of 170 lines.**
- Ablation (`mfchk` made to return its input): 96 of 170 lines differ, so the deck
  does exercise `mfchk`.

## 29. ERRORR `iwt = 1`: card 13, read once per `egnwtf` call (2026-11-13, gh:#1371)

`egnwtf` reads card 13 (`iwt = 1`) or 13b (`iwt = 4`) itself, each time it is called
(`errorr.f90:9916-9962`). It is called once by `grpav` or by `colaps` (`:1028-1032`;
`colaps` calls it whenever `iwt /= 0`, `:9122`, and does not use what it read), and a
second time by `grpav4` for `mfcov = 34` (`:5467`).

`deck::errorr::read_errorr` refused `iwt = 1`, and it read card 13b once even for
`mfcov = 34`. It now reads either card once per call. `egnwtf_card13` keeps
`win(1..iww)`, `iww` being the last non-zero word, as upstream does, and the TAB1 goes
to `ErrorrWeight::Tabulated`.

**Refused by name:**
- two copies that differ (the port runs one weight for both passes);
- a card 13 whose trailing zero would leave `wght` shorter than its TAB1, because
  `terpa` would then read past it.

**Gates** (`tests/deck_driver_covr_vs_njoy2016.rs`; the predictions were posted on
gh:#1371 first):

| deck | what it is | result | control |
|---|---|---|---|
| `E1` | deck 05 with `iwt = 1` and a two-region TAB1 | 33, 34 and 35 identical (21 471, 78 796 and 199 652 lines) | NJOY's unit 33 differs from deck 05's in 210 162 bytes |
| `E2` | deck 46 with `iwt = 1` and card 13 twice | 34 identical (8 002 lines) | NJOY's unit 34 equals deck 46's, so this deck checks only the card count |

## 30. One run date for both sides (2026-11-14, gh:#1549)

`test_25_thermal_ace_and_directory` failed once, on a run that crossed midnight. The
cause:
- NJOY2016's `dater` (`util.f90:72-94`) stamps the clock's local date into each ACE
  header (`aceth.f90:669-670` and the other ACER classes) and into DTFR's edit title
  (`dtfr.f90:813`).
- The port did the same, from its own clock, after NJOY's run had finished.
- The line comparison includes the header line, so the two dates could differ.

**Fix, with the comparison unchanged:**
- `DeckOptions::run_date` (`mm/dd/yy`) fixes `dater` for a run, through
  `TapeUnits::set_run_date`, `dater` and `fixed_dater`. `None` keeps the clock at each
  call, as upstream.
- `tests/common::run_njoy_dated` runs NJOY between two reads of the local date. It runs
  NJOY again if the date changed during the run, and passes NJOY's date to this crate.
- Used by every harness that runs NJOY and then this crate: tapes, photonuclear,
  photon, plotr, covr and groupr_weights.

`faketime` is not installed here, and installing it was refused. So the reproduction is
an ablation, not a faked clock.
- `test_25_a_different_date_differs_in_the_header_only` dates this crate's run
  `01/01/99`.
- Predicted on gh:#1549 before the run: line 1 differs on units 70, 71 and 72, and unit
  75 (the directory, which has no date) is identical.
- **Measured: exactly that.** With the shared date, all four units are identical.

**Divergence, not changed:** the port's clock date (`deck::run::dater`) is UTC, while
upstream's is local time. Tests never use it now. A user outside UTC who runs a deck
without `run_date` gets the UTC date in the header between local midnight and UTC
midnight. Matching local time would need a time-zone dependency, which this crate does
not have.

## 32. GROUPR's flux calculator, `iwt < 0` with `nsigz > 1` (2026-11-14, gh:#1370)

The tape driver refused `iwt < 0` with several sigma-zeros. No upstream deck uses it.
`flux_table::genflx_card8a` now translates `genflx` (`groupr.f90:5309-5684`) for card 8a:
- `nflmax = 0`: the Bondarenko branch with card 8a's `sigpot`.
- `nflmax > 0`: the slowing-down flux.
  - The grid comes from `gety1` on MT=1 and `gety2` on MT=2, from
    `felo = max(egn(1), 0.1)` to `fehi`.
  - `alpha(1)` comes from MT=1's AWR.
  - The solve is `slowing_down::solve_flux`. These are steps 2-4 of the existing
    `genflx_slowing_down`, moved out unchanged so that both routes share them.
  - Then the weight shape below `felo`, the computed rows with their `nl` moments, and
    the narrow-resonance completion.
- `fehi` is the module variable that `genflx` rewrites, so it carries to the next
  temperature.
- `ninwt` is written as Fortran unformatted records. The narrow-resonance rows carry the
  last computed row's `xc`, as upstream writes them.

**Refused by name** (upstream reads values it never set):
- an unresolved range below the first narrow-resonance row: the computed rows' `xtot`
  word is never set;
- `ninwt` with `nflmax = 0`: `xc` is never set;
- `gety2` on an MT=2 that starts at zero.

**Gates** (`tests/deck_driver_groupr_flux_calculator_vs_njoy2016.rs`). All decks use
Pu-238 ENDF/B-IV `t404` and run RECONR, BROADR, UNRESR, then GROUPR (LANL-30,
`iwt = -3`, `lord = 1`, `sigz = 1e10 100 10`). The prediction was posted on gh:#1370
before the first run.

| deck | card 8a | unit 26 (GENDF) | unit 30 (`ninwt`) |
|---|---|---|---|
| `SD0` | `nflmax = 0`, `sigpot = 10` | identical, 411 lines | - |
| `SD1` | `fehi = 150`, one moderator | identical, 411 lines | - |
| `SD2` | three moderators, `beta`, `ninwt = 30`, `jsigz = 2`, 300 and 900 K | identical, 820 lines | identical, 233 440 bytes |
| `SD3` | `SD1` with `ninwt = 30`, `jsigz = 3` | identical, 411 lines | identical, 122 064 bytes |

**The first run of `SD2` was not identical.** Unit 26 matched, but 19 of the 7 296
`ninwt` records differed in the last bit.
- Localised with an instrumented NJOY2016 build: a scratch copy of `groupr.f90` that
  dumps `b` before and after the solve, built with the same `-O3`.
- The cause: step 3's third-moderator term. Upstream writes it without parentheses,
  `f1*beta*(1-gamma)*(sigz(iz)-sam)`, which evaluates as
  `((f1*beta)*(1-gamma))*(sigz-sam)`. The library computed `f1*s_k3`.
- Fixed in `solve_flux`, so `genflx_slowing_down` changes in the last bit too.
  `groupr_u238_gendf_golden` (tier 5, `nalph = 3`) still passes.

**Ablations:**
- `solve_flux` given a zero elastic cross section: 63 of 411 lines of `SD1` differ, and
  `SD2` differs in 126 lines and in its `ninwt` file.
- `SD0` with `sigpot` set to 0: 213 of 411 lines differ.

## 31. RECONR's ENDF-4 TEMP record carries the entry count (2026-11-14, gh:#1546)

For an ENDF-4 evaluation (`iverf = 4`, second MF=1/MT=451 record `N1 ≠ 0`,
`reconr.f90:226-230`), `recout` writes the dictionary's entry count in the head record
(`:5036`). It writes it **again** in the TEMP record: `scr(6)=nxcc+nmtr+i152` is set just
before `hdatio` (`:5092-5094`) for every format. `deck::pendf::with_directory` wrote 0
there.

No upstream deck compares RECONR's PENDF of an ENDF-4 tape. Decks 02, 10 and 11 run
RECONR on `t404` (Pu-238, ENDF/B-IV) and hand the tape straight to BROADR.

**Gate** `test_r4_reconr_endf4_pendf_is_identical`: deck 02 cut before `broadr`, plus
`moder -22 28`. The prediction was posted on gh:#1546 before the run.
- Before the fix: 1 line differed, line 3 (the TEMP record), as predicted.
- After the fix: 0 lines differ.
- Decks 02, 10 and 11 still pass.

### 32a. `iwt = 0`: `getfwt` reads the flux back (2026-11-14, gh:#1370)

- **`flux_table::FwtReader` is `getfwt`** (`groupr.f90:5686-5775`). It keeps the
  `save`d bracket and the file position from one temperature to the next. The
  exact-match branch reads one record ahead and returns `enext = emax`, as upstream
  does.
- **`flux_table::genflx_fwt`** is the Bondarenko branch with `getfwt`'s weight and
  `fac = sigz*xc/(sigz*xc+tt)`.
- **Refused by name:** a read past a short record or past the end of the file (a
  Fortran runtime stop upstream).
- **Divergence:** upstream keeps `getfwt`'s `templ` across GROUPR runs; here it is kept
  per run.

**Deck `FW1`** (prediction posted on gh:#1370 first): `SD2`'s chain writes
`ninwt = 30`. A second GROUPR with `iwt = 0` and card 8d `-30/` then reads it back at
the same two temperatures.
- **Result:** units 26 and 27 are identical (820 lines each) and unit 30 is identical
  byte for byte.
- **Control:** NJOY's unit 27 differs from its unit 26 in 479 lines.
- **Ablation:** with `xc` left out of `fac`, 478 lines of unit 27 differ.
- **Defect found on the way:** the first run never finished. The caller read
  `getfwt`'s `(w, xc, enext)` in the wrong order, so `enext` was `xc` and the energy
  stalled. The 12 GB address-space cap stopped the run. Fixed before any comparison.

## 33. ERRORR `mfcov = 40` (2026-11-14, gh:#1371)

**Upstream, read first.** `gridd` (`errorr.f90:1184-1191`) and `covcal` (`:1889-1907`)
read an MF=40 section's HEAD (`NS` in N1) and its **first** final state's CONT
`[QM, QI, L1, LFS, 0, NL]`, set `izap = 10*L1 + LFS`, and then read one sub-subsection
per state. Neither reads a second state's CONT. With `NS = 2` (TENDL-2023 Ar-37) NJOY2016.80
stops in `covcal` with "mfcov mt found not equal to input mt". Every MF=40 evaluation held
here or in NJOY2016's `tests/resources` (Ar-37, Fe IRDFF-II, Hf-177, Pb-208, Pt-184 TENDL)
has `NS > 1` in most sections. `rdsig` ignores its `izap` argument (`:2755-2786`), so the
cross section is MF=3's for the MT, not MF=10's. The output differs from MF=33's in the
MF=1 HEAD (`-14`, `sigc` `:7824`), MF=3's word 2 (`mzap`), and each MF=40 HEAD, which
carries `izap` in place of AWR (`covout` `:7219-7241`).

**Code.** `gridd::mf40_final_state` (shared by `gridd` and `covcal`); `scan_reactions_mf`
counts MF=40; `mf31::run_cov_gendf` accepts 40 (`za`, `awr` from the last MF=40 HEAD);
`ErrorrResult::to_tape` writes the MF=40 form; `deck::errorr` routes `mfcov = 40` there.

**Refused by name:** `NS /= 1`, `NL /= 1`, and `iread = 2` with `mfcov = 40`.

**Gate** (`tests/deck_driver_errorr_mf40_nstan_vs_njoy2016.rs`, deck `M40`, prediction
posted on gh:#1371 first): Ar-37 TENDL-2023 edited in the test to one final state per MF=40
section (`NS = 1`, the `LFS = 1` state's 4 lines removed); MODER, RECONR, GROUPR (6 groups
to 30 MeV, `3 1/ 3 11/ 3 33/ 10/`, binary GENDF), MODER of the GENDF to text, ERRORR
`mfcov = 40`.
- First run: 2 of unit 24's 32 lines differed (MF=3 coarse cross sections). The cause
  was older than MF=40: GROUPR's `10/` writes each production as another MF=3/`mt`
  section with `C2 = izap`; `rdgout` takes the first (`:2889-2891`), and
  `mf31::union_group_xs` kept the last. Fixed.
- After the fix: units 25 (GENDF, 492 lines) and 24 (32 lines) identical.
- Ablation: AWR in the HEADs and `-11` in MF=1 make 3 of the 32 lines differ.
- The unedited tape is refused by name at `mf40_final_state`.

## 34. ERRORR `nstan`: `grist` and `stand` (2026-11-14, gh:#1371)

**Upstream.** An NC-type sub-subsection with `LTY` 1-3 names a standard `(MATS, MTS)`.
`gridd` calls `grist` (`errorr.f90:1559-1679`) for `LTY = 1` (energies inside `(E1, E2)`)
and `LTY = 2` (all), not for 3. With `nas = 0` or `matb(1) >= 0`, `grist` reads only the
standard's first subsection. `covcal` calls `stand` (`:2939-3009`), which stages the
standard's first-subsection NI records ahead of the reaction's own, each with the window
`(sigfig(E1), sigfig(E2))` and its `LTY`; the window tests are `:2101-2118`. Card 11
redefines a standard; the first match wins.

**Code.** `errorr::standards` (`Standards`, `grist`, `stand`, `StandardWindow`),
`CovarianceReactions::standards`, `mf31::run_cov_gendf_nstan`, and card 11 in
`deck::errorr::read_errorr`.

**Refused by name:** a negative `matb(1)` (the third-reaction mode: `kmtb`, the `ety`
window, `grist`'s later subsections); `nstan` on the PENDF route (`mfcov = 33`,
`ngout = 0`); `nstan` with `iread = 2`.

**Gates** (`tests/deck_driver_errorr_mf40_nstan_vs_njoy2016.rs`; prediction posted on
gh:#1371 first). JENDL-3.3 U-235 (`J33U235`): MF=33 MT 102 refers to `(9228, 18)` with
`LTY = 1` and `LTY = 2`. RECONR 0.005, BROADR 300 K, GROUPR LANL-30 `iwt = 6`, then ERRORR
`mfcov = 33` from the GENDF with `J33U235` again on `nstan = 24`.

| deck | card 11 | unit 27 |
|---|---|---|
| `N1` | `0/` | identical, 8 654 lines, first run |
| `N2` | `9228 18 9228 102/ 0/` | identical, 8 588 lines, first run |

- Control: NJOY's `N1` and `N2` differ in 177 lines.
- Ablation: dropping `stand`'s records, 5 981 of `N1`'s lines differ.
- **Not exercised by these decks:** the windows. Ignoring them changes nothing, because
  the standard's covariance is zero in its only bin outside the `LTY = 1` window.
  `StandardWindow::admits` is pinned by a unit test only.

## 35. GROUPR `getmf6` LAW = 2 neutrons (2026-11-14, gh:#1370)

`getmf6` sends LAW 2-5 to `getdis` (`groupr.f90:8096-8099`), which calls `getfle` with
`mft = 8`. For LAW = 2, `getfle` reads the subsection's TAB2 with `ltt = 1` and one LIST
`[0, E, LANG, 0, NW, NL]` per incident energy (`:9733-9760`), each through `getco`'s
Legendre branch, as MF=4 `LTT = 1`. Everything after that is the `getdis` path LAW = 3
already used (section 14).

**Code (reuse):** `groupr::file4::File4Angular::from_mf6_law2` reads the `ZAP = 1`
subsection with the existing `read_records`/`getco_legendre`; `deck::groupr::write_getmf6`
hands it to `TwoBodyFeed`.

**Refused by name:** `LANG /= 0`; `LCT /= 2` (upstream rewrites `LCT = 1` to CM for MT
51-90 with `AWR >= 10` and reads `LCT = 3` per particle); ~~LAW 4 and 5 for a neutron.~~
**CORRECTED 2026-11-14:** LAW 4 and LAW 5 (`LTP = 2`) for a neutron are written now (§35a).

**Gate** (`tests/deck_driver_groupr_mf6_law2_vs_njoy2016.rs`, deck `L2`, prediction on
gh:#1370 first): Si-30 ENDF/B-VIII.0, MF=6 MT 51-56 (`LAW = 2`, `LANG = 0`, `LCT = 2`);
RECONR, BROADR 293.6 K, GROUPR LANL-30 `iwt = 3`, `lord = 3`, `8 51/` to `8 56/`.
- Unit 25 identical, 332 of 332 lines, first run.
- Ablation: with CM isotropy in place of the records, 181 of 332 lines differ.

**Not done:** `getdis`'s Coulomb term (`:9529-9548`). It runs only for `mtd = 2` with
`izap > 1` (an incident charged particle's elastic, File-6 `LAW = 5`), together with
`lidp`, `wcut`, `spi` and `mft = 8` for `mfd = 3`. That is the incident charged-particle
GROUPR path; the tape driver carries no `izap`/`izat`/`aprime` for it (it sets `izap = 1`
for every MF=6 feed), so the term is not self-contained and was not started.

### 35a. GROUPR `getmf6` LAW = 4 and LAW = 5 neutrons, on a synthetic tape (2026-11-14, gh:#1370)

Maintainer decision (2026-11-14): no tape we hold has a neutron `LAW = 4` or `LAW = 5`
subsection, so verify on a dummy input fed to both codes.

**Upstream behaviour (read, then confirmed by running):**
- **LAW 4** (`groupr.f90:7636-7653`): `getmf6` backs up to the *first* subsection of the
  section, re-reads its TAB1 with the LAW-4 particle's `zap`/`awp`, and lets `getfle` read
  that subsection's records. A first subsection with `LAW = 3` gives `law = -4`, so
  `getfle` sets `iso = 1`. At normal entry `law` is 4 again, so `getdis` negates every
  quadrature cosine (`:9519`) before both the angular probability and the lab cosine.
- **LAW 5, neutron, `LTP = 2`**: `getco`'s `lang = 2` branch copies `a_0..a_NL` as given
  (`a_0` is not set to 1, `:10563-10573`). `SPI` and `LIDP` reach only the Coulomb term
  and `wcut`, which need `izap > 1` (`:9449`, `:9529`), so a neutron ignores them.
- **LAW 5, neutron, `LTP > 2`**: GROUPR's `conver` (`:11572-11624`) rewrites each such
  record on its scratch tape as `LTP = 2`, after multiplying `p(mu)` by `sig(E)(1-mu)`.
  The `getsig` it calls reads the PENDF and leaves the ENDF header globals (`l1h`, `n2h`)
  at that tape's last record, and `nm = n2h`, `getco`'s `lang = l1h` and `nmu = n2h` are
  then read from them. Found by the first run (below) and an instrumented NJOY2016.80
  (`write(77,…)` in `getfle` and `getco`, built in a scratch directory from the vendored
  objects): on MT 54 (`LTP = 12`) and MT 55 (`LTP = 14`), the first two converted records
  of each section came back `NW = 1`, `a_0 = 0`, and `getco`'s tabulated branch ran only
  for each section's first record. **Refused by name** (`File4Angular::from_mf6_law5`).
- **LAW 5, neutron, `LTP = 1`**: `getco` label 360 goes to 370 for `izap <= 1` with `y`
  never set (uninitialised). **Refused by name.**

**Code (reuse):** `File4Angular::from_mf6_law4` (the first subsection's records through the
existing `read_records`, or `isotropic_cm`), `File4Angular::from_mf6_law5`
(`getco_law5_legendre`), `TwoBodyFeed::with_law4_cosine_flip`; `deck::groupr::write_getmf6`
dispatches LAW 2-5. Also refused by name: a LAW-4 section whose first subsection is not
`LAW = 2/3`, a LAW-4 particle with `AWP /= 1` (the port's `getdis` holds `aprime = 1`),
`LCT /= 2`, `LANG /= 0`.

**Gate** (`tests/deck_driver_groupr_mf6_law45_vs_njoy2016.rs`, deck `L45`, prediction on
gh:#1370 first). The test builds the tape from Si-30 ENDF/B-VIII.0 (MAT 1431), rewriting
MF=6 MT 51-55 and the MF=1 directory counts: MT 51 Si-30 `LAW = 2` first and neutron
`LAW = 4`; MT 52 Si-30 `LAW = 3` first and neutron `LAW = 4`; MT 53 `LAW = 5 LTP = 2`
(`a_0 = 0.5, 0.45, 0.4`); MT 54 `LAW = 5 LTP = 2` with `SPI = 2.5`, `LIDP = 1`; MT 55
`LAW = 5 LTP = 14`; MT 56 untouched (`LAW = 2` control). RECONR 0.005, BROADR 293.6 K,
GROUPR LANL-30 `iwt = 3`, `lord = 3`, `3 1/ 3 51/ 8 51/ … 8 54/ 8 56/`.
- **First run** (MT 54 then `LTP = 12`, deck with `8 55/`): 114 of 326 lines differed, all
  in MT 54 and 55 (36 and 35 lines); MT 51, 52, 53, 56 identical. That is the `conver`
  finding above; the prediction ("identical") missed on `LTP > 2`.
- **Second run**: unit 25 identical, **295 of 295 lines**.
- **Ablations** (one run, disjoint MTs): no LAW-4 cosine flip changes MT 51 in 34 of 44
  lines and MT 52 in 32 of 41; `a_0` forced to 1 changes MT 53 in 30 of 37 and MT 54 in 29
  of 37; MT 56 unchanged (0 of 37).
- A deck asking for `8 55/` stops at GROUPR with the `LTP > 2` refusal
  (`l45_deck_with_ltp14_is_refused_by_name`).

### 35b. GROUPR with several materials (card 10) and an input GENDF (`ngout1`) (2026-11-14, gh:#1370)

No upstream deck uses either. Translated from `groupr.f90:433-470` (label 120), `:480-488`
(label 170), `:549-604` (the head), `:405-432` (label 410), `:590-630` (labels 590-620) and
`:990-1006` (card 10, label 650):
- **Card 10** is read after the material's `ntemp` card-9 lists, and each further material
  reads its own `ntemp` lists. ~~`read_groupr` read card 10 "until 0" straight after the
  first material~~ **CORRECTED 2026-11-14**: that read a second material's card 9 as card 10.
  `iverf`/`emaxx` stay those of `nendf`'s first material; `lrflag`, `dntc`, `fehi`, the
  `ninwt` file and `getfwt`'s position carry on as module state.
- **`ngout1`** (`deck/groupr/gout.rs`): a read position in lines (coded) or records
  (binary). `tpidio` copies the label. Label 120 copies earlier materials, skips one
  MEND-block of `matb`, and backs up at a later MAT. `totend` copies the rest. With
  `iaddmt = 1` (card-10 `-matb`), the head is copied (`contio` + `tofend`), label 410 copies
  old sections up to the one replaced, and label 600 copies the remainder.
- **Sequence numbers.** A coded copy (`tomend` and the rest) writes the running `nsh` and
  never resets it on the END lines it copies, so a new section after a copy continues the
  count. `conver` sets `nsh = 0` (`:10965`), and the `atend` after it leaves 1 for a coded
  `nendf`. Only a copied head shows this; `write_head` sets `nsh = 1` (`:554`).
- **Refused by name:**
  - a negative first `matb`. With `ngout1 /= 0`, `ruinb` keeps the sign and `findf` stops.
    With `ngout1 = 0`, label 170 reads `nendf`'s label as a head, and NJOY2016 loops for
    good (run: killed after 2 minutes at 100 % CPU);
  - a card-10 `-matb` with `ngout1 = 0`, for the same reason;
  - `ngout1`/`ngout2` of different modes (the copy routines' "mode conversion");
  - `iaddmt = 1` with the flux calculator, whose `genflx` moves `nsh` (`:5423`);
  - a binary `ngout1` that no module of the run wrote.
- **A run off the end of `ngout1`** is an error, as NJOY2016's Fortran runtime "End of
  file" is. NJOY2016 does this whenever a card-10 `-matb` is itself on `ngout1`: label 120
  skips its block, and the head copy reads the TEND (run).

**Found on the way (pre-existing, single material):** `getdis` with `mft = 4` (MF=4
two-body, the MT 251-253 ratios) used the PENDF MF=3 head's AWR. Upstream `getfle` sets
`awr` from the MF=4 head (`:9713`). Be-9 ENDF/B-VIII.0 has 8.934780 against 8.934760, and is
the only tape held here, in `tests/resources` or `reference-data/endf`, where the two differ
(checked by script over all of them). So no gated deck moves.
- Be-9 alone at 0 K: MF=6 MT=2 differed in 39 of 395 lines; with the fix, 6.
- The 6 remaining lines are in the 7th figure, at the energies where Be-9's Legendre
  order changes. They are **not resolved**; `be9_alone_control` is `#[ignore]`d with that
  reason.

**Gates** (`tests/deck_driver_groupr_materials_vs_njoy2016.rs`; predictions posted on
gh:#1370 before each run). The tapes are ENDF/B-VIII.0 H-2 (128) + Li-6 (325), joined in
the test, with C-12 (625) added for `NG3`. RECONR 0.005, then GROUPR at 0 K: LANL-30,
`iwt = 3`, `lord = 1`.

| deck | what | units | result |
|---|---|---|---|
| `MM1` | 128 and card-10 325, `3/ 6 2/` each | 24 | identical, 1062 lines |
| `NG` | `MM1`, then replace 325 (unit 25), then replace 128 (unit 26) from `ngout1 = 24` | 24, 25, 26 | identical, 1062 / 455 / 877 |
| `NGB` | `NG`'s first replacement, blocked binary, MODER to text | 34, 35 | identical, 1062 / 455 |
| `NG3` | `iaddmt = 1` for 325, not on `ngout1` (128, 625): 625's head and sections are copied around 325's | 24, 25 | identical, 716 / 470 |

- The first run missed in two ways: the Be-9 AWR above (Be-9 was then the second
  material), and `NG3`'s `nsh` after `conver`. Then an END writer that did not leave
  `nsh = 1` for a following text copy made `NG` differ in its TEND line and in 325's copied
  block. All three are fixed.
- **Ablations:**
  - label 410 skipping instead of copying: `NG3` unit 25 differs in 387 of 470 lines;
  - the text copy resetting `nsh` on copied END lines: `NG` unit 25 differs in 310 lines,
    unit 26 in 733, and `NG3` unit 25 in 369. `NGB` (binary) is unchanged, as it must be.

### 35c. GROUPR `mfd = 21-26` from MF=6; `mfchk2` and the Coulomb term deferred (2026-11-14, gh:#1370)

`getff` sends `mfd = 21-26` to label 800, which is `getmf6` on `egn` as for `mfd = 8`
(`groupr.f90:7389`). `getmf6` picks the subsection whose `ZAP` is 1001, 1002, 1003, 2003,
2004, or the residual for 26 (`:7584-7592`; `Mf6Feed::new` already carried this). The
section is written as `MF = mfd` (`:820-826`). The deck driver now routes those requests
there. A charged subsection with LAW 2-5 is refused by `Mf6Feed`, because the port's
`getdis` holds `aprime = 1`.

**Gate** (`pb_groupr_charged_particle_production_from_mf6`; prediction on gh:#1370 first).
F-19 ENDF/B-VIII.0 (MAT 925), RECONR 0.005, GROUPR at 0 K, LANL-30, `iwt = 3`, `lord = 1`,
with `3 103/ 21 103/ 21 28/ 25 22/ 25 107/`.
- Unit 24 is identical, 235 lines, on the first run.
- NJOY's GENDF holds MF=21 MT 28 (36 lines) and MT 103 (44), and MF=25 MT 22 (70) and
  MT 107 (53).
- Ablation: with `mfd = 21` selecting the neutron, the run stops in `getmf6` ("desired
  particle not found"). F-19's MT 103 has only a proton subsection.

**Deferred, and why:**
- **`mfchk2`** switches `mfd = 21-26` to 31-36 for an MT in `conver`'s `mf4r` list, i.e. an
  MT whose charged particle is given by MF=4 two-body data. That needs `getdis` with
  `aprime /= 1` and the `mf4r` lists (`conver`, `:11000-11150`). Neither is translated, so
  an `mfd = 21-26` request for an MT with no MF=6 section is refused by name instead of
  reaching `getmf6`. `mfd = 31-36` stay refused.
- **`getdis`'s Coulomb term** (`:9529-9548`) runs for an incident charged particle only
  (`izap > 1`, `mtd = 2`, `mft = 8` with `mfd = 3`). The tape driver fixes `izap = 1`,
  `aprime = 1` and the neutron `izat`, so the incident-charged sublibrary (#1511) has to
  come first.
- The automatic lists `21/`-`26/` (`nextr`'s charged lists) stay refused.

## 36. THERMR's second SCT atom: `gatef2` wired (2026-11-14, gh:#1375)

**Upstream.** For an ENDF-4/5 tape THERMR keys a mixed moderator's second
short-collision-time atom on `matde` alone (`thermr.f90:218-238`): 1099 (BeO) gets
`sz2 = 3.76`, `az2 = 15.86`, 1095 (benzene) `4.74`, `11.90`, and
`sb2 = sz2*((az2+1)/az2)**2`. `gatef2` gives `teff2` (`:422`), and `sig` label 170
adds `sigc*sb2*s2` with `a*az/az2` (`:2588-2594`).

**Code.** `IncoherentInelastic::secondary_sct` (`None` from every parser), the
label-170 term in `sct_double_differential`, and `deck::thermr::endf45_second_sct_atom`.
`gatef2` was present (`thermr::effective_temperature`) but called by nothing.

**Refused by name:** an ENDF-6 second SCT atom (`B(7) = 0` with `sb2 /= 0`; its
`T_eff2` TAB1, `:1835-1854`, is not parsed), and a third SCT atom (upstream's "only 2
sct atoms allowed"). No tsl tape held here has either.

**Gate** (`tests/deck_driver_thermr_gatef2_vs_njoy2016.rs`, deck `G2`, prediction on
gh:#1375 first). No BeO or benzene ENDF-4/5 tape is held anywhere, so `G2` is deck 01
up to GROUPR with `t322` relabelled from MAT 1065 to 1099 in the test and the second
THERMR's `matde = 1099`: a code-to-code device with no physical meaning.
- Unit 31 identical, 32 897 lines, first run.
- Control: NJOY's `G2` unit 31 differs from deck 01's in 11 222 lines.
- Ablations: without the second term, 2 063 lines differ; with `teff2 = T` in place
  of `gatef2`, 2 092.

## 37. ERRORR `iread = 1`: cards 8-9 (2026-11-14, gh:#1371)

**Upstream.** Cards 8-9 (`errorr.f90:862-905`) give `nmt nek`, the `nmt` MTs, `nek+1`
bounds (`sigfig` to 6 figures) and `nek*nmt` lines of `akxy`. `gridd` skips every
section not on card 8a and keeps the card-8a positions (`:1154-1157`); a subsection
with `mat1 > 0` or an `mt1` not on card 8a has `iok = 0` (`:1220-1225`); an `LTY = 0`
NC sub-subsection still merges its two energies into `eni` but its formula is dropped
(`:1299`); at the end of the file `gridd` returns before rebuilding `akxy`
(`:1166-1169`). `covcal` skips the same sections (`:1913-1918`).

**Code.** `errorr::gridd::Card8` and `CovarianceReactions::card8`, those branches in
`gridd` and `covcal`, `run_mf33`'s `za`/`awr` from the first card-8a section, and
`deck::errorr::read_card8`.

**Refused by name:** `iread = 1` on any route but `mfcov = 33` from a PENDF; with
MF=32 on the tape (no deck exercises `resprx` with a card-8a list); `nmt <= 0` or
`nek < 0`; a card 8a, 8b or 9 shorter than its count (upstream reads unset words).

**Gates** (`tests/deck_driver_covr_vs_njoy2016.rs`, prediction on gh:#1371 first), deck
05 (`t511` carbon) with card 7 `1 33/`:

| deck | cards 8-9 | 33 (ERRORR) | 34 (COVR) | 35 (VIEWR) |
|---|---|---|---|---|
| `I1` | `2 0/ 2 102/` | identical, 554 lines | identical, 2 223 | identical, 6 234 |
| `I2` | `3 1/ 1 2 102/ 1e-5 2e7/ 0 1 1/ 0 1 0/ 0 0 1/` | identical, 5 148 | identical, 11 001 | identical, 21 393 |

- Both identical on the first run (once the test's own deck edit was fixed: its first
  version also rewrote card 1's ` 0 33/`, and NJOY2016 stopped).
- Control: NJOY's unit 33 for deck 05 has 21 471 lines.
- Ablation: `I2` with card 9 replaced by the identity makes 5 078 of unit 33's 5 148
  lines differ.

**`iverf = 4` (cards 4-6) is not done.** No ENDF/B-IV evaluation with MF=33 is held
here or in NJOY2016's `tests/resources`.

**MF=32 sub-cases, surveyed** (the first range of every MF=32 in
`reference-data/endf/` and NJOY2016's `tests/resources`): every one has `NIS = 1`,
`NRO = 0`, and `LCOMP` 1 or 2 with `LRF` 2, 3 or 7, and every `LCOMP = 1` range has
`NLRS = 0`. So no held evaluation reaches:
- `LCOMP = 0` (`rpxlc0`, with `ssslbw`/`ssmlbw`);
- `NRO /= 0` (upstream stops, `:3120-3124`);
- `NLRS > 0` (upstream stops in `rpxlc12`, "nlrs>0 not coded", `:4602-4620`);
- `NIS > 1` with `LRF = 7`;
- `KBK > 0` with MF=32 (only Sr-88 ENDF/B-VIII.1 has `KBK = 1`, and it has no MF=32);
- `irespr = 0`: `resprp` stops on any `LCOMP /= 0` (`:8111-8117`), so it is reachable
  only through `LCOMP = 0`.

Ranges after the first were not surveyed (JENDL-3.3 U-238 has 11, Pu-239 4).

## 38. ACER's unnamed routines, `chekit`, and RECONR `LRF = 6` (2026-11-14, gh:#1372, #1375)

None of `chekit`, `tabize`, `cptab`, `fndar1`, `fndar2` or `terpc` appears in `src/` by
name. By logic, and by what the held evaluations reach (the surveys cover
`reference-data/endf/` and NJOY2016's `tests/resources`):
- **`tabize`** (`acefc.f90:813-1059`): present inline. It copies an `LNU = 1`
  polynomial unchanged (`:848-856`; deck 10, `t404`, is identical whole). Its
  linearization of a tabulated nubar with `INT > 2` is reached by nothing: every
  MF=1/MT=452/455/456 held is `INT = 2`.
- **`cptab`** (`:2868-2975`): a copy. Its one call (`:2438`) runs with `mfh = 6`, where
  it sets `nr = 1, int = 2`, so its linearization branch is dead upstream.
- **`terpc`** (`:15181-15213`): feeds only `consis`' listing (`nsyso`) check of the
  photon-production sum. Not translated: nothing reaches a tape.
- **`fndar1`, `fndar2`** (`:9019-9156`): not translated. `acelf6` calls them for
  `lang = 1, newfor /= 1` and `lang > 2, newfor = 0` (`:7722-7795`), but with
  `newfor = 0` `topfil` first turns every neutron `LAW = 1, LANG /= 2` subsection into
  `LAW = 7` (`fix6`, `:2233-2290`). No deck reaches them. Left open on gh:#1372, since
  `:7495` suggests a route not found here.
- **`chekit`** (`:3283-3314`): **translated** (`acer::photon_angular::chekit`). `pttab`
  refused `NR > 1`; upstream stops for `NR > 2` and sends `NR = 2` to `chekit`, which
  folds a last-interval step to zero into one histogram region. No held evaluation has
  such a table, so it is pinned by the unit test
  `chekit_folds_a_two_region_step_into_one_histogram` only (prediction on gh:#1372 first).
- **Listings** `acepdd`, `aceppp`, `acepcp` (`:11710-12986`): they write to `nsyso` only,
  not to a compared unit. Not written.

**RECONR `LRF = 6`** (`rdf2hy`, `cshybr`, `reconr.f90:1116-1224`, `:3609-3824`). Every
MF=2/MT=151 line held was scanned for a range CONT `(EL, EH, 1, 6, NRO, NAPS)`: there is
none in 133 materials. The first ranges are `LRU,LRF` = `(1,1)` ×10, `(1,2)` ×37,
`(1,3)` ×37, `(1,7)` ×12, `(2,1)` ×1, and 36 with no resonances. `parse_lru1` now
refuses `LRF = 6` by name (it was "unknown resolved-resonance LRF"); unit test
`reconr::mf2::lrf6_tests::lrf6_is_refused_by_name`.
## 39. POWR translated, `lib = 1` and `lib = 2` (2026-11-14, gh:#1373)

Maintainer decision (2026-11-14): "POWR to translate, unit test verification with
upstream". `src/powr/` translates `powr.f90`'s `lib = 1` (EPRI-CELL fast library:
`fast`, `gamll`, `gamxs`, `gamff`, `mergt`) and `lib = 2` (thermal: `therm`,
`packa`) line for line, with `icgrp`. `run_deck` runs it (`deck/powr.rs`). Gate:
`tests/deck_driver_powr_vs_njoy2016.rs`.

**Method.** No upstream deck runs POWR, so the decks are our own. Each runs
NJOY2016's deck-11 chain (MODER, RECONR, BROADR 300/900/2100 K, UNRESR 7 sigma
zeroes, THERMR for the thermal decks) to a **coded** GENDF, then POWR. The whole
deck is run by `NJOY2016_ROOT/build/njoy` and by `run_deck`; the GENDF and the
POWR library are compared as strings, line for line, trailing blanks included.
Because the GENDF is identical first, the POWR comparison is a module-level
comparison of the translation against upstream's compiled `powr.f90` on
identical input. Pass: no differing line, the run reaches `stop`. Predictions
were posted on gh:#1373 before the first run of each deck.

| deck | tape (MAT) | GROUPR | POWR | GENDF lines | library lines (words) | result |
|---|---|---|---|---|---|---|
| A | t404 Pu-238 (1050) | ign 6, P1 | lib 1, 3 T, 7 sig0, f-factors | 4 535 | 577 (3 325) | identical |
| C | t404 (1050) + MF6 MT16 | ign 6 | lib 1, (n,2n) | 4 559 | 600 (3 459) | identical |
| D | t404 Th-232 (1296) + MT455 | ign 6 | lib 1, 6 delayed chi | 4 362 | 725 (4 179) | identical |
| E1 | t404 (1050) | ign 1, 185 groups | lib 1 collapsed (`igf`) | 11 983 | 559 (3 205) | identical |
| B | t404 (1050) + free gas | ign 8 | lib 2, 3 cards, `itrc` 0/1 | 2 557 | 413 (2 585) | identical |
| E2 | t404 (1050) + free gas | ign 1, 185 groups | lib 2 collapsed (`igt`) | 12 632 | 687 (4 229) | identical |
| F | t511 C-nat (1306) + t322 graphite | ign 8 | lib 2, `mti 229`, `mtc 230` | 716 | 120 (755) | identical |

Ablations (the gate can fail): A's library with trailing blanks trimmed differs
on all 577 lines; our A run at `izref = 2` against NJOY's `izref = 1` differs on
46; our B run with `itrc = 1` against NJOY's `itrc = 0` differs on 404.

**Where the prediction was wrong.**
- Deck C: predicted that NJOY would stop on `gamll`'s (n,2n) branch, which goes to
  label 250 instead of 300 in the middle of a section. It does not: `contio` reads
  a LIST data line with `6e11.0` and `nint`s it, so the rest of the section is
  read as CONT records without error. The refusal written for it was removed.
- Deck B, first version: NJOY stopped (`Bad value during floating point read`).
  `therm` reads the GENDF label with `contio`, so GROUPR's title `'94-pu-238'`
  stops it. The crate's ENDF float parser reads text as 0, so the port would have
  written a library NJOY never writes. The port now checks the label with a
  strict `e11.0` grammar (`powr::reader::e_field_reads`) and stops; pinned by
  `powr_thermal_text_label_stops_as_upstream` (NJOY exits non-zero there too).

**Upstream quirks kept:** `mergt` pads every fast-library line to 80 columns;
`matd < 0` ends a `lib = 1` run (the absorption-only card-6 branch is
unreachable); `gamxs`'s spectrum-part chi index `ngnd-ig2c-1` and its delayed
`a(jgt+locb)`; `gamff` not resetting `l` after paging; `packa`'s `ng = 2` on an
empty row; the thermal group bounds taken from the first material only.

**Refused by name:** `lib = 3` (EPRI-CPM, `powr.f90:1992-4902`, not translated;
the run stops at POWR); a blocked-binary GENDF (`ngendf < 0`: NJOY reads
full-precision records and positions by record); delayed nu with no MF=5 MT455
(`nldla` undefined upstream); a GENDF record past `scr(5000)`.

**Not done:** ~~`lib = 3`;~~ (**CORRECTED 2026-11-14:** `lib = 3` stays refused because
upstream writes no library, §39a) the listing (`nsyso`, with `pgam`, `pgamff`, `tprnt`);
a stand-alone gfortran build of `powr.f90` (the whole-deck comparison above
compares upstream's compiled POWR on an identical GENDF, which is the same
check at module level).

### 39a. POWR `lib = 3` (EPRI-CPM): upstream writes no library (2026-11-14, gh:#1373)

**Question** (raised by the agent that translated `lib = 1/2`): `cpm` reads card 5 into
locals but fills the per-material values from `scr`. Is card 5 lost? Answered by running
upstream before porting anything.

**Reading.** `cpm` (`powr.f90:2075-2105`) reads `nina, ntemp, nsigz, sgref, ires, sigp, mti,
mtc, ip1opt, norf, ipos, iposr` into local variables and uses them only for the listing and
for `nnina`/`nres`. `:2212-2234` then fill `ninat`, `ntmp`, `nszt`, `s0rf`, `irest`, `sigpt`,
`mtit`, `mtct`, `ip1tab`, `norft`, `ipost`, `iposrt` from `scr(1..12*newmat)`, which nothing in
`cpm` writes. Card 6 goes to `scr(2+12*newmat..)`, just above. Only the last material's local
`ntemp` reaches `sfile3/4/6`, as an argument.

**Measured** (NJOY2016.80, 69-group Pu-238 GENDF from `t404`, GROUPR `ign = 9`; POWR
`3 1 1/ 1 1 1 0 0 0 1/ 1050/`, card 5 `nina 3 7 1.e10 ires 10.0 0 0 0 0 1 1/`):
- An instrumented build (one `write` after the fill, relinked from the vendored objects in a
  scratch directory) prints every per-material value as 0; `scr(1..13) = 0`,
  `scr(14..26)` = card 6.
- The unmodified binary on eight card-5 variants:

| `nina` | `ires` | NJOY2016 | unit 27 |
|---|---|---|---|
| 0, 3 | 0, 1 | stops in `sfile2`: "reference sigma zero not found" (`s0rf = 0`), exit 77 | empty |
| 1, 2 | 0 | "Attempt to DEALLOCATE unallocated 'glamt'" (`sfile3`, `:3032`), exit 2 | empty |
| 1, 2 | 1 | SIGSEGV, exit 139 | empty |

- **Ablation** (a scratch NJOY whose `cpm` stores card 5 into `scr`): the `sfile2` stop
  disappears for `nina = 0, ires = 0` and the run stops later, in `sfile6` ("no required p1
  matrices found"). So the card-5 loss is what stops `sfile2`. The other variants still end in
  an error or SIGSEGV, and no variant writes a library. So `lib = 3` has failure points past
  card 5 too, which were not chased.

**Decision.** There is no upstream output to reproduce. What upstream does depends on what
`scr` holds: zeros in a fresh run, or an earlier POWR's GENDF words. So `lib = 3` stays
refused by name, now with this reason in the message (`powr::run_powr`). The 2 900 lines
(`powr.f90:1992-4902`) were not translated: nothing could verify them against upstream.
Repairing `cpm` would be a change to NJOY, which is the maintainer's call.

**Gate:** `tests/deck_driver_powr_vs_njoy2016.rs::powr_cpm_lib3_upstream_writes_no_library`
(prediction on gh:#1373 first; as predicted). It runs the `nina = 0`, `nina = 1` and
`nina = 1, ires = 1` variants through both codes. It asserts that NJOY2016 exits non-zero
with unit 27 empty and the recorded stop message, and that the port refuses `lib = 3` at
POWR. If upstream ever wrote a `lib = 3` library, this test would fail and point here.
