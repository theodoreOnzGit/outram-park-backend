# Golden GENDF tapes for V&V (repo-tracked, NOT crate-packaged)

Multigroup **GENDF** output tapes produced by the upstream Fortran NJOY2016 and
used as golden references by the `njoy-outram-park-fork` GROUPR tests. Like the
raw ENDF tapes in `../endf/`, they live at the repository root, outside
`crates/`, so they are git-tracked but never part of a published crate tarball.
Read them through `njoy_outram_park_fork::reference_data::reference_file("gendf", …)`
(override the root with `OUTRAM_PARK_REFERENCE_DATA_DIR`).

A golden tape is only as reproducible as the deck that made it, so **every tape
here is committed together with the exact NJOY input deck** (`*.njoy-input`)
and the oracle build it came from. Regenerate with
`njoy < <deck>` after copying the named ENDF tape to `tape20`.

## Provenance

| File | Material | Input tape | Oracle | Deck | Generated |
|---|---|---|---|---|---|
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz.gendf` (35 KB, 427 lines) | U-238, MAT 9237 | `../endf/n-092_U_238.endf` (ENDF/B-VIII.0) | NJOY2016 upstream `ac5adf5` (2016.79), gfortran, built 2026-09-10 | `u238-ENDF8.0-293.6K-29g-iwt3-6sigz.njoy-input` | 2026-09-10 |
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-unresr.gendf` (35 KB, 427 lines) | U-238, MAT 9237 | `../endf/n-092_U_238.endf` (ENDF/B-VIII.0) | same build | `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-unresr.njoy-input` | 2026-09-10 |
| `u238-ENDF8.0-293.6K-29g-iwt-3-fehi1e4-6sigz.gendf` (35 KB, 427 lines) | U-238, MAT 9237 | `../endf/n-092_U_238.endf` (ENDF/B-VIII.0) | same build | `u238-ENDF8.0-293.6K-29g-iwt-3-fehi1e4-6sigz.njoy-input` | 2026-09-10 |
| `u238-ENDF8.0-293.6K-29g-iwt-3-fehi1e4-het-6sigz.gendf` (35 KB, 427 lines) | U-238, MAT 9237 | NJOY's own 293.6 K PENDF (as above) | same build | `u238-ENDF8.0-293.6K-29g-iwt-3-fehi1e4-het-6sigz.njoy-input` | 2026-09-10 |
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord3-inelastic-mf3-mf6.gendf` (16 KB, 186 lines) | U-238, MAT 9237 | NJOY's own 293.6 K PENDF (as above) | same build | `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord3-inelastic-mf3-mf6.njoy-input` | 2026-09-10 |
| `u238-ENDF8.0-293.6K-29g-iwt3-1sigz-lord3-inelastic-mf3-mf6.gendf` (15 KB, 177 lines) | U-238, MAT 9237 | NJOY's own 293.6 K PENDF (as above) | same build | `u238-ENDF8.0-293.6K-29g-iwt3-1sigz-lord3-inelastic-mf3-mf6.njoy-input` | 2026-09-10 |
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mt257-259.gendf` (28 KB, 340 lines) | U-238, MAT 9237 | NJOY's own 293.6 K PENDF (as above) | same build | `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mt257-259.njoy-input` | 2026-09-10 |
| `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord3-mf6.gendf` (51 KB, 624 lines) | U-238, MAT 9237 | NJOY's own 293.6 K PENDF of `../endf/n-092_U_238.endf` (the RECONR/BROADR steps of the first deck) | same build | `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord3-mf6.njoy-input` | 2026-09-10 |

### `u238-ENDF8.0-293.6K-29g-iwt3-6sigz.gendf`

`RECONR` (`err = 0.001`) → `BROADR` (293.6 K, `errthn = 0.001`) → `GROUPR`
with `ign = 1` (29 user groups, breaks 1e-5 … 2e7 eV as listed in the deck),
`igg = 0`, `iwt = 3` (1/E), `lord = 0`, `ntemp = 1` (293.6 K), `nsigz = 6`
(`1e10 1e4 1e3 1e2 10 1` barn), `iprint = 1`; reactions `MF=3 MT=1, 2, 18, 102`.
No `UNRESR`/`PURR` step, so the tape carries no URR self-shielding
(`stounr`: "no unresolved sigma zero data") — self-shielding on this tape is
the Bondarenko flux alone. Because `lord = 0` with `nsigz > 1`, GROUPR promotes
to two Legendre orders, so each group record holds `NL*NZ*NG2 = 2*6*2 = 24`
words (`il` fastest, then `iz`, then `it`).

Consumed by `crates/njoy-outram-park-fork/tests/groupr_u238_gendf_golden.rs`,
whose doc comment records the measured agreement.

### `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-unresr.gendf`

Identical to the above except that `UNRESR` (`9237 1 6 1 / 293.6 / 1e10 1e4
1e3 1e2 10 1 /`) runs between `BROADR` and `GROUPR`, so the PENDF GROUPR read
carries an MF=2/MT=152 self-shielded URR table and the tape's unresolved-range
groups (20–149 keV) are Bondarenko-shielded through `stounr`/`getunr`.
Consumed by the same test (tier 3).

