# CCCCR — CCCC standard interface files

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


> NJOY2016 module port. Theory summarised from the NJOY2016 manual
> (LA-UR-17-20093, §CCCCR); upstream Fortran: `ccccr.f90`.

## Theory

The **CCCC** interface files ("four cees") were standardised by the Committee for
Computer Code Coordination for the US Fast Breeder Reactor Program to let
different reactor codes exchange multigroup data. CCCCR produces them from GROUPR
**GENDF** output:

| File | Content |
|---|---|
| **ISOTXS** | isotope-ordered multigroup cross sections + scattering matrices |
| **BRKOXS** | Bondarenko self-shielding factors vs. σ₀ and temperature |
| **DLAYXS** | delayed-neutron precursor yields, decay constants, spectra |

These are fixed-layout **binary** records (the CCCC-III/IV standard): file
identification, control/counts, then per-isotope data blocks in the mandated
order.

## How the port implements it

~~**Not yet ported.** Requires GROUPR (`../groupr/README.md`) first. Planned: an
owned writer for each of the three record structures, reading from the GENDF
`Tape`.~~ **CORRECTED 2026-11-13 (gh:#1369):** ported as a line-for-line
translation of `ccccr.f90`, not as three owned writers. Byte identity needs
upstream's arithmetic and its memory layout, so the port keeps both:

| file | upstream | content |
|---|---|---|
| `core.rs` | module `ccccm` (`:1-71`), `stow`; `endf.f90`'s `contio`/`listio`/`tosend`/`tofend`/`tomend`/`tpidio` | the equivalenced container `a`/`ia`/`ha`/`ta` as 100 000 four-byte words; the scratch tapes; the GENDF cursor; the record writer |
| `state.rs` | module variables, `ccccr`'s cards, `ruinis`, `ruinbr` | the cards and every module variable, as one owned struct |
| `isotxs.rs` | `cisotx`, `ruinis`, `isxdat`, `prinxs`, `isomtx`, `shuffl`, `wrtmtx` | ISOTXS |
| `brkoxs.rs` | `cbrkxs`, `brkdat`, `xsproc`, `mxproc`, `thnwrt` | BRKOXS |
| `dlayxs.rs` | `cdlyxs`, `dldata` | DLAYXS |
| `../deck/ccccr.rs` | the card reads of `ccccr`, `ruinis`, `ruinbr` | the deck driver |

Three things a reader should know:

- **Mixed precision is kept.** `a` is `real(4)` and `e` is `real(8)`. An
  assignment to `a` from a `real(8)` expression rounds to nearest. An expression
  of `a` words alone is single precision: `a(locn)/a(locd)` (nu),
  `a(locsg)/a(locsg0)` (the transport factors), `abs(a(noloc)-1.0)`, and
  `a(mdex)-273.16`, where 273.16 is a default-real literal.
- **Records are gfortran unformatted sequential**, with a 4-byte
  little-endian length at each end. Hollerith words are the 8 bytes of two
  container words, so `ha(i)` is words `2i-1` and `2i`. That is how BRKOXS's
  third record carries 16 bytes of "names" for one isotope (`nwh = mult*nisosh`,
  `:1924`): the name, then the first two floats. Upstream does this, and the port
  reproduces it.
- **Scratch tapes and `skiprz` are positions in memory.** `nscrt1` is a list of
  GENDF records. `tosend`/`tofend`/`tomend` copy records unchanged, as the text
  routines copy lines (`endf.f90:1064-1240`). `brkdat`'s `skiprz(nin,-nrec)` goes
  back to the head of the material it found, which is what both its binary and
  its text record counts do.

## Testing

Gates in `tests/deck_driver_tapes_vs_njoy2016.rs`. NJOY2016.80 runs the
unmodified upstream deck beside the port, and the files are compared **byte for
byte**:

| gate | deck | files | result (2026-11-13) |
|---|---|---|---|
| `test_02_ccccr_whole_deck` | 02, Pu-238 ENDF/B-IV, 50 groups, `maxord = 4`, binary GENDF, BRKOXS at 3 temperatures × 6 σ₀ | ISOTXS 26 (42 128 bytes), BRKOXS 27 (23 164 bytes); text 28, 29 | identical |
| `test_76_ccccr_whole_deck` | 76, U-235 ENDF/B-VIII.0, 69 groups, coded GENDF, 6 delayed families | DLAYXS 30 (3 732 bytes); text 25 | identical |

A prediction was posted on gh:#1369 before the code ran ("byte-identical; a
miss would be one `f32` ulp from a single/double rounding"). All three files were
identical on the first run. `core::tests` pins the equivalence and the record
markers.

## Not translated (refused by name, `NjoyError::NotPorted`)

No upstream test deck runs these:

- ISOTXS `ifopt = 2` (blocking by Legendre order), and `wrtmtx`'s two branches for it.
- ISOTXS `nsblok = ngroup`, whether asked for or forced by `isomtx` when the
  matrix does not fit.
- The ISOTXS chi matrix (`ichix > 1`).
- BRKOXS factors larger than the container (`nsblk = nreact`).
- The listing (`lprint = 1`: `pisotx`, `pbrkxs`, `pdlyxs`) is not written. It
  goes only to the output listing, not to a file.

## Divergences (defined behaviour unless stated)

- `ichix = 1`: upstream reads `spec` list-directed straight into `real(4)`. The
  port reads it as `real(8)` and rounds. A decimal value whose double lies
  exactly at an `f32` rounding midpoint would round differently. No deck reads
  `spec`.
- `prinxs` with MT=455 before any MT=18 (`ichid = 0`) indexes `dnorm(0)`, which
  is **undefined** in Fortran. The port refuses it.
- `dldata` reads the ENDF header variable `mfh` before any read of its own. That
  variable is left by the previous module, but the `tpidio` that precedes it
  always sets `mfh = 0`, so this is defined.
- `listio` on a list longer than `maxe = 8000` words is refused. Upstream checks
  before each `moreio` page (`loc+302 > maxe`), so it can refuse a list a few
  hundred words shorter than the port's limit.
- Upstream bugs reproduced, not fixed:
  - `brkdat` copies the **first** temperature block onto `nscrt1` before it checks
    that the temperature was requested (`:2211-2222`).
  - `ntat` and `ntap` are never reset between isotopes.

## Caveats

- ~~**Lowest priority (Phase 6)** — OUTRAM PARK does not target CCCC-based fast-
  reactor codes; port on demand.~~ Ported on demand for gh:#1369 (upstream decks
  02 and 76).
- Binary word-size/endianness conventions of the CCCC standard must be honoured
  exactly for interchange. The files are little-endian, as gfortran on x86-64
  writes them.

## References

- NJOY2016 manual §CCCCR (LA-UR-17-20093)
- `ccccr.f90` (NJOY2016.80, git commit `ac5adf5`)
- R.D. O'Dell, *Standard Interface Files and Procedures for Reactor Physics
  Codes, Version IV*, LA-6941-MS (1977), cited by `ccccr.f90`
