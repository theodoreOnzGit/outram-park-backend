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

- **2026-09-15 — cross-code leg added; the 2026-09-06 entry above stands as
  what was accepted before.** The "no cross-code comparison against upstream
  LIGGGHTS" gap it records is now **closed**; the "no experimental comparison"
  gap is **not**. Upstream LIGGGHTS-PUBLIC (commit `3d5c00f2`) was **built from
  source and run**; its trajectories are committed under
  `reference-data/liggghts/` (same standard as `reference-data/gsl/` in
  `petir`). Evidence class: **cross-code comparison**, on top of the
  analytical/MMS leg above.

  Bar: reproduce upstream LIGGGHTS to floating-point round-off on the
  deterministic contact cases, and to better than 2 % bulk packing fraction on
  a settled bed. Measured — every sampled frame compared, not just endpoints:

  | Case | Frames | agreement |
  |---|---|---|
  | head-on, Hertz | 251 | **bit-identical** |
  | head-on, Hooke | 251 | **bit-identical** |
  | wall bounce + gravity (400 000 steps) | 2001 | **bit-identical** |
  | rolling resistance, CDT (`µ_r = 0.1`) | 201 | **bit-identical** |
  | oblique + friction (shear history, slip, torque) | 251 | `max|Δv| = 1.11e-16 m/s`, `max|Δω| = 5.68e-14 rad/s` (≈1–3 ulp) |
  | bulk bed, 354 pebbles, `D/d = 6` | settled state | `φ = 0.5571` vs `0.5582` — **0.20 %** |

  Measured at this entry: **107 tests pass** (102 unit + 5 cross-code), plus one
  `#[ignore]`d 210 s bulk test. Full methodology and results:
  [`docs/verification-and-validation.md`](docs/verification-and-validation.md).

  **Three defects were found in the process** (all documented in that file):
  `Particle::integrate` is not symplectic despite its doc comment claiming it
  was — an elastic collision rebounds with restitution `1.0031` at `dt = 1 µs`,
  i.e. it manufactures energy — and `contact.rs` diverges from upstream on the
  contact-radius lever arm and the tangential-damping branch, on top of having
  no shear history at all; and `rolling.rs`'s constant-torque model uses the
  damped `|F_n|` where upstream CDT uses the elastic `k_n·δ_n` (21 % apart in
  the unit-test configuration) and keeps the torsion component upstream drops.

  **What this still does NOT establish.** It shows this crate reproduces
  LIGGGHTS. It does **not** show LIGGGHTS' granular physics is right for an
  HTR-10 bed: there is still **no experimental comparison** anywhere in this
  repository. For reference, the settled voidage both codes produce
  (`ε ≈ 0.442`) sits 2.2 percentage points above the Dixon (1988) correlation
  for `D/d = 6` (`ε = 0.4198`) — an observation, not a validation, and no
  calibration of `µ`/`e`/`E` against bed data has been attempted.

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

Modules: `bonded`, `boundary`, `contact`, `coupling`, `granular`,
`granular_system`, `integrator`, `mesh_wall`, `particle`, `rolling`,
`simulation`, `thermal`, `thermal_radiation`, `timestep`.

**Which engine to use.** There are two, deliberately:

- **`granular` + `granular_system`** — the LIGGGHTS-faithful path: shear
  history, upstream contact kinematics, kick-drift-kick velocity-Verlet. **Use
  this for anything that must settle, pack, or hold a static assembly**, i.e.
  every pebble-bed case. It is the path verified against upstream.
- **`contact` + `simulation`** — the original stateless path. No tangential
  spring (`ξ_t = 0` hard-coded), so a static assembly cannot carry shear and a
  heap has zero angle of repose. Kept for the instantaneous-force queries and
  constant-force cases it was written and tested for.

`integrator::VelocityVerlet` is a translation of `fix_nve_sphere.cpp` and is
what `granular_system` runs; `Particle::integrate` is **not** velocity-Verlet
(see its doc comment and the V&V document) and should not drive contacts.

`timestep` ports `fix check/timestep/gran` — use `TimestepEstimate::
recommended_dt` to pick `dt` rather than guessing; an over-long explicit step
does not merely lose accuracy, it ejects particles.

## Licensing

LIGGGHTS-PUBLIC is GPL-2-or-later (GPL-3-compatible; see `NOTICE`). This
translation is distributed under `GPL-3.0-only`, the same as the rest of the
workspace.
