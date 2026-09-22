# ACER dosimetry (`iopt = 3`) vs NJOY2016

**Date:** 2026-09-21 · **Port:** `src/acer/dosimetry.rs` (`acedos` and
`dosout` from `acedo.f90`) · **Oracle:** NJOY2016 2016.79, built `-O3`.

## What was compared

Both codes read the **same PENDF** and write a Type-1 ACE file. Feeding one
tape to both isolates `acedos`: RECONR and BROADR are outside the comparison,
so a disagreement can only come from the dosimetry path itself.

```text
reconr / 20 21 / '<nuclide> pendf' / <mat> 0 / .001 / 0
broadr / 20 21 22 / <mat> 1 / .001 1.e6 / 293.6 / 0
acer   / 20 22 0 24 25 / 3 1 1 .00 / '<comment>' / <mat> 293.6
```

```bash
cargo run --release -p njoy-outram-park-fork \
  --example dosimetry_ace_vs_njoy2016 -- <pendf> <mat> 293.6 <njoy.ace> '<comment>' 'mm/dd/yy'
```

## Results

| case | reactions | XSS words | bytes | result |
|---|---|---|---|---|
| H-1 ENDF/B-VIII.0-β6, MAT 125, 293.6 K | 2 | 2 532 | 52 130 | **every byte identical** |
| Mn-55 ENDF/B-VIII.0, MAT 2525, 293.6 K | 119 | 70 440 | 1 427 267 | **every byte identical** |

Worst word-level relative difference before the text rounding: **3.3e-16**
(H-1 2.4e-16), i.e. the tables agree in double precision, not merely at the
file's 12 printed digits.

NXS and JXS agree exactly: `(len2, ZA, 0, ntr)` and
`lone = 1, mtr = 1, lsig = 1 + ntr, sigd = 1 + 2·ntr, end = len2`.

Both references and the PENDFs they came from are committed under
`reference-data/acer/`; the gate is `tests/acer_dosimetry_vs_njoy2016.rs`.
The four files total ~3 MB, which is the price of a byte-equality gate over
119 reactions — drop the Mn-55 pair if that is too much and the H-1 case still
covers everything except MF=10.

## What Mn-55 buys that H-1 does not

Its PENDF carries **MF=10**, so the run exercises the isomeric-production
branch that H-1 never reaches:

- the synthetic reaction number `MT + 1000·(10 + LFS)` (`acedo.f90:198`) —
  present as 10030, 12030, 10037, 11037 in the table, and asserted by name;
- the `ns = max(1, N1)` subsection loop (`:165`);
- **the interleaved region convention.** MF=3 stores a non-default
  interpolation table as all the `NBT` values followed by all the `INT` values
  (`:150-153`); MF=10 stores it as `(NBT, INT)` pairs (`:220-223`). Both are
  `2·NR` words and `dosout` writes them identically, so nothing downstream can
  tell which convention a given reaction used. This port reproduces both
  rather than picking one.

## NJOY does not test this path itself

All 40 cases in `upstream_source/NJOY2016/tests` were checked: `acer` appears
with `iopt` 1, 2 and 7 only. **No upstream test runs `iopt = 3`.** The gate
here therefore covers a path that has no regression cover upstream, which is
worth knowing before trusting either code's dosimetry output.

## One deliberate divergence: 0 K is refused

`acedos`'s temperature search is

```fortran
temp=0
do while (abs(temp-tempd).ge.tempd/100+1)
   call contio(...)   ! <- reads ZA and AWR
   ...
enddo
```

so the loop runs only while the temperature does **not** match. With
`tempd = 0` the condition is `0 >= 1`, false on entry, and the body never
executes — `za` and `awr` are never assigned. Verified by running it:
NJOY2016 asked for a 0 K Al-27 dosimetry table produced a 269 494-byte file
whose header reads

```text
     0.00y    0.000000  0.0000E+00   09/21/26
```

— ZAID `0.00y`, `AWR = 0`, and `NXS(2) = 0`. The same happens for any
`tempd < 1.0102 K`, because the tolerance `tempd/100 + 1` exceeds `tempd`
there.

**This port returns an error naming the temperature instead.** That is the one
place it does not reproduce upstream byte for byte, and it is deliberate:
writing a table with no ZAID is not a behaviour worth being compatible with,
and a caller who gets the error can ask for the PENDF's own temperature. The
refusal is pinned by `zero_kelvin_is_refused_rather_than_written_with_no_zaid`
so it cannot be quietly removed.

## Not exercised

- **A stored interpolation table (`NR != 0`).** A PENDF is linearised, so every
  reaction reaching `acedos` has `NR = 1, INT = 2`, which upstream collapses to
  a single `0` (`:155`). The `NR != 0` branch is translated and untested; a
  dosimetry *sublibrary* tape (IRDFF-II and similar), which is what upstream
  intends here, would exercise it, and none is held in
  `reference-data/endf/`.
- **Type 2 (binary) output.** `dosout`'s Type-2 branch uses `ner = 1`
  (`acedo.f90:495`) like every non-fast class and is served by the shared
  writer; no reference file was produced for it.
- **The mcnpx variant** (13-character ZAID, `f10.3,'ny '`) is implemented and
  unexercised.
- **Nothing here is validation.** It says this port writes what NJOY2016
  writes.
