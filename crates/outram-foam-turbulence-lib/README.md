# outram-foam-turbulence-lib

<!-- vv-unverified-banner -->
> ⚠️ **Unverified until validated.** All code in this workspace is **unverified and untrusted** unless a specific verification & validation (V&V) case demonstrates otherwise. V&V cases are human-reviewed and are intended for journal / arXiv publication — that is the trust workflow. See the workspace `VERIFICATION_AND_VALIDATION.md` and `RESPONSIBLE_USE.md`. Not for nuclear facility operation, reactor control, safety-critical, or licensing decisions.


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

Pure-Rust port of the OpenFOAM turbulence model library, part of the
**OUTRAM PARK** (Open-source TRAnsient Multi-Phase Advanced Reactor simulator
Kit) suite.

Provides RAS (Reynolds-Averaged Simulation) and LES (Large Eddy Simulation)
turbulence closures for use with `outram-foam-appbuilder-lib` solver loops.

## Turbulence models

| Model | Type | C++ class | Status |
|---|---|---|---|
| k-ω SST (Menter 1994) | RAS two-equation | `kOmegaSST` | Implemented + unit-tested |
| Laminar | RAS no-op | `laminar` | Partial (see Limitations) |
| k-ε (Jones & Launder 1972) | RAS two-equation | `kEpsilon` | Scaffold only (`todo!()`) |
| k-ω (Wilcox 1988) | RAS two-equation | `kOmega` | Scaffold only (`todo!()`) |
| Spalart-Allmaras (1992) | RAS one-equation | `SpalartAllmaras` | Scaffold only (`todo!()`) |
| Smagorinsky (1963) | LES sub-grid | `Smagorinsky` | Scaffold only (`todo!()`) |

All models share the `TurbulenceModel` trait (static dispatch via generics).
Only **k-ω SST** implements the full trait today; the other structs and their
model coefficients exist but their `correct()` / `div_dev_rho_reff()` /
`alpha_eff()` / `mu_eff_field()` methods `todo!()`-panic if called.

## Limitations

This is an early **0.1.0** release. Read this section before depending on the
crate — several advertised models are scaffolds, not working code.

**Model coverage (what actually runs):**

- **Only the k-ω SST model is implemented.** `KOmegaSST` implements the full
  `TurbulenceModel` trait — `div_dev_rho_reff`, `correct`, `nu_t`, `alpha_eff`,
  and `mu_eff_field` are all real — with F1/F2 blending, the Bradshaw stress
  limiter, and the k and ω transport equations (`src/k_omega_sst/mod.rs`).
- **`KEpsilon`, `KOmega`, `SpalartAllmaras`, and `Smagorinsky` are scaffolds.**
  Each has a constructor and its model constants, but every trait method except
  `nu_t()` is a `todo!()` stub that **panics at runtime** if called
  (`src/k_epsilon/mod.rs`, `src/k_omega/mod.rs`, `src/spalart_allmaras/mod.rs`,
  `src/les/smagorinsky.rs`). Constructing them is safe; using them in a solve
  is not.
- **`LaminarModel` is partial.** `correct` (no-op), `nu_t`, `alpha_eff`, and
  `mu_eff_field` work, but `div_dev_rho_reff` — the momentum stress term — is
  still `todo!()` (`src/laminar/mod.rs`), so even the laminar closure cannot be
  driven end-to-end through the trait yet.
- **No other closures.** No realizable/RNG k-ε, no k-ω SST variants (SST-SAS,
  DDES/IDDES), no other LES sub-grid models (dynamic Smagorinsky, WALE,
  k-equation eddy viscosity), no Reynolds-stress (RSM) or transition (γ-Reθ)
  models, no DES/hybrid RANS-LES.

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
  E = 9.8, y⁺_lam = 11) are hard-coded, and the functions are untested.

**Validation status:**

- **Unverified until validated** (see the banner above). The only tests are
  three formula-level unit tests inside `src/k_omega_sst/mod.rs` (wall-distance
  computation, the stress-limiter formula, and k/ω positivity over five steps).
  There is **no end-to-end CFD validation** against any benchmark (e.g.
  channel flow, backward-facing step, NACA0012); the crate ships no
  integration tests, examples, or benchmarks.
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

GPL-3.0-only (follows OpenFOAM licensing).
