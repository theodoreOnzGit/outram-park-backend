# Golden NJOY2016 WIMSR library for V&V (repo-tracked, NOT crate-packaged)

A WIMS-D format library written by the upstream Fortran NJOY2016 `WIMSR`
module, with the GENDF it read, as the oracle for a future port of
`wimsr.f90` (bead `op-cjw.14`; the crate's `src/wimsr/` is still a
`NotPorted` stub — nothing here is consumed by a test yet). Like the other
`reference-data/` directories it lives outside `crates/` so it is
git-tracked but never part of a published crate tarball. Read it through
`njoy_outram_park_fork::reference_data::reference_file("wimsr", …)`.

## Provenance

NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0, built 2026-09-10;
generated 2026-09-10 from `../endf/n-092_U_238.endf` (ENDF/B-VIII.0 U-238,
MAT 9237) with the committed deck
`u238-ENDF8.0-293.6K-29g-6sigz-wimsd.njoy-input` (83 s):

```text
reconr  err 0.001 -> broadr 293.6 K, 0.001
thermr  0 22 23 / 0 9237 8 1 1 0 0 1 221 1 / 293.6 / 0.001 4.0 /   free-gas MT=221
groupr  9237 1 0 3 1 1 6 1 / 29 groups (the GENDF goldens' grid) / iwt=3 / lord=1 / 6 sigma0
        3/1 2 18 102 252 452 221, 6/2 18 221
wimsr   24 25 / 2 4 9 / 29 15 8 15 / 9237 1 9238.0 0 /
        0 0 1e10 1 0 221 0 0 0 1 0 0 /   ntemp nsigz sgref ires sigp mti mtc ip1opt inorf isof ifprod jp1
        1 1 1 1 1 1 1 1 /                 goldstein lambdas, nrg = 8
```

| File | What |
|---|---|
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-thermal221-for-wimsr.gendf` (95 KB) | `tape24`: the GENDF WIMSR read — MF=3 MT 1/2/18/102/221/252/452 and MF=6 MT 2/18/221 (`NL = 2`, `NZ = 6`) |
| `u238-ENDF8.0-293.6K-29g-6sigz-wimsd.wims` (185 lines) | `tape25`: the WIMS-D library (`iverw = 4`): burnup header, material record `9238 2.38050770E+02 92 3 1 1 1`, temperature-independent block (potential, slowing-down power, transport, absorption, lambdas), ν σ_f / σ_f, the non-thermal P0 matrix, the 293.6 K thermal block, then per resonance group the `(T, σ_p + λ σ_pot, RI)` tables for absorption and ν-fission over the six dilutions |
| `u238-ENDF8.0-293.6K-29g-6sigz-wimsd.njoy-listing` | NJOY's `output`: the `xsecs`/`resint`/`p1scat` prints (`iprint = 2`) — intermediate results a port can be checked against stage by stage |

The 41 MB PENDF (`tape22`) is **not committed** (same policy as the GROUPR
goldens); regenerate it with the deck's RECONR/BROADR lines.

## Why this deck (scoping notes, 2026-09-10)

- WIMSR **requires a thermal inelastic matrix on the GENDF**: `xsecs`
  sets `jic` only on a temperature-dependent MF=6 section other than MT=2
  (`wimsr.f90:1167`), and without it stops with "use only 0 temps for
  mat … mti missing from higher temps". The committed GROUPR goldens in
  `../gendf/` carry no thermal section, so they cannot drive WIMSR; hence
  THERMR's free-gas MT=221 and `mti = 221` here.
- `ires = 1` with six `sigma0` values exercises `resint`/`rsiout`;
  `isof = 1` the fission-spectrum path (NJOY's listing: "spectrum
  calculated from fission matrix"); `ip1opt = 0` (P1 matrices) was left at
  the default `1` in this run — a second deck with `ip1opt = 0` is the
  natural next oracle for `p1scat`/`p1sout`.
- Port size: `wimsr.f90` is 2,150 lines — `wminit` (115), `xsecs` (560),
  `xseco` (260), `resint` + `rsiout` (440), `p1scat` + `p1sout` (300),
  `wimout` (160) — and reads the GENDF with the same `contio`/`listio`
  walk the crate's DTFR reader already performs.

Data policy: derived products of open ENDF/B-VIII.0 data processed with the
BSD-licensed NJOY2016; no proprietary content.
