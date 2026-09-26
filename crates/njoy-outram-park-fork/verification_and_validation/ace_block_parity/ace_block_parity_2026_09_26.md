# Reproducing NJOY2016's own ACE tables, block by block

Measured 2026-09-26. Goal: the continuous-energy ACE tables this crate writes
should reproduce NJOY2016's, **all of their data**, for the same evaluation and
deck.

## Methodology

**References.** NJOY2016 `ac5adf5f`'s own tables in the `reference-data/ace`
submodule, `reference-njoy/endf-b-viii.0/`:

| case | deck |
|---|---|
| `293.6K/U234.ace.gz` | `RECONR -> BROADR(293.6 K) -> PURR(20 bins, 64 ladders) -> ACER` |
| `0K/U235.ace.gz` | `RECONR -> ACER` |

Neither deck runs HEATR, so NJOY's PENDF has no MT=301 and `acelod` writes a
zero heating column (`acefc.f90:5641-5645`).

**Ours.** The same ENDF/B-VIII.0 evaluations from `reference-data/endf/`, built
through `acer::build_deck` with the same modules:
`AceDeck::default().without_heatr()` for 0 K, plus `.with_purr(20, 64, 10_000)`
at 293.6 K. The HEATR ablation is **only** for this comparison; the default
deck keeps heating on.

**Instrument.** `examples/ace_blocks_vs_reference.rs -- <U234|U235|U238>
<0K|293.6K>`. Ours goes through Type-1 text and the production reader first, so
both tables are compared as printed (`1pE20.11`). Two kinds of rows:

- **`... words`**: the XSS words from a block's locator to the next block
  present in that table, compared **exactly**. "SAME (n words, every word)"
  means every printed value is identical.
- **Content rows**: decoded values. The grid-dependent blocks (ESZ, SIG) are
  compared only at energies both grids share, and the shared fraction is
  printed.

The pass criterion is word-for-word identity. Nothing here uses a tolerance.

## Results, 2026-09-26

References: NJOY2016's own tables in `reference-data/ace`, for U-234 and
U-238 at 293.6 K (`RECONR -> BROADR -> PURR -> ACER`) and U-235 at 0 K and
293.6 K. Rows are word-for-word comparisons of whole blocks (`... words`).

| block | U-234 293.6 K | U-238 293.6 K | U-235 293.6 K | U-235 0 K |
|---|---|---|---|---|
| ESZ (grid, total, absorption, elastic, heating) | **SAME** (126 965) | **SAME** | **SAME** | **SAME** (1 167 575) |
| MTR / LQR / TYR / LSIG | **SAME** | **SAME** | **SAME** (85, MT=4 last) | **SAME** (84) |
| SIG | **SAME** (65 751) | **SAME** | **SAME** | **SAME** (9 567 354) |
| NU, LAND/AND, LDLW/DLW | **SAME** | **SAME** | **SAME** | **SAME** |
| DNU / BDD / DNEDL / DNED | **SAME** | **SAME** | **SAME** | **SAME** |
| GPD | **SAME** (25 393) | **SAME** | **SAME** | **SAME** (233 515) |
| MTRP / LSIGP / SIGP / LANDP / ANDP / LDLWP / DLWP / YP | **SAME** | **SAME** | **SAME** | **SAME** (YP 3 391 686) |
| UNR (LUNR) | **SAME** (3 152) | **SAME** (10 049) | **SAME** (2 305) | absent in both |
| charged-particle production (`acelcp`, NXS(7)) | absent in both | **SAME** | **SAME** | **SAME** |
| **XSS, whole table** | **SAME** (449 695) | **SAME** (6 247 445) | **SAME** (6 712 632) | **SAME** (15 616 079) |
| JXS | **SAME** | **SAME** | **SAME** | **SAME** |

This table is the **final state, later on 2026-09-26**: all four reference
tables are reproduced **in every XSS word**, and in NXS and JXS. The rows
"GPD", photon production, UNR and `acelcp` were different or missing when this
record was first written; how each closed is under "The last blocks" at the
end. The whole-table row compares the entire XSS array word for word, so no
block can differ without it failing.

