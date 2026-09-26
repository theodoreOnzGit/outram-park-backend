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

| block | U-234 293.6 K | U-235 0 K |
|---|---|---|
| header, MTR, TYR | SAME | SAME |
| LQR | SAME | SAME (84 words) |
| NU | SAME (37 words) | SAME (347 words) |
| TYR / LAND | SAME, every word | SAME, every word |
| **AND** | **SAME, all 78 143 words** | **SAME, all 138 687 words** |
| LDLW | SAME, every word | SAME, every word |
| **DLW** | **SAME, all 113 139 words** | **SAME, all 913 698 words** |
| DNU / BDD / DNEDL / DNED | SAME, every word (DNED 10 920) | SAME, every word (DNED 10 920) |
| UNR | grid SAME (26 pts), flags/bands SAME; band values are PURR samples | absent in both |
| photon production MTRP..DLWP | SAME entries and forms | SAME entries and forms |
| ESZ heating | SAME (zero, no-HEATR deck) | SAME |
| **ESZ grid** | **42 404 vs 25 393 points, 5.5 % shared** | **237 058 vs 233 515, 14.5 % shared** |
| ESZ / SIG values at shared points | 1.3e-3 / 9.8e-3 worst | 1.4e-4 / 9.1e-4 worst |
| GPD | missing in ours | missing in ours |
| charged-particle production (`acelcp`, NXS(7)) | absent in both | **missing in ours** (NJOY: 5 types) |

At the start of the day **every** row from AND downward differed on both
nuclides. The UNR band values cannot match word for word, because PURR samples
random ladders. The writer is proven exact on NJOY's own bands by
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

- **The energy grid.** `acelod` takes the ESZ grid from the PENDF's MT=1
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
  this, and are not quoted as agreement.
- **GPD** needs `gamsum` (`acefc.f90:3572-3866`).
- **Charged-particle production** needs `acelcp` (`acefc.f90:9178-11113`).
- **Unexercised paths:** LANG = 11-13, discrete lines (`ND > 0`), multiple
  neutron subsections in `acelf6`, and LF = 7/9/11/12 in `acelf5` are ported
  line for line. None of them is compared against NJOY here, because neither
  reference nuclide reaches them.
