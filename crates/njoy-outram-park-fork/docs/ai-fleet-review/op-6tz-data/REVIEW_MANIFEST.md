# REVIEW MANIFEST — OpenMC data-notebooks as njoy verification tests (op-6tz.6)

**⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per `RESPONSIBLE_USE.md`**
(untrusted until human inspection + the live tests are re-run and the mapping
judgements are checked against the actual notebooks).

This pass builds the notebook→test→required-API mapping for this crate's slice of
the openmc-notebooks verification effort (the **data / cross-section-generation**
notebooks) and scaffolds the test harness: tractable operations as **live**
tests, the rest `#[ignore]` with a named missing capability and a per-notebook
bead. No physics was faked — every gap is honestly `#[ignore]`d and panics with
its missing-API reason if force-run with `--ignored`.

- **Date:** 2026-07-15 (Asia/Singapore, within working hours — Wednesday 10:06).
- **Notebook oracle:** `github.com/openmc-dev/openmc-notebooks`, commit
  `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` (MIT-licensed, OpenMC project).
- **Integration base:** `develop` (`bd52224`). Changes are confined to
  `crates/njoy-outram-park-fork/` (tests, one docs file, one docs manifest, and a
  `[[test]]` stanza in `Cargo.toml`). No `src/` change — the library is
  byte-identical to develop, so the 274-test lib baseline is unaffected by
  construction.
- **Data:** open-source ENDF/B-VIII.0 fixtures already in `tests/resources/` and
  the embedded CORE WMP blob. No new data files, no restricted data
  (`DATA_POLICY.md`).

## Files changed / added

| File | Kind | Notes |
|---|---|---|
| `docs/openmc-notebooks-data-verification.md` | new doc | The notebook→API mapping table (per-notebook: op exercised, njoy equivalent/GAP, tractable-now?, notes). |
| `Cargo.toml` | edited | Added `[[test]] name = "openmc_notebooks_data"` (directory target; the dir name collides with file autodiscovery otherwise — same reason `u238_doppler_verification` is explicit). No dependency or version change. |
| `tests/openmc_notebooks_data/main.rs` | new | Harness: `mod` for each notebook. |
| `tests/openmc_notebooks_data/nuclear_data.rs` | new | **4 live** tests. |
| `tests/openmc_notebooks_data/nuclear_data_resonance_covariance.rs` | new | **1 live** (covariance→correlation) + 2 ignored. |
| `tests/openmc_notebooks_data/search.rs` | new | **1 live** (data availability) + 1 ignored. |
| `tests/openmc_notebooks_data/mgxs_part_i.rs` | new | **1 live** (group-collapse primitive) + 1 ignored. |
| `tests/openmc_notebooks_data/mgxs_part_ii.rs` | new | 2 ignored. |
| `tests/openmc_notebooks_data/mgxs_part_iii.rs` | new | 2 ignored. |
| `tests/openmc_notebooks_data/mdgxs_part_i.rs` | new | 3 ignored. |
| `tests/openmc_notebooks_data/mdgxs_part_ii.rs` | new | 2 ignored. |
| `docs/ai-fleet-review/op-6tz-data/REVIEW_MANIFEST.md` | new | This file. |

## Live vs ignored (and why)

**7 live, 13 ignored.** Live tests reproduce a data operation the notebook
performs and check it against a physical/analytical reference. Ignored tests name
the missing API and file a bead; each panics with its reason if force-run.

| Notebook | Live | Ignored (gated on) |
|---|---|---|
| nuclear-data | 4: σ-by-MT; σ over energy array; WMP temperature-Doppler; ν̄ + χ | — |
| nuclear-data-resonance-covariance | 1: covariance→correlation transform | 2: ENDF MF=32 reader; parameter sampling + reconstruct |
| search | 1: fuel XS availability | 1: `search_for_keff` (transport, outram-mc-libs) |
| mgxs-part-i | 1: 2-group collapse primitive | 1: flux-solved/self-shielded MGXS |
| mgxs-part-ii | — | 2: scatter matrix; group Chi |
| mgxs-part-iii | — | 2: HDF5 MGXS export; MG-mode k-eff (transport) |
| mdgxs-part-i | — | 3: MF=1/455 λ; delayed ν̄/β; MF=5/455 χ_delayed |
| mdgxs-part-ii | — | 2: delayed-group condensation; precursor concentration (transport) |

