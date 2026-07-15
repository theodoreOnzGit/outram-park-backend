# op-3ut — GROUPR/GAMINR matrix path — REVIEW MANIFEST

> **⚠️ AI-GENERATED DRAFT — HUMAN REVIEW REQUIRED per RESPONSIBLE_USE.md**
>
> Every source and test file listed here was produced by AI agents (Claude Opus
> 4.8, a lead + four parallel porting subagents) and is **untrusted draft
> material until a human reviews it**. It has unit tests and self-consistency
> (property) checks, but **no piece has been validated against a real NJOY
> GROUPR/GAMINR GENDF golden tape**. Do not treat any number here as
> authoritative until the golden-file V&V step is done.

## Scope

Ports the NJOY2016 GROUPR/GAMINR **matrix path** (bead **op-3ut**), extending the
previously-landed vector group-average engine (op-cjw.15). Touches only
`crates/njoy-outram-park-fork/`. Mirrors Fortran at
`/home/teddy0/Documents/research/NJOY2016/src/{groupr.f90,gaminr.f90}`.

- **NJOY2016 commit:** `ac5adf5f33d893e42f2eed7fb286b0d51c7580da`
- **Upstream licence:** modified BSD 3-Clause (LANL/DOE), GPL-compatible; these
  derivative files ship `GPL-3.0-only` with the provenance header block intact.
- **Data:** ENDF/B-VIII.0 U-235 (MAT 9228), open-source, from
  `tests/resources/n-092_U_235-ENDF8.0.endf`.

## Files changed

| File | Lines | Author | What |
|---|---|---|---|
| `src/groupr/mod.rs` | edit | lead | wire 3 new modules (`matrix`, `unresolved`, `gaminr_matrix`) |
| `src/groupr/kinematics.rs` | 1139 | subagent A | CM->lab kinematics (`cm2lab`/`f6cm`/`f6ddx`/`bach`) |
| `src/groupr/matrix.rs` | 882 | subagent B | scatter-matrix assembly + elastic feed + GENDF matrix records |
| `src/groupr/unresolved.rs` | 862 | subagent D | URR self-shielding + Bondarenko flux (`genflx`/`stounr`/`getunr`) |
| `src/groupr/gaminr_matrix.rs` | 946 | subagent E | GAMINR photon-production matrices (`gpanel`/`dspla` matrix branch) |
| `tests/openmc_notebooks_data/mgxs_part_i.rs` | edit | lead | wire op-6tz.6.3 live scatter-matrix verification test |

All four ported files carry the 4-line GPLv3/NJOY provenance header; a grep for
`Box<`, `dyn `, `lifetime` finds none (enum dispatch, no trait objects, no
lifetimes, per the design rules).

## DONE vs PARTIAL/stub (honest)

### A — `kinematics.rs` (CM->lab kinematics)
- **DONE (ported, line-cited):** `legndr` (Legendre recurrence), `bach`
  (Kalbach-86 slope, incl. natural-element->dominant-isotope map + neutron
  `d1/sqrt(E')` enhancement), `f6ddx` (continuum LANG=1 Legendre-in-CM and LANG=2
  Kalbach), `f6cm` (continuum path, ND=0), `cm2lab` (adaptive lab-energy march +
  `sum≈1` normalization check). Public entry `cm2lab(&Cm6Emission, nl)`.
- **NotPorted (return `NjoyError::NotPorted`, range-cited):** `f6dis`
  (8719-8810, discrete deltas), `ll2lab` (8934-9061, LAW-7), `f6lab` (9063-9338,
  lab-frame LAW-1); also `f6psp` (LANG=0 phase space) and tabulated LANG>=11 are
  out of the input enum's scope.

### B — `matrix.rs` (scatter-matrix assembly)
- **DONE:** `FeedFunction::{Identity, TwoBodyElastic{awr}}` (the `getdis`
  isotropic-CM elastic kernel: GL-8 CM-cosine quadrature + two-body kinematics),
  `scatter_matrix(...)` (the matrix reduction of `panel` producing
  `matrix[ig_in][il][ig_out]`), `ScatterMatrix::{element, p0_row_sum,
  to_gendf_section}` (mf=6 matrix GENDF layout, IG2LO>0, NG2>2).
- **NotPorted:** `FeedFunction::AnisotropicElastic` (File-4 Legendre CM angular
  distribution), `FeedFunction::Continuum6` (File-6 continuum via
  `getmf6`/`cm2lab`/`f6lab`); charged-particle Coulomb + `wcut` truncation.

### D — `unresolved.rs` (URR self-shielding)
- **DONE:** `genflx_bondarenko` / `bondarenko_flux_value` (P0 flux
  `phi = C(E)(sigma_0+sigma_pot)/(sigma_t(E)+sigma_0)`, one `GroupFlux::Tabulated`
  per dilution), `UnresolvedTable::{store, shield}` + `terpu` (getunr sigma-zero
  interpolation, `iovl==0` path).
- **NotPorted:** PENDF URR-tape reading (`read_urr_from_pendf` ->
  `NotPorted`, `stounr` findf/contio/listio 6822-6875; `store` accepts
  pre-parsed input per the task boundary), `genflx` slowing-down/heterogeneity
  branch (5396-5620), resolved/unresolved overlap (`iovl`/`xtot`, 6961-6989).

