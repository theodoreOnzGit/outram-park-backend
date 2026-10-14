# Incoherent-elastic thermal ACE tables vs NJOY2016, word for word (2026-10-14)

**Class:** verification, code to code against upstream. Not validation: no
experiment is compared against. AI-assisted draft, not yet reviewed by a human.
**Issue:** GitHub #866 (second checkbox). **Gate:**
`tests/acer_thermal_word_parity.rs`,
`h_in_zrh_incoherent_elastic_at_296k_is_njoy2016s_table` (31 s in release on
2 cores). **Reference:** NJOY2016 2016.79 (`ac5adf5`), built and run on this
machine. Companion record for the coherent and no-elastic cases:
[`acer_thermal_word_parity_2026_10_14.md`](acer_thermal_word_parity_2026_10_14.md).

## Result

**H in ZrH at 296 K (ENDF/B-VIII.0, `IDPNC = 3`) is NJOY2016's table in every
compared word, and in every byte of its Type-1 text but the date and the
comment.** `acer::compare` reports, with no tolerance:

| block | words | result |
|---|---|---|
| header (ZAID, AWR, kT, material, IZ/AW), NXS, JXS | | same |
| ITIE | 107 | same |
| ITIX | 106 | same |
| ITXE | 28 832 | same |
| ITCE (incoherent-elastic energies) | 107 | same |
| ITCX (incoherent-elastic cross section) | 106 | same |
| ITCA (equally probable cosines, 106 x 16) | 1 696 | same |
| ITCEI / ITCXI / ITCAI | | absent in both |
| whole XSS | 30 954 | same |

