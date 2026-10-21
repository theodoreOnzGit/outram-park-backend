# ACE route: transport channels by OpenMC's redundancy rule, fission from the partials (GitHub #366)

*2026-09-29. Research, education and V&V only; not for any operational,
licensing or safety use (see `RESPONSIBLE_USE.md`).*

## What was wrong

The five-route ICSBEP campaign
(`verification_and_validation/icsbep/five_route_keff_2026_09_29.md`) found that
outram-mc reading ACE (routes 3 and 5) sat **+1853 ± 37 pcm** (Godiva) and
**−923 ± 40 pcm** (Jemima) away from OpenMC reading the *same* tables (route 1).
The cause was two defects in `Nuclide::from_ace`:

1. **MT=4 replaced the inelastic levels.** The transport channels came from
   `CeNeutronAce::channel_mts`, the rule for reconstructing the *total*, which
   keeps a lump and drops its components. U-235's ACE table lists MT=4
   (`LQR = 0`, no law) beside MT=51..91, so the 39 levels and the MT=91
   continuum were dropped, and every inelastic collision was sampled from MT=4
   as a continuum with **Q = 0** (no energy loss).
2. **U-234 could not fission.** Its ACE table carries only the partials
   MT=19/20/21/38 (no MT=18), and the pointwise tier reads fission from MT=18.
   Also, chi was "first fission law wins", which is the first-chance spectrum
   only.

## Upstream, read first

- `openmc/data/neutron.py:634-640`: a reaction is **redundant** if any of its
  `SUM_RULES` components is present (recursively), and redundant reactions are
  never sampled. So MT=4 is redundant beside MT=50..91, and MT=18 is redundant
  beside the partials.
- `openmc/data/reaction.py:1073-1079`: each partial fission gets the table's
  single NU block and its own DLW law. OpenMC picks a fission reaction in
  proportion to its cross section.

## The fix

- `CeNeutronAce::transport_channel_mts` (njoy-outram-park-fork) applies OpenMC's
  rule to the families that carry secondary-neutron physics:
  - it keeps the levels and drops MT=4;
  - it keeps the partials and drops MT=18.

  For the absorption families it keeps the lump (MT=103 over 600..649, MT=107
  over 800..849), which is a documented difference from OpenMC. It does not
  change transport: none of those levels emits a neutron, and outram-mc reads
  them through the lump.
- `Nuclide::from_ace` builds its sections from that list. The partials are
  collapsed into one MT=18 section equal to their sum (`ce_decode::fission_xs`).
- **Chi for a partial-fission table** is a `FissionSpectrum::Mixture` of the
  partials' own laws. Partial `k` has weight `sigma_k(E) / sum_j sigma_j(E)`,
  tabulated on the table's grid. That is exactly OpenMC's
  pick-a-partial-then-its-law distribution at every grid point. Between grid
  points it differs to second order in the spacing, because the ratio of two
  lin-lin functions is not itself lin-lin. That difference is stated in the doc
  comment.
- The ENDF route already preferred the levels (`build_inelastic_levels`), so the
  two routes now build the same channel structure.

## Methodology

`crates/outram-mc-libs/tests/ace_transport_channels_vs_openmc.rs`. The reference
is OpenMC's own reader (0.16.1.dev25, `d7d3284a1`) of the same files, via
[`openmc_inputs/transport_channels_reference.py`](openmc_inputs/transport_channels_reference.py)
→ [`data/transport_channels_openmc.csv`](data/transport_channels_openmc.csv).
Libraries:
- `reference`: the NJOY2016 tables in the `reference-data/ace` submodule, which
  are byte-identical to the campaign's NJOY2016 U-234/235/238;
- `njoy` and `rust`: the campaign's NJOY2016 and Rust-NJOY tables.

1. **Channel list.** `transport_channel_mts()` must equal OpenMC's non-redundant
   list for U-234/235/238. The comparison maps only the documented
   absorption-lump difference.
2. **Levels reach transport.** `inelastic_levels_table()` on U-235 must list
   MT=51..89 and MT=91, each with its ACE `Q`, and no MT=4.
3. **Fission cross section.** `xs_at_energy(E).fission` must equal OpenMC's
   transported value at 6 energies (thermal to 14 MeV) to within `1e-10`
   relative. Both sides are lin-lin interpolation of the same grid values.
4. **U-234 chi.** The mean of `N = 4e5` sampled prompt fission-neutron energies
   must lie within **5σ** of the exact mean of OpenMC's scheme. The exact mean
   is computed from OpenMC's reader as the sum of partial weight × the
   ContinuousTabular mean, with statistical interpolation and envelope scaling.
5. **The tests can fail.** Reverting the three changes in the source (back to
   `channel_mts`, no MT=18 synthesis, no chi mixture) fails tests 2, 3 and 4.
   This was run and then reverted.
