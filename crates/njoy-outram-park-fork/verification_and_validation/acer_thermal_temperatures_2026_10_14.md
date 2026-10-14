# Thermal ACE tables away from the base temperature: NJOY2016's, word for word (2026-10-14)

GitHub #866, first checkbox. Follows
[`acer_thermal_word_parity_2026_10_14.md`](acer_thermal_word_parity_2026_10_14.md)
(GitHub #865), which gated five tables, all at their evaluation's base
temperature. Gate: `tests/acer_thermal_temperature_parity.rs`.

Research, education and V&V only; not for any operational or safety use.

## Question

Is `AceTable::thermal_from_mf7_on_pendf` NJOY2016's table at a temperature
that is not the thermal evaluation's base temperature `T₀`? And for a
temperature NJOY2016 will not process, does it refuse as NJOY2016 does?

## What upstream does (read before measuring)

NJOY2016 2016.79 (`ac5adf5`), `thermr.f90`:

| step | lines | what it does with the requested temperature `temp` |
|---|---|---|
| main loop | 347-349 | the PENDF must carry `temp` (within `1e-4·temp`); else "desired temperature not on tape" |
| `rdelas` (Bragg `S(E)`) | 526-555 | keeps the **last** tabulated temperature with `abs(tnow-temp) < temp/1000+5`; sets a found flag it never reads, so with no match it silently keeps the base temperature's `S(E)` |
| `calcem` (`S(α,β)`) | 1719-1733 | walks the MF=7/MT=4 temperatures in tape order, stops at the first with `abs(t-temp) <= temp/500`; a `t` above `temp` first, or running out, is "desired temperature not found" |
| `calcem`, a near match | 1734-1745 | prints "difference between temperatures desired and found" and goes on |
| `calcem`, `T_eff` | 1812-1834 | `T_eff` from the MF=7/MT=4 TAB1 by `terpa` at `tempt=temp`, the **requested** temperature |
| `calcem`, `kT` | 1924 | `tev=t*bk`, where `t` is still the **tabulated** temperature the walk stopped at |

So THERMR never interpolates `S(α,β)` or `S(E)` in temperature. Because a
thermal deck always reads MT=4, `calcem` refuses before `rdelas`'s missing
check could matter.

The `calcem` window (`T/500`) is not the `T/1000 + 5` K that this crate's
`select_temperature` uses (that one is `rdelas`'s). At 600 K the first is
1.2 K and the second 5.6 K.

### Prediction, written before the runs

- Tabulated temperatures (graphite 600 K, water 293.6 K): identical, since
  only the selected table changes and the #865 base-temperature cases are
  identical.
- 450 K graphite and 330 K water: NJOY2016 stops in `calcem`. This crate
  builds a table by `LI`-law interpolation.
- 603 K graphite: NJOY2016 stops (3 K > 1.206 K). This crate snaps to 600 K
  (3 K < 5.603 K) and builds a table.
- 600.5 K graphite: NJOY2016 builds a table that is not exactly the 600 K
  one, since `calcem(temp, ...)` is called with 600.5.

The last prediction was **wrong**; see the results.

## Methodology

- **NJOY2016:** 2016.79, `ac5adf5`, built from source on this machine. Decks in
  `reference-data/acer/thermal_decks/`, each the #865 deck with the
  temperature changed on the `broadr`, `thermr` and `acer` cards: `reconr`
  (0.001) → `broadr` → `thermr` (16 cosines, `iform = 0`, `tol = 0.001`,
  `emax = 4 eV`, `iinc = 2`) → `acer` (`iopt = 2`, 16 equiprobable outgoing
  energies, `iwt = 1`), every unit positive. Tape 20 the incident ENDF, tape
  21 the thermal ENDF. Tables gzipped (`-9 -n`) into `reference-data/acer/`
  with SHA-256 in the decks README.
- **This crate:** RECONR + BROADR of the same incident tape to the same
  temperature, `parse_mf7_at_temperature(.., Some(T))` of the thermal tape,
  `AceTable::thermal_from_mf7_on_pendf` with the same `tol`, `natom` and
  `emax`, on `ace_incident_grid(4.0)`.
- **Comparison:** `acer::compare` (header, NXS, JXS, every thermal block and
  the whole XSS, word by word), and the Type-1 text byte for byte except the
  date and comment.
- **Pass criterion:** identical in every compared word and every byte outside
  the date and comment; refused where NJOY2016 refuses. No tolerance.
- **Inputs:** `tsl-crystalline-graphite.endf` (MAT 30, temperatures 296, 400,
  500, 600, 700, 800, 1000, 1200, 1600, 2000 K) on `n-006_C_012-ENDF8.0.endf`
  (MAT 625); `tsl-HinH2O.endf` (MAT 1, 283.6 K base, natom 2) on
  `n-001_H_001-ENDF8.0-Beta6.endf` (MAT 125). All three MT=4 sections are
  `LAT = 1`.

## Results

### NJOY2016

| deck | NJOY2016 (exit status) | output |
|---|---|---|
| graphite 600 K | 0 | 594 506-byte table |
| graphite 600.5 K | 0 | table; "difference between temperatures desired and found is 5.00E-01" |
| water 293.6 K | 0 | 589 019-byte table |
| water 293.9 K | 0 | table; "difference ... 3.00E-01" |
| graphite 450 K | 77 | `***error in calcem***desired temperature not found.` |
| graphite 603 K | 77 | the same |
| water 330 K | 77 | the same |

**NJOY2016's 600.5 K table differs from its 600 K table in line 1 only** (kT
`5.1747E-08` against `5.1704E-08`), and its 293.9 K table from its 293.6 K
table in line 1 only. Every data word is the tabulated temperature's. That is
what the last row of the upstream table predicts once read properly: `kT`
inside `calcem` is the tabulated temperature's, and `T_eff` changes nothing
that prints below 4 eV in these two cases.

### This crate, before the change

The three tabulated-temperature tables (600 K, 600.5 K, 293.6 K) were already
identical in every word. **Graphite at 450 K was built**, not refused (the
refusal test failed there). By `select_temperature`'s rule it would also have
interpolated 330 K and snapped 603 K to 600 K (read, not run).

A second trap, in the same function: `thermal_from_mf7_on_pendf` never checked
that the `Mf7` it was given had been parsed at `temp_k`. `parse_mf7` (base
`T₀`) with `temp_k = 600` built a table of 296 K's `S(α,β)` and 600 K's Bragg
data, labelled 600 K.

### This crate, after the change

| case | result |
|---|---|
| graphite 600 K | identical in every word (ITIE 107, ITIX 106, ITXE 28 832, ITCE 136, ITCX 135; XSS 29 316 words) and every byte but the date and comment |
| graphite 600.5 K | identical, likewise, kT `5.1747E-08` included |
| water 293.6 K | identical in every word and every byte but the date and comment |
| water 293.9 K | identical, likewise |
| graphite 450 K, graphite 603 K, water 330 K | refused, `NjoyError::TemperatureNotTabulated` |
| `parse_mf7` (296 K) handed in with a 600 K request | refused, `NjoyError::ThermalTemperatureMismatch` (`s_alpha_beta_at_another_temperature_is_refused`) |

The five #865 base-temperature cases are unchanged (`tests/acer_thermal_word_parity.rs`,
5 passed). The gate file runs 7 tests in 27 s.

