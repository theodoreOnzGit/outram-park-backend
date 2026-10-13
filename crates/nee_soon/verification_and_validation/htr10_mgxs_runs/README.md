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
multiplicity). The saved libraries are what that gate reads, so these runs are
not repeated when it lands. Per-case checks (identical passes, no lost
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

Per case, in each folder's `README.md`, and collected in `INDEX.md`. No case
has been run yet (2026-10-12).