6. **k_eff parity.** The committed driver `icsbep_five_route_keff` ran routes 3
   and 5 at the campaign settings: 32 seeds, 5000 × [40 + 120], 4 threads,
   binary built at `a922711f53` plus this change. Outputs went to
   `target/ace_parity_366/`, not the campaign's data directory. Route 1 and
   route 4 values are from the campaign record.

## Results (2026-09-29)

**Unit/integration checks, identical on all three libraries:**
- **Channel lists:** equal to OpenMC's for U-234 (49), U-235 (47) and U-238 (47).
- **U-235 levels:** 40 inelastic channels, `MT=51 Q = −77.0 eV`, and no MT=4.
- **Fission cross section:** equal to OpenMC's at every probe energy, relative
  difference 0.0 as printed. For example, U-234 at 1 MeV is 1.0894840 b; before
  the fix it was 0.
- **U-234 chi mean (MeV):**

  | E_in | outram-mc | OpenMC mixture | z | MT=19-only (the old behaviour) | its z |
  |---|---|---|---|---|---|
  | 1 MeV | 1.97197 | 1.97295 | −0.40 | 1.97295 | — |
  | 7 MeV | 1.86302 | 1.86419 | −0.46 | 2.10843 | −95.8 |
  | 14 MeV | 1.92166 | 1.91925 | +0.91 | 2.11226 | −72.2 |
  | 19 MeV | 2.06669 | 2.06690 | −0.07 | 2.19622 | −44.9 |

**k_eff.** Each Δ is `mean − reference`, with σ = √(sem² + sem_ref²). All runs
are 32 seeds.

| case | route | k ± sem | seed sd [pcm] | Δ vs route 1 (OpenMC) [pcm] | Δ vs route 4 (outram-mc, ENDF) [pcm] |
|---|---|---|---|---|---|
| Godiva | 3 | 0.99962 ± 0.00035 | 195 | −54 ± 40 (1.3σ) | +29 ± 49 |
| Godiva | 5 | 0.99962 ± 0.00035 | 195 | −54 ± 40 (1.3σ) | +29 ± 49 |
| Jemima | 3 | 0.99718 ± 0.00034 | 191 | +124 ± 38 (3.3σ) | +19 ± 44 |
| Jemima | 5 | 0.99718 ± 0.00034 | 191 | +124 ± 38 (3.3σ) | +19 ± 44 |
| HST-009 | 3 | 1.00002 ± 0.00033 | 189 | −232 ± 40 (5.8σ) | +40 ± 43 |
| HST-009 | 5 | 0.99889 ± 0.00030 | 168 | −345 ± 37 (9.3σ) | −73 ± 40 |
| LCT-008s | 3 | 0.85157 ± 0.00032 | 180 | +50 ± 37 (1.4σ) | +79 ± 44 |
| LCT-008s | 5 | 0.84929 ± 0.00027 | 154 | −178 ± 33 (5.4σ) | −149 ± 40 |

Before the fix, routes 3 and 5 were +1853 (Godiva) and −923 (Jemima).

**LCT-008s loose end: route 5 − route 3 = −228 ± 44 pcm (paired over seeds).**
- The uranium, O-16 and H-1 tables of the two libraries were compared through
  OpenMC's reader. They agree on every cross section, Q value and distribution
  on the probed grid; only heating (MT=301) differs, and transport does not read
  it.
- Swapping the **Rust-NJOY H-in-H₂O S(α,β) table** for NJOY2016's, with
  everything else Rust-NJOY (including the 0 K companions), reproduces route 3
  **seed for seed** (paired Δ = 0 ± 0).
- So the whole −228 pcm is that one thermal table. Its incoherent-inelastic
  cross section differs from NJOY2016's by up to 7 % at the lowest energies
  (500.54 b against 467.53 b at the first point) and by 0.16 % median. It is a
  **Rust-NJOY THERMR/ACER data difference, not the ACE reader**.
- HST-009 route 5 − route 3 (−113 ± 45) is the same kind of water case. That
  swap was not run, so its cause is not established.

## Interpretation

- **Defects 1 and 2 are fixed.** On Godiva, Jemima, HST-009 and LCT-008s,
  outram-mc on NJOY2016 ACE (route 3) now agrees with outram-mc on ENDF
  (route 4) within 1.8σ. The two routes share no data-processing code, so the
  ACE reader is no longer the source of any difference measured here.
- **Parity with OpenMC holds on Godiva and LCT-008s** (within 2σ).
- **It does not hold on Jemima** (+124 ± 38, 3.3σ) **or HST-009** (−232 ± 40,
  5.8σ). The ENDF route shows the same residuals (+106 ± 34 and −271 ± 35,
  #367), so these are outram-mc transport or physics differences common to both
  data routes, not ACE reading. They are left to the five-route analysis.
- **Route 5 on water cases additionally carries the Rust-NJOY H-in-H₂O table
  difference.** That is a data-processing finding for Rust-NJOY's thermal path.