### `u238-ENDF8.0-293.6K-29g-iwt-3-fehi1e4-6sigz.gendf`

The first deck with `iwt = -3` (the GROUPR **flux calculator**) and card 8a
`fehi = 1e4 eV, sigpot = 11.29 b, nflmax = 300000` (no `ninwt`, `jsigz = 0`,
no heterogeneity terms): `genflx` solves the integral slowing-down equation
from `egn(1)` to `fehi` on the total-cross-section grid, extends it with the
weight shape below 0.1 eV and with the narrow-resonance flux above `fehi`.
NJOY's listing for this run: "flux calculator used weight function from 1e-5
to 9.9740E-02 ev, 926 points; computed flux from 1.0000E-01 to 1.0000E+04 ev,
99934 points; finished with narrow-resonance flux to 3.0000E+07 ev, 55488
points". Consumed by the same test (tier 4).

### `u238-ENDF8.0-293.6K-29g-iwt-3-fehi1e4-het-6sigz.gendf`

GROUPR alone (`tape20` = the ENDF tape, `tape22` = the 293.6 K PENDF written
by the first deck's RECONR/BROADR), the flux-calculator deck above with card
8a extended to its heterogeneity / multi-moderator fields:
`fehi = 1e4, sigpot = 11.29, nflmax = 300000, ninwt = 0, jsigz = 0,
alpha2 = 0.7768` (an O-16-like admixed moderator), `sam = 0.5` b per absorber
atom, `beta = 0.3` (heterogeneity), `alpha3 = 0.7143` (a C-12-like external
moderator), `gamma = 0.4`. NJOY's listing: "alpha2, sam = 7.7680E-01 0.500 /
beta, alpha3, gamma = 0.300 7.1430E-01 0.4000", same 926 / 99,934 / 55,488
point counts as the homogeneous run. `genflx` runs `nalph = 3`
(`groupr.f90:5402-5406`) with the seed `(sigz - sam)*wtf*(1 - beta)` and the
`k = 2, 3` moderator sources (`:5449,5468-5471,5496-5499,5513-5516,5536-5539`).
Consumed by the same test (tier 5).

### `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-lord3-mf6.gendf`

GROUPR alone (`tape20` = the ENDF tape, `tape22` = the 293.6 K PENDF written
by the first deck's RECONR/BROADR), same 29 groups, `iwt = 3`, six sigma-zero
values, but **`lord = 3`** and reactions `MF=3 MT=1, 2` plus **`MF=6 MT=2`**
(the elastic transfer matrix). The MF=6 section therefore carries `NL = 4`,
`NZ = 6`: per initial group the four Legendre flux components
`wtf*fac^(il+1)` (`genflx`, `groupr.f90:5651-5657`) followed by the P0–P3
transfer elements, `il` fastest, then `iz`, then the secondary slot.
Consumed by `crates/njoy-outram-park-fork/tests/groupr_u238_elastic_matrix_golden.rs`
(the `lord3` test), whose doc comment records the measured agreement.

### `u238-ENDF8.0-293.6K-29g-iwt3-{6sigz,1sigz}-lord3-inelastic-mf3-mf6.gendf`

GROUPR alone on the same PENDF, `lord = 3`, reactions `MF=3` and `MF=6` for
the **discrete inelastic levels** MT=51, 52, 60 and 89 (`QI` = -44.916 keV,
-148.38 keV, -930.55 keV, -1.2857 MeV): the `q < 0` threshold path of
`getdis` (`groupr.f90:9440,9619-9677`). The `6sigz` tape has the six
sigma-zero values of the other decks — only `MF=3/MT=51` is self-shielded
(`nz = 6`, `:5816-5823`); everything else has `nz = 1` but is still served
from the tabulated `genflx` flux at dilution 1 (`getflx`, `:6478-6510`). The
`1sigz` tape has `nsigz = 1` (`1e10` alone), so `getflx` uses the bare
`getwtf` 1/E weight on its 1 % ladder (`:6512-6516`). Consumed by
`crates/njoy-outram-park-fork/tests/groupr_u238_inelastic_matrix_golden.rs`,
whose doc comment records the three findings and the measured agreement.

### `u238-ENDF8.0-293.6K-29g-iwt3-6sigz-mt257-259.gendf`

GROUPR alone on the same PENDF, `lord = 0`, six sigma-zero values, reactions
`MF=3 MT=1` and the three **derived quantities** `MT=257` (average energy,
`sig = E`), `MT=258` (average lethargy, `sig = ln(1e7/E)`) and `MT=259`
(average reciprocal velocity, `sig = 1/sqrt(1.919e8 E)`), which `getsig`
computes analytically at retrieval time with a `1.01 E` step
(`groupr.f90:6758-6772`); GROUPR writes them with `NL = NZ = 1`. Consumed by
`crates/njoy-outram-park-fork/tests/groupr_u238_derived_quantities_golden.rs`.

Data policy: GENDF is a *derived* product of open ENDF/B-VIII.0 data processed
with the BSD-licensed NJOY2016; it carries no proprietary content.
