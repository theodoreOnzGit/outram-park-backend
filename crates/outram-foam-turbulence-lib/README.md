# outram-foam-turbulence-lib

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping pass" command). A crate is **complete** only once the maintainer has personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.


> **This is OUTRAM PARK's independent Rust translation of selected OpenFOAM®
> algorithms.** It is not the official OpenFOAM® software and is not
> affiliated with, endorsed by, or sanctioned by OpenCFD Ltd. or the ESI
> Group. OpenFOAM® is a registered trademark of OpenCFD Limited — see
> [`TRADEMARKS.md`](./TRADEMARKS.md) for the full attribution and
> non-affiliation notice. Translated from
> [`OpenFOAM/OpenFOAM-dev`](https://github.com/OpenFOAM/OpenFOAM-dev),
> `master` branch — no commit is pinned (translation was done by reading the
> C++ source directly, not from an ongoing codegen-from-clone pipeline); see
> `upstream_source/README.md` for the full provenance record.
> **CORRECTED 2026-11-27** — that holds for the six original closures. The
> models added since gh:#1377, #1544 and #1545 (realizable and RNG k-ε,
> Launder-Sharma, `kOmegaSSTLM`, LRR, SSG, the DES family, WALE, `kEqn`,
> `dynamicKEqn`, `dynamicLagrangian`) are ported from ESI OpenFOAM-v2506, as
> their module docs state.

Pure-Rust port of the OpenFOAM turbulence model library, part of the
**OUTRAM PARK** (Open-source Unified TRAnsient Multi-Physics Advanced Reactor
simulation Kit) suite.

Provides RAS (Reynolds-Averaged Simulation) and LES (Large Eddy Simulation)
turbulence closures for use with `outram-foam-appbuilder-lib` solver loops.

**Name.** `outram` is the workspace's OUTRAM: **O**pen-source **U**nified
**TR**Ansient **M**ulti-Physics (from OUTRAM PARK, `docs/ecosystem-naming.md`);
`foam` marks the OpenFOAM lineage; `turbulence-lib` is its scope. **The
question it answers:** what exactly does each OpenFOAM turbulence closure add
to the momentum equation, term by term?

**What this crate lets you study and challenge.** A turbulence model in
OpenFOAM is a string in a dictionary; here it is a Rust type whose
coefficients are named fields and whose production, destruction and
`SuSp` terms can be printed per cell through `source_terms`. That makes the
closures comparable term by term: against ESI OpenFOAM-v2506 on a frozen
360-cell duct, the record shows where the original port's arithmetic departs
from upstream (`G` without $-\frac{2}{3} (\nabla \cdot U)^2$, the SST limiter,
the Spalart-Allmaras sigma) and that the ESI forms match bit for bit. What the
closures have **not** got is equally visible: no integrated wall-function
boundary conditions and no turbulence benchmark, so the Re = 100 cavity that
develops ν_t/ν near 300 is recorded in `outram-foam-appbuilder-lib` as a
warning.

## Knowledge in this crate

See the workspace [vision](../../VISION.md).

