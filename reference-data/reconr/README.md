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
