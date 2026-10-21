# ACER photo-atomic (`iopt = 4`) vs NJOY2016

**Date:** 2026-09-21 · **Port:** `src/acer/photoatomic/` (`acepho`, `iheat`,
`alax`, `phoout` from `acepa.f90`) · **Oracle:** NJOY2016 2016.79, built
`-O3`, from `crates/njoy-outram-park-fork/upstream_source/NJOY2016`.

## What was compared

Both codes read the **same** ENDF photo-atomic evaluation and write an ACE
table. The comparison is on the *written file*, not on an intermediate:

| | tape | MAT | `Z` | `nes` | `nflo` | XSS words |
|---|---|---|---|---|---|---|
| synthetic | `reference-data/endf/photoat-synthetic-Z6.endf` | 600 | 6 | 53 | 0 | 449 |
| real | `reference-data/endf/photoat-092_U_000-ENDF8.0.endf` | 9200 | 92 | 11 942 | 6 | 71 807 |

NJOY input (both tapes, both containers):

```text
acer
20 0 0 24 25
4 1 1 .00/        $ iopt=4 photo-atomic, iprint=1, itype=1 (or 2)
'<comment>'/
<mat> 0./
stop
```

Reproduce this port's side with

```bash
cargo run --release -p njoy-outram-park-fork \
  --example photoatomic_ace_vs_njoy2016 -- <tape.endf> <mat> <njoy.ace> '<comment>' 'mm/dd/yy'
```

Committed references: `reference-data/acer/z6_photoatomic_njoy2016.ace`
(Type 1) and `…type2.ace` (Type 2). The gate is
`tests/acer_photoatomic_vs_njoy2016.rs`.

## Results

### Synthetic Z = 6 — the written file is byte-identical

| container | bytes | result |
|---|---|---|
| Type 1 (ASCII) | 9 950 | **every byte identical** |
| Type 2 (binary) | 7 692 | header record identical; **93 of 449** data words differ, worst **3 ulps / 6.7e-16** |

Byte equality of the Type-1 file is the strongest statement available, and it
is the right one: a photo-atomic table is written entirely as `1pE20.11`, so
byte equality means all 449 words agree to 12 significant figures *and* pins
the header formatting, NXS/JXS, the natural-log convention on ESZG and the
line breaking.

### U (Z = 92), ENDF/B-VIII.0 — 71 781 of 71 807 words at print precision

NXS and JXS identical (`len2 = 71807`, `Z = 92`, `nes = 11942`, `nflo = 6`;
`jxs = [1, 59711, 59732, 59842, 59866]`). Worst relative difference per block:

| block | words | worst relative |
|---|---|---|
| ESZG log E | 11 942 | 5.0e-12 |
| ESZG log incoherent | 11 942 | 4.8e-12 |
| ESZG log coherent | 11 942 | 4.8e-12 |
| ESZG log photoelectric | 11 942 | 4.9e-12 |
| ESZG log pair | 11 942 | 4.7e-12 |
| JINC `S(v, Z)` | 21 | **0** |
| JCOH sampling integral | 55 | 4.0e-12 |
| JCOH `F(v, Z)` | 55 | 3.1e-12 |
| JFLO fluorescence | 24 | **0** (all zeros both sides — no relaxation tape) |
| LHNM heating | 11 942 | **8.7e-8** |

A Type-1 word is printed as `1pE20.11`, i.e. 12 significant figures, so
**~1e-12 is the file's own resolution**. Everything except the heating column
sits at it. Word-level histogram over all 71 807 words:

| agreement | words |
|---|---|
| exact | 7 534 |
| < 1e-12 | 44 242 |
| < 1e-11 | 19 999 |
| < 1e-10 | 6 |
| < 1e-9 | 2 |
| < 1e-8 | 19 |
| < 1e-7 | 5 |

The 26 words outside print precision are **all in the heating block** and all
above 14 MeV; the worst two are at 1 472.7 MeV (8.7e-8) and 3 409.2 MeV
(8.4e-8).

## The heating residual: upstream's own quadrature, not a translation defect

The heating column is `(iheat_term + E·σ_pe + σ_pair·(E − 1.022)) / σ_tot`.
Attributing word 10 423 term by term (`OUTRAM_PARK_PHOTOATOMIC_DEBUG=10423`)
puts **all** of the disagreement in `iheat`'s incoherent term, which differs
by 6.8e-5 there while being only 1.3e-3 of the heating — pair production
carries the rest, which is why a 7e-5 error in `iheat` shows up as 9e-8 in the
column.

Four hypotheses were tested. Three are dead:

1. **Round-off amplification in the attribution.** The five ESZG columns agree
   to ≤ 4.8e-11 at that index and the amplification factor is 777, so
   reconstruction noise can account for ~3e-9 — four orders short.
2. **Conditioning.** Perturbing `E` by one ulp moves `heat` by **1.4e-16**
   (`--example photoatomic_iheat_conditioning`). The routine is not
   amplifying round-off.
3. **The `1 − unow` cancellation.** `unow = 1 + 1/enow − 1/pnow` and the
   kernel needs `1 − unow`, a cancelling difference of two reciprocals that are
   both ~3.5e-4 at 1.5 GeV. Forming `(1/pnow − 1/enow)` directly instead
   (`iheat_ablate_cancellation`) changes the answer by **0.0 to 2e-16** at
   every energy tried. Not the cause.
