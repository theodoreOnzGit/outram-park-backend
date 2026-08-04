# outram-park-fork-moltres

MSR neutronics + thermal-hydraulics on the outram-foam finite-volume layer — physics formulation from the LGPL-2.1 [Moltres](https://github.com/arfc/moltres) code, reimplemented on `outram-foam-basic-lib` rather than MOOSE/PETSc.

Circulating-fuel molten-salt reactor multiphysics: multigroup neutron diffusion, **delayed-neutron precursor drift** (the defining MSRE effect — precursors advected out of the core by the flowing fuel salt), and salt thermal-hydraulics, coupled on the OUTRAM PARK finite-volume mesh. Moltres is MOOSE/finite-element; this is an independent finite-volume reimplementation of the same validated formulation (no MOOSE/PETSc/MPI, per the workspace no-FFI rule).

> **⚠️ Untrusted AI-assisted draft — no human V&V.** First-pass physics
> implemented and machine-verified against analytic/limiting cases only, under
> the MSRE digital-twin epic (`op-6w0`). Independent OUTRAM PARK fork; not
> affiliated with the upstream project. Not for nuclear facility operation,
> reactor control, safety-critical, or licensing decisions.

## What is implemented (first pass, 2026-08-04)

- **`materials`** — SI multigroup cross-section records (`MsrMaterial`,
  `DelayedFamily` with the Keepin U-235 6-family set) materialised to per-cell
  fields (`XsFields`); reduced linear temperature feedback on the removal
  cross section.
- **`ring_mesh`** — a closed 1-D loop `FvMesh` (`RingMesh`): periodic topology
  built as a ring of internal faces (no cyclic boundary machinery), cells laid
  on a circle so every adjacent-cell distance is uniform, plus a prescribed
  slug-flow face flux.
- **`diffusion`** — static-fuel multigroup diffusion k-eigenvalue
  (`StaticDiffusion`): power iteration over `fvm::laplacian + fvm::sp` group
  systems with warm-started CG.
- **`precursors`** — delayed-neutron precursor advection–decay drift
  (`PrecursorDrift`): steady solve and backward-Euler transient step of
  `dC_i/dt + div(u C_i) - div(D_C grad C_i) = beta_i/k S_f - lambda_i C_i`,
  assembled with upwind `fvm::div`.
- **`circulating`** — the coupled flux + drifting-precursor eigenvalue on the
  closed loop (`CirculatingFuelSolver`): reproduces the static solver exactly
  at `u = 0` and the MSRE circulating-fuel reactivity loss at `u > 0`.
- **`thermal`** — reduced slug-flow salt temperature with heat-exchanger sink
  (`SaltThermalModel`) and the Picard-coupled power/temperature-feedback loop
  (`CoupledMsrSolver`).

Everything is SI (metres); convert cm-based reactor-physics tables on input.
The salt velocity is **prescribed** (rigid loop circulation) — no CFD in this
crate by design; that is the `outram-foam-appbuilder-lib`/GeN-Foam path.

## Verification snapshot (measured 2026-08-04, release build)

Automated verification against analytic/limiting references — methodology and
numbers live in each test's doc comment (`src/*.rs`):

| Check | Result |
|---|---|
| 1-group bare-slab k vs analytic | rel. err `6.3e-6` |
| 2-group bare-slab k vs analytic | rel. err `9.5e-7` |
| Zero-flow precursor equilibrium | rel. err `2.3e-16` |
| Closed-loop precursor production = decay | imbalance `<= 8.6e-11` |
| Circulating solver at `u = 0` vs static solver | `dk = 2.2e-16` |
| **Circulation reactivity loss vs loop speed** | 151 / 216 / 287 / 348 / 388 pcm at 0.15 / 0.3 / 0.6 / 1.2 / 2.4 m/s — monotone, `< beta = 650 pcm`, MSRE-order at nominal flow |
| Closed-loop energy balance (HX removal = power) | imbalance `1.1e-8`; slug-flow heat-up matches analytic to 0.03 % |
| Temperature-feedback sign (k vs power) | monotone negative, ~170 pcm/MW at 0.5/4/8 MW |

This is **verification only** (correct implementation of the equations), not
validation against MSRE benchmark data.

## Run the tests

```bash
cargo test -p outram-park-fork-moltres --lib --release
```

## Bookkeeping status

> Maintainer sign-off tracker (see the workspace `CLAUDE.md` "Bookkeeping
> pass" command). A crate is **complete** only once the maintainer has
> personally signed off on BOTH axes below.

| Axis | Status |
|---|---|
| Verification & Validation (V&V) — human-reviewed | ❌ Not yet manually checked |
| Human / user interface — human-reviewed | ❌ Not yet manually checked |

**Status: INCOMPLETE** until both axes are manually checked and cleared by the maintainer.