~~**U-234 at 293.6 K is identical to NJOY2016's table in every word except
PURR's probability-band values.** Those come from random resonance ladders.
Our PURR ports upstream's `rann` and seed, and with the same 10 000 samples
the first band count is 1 068 against NJOY's 1 070. The generator is ported,
but some input or arithmetic before it is not yet identical; this is not
closed.~~ **SUPERSEDED later 2026-09-26**: the bands match too; see
"The last blocks" at the end.

The GPD and photon rows for U-235 and U-238 were measured before the MF=12
`LO = 2` conversion was ported (`photon_blocks::build_with_pendf`,
`Lo2Cascade`); they are re-measured below when that lands.

The same fixes reproduce NJOY's **intermediate PENDFs**, word for word.
Instrument: `examples/pendf_stage_vs_njoy.rs`; regression:
`tests/pendf_stages_vs_njoy2016.rs`.

- **RECONR:** U-234, U-238, Si-30, Sr-88, Ar-37, Li-6, C-12. References are
  NJOY2016 `ac5adf5` PENDFs. The ones committed under
  `reference-data/reconr/` are Si-30, Sr-88, Ar-37 and U-234; the rest were
  regenerated in-session with the same deck.
- **BROADR** at 293.6 K: U-234 and U-238, where NJOY broadens MT 2/18/102
  and 2/5/18/102/107/800 together and writes 25 393 and 155 207 points out.
  Also H-2, Li-6, Be-9, C-12, F-19, Si-30, Cl-35 and Ar-37, against
  `reference-data/errorr/*-293.6K.pendf`.

