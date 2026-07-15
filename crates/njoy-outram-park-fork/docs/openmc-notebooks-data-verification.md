# OpenMC data-notebooks → njoy verification map

Part of the workspace direction (beads epic **op-6tz**, this crate's slice
**op-6tz.6**) that **every notebook in
[openmc-dev/openmc-notebooks](https://github.com/openmc-dev/openmc-notebooks)
becomes a verification test** as `outram-mc-libs` grows an OpenMC-like API. This
crate (`njoy-outram-park-fork`) owns the **data / cross-section-generation**
notebooks; the transport/geometry/tally/depletion notebooks belong to
`outram-mc-libs` (op-6tz.2–.5).

> **Status: AI-generated draft — human review required** per `RESPONSIBLE_USE.md`.
> The mapping below is a first-pass triage of what the current njoy API can and
> cannot verify. See `docs/ai-fleet-review/op-6tz-data/REVIEW_MANIFEST.md`.

## Provenance

- **Notebook source:** `github.com/openmc-dev/openmc-notebooks`, commit
  `cf1e5db2cd77d53a4fa76ffd9af7ab638f468713` ("Make notebooks compatible with
  OpenMC version 0.15.0", 2024-07-10).
- **License:** the openmc-notebooks are MIT-licensed (OpenMC project,
  open-source) — compatible with quoting/deriving verification cases here.
- **Data used in the live tests:** open-source ENDF/B-VIII.0 tapes shipped under
  `tests/resources/` and the embedded CORE WMP / MGXS blobs (ENDF/B-VII.1 / VIII.0,
  public). No restricted or proprietary data (per `DATA_POLICY.md`).

## This crate's notebook subset

| # | Notebook | njoy-owned aspect |
|---|---|---|
| 1 | `nuclear-data` | reading cross sections by MT, temperature dependence, secondary distributions |
| 2 | `nuclear-data-resonance-covariance` | ENDF MF=32 resonance covariance, parameter sampling, reconstruction |
| 3 | `search` | (transport-owned; njoy only supplies cross sections) |
| 4 | `mgxs-part-i` | multigroup group-collapse of cross sections |
| 5 | `mgxs-part-ii` | scatter matrices, Chi, per-nuclide MGXS |
| 6 | `mgxs-part-iii` | MGXS library export + MG-mode verification |
| 7 | `mdgxs-part-i` | delayed-neutron multigroup data (β, λ, χ_delayed) |
| 8 | `mdgxs-part-ii` | delayed-group condensation + library export |

## V&V methodology (how a "notebook test" is judged)

An OpenMC notebook is a *worked example*, not a benchmark with an accepted
answer. A njoy verification test therefore reproduces the **data operation** the
notebook performs (reconstruct a cross section, broaden it, collapse it to
groups, transform a covariance to a correlation) and checks the result against a
physical or analytical reference — the same standard as the crate's existing
`reconr_*`/`wmp_*` tests. Where the operation needs an API this crate does not
yet have (Monte-Carlo tally-based MGXS, MF=32 covariance sampling, delayed-group
data), the test is scaffolded `#[ignore]` with the missing capability named and
a per-notebook bead filed, rather than faked.

## Mapping table

Legend for **Tractable now?**: **LIVE** = a real test runs today; **PARTIAL** =
a sub-step is live, the full notebook is not; **IGNORE** = scaffolded
`#[ignore]`, gated on a named missing API.

### 1. `nuclear-data`

| Notebook operation | OpenMC API | njoy equivalent | Tractable now? | Notes |
|---|---|---|---|---|
| Load continuous-energy data | `IncidentNeutron.from_ace` / `from_hdf5` | `NuclearDataLibrary::from_file(path, mat).reconstruct(tol)`; `WmpLibrary::core().get(name)` | LIVE | njoy reconstructs from ENDF (RECONR) rather than reading an ACE/HDF5 blob; the *result* (pointwise σ(E)) is the same object. |
| Read σ by MT at energy(ies) | `nuc[MT].xs[T](E)` | `lib.total_xs/elastic_xs/fission_xs/capture_xs(E)`, `lib.xs_for_reaction(MtReaction, E)` | LIVE | Verified against accepted U-235 thermal values (585/99/15/699 b). |
| Temperature-dependent σ | `nuc[MT].xs['294K'](E)` | `lib.broaden(T)` (BROADR SIGMA1) and `WindowedMultipole::evaluate(E, T)` (analytic Doppler) | LIVE | Broadening lowers/​broadens a capture resonance monotonically with T. |
| Energy grid | `nuc.energy['294K']` | `ContinuousEnergyData` reaction `(E, σ)` tables | LIVE | Grid spans 1e-5–2e7 eV. |
| Reaction threshold, e.g. (n,2n) MT=16 | `nuc[16].xs` threshold | `MtReaction::Mt16N2n`; RECONR reconstructs the MF=3 background above threshold | PARTIAL | njoy carries the MT=16 background but not the ACE-style per-reaction threshold metadata object. |
| Secondary energy distributions (χ) | `nuc[18].products[...].distribution` | `FissionSpectrum::from_endf_mf5` (LF=1/7/9/11 + NK mixtures); `NuBar::from_endf` (ν̄) | PARTIAL | χ(E→E') and ν̄(E) are parsed; the notebook's *product/angular listing* object is not reproduced. |
| `atomic_mass`, `NATURAL_ABUNDANCE`, `atomic_weight` | `openmc.data.*` helper tables | — | GAP | No atomic-mass / abundance table in njoy. |

**Overall:** LIVE (partial coverage). The cross-section-reading, temperature, and
secondary-χ core is verifiable today; the ACE-file loader and atomic-data helper
tables are gaps.

### 2. `nuclear-data-resonance-covariance`

| Notebook operation | OpenMC API | njoy equivalent | Tractable now? | Notes |
|---|---|---|---|---|
| Load ENDF with covariance | `IncidentNeutron.from_endf(f, covariance=True)` | — (ENDF MF=32 reader not ported; `errorr::resprx` returns `NotPorted`) | IGNORE | Gd-157 ENDF/B-VII.1 in the notebook. |
| Resonance-parameter covariance matrix | `.resonance_covariance.ranges[0].covariance` | — (MF=32 parse absent) | IGNORE | |
| Covariance → correlation | (numpy, from the diagonal) | `covr::CovarianceMatrix::to_correlation`, `relative_std_dev` | **LIVE (partial)** | The covariance→correlation *math* (COVR `subroutine corr`) is ported and tested here against a hand-computed reference. |
| Sample parameters (multivariate normal) | `.sample(n_samples)` | — | IGNORE | Needs the covariance matrix + an MVN sampler. |
| Reconstruct σ from sampled params | `sample.reconstruct(energies)` | RECONR reconstructs from *nominal* params (`reconr`), but not from perturbed samples wired to MF=32 | IGNORE | RECONR reconstruction exists; the sampling→reconstruction loop does not. |

**Overall:** IGNORE (full workflow) with one LIVE partial (the correlation-matrix
transform).

### 3. `search`

| Notebook operation | OpenMC API | njoy equivalent | Tractable now? | Notes |
|---|---|---|---|---|
| Bisection criticality search | `openmc.search_for_keff` | — (a *transport* capability; `outram-mc-libs`) | IGNORE | Finds ~1926 ppm boron for a PWR pin cell. |
| Supply cross sections for the run | (implicit data load) | `NuclearDataLibrary` / WMP core for the pin-cell nuclides | PARTIAL | The only njoy-side dependency is "the fissile/absorber cross sections exist"; a live check confirms U-235 reconstructs. |

**Overall:** IGNORE — `search_for_keff` is transport (op-6tz transport slice); the
njoy contribution is limited to supplying cross sections.

### 4. `mgxs-part-i`

| Notebook operation | OpenMC API | njoy equivalent | Tractable now? | Notes |
|---|---|---|---|---|
| Define energy groups | `openmc.mgxs.EnergyGroups([0, 0.625, 20e6])` | `Mgxs::collapse` / `collapse_from_reconr` group boundaries | LIVE | Same 2-group structure usable. |
| MGXS objects (Total/Absorption/Scatter) | `mgxs.TotalXS`, `AbsorptionXS`, `ScatterXS` | `Mgxs` columns (total/elastic/fission/capture/nu_fission) | PARTIAL | njoy has the channels but not per-domain tally objects. |
| Tally + transport → group constants | tracklength tallies, `run()`, `get_xs()` | — (no MC transport/tally in njoy) | IGNORE | The *flux-weighted, self-shielded* MGXS needs a transport solve. |
| Collapse pointwise σ(E) → group σ_g | (implicit, from tallies) | `Mgxs::collapse_from_reconr(result, name, e_lo, nu, ang, spectrum)` / `Mgxs::collapse(...)` | **LIVE (partial)** | njoy's collapse is **fixed-spectrum** (Watt / 1-over-E / Maxwellian weight), **not** a solved flux — a genuine low-fidelity partial of the notebook's MGXS. |

**Overall:** IGNORE (full flux-solved MGXS) + LIVE partial (the group-collapse
primitive).

### 5. `mgxs-part-ii`

| Notebook operation | OpenMC API | njoy equivalent | Tractable now? | Notes |
|---|---|---|---|---|
| Group-to-group scatter matrix | `mgxs.ScatterMatrixXS(nu=True)` | — (needs MF=6 kinematics / GROUPR `getff`) | IGNORE | njoy's `Mgxs` carries diagonal σ_g only, no transfer matrix. |
| Fission spectrum Chi | `mgxs.Chi` | `FissionSpectrum` (pointwise χ) but not a group-collapsed Chi vector | IGNORE | |
| Per-nuclide + condensation, OpenMOC check | `by_nuclide`, `get_condensed_xs`, OpenMOC | — (transport verification) | IGNORE | |

**Overall:** IGNORE. Same GROUPR/​tally gap as part-i, plus the scatter matrix.

### 6. `mgxs-part-iii`

| Notebook operation | OpenMC API | njoy equivalent | Tractable now? | Notes |
|---|---|---|---|---|
| Build MGXS `Library`, export HDF5 | `openmc.mgxs.Library`, `build_hdf5_store` | `MgxsLibrary::pack` / `from_blob` (MGXL blob, not HDF5) | PARTIAL | njoy serialises a multigroup library (own MGXL format), but not the OpenMC HDF5 layout. |
| MG-mode transport, CE-vs-MG keff | `create_mg_mode`, run | — (transport) | IGNORE | keff 1.0236 (MC) vs 1.0305 (OpenMOC), 688 pcm. |

**Overall:** IGNORE (MG-mode verification is transport); MGXL↔HDF5 export mismatch
noted for the future OpenMC-format bridge.

### 7. `mdgxs-part-i`

| Notebook operation | OpenMC API | njoy equivalent | Tractable now? | Notes |
|---|---|---|---|---|
| Delayed ν̄·σ_f | `mgxs.DelayedNuFissionXS` | — (ENDF MF=1/455 delayed ν̄ not parsed) | IGNORE | `MtReaction::Mt455NubarDelayed` exists as an enum tag only; no parser. |
| Delayed χ | `mgxs.ChiDelayed` | — (MF=5/455 not parsed) | IGNORE | |
| Delayed fraction β | `mgxs.Beta` | — | IGNORE | 6 delayed groups, U-235 & Pu-239. |
| Precursor decay constants λ | `mgxs.DecayRate` | — (MF=1/455 λ not parsed) | IGNORE | Notebook shows λ ≈ 0.013–2.85 s⁻¹. |

**Overall:** IGNORE. njoy has only total ν̄ (MF=1/452), no delayed chain.

### 8. `mdgxs-part-ii`

| Notebook operation | OpenMC API | njoy equivalent | Tractable now? | Notes |
|---|---|---|---|---|
| Delayed-group condensation + β per group | `mgxs.Library(num_delayed_groups=6)` | — | IGNORE | Same delayed-data gap as part-i. |
| Precursor concentration, library export | tally arithmetic, `add_to_tallies_file` | — (transport + delayed data) | IGNORE | keff 1.0325 in the notebook. |

**Overall:** IGNORE. Same delayed-data gap as part-i.

## Summary

| Notebook | Verdict | Live sub-step verified today |
|---|---|---|
| nuclear-data | **LIVE (partial)** | σ-by-MT, Doppler broadening, WMP temperature interpolation, MF=5 χ |
| nuclear-data-resonance-covariance | IGNORE + LIVE partial | covariance → correlation transform |
| search | IGNORE + partial | U-235 cross sections reconstruct (data availability) |
| mgxs-part-i | IGNORE + LIVE partial | fixed-spectrum group collapse to the notebook's 2-group grid |
| mgxs-part-ii | IGNORE | — |
| mgxs-part-iii | IGNORE | — |
| mdgxs-part-i | IGNORE | — |
| mdgxs-part-ii | IGNORE | — |

### Named gaps (each a bead under op-6tz.6)

1. **Atomic-data helper tables** — `atomic_mass`, `NATURAL_ABUNDANCE`,
   `atomic_weight` (nuclear-data notebook).
2. **ENDF MF=32 resonance-parameter covariance** — reader + multivariate-normal
   sampler + reconstruct-from-sampled-parameters loop (resonance-covariance).
3. **Flux-solved / self-shielded MGXS** — the GROUPR numeric group-averaging
   engine and/or tally-based MGXS, including the group-to-group scatter matrix
   and group Chi (mgxs-part-i/ii/iii).
4. **Delayed-neutron data** — ENDF MF=1/455 (delayed ν̄ + precursor decay
   constants λ) and MF=5/455 (delayed χ), plus delayed-group MGXS (mdgxs-part-i/ii).
5. **`search_for_keff`** — criticality search is a transport capability
   (`outram-mc-libs`); njoy's role is limited to supplying cross sections
   (cross-track note).
