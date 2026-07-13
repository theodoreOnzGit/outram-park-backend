# CLAUDE.md — openfoam_algorithms (vendored into outram-park-fork-coolprop)

## Provenance

This directory is copied from
`tampines-steam-tables/src/openfoam_algorithms/` — the pure-Rust OpenFOAM
finite-volume primitives (`openfoam_source/`: matrix, PCG, DIC, GAMG, MUSCL, FV
operators, fields, mesh, thermophysics) plus the 1-D compressible
`rhoPimpleFoam/` solver. It was brought in so the CoolProp fork can host the same
transient-flow backbone, driven by the CoolProp Helmholtz EOS instead of the
IAPWS-IF97 steam tables. The other vendored solvers that shipped in the tampines
copy (`driftFluxFoam/`, `simplefoam/`, `chtMultiRegionTwoPhaseEulerFoam/`) were
**deleted** — only the EOS-wired `rhoPimpleFoam` path is kept here.

**It depends only on `uom`** (no `ndarray` / BLAS / C, and — deliberately — no
`outram-foam-basic-lib`): the numerical primitives are copied in as source, so this
crate carries them itself and stays Android-buildable.

## Differences from the tampines copy

- The tampines copy's branch policy ("commit only on `feature/validation`", the
  Edwards–O'Brien blowdown V&V contract, RELAP5 references) is **tampines-
  specific and does not apply here** — it was intentionally dropped. Follow the
  workspace-root and crate-root `CLAUDE.md` for this crate instead.
- The array is renamed `TampinesSteamArray` → **`OPCPFluidArray`** and carries a
  `fluid: Fluid`; its `correct_thermo` now does a per-cell single-phase `(p, h)`
  flash on the CoolProp Helmholtz EOS (`crate::flash`), updating `ρ`, `T` and
  `ψ = (∂ρ/∂p)_T` — not the steam tables, and not the old placeholder `ρ = ψ·p`.

## Rules that still apply (workspace directives)

- No `Box<dyn Trait>` — enums for dispatch. No `Box<T>` (except recursive).
- No lifetime parameters on structs/traits/impls — own by value or `Arc<T>`.
- `Arc<RwLock<T>>` over channels for shared simulation state.
- Every public item documented (physical quantity, valid range, units); named
  `uom` type aliases for complex quantities.

## Remaining follow-ups (bead op-kbc)

- The `rhoPimpleFoam` array is renamed and EOS-wired (see above); the other
  vendored solvers were deleted, so `rhoPimpleFoam` is the only solver here.
- `correct_thermo` is **single-phase** (no saturation/VLE) and does not update
  transport (`μ`, `αh`) — CoolProp has no transport properties yet.
- Parts of `openfoam_source` are still unused by the single kept solver; once
  `rhoPimpleFoam` fully exercises them, drop the module-level
  `#![allow(dead_code)]` / `unused_imports` on the vendored tree.