Suites run 2026-10-14 on this change: `cargo test --release -p
njoy-outram-park-fork --lib --tests` (1 109 passed, 9 ignored, and 1 failed
that is fixed below, re-run passing) and `scripts/njoy-ace-leapr-tests.sh`
(31 test files, 112 passed, 0 failed, 1 ignored, none skipped). The two
failures the crate suite found (`thermal_ace.rs` on a first, fail-fast run,
`thermal_ace_zrh.rs` on the second), were not
from this change: since #865 the ZAID is NJOY's right-justified `a6`
(`"  al27.00t"`), and both asserted `zaid.starts_with(name)`. They now trim the
field's padding first.

## What was changed, each with its upstream line

1. **`calcem`'s temperature walk** (`thermr.f90:1719-1733`):
   `thermr::mf7::calcem_temperature_k`. The NJOY-deck path calls it and returns
   `NjoyError::TemperatureNotTabulated` where `calcem` stops. Unit test
   `calcem_temperature_walk` (graphite's ten temperatures: 600, 600.5, 599
   accepted; 450, 603, 250, 2100 refused).
2. **The `Mf7` must be the one `calcem` reads.** If the S(α,β) handed in was
   interpolated, or resolved at another temperature than the one `calcem`
   selects, the path returns `NjoyError::ThermalTemperatureMismatch` instead
   of building a mislabelled table.
