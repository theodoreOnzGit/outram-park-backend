<!--
PROVENANCE / AI-ASSISTED EXTRACTION NOTICE
==========================================
Source : Xin Wang, "Coupled neutronics and thermal-hydraulics modeling for
         pebble-bed Fluoride-Salt-Cooled, High-Temperature Reactor (FHR)",
         Ph.D. dissertation, UC Berkeley, 2018.
         https://escholarship.org/uc/item/40q3985m  (open literature)
AI-ASSISTED extraction + OUTRAM PARK design mapping. The mapping from Wang's
tools (Serpent + COMSOL + PyRK) onto the OUTRAM PARK crates is a DESIGN DECISION
of this project, not a claim from the dissertation. UNVERIFIED until the pipeline
is actually run; treat as a plan. Check the source before relying on any thesis
detail cited here.
-->

# The njoy → openmc → genfoam workflow (OUTRAM PARK re-implementation)

Wang's full-core coupled model is a **Serpent → COMSOL** pipeline: Serpent (Monte
Carlo) generates homogenised group constants, and COMSOL solves the multi-group
diffusion / $SP_3$ neutronics coupled to a porous-media TH model through its
user-PDE interface. PyRK provides the 0-D reflector-corrected point kinetics.

OUTRAM PARK reproduces this on an all-Rust, open-source stack. **The tool mapping
is deliberate, not literal** — none of Wang's codes are reused; the OUTRAM PARK
crates re-implement the *methods*:

| Stage | Wang (2018) | OUTRAM PARK crate(s) | Bead |
|---|---|---|---|
| Nuclear data / MGXS | ENDF/B-VII.0 + Serpent tallies (Eq. 2.23) | `njoy-outram-park-fork` (data) + `outram-mc-libs` (tally) | op-fr2.2.2, op-cjw.24 |
| MC geometry + tallies | Serpent "FIG" input generator | `outram-mc-libs` (CSG/mesh, k-eigenvalue, MGXS tally) | op-fr2.2.3 |
| Deterministic neutronics ($SP_3$) | COMSOL General-Form PDE (App. D) | `outram-foam-appbuilder-lib::genfoam::neutronics::sp3` | op-fr2.2.4 |
| Porous-media TH | COMSOL porous-media CFD | `outram-foam-appbuilder-lib::genfoam` TH + `multi_region` | op-fr2.2.4 |
| 0-D point kinetics | PyRK (reflector-corrected) | `teh-o-prke` | (available) |
| Coupling driver | COMSOL LiveLink | `nee_soon::xin_wang_sp3_workflow` | op-fr2.2 |

The `nee_soon` crate is the **coupling layer**: it composes these public APIs into
one driver. It does not re-implement nuclear data, transport, or FV kernels.

## Stage 1 — MGXS generation (njoy)

**Goal.** Produce cell-homogenised **8-group** macroscopic cross sections for each
Mk1 material region, parametrised for feedback, from ENDF/B-VII.0.

- Group structure: the 8-group boundaries of Table 3.4 (see
  [`04-transients-fig4-29.md`](04-transients-fig4-29.md)).
- Quantities: $\Sigma_{t,g}$, $\Sigma_{s,g'\to g}$ (with $P_1/P_3$ scatter moments
  for the $SP_3$ $D_{1g}/D_{2g}$), $\nu\Sigma_{f,g}$, $\chi_g$ (prompt + delayed),
  diffusion coefficient $D_g$, group speeds $v_g$, and the 6 delayed-group
  ($\beta_i,\lambda_i$) data.
- Feedback parametrisation: linear-in-density for flibe (Eq. 2.25), linear-in-log
  for fuel temperature (Eq. 2.26).

**OUTRAM PARK status.** `njoy-outram-park-fork` owns all nuclear-data / cross-
section code. It does **not yet** expose a public multigroup-XS export entry point
for deterministic consumers — that API is requested in **op-cjw.24** (against the
njoy epic). Stage 1 in `nee_soon` scaffolds the *call*, not the njoy internals.
The flux weighting itself (Eq. 2.23) is a Monte Carlo tally and is produced in
Stage 2; njoy supplies the pointwise / GROUPR-style constants underneath.

## Stage 2 — Mesh + Monte Carlo (openmc / outram-mc)

**Goal.** Build the Mk1 PB-FHR Monte Carlo reference model and generate the MGXS +
power-by-burnup fractions the deterministic model consumes.

