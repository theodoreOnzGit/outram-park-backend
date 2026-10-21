# `process`'s PENDF precision against route 4's data (GitHub #929)

> Research, education and V&V only. AI-generated (Claude Opus 5.5 in Claude
> Code), not yet human-reviewed. **This record measures; it does not decide.**
> Whether route 4 moves onto `process` as it is (#931) is the maintainer's
> decision.

**Class: verification** (two data paths compared bit for bit, plus the
DBRC kernel run on both).

## The question

Epic #925 §5: `process::NuclearCrossSections::pendf` holds every value as
written to a PENDF tape (ENDF's 11-column field), while route 4
(`Nuclide::process_evaluation`) was described as keeping "BROADR's full
`f64`". Moving route 4 onto `process` would then change the data transport
reads.

## What reading the code showed first

- `broadr::broaden_result_with_tolerance` (`njoy-outram-park-fork/src/broadr/mod.rs:518-524`)
  rounds its input **and** its output through `through_pendf_text` whenever
  the joint walk runs (the result has MT=1). Route 4 calls that function, so
  its broadened sections are already PENDF-rounded. ~~"MC's
  `process_evaluation` keeps BROADR's full `f64`s" (#925 §5)~~ **CORRECTED
  2026-10-17**: true only of the 0 K elastic it keeps for DBRC, and of the
  per-reaction fallback taken when there is no MT=1.
- Route 4's 0 K elastic (`ProcessedEvaluation::elastic_0k`) is RECONR's raw
  MT=2 below `DBRC_GRID_MAX_EV` = 25 keV. `process`'s (#928) is the 0 K
  PENDF's MT=2.

Predictions posted on #929 before measuring: P1 broadened sections
identical; P2 0 K elastic differs in the last bits, at most 5e-7 relative;
P3 |Δk| ≪ 1 pcm, not resolvable.

## Methodology

`examples/process_precision_vs_route4.rs`, 2026-10-17, ENDF/B-VIII.0 tapes in
`reference-data/endf/`, the 13 five-route nuclides, 293.6 K.

- RECONR runs **once** per nuclide (`Nuclide::reconstruct_0k`, tolerance
  0.001). Route 4 is `Nuclide::process_from_stages(&recon0, None, 293.6,
  0.001)`; `process` is `process::cross_sections_at` on a `Reconstruction`
  holding the same `recon0` (PURR and HEATR off: neither changes a PENDF
  section).
- (1) Every broadened section compared point by point, bit for bit.
- (2) The 0 K elastic below 25 keV compared bit for bit; the largest
  relative difference in E and σ over the common length.
- (3) **The kernel, not a k.** For each nuclide, two `DbrcTable`s (route
  4's elastic and `process`'s, cap 1 keV), and 2 000 000
  `free_gas_elastic_scatter_dbrc` draws on each at log-uniform energies over
  1e-2 to 1e3 eV, kT at 293.6 K, the **same seed per draw** on both. Counted:
  draws whose outgoing energy or RNG state differ. Control: the same draws
  with DBRC off must differ, or the count could not have been non-zero.
- Hardware: Intel Core i9-13900K, 3 of 32 logical CPUs (`taskset -c 0-2`),
  62.5 GiB RAM, Linux 7.2.9, CPU only; machine shared with other agents.

## Results

| nuclide | sections | broadened points | differ | 0 K elastic points compared | differ | max dE/E | max dσ/σ | route 4 / `process` points ≤ 25 keV |
|---|---|---|---|---|---|---|---|---|
| U-234 | 53 | 142 320 | 0 | 45 484 | 45 484 | 1.0e-13 | 1.0e-13 | 45 484 / 45 485 |
| U-235 | 87 | 3 333 290 | 0 | 232 882 | 232 882 | 1.0e-13 | 1.0e-13 | 232 882 / 232 882 |
| U-238 | 52 | 1 093 217 | 0 | 447 685 | 447 685 | 1.0e-13 | 1.0e-13 | 447 685 / 447 686 |
| U-236 | 53 | 130 637 | 0 | 41 493 | 41 493 | 1.0e-13 | 1.0e-13 | 41 493 / 41 494 |
| F-19 | 33 | 18 419 | 0 | 1 259 | 1 259 | 1.0e-13 | 1.0e-13 | 1 259 / 1 259 |
| O-16 | 66 | 37 626 | 0 | 655 | 655 | 3.7e-7 | 1.0e-13 | 655 / 656 |
| H-1 | 3 | 1 893 | 0 | 558 | 558 | 1.0e-13 | 1.0e-13 | 558 / 559 |
| Al-27 | 134 | 72 778 | 0 | 1 092 | 1 092 | 1.0e-13 | 1.0e-13 | 1 092 / 1 092 |
| Si-28 | 61 | 69 973 | 0 | 560 | 560 | 1.0e-13 | 1.0e-13 | 560 / 560 |
| Si-29 | 63 | 65 868 | 0 | 937 | 937 | 1.0e-13 | 1.0e-13 | 937 / 937 |
| Si-30 | 37 | 37 065 | 0 | 1 452 | 1 452 | 1.0e-13 | 1.0e-13 | 1 452 / 1 452 |
| Mn-55 | 116 | 48 753 | 0 | 5 402 | 5 402 | 1.0e-13 | 1.0e-13 | 5 402 / 5 402 |
| B-10 | 53 | 15 917 | 0 | 563 | 563 | 1.0e-13 | 1.0e-13 | 563 / 564 |

DBRC draws: **0 of 2 000 000 differ on every one of the 13 nuclides**
(26 000 000 in all); the control (DBRC off) differs on 2 000 000 of
2 000 000 for each, so the instrument could have seen a change.

## Interpretation

- **P1 held exactly.** 5 067 756 broadened points, 0 differ. On the
  broadened sections `process` and route 4 are the same data.
- **P2 held, with a much smaller size than predicted.** Every 0 K elastic
  point differs, but by **1.0e-13 relative**: RECONR's raw values carry
  `sigfig`'s bias (`1.0000000000001`), which the text field removes. The
  prediction (≤ 5e-7) was set by the field's digit count and missed the
  cause; one O-16 energy differs by 3.7e-7, the field's own rounding.
  A side effect: raw 25 000 eV is 25 000 × (1 + 1e-13), just above the
  25 keV cut, so route 4 drops the endpoint that `process` keeps (6 of 13
  nuclides have one more point). It lies above the 1 keV DBRC limit, so no
  sampled outcome can see it.
- **P3, measured on the kernel instead of k.** No DBRC outcome changed in
  26 million draws, so a transport run on either elastic consumes the same
  random numbers and gives the same `k` unless a draw outside this sample
  flips. Expected flips at 1e-13: about 1e-13 per DBRC test. **No k run was
  made**; one would show bit-identical `k` and add nothing this does not.

## What this leaves for the maintainer

Precision is not a reason to keep route 4 off `process`: its broadened
sections are the same bits, and the 0 K elastic moves no DBRC draw. What
**does** differ between the two today, and will move route 4 under #931:

| | route 4 (`Nuclide::process_evaluation`) | `process` default |
|---|---|---|
| PURR | 20 bands / **16 ladders / 2 000 samples** (`URR_TABLE_SETTINGS`) | 20 / **64 / 10 000** |
| PURR's heating column | no HEATR (`from_endf`) | HEATR's MT=301 first (`from_endf_with_mt301`), as the NJOY deck |
| 0 K elastic kept | ≤ 25 keV, raw | whole range, PENDF-rounded |

Options, not chosen here: (a) move route 4 onto `process` as is (precision
settled above) and re-measure route 4 for the PURR change; (b) add a
full-`f64` mode to `process` anyway (the measurement says it would change
nothing); (c) keep route 4's PURR settings through
`CrossSectionSettings::with_purr(20, 16, 2000)` (and perhaps
`without_heatr()`) so #931 moves nothing. **Not checked:** whether (c)
reproduces route 4's PURR tables bit for bit.

**Decided 2026-10-17 (maintainer, on #929/#931): option (a).** Route 4 now
runs `process` at its defaults; the hand-built chain measured above as
"route 4" is kept as `Nuclide::process_evaluation_pre_931`. The re-measure
is in `../route4_process_931/README.md`.