3. **`T_eff` at the requested temperature, `kT` at the tabulated one**
   (`thermr.f90:1812-1834` against `:1924`):
   `IncoherentInelastic::teff_request_k`, read by the short-collision-time
   kernel. It moves no printed word in the four cases (as NJOY2016's own
   600/600.5 K and 293.6/293.9 K tables show), so
   `effective_temperature_is_read_at_the_request` pins the wiring directly: far
   outside graphite's α grid the kernel moves by **+8.95e-3** relative when
   `T_eff` is read at 600.5 K instead of 600 K, and not at all when the request
   equals the tabulated temperature.
4. **`T_eff` interpolation uses `terp1`'s grouping** (`endf.f90:1431`, through
   `terpa`): `y0 + (x-x0)*(y1-y0)/(x1-x0)`. Identical at a tabulated
   temperature.
5. **`rdelas`'s Bragg table** (`thermr.f90:526-555`):
   `CoherentElastic::s_of_e_rdelas`, the last match within `T/1000 + 5` K and
   otherwise the base table, never interpolated. Used by the NJOY-deck path
   for both THERMR's MF=3 grid and ITCE/ITCX. On these two evaluations it
   selects the same table as the nearest-match rule; the unit test
   `rdelas_keeps_the_last_match_and_falls_back_to_base` uses synthetic
   temperatures 4 K apart to tell the two rules apart.

### A hypothesis killed by the gate

The first version of change 3 ran all of `calcem` at the requested
temperature, reading `calcem(temp, ...)` at `thermr.f90:424` as "`kT` is the
request". The 600.5 K table then differed from NJOY2016's at ITIX(1): 10.4875
b against 10.49086 b. Reading on showed `tev=t*bk` with `t` the tabulated
temperature (`:1924`). Only `T_eff` follows the request. The gate caught a
misreading of upstream that a 1e-3 comparison would have accepted (3e-4).

## Scope of the refusal

Only `thermal_from_mf7_on_pendf` (the NJOY-deck path) refuses.
`thermal_from_mf7` and `thermal_from_mf7_with_tolerance` keep this crate's
documented generalisation (the `LI`-law interpolation in the `thermr::mf7`
module docs), which NJOY2016 does not have and which this record does not
verify.

## What is NOT covered

- Temperatures other than `T₀` for Al-27, or for incoherent-elastic and mixed
  evaluations (H in ZrH).
- A case where `T_eff` at the request moves a printed word; none of the four
  does.
- A tape with two MF=7 temperatures inside one `T/500` window, where `calcem`
  takes the first and the nearest-match parse the other: the path refuses that
  with `ThermalTemperatureMismatch` rather than reading the right one.
- Temperatures above THERMR's `break` (3000 K), where `calcem` rescales its
  incident grid; no evaluation here goes that high.
- Multi-temperature decks (`ntemp > 1`).

## Reproducing

```bash
cargo test --release -p njoy-outram-park-fork --test acer_thermal_temperature_parity -- --nocapture
# a reference table, or a refusal: NJOY2016 on a committed deck
cd <scratch> && ln -s <repo>/reference-data/endf/n-006_C_012-ENDF8.0.endf tape20 \
  && ln -s <repo>/reference-data/endf/tsl-crystalline-graphite.endf tape21 \
  && njoy < <repo>/reference-data/acer/thermal_decks/graphite_600k_tol0p001.njoy
```
