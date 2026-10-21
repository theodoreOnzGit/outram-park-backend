# GAMINR on a real photoatomic evaluation vs NJOY2016

**Date:** 2026-09-17
**Module:** `src/gaminr/` (2549 lines)
**Test:** `tests/gaminr_vs_njoy2016.rs` (2 cases)
**Status:** verification (cross-code), ~~**partial**~~ **complete for this deck since
2026-10-05**. **No human V&V.**

> **CORRECTED 2026-10-05 (GitHub #534): the disagreement below was the oracle's,
> not the port's.** The deck used the same Fortran unit for `nendf` and `npend`
> (`gaminr 20 20 0 21`); NJOY's MF=27 read moved that unit under the MF=23
> reader, so NJOY's own MF=26 MT=502/504 and MT=525 were computed from a
> corrupted cross section. With distinct units NJOY agrees with this port, which
> was not changed, to **3.71e-7 worst** on every word of every reaction. See
> [the resolution](#resolution-2026-10-05-the-oracle-deck-shared-a-unit) at the
> end; the 2026-09-17 text is kept as written, with its wrong conclusions struck
> through.

## Why this exists

GAMINR is 2549 lines of ported physics and its only oracle was a **synthetic**
Z = 6 photoatomic tape, written for the purpose. The test's own docs recorded
why: *"No photoatomic evaluation is available offline (both public data hosts
refuse downloads from this environment)"*.

That was no longer true. `reference-data/endf/photoat-092_U_000-ENDF8.0.endf`
is a real ENDF/B-VIII.0 photoatomic evaluation — MAT 9200, Z = 92, MF=23
MT=501/502/504/515/516/517/522/534–570 and MF=27 MT=502/504/505/506 — and it is
committed. The check that the data was absent was never re-run after it was
added. Same failure mode as the `openmc` binary earlier in this study, and the
same fix: run the check.

## Methodology

The **same deck** as the synthetic case, so the two are directly comparable:

```
gaminr
 20 20 0 21/
 9200 3 3 3 1/
 'u photoatomic ENDF/B-VIII.0, lanl 12-group, iwt=3, lord=3'/
 23 501 / 23 502 / 23 504 / 23 516 / 23 522 /
 26 502 / 26 504 / 26 516 /
 23 525 /
 0/ 0/
stop
```

LANL 12-group photon structure (`igg = 3`), `iwt = 3` (1/E with roll-offs),
`lord = 3`. NJOY2016 `ac5adf5f`, built from source and executed 2026-09-17.
Golden: `reference-data/gendf/photoat-U000-ENDF8.0-lanl12-iwt3-lord3.gendf`
(16 KB), deck beside it. Because MF=23 is already lin-lin (`NR = 1`, `INT = 2`,
13 999 points on MT=501) the same tape serves as `nendf` and `npend`, exactly as
in the synthetic case.

Comparison: every GENDF record word, worst relative deviation, tolerance 1e-5 —
unchanged from the synthetic case.

## Results

| reaction | agreement |
|---|---|
| MF=23 MT=501 total | **3.6e-9** |
| MF=23 MT=502 coherent | **2.7e-7** |
| MF=23 MT=504 incoherent | **3.4e-9** |
| MF=23 MT=516 pair | **3.0e-9** |
| MF=23 MT=522 photoelectric | **1.8e-8** |
| MF=26 MT=516 pair matrix | **3.0e-9** |
| MF=26 MT=502 coherent matrix | ~~**disagrees**~~ **2.75e-7** (2026-10-05, distinct units) |
| MF=26 MT=504 incoherent matrix | ~~**disagrees**~~ **3.71e-7** (2026-10-05) |
| MF=23 MT=525 total heating | ~~0.61 %~~ **2.40e-7** (2026-10-05) |

Every **vector** reaction is right on a real evaluation, to between 3.6e-9 and
2.7e-7 — which is the substantive new coverage: `gpanel`, `dspla`, the group
structure, the `iwt = 3` weight and the MF=23 retrieval are all exercised on
13 999-point real data rather than a 53-point analytic shape.

## ~~The defect this found~~ The disagreement this found (2026-09-17; resolved below)

The only matrix that consumes **no** MF=27 table — MT=516, pair production —
agrees to 3.0e-9. The two that **do** — MT=502 (coherent form factor) and
MT=504 (incoherent scattering function) — do not. At group 1:

```
MF=26/502  ours  1.822327e2  1.278700e2  9.884985e1  7.259552e1
           njoy  1.394827e2  1.056669e2  8.412942e1  6.462165e1
MF=26/504  ours  3.719060e1 -2.994461e-2 2.265225e0 -8.314837e-1
           njoy  1.302159e2 -7.558322e-1 7.802657e0 -2.991777e0
```

NJOY writes **2** records for each (groups 1 and 12) where this port writes 12.
That part is explained: `gaminr.f90:400` drops a record whose every normalised
value is below 1e-9 and always keeps `ig == ngg`. This port implements that gate
(`matrix.rs`, `igzero || ig == ngg`); the records survive here because the values
it computes are not small.

### The sharpest clue, and it is an invariant, not a hunch

`gtff_coherent` normalises its Legendre moments by `sigcoh`, so `ff[0] == 1` and
the MF=26 `l = 0` group value **must** equal the MF=23 group cross section. On
the synthetic tape NJOY's two are identical — 0.41244466 both. On uranium:

- NJOY's MF=23/502 group 1 = **182.232750**
- this port's MF=26/502 `l = 0` = **182.232750** (the same number)
- NJOY's own MF=26/502 `l = 0` = **139.482685**

So NJOY's MF=26 path is *not* simply group-averaging MF=23 with `ff[0] = 1`,
and this port's is. Whatever NJOY does differently does not show on a smooth
24-point analytic form factor and does on a 964-point real one.

Suspects, none eliminated: the nudged `elo` in the photon `gpanel`
(`gaminr.f90:951-952` — the existing test's own docs already flag it as
differing from GROUPR's `panel`); the `stest`/`xlim` termination of the coherent
x-integration (`:1311-1323`); the MF=27 `terpa` panel bounds under U's 964-point
tables against the synthetic's 24.

Tracked as a task; the three affected reactions are **printed, not asserted**
while it is open. Asserting them would mean freezing wrong numbers or widening a
gate past the point where it says anything.

## What this does NOT establish

- ~~**GAMINR's matrix path is not verified on real data.** Two of the three
  matrices disagree, and this record does not diagnose why.~~ Superseded
  2026-10-05: all three matrices agree with NJOY run on distinct units. The synthetic case
  still passes at 1e-5, so the matrix code is not wholly wrong — but a synthetic
  tape written alongside the port is the weakest kind of oracle, which is the
  whole reason for this exercise.
- **No validation**, and no human review.
- **One evaluation, one group structure, one weight.** `igg = 3`, `iwt = 3`,
  `lord = 3`, Z = 92 only.

## Resolution 2026-10-05: the oracle deck shared a unit

**Read upstream first, then predict, then measure** (GitHub #534).

**What upstream does.** `gtsig` (`gaminr.f90:1133-1160`) positions `npend` at
MF=23 and reads the cross section through `gety1`, which holds one
`npage = 306`-word page (153 pairs) and pulls the next with
`moreio(itape, ...)` as the energy advances. For MF=26 MT=502/504, `gtff`'s
initialisation (`:1229-1246`) calls `findf(matd,27,mtd,nendf)` and reads the
whole MF=27 table with `tab1io`/`moreio`. The deck above passes **unit 20 for
both** `nendf` and `npend`. So after `gtff` initialises, unit 20 sits in MF=27,
and every later `gety1` page of the MF=23 cross section is read from MF=27
records. Nothing in NJOY checks for it.

That accounts for every clue in the 2026-09-17 record:

- MF=23 vectors and the MT=516 matrix never trigger an MF=27 read, so they
  agreed;
- the synthetic tape's MF=23 has 53 points, one page, so it agreed everywhere;
- NJOY's MF=26/502 `l = 0` was not its MF=23/502, although `ff(1) = 1` makes
  them equal by construction, while this port's was;
- NJOY wrote 2 records per matrix: its corrupted values fell under `dspla`'s
  1e-9 gate.

**Hypothesis, written before running (with sign and size).** With distinct
units, NJOY's MF=26/502 `l = 0` in group 1 rises from 139.482685 to
**182.232750** (+30.7 %, its own MF=23/502 value). This port, unchanged,
agrees with that run within the existing 1e-5 gate on MT=502, 504 and 525.

**Run.** NJOY2016 `ac5adf5` rebuilt from `upstream_source/NJOY2016`
(gfortran 13.3.0, `cmake -DCMAKE_BUILD_TYPE=Release`). The original
shared-unit deck reproduced the committed tape **byte for byte**, so the
build is the same oracle. Then the deck was rerun as `gaminr 20 22 0 21`,
`tape22` a copy of `tape20`
(`reference-data/gendf/photoat-U000-ENDF8.0-lanl12-iwt3-lord3-sepunits.*`,
288 lines, SHA-256 `5c792654…22d462`).

**Result.** NJOY's group 1 of MF=26/502 is now
`182.232750 127.870004 98.8498513 72.5955151`, this port's row above to every
printed figure. NJOY writes 12 records per matrix, as this port does.
`tests/gaminr_vs_njoy2016.rs` now uses the new tape and **asserts every
reaction**:

| reaction | words | worst relative deviation |
|---|---:|---|
| MF=23 MT=501 | 24 | 3.57e-9 |
| MF=23 MT=502 | 24 | 2.73e-7 |
| MF=23 MT=504 | 24 | 3.36e-9 |
| MF=23 MT=516 | 18 | 3.03e-9 |
| MF=23 MT=522 | 24 | 1.85e-8 |
| MF=26 MT=502 coherent | 96 | **2.75e-7** |
| MF=26 MT=504 incoherent | 320 | **3.71e-7** |
| MF=26 MT=516 pair | 18 | 3.03e-9 |
| MF=23 MT=525 total heating | 24 | **2.40e-7** |

Worst overall **3.71e-7** (MF=26/504, group 5, word 5), NJOY's seven-figure
storage. Both predictions held. **No port code changed.** The synthetic case
is unchanged (worst 4.35e-7). Its deck shares the unit too, and is unaffected
because its MF=23 fits in one page; it is left as committed.

**What this establishes, and what it does not.** GAMINR's three matrix feed
functions agree with NJOY2016 on a real evaluation, for one group structure
(`igg = 3`), one weight (`iwt = 3`) and `lord = 3`, at Z = 92. It is
verification, not validation, and has no human review. The general lesson
goes in the book: a cross-code oracle is an input deck as well as a program,
and a deck can be wrong in a way only the size of the data exposes. Every
other NJOY deck in `reference-data/` was checked for a unit shared between two
roles on one module's card 1; only the two GAMINR decks do it.