**Before this change** (the same table built from `thermr::incoherent_elastic`'s
analytic values on the ACE energy grid, measured with a probe that was not
committed): ITCE was already identical; **ITCX differed in all 106 words**,
worst 2.24e-3 relative at 3.75 eV (0.643966 b against NJOY's 0.645412 b);
**ITCA differed in all 1 696 words**, worst 5.0e-8 absolute. The inelastic
blocks were already identical, because the coherent/no-elastic work had made
them so.

**Mixed elastic (`IDPNC = 5`) is not covered.** No evaluation in
`reference-data/endf/` has `LTHR = 3` (checked: the MF=7/MT=2 HEAD of all nine
`tsl-*` tapes; Al-27, both SiC tapes and the four graphite tapes are
`LTHR = 1`, H in ZrH is `LTHR = 2`, H in H₂O has no MT=2). NJOY2016's LEAPR
does not write `LTHR = 3` either, so this crate's synthetic-tape practice
(tapes made by a committed generator and fed to both codes) has no upstream
generator to follow here. None was synthesised. The writer's `IDPNC = 5` path
now uses the same `iel` port on `coh`'s merged grid, as `thermr.f90:430-435`
orders it, but nothing compares that path against NJOY.

## Methodology

The NJOY2016 deck is committed as
`reference-data/acer/thermal_decks/hzrh_296k_tol0p001.njoy`: `reconr` (0.001)
of H-1 (`n-001_H_001-ENDF8.0-Beta6.endf`, MAT 125) → `broadr` to 296 K →
`thermr` on `tsl-HinZrH-ENDF8.0.endf` (MAT 7; `nbin = 16`, `iin = 2`,
`icoh = 1`, so THERMR reads `LTHR = 2` from MF=7/MT=2 and calls
`iel(20, ...)`; `iform = 0`, `natom = 1`, `mtref = 222`, `tol = 0.001`,
`emax = 4 eV`) → `acer` (`iopt = 2`, card 8a `1001`, card 9
`222 16 223 1 1 4.0 1`: `mte = 223`, `ielas = 1` incoherent, `nmix = 1`,
`iwt = 1`). Every unit is positive (text), as OpenMC's `make_ace_thermal` writes
its decks. `natom = 1` is the tape's own `M0` (MF=7/MT=4 `B(6) = 1`).

This crate runs RECONR + BROADR on the same H-1 tape, `parse_mf7` on the same
thermal tape, and `AceTable::thermal_from_mf7_on_pendf` with the same `tol`,
`natom`, `emax` and dimensions. The test asserts NJOY's Type-1 text line by
line from line 3 on (and the ZAID/AWR/kT and material fields of lines 1-2),
then `acer::compare` over every block.

The reference table is `reference-data/acer/hzrh_296k_thermal_tol0p001_njoy2016.ace.gz`
(`gzip -9 -n` of `tape30`, 227 251 bytes, SHA-256 in
`reference-data/acer/thermal_decks/README.md`).

## What upstream does, and what the port did not

Read before writing anything (`thermr.f90` `iel` 1244-1425, `rdelas`
477-592, `tpend` 2998-3385; `aceth.f90` 147-153 and 229-279, `thrlod`
739-810):

1. **`iel` computes on `calcem`'s grid, not the ACE grid.** For every `esi`
   (all `calcem` energies, including the first above `emax`) it writes one
   MF=6 LIST holding `E` and `nbin` cosines, and keeps `xie = c1*rc2*(1-x1)`
   (`:1377-1404`).
2. **The MF=3 section for `mtref+1` lives on THERMR's PENDF grid**: the
   incident nuclide's elastic grid (after `coh` where there is one), with
   `terp(esi, xie, nne, E, 3)` at each point, **order 3**, not the inelastic
   column's 5, and the last point zero (`:1407-1418`). `tpend` then rounds it
   with `sigfig(·,7)` (`:3217-3221`).
3. **ACER takes the cross section from that MF=3, not from `iel`.**
   `aceth.f90:240-273` walks the MF=6 energies as read from the text, stops at
   the first above `emax` or within `eps = 1e-5` of it, and calls `gety1` on
   MF=3 `mtei` at each: a linear interpolation between 7-figure points on H-1's
   elastic grid (366 points in all, including the two past `emax`). That is the 2.24e-3 at 3.75 eV, where
   `σ ∝ 1/E`-like curvature meets a sparse grid.
4. **The cosines are stored as written**, through the ENDF text (`tpend`'s
   `ltt = 6` branch copies the LIST and only resets its eighth word,
   `:3293-3301`). No `sigfig`, and no clamp.
5. **`dwa` is `terpa` on the `W'(T)` TAB1 under its own law**, after moving an
   end temperature onto `T` when `T` is within 10 % beyond it, with a refusal
   outside (`:1298-1318`).

## Defects found and fixed

| # | defect | upstream line | effect before |
|---|---|---|---|
| 1 | ITCX was `iel`'s own σ at the ACE energy, not `gety1` on THERMR's 7-figure MF=3 on the incident grid | `aceth.f90:259`, `thermr.f90:1407-1418`, `:3217-3221` | ITCX all 106 words, worst 2.24e-3 |
| 2 | ITCA cosines were not passed through the ENDF text | `thermr.f90:1391-1398`, `:3293-3301` | ITCA all 1 696 words, worst 5.0e-8 abs |
| 3 | σ evaluated as `c1*(1-x1)/c2`; upstream `c1*rc2*(1-x1)` | `thermr.f90:1384` | last-bit differences in `xie` (absorbed here by `sigfig(·,7)` in this case; not separately measured) |
| 4 | cosines clamped to [-1, 1]; upstream does not clamp | `thermr.f90:1393-1398` | none on this tape (NJOY's cosines span -0.9374898 to 0.99949849); a divergence in principle |
| 5 | `W'(T)` interpolated lin-lin and clamped anywhere outside the table; upstream follows the TAB1 law, holds the end value only within 10 %, and refuses beyond | `thermr.f90:1298-1318` | none at 296 K (a tabulated temperature) |

Defects 3-5 are fixed in new functions (`IncoherentElastic::iel_record`,
`IncoherentElastic::iel_dwa`); the analytic `cross_section`,
`equiprobable_cosines` and `debye_waller` are kept for their other callers
(`thermr::scattering`, the no-PENDF ACE path) and unit-tested against the new
ones (`iel_record_agrees_with_the_analytic_helpers_to_rounding`: σ to 1e-14
relative, cosines to 1e-9).

## Code

- `thermr::mf3_grid::thermr_grid` (steps 1-3 of the old `inelastic_mf3`,
  now split out), `tpend_mf3` (step 4), `incoherent_elastic_mf3` (`iel`'s
  column on that grid). `inelastic_mf3` is unchanged in behaviour: it is
  `tpend_mf3(thermr_grid(..))`, and the five existing parity cases still pass.
- `acer::thermal::incoherent_elastic_ace` (private): ACER's read-back loop.
- `AceTable::thermal_from_mf7_on_pendf` uses them when the evaluation has
  incoherent elastic; `thermal_from_mf7` / `thermal_from_mf7_with_tolerance`
  (no incident PENDF) keep the analytic values on the caller's grid.

## What is NOT covered

- Mixed elastic (`IDPNC = 5`): no `LTHR = 3` evaluation here (above).
- Temperatures other than 296 K, the evaluation's base temperature (another
  piece of GitHub #866).
- `nmix > 1`: ACER divides ITCX by `nmix` (`aceth.f90:804`); this port has no
  `nmix` option and writes `nmix = 1`. Separately, the coherent block divides
  by `natom` where upstream divides by `nmix` (`aceth.f90:730`); the two agree
  on every gated case (`natom = 1` wherever there is coherent elastic) but are
  different inputs.
- Binary (negative-unit) decks.

## Reproducing

```bash
cargo test --release -p njoy-outram-park-fork --test acer_thermal_word_parity zrh -- --nocapture
# the reference table: NJOY2016 on the committed deck
cd <scratch> && ln -s <repo>/reference-data/endf/n-001_H_001-ENDF8.0-Beta6.endf tape20 \
  && ln -s <repo>/reference-data/endf/tsl-HinZrH-ENDF8.0.endf tape21 \
  && njoy < <repo>/reference-data/acer/thermal_decks/hzrh_296k_tol0p001.njoy
```
