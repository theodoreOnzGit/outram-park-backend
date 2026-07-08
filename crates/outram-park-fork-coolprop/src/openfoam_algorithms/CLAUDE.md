# CLAUDE.md — openfoam_algorithms (vendored into outram-park-fork-coolprop)

## Provenance

This directory is a **verbatim copy** of
`tampines-steam-tables/src/openfoam_algorithms/` — the pure-Rust OpenFOAM
finite-volume primitives (`openfoam_source/`: matrix, PCG, DIC, GAMG, MUSCL, FV
operators, fields, mesh, thermophysics) plus the 1-D compressible solvers
(`rhoPimpleFoam/`, `driftFluxFoam/`, …). It was brought in so the CoolProp fork
can host the same transient-flow backbone, driven by the CoolProp Helmholtz EOS
instead of the IAPWS-IF97 steam tables.

**It depends only on `uom`** (no `ndarray` / BLAS / C, and — deliberately — no
`openfoam-basic-lib`): the numerical primitives are copied in as source, so this
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

- The `rhoPimpleFoam` array is renamed and EOS-wired (see above). Other vendored
  solvers (`driftFluxFoam`, `chtMultiRegionTwoPhaseEulerFoam`, `simplefoam`)
  still carry their as-copied form and are not yet wired to the CoolProp EOS.
- `correct_thermo` is **single-phase** (no saturation/VLE) and does not update
  transport (`μ`, `αh`) — CoolProp has no transport properties yet.
- Once the solvers are fully wired, drop the module-level
  `#![allow(dead_code)]` / `unused_imports` on the vendored tree.
