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

### Code to code against OpenMC run on the same files

| Test | Reference | Result |
|---|---|---|
| `tests/chain_from_endf_vs_openmc_run.rs` | OpenMC `e4febe9`'s own `Chain.from_endf`, run 2026-11-17 on the same decay/NFY sublibraries and (full) the Th-232, Pa-233, U-233, U-235, U-238 tapes; `openmc_inputs/ref_chain.py`, decay spectra removed by `strip_sources.py`; outputs in `openmc_outputs/` | **all 3,820 nuclide records equal, export byte-identical, both chains** (predicted). This supersedes the packaged-chain comparison above as the reference for `from_endf`: the packaged chain was built by an older OpenMC |
| `tests/external_rates_vs_openmc_run.rs` (gh:#1622) | OpenMC `e4febe9`'s own `pool.deplete` with `TransferRates` / `ExternalSourceRates`, `chain_simple.xml`, two materials, one 1-day step, CRAM-48; `openmc_inputs/ref_transfer_rates.py`, output `openmc_outputs/transfer_rates.tsv` | transfer, removal, redox: worst **2.8e-15** (predicted ≤ 1e-10). Feed: **1.5e-8, prediction failed**, diagnosed to PETIR's dense CRAM-48 on the badly scaled augmented system (OpenMC's own CRAM-48 is 1.3e-11 from `expm` there); gh:#1645 |
| `tests/independent_operator_vs_openmc_run.rs` (gh:#1626 arm 1) | OpenMC `e4febe9`'s `IndependentOperator` with all eight integrators on its own `deplete_no_transport` regression inputs (`openmc_inputs/micro_xs_simple.csv`, `chain_simple_tests.xml`), three 5-day steps, `source-rate` (1.0) and `fission-q` (174 W), CRAM-48; `openmc_inputs/ref_independent_operator.py` with `libopenmc` built, output `openmc_outputs/independent_operator.tsv` | step-0 rates **bit-identical** in all 16 runs; atoms: 480 entries, 69 bit-identical, worst **4.7e-15** (predicted ≤ 1e-10). The residual is the CRAM-48 linear solve (dense elimination against SuperLU); below the 1e-9 allowance, flagged for the maintainer |
| `tests/transfer_rates_vs_openmc_regression.rs` (gh:#1626 arm 3) | the committed results of OpenMC's own `deplete_with_transfer_rates` regression (CoupledOperator + Predictor, 1 day, 174 W or 0 W; Xe removal/feed/transfer, Gd-157 redox, 0.1 g/s natural-U source), dumped by `openmc_inputs/dump_transfer_regression.py` to `openmc_outputs/transfer_regression.tsv`; the port is fed OpenMC's beginning-of-step rates | 11 of 12 replayed (one file has no rates): worst **2.0e-15** without a source, **1.0e-13** with (predicted ≤ 1e-10 and ~1e-8). The files predate `e4febe9`; agreement at round-off says the transfer-rate algebra has not moved since |
| `tests/pincell_depletion_vs_openmc.rs` (gh:#1626 arm 2) | OpenMC `e4febe9`'s `examples/pincell_depletion` (Predictor, 5 x 1 d, 174 W/cm, 1000 x [10+90]) on one ACE library made by the port's NJOY (`openmc_inputs/pincell_ace_to_hdf5.py`, `ref_pincell_depletion.py`), 16 seeds in flux mode (CCFE-709) and 16 in direct mode; the port: `depletion::coupled_operator` (flux mode), `examples/pincell_depletion_vs_openmc.rs`, 16 seeds | OpenMC's rates through the port's Predictor/CRAM-48: 160 steps, worst **4.1e-15**. Port vs OpenMC flux mode: **0 of 41** k/atom comparisons above 3σ, worst **1.76σ** (all predicted). ~~Recorded, not resolved: U-238 low at every step (−1.9 to −2.8σ)~~ **CORRECTED:** that first run had U-234 fission at zero (a defect in `reaction_from_ace`, found by the `collapse_rate` comparison below); re-run after the fix, U-238 is −0.6 to −1.3σ. Direct vs flux at day 5: Xe-135 −0.19 %, under the predicted 0.5 % |
| `tests/collapse_rate_vs_libopenmc.rs` (gh:#1625) | libopenmc `e4febe9`'s own `Nuclide::collapse_rate` on the pincell library, CCFE-709, a fixed made-up flux, 9 nuclides × MT 102/18/16 (`openmc_inputs/ref_collapse_rate.py`, `openmc_outputs/collapse_rate.tsv`); gated on `OUTRAM_PINCELL_ACE_LIB` | **27 of 27 bit-identical** (predicted). Found a port defect (U-234's redundant MT=18 missing, fixed) and an upstream one, reproduced on purpose: rates go negative for flux above the nuclide's grid (gh:#1646) |

OpenMC provenance: OpenMC `e4febe9` (MIT, `openmc_inputs/LICENSE.openmc`),
installed as a pure-Python package in a Python 3.11 venv (`--ignore-requires-python`;
upstream asks for 3.12), with numpy, h5py, scipy, pandas, lxml,
uncertainties and endf from PyPI. For `from_endf` and the transfer rates
`libopenmc` was not needed, and `openmc.lib` was loaded through upstream's
documentation-build mock (`READTHEDOCS=True`). For the independent
operator `libopenmc` was built from the same commit (CMake Release, no MPI,
no tests; system HDF5 1.10.10, pugixml, fmt from apt) and copied into the
package; `OPENMC_CROSS_SECTIONS` unset, which the transport-free operator
does not read. Command:

```bash
READTHEDOCS=True python openmc_inputs/ref_chain.py \
  reference-data/ace/endf/endf-b-viii.0 reference-data/endf full.xml \
  n-090_Th_232-ENDF8.0.endf n-091_Pa_233-ENDF8.0.endf n-092_U_233-ENDF8.0.endf \
  n-092_U_235-ENDF8.0.endf n-092_U_238.endf
python openmc_inputs/strip_sources.py full.xml openmc_outputs/full_nosrc.xml
```

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

- ~~**OpenMC transport as the reference** (gh:#1626): OpenMC's pincell
  depletion and its coupled operator as the reference for tracking. These
  need `libopenmc` built (with HDF5) and a cross-section library.~~
  **Done 2026-11-17** (arm 2 above). Still open: tracking (#1630) against
  OpenMC. ~~and the U-238 signal arm 2 recorded~~ (resolved: a U-234 defect, fixed). ~~A chain
  generated by OpenMC `e4febe9` itself~~ **done 2026-11-17**, above.
- `get_microxs_and_flux` direct mode, and the tally half of the helpers.
- A noise-aware refresh criterion, and tracking re-measured where the
  spectrum shift is resolved.
- `k` returned through the tracking path as a cross-check.