- Geometry: annular pebble-bed core — center graphite reflector (inner radius
  35 cm), active fuel-pebble region, 20 cm graphite blanket-pebble ring, outer
  reflector, core barrel / downcomer / vessel (Mk1 Tables 4.1, 4.4, 4.8/4.9 and
  Appendix C). Pebbles as an FCC lattice, packing 60%, 3 cm pebbles, 4730
  TRISO/pebble.
- Tallies: `RegularMesh` + energy-bin flux/reaction-rate tallies → the 8-group
  MGXS (Eq. 2.23); the per-burnup power fraction (Fig. 4.7) for equilibrium fuel.
- Reference: Wang runs Serpent full-core at $10^4$ particles $\times\,10^4$ cycles
  (500 skipped), ENDF/B-VII.0 — the code-to-code reference for verification.

**OUTRAM PARK status.** `outram-mc-libs` is data-free and pulls cross sections
from `njoy-outram-park-fork`. Multigroup transport mode + `MGXSLibrary`
(op-6tz.15) and `RegularMesh`/`MeshFilter` tallies (op-6tz.13) are the enabling
capabilities; Stage 2 (op-fr2.2.3) is **blocked** on them. Compose the public API
only — do not add MGXS internals to `outram-mc-libs` from `nee_soon`.

## Stage 3 — $SP_3$ multiphysics (genfoam)

**Goal.** Drive the GeN-Foam $SP_3$ neutronics coupled to porous-media TH + the
multi-scale fuel model over the control-rod-removal transient.

- Neutronics: `outram_foam_appbuilder_lib::genfoam::neutronics::sp3::Sp3Neutronics`
  (8-group $SP_3$, two moment fields; see the moment-field mapping note in
  [`02-methodology-sp3.md`](02-methodology-sp3.md)). Build with
  `Sp3Neutronics::with_cross_sections(...)`, run `solve_eigenvalue()` for the
  steady state then `step(dt)` for the transient; or wrap in
  `NeutronicsModel::Sp3(..)` for the shared `power()`/`k_eff()`/`kind()` surface.
- TH: GeN-Foam porous-media energy + Ergun/Wakao closures (Stage-1 Mk1 values:
  $E_1=150,\ E_2=1.75,\ c_F=0.52$).
- Feedback: multi-scale 1-D spherical conduction inside pebble/TRISO layers feeds
  $T_{fuel}$ (Doppler) and flibe density back into the MGXS parametrisation.
- Coupling: the `multi_region` outer Picard loop exchanges $T_{fuel}$, $T_{struct}$,
  coolant density, and power density between regions.

**OUTRAM PARK status.** This stage (op-fr2.2.4) is **blocked** on two in-progress
items in the parallel GeN-Foam port:

1. **The $SP_3$ solver port itself** (`Sp3Neutronics` eigenvalue/transient
   solvers + boundary handling / benchmark) — tracked by op-p6p.15. Today
   `Sp3Neutronics::new(..)` is a state-only scaffold that returns
   `ModelNotImplemented(Sp3)` from its solvers.
2. **Mesh-based neutronics coupling dispatch** — `multi_region::outer_iteration`'s
   `RegionModel` enum has **no `Sp3` variant yet**; mesh-based neutronics coupling
   through `MultiPhysicsSolver` is "wired-in-waiting" (op-p6p.8.4). Until that
   lands, SP3 must be driven directly via `Sp3Neutronics`, not through the
   coupling loop.

## Stage 4 — Validation against Figure 4.29

**Goal.** Reproduce the Mk1 **maximum fuel temperature during a control-rod-
removal transient** (Fig. 4.29) and compare to the digitised reference curve.

- Reference: the digitised Fig. 4.27–4.29 tables in
  [`04-transients-fig4-29.md`](04-transients-fig4-29.md).
- Nature of the check: Wang's own reference is a **code-to-code** result
  (Serpent + COMSOL), and PB-FHR has **no experimental data**. So this is
  code-to-code **verification**, not experimental validation — state that plainly
  in the V&V write-up (workspace V&V rule: methodology + measured numbers with
  uncertainty).

**OUTRAM PARK status.** op-fr2.2.5, depends on Stages 1–3. Scaffolded, not run.

## Honest scope statement

This whole pipeline is presently a **scaffold**. No MGXS has been generated, no MC
model built, no $SP_3$ transient run, and Fig. 4.29 has **not** been reproduced.
The scaffold exists so the coupling surface compiles and each stage has a bead and
a documented placeholder. Reproducing Fig. 4.29 is the beaded next step and
depends on capabilities still being built in `njoy-outram-park-fork`,
`outram-mc-libs`, and the GeN-Foam port.