### E — `gaminr_matrix.rs` (photon-production matrices)
- **DONE:** `PhotonProductionSpectrum` (File-15 continuous distribution,
  normalized), `PhotonFeed::{Continuous, PairProduction}`,
  `photon_production_matrix` (`gpanel` matrix loop + `dspla` matrix-branch
  division producing `matrix[ig_neutron][ig_photon]` P0),
  `photon_matrix_gendf_section` (mfh=26, IG2LO>0, NG2>2).
- **NotPorted:** discrete File-12 photon lines; the photon-*interaction* `gtff`
  branches (coherent mt=502, incoherent/Klein-Nishina mt=504, 1259-1486); live
  `gaminm` ENDF/PENDF tape control flow.
- **Reviewer note (E):** E flagged that `gaminr.f90`'s own `gtff` (1162-1514) is
  photon-*interaction*, not neutron->photon *production*; E ported the
  production-feed physics (yield x normalized spectrum) which is standard ENDF,
  and the matrix-reduction quadrature which is line-traceable to `gpanel`/`dspla`.
  Confirm this framing is acceptable.

## Build & test output (measured)

Release only, via the 12 GB-capped `scripts/test.sh` (mandatory). NJOY2016 commit
ac5adf5, date 2026-07-15.

- `cargo build -p njoy-outram-park-fork --release` — **green**.
- New per-module unit tests (all pass):
  - `groupr::kinematics` — 8 passed. `cm2lab` P0 integral 1.0000596 (Legendre) /
    1.0018403 (Kalbach), within 1% `sum≈1`; `bach` slopes positive & monotone.
  - `groupr::matrix` — 7 passed. Detailed balance / sum-to-vector max deviation
    2.0e-10 (A=12 constant XS); elastic kinematic banding (A=12 alpha=0.71598,
    A=1 no up-scatter); GENDF matrix round-trip.
  - `groupr::unresolved` — 8 passed. Flux limits (sigma_0->inf flat=1.0;
    sigma_0->0 gives phi*sigma_t=sigma_pot); self-shielding monotone in dilution;
    getunr/terpu interpolation exact at nodes.
  - `groupr::gaminr_matrix` — 8 passed. **Note:** these 8 were authored by E but
    never executed in E's environment (its worktree lacked the crate); they were
    first compiled and executed during lead integration and **pass** as of
    2026-07-15. E's doc-comment "Result" lines still read "Expected (NOT
    executed)" — they are now executed and green; the labels understate rather
    than overclaim.
- **op-6tz.6.3 live verification** — `mgxs_part_i::groupr_elastic_scatter_matrix_u235`
  (real U-235 ENDF/B-VIII.0, 1/E weight, notebook 2-group `[1e-3, 0.625, 2e7]` eV,
  A=233.0248): **passes**. Measured:
  - P0 matrix `[0->0]=13.9178, [0->1]=0 (2.4e-19), [1->0]=0.00684, [1->1]=9.73566` b.
  - Fast-group row sum `9.74251` b == vector elastic `9.74251` b (rel < 1e-6):
    detailed balance / sum-to-vector.
  - Thermal-group row sum `13.9178` b vs vector `13.9373` b: 0.14% leaks below the
    1e-3 eV floor (physically expected).
  - No up-scatter (thermal->fast fraction 2.4e-19); GENDF mf=6 matrix section
    round-trips.
- Full-suite regression count: see `full_test.log` in the op-3ut scratchpad /
  recorded in the bead note. No pre-existing test regressed.

## Human-verify list (top asks)

1. **cm2lab kinematics correctness (A) — TOP ASK.** Property tests only
   (normalization, kinematic edges, Kalbach monotonicity) — **no NJOY numeric
   oracle**. Cross-check `bach` slopes and lab distributions against a real NJOY
   GROUPR run. Confirm the stateless `f6ddx` bracketing search reproduces NJOY's
   `save`-cursor `epnext` semantics on histogram (`lep=1`) data.
2. **Elastic kinematics correctness (B) — TOP ASK.** Verify the CM->lab map,
   `ast = sqrt(awr2) = A` reduction, and the `mu_of(ep)` inversion of
   `getdis:9480`. This is the load-bearing physics of the scatter matrix.
3. **Golden-file gap (ALL) — TOP ASK.** Nothing here is validated against a real
   NJOY GENDF tape. Produce/locate a golden GROUPR GENDF (e.g. U-238 elastic
   matrix, or an H-1 scatter matrix) and compare. Until then these are
   self-consistency checks only.
4. **E: compile-then-run provenance.** E's file was written without ever being
   compiled in E's environment; it compiled and passed cleanly on integration,
   but treat as freshly-integrated draft — re-read for subtle logic.
5. **E: production-vs-interaction framing** (see E note above).
6. **D: flux normalization + reaction-column mapping + overlap** — confirm the
   NJOY `genflx` normalized form, the `UrrReaction` ix=1..5 order, and that not
   applying `iovl` overlap is acceptable for the intended nuclides.
7. **op-6tz.6.3 remaining gap:** the *self-shielded* (Bondarenko-dilution) MGXS
   and group **Chi** are still not wired end-to-end — they need the URR PENDF
   feeder (D's NotPorted boundary) and a fission matrix, or transport tallies.
   `flux_weighted_self_shielded_mgxs` remains `#[ignore]`.

## Provenance / worktree note

The four subagents ran in isolated worktrees that (due to a base-commit
mismatch) did not all contain the njoy crate; each authored its file and handed
it off via a shared scratchpad collection dir. The lead integrated every file
into the `agent-af5a551f2cfdfd0e2` worktree from the base commit `9a713a0`
(vector engine + skeleton) and ran the authoritative build+test. Subagents A, B,
D verified their files against the correct base; E did not (see ask 4).
