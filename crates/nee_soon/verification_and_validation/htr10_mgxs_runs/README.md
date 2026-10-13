# HTR-10 MGXS case runs (gh:#845)

> **Research, education and V&V only.** Not for reactor operation, licensing
> or safety decisions (`RESPONSIBLE_USE.md`).

One folder per case of `nee_soon::htr10_mgxs::cases::MgxsCase::all`, written by
`examples/htr10_mgxs_case.rs`. [`INDEX.md`](INDEX.md) lists every case with its
numbers and is rewritten after each run. How to run them:
[`docs/htr10-mgxs-desktop-runbook.md`](../../docs/htr10-mgxs-desktop-runbook.md).

## Methodology (common to every case)

Each case condenses multigroup cross sections from a continuous-energy Monte
Carlo run of the full HTR-10 core, one zone per material:

1. ENDF/B-VIII.0 processed at the case temperature through the NJOY port
   (`htr10_rmc::data`), every material at that temperature (isothermal).
2. The full core of `htr10_rmc::core_model::assemble_explicit_triso` (Şeker &
   Çolak 2003's 13-ball bed, explicit TRISO, explicit reflector, rods
   withdrawn).
3. Two k-eigenvalue passes at one seed with the hybrid tracker: a scalar tally
   (`Flux, Total, Absorption, NuFission, KappaFission, Fission`) and a
   scattering-matrix tally with fission births, both `[Material, Energy]`
   binned on 70 groups (NJOY's WIMS-69 plus 10 to 20 MeV).
4. Condensation by `nee_soon::mgxs::condense`, raw (nothing rebalanced or
   adjusted), and exact collapse onto 2, 4 and 8 groups.

**What these runs are and are not.** They produce in-core MGXS and the
continuous-energy `k_eff` of the same run. They are **not** yet a verification
gate: the multigroup `k_eff` that #850 compares against the continuous-energy
one needs #848 (the converter to `physics_mg`) and #682 (scattering
multiplicity). ~~The saved libraries are what that gate reads, so these runs are
not repeated when it lands.~~

**CORRECTED 2026-10-13.** #848 and #682 have landed, and the converter
(`MgxsLibrary::to_physics_mg`) needs two things a record written before that
date does not carry:
- per-group standard errors (`ZoneMgxs::sigma`), to judge whether a scattering
  row agrees with `Sigma_t - Sigma_a`;
- the scatter-only matrix (`ZoneMgxs::scatter_only`), to form the (n,xn)
  multiplicity.

Such a record still loads (both fields default to empty), but the default
conversion refuses it (`NoUncertainty`, `NoScatterOnly`). It converts only
through explicit ablations: `rebalanced()` and
`to_physics_mg_without_multiplicity()`. So runs made before 2026-10-13 either
are repeated with the current tallies, or are read by the gate with both
ablations stated. Checked by
`mgxs::tests::a_library_without_sigma_must_balance_exactly` and
`no_scatter_only_is_refused_unless_ablated`. On HTR-10 the multiplicity is
expected to be small (C-12's (n,2n) opens at 18.7 MeV, U-238's at 6 MeV), but
that is not measured. Per-case checks (identical passes, no lost
histories, collapse conserves `k_inf`, every kernel group visited, agreement of
the 300 K `k` with the 2026-10-07 record) are in each `README.md`.

## Reference

The 300.15 K cases are compared with this crate's own earlier record, not with
an external benchmark:
`../htr10_seker_2026_10_07_10k/results_table.md` (N = 12, ENDF/B-VIII.0,
`k = 0.995125 ± 0.001055`).

The group structure is NJOY2016's built-in EPRI-CPM/WIMS 69-group table
(`groupr.f90`, `ign = 9`), as ported in
`njoy-outram-park-fork::errorr::groups`.

## Results

Per case, in each folder's `README.md`, and collected in `INDEX.md`.

### Tally verification on the full core (2026-10-13, `verify_tallies/`)

Run before any recorded case was handed to the run machine, to check that the
tallies are right on the real geometry, where the bed is delta-tracked and
tallied with the tentative-collision estimator. Budget 4000 × \[10 + 40\], one
seed, 4 threads, 2172 s. Every check passed:

| check | measured |
|---|---|
| the two passes ran identical histories | k 0.993080613 in both, 200 000 histories each |
| no history lost | 0 |
| `Sigma_f <= Sigma_a <= Sigma_t` in every visited bin | no violation |
| kernel `nu-bar` physical in every group | 70 groups, 2.4024 to 4.0859 |
| tallied production/absorption reproduces `k_eff` | 0.991404 (after leakage 1.65e-4) vs 0.993081 ± 0.003017: −168 pcm, 0.56σ |
| collapse conserves `k_inf` | to 9.0e-16 |
| every kernel group visited | 0 of 70 empty |
| `k` against the 2026-10-07 record | −204 pcm, −0.64σ |

The collision-weighted balance residual `|Sigma_a + Sigma_s,row - Sigma_t| / Sigma_t`
is 4.7e-4 on 2 groups, 6.4e-4 on 8 and 1.7e-3 on 70, so the scalar and matrix
tallies agree in the bulk of the collisions. The worst single bins (0.53 on 2
groups, 64 on 70) belong to sparsely visited zone/group pairs. On the small
pin cell of `tests/mgxs_from_monte_carlo.rs`, residuals of that kind fell
with the statistics when the budget was raised tenfold. The recorded cases'
budget is 7.75 times this one.

The same checks on the HEU/H-1 pin cell
(`tests/mgxs_from_monte_carlo.rs`, 4000 × \[10 + 100\]): tallied
`k_inf` against the driver's `k` −60 ± 195 pcm, `nu-bar` 2.43 to 2.61.

The within-run σ understates the true uncertainty, because the HTR-10's
dominance ratio is near 1 (#495).