## Actual build & test output (run by this agent, from the worktree)

```
cargo test -p njoy-outram-park-fork --release --test openmc_notebooks_data (12 GB cap)
  → test result: ok. 7 passed; 0 failed; 13 ignored; finished in 27.14s
cargo test -p njoy-outram-park-fork --release --lib (baseline, unchanged src)
  → test result: ok. 274 passed; 0 failed; 0 ignored
```

### Measured results of the live tests (2026-07-15, ENDF/B-VIII.0)

- **U-235 σ at 0.0253 eV:** σ_f = 586.3 b, σ_γ = 99.3 b, σ_el = 14.1 b,
  σ_t = 699.7 b — matches accepted 2200 m/s values (≈585 / 99 / 15 / 699 b) to a
  few percent. Also σ_f over [0.0253, 1, 10, 1000] eV = [586.3, 67.0, 14.0, 3.2] b
  (1/v thermal peak, decaying above).
- **U-235 ν̄(0.0253 eV) = 2.4299** (accepted ≈2.44); **MF=5/18 χ mean outgoing
  energy at E_in=1 MeV = 2.025 MeV** (physical fission range).
- **U-238 6.67 eV capture peak (WMP analytic Doppler):** 0 K = 22 257 b,
  294 K = 7 109 b, 1000 K = 4 283 b — monotonically decreasing with temperature,
  as Doppler broadening requires.
- **U-235 collapsed to the notebook 2-group `[0, 0.625, 20e6]` eV (1/E weight):**
  thermal σ_t = 1071.4 b, fast σ_t = 35.2 b — the expected 1/v-absorber
  thermal-dominant profile. (Note: fixed-spectrum collapse, **not** OpenMC's
  flux-solved MGXS — a deliberate partial.)

Uncertainty: these are deterministic reconstructions/collapses, so there is no MC
statistical uncertainty; the "±" is the tolerance band asserted in each test
(e.g. σ_f within ±25 b of 585). The reference is the accepted textbook thermal
value / analytical Doppler monotonicity, not a benchmark k-eff.

## Beads filed (under op-6tz.6)

- **op-6tz.6.1** — nuclear-data: atomic-data helper tables (atomic_mass /
  NATURAL_ABUNDANCE / atomic_weight).
- **op-6tz.6.2** — resonance-covariance: ENDF MF=32 covariance reader +
  resonance-parameter sampling + reconstruct-from-sampled.
- **op-6tz.6.3** — mgxs: flux-solved/self-shielded MGXS (GROUPR engine or tally
  MGXS) + scatter matrix + group Chi.
- **op-6tz.6.4** — mdgxs: delayed-neutron data (MF=1/455 λ+ν̄, MF=5/455
  χ_delayed) + delayed-group MGXS.
- **op-6tz.6.5** — search: cross-track note (search_for_keff is transport;
  njoy supplies XS only).

## What a human should verify

1. **Mapping accuracy.** The notebook contents were read via a summarizing fetch,
   not cell-by-cell. Spot-check the mapping table against the actual notebooks
   (esp. which OpenMC MGXS classes each part uses) before trusting the GAP calls.
2. **Live-test tolerances.** The asserted bands (e.g. σ_f ±25 b, ν̄ ±0.15) are
   engineering tolerances, not derived uncertainties — confirm they are neither
   too loose (masking a regression) nor too tight (flaky).
3. **The "PARTIAL" framings.** The mgxs-part-i live test verifies a
   *fixed-spectrum* collapse, explicitly **not** OpenMC's flux-solved MGXS.
   Confirm the doc comments make that limitation unmissable so no one reads a
   green test as "njoy reproduces OpenMC MGXS".
4. **Ignored-test honesty.** Each ignored test panics with its missing-API reason
   if run with `--ignored`; confirm none silently "passes".
5. **U-238 in CORE WMP.** `temperature_dependent_xs_doppler` skips cleanly if
   U-238 is absent from the embedded CORE set; on this build it ran (U-238
   present). Confirm this is acceptable (a skip is not a failure).
