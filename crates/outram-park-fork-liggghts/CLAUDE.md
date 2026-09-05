# CLAUDE.md — outram-park-fork-liggghts

Pure-Rust granular-DEM library: particles, contact mechanics, thermal DEM,
pebble/packed-bed physics. Ports LIGGGHTS/LAMMPS-granular.

> This crate is a member of the **OUTRAM PARK** workspace
> (`crates/outram-park-fork-liggghts`). See the workspace root `CLAUDE.md` for
> the shared dependency policy. Dependencies are inherited from the root
> `[workspace.dependencies]` — do not pin versions in this crate's
> `Cargo.toml`.

## Maturity: DECLARED MATURE (2026-09-06)

The API-usability rules in the root `CLAUDE.md` ("Human interface layer", and
the Haiku dogfooding hard rule) **are in force for this crate**. See the
maturity gate in that file for what this means and how the bar is revised.

Declared because LIGGGHTS/DEM is now on the HTR-10 pebble-bed critical path
(`op-jyyp`) and on the HTGR calibration path (gh #148), so its public API needs
the same quality bar — **not** because its granular physics is validated. Those
are different claims and this entry keeps them apart.

- **2026-09-06 — mature.** Bar: the **numerics** are verified against
  closed-form solutions. The integrator reproduces free flight under gravity as
  the analytical parabola and `omega(t) = (tau / I) * t` for angular
  acceleration; the Hertz/Hooke/Mindlin contact force laws are checked against
  **hand-computed analytical values**; signed-distance functions for plane,
  wall, box, cylinder and sphere are checked at known points (after I. Quilez,
  "Distance functions"). Evidence class: **analytical / manufactured
  solution**, supported by unit tests and internal consistency.

  Measured at declaration: **72 tests pass, 0 fail, 0 ignored.**

  **What this bar does NOT cover, and the distinction matters.** What is
  verified is the *algebra and the integrator*: given a contact law, is it
  evaluated correctly; given forces, is the trajectory integrated correctly.

  What is **not** validated is the **granular physics** — packing fractions,
  bulk flow behaviour, heat transfer through a bed, wall effects, or agreement
  with LIGGGHTS proper on any case. There is no cross-code comparison against
  upstream LIGGGHTS and no experimental comparison in this repository.

  **Do not cite this declaration as evidence that a DEM-settled pebble bed is
  physically right.** A correct integrator over an uncalibrated contact model
  produces a precisely-computed wrong answer. Closing that gap needs a
  cross-code run against LIGGGHTS proper or a published granular benchmark,
  and neither exists yet.

  Sphere packing does **not** live in this crate — it is in
  `outram-mc-libs/src/pebble_beds/crp_packing.rs` (Jodrey-Tory CRP). Whether it
  belongs here instead is an open architectural question (gh #65); DEM-settled
  beds and CRP-generated beds are different things carrying different validity
  claims.

  Version is `0.0.0` and the crate has **no prelude** at declaration time. Both
  tracked in gh #64. Absent prelude was the single most predictive defect
  across the seven crates dogfooded in gh #58 — the two worst failures there
  were both prelude failures — so this crate starts from behind.

## Scope

Modules: `bonded`, `boundary`, `contact`, `coupling`, `mesh_wall`, `particle`,
`rolling`, `simulation`, `thermal`, `thermal_radiation`.

## Licensing

LIGGGHTS-PUBLIC is GPL-2-or-later (GPL-3-compatible; see `NOTICE`). This
translation is distributed under `GPL-3.0-only`, the same as the rest of the
workspace.
