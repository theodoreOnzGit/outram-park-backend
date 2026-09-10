# Golden NJOY2016 RECONR tapes for V&V (repo-tracked, NOT crate-packaged)

0 K pointwise (PENDF) tapes written by the upstream Fortran NJOY2016
`RECONR` module — `tape21` of the committed decks — used as like-for-like
oracles for the crate's resonance-grid reconstruction
(`tests/reconr_si30_inter_resonance_capture.rs`). Like `../endf/`,
`../errorr/`, `../covr/` and `../leapr/`, they live at the repository root,
outside `crates/`, so they are git-tracked but never part of a published
crate tarball. Read them through
`njoy_outram_park_fork::reference_data::reference_file("reconr", …)`.

The 293.6 K BROADR outputs of the same decks are in `../errorr/*.pendf`
(`tape22`); this directory holds the un-broadened `tape21`, which is what
isolates RECONR's grid from BROADR's.

## Provenance

NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0, built 2026-09-10;
generated 2026-09-10 (wave 3) from `../endf/n-014_Si_030-ENDF8.0.endf`.

| Tape | Material | Deck | Points |
|---|---|---|---|
| `si30-ENDF8.0-0K.pendf` (1.3 MB) | Si-30, MAT 1431, `LRP=1`, RRR 1e-5 eV – 1.5 MeV, `LRF=3` | `reconr 20 21 / 1431 0 / 0.001 /` (err = 0.001, 0 K, `errmax`/`errint` defaulted) | 7,284 per MF=3 section (unionised) |

## Measured agreement (2026-09-10, `tests/reconr_si30_inter_resonance_capture.rs`)

Before the fix (bead `op-yr43`): between the 2.2 and 4.9 keV resonances the
crate kept 8 grid points where NJOY has 47, and MT=102 at 3324.8 eV was
6.276e-4 b against NJOY's 5.453e-4 b (+13 %) — the crate measured a
panel's midpoint error against `max(sigma, 0.1 b)`, a floor `resxs`
(`reconr.f90:2240-2570`) does not have.

After porting upstream's test (per-reaction relative error on
elastic/fission/capture, the `errmax = 10 err` / `errint = err/20000`
resonance-integral check, `err/5` below 0.4999 eV, 7-figure midpoint
rounding with the significant-figure termination, and the `estp = 4.1`
step-increase rule): MT=102 at 3324.8 eV is **5.4526e-4 b, identical to
NJOY**; the 2.5–4.5 keV window holds **47 points for every reaction, as
NJOY**; over the whole resolved range the worst deviation is 9.2e-3
(MT=102 at 1.3 MeV, 4.5e-5 b) and 5–6e-3 (MT=1/2 at 172 keV, 1e-2 b), all
on cross sections small enough to sit in the `errmax` (1 %) relaxation
regime where two independently chosen grids may differ by up to ~2 %. The
crate's per-reaction point counts are 7,184 (MT=1/2) and 5,011 (MT=102)
against NJOY's unionised 7,284.

Data policy: derived products of open ENDF/B-VIII.0 data processed with the
BSD-licensed NJOY2016; no proprietary content.

## Ar-37 (TENDL-2023, MLBW) — added 2026-09-10 (bead `op-cral`)

| Tape | Material | Deck | Points |
|---|---|---|---|
| `ar37-tendl2023-0K.pendf` (632 KB) | Ar-37, MAT 1828, `LRP=1`, RRR 1e-5 eV – 4268.035 eV `LRU=1/LRF=2` (MLBW, 7 levels incl. three bound levels), URR 4268–5548 eV `LRF=2` | `reconr 20 21 / 1828 0 / 0.001 / 0 /` (err = 0.001, 0 K) from `../endf/n-018_Ar_37-tendl2023.endf` | 1,405 resonance points |

`tape21` is byte-identical whether NJOY reads the original tape or the
`-mf2-L1-last` variant `../errorr/` uses (RECONR sums over L).

Oracle for `tests/reconr_ar37_mlbw_njoy_golden.rs`. Before the fix the
crate evaluated every `LRF=2` range with the SLBW elastic formula
(`csslbw`), which for Ar-37's large bound-level neutron widths put the
elastic +6.33 % high from 1e-5 eV to 1 eV, +6.23 % at 100 eV, −8.23 % at
1 keV, −5.53 % at 4 keV against this tape while capture matched to 1e-7.
After porting `csmlbw`'s per-`J` interference assembly
(`reconr::slbw::eval_mlbw_lstate`): elastic within 4.1e-7 below 100 eV,
worst 4.5e-4 at 1400 eV (a resonance wing, grid interpolation); capture
unchanged (worst 3.0e-4 at 1400 eV). MT=1 still differs by 1.45 % at
4 keV and 0.34 % at 4268 eV where MT=2/MT=102 agree — the MF=3 total
background near the RRR/URR boundary, not the resonance kernel (recorded,
not chased).