At the start of the day **every** row from AND downward differed on both
nuclides. ~~The UNR band values cannot match word for word, because PURR samples
random ladders.~~ **CORRECTED later 2026-09-26**: they can and do, because
`rann` and its seed are ported, so the ladders are the same ladders (see "The
last blocks"). The writer is proven exact on NJOY's own bands by
`tests/unr_block_write_vs_njoy2016.rs`.

## What was wrong, in the order found

Each item was a port divergence from the upstream routine that owns the
behaviour. The upstream reference is given with each.

1. **AND used this crate's own Legendre linearisation** (`ANGLE_TOL = 5e-3`)
   instead of `ptleg2`/`pttab2` (`acecm.f90:226-434`). With `newfor = 1`,
   `acensd` also never writes a per-energy isotropic locator (`:6411`): an
   isotropic row becomes three cosines. Fixed by porting `ptleg2`, `pttab2`
   and `acensd`'s rounding (`src/acer/acensd.rs`). Transport keeps its own
   parser, `parse_mf4_angular`.
2. **RECONR put resonance fission on MT=18 only.** Upstream drops the tape's
   MT=18 when MT=19 exists (`lunion`, `reconr.f90:1893`) and gives MT=19 the
   resonance fission (`itype = 3`, `:4760`). It then rebuilds MT=18 as the sum
   of MT=19/20/21/38 (`:4886`). So our ACE carried MT=18 where NJOY carries
   19/20/21/38. Predicted before running: MT=19 takes exactly the
   3.448068842 b MT=18 had at 1e-5 eV, and the total does not move. Measured:
   both held.
3. **MF=6 was written as ACE law 4** (energy only, angle dropped) behind a
   generic header. It is now a port of `acelf6` (`src/acer/acelf6.rs`):
   - law 44 and law 61 (the latter through `ptleg2`);
   - constant and generalized yields (U-235 MT=5: `TYR = -101`);
   - `LNW` chains, law 66, and `LAND = -1`.
4. **MF=5 was written unrounded** behind the same generic header. It is now a
   port of `acelf5` (`src/acer/acelf5.rs`), with `topfil`'s leading-zero
   collapse.
5. **`sigfig` in `acer::build` omitted upstream's rounding nudge and bias.** It
   now delegates to the faithful `mixr::mix::sigfig`: 654 DNED words, and
   later the Law 3 words.
6. **`fx = .8409` is a single-precision literal** (`acefc.f90:6183`, `:7555`).
   With it read as double, 584 DNED words differed at about 2e-7.
7. **MT=455 decay constants were read as the ν̄ TAB1**, which made DNU 15
   words against NJOY's 11.
8. **SIG always started at the grid floor.** `acelod` stores each reaction
   from its threshold index (`:5466-5505`), and the inline Law 3 header uses
   that index's energy (`:5921-5935`).
9. **`lunion`'s kinematic threshold raise was unported** (`:1913-1938`). It
   moves a first energy below `sigfig(-Q(A+1)/A, 7, +1)` up to that value.
   Before the fix, all 40 U-234 level thresholds were one unit low in the 7th
   figure.
10. **`acelod`'s Law 3 `awr` is whatever the last MF=4/MF=6 HEAD held**
    (`awr=c2h`, `:5806`). For U-234 that is 232.0300 (MF=6), not 232.0304
    (MF=1). One level's `ldat1` sits on a 7th-figure boundary that this
    decides. It is reproduced as an upstream quirk, and the code says so.
11. **The ENDF float parser rounded twice** (`m * 10^e`). As a result
    `1.110000+7 - 1.090000+7` came out as 200000.00000000186. That tripped
    `acelf5`'s strict `dele > 2e5` on U-235's exactly-200-keV panels:
    16 extra points per incident energy, 2 568 extra DLW words. The parser now
    converts the whole decimal once, as a Fortran formatted read does.
12. **MT=103-107 kept the tape's own section and Q.** Upstream always rebuilds
    them from MT=600-849 and writes `Q = 0` (`recout`, `:5280`). NJOY stores
    0 for U-235's MT=103 and MT=107, where we had -0.82 and 11.12 MeV.

## What this does not yet establish, and why

- ~~**The energy grid.**~~ **CLOSED 2026-09-26**: the ESZ grid and every
  SIG word now match (table above). What was missing, in the order found,
  is under "Closing the grid" below. The original entry:
  ~~**The energy grid.** `acelod` takes the ESZ grid from the PENDF's MT=1
  (`acefc.f90:5345-5356`), and NJOY's RECONR writes every reaction on one
  union grid. Three pieces of it are unported:
  - `rdf2*`'s resonance nodes: `E_r`, `E_r ± Γ/2` at `sigfig(·, ndig)`, plus
    range edges and 0.0253 eV;
  - `lunion`'s union linearisation: decade points and the `1+sqrt(5.3·err)`
    step cap below `elim`;
  - `emerge` evaluating every reaction on that grid.

  This crate keeps a grid per section and seeds its resonance refinement from
  its own halo of `E_r ± k·Γ/2`. NJOY bisects [1e-5, 2e-5] eV in steps of
  1/32; ours places unrounded points such as 1.06313123764e-5 eV that no
  `resxs` step can produce. The ESZ and SIG value residuals above follow from
  this, and are not quoted as agreement.~~
- ~~**GPD** needs `gamsum` (`acefc.f90:3572-3866`).~~ Ported
  (`photon_blocks::gpd`); U-234 SAME in all 25 393 words.
- ~~**Charged-particle production** needs `acelcp` (`acefc.f90:9178-11113`).~~
  Ported (`src/acer/acelcp.rs`); see "The last blocks".
- **Unexercised paths:** LANG = 11-13, discrete lines (`ND > 0`), multiple
  neutron subsections in `acelf6`, and LF = 7/9/11/12 in `acelf5` are ported
  line for line. None of them is compared against NJOY here, because neither
  reference nuclide reaches them.

## Closing the grid, 2026-09-26

Each item is a divergence from the upstream routine that owns it, found by
the stage comparator. The number in brackets is what it moved.

13. **RECONR's union grid** (`lunion`, `reconr.f90:1771-2238`) and
    `rdf2*`'s resonance nodes were ported (`src/reconr/lunion.rs`). `emerge`
    now evaluates every section with `gety1` of its own TAB1 and rounds once.
    [U-235 0 K: grid 14.5 % -> 100 % shared.]
14. **PENDF text at the ACER boundary.** ACER reads 11-column floats, which
    carry six figures when the exponent has two digits. [U-235 0 K SIG:
    246 768 differing words -> 4 554.]
15. **Background from the raw TAB1, rounded once.** It was lin-lin of
    already-rounded union values. [4 554 -> 11.]
16. **Redundant sums rounded as `recout` writes them**:
    `sigfig(sum, 7, 0)` for MT=1, 4, 18 and 103-107 (`reconr.f90:5308`).
    [11 -> 0: every U-235 0 K SIG word identical.]
17. **Resonance reactions are written from the first resonance point.**
    For `itype != 0`, `emerge` lowers the threshold to the first resonance
    point and skips no grid point (`reconr.f90:4755-4784`). U-234's MF=3
    backgrounds are zero to 100 keV, and its sections started at 0.0253 eV.
    [U-234 293.6 K elastic at 0.0253 eV: +1.7 % -> SAME.]
18. **The unresolved range through `genunr`/`sigunr`.** `resxs` bisects the
    interpolated MT=152 table, and `emerge` zeroes the background in
    `[eresr, eresh)` for MT=1/2/18/19/102 (`reconr.f90:1628-1769, 4789`).
    It used to evaluate the formula directly, with this crate's own
    refinement. [U-234 RECONR: 47 extra points and unrounded values -> SAME.]
19. **MT=3 and MT=4 as `anlyzd` has them.** MT=3 is written only as a
    redundant sum, and only when MF=12 carries MT=3. MT=4 is redundant, with
    Q = 0, when levels 51-91 exist. [U-234: an extra MT=3, and MT=4 Q
    -43.5 keV -> 0.]
20. **BROADR's joint walk** (`broadr/joint.rs`): every low-threshold reaction
    broadened together on MT=1's grid, as `broadr`/`bfile3`/`broadn`/`bsigma`
    do. That includes `nstack = 12`, paging, SLATEC `erfc` and `hnabb`.
    [U-234 grid 41 541 vs 25 393 points -> SAME; U-238 -> SAME.]
21. **MF=10 in the union**, with the same threshold raise (`lunion`,
    `nss = n1h`). [Ar-37: two NJOY points above 22 MeV.]
22. **`emerge` reads `lunion`'s TAB1s through a formatted scratch tape.** A
    raised threshold `sigfig(thr,7,+1)` loses sigfig's 1e-13 bias there.
    [Sr-88: MT=63/66/72 one unit off in the 7th figure -> SAME.]
23. **LRP != 1 never reaches `rdfil2`.** There are no nodes, and
    `eresr = eresh = 20 MeV`; the PENDF limit is 20 MeV. [C-12, Li-6: 1e5 eV
    decade point missing, and unbroadened above 1e5 eV -> SAME.]
24. **MT=4 kept for URR competition** (`mtcomp`, `acefc.f90:1164-1181`).
    PURR's MT=153 names MT=4 for U-235. [U-235 293.6 K: 84 -> 85 reactions,
    MT=4 last as `acelod` appends it.]