4. **Upstream's panel rule is not converged above ~300 MeV.** This is the
   cause. `iheat_refined` tightens the `pnow/2` panel limit to
   `pnow·(1 − 1/128)` and re-measures:

   | E (MeV) | panels, `split = 2` → `128` | truncation error |
   |---|---|---|
   | 0.1 | 88 → 114 | 2.9e-12 |
   | 10 | 78 → 529 | 1.5e-12 |
   | 100 | 39 → 785 | 9.8e-13 |
   | 322 | 20 → 918 | 8.0e-13 |
   | **1 000** | **19 → 1 061** | **4.0e-5** |
   | **1 473** | **19 → 1 106** | **4.9e-5** |
   | **3 409** | **18 → 1 215** | **1.2e-4** |
   | **10 000** | **16 → 1 349** | **9.9e-4** |

   The onset energy and the magnitude both match the disagreement with NJOY.
   Below ~300 MeV the 10-point Lobatto rule over `pnow/2` panels is converged
   to 1e-12 and the two codes agree to 1e-12; above it the rule carries
   5e-5 to 1e-3 of its own truncation, and at that point *which* of two
   arithmetically equivalent implementations one runs decides the last five
   digits.

**This is reported, not fixed.** `iheat` keeps upstream's panel rule, because
a port that silently improves the numerics stops being a port and the
comparison against NJOY stops meaning anything (root `CLAUDE.md`, "Debugging a
port: read upstream first"). `iheat_refined` exists as a control so the
truncation can be re-measured, not as the default. Anyone who needs photon
heating above ~300 MeV to better than 1e-4 should know that **neither** code
supplies it.

Physically this changes nothing for a reactor: above 100 MeV the incoherent
term is under 1 % of photon heating and pair production carries the rest, and
no application in this workspace transports photons above 20 MeV.

## The Type-2 1-ulp residual

The Type-2 container stores full doubles, so it is the only comparison here
that can see a single bit. On the Z = 6 table 93 of 449 words differ, worst 3
ulps (6.7e-16). `--example photoatomic_grid_ulp_probe` splits them: **12 follow
a one-ulp nudge of the grid energy** (so they trace to `sigfig`) and **19 do
not** (so they trace to `terp1`), the rest being downstream of those.

Four candidate causes were tried and **none removes it**:

| candidate | differing ESZG words |
|---|---|
| upstream's grouping `y1 + (x−x1)(y2−y1)/(x2−x1)` (kept) | 31 |
| `y1 + (x−x1)·((y2−y1)/(x2−x1))` | 32 |
| `y1 + ((x−x1)/(x2−x1))·(y2−y1)` | 31 |
| FMA-contracted `terp1` | 32 |
| FMA-contracted `sigfig` (`x·10^ipwr + 10^(ndig−11)`) | 31 |

The remaining suspect is `10**ipwr` — gfortran's `__builtin_powi` against
Rust's `f64::powi` — which is a compiler artefact rather than a port question.
It is left **open and recorded** rather than chased: it is 2e-16, it is below
anything the Type-1 container can express (which is why that one is
byte-exact), and `acer` writes Type 1 by default.

## Two upstream behaviours a port must reproduce, not "fix"

1. **Type-2 photo-atomic ESZG is *linear*; Type-1 is *logs*.** `phoout` applies
   `log` inside its `itype == 1` branch only (`acepa.f90:966-973`); the Type-2
   branch (`:997-1010`) writes `xss` verbatim. Measured: NJOY's own Type-2
   Z = 6 file begins its data at `1.0000000000001e-3` (MeV) where the Type-1
   file has `-6.90775527898`. `PhotoatomicAce::into_raw` therefore takes the
   target container.
2. **`ner = 1` for every class except the fast path.** Thermal
   (`aceth.f90:571`), photo-atomic (`acepa.f90:297`), dosimetry
   (`acedo.f90:319`) and photonuclear (`acepn.f90:1877`) all block the Type-2
   XSS **one real per record**; only `acefc.f90:187` uses 512. Measured on
   NJOY's Z = 6 Type-2 output: a 500-byte header record then 449 records of 8
   bytes, 7 692 bytes total. This corrects the Type-2 writer committed in
   `42c361576`, which used 512 for every class — see
   `ace_reader_vs_njoy2016.md`.

## Not exercised

- **Fluorescence (`alax`, the JFLO block).** `reference-data/endf/` holds no
  atomic-relaxation sublibrary tape, so both photo-atomic tapes take upstream's
  `nlax = 0` path and NJOY itself writes JFLO as 24 zeros. The port is a
  faithful translation **verified by reading the Fortran, not by running it**;
  `src/acer/photoatomic/fluorescence.rs` says so at its head. Only the `nflo`
  table (`nflo_for`) is gated, by a unit test.
- ~~**The mcnpx variant** (13-character ZAID, `f10.3,'pp '`) is implemented and
  unexercised — no reference file.~~ **CORRECTED 2026-09-22** — a reference
  was produced (`acer` with `iopt = -4`) and the output is **byte-identical**
  to it, all 9 953 bytes. Gate:
  `mcnpx_type1_output_is_byte_identical_to_njoy2016`.
- **ENDF-4/5 tapes**, where photoelectric absorption is MT=602 rather than
  MT=522. The branch exists (`endf_version`); no such tape is held here.
