# Spectrum-aware depletion: V&V record (gh:#1631, epic gh:#1623)

> **Research, education and V&V only.** Not for reactor operation,
> licensing or safety decisions (`RESPONSIBLE_USE.md`).

Recorded 2026-11-17. AI-written; **not human-reviewed**. Every number below
was produced by the test named beside it, on the commit given in the
issue comment cited. Predictions were written in each test's doc comment
before it was run; where one failed, this record says so.

## What was built

| Piece | Module | Issue |
|---|---|---|
| Transfer and external source rates (OpenMC `TransferRates`, `ExternalSourceRates`) | `outram_mc_libs::depletion::external_rates` | #1622 |
| `Chain.from_endf`, `reduce`, `set_branch_ratios`, `export_to_xml` | `njoy_outram_park_fork::hdf5::{depletion_chain_from_endf, depletion_chain_ops}` | #1621 |
| Helper arithmetic (power normalisation, fission-yield choice) | `outram_mc_libs::depletion::helpers` | #1624 |
| `MicroXS`, `collapse_rate`, `get_microxs_and_flux` (flux mode) | `outram_mc_libs::depletion::{microxs, microxs_transport}` | #1625 |
| Isomeric branching from MF=8/9/10 on a spectrum | `njoy_outram_park_fork::isomeric_branching` | #1628 |
| The simple API, `SpectrumDepletion` | `outram_mc_libs::depletion::spectrum_depletion` | #1629 |
| Refresh criterion, tracking drivers, removal pair, chain regeneration | `outram_mc_libs::depletion::{tracking, spectrum_depletion}` | #1630 |

Data: ENDF/B-VIII.0 decay (3,821 tapes) and NFY (31) sublibraries and the
Th-232, Pa-233 and U-233 tapes, from NNDC, in `reference-data/ace` at
`9bd47b1` (provenance on #1627); the other tapes in `reference-data/endf/`.

## Verification: methodology and results

### Against analytic solutions

| Test | Reference | Result |
|---|---|---|
| `external_rates::tests` (11) | removal `N0 e^{-rt}`, feed `s/r(1-e^{-rt})`, two-material transfer and conservation, padded block system | ≤ 1e-11 relative |
| same | the old first-order `apply_feed` at `r dt = 0.1` | 5.083 % error (predicted ≈ x/2) |
| `microxs::tests` (6) | constant, linear and threshold σ; temperature interpolation | exact to 1e-15 |
| `helpers::tests` (5) | normalisation, yield choice and blending | exact |
| `isomeric_branching::tests` | Simpson-exact products; Zn-68 MF=9 constant below 1 eV | 1e-15; 0.934/0.066 to 1e-14 |
| `tracking::tests` | tolerance 0 = `deplete_coupled` bit for bit; ∞ freezes BOL; fixed spectrum gives c = 0 exactly | held |

### Code to code (two independent implementations in this workspace)

| Test | Comparison | Result |
|---|---|---|
| `tests/microxs_on_reconr_data.rs` | `collapse_rate` against the #1628 Simpson integrator, RECONR Zn-68 (n,γ), three group structures | 2.9e-16 to 1.5e-14 relative |

### Code to code against OpenMC artefacts (not OpenMC running)

| Test | Reference | Result |
|---|---|---|
| `depletion_chain_ops::tests` | OpenMC's `chain_ni.xml`, written by `Chain.export_to_xml` | export **byte for byte** |
| `tests/chain_from_endf_vs_openmc_chain.rs` | OpenMC's packaged `chain_endfb80` (from `from_endf` on the same decay sublibrary), all 3,820 nuclides | half-life and decay energy **exact on all** (predicted); decay modes equal on **92.3 %, prediction of ≥ 99 % FAILED**: 127 differ only in the spontaneous-fission target (current upstream sets none, the older OpenMC that built the package set one) and 168 only in branching ratios, ≤ 9.0e-7 (pin set after seeing it) |

### Demonstrations with a known sign

| Test | Prediction | Result |
|---|---|---|
| `tracking::tests::xenon_removal_has_the_known_sign` | Xe sparging lowers Xe-135 and Cs-135, raises `k_inf` | Xe-135 ×1/30, `k_inf` 1.8279 → 1.8551 |
| `tracking::tests::regenerated_chains_take_effect_at_refreshes` | I-135 capture to Xe-135 raises Xe-135, lowers Xe-136 | +5.1e-5, −8.8e-6 |
| `spectrum_depletion::tests::thorium_breeding_and_pa_removal` | U-233 bred from Th-232; Pa removal to a tank raises total U-233 | held; **+0.50 %** |
| `spectrum_depletion::tests::zinc68_isomer_branching_is_the_default` | Zn-69m share between 0.066 and 0.244; zero Zn-69m under presets | share 0.068839 (held); **zero under presets FAILED**: U-233 fission feeds Zn-69m (2.58e-15 vs 1.20e-9 by default) |

### Tracking on transport spectra

`spectrum_depletion::tests::tracking_on_transport_spectra`: U-235/U-238
pin, 5 × 30 d at 300 W/cm³, 400 × [8 + 20] histories per transport.
Tolerance 0 refreshes every step and ∞ equals `run` (both predicted). The
predicted convergence with tightening tolerance **FAILED**: never refreshing
put end-of-life U-235 6.5e-7 from the refresh-every-step result, one
refresh 1.2e-5. The criterion (0.006–0.042) is mostly Monte Carlo noise at
these statistics. Next step proposed on #1630: judge it against the
tally's own uncertainty.

## Defects found and fixed during the work

- `DepletionChain::from_chain_xml` drops a branched channel from the matrix
  entirely (deliberate, pinned); spectrum branching therefore removed the
  parent's capture. `from_chain_xml_with_branches` carries branched
  (n,γ)/(n,2n) at their ratios; every other constructor is unchanged
  (`tests/depletion_move_fingerprints.rs`).
- A finding in OpenMC, read from its source and not reproduced by running
  it: `ExternalSourceRates.reformat_nuclide_vectors` strips the last entry
  of any material with a source at any step, though only materials with a
  source at the current step are augmented. The port strips only
  augmented materials (pinned).

## Validation

None. No comparison against measurement or a benchmark has been made. The
thorium demonstration uses a stand-in spectrum (a CORE U pin): it shows
the API and the signs, not an LFTR result.

## What this record does not yet contain

- **OpenMC run as the reference** (gh:#1626): OpenMC's pincell depletion,
  a chain generated by OpenMC `e4febe9` itself, and its coupled operator as
  the reference for tracking. Needs OpenMC installed and run, which has not
  been approved in these sessions.
- `get_microxs_and_flux` direct mode, and the tally half of the helpers.
- A noise-aware refresh criterion, and tracking re-measured where the
  spectrum shift is resolved.
- `k` returned through the tracking path as a cross-check.
