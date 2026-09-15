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

## Cl-35 (ENDF/B-VII.1, `LRF=7` R-matrix limited) — added 2026-09-11 (bead `op-cjw.2`)

| Tape | Material | Deck | Points |
|---|---|---|---|
| `cl35-ENDF7.1-0K-err0.01.pendf` (4.0 MB) | Cl-35, MAT 1725, `LRP=1`, RRR 1e-5 eV – 1.2 MeV `LRU=1/LRF=7` (`KRM=3`, 8 spin groups, pairs γ/n/p) | `cl35-ENDF7.1-0K-err0.01.njoy-input` — NJOY2016's own `tests/20/input` (`errorr` 999 dummy-MF=33 step, then `reconr 21 22 / 1725 0 0 / 0.01 /`; the PENDF is that deck's `tape22`) from `../endf/n-017_Cl_035-ENDF7.1.endf` | 10,730 per section, unionised; `MT=600` carries the R-matrix (n,p) channel |

The deck's `err = 0.01` is the upstream test's own setting; RECONR writes
the exact 0 K value at every node regardless, which is what
`tests/reconr_cl35_rml_njoy_golden.rs` uses: the crate's SAMM kernel
(`samm::xsformula::cssammy`) evaluated at all 10,417 nodes below 1.2 MeV
plus the ENDF MF=3 background agrees with the tape to **4.9e-7** (elastic,
whose background is zero), **4.5e-7** (capture) and **4.9e-7** ((n,p)) —
the 7-figure printing floor. `MT=1` on this tape is the *sum of every
partial* (RECONR: "redundant reactions are reconstructed to be the sum of
their parts"), which differs from the evaluation's own MF=3 `MT=1` TAB1
by `MT=600`'s and `MT=107`'s backgrounds (2.7e-3 at 1.15 MeV); the test
checks the kernel's total against its own three channels instead. The
crate's end-to-end RECONR (`err = 0.001`) sits within 1.3e-4 (elastic),
1.1e-3 (capture) and 4.0e-4 ((n,p)) of the tape at sampled energies —
interpolation between two independently thinned grids. Before 2026-09-11
the crate dropped the (n,p) channel from RECONR's output (its `MT=600`
had only the 80 background points); it now carries every extra `LRF=7`
particle-pair channel as upstream `emerge` does.

Data policy: derived product of open ENDF/B-VII.1 data processed with the
BSD-licensed NJOY2016; the input tape is NJOY2016's own test resource.

## `sr88-ENDF8.1-0K-err0.001.pendf` — the LRF=7 background-R-matrix oracle

Added 2026-09-14. NJOY2016 upstream `ac5adf5f` (2016.79), gfortran 13.3.0,
built and run in-session; generated from `../endf/n-038_Sr_088-ENDF8.1.endf`
(MAT 3837) with the deck committed beside it as
`sr88-ENDF8.1-0K-err0.001.njoy-input`.

Sr-88 is the **only LRF=7 (R-matrix limited) evaluation** held in
`../endf/`, and it is the oracle for `KBK > 0` — a background R-matrix.
All seven of its spin groups carry `KBK = 1` with `LCH = 2`, `LBK = 2`
(SAMMY parametrisation) on the elastic channel, and all 443 of its
resonances sit at or above 12.41 keV, so at thermal that background term
carries essentially the whole elastic cross section beyond hard-sphere
scattering.

Consumed by `tests/reconr_sr88_lrf7_kbk_njoy_golden.rs`, which compares
MF=3 MT=1/2/102 on NJOY's own 44,326-point grid across the resolved range.
It is the regression gate for gh:#202 / `bn:op-hb9l`, where discarding this
term left elastic flat at the potential value `4 pi a^2 = 4.969327 b`
against NJOY's 8.843210 b at 1e-5 eV.

## `fe58-…-0K-err0.001.mt152.pendf`, `u238-…-0K-err0.001.mt152.pendf` — the Case A and `LSSF=1` MT=152 oracles

Added 2026-09-15. NJOY2016 upstream `ac5adf5f` (2016.79), gfortran 13.3.0,
built and run in-session, from `../endf/n-026_Fe_058-ENDF8.0-Beta4.endf`
(MAT 2637) and `../endf/n-092_U_238.endf` (MAT 9237), with the decks
committed beside them as `*.njoy-input`.

**These two hold only the MF=2/MT=152 section**, wrapped in the minimum ENDF
terminators needed to parse — unlike every other file here, which is a whole
PENDF. U-238's full RECONR output is 97 MB, far past what belongs in a
repository, and extracting the one section keeps both materials on a single
convention. To regenerate either in full, run the committed deck; to reduce it
to what is committed here, keep the records whose columns 71-75 read `  2152`
and append a SEND/FEND/MEND/TEND.

They exist because the U-234 oracle beside them reaches only Case C with
`LSSF = 0`, leaving two paths in the MT=152 writer untested:

- **Fe-58** carries `LRU=2, LRF=1, LFW=0, LSSF=1` over `3.5e5 .. 3.0e6 eV`.
  It is the **only** `LRU=2/LRF=1` range in `../endf/` (screened with
  `../endf/screen_endf_flags.py`), so it is the only Case A oracle available,
  and it happens to be `LSSF = 1` as well.
- **U-238** carries Case C with `LSSF = 1` and `L <= 2`, which isolates the
  `LSSF = 1` early return from the Case A grid change.

Consumed by `tests/reconr_mt152_case_a_and_lssf1_vs_njoy2016.rs`. U-238 agrees
to **1e-13** on all 336 stored values.

**Fe-58's stored cross sections are wrong, and are committed deliberately as
the record of an upstream defect.** `unfac` (`reconr.f90:4473-4496`) has no
`l >= 3` branch — no `else` after `else if (l.eq.2)` — so `vl` and `ps` keep
their previous values, and `csunr1`'s `vl = vl*e2` (`:4010`) sits inside the
J loop, multiplying `vl` by another `sqrt(E)` per J-state. Fe-58 has `NLS = 4`,
so its stored total runs `1.42e4 b` at 350 keV to `6.62e5 b` at 3 MeV, roughly
1900x the unitarity limit and rising with energy. The test reproduces those
numbers from the defective recurrence to 2.7e-7, so the diagnosis is measured
rather than asserted. NJOY's *UNRESR* does not share the defect: `uunfac`
(`unresr.f90:1213-1239`) writes a bare `else`, and this crate's kernel ports
`uunfac`. With `LSSF = 1` the values never reach a transport calculation —
the evaluation's own MF=3 carries the cross sections, and MT=152 is consumed
by UNRESR/PURR only as the denominator of self-shielding ratios.

Data policy: derived products of open ENDF/B-VIII.0 data processed with the
BSD-licensed NJOY2016. Fe-58 is the ENDF/B-VIII.0 Beta4 release; see
`../endf/README.md` for both tapes' provenance.
