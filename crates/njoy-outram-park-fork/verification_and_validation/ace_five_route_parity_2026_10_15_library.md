# The LCT-008 library itself against NJOY2016, bit for bit (2026-10-15)

**Research, education and V&V only** (`RESPONSIBLE_USE.md`).

Companion to `ace_five_route_parity_2026_10_14.md`, whose "Unrounded"
section compares `process::ace` **without HEATR** (the five-route NJOY2016
deck has none). This record checks the files the LCT-008 runs actually read.

## Question

Routes 2 and 5 read the Rust ACE library that
`crates/outram-mc-libs/verification_and_validation/icsbep/five_route_keff/run_all.sh`
builds with `examples/write_ace_library.rs`, one `--nuclide` per row of
`nuclides.tsv` (the 36 tapes of `lct008_model::TAPES`, plus HST-009's F-19
and U-236) and `--thermal-leapr` for H in H₂O. That writer runs **HEATR**
(the crate's default, correct physics on), so its heating numbers are not
zero. Is each of its tables NJOY2016's, bit for bit, on the same deck?

## Methodology

- **Ours:** `write_ace_library --out … --temp 293.6 --nuclide NAME:TAPE:MAT`
  per nuclide, exactly as `run_all.sh` calls it, with
  `OUTRAM_ACE_TYPE2_ALSO=1` so each table is also written as Type 2 (the
  unrounded doubles).
- **Reference:** NJOY2016 2016.79 (`ac5adf5`, `build2`) on the five-route
  decks with HEATR inserted, every unit positive (text tapes), ACER `itype 2`:
  - 293.6 K: `reconr .001 -> broadr 293.6 K -> heatr (20 22 26 / MAT 0 0 0 0 0 /) -> purr 20/64 -> acer`;
  - 0 K: `reconr .001 -> heatr (20 21 26 / MAT 0 0 0 0 0 /) -> acer`.
- **Comparison:** NXS, JXS and every XSS word by bits; a differing word is
  named by its JXS block (ESZ by column).
- **Thermal:** NJOY2016 writes thermal tables only as Type 1
  (`aceth.f90:541`), so ours (`--thermal-leapr`) is compared with route 1's
  NJOY2016 table (OpenMC `make_ace_thermal`, `iwt = 1`) byte for byte.
- **Pass:** zero differing bits in every table; identical thermal data lines.

## What differed, and the fixes

With HEATR on both sides, the first survey found differences only in the
heating: the ESZ heating column (one unit in the 7th figure at isolated
energies; clusters off by up to 25 % on dense 0 K resonance grids, e.g.
Ti-48 near 155 keV) and PURR's heating column (Zn-70: ours 0, NJOY 1).
Each was localised with an instrumented NJOY2016 build (prints added to
`acelod`, `gety1`, `nheat`, `gheat`).

| # | symptom | cause (upstream) | fix |
|---|---|---|---|
| 22 | Zn-70 293.6 K LUNR heating 0 against 1 | PURR's `rdheat` finds HEATR's MT=301 on the PENDF (`ihave = 1`) and writes `1` for LSSF = 1, else `heat/sigu(1,1,1)` (`purr.f90:526-532`, `:1237-1289`). Ours always wrote the no-HEATR column | the deck runs HEATR before PURR (`deck_modules`); `UrrProbabilityTables::from_endf_with_mt301` |
| 23 | B-10: 1 word one unit off at each temperature (and part of Zn-70's 32 and 54) | `acelod` divides by the total **as it stands at MT=301** in its MF=3 walk, before the final 9-figure pass (`acefc.f90:5641-5645`, `:6315`; NJOY's `xss(it+j)` printed 1438.40835365984390 against the stored 1438.40835) | `total_at_301` |
| 24 | Ti-48 0 K: 217 words, up to 25 % | ACER reads HEATR's tape as text; we read its in-memory rows, whose energies sat a few ulps off their printed values, so `gety1` interpolated from the neighbouring node | `heatr_kerma` reads MT=301 back through `Tape::write` |
| 25 | Ti-48: 2 words one unit off | `unionx` carries MT=301 onto the union grid (first point at `E*(1+eps)`) and writes it to a scratch tape that is text when the PENDF is (`:1110`, `:1780-1800`): 9005.658499863 becomes 9005.6585, which `sigfig` takes to 9005.659 | `a11` round trip of the union-grid value |
| 26 | Ti-48: MT=301 one unit off at 3.82 and 5.70 MeV | HEATR's `hconvr` writes its converted MF=12 LO=2 yields to `nscr`/`nend4`, text when card 1's ENDF unit is positive (`heatr.f90:289-292`): MT=55's yield 1.7845837955e-3 is 1.784584e-3 there | `HeatrInput::units.nendf > 0` sets `text_scratch`; `hinit` reads `hconvr`'s output back through text. The committed HEATR parity decks use `nendf = -21` (binary) and are unchanged |

And one cross-platform fix found by CI, not by this survey:

| 27 | Windows only: 27 Zn-70 words differ in the last bit (Linux, macOS identical) | `sigfig`'s `ten**(-ipwr)` used `f64::powi`, which each target lowers itself; gfortran calls libgcc's `__powidf2` | `petir::real::powi` (square-and-multiply, one reciprocal) and `petir::real::log10` in `sigfig` |

## Results (2026-10-15)

- **All 76 library tables are bit-identical** to NJOY2016 with HEATR:
  101 197 626 XSS words, 0 differing, NXS and JXS equal on every table
  (38 nuclides × 293.6 K and 0 K). U-235 and U-238 at 0 K were re-run on
  their own after the first NJOY2016 runs died for lack of disk; the
  re-runs took ours 248 s and 194 s (both temperatures) and NJOY2016 360 s
  and 202 s (both decks in parallel).
- **Thermal (H in H₂O, LEAPR):** identical in every byte of its 32 166 data
  lines.
- **Gate:** `tests/ace_five_route_parity.rs` gains
  `ti48_heating_number_with_heatr_is_njoy2016s` (0 K) and
  `zn70_purr_heating_column_with_heatr_is_njoy2016s` (293.6 K), against
  NJOY2016 Type-2 tables made with the HEATR deck
  (`reference-data/acer/five_route/*_heatr_njoy2016.ace.gz`); 13 passed.
  `tests/heatr_driver_vs_njoy2016.rs` (binary-unit decks): 8 passed,
  unchanged.
- **Hardware:** Intel Xeon @ 2.80 GHz, 4 logical cores, 15 GiB RAM, Linux
  6.18, CPU only, cloud container; two nuclides at a time.

## Scope

- The heating number and PURR's heating column are not read by a
  k-eigenvalue calculation, so no LCT-008 `k` moves. They matter for
  heating tallies.
- HEATR's own byte parity (62 tapes) was measured with binary scratch units;
  the text-scratch path is now also gated (Ti-48 above) but only through the
  ACE table, not a HEATR tape gate.
