# WIMSR (WIMS-D library) against NJOY2016 — ENDF/B-VIII.0 U-238

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

**Generated:** 2026-09-11T03:30:00Z
**Crate version / commit:** `outram-park-backend` `develop` at `b9d44326` plus the commit this file is part of (the `src/wimsr/` port)

## 1. Methodology

**What is computed.** The crate's port of `wimsr.f90` (`wimsr::run_gendf`:
`wminit`, `xsecs`/`xseco`, `resint`/`rsiout`, `p1scat`/`p1sout`, `wimout`)
run on the GENDF NJOY2016 itself wrote, producing the WIMS-D library text
and the intermediate quantities the `iprint = 2` listing prints.

**Oracle.** Upstream Fortran NJOY2016 at commit
`ac5adf5f33d893e42f2eed7fb286b0d51c7580da` (2016.79), gfortran 13.3.0,
built 2026-09-10, on the committed deck
`reference-data/wimsr/u238-ENDF8.0-293.6K-29g-6sigz-wimsd.njoy-input`:

```text
reconr  err 0.001 -> broadr 293.6 K, 0.001
thermr  0 22 23 / 0 9237 8 1 1 0 0 1 221 1 / 293.6 / 0.001 4.0 /   free-gas MT=221
groupr  9237 1 0 3 1 1 6 1 / 29 groups / iwt=3 / lord=1 / sigz 1e10 1e4 1e3 1e2 10 1
        3/1 2 18 102 252 452 221, 6/2 18 221
wimsr   24 25 / 2 4 9 / 29 15 8 15 / 9237 1 9238.0 0 /
        0 0 1e10 1 0 221 0 0 0 1 0 0 /   ntemp nsigz sgref ires sigp mti mtc ip1opt inorf isof ifprod jp1
        1 1 1 1 1 1 1 1 /                 goldstein lambdas, nrg = 8
```

Inputs to the crate: `tape24` (the GENDF, 95 KB, committed) and the deck's
WIMSR cards as a `WimsrInput`. Oracle outputs: `tape25` (the 185-line
WIMS-D library, committed) and NJOY's `output` listing (committed).

**Predictions, stated before the first run.**

1. *Library:* byte-identical. Every library word is a sum or a ratio of
   GENDF words, printed with `1PE15.8`; an arithmetic-order difference
   could at most move the ninth figure. The test compares line by line
   and, for a differing line, falls back to a numeric comparison at 1e-8
   so a last-digit difference is reported rather than hidden.
2. *Listing stages:* within the printed precision — 4 figures
   (`1p,6e12.4`, so 5e-5 relative) for the `xseco` tables and the P1 rows,
   6 figures for the resonance-integral tables.

**Pass criterion.** Library: zero differing lines (numeric fallback
≤ 1e-8). Listing: worst relative deviation ≤ 5e-5 (4-figure tables),
≤ 5e-6 (6-figure tables).

## 2. Reference

```bibtex
@techreport{njoy2016,
  author      = {MacFarlane, R. E. and Muir, D. W. and Boicourt, R. M. and Kahler, A. C. and Conlin, J. L.},
  title       = {The {NJOY} Nuclear Data Processing System, Version 2016},
  institution = {Los Alamos National Laboratory},
  number      = {LA-UR-17-20093},
  year        = {2016},
  note        = {\S WIMSR; source \texttt{wimsr.f90}, git ac5adf5 (2016.79)}
}
@techreport{endfb80,
  author      = {Brown, D. A. and others},
  title       = {{ENDF/B-VIII.0}: The 8th Major Release of the Nuclear Reaction Data Library},
  journal     = {Nuclear Data Sheets},
  volume      = {148},
  pages       = {1--142},
  year        = {2018},
  note        = {n-092\_U\_238, MAT 9237}
}
```

The reference numbers are the committed oracle files themselves
(`reference-data/wimsr/*.wims`, `*.njoy-listing`).

## 3. Results (2026-09-11, `tests/wimsr_u238_njoy_golden.rs`)

Command:

```text
cargo test --release -p njoy-outram-park-fork --test wimsr_u238_njoy_golden -- --nocapture
```

Output (abridged):

