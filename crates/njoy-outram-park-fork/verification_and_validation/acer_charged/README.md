# ACER of an incident charged particle vs NJOY2016 (gh:#1485, parent #1369, epic #1336)

Research, education and V&V only (`RESPONSIBLE_USE.md`). NJOY2016.80 at commit
`ac5adf5f33d893e42f2eed7fb286b0d51c7580da`; this port is a modified, non-LANL
version.

## What was ported

`acefc.f90`'s `izai != 1` branches, in `src/acer/charged.rs`,
`charged_coulomb.rs` (`ptlegc`, `coul`), `acelcp.rs` and `acelcp_ext.rs`:
`first`/`unionx` (union grid, MT=3 redundancy, empty dictionary), `acelod`,
`acecpe`, `acelpp` for a LAW=2 photon, and `acelcp` (production blocks, the
elastic recoil MT=2, LAW=6, MF=6 `mt102` recoil, recoil heating).

## Integration gate: whole ACE table, test decks 14, 50, 51, 52, 53, 54, 62, 71

`tests/deck_driver_tapes_vs_njoy2016.rs` runs each deck's `moder; moder; acer
(iopt = 1)` through the deck driver and through the compiled NJOY2016, and
compares unit 31 (NXS, JXS, every XSS word at the 12 figures the Type-1 file
prints). **Result, 2026-11-11, rustc 1.99.1: identical in every word on all
eight decks** (words: 71 369, 604, 5 989, 15 895, 48 072, 29 259, 28 836,
290 327). The decks' second `acer` card (`iopt = 7`, the plot file) and `viewr`
are cut; they are not ported (survey rows edited by hand, not regenerated).

Differences found on the way, each read from upstream and fixed (numbers are
before the fix):

| defect | deck | cause upstream |
|---|---|---|
| NXS(7) = 0 expected, ours wrote production | 62 | an evaluation with an empty MF=1/MT=451 dictionary (NXC = 0) has no MF=4/6 item for `first` (`dictio`, `nw = nx`, `acefc.f90:373`) |
| MF=3/MT=102 kept as a production item for any `izai` | 51 | the dictionary test has `izai.eq.1` (`:405`) |
| heating 1e-6 high in 3 production blocks | 54 | `acelod` overwrites the module `awr` with the AWR of the last MF=4/6 section it reads (`awr=c2h`, `:5805`): 2.9896, not MF=1's 2.989596 |
| elastic recoil refused (MT=2, MF=4 or MF=6) | 54 | ported: `:9370-9381`, `:9555-9636`, `:10134` |
| LAW=6 refused | 51 | ported: `:10601-10742` |
| `mt102` recoil block with the wrong AWP | 51 | MF=6 `mt102` branch, `:10287-10316` |
| a LAW=2 photon's energy rounded through text | 51 | `convr` opens its scratch tape in the mode of the ENDF unit (`if (nin.lt.0) nout=-nout`, `:3908`); binary here |
| GPD rounded through text, 18 words at 6 figures | 71 | `gamsum`'s `mscr` takes the PENDF unit's sign (`:176`); binary here |
| MT=3 refused | 71 | `:1713-1734`: MT=4 is checked for `izai = 1` only; MT=3 is kept only if MF=12 has it |

## Function-level gates

`tests/acer_charged_functions_code_to_code.rs` against `reference_output.txt`,
which `build_and_run.sh` makes by compiling NJOY2016's own `coul`, `ptlegc`
(exported by a one-line edit of the `public` statement; `acefc.f90` SHA-256
checked) and `terp1` into `driver.f90` (gfortran 13.3, `-O3`; `-O0` gives the
same bits), on the LIST records `extract_lists.py` takes from the test
evaluations (`lists.txt`). **Prediction, stated here after the first run and
not before it** (that is a departure from the brief): bit-identical, with the
maths library the one place a last bit could move.

| routine | inputs | bit-identical |
|---|---|---|
| `terp1`, `INT = 6` | 6 points, thr6 = 0 | 6 / 6 |
| `ptlegc` | 9 records (p+H-1, p+H-3, d+H-2; first, middle, last energy; LIDP 0 and 1) | 9 / 9 records, **439 / 439 points** |
| `coul` | the same 9 records at 5 cosines | **43 / 45**; the two others differ by 1.52e-16 relative |

The two `coul` values are d + H-2 at 1.4 MeV, x = +-0.5 (LTP = 1, LIDP = 1).
Cause, shown and not assumed: `cos(eta ln 0.25)` is 0.9659188400321088 in
PETIR's `libm` and 0.9659188400321087 in glibc, at the exact `eta` `coul` uses
(`identical_particle_cosine_differs_only_in_libm`); a Python transcription of
the Fortran expression with glibc maths reproduces NJOY's bits
(`0x3FD75F5020806155`). The workspace keeps PETIR's maths (gh:#1455), so this
is pinned as exactly these two words, under 2e-16. ~~Not covered: `LTP = 2`
(no held evaluation has one), `LTP >= 12` (tabulated; `acecpe` never calls
`coul` for it; the driver segfaulted when fed such a record, probably through
`coul`'s 65-word `p`, which was not investigated).~~ **CORRECTED 2026-11-12
(gh:#1490):** `LTP = 2` is now gated on synthetic records, and the `LTP >= 12`
crash is explained; see the next section.

`law6_h` (the LAW=6 heating; it never reaches the ACE for a neutron, so the
deck cannot gate it): 119 / 119 bit-identical to NJOY2016.80 built with
`law6_h_instrument.diff` (one `write`), `law6_h_reference.txt`, gfortran
`-O0`. The grid itself is gated by deck 51's ACE words.

`tests/acer_charged_particle.rs::union_grid_matches_esz_of_every_charged_deck`:
the ESZ energy column of the seven decks, 894 energies, 0 differing
(`esz_energies.txt`, from NJOY2016's `tape31`).

## gh:#1490: the branches left after #1485 (2026-11-12)

Predictions were posted on #1490 before any of these runs.

**Reachability.** `tests/acer_charged_particle.rs::held_charged_evaluations_reach_only_ported_branches`
walks every MF=6 subsection of the seven held charged-particle evaluations
(`epn14`, `p-001_H_001/002/003`, `d-001_H_002`, `d-002_He_003`,
`a-002_He_004`; `reference-data/endf/` holds only the same alpha + He-4
tape) and pins what it finds:

| branch | held evaluations that reach it | status |
|---|---|---|
| `coul` with `LTP = 2` | none; but d + He-3's 100 eV record has `LTP = 0`, undefined in ENDF-6, which upstream's `if (ltp.eq.1) ... else` sends down the `LTP = 2` formula (values near 1e-238; deck 62's ACE is identical) | gated at function level on synthetic records, below |
| `LTP >= 12` | `epn14`, p + H-2, alpha + He-4 | tabulated; used as given, gated by decks 14, 51, 50. Never passed to `coul` (`if (ltp.lt.12)`, `acefc.f90:6552`) |
| LAW = 5 product other than the projectile | none | refused. ENDF-6 gives LAW = 5 to the scattered projectile only, and upstream `acelcp`'s product loop has no LAW = 5 branch (`:10318-10743`): such an item would be read past without a skip |
| LAW = 4 recoil heating (`izap > 2004`, `:11036-11080`) | none (the heaviest LAW = 4 recoil held is ZAP 2004, which the branch excludes) | refused. The branch reads `ne` (in the interpolation header, before it is re-read), `e` (in `betasq`) and `amass`, all left over from earlier code, and adds `q` in MeV to `e` in eV; a port could only be checked on a synthetic tape |
| elastic recoil (MT = 2) | deck 54 (p + H-3). Deck 62's recoil is never reached: its empty dictionary gives it no production | gated at function level, below |

The full ENDF/B-VIII.0 charged-particle sublibraries are not held. Their
hosts (NNDC, IAEA NDS) are refused by this environment's proxy, so whether a
LAW = 4 recoil heavier than an alpha occurs there (likely, for p + Li-6/7
and heavier targets) was **not checked**.

**`LTP >= 12` crash: the harness, not NJOY.** `ltp12_probe.f90` feeds the
LTP = 12 records (`lists_ltp12.txt`, `extract_lists.py --ltp12`) to `coul`.
`coul` holds `real(kr)::p(65)` and calls `legndr(x,p,np)` with
`np = 2*nint(c(6))`; for LTP = 12, `c(6)` is the number of tabulated
cosines. p + H-2 has NL = 36, so `legndr` (whose `p(*)` is assumed-size and
never bounds checked) writes `p(1..73)`. With gfortran 13.3 the next local
is `np` itself (`&p(65) = ...8210`, `&np = ...821c`, read in gdb), so
`p(66)`'s upper half becomes the loop bound. Measured: `-O3`, SIGSEGV on the
first call; `-O0 -fcheck=bounds`, `Index '66' of dimension 1 of array 'p'
above upper bound of 65` at `x = 0.5` (at `x = 0`, `P_65(0) = 0` sets
`np = 0` and the call returns quietly). The alpha + He-4 records (NL <= 27
in the picked records, so `np + 1 <= 65`) run without error. The prediction
named the right overrun but expected `-fcheck=bounds` to catch it inside
`legndr`; it cannot, and caught the read in `coul` instead. Upstream never
makes this call, so there is nothing to port.

**`coul`/`ptlegc` with `LTP = 2`** (`driver_ltp2.f90`,
`reference_output_ltp2.txt`; the nine `lists.txt` records with `c(3) = 2`, a
test of the arithmetic, not physics):
`tests/acer_charged_functions_code_to_code.rs::coul_and_ptlegc_ltp2_match_njoy2016`.
**Result: `coul` 45 / 45, `ptlegc` 9 records and 447 / 447 points
bit-identical**, as predicted.

**`elastic_recoil_angular`** (`acefc.f90:9555-9636`):
`elastic_recoil_instrument.diff` adds `write`s to the `mt.eq.2` block;
NJOY2016.80 built from it (gfortran 13.3, `-O3`, every file) gives deck 54's
unit 31 byte-identical to the unpatched build's, and prints the block's
pointers, `awr`/`awi`, its input words (the scattered particle's AND block,
4 628 words; the ESZ energies; the production's HPD block before the
recoil heating) and its output words (the recoil's AND block, 4 628 words,
and the 140 heating accumulators), all as IEEE-754 bits.
`elastic_recoil_reference.sh` rebuilds `elastic_recoil_reference.txt`. The
unit test `acer::acelcp_ext::tests::elastic_recoil_matches_njoy2016` puts the
inputs at the same XSS indices and calls the Rust function. **Result: 4 768 /
4 768 words bit-identical**, as predicted (`next` and the LANDH word equal).
The prediction also named deck 62; it does not reach this block (above).

## Not done

~~`elastic_recoil_angular` has no separate driver: its words (AND block, the
cdf sums, the heating through `h(E)` log-log) are in every word of deck 54's
ACE and its heating in ESZ. An instrumented NJOY was read once for it: `e` and
`ubar` matched to 17 figures at the first five energies, and `amass` did not,
which is how the `awr` defect above was found; it was not re-compared after
that fix, the deck-54 identity is the evidence.~~ **CORRECTED 2026-11-12
(gh:#1490):** it has a function-level gate now (above). LAW=5 products other than the projectile, and the LAW=4
recoil heating of an incident charged particle, are still refused, and are
not reachable from any held evaluation (above).