| Knowledge artifact | In this crate |
|---|---|
| Scientific literature | Upstream code: OpenFOAM (GPL-3.0), Foundation [`OpenFOAM-dev`](https://github.com/OpenFOAM/OpenFOAM-dev) `master` for the original six closures, ESI OpenFOAM-v2506 for the later ports and as the comparison build. Model papers cited in the module docs: Menter (1994), Jones and Launder (1972), Wilcox, Spalart and Allmaras (1992), Smagorinsky (1963), Shih et al. (1995), Yakhot et al. (1992), Launder and Sharma (1974), Langtry and Menter, Launder, Reece and Rodi (1975), Speziale, Sarkar and Gatski (1991), Strelets (2001), Nicoud and Ducros (1999), Yoshizawa and Horiuti (1985), Kim and Menon (1995), Meneveau, Lund and Cabot (1996). No `crates/kovan-literature/CATALOGUE.md` entry is tied to this crate. |
| Mathematical equations | Module docs of each model in `src/` (eddy-viscosity relations, k / ε / ω / ν̃ transport sources, the Reynolds-stress production tensor, LES filter widths in `les/deltas.rs`); a model-by-model summary in [`docs/model-reference.md`](docs/model-reference.md). |
| Source code | `traits`, `source_terms`, `laminar`, `k_epsilon`, `k_epsilon_step`, `k_omega`, `k_omega_sst`, `k_omega_sst_lm`, `realizable_ke`, `rng_k_epsilon`, `launder_sharma_ke`, `spalart_allmaras`, `reynolds_stress` (`lrr`, `ssg`), `des`, `les` (`smagorinsky`, `wale`, `k_eqn`, `dynamic_k_eqn`, `dynamic_lagrangian`, `deltas`), `wall_functions`. |
| Interactive simulations | None yet. |
| Visualisations | None yet. |
| Verification and validation evidence | [`verification_and_validation/upstream_turb_gh1362/`](verification_and_validation/upstream_turb_gh1362/README.md) (2026-11-13/14, 360-cell graded duct, frozen fields, against the real ESI v2506 models): k-ε, k-ω and Spalart-Allmaras every term 360/360 bit-identical in the ESI form; later slices (realizable and RNG k-ε, DES, LES, Launder-Sharma, LES filters, `dynamicKEqn`, `dynamicLagrangian`) report every listed term bit-identical, max 0 ulp. [`verification_and_validation/upstream_wallfn_gh1362/`](verification_and_validation/upstream_wallfn_gh1362/README.md) (2026-11-13): `y_plus_lam` 3/3 and `nu_t_wall_esi_v2506` 108/108 bit-identical; the older `nu_t_wall` 42/108. Tests: `tests/openfoam_turbulence_code_to_code.rs`, `tests/openfoam_wall_function_code_to_code.rs`. No turbulence benchmark. |
| Human reviews and corrections | 0 functions carry a kovan human-review stamp (no `kind = "review"` block in any `review.md` here). No entry in `docs/human-corrections-to-ai-work.md` names this crate. The struck-through "The formulae match upstream" below (gh:#1362) is the notable correction. |

**Status:** the source terms of the ported closures have AI-run code-to-code
evidence against ESI v2506 on one frozen field; no closure is validated
against a turbulence benchmark, and nothing here has been reviewed by a human.

## `no_std` + `alloc` (gh:#1595)

The default `std` feature can be turned off: the library is then `no_std` +
`alloc` and builds for bare metal
(`cargo check --release -p outram-foam-turbulence-lib --no-default-features --target x86_64-unknown-none`,
gated by `scripts/check-nostd.sh`). `std` turns on `outram-foam-basic-lib`'s and `thiserror`'s `std`. With `std` on (the default)
nothing changes.

## Turbulence models

| Model | Type | C++ class | Status |
|---|---|---|---|
| k-ω SST (Menter 1994) | RAS two-equation | `kOmegaSST` | Implemented + unit-tested |
| Laminar | RAS no-op | `laminar` | Implemented + unit-tested |
| k-ε (Jones & Launder 1972) | RAS two-equation | `kEpsilon` | Implemented + unit-tested |
| k-ω (Wilcox) | RAS two-equation | `kOmega` | Implemented + unit-tested |
| Spalart-Allmaras (1992) | RAS one-equation | `SpalartAllmaras` | Implemented + unit-tested |
| Smagorinsky (1963) | LES sub-grid | `Smagorinsky` | Implemented + unit-tested |

**ADDED 2026-11-27** (checked in `src/`; the table above listed six models):

| Model | Type | C++ class (ESI v2506) | Status |
|---|---|---|---|
| Realizable k-ε (Shih et al. 1995) | RAS two-equation | `realizableKE` | `TurbulenceModel` implemented; terms compared with ESI (gh:#1377) |
| RNG k-ε (Yakhot et al. 1992) | RAS two-equation | `RNGkEpsilon` | `TurbulenceModel` implemented; terms compared with ESI (gh:#1377) |
| WALE (Nicoud & Ducros 1999) | LES sub-grid | `WALE` | `TurbulenceModel` implemented; terms compared with ESI |
| kEqn (Yoshizawa & Horiuti 1985) | LES one-equation | `kEqn` | `TurbulenceModel` implemented; terms compared with ESI |
| Launder-Sharma low-Re k-ε (1974) | RAS two-equation | `LaunderSharmaKE` | source terms only, no `TurbulenceModel` impl (gh:#1544) |
| `kOmegaSSTLM` (Langtry-Menter γ-Reθ) | RAS transition | `kOmegaSSTLM` | source terms only (gh:#1544) |
| LRR (1975), SSG (1991) | Reynolds-stress | `LRR`, `SSG` | source terms only (gh:#1544) |
| SA-DES / DDES / IDDES, `kOmegaSSTDES` (Strelets 2001) | DES | `SpalartAllmarasDES` family, `kOmegaSSTDES` | terms only, compared with ESI on a frozen duct (gh:#1545) |
| `dynamicKEqn` (Kim & Menon 1995), `dynamicLagrangian` (Meneveau et al. 1996) | LES dynamic | `dynamicKEqn`, `dynamicLagrangian` | source terms only (gh:#1544) |

All models share the `TurbulenceModel` trait (static dispatch via generics).
Every model implements the full trait (`div_dev_rho_reff`, `correct`, `nu_t`,
`alpha_eff`, `mu_eff_field`) and is unit-tested — none `todo!()`-panic. "Unit-tested"
means the formulae/coefficients/positivity are checked against the upstream
OpenFOAM source and analytical values; it does **not** mean benchmark-validated
(see Limitations). ~~The formulae match upstream.~~ **CORRECTED 2026-11-13** (gh:#1362):
on a frozen field against ESI v2506 the default arithmetic differs in `G` (no
`-(2/3)(div U)^2`), the `div U` `SuSp` terms, SST gamma/`GbyNu` limiter, SA sigma and the
wall distance; the ESI form is available as `source_terms(SourceForm::EsiV2506, ..)`
(`verification_and_validation/upstream_turb_gh1362/README.md`).

## Limitations

This is an early ~~**0.1.0**~~ **0.1.6** release (CORRECTED 2026-11-27, `Cargo.toml`). Read this section before depending on the
crate — the models are unit-tested translations, not benchmark-validated code.

**Model coverage (what actually runs):**

- **All six original closures are implemented and unit-tested** (CORRECTED 2026-11-27: ten models implement the trait since the additions above). `KOmegaSST`, `KEpsilon`,
  `KOmega` (Wilcox), `SpalartAllmaras`, `Smagorinsky`, and `LaminarModel` each
  implement the full `TurbulenceModel` trait — `div_dev_rho_reff`, `correct`,
  `nu_t`, `alpha_eff`, and `mu_eff_field` are all real (no `todo!()` stubs). Each
  was translated line-by-line from the OpenFOAM-dev C++ source with the upstream
  `File.C:line` provenance cited in the module docs, and carries unit tests for
  its constitutive formula and positivity/stability. See each module's "Honest
  scope" section for exactly what was and was not reproduced.
- **Unit-tested ≠ validated.** No model has been validated end-to-end against a
  published turbulence benchmark; that requires a working solver loop and
  turbulence wall-function BCs (tracked in `outram-foam-appbuilder-lib`). Treat
  every model as untrusted draft material until a V&V case clears it.
- ~~**No other closures.** No realizable/RNG k-ε, no k-ω SST variants (SST-SAS,
  DDES/IDDES), no other LES sub-grid models (dynamic Smagorinsky, WALE,
  k-equation eddy viscosity), no Reynolds-stress (RSM) or transition (γ-Reθ)
  models, no DES/hybrid RANS-LES.~~ **CORRECTED 2026-11-27** — realizable and
  RNG k-ε, DDES/IDDES, WALE, kEqn, dynamic LES models, LRR/SSG Reynolds-stress
  and γ-Reθ transition are now present (table above; several as source terms
  only). Still absent, checked by listing `src/`: SST-SAS and dynamic
  Smagorinsky.

**Compressibility:**

- **Incompressible only.** The implemented k-ω SST path treats ν_t as a
  kinematic viscosity and forms μ_eff = μ + ν_t with no density weighting
  (`src/k_omega_sst/mod.rs`). Although the `TurbulenceModel` trait doc mentions
  a compressible counterpart, no ρ-weighted (compressible) closure is provided.

**Wall functions:**

- **Standalone utilities, not integrated boundary conditions.**
  `src/wall_functions/mod.rs` provides free functions `y_plus`, `u_tau`
  (log-law Newton iteration), and `nu_t_wall` (nutWallFunction). They are **not**
  wired into any model as patch boundary conditions — `KOmegaSST::correct`
  applies zero-gradient boundaries to k, ω, and ν_t rather than wall functions.
- **No k / ε / ω wall functions.** Only a ν_t (nutWallFunction-style) log-law
  is provided; there are no `kqRWallFunction`, `epsilonWallFunction`, or
  `omegaWallFunction` equivalents. The wall-function constants (κ = 0.41,
  E = 9.8, y⁺_lam = 11) are hard-coded, and the functions are ~~untested~~
  **CORRECTED 2026-11-27** — tested: `nu_t_wall` and `nu_t_wall_esi_v2506`
  against ESI v2506 (`tests/openfoam_wall_function_code_to_code.rs`).

**Validation status:**

- **Unverified until validated** (see the banner above). ~~The only tests are
  three formula-level unit tests inside `src/k_omega_sst/mod.rs` (wall-distance
  computation, the stress-limiter formula, and k/ω positivity over five steps).~~
  **CORRECTED 2026-11-27** — 48 `#[test]`s sit across the model modules in
  `src/`, and two code-to-code integration tests compare with ESI v2506
  (`tests/`). There is **no end-to-end CFD validation** against any benchmark (e.g.
  channel flow, backward-facing step, NACA0012); the crate ships ~~no
  integration tests,~~ no examples or benchmarks.
- End-to-end validation additionally requires a working solver loop and
  turbulence wall-function boundary conditions, both external to this crate and
  tracked in `outram-foam-appbuilder-lib`.

**Scope boundaries:**

- **Turbulence closures only.** This crate provides the closure models and the
  turbulent-stress / effective-viscosity terms they contribute. The PISO/PIMPLE
  solver loop, pressure-velocity coupling, mesh construction, and field
  boundary-condition machinery live in `outram-foam-basic-lib` (Layers 1–4) and
  the solver crates (Layer 5, e.g. `outram-foam-appbuilder-lib`), not here.
- **Wall distance is brute-force.** `compute_wall_distance` does an O(cells ×
  wall-faces) nearest-face search per construction (`src/k_omega_sst/mod.rs`) —
  fine for small meshes, not for large ones.

## License

~~GPL-3.0-only~~ **AGPL-3.0-only** combined work (relicensed 2026-10-20, gh:#973; ported files keep GPL-3.0 and `LICENSE.GPL-3.0` ships beside `LICENSE`; see `docs/relicensing-agpl-2026-10-20.md`). The ported files follow OpenFOAM licensing (GPL-3.0).

## Copyright

Copyright (C) 2026 Ong Kay Chen Theodore, Professor Per F. Peterson,
University of California, Berkeley Thermal Hydraulics Lab,
Singapore Nuclear Research and Safety Institute (SNRSI),
National University of Singapore (NUS), Repository Contributors.