25. **Photon blocks, U-234:**
    - MF=13 SIGP entries start at `gety2`'s thresholded point, with values
      at `sigfig(y,7)`;
    - MF=15 law 4 follows `acelpp`: pdf at 7 figures, cdf from raw values,
      both at 9 figures after renormalizing, and NR = 0 for one lin-lin
      region;
    - the MF=13 law header range comes from the ESZ grid;
    - GPD is `gamsum`.

    [U-234 photon blocks and GPD -> SAME.]

**Still open** (as of item 25; see below for what closed):
- ~~`acelcp` (charged-particle production, U-235/U-238);~~
- ~~the LO=2 photon conversion, re-measured once it lands;~~
- ~~PURR band values;~~
- a multi-subsection LO=1 MF=12 total rebuilt by `convr`. No reference tape
  exercises it; `gpd` returns `None` for such a tape rather than a guessed
  block.

## The last blocks, later 2026-09-26

The same instrument, the same four references. Each item is a divergence from
the upstream routine that owns it. The bracket is what it moved.

26. **Photon production, U-235 and U-238** (`photon_blocks.rs`):
    - MF=12 `LO = 2` through `convr`'s cascade (`Lo2Cascade`);
    - MF=16 law 4 rounding as `acelpp` does it;
    - MF=6/MT=18 photons skipped when `jp - 10*(jp mod 10) /= 0`, the JP rule.

    [U-235/U-238 GPD, MTRP-DLWP and YP: different -> SAME.]
