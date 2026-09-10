# Golden NJOY2016 LEAPR tapes for V&V (repo-tracked, NOT crate-packaged)

MF=7 thermal-scattering-law tapes produced by the upstream Fortran NJOY2016
`LEAPR` module from the crate's **own embedded decks**
(`crates/njoy-outram-park-fork/src/leapr/decks/`), used as like-for-like
oracles by the LEAPR tests. Like `../endf/`, `../errorr/` and `../covr/`,
they live at the repository root, outside `crates/`, so they are
git-tracked but never part of a published crate tarball. Read them through
`njoy_outram_park_fork::reference_data::reference_file("leapr", …)`.

These differ from the *published* evaluations in `../endf/` (which the
same decks produced, but with the LEAPR build and physical constants of
their day): here the generator is a known NJOY2016 commit, so every
difference is a port difference. Regenerate with `njoy < <deck>`; `tape24`
is the MF=7 tape.

## Provenance

All tapes: NJOY2016 upstream `ac5adf5` (2016.79), gfortran 13.3.0, built
2026-09-10; generated 2026-09-10. NJOY2016 at that commit uses
`bk = 8.617333262e-5 eV/K` (`phys.f90:22`, CODATA 2018); the crate's tests
regenerate with `LeaprDeck::with_constants(PhysicalConstants::Codata2018)`
because both decks carry pre-2017-10 `EVAL` dates and would otherwise be
regenerated with the legacy `bk` their published tapes used.

| Tape | Deck | What it exercises | Run |
|---|---|---|---|
| `tsl-SiO2-alpha-njoy2016-leapr.endf` (9,777 lines) | `tsl-SiO2-alpha.leapr`, unmodified: Si in α-quartz, MAT 47, 5 temperatures (293.6/350/400/500/800 K), `nphon` default | **Mixed moderator**: card 6 `1 0 15.862 7.4975 1` — oxygen as a short-collision-time secondary (`b7 = 0`), so LEAPR runs a second temperature loop over the oxygen's own spectrum with `alpha / (aws/awr)`, merges `S = S_Si + (sbs/sb) S_O`, and writes two `T_eff` TAB1s (`leapr.f90:399-408, 3013-3025, 3578-3617`). `twt = 0`, no oscillators, `iel = 0`. | 9.6 s |
| `tsl-HinH2O-293.6K-njoy2016-leapr.endf` (24,168 lines) | `tsl-HinH2O.leapr` (CAB model, MAT 1, `EVAL-JUN17`) cut to its 293.6 K block only (`ntempr = 1`; the full 18-temperature tape is 18.5 MB), `nphon = 200`, 222 α x 317 β | **`contin` + `trans` + `discre`** with a free-gas oxygen secondary (`b7 = 1`): translational weight and two discrete oscillators. NJOY's listing: `T_eff` 480.905 K after `contin`, 478.107 K after `trans`, 1194.341 K after `discre`; lambda 1.724930. | 6.3 s |

## Measured agreement (2026-09-10)

`tests/leapr_sio2_mixed_moderator_oracle.rs` — 43,449 tabulated
`S(alpha, beta)` points above the `smin` floor over the 5 temperatures
agree with the NJOY tape to **1.0e-13** relative (identical after the
tape's 7-figure rounding); both effective-temperature tables (principal
508.3411 K, secondary 486.4483 K at 293.6 K, … 896.2843 / 889.3055 K at
800 K) and the `contin` lambdas (1.893760 / 2.063704) match to every
printed figure. Regenerated with the deck's inferred (legacy-`bk`) vintage
instead, `T_eff` came out 4e-6 low, lambda 1.1e-5 high and the SCT tail
3e-4 off — the constants, not the merge.

`tests/leapr_h2o_njoy_oracle.rs` — 44,961 points at 293.6 K agree to
**1.0e-13**; `T_eff` 1194.341 K exactly; the `B(1..12)` list (free-gas
oxygen, `B(8) = mss·sps`, `B(9) = aws`) identical. Before this tape the
H-in-H2O chain had only been compared with the published evaluation
(σ_inel ~0.6 %, `T_eff` +0.09 %), which a different LEAPR build produced.

Data policy: derived products of open ENDF/B-VIII.0 evaluation inputs
processed with the BSD-licensed NJOY2016; no proprietary content.