```text
[wimsr-u238] sigma potential (16-23): worst 1.71e-5 over 8 values
[wimsr-u238] scattering power per unit lethargy (16-23): worst 1.77e-5 over 8 values
[wimsr-u238] transport corrected total (1-23): worst 3.58e-5 over 23 values
[wimsr-u238] absorption (1-23): worst 2.42e-5 over 23 values
[wimsr-u238] neutron current spectrum (1-23): worst 2.95e-5 over 23 values
[wimsr-u238] transport corrected total at 294 K (24-29): worst 2.30e-5 over 6 values
[wimsr-u238] absorption at 294 K (24-29): worst 5.67e-6 over 6 values
[wimsr-u238] resonance integral absorption group 16: worst 5.75e-7 over 6 values
[wimsr-u238] resonance integral absorption group 20: worst 2.83e-6 over 6 values
[wimsr-u238] resonance integral fission yield group 20: worst 1.30e-6 over 6 values
[wimsr-u238] flux per unit lethargy group 17: worst 4.61e-6 over 6 values
[wimsr-u238] p1 row 1: worst 5.87e-6 over 2 values
[wimsr-u238] p1 row 26: worst 1.60e-5 over 5 values
[wimsr-u238] fission spectrum (1-3): worst 1.48e-5 over 3 values
[wimsr-u238] message: xsecs: spectrum calculated from fission matrix — only prompt contribution available
[wimsr-u238] 185 lines (njoy 185), ifiss 3, ntemp 1, nfiss 23
[wimsr-u238] identical lines 185/185, differing 0, worst numeric 0.000e0
test listing_stage_values ... ok
test library_matches_njoy_wimsd_tape ... ok
```

| Quantity | Where | Values | Worst relative deviation | Criterion | Met |
|---|---|---|---|---|---|
| WIMS-D library | `tape25` | 185 lines | 0 differing lines | 0 | yes |
| σ_pot, slowing-down power (groups 16–23) | listing | 16 | 1.77e-5 | 5e-5 | yes |
| transport-corrected total, absorption (1–23) | listing | 46 | 3.58e-5 | 5e-5 | yes |
| neutron current spectrum (1–23) | listing | 23 | 2.95e-5 | 5e-5 | yes |
| 293.6 K thermal total, absorption (24–29) | listing | 12 | 2.30e-5 | 5e-5 | yes |
| resonance integrals (abs. g16, g20; ν σ_f g20) | listing | 18 | 2.83e-6 | 5e-6 | yes |
| flux per unit lethargy (g17) | listing | 6 | 4.61e-6 | 5e-6 | yes |
| P1 rows 1 and 26 | listing | 7 | 1.60e-5 | 5e-5 | yes |
| fission spectrum (1–3) | listing | 3 | 1.48e-5 | 5e-5 | yes |

Both predictions held; the library was byte-identical on the first run.

**One finding (listing only).** The "neutron current spectrum" check
failed on the first run: the crate printed `1.9963` for group 1 where
NJOY prints `1.9876`, and NJOY's group 15 (`igref`) is `0.99561`, not 1.
`xseco` computes `p1nrm = 1/p1flx(igref)` (`wimsr.f90:1467`) but declares
`p1nrm` as an **integer** (l.1435), so the quotient truncates to 1 and the
raw `p1flx` is printed. Predicted before the fix: mirroring the truncation
makes group 1 print the raw `1.9876`. Measured after: 2.95e-5. The library
does not use this quantity.

## 4. What this does and does not establish

- The port reproduces NJOY2016's WIMS-D library exactly for a fissile
  material with a resonance table (`ires = 1`, six σ₀), P1 matrices
  (`ip1opt = 0`), a fission spectrum from the matrix (`isof = 1`) and one
  temperature with a free-gas thermal matrix; and every intermediate stage
  NJOY prints for that case.
- Not established: WIMS-E output (`iverw = 5`), more than one temperature,
  a coherent thermal matrix (`mtc`), user current spectra (`jp1 > 0`),
  burnup tables (`iburn > 0`), `ifprod ≠ 0`, `inorf > 0`, materials
  without fission — all translated from the same lines but unexercised.
  The card-deck reader is not ported.
- The oracle is NJOY2016 itself, not a WIMS calculation: this is
  verification of the translation, not validation of WIMS libraries.