27. **`acelcp`, charged-particle production** (`src/acer/acelcp.rs`, a port
    of `acefc.f90:9178-11113` for the branches these evaluations reach):
    - HPD, MTRH, TYRH, LSIGH, SIGH;
    - LANDH (`-1` for MF=6, `0` for isotropic MF=4);
    - DLWH: law 33 for MF=4, and MF=6 LAW=1 with LANG 1 (`ptleg2`), 2 (law
      44 via `bachaa`) and 11+ (law 61);
    - heating and YH, and NXS(7), JXS(29-31).

    Branches no reference reaches are refused, not approximated. [U-235 0 K,
    U-235/U-238 293.6 K: missing -> SAME.]
28. **PURR's resonance window is `fsrch`'s** (`unrest`, `purr.f90:1931-1934`).
    It includes the last sample at or below `elo`. `partition_point` started
    at the first sample above it. [U-234 LUNR: 2 551 differing words -> 2 415.]
29. **A sample equal to a bin edge goes to the bin above**
    (`purr.f90:2333-2335`: `fsrch`, then `ii=ii+1` unless below the first
    edge). The edges are samples themselves, so ties are common. [2 415 ->
    2 225.]
30. **MT=153 is a text hand-off.** PURR writes `sigfig(tabl, 7, 0)` to the
    PENDF (`purr.f90:505`, `:522`), and ACER sums the probabilities it reads
    back (`acefc.f90:5978-5979`). The cumulative is therefore a sum of
    7-figure values. [2 225 -> 2 214; the first band's cumulative became
    identical.]
31. **PURR's temperature is the card's 293.6 K.** The comparison example took
    it from the reference header's kT, which is printed to five figures, and
    `2.5301e-8 MeV / k_B` is not 293.6 K. This was an instrument defect, not
    a library one. The fix is `AceDeck::with_temperature`, which the example
    now uses. Located with an **instrumented NJOY2016 build** (a scratch copy
    of `purr.f90` that writes `unresx`'s sequence constants, each ladder's
    means and the unnormalised table at full precision). Its sequence
    constants were bit-identical to ours and its ladder means agreed to
    1e-14. So the ladders were the same, and only the table differed. [U-234
    LUNR 2 214 -> **0**.]
32. **The LSSF=1 background keeps the competition remainder** as `rdf3un`
    does: `tol = 1e-6`, and zeroed only without competition or below `ecomp`
    (`purr.f90:1195-1230`). Ours had `tol = 1e-3` and neither test, so just
    above U-238's 45 keV inelastic threshold, where the remainder is under
    0.1 % of the total, it was dropped. [U-238 LUNR 252 differing words
    (13 energies, 45.1-45.8 keV, values only) -> **0**.]
33. **`ladr2` samples one resonance past `ehigh` and discards it**
    (`nr=ir-1`, `purr.f90:1785`). Ours kept it. Found by dumping every
    sample of one ladder from both codes: the last 174 of 10 000 differed
    smoothly, one resonance's tail. U-235's fission widths make that tail
    reach the top of the ladder; U-234's and U-238's do not. [U-235 LUNR
    1 461 differing words -> **0**.]

Also ported, with no measured effect on these three: upstream skips a single
sample in the x <= -100 asymptotic run (`if (i1.le.i0) go to 240`). At U-234's
first energy the window edge is at x ~ -21, so it cannot fire there.

**Which numbers to quote.** The whole-table row: four of four NJOY2016
reference tables reproduced in every XSS word. The PURR bands are
deterministic, because `rann` and its seed are ported, so there is no
statistical allowance anywhere in that claim.

**What it does not establish.** It covers these four decks on three
evaluations. The unexercised paths listed above are still unexercised, and
`acelcp` refuses the branches no reference reaches. Nothing here tests a
nuclide with photon or charged-particle laws outside the ones U-234/235/238
use.

**Regression gates.** `tests/unr_block_write_vs_njoy2016.rs`,
`purr_generates_njoys_bands_word_for_word`, generates all three nuclides' bands
from the evaluations and requires NJOY's words. The whole-table comparison is
`examples/ace_blocks_vs_reference.rs`. It takes 4 to 8 minutes per 293.6 K
case, so it is an instrument and not a test.
